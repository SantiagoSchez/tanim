//! Raw terminal setup/teardown and signal handling, straight on top of libc.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static RUNNING: AtomicBool = AtomicBool::new(true);
static RESIZED: AtomicBool = AtomicBool::new(false);
static ORIG: OnceLock<libc::termios> = OnceLock::new();

const ENTER: &[u8] = b"\x1b[?1049h\x1b[?25l\x1b[?7l\x1b[0m\x1b[2J";
const LEAVE: &[u8] = b"\x1b[0m\x1b[?7h\x1b[?25h\x1b[?1049l";

extern "C" fn on_quit(_: libc::c_int) {
    RUNNING.store(false, Ordering::Relaxed);
}

extern "C" fn on_winch(_: libc::c_int) {
    RESIZED.store(true, Ordering::Relaxed);
}

pub fn running() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

pub fn take_resized() -> bool {
    RESIZED.swap(false, Ordering::Relaxed)
}

pub use tanim::Key;

/// Sleep up to `timeout` while watching stdin, returning early when Enter,
/// a lone Esc, Q, the up/down arrows, WASD or space is pressed. Other keys
/// are swallowed.
pub fn wait_key(timeout: std::time::Duration) -> Key {
    static STDIN_CLOSED: AtomicBool = AtomicBool::new(false);
    if STDIN_CLOSED.load(Ordering::Relaxed) {
        std::thread::sleep(timeout);
        return Key::None;
    }
    let ms = timeout.as_micros().div_ceil(1000).min(i32::MAX as u128) as libc::c_int;
    let mut pfd = libc::pollfd { fd: libc::STDIN_FILENO, events: libc::POLLIN, revents: 0 };
    if unsafe { libc::poll(&mut pfd, 1, ms) } <= 0 {
        return Key::None;
    }
    let mut buf = [0u8; 64];
    let n = unsafe { libc::read(libc::STDIN_FILENO, buf.as_mut_ptr().cast(), buf.len()) };
    if n <= 0 {
        // EOF or error (stdin not interactive): stop polling it.
        STDIN_CLOSED.store(true, Ordering::Relaxed);
        return Key::None;
    }
    let bytes = &buf[..n as usize];
    // Arrow/function keys arrive as ESC-prefixed sequences in a single read;
    // only an ESC with nothing after it is the Esc key.
    let arrow = |c: u8| bytes.windows(3).any(|s| s == [0x1b, b'[', c] || s == [0x1b, b'O', c]);
    // Letters count only outside escape sequences: Q quits like Esc, WASD pan.
    let plain = bytes.first() != Some(&0x1b);
    let letter = |l: u8| plain && bytes.iter().any(|b| b.to_ascii_lowercase() == l);
    if bytes.last() == Some(&0x1b) || letter(b'q') {
        Key::Esc
    } else if arrow(b'A') {
        Key::Up
    } else if arrow(b'B') {
        Key::Down
    } else if letter(b'w') {
        Key::PanUp
    } else if letter(b'a') {
        Key::PanLeft
    } else if letter(b's') {
        Key::PanDown
    } else if letter(b'd') {
        Key::PanRight
    } else if letter(b' ') {
        Key::Space
    } else if bytes.iter().any(|&b| b == b'\n' || b == b'\r') {
        Key::Enter
    } else {
        Key::None
    }
}

pub fn is_tty() -> bool {
    unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 }
}

/// Terminal size in cells, falling back to 80x24.
pub fn size() -> (usize, usize) {
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut ws) == 0
            && ws.ws_col > 0
            && ws.ws_row > 0
        {
            (ws.ws_col as usize, ws.ws_row as usize)
        } else {
            (80, 24)
        }
    }
}

fn restore() {
    if let Some(t) = ORIG.get() {
        unsafe {
            libc::tcflush(libc::STDIN_FILENO, libc::TCIFLUSH);
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, t);
        }
    }
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(LEAVE);
    let _ = out.flush();
}

/// Guard: alternate screen, hidden cursor, no echo. Restores on drop.
pub struct Term;

impl Term {
    pub fn enter() -> Term {
        unsafe {
            let mut t: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(libc::STDIN_FILENO, &mut t) == 0 {
                let _ = ORIG.set(t);
                // Keep ISIG so Ctrl+C still raises SIGINT; stop keys from echoing.
                t.c_lflag &= !(libc::ECHO | libc::ICANON);
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &t);
            }
            let quit = on_quit as extern "C" fn(libc::c_int) as libc::sighandler_t;
            libc::signal(libc::SIGINT, quit);
            libc::signal(libc::SIGTERM, quit);
            libc::signal(libc::SIGHUP, quit);
            libc::signal(
                libc::SIGWINCH,
                on_winch as extern "C" fn(libc::c_int) as libc::sighandler_t,
            );
        }
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            prev(info);
        }));
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(ENTER);
        let _ = out.flush();
        Term
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        restore();
    }
}
