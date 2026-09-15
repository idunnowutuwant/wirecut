use wirecut::ast::{KernelAST, MicroOp};
use wirecut::bench::{
    bench_hashmap_comparison, bench_native_mixer, bench_splitmix64, bench_vector_throughput,
    detect_host_silicon, splitmix64,
};
use wirecut::jit::JITEmitter;
use wirecut::manifold::PoincareManifold;
use wirecut::verify::StrictAvalancheVerifier;

use rand::Rng;

pub fn synthesize_kernel(k0: u64, k1: u64, budget: usize) -> (KernelAST, PoincareManifold) {
    let mut rng = rand::thread_rng();
    let pool_size = 7usize;
    let mut manifold = PoincareManifold::from_key(k0, k1, pool_size);

    let sample_op = |op_type: usize| -> MicroOp {
        let mut r = rand::thread_rng();
        match op_type {
            0 => MicroOp::MulOdd(r.gen::<u64>() | 1),
            1 => MicroOp::XorShr(r.gen_range(17..34)),
            2 => MicroOp::Ror(r.gen_range(7..57)),
            3 => MicroOp::AddConst(r.gen::<u64>()),
            4 => MicroOp::XorConst(r.gen::<u64>()),
            5 => MicroOp::KeyXor(r.gen_range(0..2)),
            _ => MicroOp::XorShl(r.gen_range(11..29)),
        }
    };

    let c1 = (0x9E3779B97F4A7C15u64 ^ k0) | 1;
    let c2 = (0xBF58476D1CE4E5B9u64 ^ k1) | 1;

    let mut current = KernelAST::new(vec![
        MicroOp::MulOdd(c1),
        MicroOp::XorShr(33),
        MicroOp::MulOdd(c2),
        MicroOp::XorShr(33),
    ]);

    let mut best_fitness = StrictAvalancheVerifier::evaluate_fitness(&current);

    for _ in 0..budget {
        let mut cand_ops = current.ops.clone();
        let m = rng.gen_range(0..3);
        let mut path = (0, 0);

        if m == 0 {
            let r = rng.gen_range(0..cand_ops.len());
            let op_type = if r == 3 { 1 } else { rng.gen_range(0..pool_size) };
            cand_ops[r] = sample_op(op_type);
        } else if m == 1 {
            let r = rng.gen_range(0..cand_ops.len());
            let prev = cand_ops[r].type_index();
            let nxt = manifold.sample_transition(prev, 0.65);
            path = (prev, nxt);
            cand_ops[r] = sample_op(nxt);
        } else if cand_ops.len() >= 2 {
            let s = rng.gen_range(0..cand_ops.len() - 2);
            cand_ops.swap(s, s + 1);
        }

        let cand = KernelAST::new(cand_ops);
        let fit = StrictAvalancheVerifier::evaluate_fitness(&cand);

        if fit > best_fitness {
            best_fitness = fit;
            current = cand;
            if path.0 != path.1 {
                manifold.step_geodesic(path.1, path.0, 1.0, 0.05);
            }
        }
    }

    (current, manifold)
}

fn main() {
    let silicon = detect_host_silicon();
    let mut rng = rand::thread_rng();
    let k0 = rng.gen::<u64>();
    let k1 = rng.gen::<u64>();
    let keys = [k0, k1];

    let budget = 6_000;
    let (kernel, manifold) = synthesize_kernel(k0, k1, budget);
    let buf = JITEmitter::emit(&kernel);

    let (_, global_sac) = StrictAvalancheVerifier::measure_sac_matrix(&kernel, 512);
    let diff_prob = StrictAvalancheVerifier::measure_differential_bound(&kernel, 512);
    let (chi2_score, chi2_pass) = StrictAvalancheVerifier::run_chi_squared_test(&kernel, 65536);
    let (diehard_chi2, diehard_pass) = StrictAvalancheVerifier::run_dieharder_matrix_rank_test(&kernel, 5000);
    let iters = 4_000;

    let wirecut_cycles = unsafe { bench_native_mixer(buf.as_fn(), iters) };
    let splitmix_cycles = bench_splitmix64(iters);
    let (gb_per_sec, m_ops_sec) = bench_vector_throughput(buf.as_fn());
    let (std_ms, wirecut_ms, hashmap_speedup) = bench_hashmap_comparison();

    let test_input = 0x0123456789ABCDEFu64;
    let wirecut_out = unsafe { (buf.as_fn())(test_input, keys.as_ptr()) };
    let splitmix_out = splitmix64(test_input);

    let inverted = kernel.invert_keyed(wirecut_out, &keys);
    assert_eq!(inverted, test_input);

    let _ = std::fs::write("wirecut_mixer.h", kernel.emit_c());
    let _ = std::fs::write("wirecut_mixer.rs", kernel.emit_rust());

    manifold.export_svg("poincare_manifold.svg", &kernel);
    StrictAvalancheVerifier::emit_audit_report(&kernel, "WIRECUT_CRYPTANALYTIC_AUDIT.md");

    println!("wirecut (target: {})", silicon.name);
    println!("  pipeline    : {} ops", kernel.ops.len());
    println!("  latency     : {} cycles (jit), {} cycles (splitmix64)", wirecut_cycles, splitmix_cycles);
    println!("  throughput  : {:.2} GB/s ({:.1} M ops/sec via 8-way stream)", gb_per_sec, m_ops_sec);
    println!("  hashmap     : std={:.1}ms, wirecut={:.1}ms ({:.2}x speedup on 200k keys)", std_ms, wirecut_ms, hashmap_speedup);
    println!("  sac         : {:.2}%", global_sac);
    println!("  diff_bound  : {:.4}", diff_prob);
    println!("  chi_squared : {:.2} ({})", chi2_score, if chi2_pass { "pass" } else { "warning" });
    println!("  dieharder   : rank_chi2={:.2} ({})", diehard_chi2, if diehard_pass { "pass" } else { "warning" });
    println!("  sample      : in={:#018X} out={:#018X} (ref={:#018X})", test_input, wirecut_out, splitmix_out);
    println!("  artifacts   : wirecut_mixer.h (AVX2/AVX-512), wirecut_mixer.rs, poincare_manifold.svg, WIRECUT_CRYPTANALYTIC_AUDIT.md");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyed_bijectivity_full_domain() {
        let ast = KernelAST::new(vec![
            MicroOp::KeyXor(0),
            MicroOp::MulOdd(0x9E3779B97F4A7C15),
            MicroOp::XorShr(17),
            MicroOp::KeyXor(1),
            MicroOp::AddConst(0xBF58476D1CE4E5B9),
            MicroOp::Ror(23),
            MicroOp::XorShl(19),
        ]);

        let keys = [0xDEADBEEFCAFEBABE, 0x0123456789ABCDEF];
        let inputs = [0u64, 1u64, u64::MAX, 0xCAFEBABEDEADBEEF, 0x0123456789ABCDEF];
        for &x in &inputs {
            let y = ast.eval_keyed(x, &keys);
            let inv = ast.invert_keyed(y, &keys);
            assert_eq!(inv, x);
        }
    }

    #[test]
    fn test_jit_native_execution_equivalence() {
        let ast = KernelAST::new(vec![
            MicroOp::KeyXor(0),
            MicroOp::XorShr(19),
            MicroOp::MulOdd(0x517CC1B727220A95),
            MicroOp::KeyXor(1),
            MicroOp::Ror(31),
            MicroOp::XorShl(13),
        ]);

        let keys = [0x55555555AAAAAAAA, 0x1234567890ABCDEF];
        let buf = JITEmitter::emit(&kernel);
        let f = buf.as_fn();

        for x in [0u64, 42u64, u64::MAX, 0xCAFEBABE12345678] {
            assert_eq!(ast.eval_keyed(x, &keys), unsafe { f(x, keys.as_ptr()) });
        }
    }
}