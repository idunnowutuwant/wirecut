#![no_std]

#[inline(always)]
pub fn wirecut_mix64_keyed(mut x: u64, keys: &[u64; 2]) -> u64 {
    x = x.wrapping_mul(0x77BEB5B165DFB6BB);
    x ^= x >> 33;
    x = x.wrapping_mul(0x00A3C9A3A955C959);
    x ^= x >> 33;
    x
}

#[inline(always)]
pub fn wirecut_mix64x4_batch(mut v: [u64; 4], keys: &[u64; 2]) -> [u64; 4] {
    v[0] = wirecut_mix64_keyed(v[0], keys);
    v[1] = wirecut_mix64_keyed(v[1], keys);
    v[2] = wirecut_mix64_keyed(v[2], keys);
    v[3] = wirecut_mix64_keyed(v[3], keys);
    v
}

#[inline(always)]
pub fn wirecut_mix64x8_batch(mut v: [u64; 8], keys: &[u64; 2]) -> [u64; 8] {
    for i in 0..8 { v[i] = wirecut_mix64_keyed(v[i], keys); }
    v
}

pub struct WirecutHasher {
    state: u64,
    keys: [u64; 2],
}

impl WirecutHasher {
    #[inline(always)]
    pub const fn new(k0: u64, k1: u64) -> Self {
        Self { state: k0 ^ 0x517CC1B727220A95, keys: [k0, k1] }
    }
}

impl core::hash::Hasher for WirecutHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        wirecut_mix64_keyed(self.state ^ self.keys[1], &self.keys)
    }
    #[inline(always)]
    fn write(&mut self, mut bytes: &[u8]) {
        while bytes.len() >= 8 {
            let val = u64::from_le_bytes(bytes[..8].try_into().unwrap());
            self.state ^= val;
            self.state = wirecut_mix64_keyed(self.state, &self.keys);
            bytes = &bytes[8..];
        }
        if !bytes.is_empty() {
            let mut buf = [0u8; 8];
            buf[..bytes.len()].copy_from_slice(bytes);
            self.state ^= u64::from_le_bytes(buf);
            self.state = wirecut_mix64_keyed(self.state, &self.keys);
        }
    }
}
