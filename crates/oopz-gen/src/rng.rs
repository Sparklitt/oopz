//! A small seeded random generator (SplitMix64): the same seed gives the same backup on every machine, with no
//! dependency. Not for anything secret.

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// A generator for one part of the world (a chat, a person), independent of how much the others draw.
    pub fn fork(seed: u64, stream: u64) -> Self {
        Self(mix(seed ^ mix(stream.wrapping_add(0x5EED))))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// A number in `0..n` (`n` > 0).
    pub fn below(&mut self, n: u64) -> u64 {
        ((self.next_u64() as u128 * n as u128) >> 64) as u64
    }

    /// A number in `lo..=hi`.
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        lo + self.below(hi - lo + 1)
    }

    /// A number in `[0, 1)`.
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u64) as usize]
    }

    /// An index into `weights`, each chosen in proportion to its weight (at least one must be positive).
    pub fn weighted(&mut self, weights: &[u32]) -> usize {
        let total: u64 = weights.iter().map(|w| *w as u64).sum();
        let mut at = self.below(total);
        for (i, w) in weights.iter().enumerate() {
            if at < *w as u64 {
                return i;
            }
            at -= *w as u64;
        }
        weights.len() - 1
    }

    /// An exponential wait with this mean: the gaps between messages.
    pub fn exp(&mut self, mean: f64) -> f64 {
        -mean * (1.0 - self.unit()).ln()
    }
}

/// SplitMix64's finaliser: a bijection on u64, so distinct inputs give distinct outputs (unique key_ids).
pub fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_numbers_and_ranges_hold() {
        let (mut a, mut b) = (Rng::new(7), Rng::new(7));
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
            let r = a.range(3, 5);
            b.range(3, 5);
            assert!((3..=5).contains(&r));
        }
        assert_eq!(Rng::new(1).weighted(&[0, 0, 4, 0]), 2);
    }
}
