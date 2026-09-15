#[inline(always)]
pub fn wirecut_arx64(mut x: u64) -> u64 {
    x ^= x >> 31;
    x ^= x << 18;
    x ^= x << 9;
    x ^= x << 30;
    x ^= x >> 14;
    x ^= x >> 24;
    x = x.rotate_right(33);
    x ^= x >> 27;
    x
}
