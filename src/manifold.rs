use crate::ast::KernelAST;
use rand::Rng;

pub struct PoincareManifold {
    pub coords: Vec<[f64; 2]>,
    pub trajectory: Vec<[f64; 2]>,
}

impl PoincareManifold {
    pub fn from_key(k0: u64, k1: u64, size: usize) -> Self {
        let mut coords = Vec::with_capacity(size);
        let base_r = 0.35f64 + (((k0 ^ (k0 >> 32)) & 0xFFFF) as f64 / 65535.0) * 0.20f64;
        let base_th = (((k1 ^ (k1 >> 32)) & 0xFFFF) as f64 / 65535.0) * 2.0 * std::f64::consts::PI;

        for i in 0..size {
            let th = base_th + (2.0 * std::f64::consts::PI * i as f64) / size as f64;
            let perturb = (((k0.rotate_left(i as u32 * 7)) & 0xFF) as f64 / 255.0 - 0.5) * 0.08;
            let r = (base_r + perturb).clamp(0.15, 0.65);
            coords.push([r * th.cos(), r * th.sin()]);
        }
        let initial = coords[0];
        Self { coords, trajectory: vec![initial] }
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
        self.trajectory.push(*v);
    }

    pub fn export_svg(&self, filepath: &str, kernel: &KernelAST) {
        let size = 720.0f64;
        let center = size / 2.0;
        let radius = 280.0f64;

        let active_seq: Vec<usize> = kernel.ops.iter().map(|op| op.type_index()).collect();

        let mut svg = String::new();
        svg.push_str(&format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {0:.1} {0:.1}" width="{0:.1}" height="{0:.1}" style="background-color: #0f141c; font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;">"##,
            size
        ));

        svg.push_str(&format!(
            r##"<circle cx="{0:.1}" cy="{0:.1}" r="{1:.1}" fill="#161d27" stroke="#2d3748" stroke-width="1.5" />"##,
            center, radius
        ));

        for r_factor in [0.25f64, 0.50f64, 0.75f64] {
            let r_curr = radius * r_factor;
            svg.push_str(&format!(
                r##"<circle cx="{0:.1}" cy="{0:.1}" r="{1:.1}" fill="none" stroke="#232d3d" stroke-width="1" stroke-dasharray="3,3" />"##,
                center, r_curr
            ));
        }

        let calc_geodesic = |u: [f64; 2], v: [f64; 2]| -> String {
            let sx1 = center + u[0] * radius;
            let sy1 = center + u[1] * radius;
            let sx2 = center + v[0] * radius;
            let sy2 = center + v[1] * radius;
            let cross = u[0] * v[1] - u[1] * v[0];
            if cross.abs() < 1e-5 {
                return format!("M {:.1} {:.1} L {:.1} {:.1}", sx1, sy1, sx2, sy2);
            }
            let u_sq = u[0] * u[0] + u[1] * u[1] + 1.0;
            let v_sq = v[0] * v[0] + v[1] * v[1] + 1.0;
            let cx = (u_sq * v[1] - v_sq * u[1]) / (2.0 * cross);
            let cy = (u[0] * v_sq - v[0] * u_sq) / (2.0 * cross);
            let r_sq = cx * cx + cy * cy - 1.0;
            if r_sq <= 0.0 {
                return format!("M {:.1} {:.1} L {:.1} {:.1}", sx1, sy1, sx2, sy2);
            }
            let r = r_sq.sqrt() * radius;
            let scx = center + cx * radius;
            let scy = center + cy * radius;
            let th1 = (sy1 - scy).atan2(sx1 - scx);
            let th2 = (sy2 - scy).atan2(sx2 - scx);
            let diff = (th2 - th1).rem_euclid(2.0 * std::f64::consts::PI);
            let mid_th_0 = th1 + diff / 2.0;
            let mx0 = (scx + r * mid_th_0.cos() - center) / radius;
            let my0 = (scy + r * mid_th_0.sin() - center) / radius;
            let dist0 = mx0 * mx0 + my0 * my0;
            let (sweep, large_arc) = if dist0 < 1.0 {
                (1, if diff > std::f64::consts::PI { 1 } else { 0 })
            } else {
                (0, if (2.0 * std::f64::consts::PI - diff) > std::f64::consts::PI { 1 } else { 0 })
            };
            format!("M {:.1} {:.1} A {:.1} {:.1} 0 {} {} {:.1} {:.1}", sx1, sy1, r, r, large_arc, sweep, sx2, sy2)
        };

        for i in 0..self.coords.len() {
            for j in (i + 1)..self.coords.len() {
                let arc = calc_geodesic(self.coords[i], self.coords[j]);
                svg.push_str(&format!(
                    r##"<path d="{}" fill="none" stroke="#232d3d" stroke-width="1" />"##,
                    arc
                ));
            }
        }

        if active_seq.len() >= 2 {
            for k in 0..(active_seq.len() - 1) {
                let idx_a = active_seq[k];
                let idx_b = active_seq[k + 1];
                let arc = calc_geodesic(self.coords[idx_a], self.coords[idx_b]);
                svg.push_str(&format!(
                    r##"<path d="{}" fill="none" stroke="#4a9eff" stroke-width="2.5" stroke-linecap="round" />"##,
                    arc
                ));
            }
        }

        let op_names = ["MulOdd", "XorShr", "Ror", "AddConst", "XorConst", "KeyXor", "XorShl"];

        for (i, coord) in self.coords.iter().enumerate() {
            let sx = center + coord[0] * radius;
            let sy = center + coord[1] * radius;
            let is_active = active_seq.contains(&i);
            let name = op_names[i % op_names.len()];

            if is_active {
                svg.push_str(&format!(
                    r##"<circle cx="{:.1}" cy="{:.1}" r="6" fill="#4a9eff" stroke="#ffffff" stroke-width="1.5" />"##,
                    sx, sy
                ));
            } else {
                svg.push_str(&format!(
                    r##"<circle cx="{:.1}" cy="{:.1}" r="4.5" fill="#64748b" stroke="#1e293b" stroke-width="1" />"##,
                    sx, sy
                ));
            }

            svg.push_str(&format!(
                r##"<text x="{:.1}" y="{:.1}" fill="{}" font-size="11" text-anchor="middle">{}</text>"##,
                sx, sy - 10.0, if is_active { "#ffffff" } else { "#94a3b8" }, name
            ));
        }

        let mut node_visits = std::collections::HashMap::new();
        for (order, &node_idx) in active_seq.iter().enumerate() {
            node_visits.entry(node_idx).or_insert_with(Vec::new).push(order + 1);
        }

        for (node_idx, visits) in node_visits {
            let coord = self.coords[node_idx];
            let sx = center + coord[0] * radius;
            let sy = center + coord[1] * radius;
            let total = visits.len();
            for (v_idx, step) in visits.iter().enumerate() {
                let offset_x = (v_idx as f64 - (total as f64 - 1.0) / 2.0) * 16.0;
                let badge_x = sx + offset_x;
                let badge_y = sy + 18.0;
                svg.push_str(&format!(
                    r##"<circle cx="{:.1}" cy="{:.1}" r="6" fill="#1e293b" stroke="#4a9eff" stroke-width="1" />"##,
                    badge_x, badge_y
                ));
                svg.push_str(&format!(
                    r##"<text x="{:.1}" y="{:.1}" fill="#cbd5e1" font-size="8.5" text-anchor="middle">{}</text>"##,
                    badge_x, badge_y + 3.0, step
                ));
            }
        }

        svg.push_str(r##"<text x="32" y="42" fill="#94a3b8" font-size="12">Poincaré Hyperbolic Manifold (K = -1)</text>"##);
        svg.push_str(r##"<text x="32" y="58" fill="#475569" font-size="10">ds² = 4|dz|² / (1 - |z|²)²</text>"##);
        svg.push_str("</svg>");

        let _ = std::fs::write(filepath, svg);
    }
}