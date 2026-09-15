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
    #[cfg(not(target_arch = "x86_64"))]
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
pub unsafe fn bench_native_mixer(func: unsafe extern "C" fn(u64) -> u64, iters: usize) -> u64 {
    let window = 64usize;
    let mut inputs = vec![0u64; window];
    let mut min = u64::MAX;

    for step in 0..iters {
        for w in 0..window {
            inputs[w] = ((step + w) * 6364136223846793005 + 1) as u64;
        }

        let mut sink = 0u64;
        let t0 = rdtscp();
        for w in 0..window {
            sink = sink.wrapping_add(func(std::hint::black_box(inputs[w])));
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