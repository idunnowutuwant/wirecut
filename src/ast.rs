#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MicroOp {
    MulOdd(u64),
    AddConst(u64),
    XorConst(u64),
    Ror(u8),
    XorShr(u8),
    XorShl(u8),
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
    pub fn eval(&self, mut x: u64) -> u64 {
        for &op in &self.ops {
            match op {
                MicroOp::MulOdd(c) => x = x.wrapping_mul(c | 1),
                MicroOp::AddConst(c) => x = x.wrapping_add(c),
                MicroOp::XorConst(c) => x ^= c,
                MicroOp::Ror(k) => x = x.rotate_right(k as u32),
                MicroOp::XorShr(k) => x ^= x >> k,
                MicroOp::XorShl(k) => x ^= x << k,
            }
        }
        x
    }

    pub fn invert(&self, mut x: u64) -> u64 {
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
            }
        }
        x
    }

    pub fn emit_c(&self) -> String {
        let mut s = String::new();
        s.push_str("#ifndef WIRECUT_KERNEL_H\n#define WIRECUT_KERNEL_H\n\n#include <stdint.h>\n\n");
        s.push_str("static inline uint64_t wirecut_mix64(uint64_t x) {\n");
        for op in &self.ops {
            match op {
                MicroOp::MulOdd(c) => s.push_str(&format!("    x *= UINT64_C({:#018X});\n", c | 1)),
                MicroOp::AddConst(c) => s.push_str(&format!("    x += UINT64_C({:#018X});\n", c)),
                MicroOp::XorConst(c) => s.push_str(&format!("    x ^= UINT64_C({:#018X});\n", c)),
                MicroOp::Ror(k) => s.push_str(&format!("    x = (x >> {}) | (x << (64 - {}));\n", k, k)),
                MicroOp::XorShr(k) => s.push_str(&format!("    x ^= (x >> {});\n", k)),
                MicroOp::XorShl(k) => s.push_str(&format!("    x ^= (x << {});\n", k)),
            }
        }
        s.push_str("    return x;\n}\n\n#endif\n");
        s
    }

    pub fn emit_rust(&self) -> String {
        let mut s = String::new();
        s.push_str("#[inline(always)]\npub fn wirecut_mix64(mut x: u64) -> u64 {\n");
        for op in &self.ops {
            match op {
                MicroOp::MulOdd(c) => s.push_str(&format!("    x = x.wrapping_mul({:#018X});\n", c | 1)),
                MicroOp::AddConst(c) => s.push_str(&format!("    x = x.wrapping_add({:#018X});\n", c)),
                MicroOp::XorConst(c) => s.push_str(&format!("    x ^= {:#018X};\n", c)),
                MicroOp::Ror(k) => s.push_str(&format!("    x = x.rotate_right({});\n", k)),
                MicroOp::XorShr(k) => s.push_str(&format!("    x ^= x >> {};\n", k)),
                MicroOp::XorShl(k) => s.push_str(&format!("    x ^= x << {};\n", k)),
            }
        }
        s.push_str("    x\n}\n");
        s
    }
}