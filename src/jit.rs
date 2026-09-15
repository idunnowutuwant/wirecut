use crate::ast::{KernelAST, MicroOp};
use std::ptr;

pub struct MmapBuf {
    ptr: *mut u8,
    #[allow(dead_code)]
    size: usize,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn VirtualAlloc(addr: *mut std::ffi::c_void, size: usize, alloc_type: u32, protect: u32) -> *mut std::ffi::c_void;
    fn VirtualFree(addr: *mut std::ffi::c_void, size: usize, free_type: u32) -> i32;
    fn VirtualProtect(addr: *mut std::ffi::c_void, size: usize, new_protect: u32, old_protect: *mut u32) -> i32;
}

impl MmapBuf {
    pub fn new(code: &[u8]) -> Self {
        let page_size = (code.len() + 4095) & !4095;

        #[cfg(windows)]
        let (ptr, ok) = unsafe {
            let p = VirtualAlloc(ptr::null_mut(), page_size, 0x1000 | 0x2000, 0x04) as *mut u8;
            assert!(!p.is_null());
            ptr::copy_nonoverlapping(code.as_ptr(), p, code.len());
            let mut old = 0u32;
            let success = VirtualProtect(p as *mut std::ffi::c_void, page_size, 0x20, &mut old) != 0;
            (p, success)
        };

        #[cfg(not(windows))]
        let (ptr, ok) = unsafe {
            let p = libc::mmap(
                ptr::null_mut(),
                page_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            ) as *mut u8;
            assert!(!p.is_null() && p != libc::MAP_FAILED as *mut u8);
            ptr::copy_nonoverlapping(code.as_ptr(), p, code.len());
            let r = libc::mprotect(p as *mut libc::c_void, page_size, libc::PROT_READ | libc::PROT_EXEC);
            (p, r == 0)
        };

        assert!(ok);
        Self { ptr, size: page_size }
    }

    #[inline(always)]
    pub fn as_fn(&self) -> unsafe extern "C" fn(u64, *const u64) -> u64 {
        unsafe { std::mem::transmute(self.ptr) }
    }
}

impl Drop for MmapBuf {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            VirtualFree(self.ptr as *mut std::ffi::c_void, 0, 0x8000);
        }
        #[cfg(not(windows))]
        unsafe {
            libc::munmap(self.ptr as *mut libc::c_void, self.size);
        }
    }
}

pub struct JITEmitter;

impl JITEmitter {
    pub fn emit(ast: &KernelAST) -> MmapBuf {
        let mut code = Vec::with_capacity(512);

        #[cfg(target_arch = "x86_64")]
        {
            if cfg!(windows) {
                code.extend_from_slice(&[0x48, 0x89, 0xC8]);
                code.extend_from_slice(&[0x49, 0x89, 0xD2]);
            } else {
                code.extend_from_slice(&[0x48, 0x89, 0xF8]);
                code.extend_from_slice(&[0x49, 0x89, 0xF2]);
            }

            for &op in &ast.ops {
                match op {
                    MicroOp::MulOdd(c) => {
                        let odd = c | 1;
                        code.extend_from_slice(&[0x48, 0xBA]);
                        code.extend_from_slice(&odd.to_le_bytes());
                        code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC2]);
                    }
                    MicroOp::AddConst(c) => {
                        code.extend_from_slice(&[0x48, 0xBA]);
                        code.extend_from_slice(&c.to_le_bytes());
                        code.extend_from_slice(&[0x48, 0x01, 0xD0]);
                    }
                    MicroOp::XorConst(c) => {
                        code.extend_from_slice(&[0x48, 0xBA]);
                        code.extend_from_slice(&c.to_le_bytes());
                        code.extend_from_slice(&[0x48, 0x31, 0xD0]);
                    }
                    MicroOp::Ror(k) => {
                        code.extend_from_slice(&[0x48, 0xC1, 0xC8, k]);
                    }
                    MicroOp::XorShr(k) => {
                        code.extend_from_slice(&[0x48, 0x89, 0xC2]);
                        code.extend_from_slice(&[0x48, 0xC1, 0xEA, k]);
                        code.extend_from_slice(&[0x48, 0x31, 0xD0]);
                    }
                    MicroOp::XorShl(k) => {
                        code.extend_from_slice(&[0x48, 0x89, 0xC2]);
                        code.extend_from_slice(&[0x48, 0xC1, 0xE2, k]);
                        code.extend_from_slice(&[0x48, 0x31, 0xD0]);
                    }
                    MicroOp::KeyXor(idx) => {
                        let offset = ((idx & 1) * 8) as u8;
                        code.extend_from_slice(&[0x49, 0x33, 0x42, offset]);
                    }
                }
            }
            code.push(0xC3);
        }

        #[cfg(target_arch = "aarch64")]
        {
            let encode_mov_u64 = |reg: u32, val: u64| -> [u32; 4] {
                [
                    0xD2800000 | (((val & 0xFFFF) as u32) << 5) | reg,
                    0xF2A00000 | ((((val >> 16) & 0xFFFF) as u32) << 5) | reg,
                    0xF2C00000 | ((((val >> 32) & 0xFFFF) as u32) << 5) | reg,
                    0xF2E00000 | ((((val >> 48) & 0xFFFF) as u32) << 5) | reg,
                ]
            };

            for &op in &ast.ops {
                match op {
                    MicroOp::MulOdd(c) => {
                        for ins in encode_mov_u64(2, c | 1) {
                            code.extend_from_slice(&ins.to_le_bytes());
                        }
                        code.extend_from_slice(&0x9B027C00u32.to_le_bytes());
                    }
                    MicroOp::AddConst(c) => {
                        for ins in encode_mov_u64(2, c) {
                            code.extend_from_slice(&ins.to_le_bytes());
                        }
                        code.extend_from_slice(&0x8B020000u32.to_le_bytes());
                    }
                    MicroOp::XorConst(c) => {
                        for ins in encode_mov_u64(2, c) {
                            code.extend_from_slice(&ins.to_le_bytes());
                        }
                        code.extend_from_slice(&0xCA020000u32.to_le_bytes());
                    }
                    MicroOp::Ror(k) => {
                        let ins = 0x93C00000u32 | ((k as u32) << 10) | (0 << 16) | 0;
                        code.extend_from_slice(&ins.to_le_bytes());
                    }
                    MicroOp::XorShr(k) => {
                        let lsr = 0xD3400000u32 | ((k as u32) << 16) | (63 << 10) | (0 << 5) | 2;
                        code.extend_from_slice(&lsr.to_le_bytes());
                        code.extend_from_slice(&0xCA020000u32.to_le_bytes());
                    }
                    MicroOp::XorShl(k) => {
                        let immr = (64 - (k as u32)) & 63;
                        let imms = 63 - (k as u32);
                        let lsl = 0xD3400000u32 | (immr << 16) | (imms << 10) | (0 << 5) | 2;
                        code.extend_from_slice(&lsl.to_le_bytes());
                        code.extend_from_slice(&0xCA020000u32.to_le_bytes());
                    }
                    MicroOp::KeyXor(idx) => {
                        let offset = ((idx & 1) * 8) as u32;
                        let ldr = 0xF9400000u32 | ((offset / 8) << 10) | (1 << 5) | 2;
                        code.extend_from_slice(&ldr.to_le_bytes());
                        code.extend_from_slice(&0xCA020000u32.to_le_bytes());
                    }
                }
            }
            code.extend_from_slice(&0xD65F03C0u32.to_le_bytes());
        }

        MmapBuf::new(&code)
    }
}