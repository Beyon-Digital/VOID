//! Deterministic xorshift128+ PRNG (W21/T78).
//!
//! Same seed ⇒ same output on every platform — the sequence is pure
//! integer arithmetic, no floats or OS entropy. Used by every generator
//! so a recorded `seed` in provenance actually reproduces a take.

/// SplitMix64 finalizer — expands one 64-bit seed into the 128-bit
/// xorshift state so single-integer seeds decorrelate well.
fn splitmix64(x: &mut u64) -> u64 {
    *x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[derive(Debug, Clone)]
pub struct XorShift128 {
    s: [u64; 2],
}

impl XorShift128 {
    /// `seed` is the recorded provenance value. A zero seed still works
    /// (state is derived through splitmix64, never all-zero).
    pub fn new(seed: u64) -> Self {
        let mut x = seed;
        let s = [splitmix64(&mut x), splitmix64(&mut x)];
        // splitmix64 never returns two zeroes for a real sequence, but
        // guard anyway — xorshift dies on the all-zero state.
        if s == [0, 0] {
            return Self { s: [0x9E37_79B9_7F4A_7C15, 1] };
        }
        Self { s }
    }

    /// Derive a sibling stream (candidate k of one spec) without
    /// overlapping the parent's sequence space.
    pub fn fork(&self, tag: u64) -> Self {
        Self::new(self.s[0] ^ tag.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ self.s[1])
    }

    pub fn next_u64(&mut self) -> u64 {
        let s1 = self.s[0];
        let s0 = self.s[1];
        self.s[0] = s0;
        let x = s1 ^ (s1 << 23);
        self.s[1] = x ^ s0 ^ (x >> 18) ^ (s0 >> 5);
        self.s[1].wrapping_add(s0)
    }

    /// Uniform f64 in [0, 1) — 53 mantissa bits.
    pub fn next_f64(&mut self) -> f64 {
        const SCALE: f64 = 1.0 / (1u64 << 53) as f64;
        (self.next_u64() >> 11) as f64 * SCALE
    }

    /// Uniform in [0, n) via Lemire's multiply-shift — no modulo bias.
    /// `n == 0` returns 0 rather than panicking (callers bound upstream).
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        ((self.next_u64() as u128 * n as u128) >> 64) as u64
    }

    /// True with probability `ppm / 1_000_000` (parts-per-million — the
    /// codebase's density/intensity convention).
    pub fn chance(&mut self, ppm: u32) -> bool {
        self.below(1_000_000) < u64::from(ppm.min(1_000_000))
    }

    /// Signed jitter in [-span, +span] ticks.
    pub fn jitter(&mut self, span: i64) -> i64 {
        if span <= 0 {
            return 0;
        }
        self.below((2 * span + 1) as u64) as i64 - span
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_stream() {
        let mut a = XorShift128::new(42);
        let mut b = XorShift128::new(42);
        for _ in 0..64 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut c = XorShift128::new(43);
        let mut same = 0usize;
        let mut a = XorShift128::new(42);
        for _ in 0..64 {
            if a.next_u64() == c.next_u64() {
                same += 1;
            }
        }
        assert!(same < 64); // different seed decorrelates
    }

    #[test]
    fn below_is_bounded_and_covers() {
        let mut r = XorShift128::new(7);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            let v = r.below(8);
            assert!(v < 8);
            seen.insert(v);
        }
        assert_eq!(seen.len(), 8);
        assert_eq!(XorShift128::new(1).below(0), 0);
    }

    #[test]
    fn chance_bounds() {
        let mut r = XorShift128::new(3);
        assert!(r.chance(1_000_000));
        let mut hits = 0usize;
        for _ in 0..100_000 {
            if r.chance(100_000) {
                hits += 1;
            }
        }
        assert!((5_000..=15_000).contains(&hits), "hits={hits}");
    }

    #[test]
    fn jitter_within_span() {
        let mut r = XorShift128::new(9);
        for _ in 0..1_000 {
            let j = r.jitter(240_000);
            assert!((-240_000..=240_000).contains(&j));
        }
        assert_eq!(r.jitter(0), 0);
    }
}
