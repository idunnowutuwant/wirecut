use crate::ast::KernelAST;
use rand::Rng;

pub struct StrictAvalancheVerifier;

impl StrictAvalancheVerifier {
    pub fn measure_sac_matrix(ast: &KernelAST, trials: usize) -> ([[f64; 64]; 64], f64) {
        let mut rng = rand::thread_rng();
        let mut counts = [[0usize; 64]; 64];

        for _ in 0..trials {
            let base = rng.gen::<u64>();
            let base_out = ast.eval(base);

            for in_bit in 0..64 {
                let perturbed = base ^ (1u64 << in_bit);
                let diff = base_out ^ ast.eval(perturbed);
                for out_bit in 0..64 {
                    if (diff >> out_bit) & 1 == 1 {
                        counts[in_bit][out_bit] += 1;
                    }
                }
            }
        }

        let mut matrix = [[0.0f64; 64]; 64];
        let mut total_bias = 0.0f64;

        for i in 0..64 {
            for j in 0..64 {
                let prob = (counts[i][j] as f64 / trials as f64) * 100.0;
                matrix[i][j] = prob;
                total_bias += (prob - 50.0).abs();
            }
        }

        let mean_bias = total_bias / 4096.0;
        let global_conformity = (100.0 - mean_bias * 2.0).max(0.0);
        (matrix, global_conformity)
    }

    pub fn evaluate_fitness(ast: &KernelAST) -> f64 {
        let (_, sac_score) = Self::measure_sac_matrix(ast, 16);
        let len_penalty = ast.ops.len() as f64 * 1.5;
        sac_score - len_penalty
    }
}