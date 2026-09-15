use rand::Rng;

pub struct PoincareManifold {
    pub coords: Vec<[f64; 2]>,
}

impl PoincareManifold {
    pub fn new(size: usize) -> Self {
        let mut rng = rand::thread_rng();
        let mut coords = Vec::with_capacity(size);
        for _ in 0..size {
            let r = rng.gen_range(0.01..0.25);
            let th = rng.gen_range(-std::f64::consts::PI..std::f64::consts::PI);
            coords.push([r * th.cos(), r * th.sin()]);
        }
        Self { coords }
    }

    #[inline(always)]
    pub fn distance(&self, a: usize, b: usize) -> f64 {
        let u = &self.coords[a];
        let v = &self.coords[b];
        let dx = u[0] - v[0];
        let dy = u[1] - v[1];
        let sq_d = dx * dx + dy * dy;
        let u_sq = (u[0] * u[0] + u[1] * u[1]).min(0.996);
        let v_sq = (v[0] * v[0] + v[1] * v[1]).min(0.996);
        let arg = 1.0 + 2.0 * sq_d / ((1.0 - u_sq) * (1.0 - v_sq));
        arg.max(1.0).acosh()
    }

    pub fn sample_transition(&self, current: usize, temperature: f64) -> usize {
        let mut rng = rand::thread_rng();
        let size = self.coords.len();
        let mut logits = vec![0.0f64; size];
        let mut max_l = f64::NEG_INFINITY;

        for i in 0..size {
            let d = self.distance(current, i);
            let l = -d / temperature.max(1e-4);
            logits[i] = l;
            if l > max_l {
                max_l = l;
            }
        }

        let mut sum = 0.0f64;
        for l in &mut logits {
            *l = (*l - max_l).exp();
            sum += *l;
        }

        let roll = rng.gen::<f64>() * sum;
        let mut accum = 0.0f64;
        for (i, &w) in logits.iter().enumerate() {
            accum += w;
            if accum >= roll {
                return i;
            }
        }
        size - 1
    }

    pub fn step_geodesic(&mut self, mover: usize, target: usize, reward: f64, lr: f64) {
        let u = self.coords[target];
        let v = &mut self.coords[mover];
        let dx = u[0] - v[0];
        let dy = u[1] - v[1];
        let norm = (dx * dx + dy * dy).sqrt();
        if norm < 1e-6 {
            return;
        }
        let v_sq = v[0] * v[0] + v[1] * v[1];
        let lambda = 2.0 / (1.0 - v_sq).max(1e-4);
        let conformal = 1.0 / (lambda * lambda);
        let step = conformal * lr * reward;
        v[0] += (dx / norm) * step;
        v[1] += (dy / norm) * step;
        let new_norm = (v[0] * v[0] + v[1] * v[1]).sqrt();
        if new_norm >= 0.998 {
            v[0] = (v[0] / new_norm) * 0.998;
            v[1] = (v[1] / new_norm) * 0.998;
        }
    }
}