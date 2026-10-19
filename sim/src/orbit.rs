//! Mecânica orbital de dois corpos: elementos keplerianos, propagação, Lambert,
//! manobras clássicas e o movimento relativo de Clohessy-Wiltshire.

use crate::math::{PI, TAU, Vec3, wrap_2pi, wrap_pi};
use serde::{Deserialize, Serialize};

/// Elementos clássicos. Ângulos em radianos, distâncias em metros.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Elements {
    /// Semieixo maior (negativo em hipérbole).
    pub a: f64,
    pub e: f64,
    pub i: f64,
    /// Longitude do nó ascendente.
    pub raan: f64,
    /// Argumento do periastro.
    pub argp: f64,
    /// Anomalia verdadeira.
    pub nu: f64,
}

impl Elements {
    pub fn periapsis(&self) -> f64 {
        self.a * (1.0 - self.e)
    }
    /// Apoastro; infinito em órbita aberta.
    pub fn apoapsis(&self) -> f64 {
        if self.e >= 1.0 { f64::INFINITY } else { self.a * (1.0 + self.e) }
    }
    pub fn period(&self, mu: f64) -> f64 {
        if self.e >= 1.0 || self.a <= 0.0 {
            f64::INFINITY
        } else {
            TAU * (self.a.powi(3) / mu).sqrt()
        }
    }
    pub fn mean_motion(&self, mu: f64) -> f64 {
        (mu / self.a.abs().powi(3)).sqrt()
    }
    /// Anomalia média (elipse) ou hiperbólica média.
    pub fn mean_anomaly(&self) -> f64 {
        true_to_mean(self.nu, self.e)
    }
    /// Semi-latus rectum.
    pub fn p(&self) -> f64 {
        if (self.e - 1.0).abs() < 1e-12 {
            // Parábola: guardamos p em `a` não faz sentido; usa a·(1−e²) mesmo assim.
            self.a.abs() * 2.0
        } else {
            self.a * (1.0 - self.e * self.e)
        }
    }
}

/// Elementos a partir de posição e velocidade.
pub fn elements_from_state(r: Vec3, v: Vec3, mu: f64) -> Elements {
    let rn = r.norm();
    let vn = v.norm();
    let h = r.cross(v);
    let hn = h.norm();
    let n = Vec3::Z.cross(h);
    let nn = n.norm();
    let ev = (r * (vn * vn - mu / rn) - v * r.dot(v)) / mu;
    let e = ev.norm();
    let energy = vn * vn / 2.0 - mu / rn;
    let a = if energy.abs() > 1e-12 { -mu / (2.0 * energy) } else { f64::INFINITY };
    let i = if hn > 0.0 { (h.z / hn).clamp(-1.0, 1.0).acos() } else { 0.0 };
    let circular = e < 1e-9;
    let equatorial = nn < 1e-9 * hn.max(1.0);
    let raan = if equatorial { 0.0 } else { wrap_2pi(n.y.atan2(n.x)) };
    let argp = if circular {
        0.0
    } else if equatorial {
        // Longitude do periastro, no sentido do movimento.
        let w = ev.y.atan2(ev.x);
        wrap_2pi(if h.z < 0.0 { -w } else { w })
    } else {
        let c = (n.dot(ev) / (nn * e)).clamp(-1.0, 1.0).acos();
        if ev.z < 0.0 { TAU - c } else { c }
    };
    let nu = if circular {
        // Argumento de latitude (ou longitude verdadeira, se equatorial).
        if equatorial {
            let l = r.y.atan2(r.x);
            wrap_2pi(if h.z < 0.0 { -l } else { l })
        } else {
            let c = (n.dot(r) / (nn * rn)).clamp(-1.0, 1.0).acos();
            if r.z < 0.0 { TAU - c } else { c }
        }
    } else {
        let c = (ev.dot(r) / (e * rn)).clamp(-1.0, 1.0).acos();
        if r.dot(v) < 0.0 { TAU - c } else { c }
    };
    Elements { a, e, i, raan, argp, nu }
}

/// Posição e velocidade a partir dos elementos.
pub fn state_from_elements(el: &Elements, mu: f64) -> (Vec3, Vec3) {
    let p = el.p();
    let (sn, cn) = el.nu.sin_cos();
    let r_pf = Vec3::new(cn, sn, 0.0) * (p / (1.0 + el.e * cn));
    let v_pf = Vec3::new(-sn, el.e + cn, 0.0) * (mu / p).sqrt();
    let rot = |v: Vec3| v.rot_z(el.argp).rot_x(el.i).rot_z(el.raan);
    (rot(r_pf), rot(v_pf))
}

/// Anomalia excêntrica (ou hiperbólica) a partir da média, por Newton.
pub fn solve_kepler(m: f64, e: f64) -> f64 {
    if e < 1.0 {
        let m = wrap_pi(m);
        let mut ea = if e > 0.8 { PI.copysign(m) } else { m };
        for _ in 0..50 {
            let f = ea - e * ea.sin() - m;
            let d = f / (1.0 - e * ea.cos());
            ea -= d;
            if d.abs() < 1e-14 {
                break;
            }
        }
        ea
    } else {
        let mut h = (2.0 * m / e).asinh();
        for _ in 0..80 {
            let f = e * h.sinh() - h - m;
            let d = f / (e * h.cosh() - 1.0);
            h -= d;
            if d.abs() < 1e-13 {
                break;
            }
        }
        h
    }
}

pub fn true_to_mean(nu: f64, e: f64) -> f64 {
    if e < 1.0 {
        let ea = 2.0 * (((1.0 - e) / (1.0 + e)).sqrt() * (nu / 2.0).tan()).atan();
        ea - e * ea.sin()
    } else {
        let h = 2.0 * (((e - 1.0) / (e + 1.0)).sqrt() * (nu / 2.0).tan()).atanh();
        e * h.sinh() - h
    }
}

pub fn mean_to_true(m: f64, e: f64) -> f64 {
    let x = solve_kepler(m, e);
    if e < 1.0 {
        2.0 * (((1.0 + e) / (1.0 - e)).sqrt() * (x / 2.0).tan()).atan()
    } else {
        2.0 * (((e + 1.0) / (e - 1.0)).sqrt() * (x / 2.0).tanh()).atan()
    }
}

/// Funções de Stumpff.
fn stumpff_c(z: f64) -> f64 {
    if z > 1e-6 {
        (1.0 - z.sqrt().cos()) / z
    } else if z < -1e-6 {
        ((-z).sqrt().cosh() - 1.0) / -z
    } else {
        0.5 - z / 24.0 + z * z / 720.0
    }
}
fn stumpff_s(z: f64) -> f64 {
    if z > 1e-6 {
        let s = z.sqrt();
        (s - s.sin()) / s.powi(3)
    } else if z < -1e-6 {
        let s = (-z).sqrt();
        (s.sinh() - s) / s.powi(3)
    } else {
        1.0 / 6.0 - z / 120.0 + z * z / 5040.0
    }
}

/// Propaga um estado por `dt` segundos no problema de dois corpos (variáveis
/// universais: serve a elipse, parábola e hipérbole).
pub fn propagate_kepler(r0: Vec3, v0: Vec3, dt: f64, mu: f64) -> (Vec3, Vec3) {
    if dt == 0.0 {
        return (r0, v0);
    }
    let r0n = r0.norm();
    let vr0 = r0.dot(v0) / r0n;
    let alpha = 2.0 / r0n - v0.norm2() / mu;
    let sqmu = mu.sqrt();
    // Chute inicial.
    let mut chi = if alpha > 1e-12 {
        sqmu * dt * alpha
    } else {
        sqmu * dt / r0n
    };
    for _ in 0..100 {
        let z = alpha * chi * chi;
        let c = stumpff_c(z);
        let s = stumpff_s(z);
        let f = r0n * vr0 / sqmu * chi * chi * c + (1.0 - alpha * r0n) * chi.powi(3) * s
            + r0n * chi
            - sqmu * dt;
        let df = r0n * vr0 / sqmu * chi * (1.0 - alpha * chi * chi * s)
            + (1.0 - alpha * r0n) * chi * chi * c
            + r0n;
        let step = f / df;
        chi -= step;
        if !chi.is_finite() {
            break;
        }
        if step.abs() < 1e-9 * chi.abs().max(1.0) {
            break;
        }
    }
    let z = alpha * chi * chi;
    let c = stumpff_c(z);
    let s = stumpff_s(z);
    let f = 1.0 - chi * chi / r0n * c;
    let g = dt - chi.powi(3) / sqmu * s;
    let r = r0 * f + v0 * g;
    let rn = r.norm();
    let fdot = sqmu / (rn * r0n) * (alpha * chi.powi(3) * s - chi);
    let gdot = 1.0 - chi * chi / rn * c;
    (r, r0 * fdot + v0 * gdot)
}

/// Problema de Lambert: velocidades de saída e chegada para ir de `r1` a `r2`
/// em `tof` segundos, no sentido prógrado (ou retrógrado). Variáveis universais
/// com bisseção (Curtis, algoritmo 5.2, com salvaguardas).
pub fn lambert(r1: Vec3, r2: Vec3, tof: f64, mu: f64, prograde: bool) -> Option<(Vec3, Vec3)> {
    if tof <= 0.0 {
        return None;
    }
    let r1n = r1.norm();
    let r2n = r2.norm();
    let c12 = r1.cross(r2);
    let mut dtheta = (r1.dot(r2) / (r1n * r2n)).clamp(-1.0, 1.0).acos();
    let short = if prograde { c12.z >= 0.0 } else { c12.z < 0.0 };
    if !short {
        dtheta = TAU - dtheta;
    }
    let a_ = dtheta.sin() * (r1n * r2n / (1.0 - dtheta.cos())).sqrt();
    if a_.abs() < 1e-9 {
        return None;
    }
    let y = |z: f64| r1n + r2n + a_ * (z * stumpff_s(z) - 1.0) / stumpff_c(z).sqrt();
    let time_of = |z: f64| -> f64 {
        let yz = y(z);
        if yz < 0.0 {
            return f64::NAN;
        }
        let c = stumpff_c(z);
        ((yz / c).powf(1.5) * stumpff_s(z) + a_ * yz.sqrt()) / mu.sqrt()
    };
    // Na volta simples o tempo de voo cresce com z até o infinito em z → 4π².
    // Varre z até achar onde ele passa de `tof`, depois bisseção.
    let z_max = 4.0 * PI * PI - 1e-9;
    let mut prev: Option<f64> = None;
    let mut bracket = None;
    let mut zz = -120.0;
    while zz < z_max {
        let f = time_of(zz) - tof;
        if f.is_finite() {
            if f >= 0.0 {
                if let Some(zp) = prev {
                    bracket = Some((zp, zz));
                }
                break;
            }
            prev = Some(zz);
        }
        zz += if zz < -10.0 { 1.0 } else { 0.2 };
    }
    if bracket.is_none() {
        if let Some(zp) = prev {
            if time_of(z_max) - tof >= 0.0 {
                bracket = Some((zp, z_max));
            }
        }
    }
    let (mut lo, mut hi) = bracket?;
    let mut z = 0.0;
    for _ in 0..200 {
        z = 0.5 * (lo + hi);
        let t = time_of(z);
        if t.is_nan() || t < tof {
            lo = z;
        } else {
            hi = z;
        }
        if (hi - lo).abs() < 1e-11 {
            break;
        }
    }
    let yz = y(z);
    let f = 1.0 - yz / r1n;
    let g = a_ * (yz / mu).sqrt();
    let gdot = 1.0 - yz / r2n;
    if g.abs() < 1e-12 {
        return None;
    }
    let v1 = (r2 - r1 * f) / g;
    let v2 = (r2 * gdot - r1) / g;
    if v1.is_finite() && v2.is_finite() { Some((v1, v2)) } else { None }
}

/// Velocidade circular.
pub fn v_circular(mu: f64, r: f64) -> f64 {
    (mu / r).sqrt()
}

/// Velocidade numa órbita de semieixo `a` à distância `r` (vis-viva).
pub fn vis_viva(mu: f64, r: f64, a: f64) -> f64 {
    (mu * (2.0 / r - 1.0 / a)).max(0.0).sqrt()
}

/// Velocidade de escape.
pub fn v_escape(mu: f64, r: f64) -> f64 {
    (2.0 * mu / r).sqrt()
}

/// Transferência de Hohmann entre órbitas circulares: Δv de cada queima e tempo.
pub fn hohmann(mu: f64, r1: f64, r2: f64) -> (f64, f64, f64) {
    let at = (r1 + r2) / 2.0;
    let dv1 = vis_viva(mu, r1, at) - v_circular(mu, r1);
    let dv2 = v_circular(mu, r2) - vis_viva(mu, r2, at);
    (dv1, dv2, PI * (at.powi(3) / mu).sqrt())
}

/// Bielíptica com raio intermediário `rb`: os três Δv e o tempo total.
pub fn bielliptic(mu: f64, r1: f64, r2: f64, rb: f64) -> ([f64; 3], f64) {
    let a1 = (r1 + rb) / 2.0;
    let a2 = (r2 + rb) / 2.0;
    let dv1 = vis_viva(mu, r1, a1) - v_circular(mu, r1);
    let dv2 = vis_viva(mu, rb, a2) - vis_viva(mu, rb, a1);
    let dv3 = vis_viva(mu, r2, a2) - v_circular(mu, r2);
    let t = PI * ((a1.powi(3) / mu).sqrt() + (a2.powi(3) / mu).sqrt());
    ([dv1, dv2, dv3], t)
}

/// Δv para girar o plano em `di` com velocidade `v`.
pub fn plane_change(v: f64, di: f64) -> f64 {
    2.0 * v * (di / 2.0).sin()
}

/// Equações de Clohessy-Wiltshire: estado relativo depois de `t`, em torno de um
/// alvo em órbita circular de movimento médio `n`. Eixos LVLH: x radial (para
/// fora), y ao longo da trajetória, z normal à órbita.
pub fn cw_propagate(r0: Vec3, v0: Vec3, n: f64, t: f64) -> (Vec3, Vec3) {
    let (s, c) = (n * t).sin_cos();
    let x = (4.0 - 3.0 * c) * r0.x + s / n * v0.x + 2.0 / n * (1.0 - c) * v0.y;
    let y = 6.0 * (s - n * t) * r0.x + r0.y - 2.0 / n * (1.0 - c) * v0.x
        + (4.0 * s - 3.0 * n * t) / n * v0.y;
    let z = c * r0.z + s / n * v0.z;
    let vx = 3.0 * n * s * r0.x + c * v0.x + 2.0 * s * v0.y;
    let vy = -6.0 * n * (1.0 - c) * r0.x - 2.0 * s * v0.x + (4.0 * c - 3.0) * v0.y;
    let vz = -n * s * r0.z + c * v0.z;
    (Vec3::new(x, y, z), Vec3::new(vx, vy, vz))
}

/// Velocidade relativa inicial que leva de `r0` a `rf` em `t` segundos (CW),
/// para as queimas de aproximação final do encontro.
pub fn cw_targeting(r0: Vec3, rf: Vec3, n: f64, t: f64) -> Vec3 {
    let (s, c) = (n * t).sin_cos();
    // Matrizes Φrr e Φrv no plano (x, y) e fora dele (z).
    let prr = [[4.0 - 3.0 * c, 0.0], [6.0 * (s - n * t), 1.0]];
    let prv = [[s / n, 2.0 / n * (1.0 - c)], [-2.0 / n * (1.0 - c), (4.0 * s - 3.0 * n * t) / n]];
    let bx = rf.x - (prr[0][0] * r0.x + prr[0][1] * r0.y);
    let by = rf.y - (prr[1][0] * r0.x + prr[1][1] * r0.y);
    let det = prv[0][0] * prv[1][1] - prv[0][1] * prv[1][0];
    let vx = (bx * prv[1][1] - prv[0][1] * by) / det;
    let vy = (prv[0][0] * by - prv[1][0] * bx) / det;
    let vz = if s.abs() > 1e-9 { (rf.z - c * r0.z) * n / s } else { 0.0 };
    Vec3::new(vx, vy, vz)
}

/// Eixos LVLH de um alvo: (radial, ao longo, normal) no inercial.
pub fn lvlh_axes(r: Vec3, v: Vec3) -> (Vec3, Vec3, Vec3) {
    let x = r.unit();
    let z = r.cross(v).unit();
    let y = z.cross(x);
    (x, y, z)
}

/// Eixos RTN (radial, transversal, normal): os mesmos do LVLH acima.
pub fn rtn_to_inertial(r: Vec3, v: Vec3, rtn: Vec3) -> Vec3 {
    let (x, y, z) = lvlh_axes(r, v);
    x * rtn.x + y * rtn.y + z * rtn.z
}

pub fn inertial_to_rtn(r: Vec3, v: Vec3, w: Vec3) -> Vec3 {
    let (x, y, z) = lvlh_axes(r, v);
    Vec3::new(w.dot(x), w.dot(y), w.dot(z))
}

/// Tempo até a anomalia verdadeira `nu_target` (sempre para a frente).
pub fn time_to_true_anomaly(el: &Elements, mu: f64, nu_target: f64) -> f64 {
    if el.e >= 1.0 {
        let m0 = true_to_mean(el.nu, el.e);
        let m1 = true_to_mean(nu_target, el.e);
        return (m1 - m0) / el.mean_motion(mu);
    }
    let m0 = true_to_mean(el.nu, el.e);
    let m1 = true_to_mean(nu_target, el.e);
    wrap_2pi(m1 - m0) / el.mean_motion(mu)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::DEG;

    const MU: f64 = 3.986_004_418e14;

    #[test]
    fn elementos_ida_e_volta() {
        let el = Elements { a: 7_000e3, e: 0.1, i: 51.6 * DEG, raan: 40.0 * DEG, argp: 30.0 * DEG, nu: 70.0 * DEG };
        let (r, v) = state_from_elements(&el, MU);
        let back = elements_from_state(r, v, MU);
        assert!((back.a - el.a).abs() < 1e-3);
        assert!((back.e - el.e).abs() < 1e-12);
        assert!((back.i - el.i).abs() < 1e-12);
        assert!((back.raan - el.raan).abs() < 1e-12);
        assert!((back.argp - el.argp).abs() < 1e-9);
        assert!((back.nu - el.nu).abs() < 1e-9);
    }

    #[test]
    fn periodo_da_estacao() {
        // Órbita de 420 km: cerca de 92,8 minutos.
        let el = Elements { a: 6_378_137.0 + 420e3, ..Default::default() };
        let p = el.period(MU) / 60.0;
        assert!((p - 92.8).abs() < 0.2, "{p}");
    }

    #[test]
    fn kepler_bate_com_os_elementos() {
        let el = Elements { a: 8_000e3, e: 0.3, i: 0.5, raan: 1.0, argp: 2.0, nu: 0.3 };
        let (r0, v0) = state_from_elements(&el, MU);
        let dt = 3000.0;
        let (r1, v1) = propagate_kepler(r0, v0, dt, MU);
        let m1 = el.mean_anomaly() + el.mean_motion(MU) * dt;
        let el1 = Elements { nu: mean_to_true(m1, el.e), ..el };
        let (r_ref, v_ref) = state_from_elements(&el1, MU);
        assert!((r1 - r_ref).norm() < 1e-2, "{}", (r1 - r_ref).norm());
        assert!((v1 - v_ref).norm() < 1e-5);
        // Hipérbole também.
        let hyp = Elements { a: -20_000e3, e: 1.4, i: 0.2, raan: 0.0, argp: 0.0, nu: 0.1 };
        let (r0, v0) = state_from_elements(&hyp, MU);
        let (r1, _) = propagate_kepler(r0, v0, 5000.0, MU);
        let m1 = hyp.mean_anomaly() + hyp.mean_motion(MU) * 5000.0;
        let (r_ref, _) = state_from_elements(&Elements { nu: mean_to_true(m1, hyp.e), ..hyp }, MU);
        assert!((r1 - r_ref).norm() < 1.0);
    }

    #[test]
    fn hohmann_leo_geo() {
        // De 300 km à geoestacionária: cerca de 3,9 km/s ao todo.
        let (a, b, t) = hohmann(MU, 6_678e3, 42_164e3);
        assert!(((a + b) - 3_890.0).abs() < 30.0, "{}", a + b);
        assert!((t / 3600.0 - 5.3).abs() < 0.1);
    }

    #[test]
    fn lambert_reproduz_a_propagacao() {
        let el = Elements { a: 9_000e3, e: 0.2, i: 0.3, raan: 0.4, argp: 0.5, nu: 0.1 };
        let (r1, v1) = state_from_elements(&el, MU);
        let tof = 2500.0;
        let (r2, v2) = propagate_kepler(r1, v1, tof, MU);
        let (lv1, lv2) = lambert(r1, r2, tof, MU, true).expect("solução");
        assert!((lv1 - v1).norm() < 0.05, "{}", (lv1 - v1).norm());
        assert!((lv2 - v2).norm() < 0.05);
    }

    #[test]
    fn cw_vai_aonde_mandou() {
        let n = (MU / (6_798e3f64).powi(3)).sqrt();
        let r0 = Vec3::new(-2_000.0, -10_000.0, 300.0);
        let rf = Vec3::new(0.0, -200.0, 0.0);
        let t = 2_700.0;
        let v0 = cw_targeting(r0, rf, n, t);
        let (r, _) = cw_propagate(r0, v0, n, t);
        assert!((r - rf).norm() < 1e-6);
    }
}

