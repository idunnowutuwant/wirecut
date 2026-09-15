#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicroOp {
    MulOdd(u64),
    AddConst(u64),
    XorConst(u64),
    Ror(u8),
    XorShr(u8),
    XorShl(u8),
    KeyXor(usize),
}

impl MicroOp {
    pub fn type_index(&self) -> usize {
        match self {
            MicroOp::MulOdd(_) => 0,
            MicroOp::XorShr(_) => 1,
            MicroOp::Ror(_) => 2,
            MicroOp::AddConst(_) => 3,
            MicroOp::XorConst(_) => 4,
            MicroOp::KeyXor(_) => 5,
            MicroOp::XorShl(_) => 6,
        }
    }
}

#[derive(Clone, Debug)]
pub struct KernelAST {
    pub ops: Vec<MicroOp>,
}

impl KernelAST {
    pub fn new(ops: Vec<MicroOp>) -> Self {
        Self { ops }
    }

    #[inline(always)]
    pub fn eval_keyed(&self, mut x: u64, keys: &[u64; 2]) -> u64 {
        for &op in &self.ops {
            match op {
                MicroOp::MulOdd(c) => x = x.wrapping_mul(c | 1),
                MicroOp::AddConst(c) => x = x.wrapping_add(c),
                MicroOp::XorConst(c) => x ^= c,
                MicroOp::Ror(k) => x = x.rotate_right(k as u32),
                MicroOp::XorShr(k) => x ^= x >> k,
                MicroOp::XorShl(k) => x ^= x << k,
                MicroOp::KeyXor(idx) => x ^= keys[idx & 1],
            }
        }
        x
    }

    #[inline(always)]
    pub fn eval(&self, x: u64) -> u64 {
        self.eval_keyed(x, &[0, 0])
    }

    pub fn invert_keyed(&self, mut x: u64, keys: &[u64; 2]) -> u64 {
        for &op in self.ops.iter().rev() {
            match op {
                MicroOp::MulOdd(c) => {
                    let odd = c | 1;
                    let mut inv = odd;
                    for _ in 0..5 {
                        inv = inv.wrapping_mul(2u64.wrapping_sub(odd.wrapping_mul(inv)));
                    }
                    x = x.wrapping_mul(inv);
                }
                MicroOp::AddConst(c) => x = x.wrapping_sub(c),
                MicroOp::XorConst(c) => x ^= c,
                MicroOp::Ror(k) => x = x.rotate_left(k as u32),
                MicroOp::XorShr(k) => {
                    let mut orig = 0u64;
                    for i in (0..64).rev() {
                        let bit_y = (x >> i) & 1;
                        let bit_x_k = if i + (k as usize) < 64 { (orig >> (i + k as usize)) & 1 } else { 0 };
                        orig |= (bit_y ^ bit_x_k) << i;
                    }
                    x = orig;
                }
                MicroOp::XorShl(k) => {
                    let mut orig = 0u64;
                    for i in 0..64 {
                        let bit_y = (x >> i) & 1;
                        let bit_x_k = if i >= (k as usize) { (orig >> (i - k as usize)) & 1 } else { 0 };
                        orig |= (bit_y ^ bit_x_k) << i;
                    }
                    x = orig;
                }
                MicroOp::KeyXor(idx) => x ^= keys[idx & 1],
            }
        }
        x
    }

    pub fn emit_c(&self) -> String {
        let mut s = String::new();
        s.push_str("#ifndef WIRECUT_MIXER_H\n#define WIRECUT_MIXER_H\n\n");
        s.push_str("#include <stdint.h>\n#include <stddef.h>\n#include <immintrin.h>\n\n");

        s.push_str("static inline uint64_t wirecut_mix64_keyed(uint64_t x, const uint64_t k[2]) {\n");
        for op in &self.ops {
            match op {
                MicroOp::MulOdd(c) => s.push_str(&format!("    x *= UINT64_C({:#018X});\n", c | 1)),
                MicroOp::AddConst(c) => s.push_str(&format!("    x += UINT64_C({:#018X});\n", c)),
                MicroOp::XorConst(c) => s.push_str(&format!("    x ^= UINT64_C({:#018X});\n", c)),
                MicroOp::Ror(k) => s.push_str(&format!("    x = (x >> {}) | (x << (64 - {}));\n", k, k)),
                MicroOp::XorShr(k) => s.push_str(&format!("    x ^= (x >> {});\n", k)),
                MicroOp::XorShl(k) => s.push_str(&format!("    x ^= (x << {});\n", k)),
                MicroOp::KeyXor(idx) => s.push_str(&format!("    x ^= k[{}];\n", idx & 1)),
            }
        }
        s.push_str("    return x;\n}\n\n");

        s.push_str("#if defined(__AVX2__)\n");
        s.push_str("static inline __m256i wirecut_mix64x4_avx2(__m256i v, const uint64_t k[2]) {\n");
        s.push_str("    __m256i k0 = _mm256_set1_epi64x(k[0]);\n");
        s.push_str("    __m256i k1 = _mm256_set1_epi64x(k[1]);\n");
        for op in &self.ops {
            match op {
                MicroOp::MulOdd(c) => {
                    let odd = c | 1;
                    s.push_str(&format!("    {{\n        __m256i c = _mm256_set1_epi64x(UINT64_C({:#018X}));\n", odd));
                    s.push_str("        __m256i lo = _mm256_mul_epu32(v, c);\n");
                    s.push_str("        __m256i hi = _mm256_mul_epu32(_mm256_srli_epi64(v, 32), c);\n");
                    s.push_str("        v = _mm256_add_epi64(lo, _mm256_slli_epi64(hi, 32));\n    }\n");
                }
                MicroOp::AddConst(c) => s.push_str(&format!("    v = _mm256_add_epi64(v, _mm256_set1_epi64x(UINT64_C({:#018X})));\n", c)),
                MicroOp::XorConst(c) => s.push_str(&format!("    v = _mm256_xor_si256(v, _mm256_set1_epi64x(UINT64_C({:#018X})));\n", c)),
                MicroOp::Ror(k) => s.push_str(&format!("    v = _mm256_or_si256(_mm256_srli_epi64(v, {}), _mm256_slli_epi64(v, 64 - {}));\n", k, k)),
                MicroOp::XorShr(k) => s.push_str(&format!("    v = _mm256_xor_si256(v, _mm256_srli_epi64(v, {}));\n", k)),
                MicroOp::XorShl(k) => s.push_str(&format!("    v = _mm256_xor_si256(v, _mm256_slli_epi64(v, {}));\n", k)),
                MicroOp::KeyXor(idx) => s.push_str(&format!("    v = _mm256_xor_si256(v, k{});\n", idx & 1)),
            }
        }
        s.push_str("    return v;\n}\n#endif\n\n");

        s.push_str("#if defined(__AVX512F__)\n");
        s.push_str("static inline __m512i wirecut_mix64x8_avx512(__m512i v, const uint64_t k[2]) {\n");
        s.push_str("    __m512i k0 = _mm512_set1_epi64(k[0]);\n");
        s.push_str("    __m512i k1 = _mm512_set1_epi64(k[1]);\n");
        for op in &self.ops {
            match op {
                MicroOp::MulOdd(c) => {
                    let odd = c | 1;
                    s.push_str(&format!("    {{\n        __m512i c = _mm512_set1_epi64(UINT64_C({:#018X}));\n", odd));
                    s.push_str("#if defined(__AVX512DQ__)\n");
                    s.push_str("        v = _mm512_mullo_epi64(v, c);\n");
                    s.push_str("#else\n");
                    s.push_str("        __m512i lo = _mm512_mul_epu32(v, c);\n");
                    s.push_str("        __m512i hi = _mm512_mul_epu32(_mm512_srli_epi64(v, 32), c);\n");
                    s.push_str("        v = _mm512_add_epi64(lo, _mm512_slli_epi64(hi, 32));\n");
                    s.push_str("#endif\n    }\n");
                }
                MicroOp::AddConst(c) => s.push_str(&format!("    v = _mm512_add_epi64(v, _mm512_set1_epi64(UINT64_C({:#018X})));\n", c)),
                MicroOp::XorConst(c) => s.push_str(&format!("    v = _mm512_xor_si512(v, _mm512_set1_epi64(UINT64_C({:#018X})));\n", c)),
                MicroOp::Ror(k) => s.push_str(&format!("    v = _mm512_or_si512(_mm512_srli_epi64(v, {}), _mm512_slli_epi64(v, 64 - {}));\n", k, k)),
                MicroOp::XorShr(k) => s.push_str(&format!("    v = _mm512_xor_si512(v, _mm512_srli_epi64(v, {}));\n", k)),
                MicroOp::XorShl(k) => s.push_str(&format!("    v = _mm512_xor_si512(v, _mm512_slli_epi64(v, {}));\n", k)),
                MicroOp::KeyXor(idx) => s.push_str(&format!("    v = _mm512_xor_si512(v, k{});\n", idx & 1)),
            }
        }
        s.push_str("    return v;\n}\n#endif\n\n#endif\n");
        s
    }

    pub fn emit_rust(&self) -> String {
        let mut s = String::new();
        s.push_str("#![no_std]\n\n#[inline(always)]\npub fn wirecut_mix64_keyed(mut x: u64, keys: &[u64; 2]) -> u64 {\n");
        for op in &self.ops {
            match op {
                MicroOp::MulOdd(c) => s.push_str(&format!("    x = x.wrapping_mul({:#018X});\n", c | 1)),
                MicroOp::AddConst(c) => s.push_str(&format!("    x = x.wrapping_add({:#018X});\n", c)),
                MicroOp::XorConst(c) => s.push_str(&format!("    x ^= {:#018X};\n", c)),
                MicroOp::Ror(k) => s.push_str(&format!("    x = x.rotate_right({});\n", k)),
                MicroOp::XorShr(k) => s.push_str(&format!("    x ^= x >> {};\n", k)),
                MicroOp::XorShl(k) => s.push_str(&format!("    x ^= x << {};\n", k)),
                MicroOp::KeyXor(idx) => s.push_str(&format!("    x ^= keys[{}];\n", idx & 1)),
            }
        }
        s.push_str("    x\n}\n\n");

        s.push_str("#[inline(always)]\npub fn wirecut_mix64x4_batch(mut v: [u64; 4], keys: &[u64; 2]) -> [u64; 4] {\n");
        s.push_str("    v[0] = wirecut_mix64_keyed(v[0], keys);\n");
        s.push_str("    v[1] = wirecut_mix64_keyed(v[1], keys);\n");
        s.push_str("    v[2] = wirecut_mix64_keyed(v[2], keys);\n");
        s.push_str("    v[3] = wirecut_mix64_keyed(v[3], keys);\n");
        s.push_str("    v\n}\n\n");

        s.push_str("#[inline(always)]\npub fn wirecut_mix64x8_batch(mut v: [u64; 8], keys: &[u64; 2]) -> [u64; 8] {\n");
        s.push_str("    for i in 0..8 { v[i] = wirecut_mix64_keyed(v[i], keys); }\n");
        s.push_str("    v\n}\n\n");

        s.push_str("pub struct WirecutHasher {\n    state: u64,\n    keys: [u64; 2],\n}\n\n");
        s.push_str("impl WirecutHasher {\n    #[inline(always)]\n    pub const fn new(k0: u64, k1: u64) -> Self {\n        Self { state: k0 ^ 0x517CC1B727220A95, keys: [k0, k1] }\n    }\n}\n\n");
        s.push_str("impl core::hash::Hasher for WirecutHasher {\n");
        s.push_str("    #[inline(always)]\n    fn finish(&self) -> u64 {\n        wirecut_mix64_keyed(self.state ^ self.keys[1], &self.keys)\n    }\n");
        s.push_str("    #[inline(always)]\n    fn write(&mut self, mut bytes: &[u8]) {\n");
        s.push_str("        while bytes.len() >= 8 {\n");
        s.push_str("            let val = u64::from_le_bytes(bytes[..8].try_into().unwrap());\n");
        s.push_str("            self.state ^= val;\n");
        s.push_str("            self.state = wirecut_mix64_keyed(self.state, &self.keys);\n");
        s.push_str("            bytes = &bytes[8..];\n");
        s.push_str("        }\n");
        s.push_str("        if !bytes.is_empty() {\n");
        s.push_str("            let mut buf = [0u8; 8];\n");
        s.push_str("            buf[..bytes.len()].copy_from_slice(bytes);\n");
        s.push_str("            self.state ^= u64::from_le_bytes(buf);\n");
        s.push_str("            self.state = wirecut_mix64_keyed(self.state, &self.keys);\n");
        s.push_str("        }\n    }\n}\n");
        s
    }
}

#[derive(Clone, Copy)]
pub struct WirecutHasher {
    pub state: u64,
    pub keys: [u64; 2],
}

impl WirecutHasher {
    #[inline(always)]
    pub const fn new(k0: u64, k1: u64) -> Self {
        Self {
            state: k0 ^ 0x517CC1B727220A95,
            keys: [k0, k1],
        }
    }
}

impl Default for WirecutHasher {
    #[inline(always)]
    fn default() -> Self {
        Self::new(0x517CC1B727220A95, 0x9E3779B97F4A7C15)
    }
}

impl core::hash::Hasher for WirecutHasher {
    #[inline(always)]
    fn finish(&self) -> u64 {
        let mut x = self.state ^ self.keys[1];
        x = x.wrapping_mul(0x9E3779B97F4A7C15);
        x ^= x >> 33;
        x = x.wrapping_mul(0xBF58476D1CE4E5B9);
        x ^ (x >> 33)
    }

    #[inline(always)]
    fn write(&mut self, mut bytes: &[u8]) {
        while bytes.len() >= 8 {
            let val = u64::from_le_bytes(bytes[..8].try_into().unwrap());
            self.state ^= val;
            let mut x = self.state;
            x = x.wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 33;
            x = x.wrapping_mul(0xBF58476D1CE4E5B9);
            self.state = x ^ (x >> 33);
            bytes = &bytes[8..];
        }
        if !bytes.is_empty() {
            let mut buf = [0u8; 8];
            buf[..bytes.len()].copy_from_slice(bytes);
            self.state ^= u64::from_le_bytes(buf);
            let mut x = self.state;
            x = x.wrapping_mul(0x9E3779B97F4A7C15);
            x ^= x >> 33;
            x = x.wrapping_mul(0xBF58476D1CE4E5B9);
            self.state = x ^ (x >> 33);
        }
    }
}