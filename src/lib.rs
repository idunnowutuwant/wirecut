pub mod ast;
pub mod bench;
pub mod jit;
pub mod manifold;
pub mod verify;

#[cfg(test)]
mod tests {
    use crate::ast::{KernelAST, MicroOp};
    use crate::jit::JITEmitter;
    use crate::verify::StrictAvalancheVerifier;

    #[test]
    fn test_bijective_roundtrip_all_edge_cases() {
        let kernel = KernelAST::new(vec![
            MicroOp::KeyXor(0),
            MicroOp::MulOdd(0x9E3779B97F4A7C15),
            MicroOp::XorShr(29),
            MicroOp::KeyXor(1),
            MicroOp::AddConst(0xBF58476D1CE4E5B9),
            MicroOp::Ror(31),
            MicroOp::XorShl(17),
        ]);

        let keys = [0x517CC1B727220A95, 0xBF58476D1CE4E5B9];
        let edge_cases = [
            0u64,
            1u64,
            u64::MAX,
            0x5555555555555555,
            0xAAAAAAAAAAAAAAAA,
            0x0123456789ABCDEF,
            0xFEDCBA9876543210,
        ];

        for &x in &edge_cases {
            let enc = kernel.eval_keyed(x, &keys);
            let dec = kernel.invert_keyed(enc, &keys);
            assert_eq!(dec, x, "Inversion failed on edge case: {:#018X}", x);
        }

        for bit in 0..64 {
            let walking_one = 1u64 << bit;
            let enc = kernel.eval_keyed(walking_one, &keys);
            let dec = kernel.invert_keyed(enc, &keys);
            assert_eq!(dec, walking_one, "Walking one failed at bit {}", bit);
        }
    }

    #[test]
    fn test_jit_matches_ast_evaluation() {
        let kernel = KernelAST::new(vec![
            MicroOp::KeyXor(0),
            MicroOp::MulOdd(0x517CC1B727220A95),
            MicroOp::XorShr(23),
            MicroOp::KeyXor(1),
            MicroOp::AddConst(0x9E3779B97F4A7C15),
            MicroOp::Ror(27),
            MicroOp::XorShl(19),
        ]);

        let keys = [0xDEADBEEFCAFEBABE, 0x0123456789ABCDEF];
        let buf = JITEmitter::emit(&kernel);
        let f = buf.as_fn();

        for x in [0u64, 42u64, u64::MAX, 0x123456789ABCDEF0] {
            let ast_out = kernel.eval_keyed(x, &keys);
            let jit_out = unsafe { f(x, keys.as_ptr()) };
            assert_eq!(jit_out, ast_out, "JIT output mismatch for input: {:#018X}", x);
        }
    }

    #[test]
    fn test_hashdos_key_separation() {
        let kernel = KernelAST::new(vec![
            MicroOp::KeyXor(0),
            MicroOp::MulOdd(0x9E3779B97F4A7C15),
            MicroOp::XorShr(31),
            MicroOp::KeyXor(1),
            MicroOp::MulOdd(0xBF58476D1CE4E5B9),
        ]);

        let x = 0xCAFEBABE12345678u64;
        let keys_a = [0x0000000000000001, 0x0000000000000000];
        let keys_b = [0x0000000000000002, 0x0000000000000000];

        let out_a = kernel.eval_keyed(x, &keys_a);
        let out_b = kernel.eval_keyed(x, &keys_b);

        let diff_bits = (out_a ^ out_b).count_ones();
        assert!(diff_bits >= 20, "Insufficient key separation bit diffusion: {}", diff_bits);
    }

    #[test]
    fn test_strict_avalanche_and_differential_bound() {
        let kernel = KernelAST::new(vec![
            MicroOp::KeyXor(0),
            MicroOp::MulOdd(0x9E3779B97F4A7C15),
            MicroOp::XorShr(31),
            MicroOp::KeyXor(1),
            MicroOp::MulOdd(0xBF58476D1CE4E5B9),
            MicroOp::XorShr(27),
        ]);

        let (_, sac_score) = StrictAvalancheVerifier::measure_sac_matrix(&kernel, 64);
        let diff_prob = StrictAvalancheVerifier::measure_differential_bound(&kernel, 64);

        assert!(sac_score > 85.0, "SAC score is too low: {:.2}%", sac_score);
        assert!(diff_prob < 0.25, "Differential collision probability is too high: {:.4}", diff_prob);
    }
}