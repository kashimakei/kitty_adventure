//! Small seedable random numbers, so a level can be regenerated from its seed.

#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    pub fn pair(&mut self, (lo, hi): (f32, f32)) -> f32 {
        self.range(lo, hi)
    }

    /// Uniform integer in [lo, hi].
    pub fn int(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next_u64() % ((hi - lo + 1) as u64)) as i32
    }

    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }

    pub fn sign(&mut self) -> f32 {
        if self.chance(0.5) { 1.0 } else { -1.0 }
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u64() % items.len() as u64) as usize]
    }
}
