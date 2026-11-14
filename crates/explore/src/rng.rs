//! Deterministic PRNG for reproducible exploration runs.

/// Simple 64-bit xorshift PRNG with no external dependencies.
#[derive(Debug, Clone)]
pub struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    /// Creates a new generator from `seed`. Zero is mapped to a non-zero constant.
    pub fn new(seed: u64) -> Self {
        let state = if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        };
        Self { state }
    }

    /// Returns the next pseudo-random `u64`.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Returns a value in `0..upper` (or `0` when `upper == 0`).
    pub fn next_usize(&mut self, upper: usize) -> usize {
        if upper == 0 {
            return 0;
        }
        (self.next_u64() % upper as u64) as usize
    }

    /// Picks a random element from `slice`, returning `None` if empty.
    pub fn choose<'a, T>(&mut self, slice: &'a [T]) -> Option<&'a T> {
        if slice.is_empty() {
            None
        } else {
            let idx = self.next_usize(slice.len());
            Some(&slice[idx])
        }
    }

    /// Returns `true` with approximate probability `numerator / denominator`.
    pub fn chance(&mut self, numerator: u64, denominator: u64) -> bool {
        if denominator == 0 || numerator == 0 {
            return false;
        }
        if numerator >= denominator {
            return true;
        }
        (self.next_u64() % denominator) < numerator
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_zero_produces_nonzero_sequence() {
        let mut rng = Xorshift64::new(0);
        let a = rng.next_u64();
        let b = rng.next_u64();
        assert_ne!(a, 0);
        assert_ne!(a, b);
    }

    #[test]
    fn same_seed_replays_identically() {
        let mut r1 = Xorshift64::new(42);
        let mut r2 = Xorshift64::new(42);
        for _ in 0..20 {
            assert_eq!(r1.next_u64(), r2.next_u64());
        }
    }

    #[test]
    fn bounds_and_choice_work() {
        let mut rng = Xorshift64::new(7);
        assert_eq!(rng.next_usize(0), 0);
        assert!(rng.choose::<i32>(&[]).is_none());
        for _ in 0..50 {
            assert!(rng.next_usize(5) < 5);
        }
    }
}
