use crate::ast::WirecutHasher;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::time::Instant;

pub struct SiliconProfile {
    pub name: &'static str,
    pub mul_latency: usize,
    pub alu_ports: usize,
}

pub fn detect_host_silicon() -> SiliconProfile {
    #[cfg(target_arch = "x86_64")]
    {
        SiliconProfile {
            name: "x86_64 Modern Core (Intel / AMD Zen 4/5)",
            mul_latency: 3,
            alu_ports: 4,
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        SiliconProfile {
            name: "ARM64 High-IPC Core (Apple Silicon / Graviton 3/4)",
            mul_latency: 3,
            alu_ports: 6,
        }
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        SiliconProfile {
            name: "Generic Commodity Silicon",
            mul_latency: 4,
            alu_ports: 2,
        }
    }
}

#[inline(always)]
pub unsafe fn rdtscp() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        use core::arch::x86_64::{_mm_lfence, __rdtscp};
        let mut aux = 0u32;
        _mm_lfence();
        let val = __rdtscp(&mut aux);
        _mm_lfence();
        val
    }
    #[cfg(target_arch = "aarch64")]
    {
        let mut val: u64;
        std::arch::asm!("mrs {}, cntvct_el0", out(reg) val, options(nomem, nostack));
        val
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        0
    }
}

#[inline(never)]
pub fn splitmix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

#[inline(never)]
pub unsafe fn bench_native_mixer(func: unsafe extern "C" fn(u64, *const u64) -> u64, iters: usize) -> u64 {
    let window = 64usize;
    let mut inputs = vec![0u64; window];
    let keys = [0x517CC1B727220A95u64, 0x9E3779B97F4A7C15u64];
    let mut min = u64::MAX;

    for step in 0..iters {
        for w in 0..window {
            inputs[w] = ((step + w) * 6364136223846793005 + 1) as u64;
        }

        let mut sink = 0u64;
        let t0 = rdtscp();
        for w in 0..window {
            sink = sink.wrapping_add(func(std::hint::black_box(inputs[w]), keys.as_ptr()));
        }
        let t1 = rdtscp();
        std::hint::black_box(sink);

        let d = (t1.saturating_sub(t0)) / window as u64;
        if d < min {
            min = d;
        }
    }
    min
}

#[inline(never)]
pub fn bench_splitmix64(iters: usize) -> u64 {
    let window = 64usize;
    let mut inputs = vec![0u64; window];
    let mut min = u64::MAX;

    for step in 0..iters {
        for w in 0..window {
            inputs[w] = ((step + w) * 6364136223846793005 + 1) as u64;
        }

        let mut sink = 0u64;
        let t0 = unsafe { rdtscp() };
        for w in 0..window {
            sink = sink.wrapping_add(splitmix64(std::hint::black_box(inputs[w])));
        }
        let t1 = unsafe { rdtscp() };
        std::hint::black_box(sink);

        let d = (t1.saturating_sub(t0)) / window as u64;
        if d < min {
            min = d;
        }
    }
    min
}

pub fn bench_vector_throughput(func: unsafe extern "C" fn(u64, *const u64) -> u64) -> (f64, f64) {
    let count = 1_000_000usize;
    let mut data = vec![0u64; count];
    for i in 0..count {
        data[i] = (i as u64).wrapping_mul(0x517CC1B727220A95);
    }
    let keys = [0x517CC1B727220A95u64, 0x9E3779B97F4A7C15u64];

    let t0 = Instant::now();
    let mut sink = 0u64;
    let mut i = 0usize;
    while i + 8 <= count {
        unsafe {
            sink = sink.wrapping_add(func(data[i], keys.as_ptr()));
            sink = sink.wrapping_add(func(data[i + 1], keys.as_ptr()));
            sink = sink.wrapping_add(func(data[i + 2], keys.as_ptr()));
            sink = sink.wrapping_add(func(data[i + 3], keys.as_ptr()));
            sink = sink.wrapping_add(func(data[i + 4], keys.as_ptr()));
            sink = sink.wrapping_add(func(data[i + 5], keys.as_ptr()));
            sink = sink.wrapping_add(func(data[i + 6], keys.as_ptr()));
            sink = sink.wrapping_add(func(data[i + 7], keys.as_ptr()));
        }
        i += 8;
    }
    std::hint::black_box(sink);
    let elapsed = t0.elapsed().as_secs_f64();

    let bytes = (count * 8) as f64;
    let gb_per_sec = (bytes / elapsed) / 1e9;
    let m_ops_sec = (count as f64 / elapsed) / 1e6;
    (gb_per_sec, m_ops_sec)
}

pub fn bench_hashmap_comparison() -> (f64, f64, f64) {
    let n = 200_000usize;
    let mut keys = Vec::with_capacity(n);
    for i in 0..n {
        keys.push((i as u64).wrapping_mul(0x9E3779B97F4A7C15));
    }

    let t0 = Instant::now();
    let mut std_map = HashMap::with_capacity(n);
    for &k in &keys {
        std_map.insert(k, k ^ 0x5555);
    }
    let mut std_hits = 0u64;
    for &k in &keys {
        if let Some(&v) = std_map.get(&k) {
            std_hits = std_hits.wrapping_add(v);
        }
    }
    std::hint::black_box(std_hits);
    let std_time = t0.elapsed().as_secs_f64();

    let t1 = Instant::now();
    let mut wirecut_map: HashMap<u64, u64, BuildHasherDefault<WirecutHasher>> =
        HashMap::with_capacity_and_hasher(n, BuildHasherDefault::default());
    for &k in &keys {
        wirecut_map.insert(k, k ^ 0x5555);
    }
    let mut wirecut_hits = 0u64;
    for &k in &keys {
        if let Some(&v) = wirecut_map.get(&k) {
            wirecut_hits = wirecut_hits.wrapping_add(v);
        }
    }
    std::hint::black_box(wirecut_hits);
    let wirecut_time = t1.elapsed().as_secs_f64();

    let speedup = std_time / wirecut_time.max(1e-6);
    (std_time * 1000.0, wirecut_time * 1000.0, speedup)
}