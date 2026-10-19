//! Planejamento de manobras. Cada função devolve queimas (nós) prontas para a
//! fila do veículo, com o Δv no referencial RTN do ponto da queima.

use crate::bodies::Body;
use crate::guidance::predict_landing;
use crate::math::{TAU, Vec3, wrap_2pi};
use crate::orbit::*;
use crate::vehicle::{Node, Vehicle, great_circle_km};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct Plan {
    pub nodes: Vec<Node>,
    pub total_dv: f64,
    pub summary: String,
    /// Dados extras para a interface (ponto de pouso previsto, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub landing: Option<(f64, f64)>,
}

fn node(label: &str, t: f64, r: Vec3, v: Vec3, dv_inertial: Vec3) -> Node {
    Node {
        id: 0,
        label: label.into(),
        t,
        dv_rtn: inertial_to_rtn(r, v, dv_inertial),
        dir: None,
        remaining: 0.0,
        executing: false,
        done: false,
        rcs: false,
    }
}

fn plan(nodes: Vec<Node>, summary: String) -> Plan {
    let total = nodes.iter().map(|n| n.dv_rtn.norm()).sum();
    Plan { nodes, total_dv: total, summary, landing: None }
}

/// Estado do veículo daqui a `dt` (dois corpos).
fn state_at(v: &Vehicle, dt: f64) -> (Vec3, Vec3) {
    propagate_kepler(v.r, v.v, dt, v.central.mu())
}

fn time_to_apsis(v: &Vehicle, apo: bool) -> f64 {
    let el = elements_from_state(v.r, v.v, v.central.mu());
    let nu = if apo { std::f64::consts::PI } else { 0.0 };
    let dt = time_to_true_anomaly(&el, v.central.mu(), nu);
    // Muito perto: espera a próxima volta (precisa de tempo para orientar).
    if dt < 90.0 && el.e < 1.0 { dt + el.period(v.central.mu()) } else { dt }
}

fn re(v: &Vehicle) -> f64 {
    v.central.radius()
}

/// Circulariza no apoastro (ou periastro).
pub fn circularize(v: &Vehicle, at_apoapsis: bool) -> Option<Plan> {
    let mu = v.central.mu();
    let el = elements_from_state(v.r, v.v, mu);
    if el.e >= 1.0 && at_apoapsis {
        return None;
    }
    let dt = time_to_apsis(v, at_apoapsis);
    let (r, vel) = state_at(v, dt);
    let target = r.cross(vel).cross(r).unit() * v_circular(mu, r.norm());
    let dv = target - vel;
    let alt = (r.norm() - re(v)) / 1e3;
    Some(plan(vec![node("Circularização", v_t0(v) + dt, r, vel, dv)], format!("Circular a {:.0} km", alt)))
}

/// Instante "agora" do veículo: os nós usam tempo absoluto; o mundo soma.
fn v_t0(_v: &Vehicle) -> f64 {
    0.0
}

/// Muda o apoastro (queimando no periastro) ou o periastro (no apoastro).
pub fn change_apsis(v: &Vehicle, set_apoapsis: bool, alt_km: f64) -> Option<Plan> {
    let mu = v.central.mu();
    let dt = time_to_apsis(v, !set_apoapsis);
    let (r, vel) = state_at(v, dt);
    let rn = r.norm();
    let other = re(v) + alt_km * 1e3;
    let a = (rn + other) / 2.0;
    let speed = vis_viva(mu, rn, a);
    let dir = r.cross(vel).cross(r).unit();
    let dv = dir * speed - vel;
    let label = if set_apoapsis { "Ajuste do apoastro" } else { "Ajuste do periastro" };
    Some(plan(vec![node(label, dt, r, vel, dv)], format!("{} para {:.0} km", label, alt_km)))
}

/// Hohmann para uma órbita circular de altitude `alt_km`.
pub fn hohmann_to(v: &Vehicle, alt_km: f64) -> Option<Plan> {
    let mu = v.central.mu();
    let r2 = re(v) + alt_km * 1e3;
    let el = elements_from_state(v.r, v.v, mu);
    if el.e >= 1.0 {
        return None;
    }
    // Primeira queima num ápside: no periastro se subindo, no apoastro se descendo.
    let up = r2 > el.a;
    let dt1 = time_to_apsis(v, !up);
    let (r1, v1) = state_at(v, dt1);
    let rn1 = r1.norm();
    let at = (rn1 + r2) / 2.0;
    let dir1 = r1.cross(v1).cross(r1).unit();
    let dv1 = dir1 * vis_viva(mu, rn1, at) - v1;
    let v1_after = v1 + dv1;
    let t_transfer = std::f64::consts::PI * (at.powi(3) / mu).sqrt();
    let (r2v, v2) = propagate_kepler(r1, v1_after, t_transfer, mu);
    let dir2 = r2v.cross(v2).cross(r2v).unit();
    let dv2 = dir2 * v_circular(mu, r2v.norm()) - v2;
    Some(plan(
        vec![node("Hohmann: 1ª queima", dt1, r1, v1, dv1), node("Hohmann: 2ª queima", dt1 + t_transfer, r2v, v2, dv2)],
        format!("Transferência de Hohmann para {:.0} km ({:.0} min)", alt_km, t_transfer / 60.0),
    ))
}

/// Muda a inclinação no próximo nodo (mantendo o resto).
pub fn change_inclination(v: &Vehicle, inc_deg: f64) -> Option<Plan> {
    let mu = v.central.mu();
    let el = elements_from_state(v.r, v.v, mu);
    if el.e >= 1.0 {
        return None;
    }
    // Argumento de latitude atual; nodos em u = 0 e π.
    let n = el.mean_motion(mu);
    let mut best: Option<(f64, Vec3, Vec3)> = None;
    for u_node in [0.0, std::f64::consts::PI] {
        let nu_t = wrap_2pi(u_node - el.argp);
        let mut dt = time_to_true_anomaly(&el, mu, nu_t);
        if dt < 120.0 {
            dt += TAU / n;
        }
        let (r, vel) = state_at(v, dt);
        if best.as_ref().is_none_or(|b| dt < b.0) {
            best = Some((dt, r, vel));
        }
    }
    let (dt, r, vel) = best?;
    let di = inc_deg.to_radians() - el.i;
    // Gira a velocidade em torno do raio vetor pelo ângulo de inclinação.
    let ascending = vel.z > 0.0;
    let ang = if ascending { -di } else { di };
    let q = crate::math::Quat::from_axis_angle(r.unit(), ang);
    let new_v = q.rotate(vel);
    let dv = new_v - vel;
    Some(plan(vec![node("Mudança de plano", dt, r, vel, dv)], format!("Inclinação de {:.2}° para {:.2}°", el.i.to_degrees(), inc_deg)))
}

/// Ajusta o plano para o de outro veículo (nodo relativo).
pub fn match_plane(v: &Vehicle, target: &Vehicle) -> Option<Plan> {
    if target.central != v.central {
        return None;
    }
    let mu = v.central.mu();
    let h1 = v.r.cross(v.v).unit();
    let h2 = target.r.cross(target.v).unit();
    let line = h1.cross(h2);
    if line.norm() < 1e-6 {
        return None;
    }
    let el = elements_from_state(v.r, v.v, mu);
    if el.e >= 1.0 {
        return None;
    }
    // Procura o ponto da órbita mais próximo da linha dos nodos relativos.
    let period = el.period(mu);
    let mut best = (f64::INFINITY, 0.0);
    let steps = 360;
    for k in 1..=steps {
        let dt = period * k as f64 / steps as f64;
        let (r, _) = state_at(v, dt);
        let a = r.unit().cross(line.unit()).norm();
        if a < best.0 && dt > 120.0 {
            best = (a, dt);
        }
    }
    let dt = best.1;
    let (r, vel) = state_at(v, dt);
    // Nova velocidade: mesmo módulo e componente radial, no plano do alvo.
    let vr = r.unit() * vel.dot(r.unit());
    let vt = vel - vr;
    let new_dir = h2.cross(r).unit();
    let new_v = vr + new_dir * vt.norm();
    let dv = new_v - vel;
    let ang = h1.angle(h2).to_degrees();
    Some(plan(vec![node("Igualar plano ao alvo", dt, r, vel, dv)], format!("Diferença de plano de {:.2}°", ang)))
}

/// Saída de órbita: retroqueima que leva o periastro a `pe_km`. Com alvo de
/// pouso, procura nas próximas três voltas o instante que leva mais perto dele.
pub fn deorbit(v: &Vehicle, t_now: f64, pe_km: f64, target: Option<(f64, f64)>) -> Option<Plan> {
    let mu = v.central.mu();
    let el = elements_from_state(v.r, v.v, mu);
    if el.e >= 1.0 {
        return None;
    }
    let period = el.period(mu);
    let _ = period;
    // Estados futuros com J2 a cada 60 s: em dois dias o plano da órbita gira
    // vários graus, e o ponto de pouso anda centenas de km.
    let horizon = if target.is_some() { 2.0 * 86_400.0 + 600.0 } else { 1_200.0 };
    let mut states = vec![(v.r, v.v)];
    let (mut r, mut vel) = (v.r, v.v);
    let mut s_t = 0.0;
    while s_t < horizon {
        let (r2, v2) = crate::vehicle::propagate_gravity(v.central, r, vel, 60.0, 20.0);
        r = r2;
        vel = v2;
        s_t += 60.0;
        states.push((r, vel));
    }
    let state_at_j2 = |dt: f64| -> (Vec3, Vec3) {
        let k = ((dt / 60.0).floor() as usize).min(states.len() - 1);
        let (r, vel) = states[k];
        crate::vehicle::propagate_gravity(v.central, r, vel, dt - k as f64 * 60.0, 20.0)
    };
    let make = |dt: f64| -> (Vec3, Vec3, Vec3) {
        let (r, vel) = state_at_j2(dt);
        let rn = r.norm();
        let rp = re(v) + pe_km * 1e3;
        let a = (rn + rp) / 2.0;
        let dir = r.cross(vel).cross(r).unit();
        let vr = r.unit() * vel.dot(r.unit());
        let dv = dir * vis_viva(mu, rn, a) + vr - vel;
        (r, vel, dv)
    };
    // Nave como chega na reentrada (sem os estágios de baixo).
    let mut entry_v = v.clone();
    while entry_v.stages.len() > 1 {
        entry_v.stages.remove(0);
    }
    let land = |dt: f64| -> Option<(f64, f64, f64)> {
        let (r, vel, dv) = make(dt);
        let mut ev = entry_v.clone();
        ev.r = r;
        ev.v = vel + dv;
        // Avança até 140 km.
        let mut tt = 0.0;
        for _ in 0..400 {
            if ev.central.altitude(ev.r) < 140e3 {
                break;
            }
            let (r2, v2) = propagate_kepler(ev.r, ev.v, 20.0, mu);
            ev.r = r2;
            ev.v = v2;
            tt += 20.0;
        }
        let bank = if v.kind == crate::vehicle::Kind::Shuttle { 50.0 } else { 55.0 };
        predict_landing(&ev, t_now + dt + tt, bank).map(|(la, lo, _)| (la, lo, tt))
    };
    let (dt, landing) = match target {
        Some((lat, lon)) => {
            let mut best: Option<(f64, f64, (f64, f64))> = None;
            // Dois dias de voltas: a faixa do solo passa perto do alvo em algumas.
            let mut dt = 300.0;
            while dt < 2.0 * 86_400.0 {
                if let Some((la, lo, _)) = land(dt) {
                    let d = great_circle_km(la, lo, lat, lon);
                    if best.is_none_or(|b| d < b.1) {
                        best = Some((dt, d, (la, lo)));
                    }
                }
                dt += 60.0;
            }
            // Refina em torno do melhor.
            let (mut bdt, mut bd, mut bl) = best?;
            let mut s = -60.0;
            while s <= 60.0 {
                if let Some((la, lo, _)) = land(bdt + s) {
                    let d = great_circle_km(la, lo, lat, lon);
                    if d < bd {
                        bd = d;
                        bl = (la, lo);
                        bdt += s;
                        s = -60.0;
                        continue;
                    }
                }
                s += 5.0;
            }
            (bdt, Some(bl))
        }
        None => (300.0, land(300.0).map(|(a, b, _)| (a, b))),
    };
    let (r, vel, dv) = make(dt);
    let mut p = plan(vec![node("Saída de órbita", dt, r, vel, dv)], match landing {
        Some((la, lo)) => format!("Retroqueima de {:.0} m/s; pouso previsto em {:.2}°, {:.2}°", dv.norm(), la, lo),
        None => format!("Retroqueima de {:.0} m/s para periastro de {:.0} km", dv.norm(), pe_km),
    });
    p.landing = landing;
    Some(p)
}

/// Encontro com outro veículo: duas queimas (Lambert) que levam a 1 km atrás
/// dele, escolhendo a partida e a duração de menor Δv nas próximas voltas.
pub fn rendezvous(v: &Vehicle, target: &Vehicle) -> Option<Plan> {
    if target.central != v.central {
        return None;
    }
    let mu = v.central.mu();
    let el = elements_from_state(v.r, v.v, mu);
    let el_t = elements_from_state(target.r, target.v, mu);
    if el.e >= 1.0 || el_t.e >= 1.0 {
        return None;
    }
    let p = el.period(mu);
    let mut best: Option<(f64, f64, f64, Vec3, Vec3, Vec3, Vec3, Vec3)> = None;
    let mut dep = 600.0;
    // Até dois dias de espera: a diferença de altitude faz a fase andar sozinha.
    while dep < 2.0 * 86_400.0 {
        let (r1, v1) = state_at(v, dep);
        let mut tof = 0.35 * p;
        while tof < 1.6 * p {
            let (rt, vt) = propagate_kepler(target.r, target.v, dep + tof, mu);
            // Ponto 1 km atrás do alvo, na V-bar.
            let aim = rt - vt.unit() * 1_000.0;
            for prograde in [true] {
                if let Some((lv1, lv2)) = lambert(r1, aim, tof, mu, prograde) {
                    let dv1 = lv1 - v1;
                    let dv2 = vt - lv2;
                    let total = dv1.norm() + dv2.norm();
                    // Não pode passar por dentro da Terra.
                    let rp = elements_from_state(r1, lv1, mu).periapsis();
                    if rp > re(v) + 120e3 && best.as_ref().is_none_or(|b| total < b.0) {
                        best = Some((total, dep, tof, r1, v1, dv1, aim, dv2));
                    }
                }
            }
            tof += 0.05 * p;
        }
        dep += p / 12.0;
    }
    let (total, dep, tof, r1, v1, dv1, _aim, dv2) = best?;
    let (r2, v2) = propagate_kepler(r1, v1 + dv1, tof, mu);
    Some(plan(
        vec![node("Encontro: transferência", dep, r1, v1, dv1), node("Encontro: igualar velocidade", dep + tof, r2, v2, dv2)],
        format!("Encontro em {:.1} h com {:.0} m/s; aproximação final por acoplamento automático", (dep + tof) / 3600.0, total),
    ))
}

/// Injeção para outro corpo: da órbita de estacionamento (em torno da Terra)
/// para chegar a `to` em `arrival` (instante absoluto). Para a Lua, Lambert no
/// referencial da Terra; para planetas, o v∞ heliocêntrico.
pub fn injection(v: &Vehicle, t_now: f64, to: Body, depart_in: f64, tof: f64) -> Option<Plan> {
    if v.central != Body::Earth {
        return None;
    }
    let mu = v.central.mu();
    let t_dep = t_now + depart_in;
    let t_arr = t_dep + tof;
    let el = elements_from_state(v.r, v.v, mu);
    if el.e >= 1.0 {
        return None;
    }
    if to == Body::Moon {
        // Lambert direto: do ponto da órbita à Lua (mirando 2000 km à frente dela).
        let mut best: Option<(f64, f64, Vec3, Vec3, Vec3)> = None;
        let period = el.period(mu);
        let mut dt = depart_in;
        while dt < depart_in + period {
            let (r1, v1) = state_at(v, dt);
            let (rm, vm) = Body::Moon.state(t_now + dt + tof);
            let aim = rm + vm.unit().cross(rm.cross(vm).unit()) * -2_000e3;
            if let Some((lv1, _)) = lambert(r1, aim, tof, mu, true) {
                let dv = lv1 - v1;
                if best.as_ref().is_none_or(|b| dv.norm() < b.0) {
                    best = Some((dv.norm(), dt, r1, v1, dv));
                }
            }
            dt += period / 90.0;
        }
        let (dvn, dt, r1, v1, dv) = best?;
        return Some(plan(vec![node("Injeção translunar", dt, r1, v1, dv)], format!("Injeção translunar de {:.0} m/s; chegada em {:.1} dias", dvn, tof / 86_400.0)));
    }
    // Planetas: v∞ pelo Lambert heliocêntrico.
    let (re_, ve_) = Body::Earth.helio_state(t_dep);
    let (rp, _) = to.helio_state(t_arr);
    let (lv1, _) = lambert(re_, rp, tof, Body::Sun.mu(), true)?;
    let vinf = lv1 - ve_;
    let vinf_n = vinf.norm();
    // Ponto de queima: o periastro da hipérbole de partida, no plano da órbita
    // de estacionamento, com a assíntota alinhada ao v∞ projetado.
    let h = v.r.cross(v.v).unit();
    let s = vinf.reject(h).unit();
    let rp_n = v.r.norm();
    let e = 1.0 + rp_n * vinf_n * vinf_n / mu;
    let nu_inf = (-1.0 / e).acos();
    let q = crate::math::Quat::from_axis_angle(h, -nu_inf);
    let peri_dir = q.rotate(s);
    // Quando o veículo passa por essa direção.
    let period = el.period(mu);
    let mut best = (f64::INFINITY, 0.0);
    let n = 720;
    for k in 0..n {
        let dt = depart_in + period * k as f64 / n as f64;
        let (r, _) = state_at(v, dt);
        let a = r.unit().angle(peri_dir);
        if a < best.0 {
            best = (a, dt);
        }
    }
    let dt = best.1;
    let (r1, v1) = state_at(v, dt);
    let v_need = (vinf_n * vinf_n + 2.0 * mu / r1.norm()).sqrt();
    let dv = v1.unit() * (v_need - v1.norm());
    let c3 = vinf_n * vinf_n / 1e6;
    Some(plan(
        vec![node(&format!("Injeção para {}", to.name()), dt, r1, v1, dv)],
        format!("C3 = {:.1} km²/s², Δv {:.0} m/s; chegada em {:.0} dias (corrigir com TCM no cruzeiro)", c3, dv.norm(), tof / 86_400.0),
    ))
}

/// Correção de trajetória no cruzeiro heliocêntrico (ou no caminho para a Lua):
/// Lambert até o alvo em `arrival`, mirando um periastro de `pe_km`.
pub fn tcm(v: &Vehicle, t_now: f64, to: Body, arrival: f64, pe_km: f64) -> Option<Plan> {
    let central = v.central;
    let mu = central.mu();
    let dt = 600.0;
    let (r1, v1) = state_at(v, dt);
    let tof = arrival - (t_now + dt);
    if tof <= 0.0 {
        return None;
    }
    let (rt, vt) = to.state_relative_to(central, arrival);
    // Mira deslocada do centro pelo parâmetro de impacto que dá o periastro pedido.
    let rp = to.radius() + pe_km * 1e3;
    let v_rel = (vt - v1).norm().max(100.0);
    let b = rp * (1.0 + 2.0 * to.mu() / (rp * v_rel * v_rel)).sqrt();
    let side = vt.cross(rt).cross(vt).unit();
    let aim = rt + side * b;
    let (lv1, _) = lambert(r1, aim, tof, mu, true)?;
    let dv = lv1 - v1;
    Some(plan(vec![node("Correção de trajetória (TCM)", dt, r1, v1, dv)], format!("TCM de {:.1} m/s para chegar a {} com periastro de {:.0} km", dv.norm(), to.name(), pe_km)))
}

/// Captura (inserção orbital) no periastro de uma hipérbole de chegada.
pub fn capture(v: &Vehicle, apo_km: f64) -> Option<Plan> {
    let mu = v.central.mu();
    let el = elements_from_state(v.r, v.v, mu);
    let dt = if el.e >= 1.0 {
        let dt = time_to_true_anomaly(&el, mu, 0.0);
        if dt <= 0.0 { return None; }
        dt
    } else {
        time_to_apsis(v, false)
    };
    let (r, vel) = state_at(v, dt);
    let rn = r.norm();
    let ra = re(v) + apo_km * 1e3;
    let a = (rn + ra.max(rn)) / 2.0;
    let dv = vel.unit() * (vis_viva(mu, rn, a) - vel.norm());
    Some(plan(vec![node(&format!("Inserção na órbita de {}", v.central.name()), dt, r, vel, dv)], format!("Captura com {:.0} m/s: órbita de {:.0} × {:.0} km", dv.norm(), (rn - re(v)) / 1e3, apo_km)))
}

#[derive(Clone, Debug, Serialize)]
pub struct Porkchop {
    pub from: Body,
    pub to: Body,
    pub departures: Vec<f64>,
    pub tofs_days: Vec<f64>,
    /// C3 de partida (km²/s²) [tof][partida]; NaN sem solução.
    pub c3: Vec<Vec<f64>>,
    /// v∞ de chegada (km/s).
    pub vinf_arr: Vec<Vec<f64>>,
    pub best: Option<(f64, f64, f64, f64)>,
}

/// Gráfico porkchop: C3 de partida e v∞ de chegada numa grade de datas.
pub fn porkchop(from: Body, to: Body, t_start: f64, span_days: f64, tof_min_days: f64, tof_max_days: f64, n: usize) -> Porkchop {
    let n = n.clamp(5, 80);
    let parent = if to == Body::Moon { Body::Earth } else { Body::Sun };
    let departures: Vec<f64> = (0..n).map(|i| t_start + span_days * 86_400.0 * i as f64 / (n - 1) as f64).collect();
    let tofs_days: Vec<f64> = (0..n).map(|j| tof_min_days + (tof_max_days - tof_min_days) * j as f64 / (n - 1) as f64).collect();
    let mut c3 = vec![vec![f64::NAN; n]; n];
    let mut va = vec![vec![f64::NAN; n]; n];
    let mut best: Option<(f64, f64, f64, f64)> = None;
    for (j, &tof_d) in tofs_days.iter().enumerate() {
        for (i, &td) in departures.iter().enumerate() {
            let tof = tof_d * 86_400.0;
            let (r1, v1) = from.state_relative_to(parent, td);
            let (r2, v2) = to.state_relative_to(parent, td + tof);
            if let Some((lv1, lv2)) = lambert(r1, r2, tof, parent.mu(), true) {
                let c = (lv1 - v1).norm2() / 1e6;
                let a = (lv2 - v2).norm() / 1e3;
                if c.is_finite() && c < 200.0 {
                    c3[j][i] = c;
                    va[j][i] = a;
                    let score = c.sqrt() + a;
                    if best.is_none_or(|b| score < b.2.sqrt() + b.3) {
                        best = Some((td, tof_d, c, a));
                    }
                }
            }
        }
    }
    Porkchop { from, to, departures, tofs_days, c3, vinf_arr: va, best }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;
    use crate::time::from_calendar;

    fn in_orbit(id: &str, alt_km: f64, inc: f64) -> Vehicle {
        let mut v = catalog::build(id).unwrap();
        let el = Elements { a: Body::Earth.radius() + alt_km * 1e3, e: 0.0, i: inc.to_radians(), raan: 0.3, argp: 0.0, nu: 0.5 };
        let (r, vel) = state_from_elements(&el, Body::Earth.mu());
        v.r = r;
        v.v = vel;
        v
    }

    #[test]
    fn hohmann_planejado() {
        let v = in_orbit("amazonia", 400.0, 51.6);
        let p = hohmann_to(&v, 800.0).unwrap();
        assert_eq!(p.nodes.len(), 2);
        // 400 → 800 km: ~ 215 m/s.
        assert!((p.total_dv - 215.0).abs() < 15.0, "{}", p.total_dv);
    }

    #[test]
    fn porkchop_marte_2026() {
        // Janela de Marte do fim de 2026: C3 mínimo ~ 9–14 km²/s².
        let t0 = from_calendar(2026, 9, 1, 0, 0, 0.0);
        let pc = porkchop(Body::Earth, Body::Mars, t0, 180.0, 150.0, 350.0, 25);
        let (_, _, c3, _) = pc.best.unwrap();
        assert!(c3 > 5.0 && c3 < 25.0, "{c3}");
    }

    #[test]
    fn encontro_acha_solucao() {
        let v = in_orbit("amazonia", 380.0, 51.6);
        let mut t = in_orbit("estacao", 420.0, 51.6);
        let el = Elements { a: Body::Earth.radius() + 420e3, e: 0.0, i: 51.6f64.to_radians(), raan: 0.3, argp: 0.0, nu: 1.2 };
        let (r, vel) = state_from_elements(&el, Body::Earth.mu());
        t.r = r;
        t.v = vel;
        let p = rendezvous(&v, &t).unwrap();
        assert!(p.total_dv < 200.0, "{}", p.total_dv);
    }
}
