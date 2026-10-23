//! Cruzeiro heliocêntrico com n corpos: o Sol puxa a sonda e os planetas
//! perturbam (Vênus, Terra, Lua, Marte, Júpiter e Saturno; os outros mexem
//! menos que um milímetro por segundo numa viagem a Marte). O mesmo modelo
//! serve à simulação, à previsão do traço e ao planejador, que mira o plano B
//! do planeta de chegada por correção diferencial.

use crate::bodies::Body;
use crate::math::Vec3;

/// Quem perturba no cruzeiro heliocêntrico.
pub const PERTURBERS: [Body; 6] = [Body::Venus, Body::Earth, Body::Moon, Body::Mars, Body::Jupiter, Body::Saturn];

/// Maré de um corpo de parâmetro `mu` em `rp` sobre a sonda em `r` (os dois
/// relativos ao Sol): a atração direta menos a que o corpo faz no Sol.
pub fn tide(r: Vec3, rp: Vec3, mu: f64) -> Vec3 {
    let d = rp - r;
    let dn = d.norm();
    let pn = rp.norm();
    d * (mu / (dn * dn * dn)) - rp * (mu / (pn * pn * pn))
}

/// Só a parte dos planetas (o Sol fica com o integrador do veículo).
pub fn perturbation(r: Vec3, t: f64) -> Vec3 {
    let mut a = Vec3::ZERO;
    for p in PERTURBERS {
        a += tide(r, p.helio_state(t).0, p.mu());
    }
    a
}

/// Aceleração heliocêntrica em `r` no instante `t`, sem o corpo `skip` (o
/// planejador tira o planeta de chegada quando mira a posição lá).
pub fn accel(r: Vec3, t: f64, skip: Option<Body>) -> Vec3 {
    let rn = r.norm();
    let mut a = r * (-Body::Sun.mu() / (rn * rn * rn));
    for p in PERTURBERS {
        if Some(p) != skip {
            a += tide(r, p.helio_state(t).0, p.mu());
        }
    }
    a
}

/// Passo que acompanha o que muda mais depressa: a volta em torno do Sol e a
/// passagem perto de um planeta.
pub fn step_size(r: Vec3, v: Vec3, t: f64) -> f64 {
    let rn = r.norm();
    let mut h = 0.002 * (rn * rn * rn / Body::Sun.mu()).sqrt();
    for p in PERTURBERS {
        let (rp, vp) = p.helio_state(t);
        let d = (r - rp).norm();
        let w = (v - vp).norm().max(1.0);
        h = h.min(0.02 * d / w);
    }
    h.clamp(30.0, 21_600.0)
}

fn rk4(r: Vec3, v: Vec3, t: f64, h: f64, skip: Option<Body>) -> (Vec3, Vec3) {
    let a1 = accel(r, t, skip);
    let (r2, v2) = (r + v * (h / 2.0), v + a1 * (h / 2.0));
    let a2 = accel(r2, t + h / 2.0, skip);
    let (r3, v3) = (r + v2 * (h / 2.0), v + a2 * (h / 2.0));
    let a3 = accel(r3, t + h / 2.0, skip);
    let (r4, v4) = (r + v3 * h, v + a3 * h);
    let a4 = accel(r4, t + h, skip);
    (r + (v + v2 * 2.0 + v3 * 2.0 + v4) * (h / 6.0), v + (a1 + a2 * 2.0 + a3 * 2.0 + a4) * (h / 6.0))
}

/// Propaga de `t0` até `t1` ou até `stop` dizer que basta. Devolve o instante
/// e o estado em que parou.
pub fn propagate(mut r: Vec3, mut v: Vec3, t0: f64, t1: f64, skip: Option<Body>, mut stop: impl FnMut(f64, Vec3, Vec3) -> bool) -> (f64, Vec3, Vec3) {
    let mut t = t0;
    while t < t1 {
        let h = step_size(r, v, t).min(t1 - t);
        (r, v) = rk4(r, v, t, h, skip);
        t += h;
        if stop(t, r, v) {
            break;
        }
    }
    (t, r, v)
}

/// Amostras (t, r) a cada `step` segundos, para o traço previsto.
pub fn samples(r: Vec3, v: Vec3, t0: f64, span: f64, step: f64) -> Vec<(f64, Vec3)> {
    let mut out = vec![(t0, r)];
    let (mut r, mut v, mut t) = (r, v, t0);
    let mut next = t0 + step;
    while t < t0 + span {
        let h = step_size(r, v, t).min(next - t).max(1e-3);
        (r, v) = rk4(r, v, t, h, None);
        t += h;
        if t >= next - 1e-6 {
            out.push((t, r));
            next += step;
        }
    }
    out
}

/// Entra na esfera de influência de `to`: instante e estado relativo a ele.
pub fn soi_entry(r: Vec3, v: Vec3, t0: f64, to: Body, t_max: f64) -> Option<(f64, Vec3, Vec3)> {
    let soi = to.soi();
    let mut hit = None;
    propagate(r, v, t0, t_max, None, |t, r, v| {
        let (rp, vp) = to.helio_state(t);
        if (r - rp).norm() < soi {
            hit = Some((t, r - rp, v - vp));
            true
        } else {
            false
        }
    });
    hit
}

/// Plano B da hipérbole de chegada: (B·T, B·R) em metros, a direção S da
/// assíntota de chegada e o v∞. T fica no plano do equador (perpendicular a
/// S e ao polo do referencial) e R = S × T.
pub fn b_plane(r: Vec3, v: Vec3, mu: f64) -> Option<(f64, f64, Vec3, f64)> {
    let rn = r.norm();
    let energy = v.norm2() / 2.0 - mu / rn;
    if energy <= 0.0 {
        return None;
    }
    let vinf = (2.0 * energy).sqrt();
    let h = r.cross(v);
    let e_vec = v.cross(h) / mu - r / rn;
    let e = e_vec.norm();
    if e <= 1.0 {
        return None;
    }
    let p = e_vec / e;
    let q = h.unit().cross(p);
    // Direção de chegada (a velocidade lá longe, antes do periastro).
    let s = (p + q * (e * e - 1.0).sqrt()) / e;
    let b = s.cross(h) / vinf;
    let t_ax = s.cross(Vec3::Z).unit();
    let r_ax = s.cross(t_ax);
    Some((b.dot(t_ax), b.dot(r_ax), s, vinf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plano_b_de_uma_hiperbole_conhecida() {
        // Chegando de longe em linha quase reta, a 10 000 km de lado.
        let mu = Body::Mars.mu();
        let r = Vec3::new(-5e8, 1e7, 0.0);
        let v = Vec3::new(3_000.0, 0.0, 0.0);
        let (bt, br, s, vinf) = b_plane(r, v, mu).unwrap();
        let b = (bt * bt + br * br).sqrt();
        println!("B {b:.0} m, S {s:?}, v∞ {vinf:.1}");
        // O raio de 10 000 km, a 500 000 km, é quase o B (a curva ainda é pequena).
        assert!((b - 1e7).abs() < 2e5, "B = {b}");
        assert!(s.x > 0.99);
        assert!((vinf - (9e6 - 2.0 * mu / r.norm()).sqrt()).abs() < 1e-6);
    }

    #[test]
    fn sem_planetas_por_perto_e_kepler() {
        // Uma volta curta longe de todos: bate com dois corpos.
        let t = crate::time::from_calendar(2026, 11, 1, 0, 0, 0.0);
        let r = Vec3::new(0.0, -2.2e11, 0.0).rot_x(0.4);
        let v = Vec3::new(24_000.0, 0.0, 0.0).rot_x(0.4);
        let (_, r1, _) = propagate(r, v, t, t + 10.0 * 86_400.0, None, |_, _, _| false);
        let (r2, _) = crate::orbit::propagate_kepler(r, v, 10.0 * 86_400.0, Body::Sun.mu());
        let err = (r1 - r2).norm();
        println!("diferença em 10 dias: {:.0} km", err / 1e3);
        assert!(err < 200e3, "{err}");
    }
}
