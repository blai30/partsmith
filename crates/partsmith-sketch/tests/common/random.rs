//! A small deterministic pseudo-random generator for fixtures that need a fixed seed.

/// SplitMix64: tiny, fast and fully determined by its seed, so fixtures repeat exactly.
#[derive(Clone, Debug)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator that always produces the same sequence for the same seed.
    pub fn new(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.state;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        // 53 random bits fill the mantissa exactly.
        (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64
    }

    /// Uniform in [low, high).
    pub fn uniform(&mut self, low: f64, high: f64) -> f64 {
        low + (high - low) * self.unit()
    }

    /// Uniform index in 0..length; `length` must be positive.
    pub fn index(&mut self, length: usize) -> usize {
        assert!(length > 0, "cannot pick from an empty range");
        (self.next_u64() % length as u64) as usize
    }

    /// True with the given probability.
    pub fn chance(&mut self, probability: f64) -> bool {
        self.unit() < probability
    }

    /// +1 or -1 with equal probability.
    pub fn sign(&mut self) -> f64 {
        if self.chance(0.5) { 1.0 } else { -1.0 }
    }

    /// One element of a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.index(items.len())]
    }

    /// Fisher-Yates shuffle in place.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let other = self.index(i + 1);
            items.swap(i, other);
        }
    }
}
