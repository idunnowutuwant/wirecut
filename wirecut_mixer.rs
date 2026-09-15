#[inline(always)]
pub fn wirecut_mix64(mut x: u64) -> u64 {
    x = x.wrapping_mul(0x9E3779B97F4A7C15);
    x ^= x >> 31;
    x = x.wrapping_mul(0xBF58476D1CE4E5B9);
    x ^= x >> 31;
    x
}
