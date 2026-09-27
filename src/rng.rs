//! Tiny, fast xorshift64* PRNG. Good enough for visuals, zero dependencies.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // SplitMix64 scrambles the seed so every seed gives its own stream
        // (xorshift needs a non-zero state; zero is the one value it avoids).
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        let mut r = Rng(if z == 0 { 0x9E37_79B9_7F4A_7C15 } else { z });
        for _ in 0..4 {
            r.next_u64();
        }
        r
    }

    /// Seed from the clock and the process id.
    pub fn from_entropy() -> Self {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        Rng::new(t ^ ((std::process::id() as u64) << 32))
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform integer in `0..n`. Returns 0 when `n == 0`.
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        ((self.next_u64() >> 32) * n as u64 >> 32) as usize
    }

    /// Uniform integer in `lo..hi`. Returns `lo` when the range is empty.
    #[inline]
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo) as usize) as i32
    }

    /// Uniform float in `[0, 1)`.
    #[inline]
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// Uniform float in `[lo, hi)`.
    #[inline]
    pub fn rangef(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    /// Random element of a non-empty slice.
    #[inline]
    pub fn pick<'a, T>(&mut self, s: &'a [T]) -> &'a T {
        &s[self.below(s.len())]
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn neighbouring_seeds_differ() {
        let first: Vec<u64> = (0..64).map(|s| Rng::new(s).next_u64()).collect();
        for i in 0..first.len() {
            for j in i + 1..first.len() {
                assert_ne!(first[i], first[j], "seeds {i} and {j} collide");
            }
        }
    }
}
