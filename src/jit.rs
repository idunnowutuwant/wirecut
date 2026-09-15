use crate::ast::{MicroOp, KernelAST};
use std::ptr;

pub struct MmapBuf {
    ptr: *mut u8,
    #[allow(dead_code)]
    size: usize,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn VirtualAlloc(addr: *mut libc::c_void, size: usize, alloc_type: u32, protect: u32) -> *mut libc::c_void;
    fn VirtualFree(addr: *mut libc::c_void, size: usize, free_type: u32) -> i32;
}

impl MmapBuf {
    pub fn new(size: usize) -> Self {
        let page_size = (size + 4095) & !4095;

        #[cfg(windows)]
        let ptr = unsafe {
            VirtualAlloc(ptr::null_mut(), page_size, 0x1000 | 0x2000, 0x40) as *mut u8
        };

        #[cfg(not(windows))]
        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                page_size,
                libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            ) as *mut u8
        };

        assert!(!ptr.is_null());
        Self { ptr, size: page_size }
    }

    #[inline(always)]
    pub fn as_fn(&self) -> unsafe extern "C" fn(u64) -> u64 {
        unsafe { std::mem::transmute(self.ptr) }
    }
}

impl Drop for MmapBuf {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            VirtualFree(self.ptr as *mut libc::c_void, 0, 0x8000);
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
        let mut code = Vec::with_capacity(256);

        if cfg!(windows) {
            code.extend_from_slice(&[0x48, 0x89, 0xC8]);
        } else {
            code.extend_from_slice(&[0x48, 0x89, 0xF8]);
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
            }
        }

        code.push(0xC3);

        let buf = MmapBuf::new(code.len());
        unsafe {
            ptr::copy_nonoverlapping(code.as_ptr(), buf.ptr, code.len());
        }
        buf
    }
}