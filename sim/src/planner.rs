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
        target_energy: None,
        ignite_at: None,
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
    let shift = if to == Body::Moon { 0.0 } else { best_departure(v, t_now, to, depart_in, tof) - depart_in };
    let depart_in = depart_in + shift;
    let mu = v.central.mu();
    let t_dep = t_now + depart_in;
    let t_arr = t_dep + tof;
    let el = elements_from_state(v.r, v.v, mu);
    if el.e >= 1.0 {
        return None;
    }
    // Estado na hora da partida propagado com J2: em dias de espera o plano da
    // órbita de estacionamento gira dezenas de graus.
    let mut vd = v.clone();
    let (rd, vdv) = crate::vehicle::propagate_gravity(v.central, v.r, v.v, depart_in, 30.0);
    vd.r = rd;
    vd.v = vdv;
    let v = &vd;
    let base = depart_in;
    let depart_in = 0.0;
    if to == Body::Moon {
        // Lambert direto: do ponto da órbita à Lua (mirando 2000 km à frente dela).
        let mut best: Option<(f64, f64, Vec3, Vec3, Vec3)> = None;
        let period = el.period(mu);
        let mut dt = depart_in;
        while dt < depart_in + period {
            let (r1, v1) = state_at(v, dt);
            let (rm, vm) = Body::Moon.state(t_now + base + dt + tof);
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
        return Some(plan(vec![node("Injeção translunar", base + dt, r1, v1, dv)], format!("Injeção translunar de {:.0} m/s; chegada em {:.1} dias", dvn, tof / 86_400.0)));
    }
    // Planetas: v∞ pelo Lambert heliocêntrico.
    let (re_, ve_) = Body::Earth.helio_state(t_dep);
    let (rp, _) = to.helio_state(t_arr);
    let (lv1, _) = lambert(re_, rp, tof, Body::Sun.mu(), true)?;
    let vinf = lv1 - ve_;
    let vinf_n = vinf.norm();
    // Queima longa (minutos): simula a queima de verdade, prógrada, até a
    // energia do v∞, e escolhe a ignição que deixa a assíntota mais perto da
    // direção pedida.
    let (f, mdot) = crate::guidance::burn_capacity(v);
    if f > 0.0 {
        let target_e = vinf_n * vinf_n / 2.0;
        let prop: f64 = v.active_stages().iter().map(|&i| v.stages[i].propellant).sum();
        let period = el.period(mu);
        let try_at = |t_ign: f64| -> Option<(f64, f64)> {
            let (r0, v0) = state_at(v, t_ign);
            let (r1, v1, dur) = simulate_burn(v.central, r0, v0, v.mass(), f, mdot, prop, target_e)?;
            let (_, vfar) = propagate_kepler(r1, v1, 20.0 * 86_400.0, mu);
            Some((vfar.angle(vinf), dur))
        };
        let mut best: Option<(f64, f64, f64)> = None;
        let n = 180;
        for k in 0..n {
            let tk = period * k as f64 / n as f64;
            if let Some((ang, dur)) = try_at(tk) {
                if best.is_none_or(|b| ang < b.1) {
                    best = Some((tk, ang, dur));
                }
            }
        }
        if let Some((mut tb, mut ab, mut db)) = best {
            // Refina em passos menores em volta.
            let mut step = period / n as f64 / 2.0;
            while step > 1.0 {
                for cand in [tb - step, tb + step] {
                    if let Some((ang, dur)) = try_at(cand.max(0.0)) {
                        if ang < ab {
                            tb = cand.max(0.0);
                            ab = ang;
                            db = dur;
                        }
                    }
                }
                step /= 2.0;
            }
            let (r1, v1) = state_at(v, tb);
            let v_need = (vinf_n * vinf_n + 2.0 * mu / r1.norm()).sqrt();
            let dv = v1.unit() * (v_need - v1.norm());
            let mut nd = node(&format!("Injeção para {}", to.name()), base + tb + db / 2.0, r1, v1, dv);
            nd.target_energy = Some(target_e);
            nd.ignite_at = Some(base + tb);
            let c3 = vinf_n * vinf_n / 1e6;
            return Some(plan(
                vec![nd],
                format!(
                    "C3 = {:.1} km²/s², queima de {:.0} s (assíntota a {:.1}° do v∞ pedido); chegada em {:.0} dias (corrigir com TCM no cruzeiro)",
                    c3,
                    db,
                    ab.to_degrees(),
                    tof / 86_400.0
                ),
            ));
        }
    }
    // Sem propulsão para simular: ponto da queima impulsiva, o periastro da
    // hipérbole de partida, com a assíntota alinhada ao v∞ projetado.
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
        vec![node(&format!("Injeção para {}", to.name()), base + dt, r1, v1, dv)],
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
    // Mira deslocada do centro pelo parâmetro de impacto que dá o periastro
    // pedido, com o v∞ de chegada de um primeiro Lambert ao centro.
    let rp = to.radius() + pe_km * 1e3;
    let v_rel = lambert(r1, rt, tof, mu, true).map_or((vt - v1).norm(), |(_, v2)| (v2 - vt).norm()).max(100.0);
    let b = rp * (1.0 + 2.0 * to.mu() / (rp * v_rel * v_rel)).sqrt();
    let side = vt.cross(rt).cross(vt).unit();
    let aim = rt + side * b;
    let (lv1, _) = lambert(r1, aim, tof, mu, true)?;
    let mut dv = lv1 - v1;
    // No cruzeiro heliocêntrico, o Lambert é só o chute: a correção
    // diferencial com os planetas acerta o plano B na chegada.
    let mut note = "";
    if central == Body::Sun {
        let t1 = t_now + dt;
        match target_position(r1, v1, t1, to, arrival, aim - rt).and_then(|d| target_b_plane(r1, v1, t1, d, to, arrival, rp, side)) {
            Some(d) => dv = d,
            None => note = " (sem convergir com n corpos: só Lambert)",
        }
    }
    Some(plan(vec![node("Correção de trajetória (TCM)", dt, r1, v1, dv)], format!("TCM de {:.1} m/s para chegar a {} com periastro de {:.0} km{note}", dv.norm(), to.name(), pe_km)))
}

/// Primeira fase: a queima que leva a sonda, com os planetas mas sem o de
/// chegada, ao ponto `aim` (relativo a `to`) no instante `arrival`. Newton
/// 3×3 a partir de nenhuma queima.
fn target_position(r1: Vec3, v1: Vec3, t1: f64, to: Body, arrival: f64, aim: Vec3) -> Option<Vec3> {
    let at = |dv: Vec3| {
        let (_, r, _) = crate::nbody::propagate(r1, v1 + dv, t1, arrival, Some(to), |_, _, _| false);
        r - to.helio_state(arrival).0
    };
    let mut dv = Vec3::ZERO;
    for _ in 0..8 {
        let p = at(dv);
        let err = aim - p;
        if err.norm() < 0.1 * to.soi() {
            return Some(dv);
        }
        let eps = 0.05;
        let cols: Vec<Vec3> = [Vec3::X, Vec3::Y, Vec3::Z].into_iter().map(|e| (at(dv + e * eps) - p) / eps).collect();
        // Cramer: J·x = erro.
        let det = cols[0].dot(cols[1].cross(cols[2]));
        if det.abs() < 1e-9 {
            return None;
        }
        let x = Vec3::new(err.dot(cols[1].cross(cols[2])), cols[0].dot(err.cross(cols[2])), cols[0].dot(cols[1].cross(err))) / det;
        dv += if x.norm() > 300.0 { x.unit() * 300.0 } else { x };
    }
    None
}

/// Plano B na entrada da esfera de `to` depois de uma queima `dv` em `t1`
/// (n corpos). Devolve (B·T, B·R), a direção de chegada e o v∞.
fn b_after(r1: Vec3, v1: Vec3, t1: f64, dv: Vec3, to: Body, t_max: f64) -> Option<(f64, f64, Vec3, f64)> {
    let (_, r, v) = crate::nbody::soi_entry(r1, v1 + dv, t1, to, t_max)?;
    crate::nbody::b_plane(r, v, to.mu())
}

/// Newton no plano B: corrige `dv` até o B da chegada ter o tamanho que dá o
/// periastro `rp` (na direção de `side`, a da mira). Duas metas e três
/// incógnitas: a correção de menor norma, Jᵀ(JJᵀ)⁻¹·erro.
#[allow(clippy::too_many_arguments)]
fn target_b_plane(r1: Vec3, v1: Vec3, t1: f64, mut dv: Vec3, to: Body, arrival: f64, rp: f64, side: Vec3) -> Option<Vec3> {
    let t_max = arrival + 60.0 * 86_400.0;
    for _ in 0..10 {
        let (bt, br, s, vinf) = b_after(r1, v1, t1, dv, to, t_max)?;
        // B pedido: o parâmetro de impacto do periastro, na direção da mira.
        let b = rp * (1.0 + 2.0 * to.mu() / (rp * vinf * vinf)).sqrt();
        let t_ax = s.cross(Vec3::Z).unit();
        let r_ax = s.cross(t_ax);
        let dir = (side - s * side.dot(s)).unit();
        let (gt, gr) = (b * dir.dot(t_ax), b * dir.dot(r_ax));
        let (et, er) = (gt - bt, gr - br);
        if (et * et + er * er).sqrt() < 5e3 {
            return Some(dv);
        }
        // Jacobiano por diferenças (0,05 m/s em cada eixo).
        let eps = 0.05;
        let mut j = [[0.0; 3]; 2];
        for (k, e) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
            let (bt2, br2, _, _) = b_after(r1, v1, t1, dv + e * eps, to, t_max)?;
            j[0][k] = (bt2 - bt) / eps;
            j[1][k] = (br2 - br) / eps;
        }
        // (JJᵀ)⁻¹ de 2×2.
        let a = j[0][0] * j[0][0] + j[0][1] * j[0][1] + j[0][2] * j[0][2];
        let bb = j[0][0] * j[1][0] + j[0][1] * j[1][1] + j[0][2] * j[1][2];
        let d = j[1][0] * j[1][0] + j[1][1] * j[1][1] + j[1][2] * j[1][2];
        let det = a * d - bb * bb;
        if det.abs() < 1e-12 {
            return None;
        }
        let (yt, yr) = ((d * et - bb * er) / det, (a * er - bb * et) / det);
        let step = Vec3::new(j[0][0] * yt + j[1][0] * yr, j[0][1] * yt + j[1][1] * yr, j[0][2] * yt + j[1][2] * yr);
        // Passos grandes demais saem da região em que o linear vale.
        dv += if step.norm() > 50.0 { step.unit() * 50.0 } else { step };
    }
    None
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

/// Partida, até seis dias em volta da pedida, em que o v∞ para o planeta fica
/// mais perto do plano da órbita de estacionamento (que gira com o J2), sem
/// encarecer o C3 mais de 15 %. Assim a queima no plano acerta a assíntota.
fn best_departure(v: &Vehicle, t_now: f64, to: Body, depart_in: f64, tof: f64) -> f64 {
    let step = 3.0 * 3_600.0;
    let lo = (depart_in - 6.0 * 86_400.0).max(3_600.0);
    let hi = depart_in + 6.0 * 86_400.0;
    let vinf_at = |td: f64| -> Option<Vec3> {
        let (re_, ve_) = Body::Earth.helio_state(t_now + td);
        let (rp, _) = to.helio_state(t_now + td + tof);
        lambert(re_, rp, tof, Body::Sun.mu(), true).map(|(l, _)| l - ve_)
    };
    let Some(ref_v) = vinf_at(depart_in) else { return depart_in };
    let c3_ref = ref_v.norm2();
    // Estados da órbita de estacionamento, com J2, de 3 em 3 horas.
    let (mut r, mut vel) = crate::vehicle::propagate_gravity(v.central, v.r, v.v, lo, 30.0);
    let mut td = lo;
    let mut best = (f64::INFINITY, depart_in);
    while td <= hi {
        if let Some(vi) = vinf_at(td) {
            let h = r.cross(vel).unit();
            let out = (vi.dot(h) / vi.norm()).asin().abs();
            if vi.norm2() <= c3_ref * 1.15 && out < best.0 {
                best = (out, td);
            }
        }
        let (r2, v2) = crate::vehicle::propagate_gravity(v.central, r, vel, step, 30.0);
        r = r2;
        vel = v2;
        td += step;
    }
    best.1
}

/// Simula uma queima prógrada de empuxo constante até a energia `target_e`
/// (ou até acabar o propelente). Devolve o estado final e a duração.
fn simulate_burn(body: Body, r0: Vec3, v0: Vec3, m0: f64, thrust: f64, mdot: f64, prop: f64, target_e: f64) -> Option<(Vec3, Vec3, f64)> {
    let (mut r, mut v, mut m) = (r0, v0, m0);
    let mu = body.mu();
    let h = 2.0;
    let mut t = 0.0;
    let m_end = m0 - prop;
    while m > m_end {
        let e = v.norm2() / 2.0 - mu / r.norm();
        if e >= target_e {
            return Some((r, v, t));
        }
        let acc = |r: Vec3, v: Vec3| crate::vehicle::gravity(body, r) + v.unit() * (thrust / m);
        let k1v = acc(r, v);
        let k1r = v;
        let k2v = acc(r + k1r * (h / 2.0), v + k1v * (h / 2.0));
        let k2r = v + k1v * (h / 2.0);
        let k3v = acc(r + k2r * (h / 2.0), v + k2v * (h / 2.0));
        let k3r = v + k2v * (h / 2.0);
        let k4v = acc(r + k3r * h, v + k3v * h);
        let k4r = v + k3v * h;
        r += (k1r + k2r * 2.0 + k3r * 2.0 + k4r) * (h / 6.0);
        v += (k1v + k2v * 2.0 + k3v * 2.0 + k4v) * (h / 6.0);
        m -= mdot * h;
        t += h;
    }
    None
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

/// Uma trajetória com um sobrevoo (gravidade assistida): partida, passagem
/// pelo planeta do meio e chegada. Velocidades em km/s.
#[derive(Clone, Debug, Serialize)]
pub struct Flyby {
    pub depart: f64,
    pub flyby: f64,
    pub arrive: f64,
    /// C3 de partida (km²/s²).
    pub c3: f64,
    /// v∞ chegando e saindo do planeta do sobrevoo.
    pub vinf_in: f64,
    pub vinf_out: f64,
    /// Deflexão pedida e a máxima com o periastro na altitude mínima (graus).
    pub turn_deg: f64,
    pub turn_max_deg: f64,
    /// Altitude do periastro do sobrevoo (km).
    pub rp_alt_km: f64,
    /// Queima no periastro para casar os v∞ (sobrevoo propulsado).
    pub dv_flyby: f64,
    pub vinf_arr: f64,
    /// Ganho de velocidade heliocêntrica no sobrevoo.
    pub gain: f64,
    /// √C3 + Δv do sobrevoo + v∞ de chegada.
    pub cost: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FlybySearch {
    pub from: &'static str,
    pub via: &'static str,
    pub to: &'static str,
    /// As melhores, da mais barata para a mais cara (sem repetir a mesma janela).
    pub results: Vec<Flyby>,
    /// O melhor voo direto na mesma janela: partida, viagem (dias), C3 e v∞ de chegada.
    pub direct: Option<(f64, f64, f64, f64)>,
    pub evaluated: usize,
}

/// Altitude mínima do periastro de um sobrevoo (m): acima da atmosfera, e
/// longe dos cinturões de radiação nos gigantes.
fn flyby_min_alt(b: Body) -> f64 {
    match b {
        Body::Jupiter => b.radius() * 3.0,
        Body::Saturn | Body::Uranus | Body::Neptune => b.radius() * 0.5,
        Body::Venus | Body::Earth | Body::Mars => 300e3,
        _ => 100e3,
    }
}

/// Uma trajetória com sobrevoo para as datas dadas (partida, e as duas viagens em s).
fn flyby_eval(from: Body, via: Body, to: Body, td: f64, t1: f64, t2: f64) -> Option<Flyby> {
    if t1 <= 0.0 || t2 <= 0.0 {
        return None;
    }
    let sun = Body::Sun;
    let mu_s = sun.mu();
    let mu_v = via.mu();
    let rp_min = via.radius() + flyby_min_alt(via);
    let (r0, v0) = from.state_relative_to(sun, td);
    let tf = td + t1;
    let ta = tf + t2;
    let (rf, vf) = via.state_relative_to(sun, tf);
    let (ra, va) = to.state_relative_to(sun, ta);
    let (l1, l2) = lambert(r0, rf, t1, mu_s, true)?;
    let (l3, l4) = lambert(rf, ra, t2, mu_s, true)?;
    let (vin, vout) = (l2 - vf, l3 - vf);
    let (a, b) = (vin.norm(), vout.norm());
    let vm = (a + b) / 2.0;
    let turn = vin.angle(vout);
    let turn_max = 2.0 * (1.0 / (1.0 + rp_min * vm * vm / mu_v)).asin();
    // A gravidade gira até turn_max com o periastro na altitude mínima; o que passar disso
    // é uma queima para girar o vetor (2·v∞·sen(excesso/2)).
    let (rp, dv_turn) = if turn > turn_max {
        (rp_min, 2.0 * vm * ((turn - turn_max) / 2.0).sin())
    } else {
        (mu_v / (vm * vm) * (1.0 / (turn / 2.0).sin().max(1e-9) - 1.0), 0.0)
    };
    // Queima no periastro que casa os módulos das duas hipérboles.
    let dv_mag = ((b * b + 2.0 * mu_v / rp).sqrt() - (a * a + 2.0 * mu_v / rp).sqrt()).abs();
    let dv_flyby = dv_mag + dv_turn;
    let c3 = (l1 - v0).norm2() / 1e6;
    let vinf_arr = (l4 - va).norm() / 1e3;
    let cost = c3.sqrt() + dv_flyby / 1e3 + vinf_arr;
    (cost.is_finite() && c3 < 400.0).then_some(Flyby {
        depart: td,
        flyby: tf,
        arrive: ta,
        c3,
        vinf_in: a / 1e3,
        vinf_out: b / 1e3,
        turn_deg: turn.to_degrees(),
        turn_max_deg: turn_max.to_degrees(),
        rp_alt_km: (rp - via.radius()) / 1e3,
        dv_flyby: dv_flyby / 1e3,
        vinf_arr,
        gain: (l3.norm() - l2.norm()) / 1e3,
        cost,
    })
}

/// Procura trajetórias de `from` a `to` passando por `via`: partidas em
/// `span_days` a partir de `t_start` e os dois trechos nos intervalos de
/// viagem dados (dias). Uma grade de n × n × n acha as regiões boas e cada
/// uma das melhores é refinada (busca por padrões nas três datas), porque as
/// soluções quase balísticas ficam em faixas estreitas.
pub fn flyby_search(from: Body, via: Body, to: Body, t_start: f64, span_days: f64, tof1: (f64, f64), tof2: (f64, f64), n: usize) -> FlybySearch {
    let n = n.clamp(5, 60);
    let day = 86_400.0;
    let grid = |a: f64, b: f64, k: usize| a + (b - a) * k as f64 / (n - 1) as f64;
    let mut coarse: Vec<(f64, [f64; 3])> = Vec::new();
    let mut evaluated = 0;
    for i in 0..n {
        for j in 0..n {
            for k in 0..n {
                evaluated += 1;
                let x = [grid(0.0, span_days, i), grid(tof1.0, tof1.1, j), grid(tof2.0, tof2.1, k)];
                if let Some(f) = flyby_eval(from, via, to, t_start + x[0] * day, x[1] * day, x[2] * day) {
                    coarse.push((f.cost, x));
                }
            }
        }
    }
    coarse.sort_by(|a, b| a.0.total_cmp(&b.0));
    // Sementes distintas (partidas a mais de 10 dias uma da outra).
    let mut seeds: Vec<[f64; 3]> = Vec::new();
    for (_, x) in &coarse {
        if seeds.iter().all(|s| (s[0] - x[0]).abs() > 10.0) {
            seeds.push(*x);
        }
        if seeds.len() >= 24 {
            break;
        }
    }
    let lo = [0.0, tof1.0, tof2.0];
    let hi = [span_days, tof1.1, tof2.1];
    let cost = |x: [f64; 3]| flyby_eval(from, via, to, t_start + x[0] * day, x[1] * day, x[2] * day).map_or(f64::INFINITY, |f| f.cost);
    let mut refined: Vec<Flyby> = Vec::new();
    for seed in seeds {
        let mut x = seed;
        let mut best = cost(x);
        let mut step = [span_days / (n - 1) as f64, (tof1.1 - tof1.0) / (n - 1) as f64, (tof2.1 - tof2.0) / (n - 1) as f64];
        for _ in 0..14 {
            let mut moved = true;
            while moved {
                moved = false;
                for d in 0..3 {
                    for sgn in [-1.0, 1.0] {
                        let mut y = x;
                        y[d] = (y[d] + sgn * step[d]).clamp(lo[d], hi[d]);
                        evaluated += 1;
                        let c = cost(y);
                        if c < best - 1e-6 {
                            best = c;
                            x = y;
                            moved = true;
                        }
                    }
                }
            }
            for s in &mut step {
                *s /= 2.0;
            }
        }
        if let Some(f) = flyby_eval(from, via, to, t_start + x[0] * day, x[1] * day, x[2] * day) {
            refined.push(f);
        }
    }
    refined.sort_by(|a, b| a.cost.total_cmp(&b.cost));
    // Uma por janela: descarta as que partem a menos de 15 dias de uma melhor.
    let mut results: Vec<Flyby> = Vec::new();
    for f in refined {
        if results.iter().all(|r| (r.depart - f.depart).abs() > 15.0 * day) {
            results.push(f);
        }
        if results.len() >= 8 {
            break;
        }
    }
    let direct = porkchop(from, to, t_start, span_days, tof1.0 + tof2.0, tof1.1 + tof2.1, 40).best;
    FlybySearch { from: from.id(), via: via.id(), to: to.id(), results, direct, evaluated }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sobrevoo_de_venus_a_caminho_de_marte() {
        // Terra → Vênus → Marte em 2026–2030: há trajetórias quase balísticas, em que
        // Vênus dá velocidade de graça e o periastro fica acima da altitude mínima.
        let t0 = crate::time::from_calendar(2026, 10, 1, 0, 0, 0.0);
        let s = flyby_search(Body::Earth, Body::Venus, Body::Mars, t0, 1500.0, (80.0, 250.0), (100.0, 400.0), 24);
        let best = s.results.iter().find(|f| f.dv_flyby < 0.05).expect("nenhum sobrevoo balístico");
        println!("{best:?}");
        assert!(best.gain > 1.0, "ganho {}", best.gain);
        assert!(best.rp_alt_km >= 299.0, "periastro {} km", best.rp_alt_km);
        assert!(best.turn_deg <= best.turn_max_deg + 0.01);
        assert!((best.vinf_in - best.vinf_out).abs() < 0.05);
        assert!(s.results.windows(2).all(|w| w[0].cost <= w[1].cost));
        assert!(s.direct.is_some());
    }

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
