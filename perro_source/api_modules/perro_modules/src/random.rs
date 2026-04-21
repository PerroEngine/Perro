const STREAM_GAMMA: u32 = 0x9e37_79b9;
const FNV1A_OFFSET: u32 = 0x811c_9dc5;
const FNV1A_PRIME: u32 = 0x0100_0193;
const FNV1A_OFFSET64: u64 = 0xcbf2_9ce4_8422_2325;
const FNV1A_PRIME64: u64 = 0x0000_0100_0000_01b3;

pub trait HashToU32 {
    fn hash_to_u32(self) -> u32;
}

pub trait RandRangeValue: Copy {
    fn sample_from_u32(min: Self, max: Self, random: u32) -> Self;
}

#[inline]
pub fn hash<T: HashToU32>(value: T) -> u32 {
    value.hash_to_u32()
}

#[inline]
pub const fn hash_u32(mut value: u32) -> u32 {
    value ^= value >> 16;
    value = value.wrapping_mul(0x7feb_352d);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846c_a68b);
    value ^= value >> 16;
    value
}

#[inline]
pub const fn hash_i32(value: i32) -> u32 {
    hash_u32(value as u32)
}

#[inline]
pub const fn hash_u64(value: u64) -> u32 {
    let lower = value as u32;
    let upper = (value >> 32) as u32;
    hash_u32(lower ^ hash_u32(upper))
}

#[inline]
pub const fn hash_i64(value: i64) -> u32 {
    hash_u64(value as u64)
}

#[inline]
pub const fn hash_u128(value: u128) -> u32 {
    let lower = value as u64;
    let upper = (value >> 64) as u64;
    hash_u32(hash_u64(lower) ^ hash_u64(upper))
}

#[inline]
pub const fn hash_combine(a: u32, b: u32) -> u32 {
    hash_u32(
        a ^ b
            .wrapping_add(0x9e37_79b9)
            .wrapping_add(a << 6)
            .wrapping_add(a >> 2),
    )
}

#[inline]
pub const fn hash_combine3(a: u32, b: u32, c: u32) -> u32 {
    hash_combine(hash_combine(a, b), c)
}

#[inline]
pub const fn hash_combine4(a: u32, b: u32, c: u32, d: u32) -> u32 {
    hash_combine(hash_combine3(a, b, c), d)
}

#[inline]
pub const fn hash2_u32(x: u32, y: u32) -> u32 {
    hash_combine(hash_u32(x), hash_u32(y))
}

#[inline]
pub const fn hash3_u32(x: u32, y: u32, z: u32) -> u32 {
    hash_combine3(hash_u32(x), hash_u32(y), hash_u32(z))
}

#[inline]
pub const fn hash64_u64(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^= value >> 31;
    value
}

#[inline]
pub const fn hash64_u32(value: u32) -> u64 {
    hash64_u64(value as u64)
}

#[inline]
pub const fn hash64_u128(value: u128) -> u64 {
    let lower = value as u64;
    let upper = (value >> 64) as u64;
    hash64_u64(lower ^ hash64_u64(upper))
}

#[inline]
pub const fn hash_bool(value: bool) -> u32 {
    hash_u32(value as u32)
}

#[inline]
pub const fn hash_f32(value: f32) -> u32 {
    hash_u32(value.to_bits())
}

impl HashToU32 for u32 {
    #[inline]
    fn hash_to_u32(self) -> u32 {
        hash_u32(self)
    }
}

impl HashToU32 for i32 {
    #[inline]
    fn hash_to_u32(self) -> u32 {
        hash_i32(self)
    }
}

impl HashToU32 for u64 {
    #[inline]
    fn hash_to_u32(self) -> u32 {
        hash_u64(self)
    }
}

impl HashToU32 for i64 {
    #[inline]
    fn hash_to_u32(self) -> u32 {
        hash_i64(self)
    }
}

impl HashToU32 for u128 {
    #[inline]
    fn hash_to_u32(self) -> u32 {
        hash_u128(self)
    }
}

impl HashToU32 for bool {
    #[inline]
    fn hash_to_u32(self) -> u32 {
        hash_bool(self)
    }
}

impl HashToU32 for f32 {
    #[inline]
    fn hash_to_u32(self) -> u32 {
        hash_f32(self)
    }
}

#[inline]
pub fn hash_bytes(bytes: &[u8]) -> u32 {
    let mut hash = FNV1A_OFFSET;
    for &byte in bytes {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(FNV1A_PRIME);
    }
    hash_u32(hash ^ bytes.len() as u32)
}

#[inline]
pub fn hash_str(value: &str) -> u32 {
    hash_bytes(value.as_bytes())
}

#[inline]
pub fn hash64_bytes(bytes: &[u8]) -> u64 {
    let mut hash = FNV1A_OFFSET64;
    for &byte in bytes {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV1A_PRIME64);
    }
    hash64_u64(hash ^ bytes.len() as u64)
}

#[inline]
pub fn hash64_str(value: &str) -> u64 {
    hash64_bytes(value.as_bytes())
}

#[inline]
pub const fn rand_u32(seed: u32) -> u32 {
    hash_u32(seed)
}

#[inline]
pub fn rand01(seed: u32) -> f32 {
    rand_u32(seed) as f32 / u32::MAX as f32
}

#[inline]
pub fn rand11(seed: u32) -> f32 {
    rand01(seed) * 2.0 - 1.0
}

impl RandRangeValue for f32 {
    #[inline]
    fn sample_from_u32(min: Self, max: Self, random: u32) -> Self {
        if min >= max {
            min
        } else {
            min + (max - min) * (random as f32 / u32::MAX as f32)
        }
    }
}

impl RandRangeValue for u32 {
    #[inline]
    fn sample_from_u32(min: Self, max: Self, random: u32) -> Self {
        if min >= max {
            return min;
        }

        let span = (max - min) as u64;
        min + (((random as u64).wrapping_mul(span) >> 32) as u32)
    }
}

impl RandRangeValue for i32 {
    #[inline]
    fn sample_from_u32(min: Self, max: Self, random: u32) -> Self {
        if min >= max {
            return min;
        }

        let span = (max as i64 - min as i64) as u64;
        let offset = (((random as u64).wrapping_mul(span)) >> 32) as i64;
        (min as i64 + offset) as i32
    }
}

#[inline]
pub fn rand_range<T: RandRangeValue>(min: T, max: T, seed: u32) -> T {
    T::sample_from_u32(min, max, rand_u32(seed))
}

#[inline]
pub fn rand_range_f32(min: f32, max: f32, seed: u32) -> f32 {
    rand_range(min, max, seed)
}

#[inline]
pub fn rand_range_u32(min: u32, max: u32, seed: u32) -> u32 {
    rand_range(min, max, seed)
}

#[inline]
pub fn rand_range_i32(min: i32, max: i32, seed: u32) -> i32 {
    rand_range(min, max, seed)
}

#[inline]
pub fn chance(probability: f32, seed: u32) -> bool {
    let p = probability.clamp(0.0, 1.0);
    rand01(seed) < p
}

#[inline]
pub fn choose_index(len: usize, seed: u32) -> Option<usize> {
    if len == 0 {
        return None;
    }

    if len <= u32::MAX as usize {
        Some(rand_range_u32(0, len as u32, seed) as usize)
    } else {
        Some((hash64_u64(seed as u64) % len as u64) as usize)
    }
}

#[inline]
pub const fn rand_u32_stream(seed: u32, index: u32) -> u32 {
    hash_u32(seed.wrapping_add(index.wrapping_mul(STREAM_GAMMA)))
}

#[inline]
pub fn rand01_stream(seed: u32, index: u32) -> f32 {
    rand_u32_stream(seed, index) as f32 / u32::MAX as f32
}

#[inline]
pub fn rand11_stream(seed: u32, index: u32) -> f32 {
    rand01_stream(seed, index) * 2.0 - 1.0
}

#[inline]
pub fn rand_unit_vec2(seed: u32) -> (f32, f32) {
    let angle = rand01(seed) * std::f32::consts::TAU;
    (angle.cos(), angle.sin())
}

#[inline]
pub fn rand_in_circle(seed: u32) -> (f32, f32) {
    let angle = rand01(seed) * std::f32::consts::TAU;
    let radius = rand01(seed.wrapping_add(STREAM_GAMMA)).sqrt();
    (radius * angle.cos(), radius * angle.sin())
}

#[inline]
pub fn rand_unit_vec3(seed: u32) -> (f32, f32, f32) {
    let z = rand11(seed);
    let angle = rand01(seed.wrapping_add(STREAM_GAMMA)) * std::f32::consts::TAU;
    let radial = (1.0 - z * z).max(0.0).sqrt();
    (radial * angle.cos(), radial * angle.sin(), z)
}

#[inline]
pub fn shuffle<T>(seed: u32, values: &mut [T]) {
    if values.len() < 2 {
        return;
    }

    let mut rng = SeededRng::new(seed);
    for i in (1..values.len()).rev() {
        let j = ((rng.next_u32() as u64 * (i as u64 + 1)) >> 32) as usize;
        values.swap(i, j);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeededRng {
    state: u32,
}

impl SeededRng {
    #[inline]
    pub const fn new(seed: u32) -> Self {
        Self { state: seed }
    }

    #[inline]
    pub const fn seed(&self) -> u32 {
        self.state
    }

    #[inline]
    pub fn reseed(&mut self, seed: u32) {
        self.state = seed;
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(STREAM_GAMMA);
        hash_u32(self.state)
    }

    #[inline]
    pub fn next_01(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    #[inline]
    pub fn next_11(&mut self) -> f32 {
        self.next_01() * 2.0 - 1.0
    }

    #[inline]
    pub fn next_range<T: RandRangeValue>(&mut self, min: T, max: T) -> T {
        rand_range(min, max, self.next_u32())
    }

    #[inline]
    pub fn next_range_f32(&mut self, min: f32, max: f32) -> f32 {
        self.next_range(min, max)
    }

    #[inline]
    pub fn next_range_u32(&mut self, min: u32, max: u32) -> u32 {
        self.next_range(min, max)
    }

    #[inline]
    pub fn next_range_i32(&mut self, min: i32, max: i32) -> i32 {
        self.next_range(min, max)
    }

    #[inline]
    pub fn next_chance(&mut self, probability: f32) -> bool {
        chance(probability, self.next_u32())
    }

    #[inline]
    pub fn next_index(&mut self, len: usize) -> Option<usize> {
        choose_index(len, self.next_u32())
    }
}

#[cfg(test)]
#[path = "../tests/unit/random_tests.rs"]
mod tests;
