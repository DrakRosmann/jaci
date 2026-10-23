//! Guiamento e sequenciamento automático: subida (arfagem em malha aberta e
//! malha fechada no estágio superior), foguete de sondagem, estágios, queimas
//! planejadas, reentrada guiada por preditor, paraquedas, aproximação e pouso
//! do ônibus espacial, e encontro com acoplamento.

use crate::aero;
use crate::atmosphere;
use crate::bodies::Body;
use crate::math::{G0, Quat, Vec3};
use crate::orbit::{lvlh_axes, rtn_to_inertial};
use crate::subsystems::AttMode;
use crate::vehicle::*;

/// Ponto de entrada: chamado a cada passo antes da atitude e dos motores.
pub fn run(v: &mut Vehicle, env: &Env, h: f64, out: &mut StepOutput) {
    let t = env.t;
    if v.kind == Kind::Debris {
        return;
    }
    staging(v, t, h, out);
    match v.guidance.program {
        Program::Ascent => ascent(v, t, out),
        Program::Sounding => sounding(v),
        Program::Entry => entry(v, t),
        Program::Docking => docking(v, env, out),
        Program::Rtls => rtls(v, t, out),
        Program::PoweredDescent => powered_descent(v, out),
        Program::None => {
            v.guidance.att_cmd = None;
        }
    }
    if v.guidance.program != Program::Docking {
        v.guidance.rcs_accel = Vec3::ZERO;
    }
    nodes(v, t, h, out);
    auto_entry(v, t, out);
    chutes(v, out);
    auto_pdi(v, out);
    manual_capture(v, env, out);
    post_separation(v, h, out);
    // Pilotagem manual: os programas que pedem o ADCS no guiamento não tomam a
    // nave do piloto; ela segura a atitude de agora até a próxima tecla.
    if v.guidance.manual {
        let att = v.att;
        if let Some(sys) = v.systems.as_mut().filter(|s| s.adcs.mode == AttMode::Guidance) {
            sys.adcs.mode = AttMode::Inertial(att);
        }
    }
}

// ───────────────────────── Subida ─────────────────────────

impl Vehicle {
    /// Prepara o plano alvo e o azimute no instante da decolagem.
    pub fn setup_ascent(&mut self, t: f64) {
        let Some(p) = self.guidance.ascent.clone() else { return };
        let Some(site) = self.site.clone() else { return };
        let body = self.central;
        let lat = site.lat.to_radians();
        let mut inc = p.inclination_deg.to_radians();
        if inc < lat.abs() {
            inc = lat.abs();
        }
        let r_t = body.radius() + p.target_pe_km * 1e3;
        let v_orb = (body.mu() / r_t).sqrt();
        let up = self.r.unit();
        let east = Vec3::Z.cross(up).unit();
        let north = up.cross(east);
        let plane = match p.raan_deg {
            Some(raan) => {
                let o = raan.to_radians();
                Vec3::new(inc.sin() * o.sin(), -inc.sin() * o.cos(), inc.cos())
            }
            None => {
                let sb = (inc.cos() / lat.cos()).clamp(-1.0, 1.0);
                let mut beta = sb.asin();
                if !p.north {
                    beta = std::f64::consts::PI - beta;
                }
                let d = north * beta.cos() + east * beta.sin();
                up.cross(d).unit()
            }
        };
        // Direção inercial no plano, na horizontal local, no sentido do movimento.
        let d_in = plane.cross(up).unit();
        let beta = d_in.dot(east).atan2(d_in.dot(north));
        // Corrige pela rotação da Terra: o azimute em relação ao solo.
        let v_eq = body.rotation_rate() * body.radius() * lat.cos();
        let az = (v_orb * beta.sin() - v_eq).atan2(v_orb * beta.cos());
        self.guidance.plane = Some(plane);
        self.guidance.azimuth = Some(az.to_degrees());
        let _ = t;
    }
}

/// Aceleração máxima disponível e vazão dos motores acesos.
fn engine_capacity(v: &Vehicle) -> (f64, f64) {
    let mut f = 0.0;
    let mut m = 0.0;
    for s in &v.stages {
        for e in &s.engines {
            if e.lit() {
                f += e.thrust_vac;
                m += e.mdot_max();
            }
        }
    }
    (f, m)
}

fn local_frame(r: Vec3) -> (Vec3, Vec3, Vec3) {
    let up = r.unit();
    let east = Vec3::Z.cross(up).unit();
    let north = up.cross(east);
    (up, east, north)
}

fn ascent(v: &mut Vehicle, t: f64, out: &mut StepOutput) {
    let Some(p) = v.guidance.ascent.clone() else {
        v.guidance.program = Program::None;
        return;
    };
    if v.guidance.inserted {
        v.guidance.program = Program::None;
        return;
    }
    let body = v.central;
    let d = v.derived.clone();
    let (up, east, north) = local_frame(v.r);
    let mass = v.mass();
    let (f_max, mdot) = engine_capacity(v);

    // Aceleração: limite de g e redução na pressão dinâmica máxima.
    if f_max > 0.0 {
        let mut k = (p.max_g * G0 * mass / f_max).min(1.0);
        if let Some((q_kpa, frac)) = p.throttle_bucket {
            if d.q > q_kpa * 1e3 && !v.stats.passed_max_q {
                k = k.min(frac);
            }
        }
        v.set_throttle(k);
    }

    let stage_index = v.initial_stages - v.stages.len();
    if v.guidance.kick_wait || (p.kick_stage == Some(stage_index) && v.any_engine_lit()) {
        kick(v, t, out);
        return;
    }
    if stage_index >= p.closed_loop_stage && !v.guidance.closed_loop && v.any_engine_lit() {
        v.guidance.closed_loop = true;
        out.events.push((Level::Info, "Guiamento em malha fechada".into()));
    }

    if !v.guidance.closed_loop {
        // Malha aberta: vertical até a velocidade de início da curva, depois a arfagem
        // cai com a altitude até o valor final.
        let az = v.guidance.azimuth.unwrap_or(90.0).to_radians();
        let horiz = north * az.cos() + east * az.sin();
        let pitch = if d.vspeed < p.turn_start_speed && v.guidance.turn_start_alt.is_none() {
            90.0
        } else {
            let a0 = *v.guidance.turn_start_alt.get_or_insert(d.alt);
            let x = ((d.alt - a0) / (p.turn_end_km * 1e3 - a0).max(1.0)).clamp(0.0, 1.0);
            90.0 - (90.0 - p.end_pitch_deg) * x.powf(p.turn_shape)
        };
        let pr = pitch.to_radians();
        let dir = up * pr.sin() + horiz * pr.cos();
        v.guidance.att_cmd = Some(Quat::look(dir, -up));
        return;
    }

    // Malha fechada: velocidade a ganhar no plano horizontal, e a componente
    // radial que leva a r_alvo com velocidade radial nula no fim.
    // Órbitas altas: insere numa elíptica baixa e circulariza no apoastro com
    // uma segunda ignição, como fazem os lançadores.
    let insert_pe = if p.target_pe_km > 350.0 { 200.0 } else { p.target_pe_km };
    let r_p = body.radius() + insert_pe * 1e3;
    let r_a = body.radius() + p.target_ap_km.max(p.target_pe_km) * 1e3;
    let a_t = (r_p + r_a) / 2.0;
    let mu = body.mu();
    let v_t = (mu * (2.0 / r_p - 1.0 / a_t)).sqrt();
    let rn = v.r.norm();
    let vr = v.v.dot(up);
    let vh = v.v - up * vr;
    let plane = v.guidance.plane.unwrap_or_else(|| v.r.cross(v.v).unit());
    let hd = plane.cross(up).unit();
    let dv_h = hd * v_t - vh;
    // Energia prevista no fim do corte: o motor ainda empurra enquanto desliga.
    let thrust_now: f64 = v.stages.iter().flat_map(|s| &s.engines).map(|e| e.thrust).sum();
    let tail = v.v.norm() * thrust_now / mass * 0.5;
    let energy = v.v.norm2() / 2.0 - mu / rn + tail;
    let e_t = -mu / (2.0 * a_t);
    // Elíptica: corta pelo apoastro previsto; circular, pela energia.
    let reached = if r_a > r_p * 1.01 {
        let v_after = v.v + v.att.forward() * (thrust_now / mass * 0.5);
        let el = crate::orbit::elements_from_state(v.r, v_after, mu);
        el.e >= 1.0 || el.apoapsis() >= r_a
    } else {
        energy >= e_t
    };
    if reached || (dv_h.dot(hd) < 0.0 && d.alt > 0.8 * insert_pe * 1e3) {
        // Motor sólido não desliga: segue prógrado até acabar.
        if v.stages.iter().flat_map(|s| &s.engines).any(|e| e.lit() && e.solid) {
            v.guidance.att_cmd = Some(Quat::look(v.v.unit(), -up));
            return;
        }
        if v.any_engine_lit() {
            v.shutdown();
            v.guidance.inserted = true;
            v.guidance.program = Program::None;
            let el = d.elements;
            out.events.push((
                Level::Good,
                format!(
                    "Corte do motor: inserção em {:.0} × {:.0} km, {:.2}°",
                    (el.periapsis() - body.radius()) / 1e3,
                    (el.apoapsis() - body.radius()) / 1e3,
                    el.i.to_degrees()
                ),
            ));
            if insert_pe < p.target_pe_km - 1.0 {
                // Circularização no apoastro com o mesmo estágio.
                let mut probe = v.clone();
                probe.guidance.nodes.clear();
                for e in probe.stages.iter_mut().flat_map(|s| s.engines.iter_mut()) {
                    e.state = EngineState::Off;
                    e.thrust = 0.0;
                }
                if let Some(plan) = crate::planner::change_apsis(&probe, false, p.target_pe_km) {
                    for n in plan.nodes {
                        let id = 10_000 + v.guidance.nodes.len() as u32;
                        v.guidance.nodes.push(Node { id, t: t + n.t, label: "Circularização (2ª ignição)".into(), ..n });
                    }
                    out.events.push((Level::Info, "Fase balística até o apoastro para a 2ª ignição".into()));
                }
                v.guidance.payload_after_burns = p.payload_sep;
            } else if p.payload_sep {
                v.guidance.payload_timer = Some(30.0);
            }
        }
        return;
    }
    if f_max <= 0.0 || mdot <= 0.0 {
        // Entre estágios: mantém a última atitude.
        return;
    }
    let throttle = v.stages.iter().flat_map(|s| &s.engines).filter(|e| e.lit() && !e.solid).map(|e| e.throttle).next().unwrap_or(1.0);
    let a_t_now = f_max * throttle / mass;
    let ve = f_max / mdot;
    // PEG: componente radial do empuxo; sem solução (estágio que não chega
    // sozinho, por exemplo), a lei antiga.
    let peg = if v.guidance.legacy_ascent { None } else { peg_step(v, t, r_p, v_t, ve, a_t_now) };
    let s = match peg {
        Some(fr) => fr,
        None => {
            v.guidance.peg = None;
            let need = dv_h.norm().max(1.0);
            let t_go = ((mass / (mdot * throttle)) * (1.0 - (-need / ve).exp())).max(2.0);
            v.guidance.t_go = t_go;
            if t_go < 4.0 && v.guidance.att_cmd.is_some() {
                // Fase terminal: congela a atitude.
                return;
            }
            let a0 = 6.0 * (r_p - rn) / (t_go * t_go) - 4.0 * vr / t_go;
            let g = mu / (rn * rn);
            let a_r = a0 + g - vh.norm2() / rn;
            a_r / a_t_now
        }
    };
    let s = s.clamp(-0.6, 0.9);
    let c = (1.0 - s * s).sqrt();
    let dir = up * s + dv_h.unit() * c;
    v.guidance.att_cmd = Some(Quat::look(dir, -up));
}

/// Intervalo entre os ciclos maiores do PEG (s de simulação).
const PEG_CYCLE: f64 = 1.0;
/// Abaixo disto o PEG não recalcula: segue A + B·t até o corte.
const PEG_TERMINAL: f64 = 10.0;

/// PEG 2D (Powered Explicit Guidance, o do ônibus espacial na forma
/// clássica): a componente radial do empuxo é linear no tempo mais a
/// compensação da gravidade menos a centrífuga, fr = A + B·t + C, com A, B e o
/// tempo até o corte T resolvidos para chegar a `r_t` com velocidade radial
/// nula e horizontal `vh_t`. Devolve fr agora, ou None se não há solução.
fn peg_step(v: &mut Vehicle, t: f64, r_t: f64, vh_t: f64, ve: f64, acc: f64) -> Option<f64> {
    let mu = v.central.mu();
    let up = v.r.unit();
    let r = v.r.norm();
    let vr = v.v.dot(up);
    let vh = (v.v - up * vr).norm();
    let omega = vh / r;
    let tau = ve / acc;
    let c_now = (mu / (r * r) - omega * omega * r) / acc;
    let mut st = v.guidance.peg.unwrap_or(Peg { a: 0.0, b: 0.0, t: 0.0, at: t, converged: false });
    let dt = t - st.at;
    if v.guidance.peg.is_some() && (dt < PEG_CYCLE || (st.converged && st.t - dt < PEG_TERMINAL)) {
        // Entre ciclos e na fase final: segue a lei calculada.
        v.guidance.t_go = (st.t - dt).max(0.0);
        return Some(st.a + st.b * dt + c_now);
    }
    if v.guidance.peg.is_none() {
        // Primeira estimativa de T: a equação do foguete com o Δv horizontal.
        st.t = tau * (1.0 - (-((vh_t - vh).abs().max(1.0)) / ve).exp());
    } else {
        st.t -= dt;
        st.a += st.b * dt;
    }
    let mut t_go = st.t;
    let (mut a, mut b) = (st.a, st.b);
    let mut converged = false;
    for _ in 0..4 {
        if !(t_go > 0.5 && t_go < 0.98 * tau) {
            return None;
        }
        let b0 = -ve * (1.0 - t_go / tau).ln();
        let b1 = b0 * tau - ve * t_go;
        let c0 = b0 * t_go - b1;
        let c1 = c0 * tau - ve * t_go * t_go / 2.0;
        let det = b0 * c1 - b1 * c0;
        if det.abs() < 1e-9 {
            return None;
        }
        let (rhs1, rhs2) = (-vr, r_t - r - vr * t_go);
        a = (c1 * rhs1 - b1 * rhs2) / det;
        b = (b0 * rhs2 - c0 * rhs1) / det;
        // Tempo até o corte pelo momento angular que falta.
        let fr = a + c_now;
        let a_end = acc / (1.0 - t_go / tau);
        let omega_t = vh_t / r_t;
        let fr_t = a + b * t_go + (mu / (r_t * r_t) - omega_t * omega_t * r_t) / a_end;
        let fr_dot = (fr_t - fr) / t_go;
        let f_th = 1.0 - fr * fr / 2.0;
        let f_th_dot = -fr * fr_dot;
        let f_th_ddot = -fr_dot * fr_dot / 2.0;
        let dh = r_t * vh_t - r * vh;
        let r_avg = (r + r_t) / 2.0;
        let dv = (dh / r_avg + ve * t_go * (f_th_dot + f_th_ddot * tau) + f_th_ddot * ve * t_go * t_go / 2.0) / (f_th + f_th_dot * tau + f_th_ddot * tau * tau);
        if !dv.is_finite() || dv <= 0.0 {
            return None;
        }
        let t_new = tau * (1.0 - (-dv / ve).exp());
        converged = (t_new - t_go).abs() < 0.5;
        t_go = t_new;
        if converged {
            break;
        }
    }
    if !(a.is_finite() && b.is_finite()) {
        return None;
    }
    // O PEG daqui é de um estágio: se este acaba antes do corte, quem guia até
    // a separação é a lei antiga, e o PEG assume no estágio seguinte.
    let (f_now, mdot_now) = burn_capacity(v);
    let prop: f64 = v.active_stages().iter().map(|&i| v.stages[i].propellant).sum();
    if f_now > 0.0 && mdot_now > 0.0 && prop / mdot_now < t_go - 1.0 {
        return None;
    }
    v.guidance.peg = Some(Peg { a, b, t: t_go, at: t, converged });
    v.guidance.t_go = t_go;
    Some(a + c_now)
}

/// Motor de apogeu: horizontal, no plano, aceso quando faltar metade da
/// queima para o apoastro.
fn kick(v: &mut Vehicle, t: f64, out: &mut StepOutput) {
    let up = v.r.unit();
    let plane = v.guidance.plane.unwrap_or_else(|| v.r.cross(v.v).unit());
    let hd = plane.cross(up).unit();
    v.guidance.att_cmd = Some(Quat::look(hd, -up));
    if !v.guidance.kick_wait {
        return;
    }
    let mu = v.central.mu();
    let el = crate::orbit::elements_from_state(v.r, v.v, mu);
    let to_apo = if v.derived.vspeed > 0.0 { crate::orbit::time_to_true_anomaly(&el, mu, std::f64::consts::PI) } else { 0.0 };
    let (f, mdot) = burn_capacity(v);
    let prop = v.active_stages().first().map_or(0.0, |&i| v.stages[i].propellant);
    let burn = if mdot > 0.0 { prop / mdot } else { 0.0 };
    v.guidance.t_go = to_apo;
    if to_apo <= burn / 2.0 && f > 0.0 && v.att.forward().angle(hd) < 0.1 {
        v.guidance.kick_wait = false;
        if v.ignite(out) {
            out.events.push((Level::Info, format!("Ignição do motor de apogeu a {:.0} km", v.derived.alt / 1e3)));
        }
    }
    let _ = t;
}

fn sounding(v: &mut Vehicle) {
    let Some(p) = v.guidance.sounding.clone() else { return };
    let d = &v.derived;
    let (up, east, north) = local_frame(v.r);
    // No trilho (primeiros 30 m): elevação e azimute do lançador.
    let climbed = v.stats.max_alt - v.site.as_ref().map_or(0.0, |s| s.alt);
    if climbed < 30.0 {
        let el = p.elevation_deg.to_radians();
        let az = p.azimuth_deg.to_radians();
        let dir = up * el.sin() + (north * az.cos() + east * az.sin()) * el.cos();
        v.guidance.att_cmd = Some(Quat::look(dir, -up));
    } else if d.q > 20.0 {
        // Estável aerodinamicamente: aponta para o vento relativo.
        let vrel = v.v - v.central.surface_velocity(v.r);
        v.guidance.att_cmd = Some(Quat::look(vrel.unit(), -up));
    }
    // No vácuo, girando, mantém a última atitude.
}

// ───────────────────────── Estágios ─────────────────────────

fn staging(v: &mut Vehicle, t: f64, h: f64, out: &mut StepOutput) {
    if !v.guidance.auto_stage || v.launch_time.is_none() {
        return;
    }
    let in_ascent = matches!(v.guidance.program, Program::Ascent | Program::Sounding) && !v.guidance.inserted;
    // Boosters gastos.
    if let Some(s0) = v.stages.first() {
        if s0.boosters && s0.propellant <= 0.0 && v.stages.len() > 1 {
            let name = s0.name.clone();
            if let Some(mut d) = v.jettison_bottom(t) {
                d.v += (d.r.unit() * 3.0).reject(v.v.unit());
                out.jettisoned.push(d);
            }
            out.events.push((Level::Info, format!("Separação: {}", name)));
        }
    }
    // Estágio de baixo esgotado: separa e acende o próximo.
    if let Some(s0) = v.stages.first() {
        let spent = !s0.boosters && v.stages.len() > 1 && !s0.any_lit()
            && (s0.propellant <= 0.0 || s0.engines.iter().all(|e| e.state == EngineState::Failed || e.ignitions == 0));
        let launcher = !s0.spacecraft;
        if spent && launcher && in_ascent && v.guidance.stage_timer.is_none() {
            v.guidance.stage_timer = Some(0.0);
        }
    }
    let next_is_spacecraft = v.stages.get(1).is_some_and(|s| s.spacecraft);
    if let Some(timer) = v.guidance.stage_timer.as_mut() {
        *timer += h;
        let elapsed = *timer;
        if elapsed >= 2.0 && !v.guidance.pending_ignition {
            let n = v.stage_number();
            let name = v.stages[0].name.clone();
            if let Some(d) = v.jettison_bottom(t) {
                out.jettisoned.push(d);
                out.events.push((Level::Info, format!("Separação do estágio {} ({})", n, name)));
            }
            let sounding = v.guidance.program == Program::Sounding;
            v.guidance.pending_ignition = in_ascent && (!next_is_spacecraft || sounding);
            // O motor de apogeu espera o apoastro.
            let idx = v.initial_stages - v.stages.len();
            if v.guidance.ascent.as_ref().is_some_and(|a| a.kick_stage == Some(idx)) && in_ascent {
                v.guidance.pending_ignition = false;
                v.guidance.kick_wait = true;
                out.events.push((Level::Info, "Voo balístico até o apogeu".into()));
            }
            if !v.guidance.pending_ignition {
                v.guidance.stage_timer = None;
            }
            if in_ascent && next_is_spacecraft && !sounding {
                // Acabou o lançador antes do corte: a órbita é a que deu.
                v.guidance.inserted = true;
                v.guidance.program = Program::None;
                let el = crate::orbit::elements_from_state(v.r, v.v, v.central.mu());
                let re = v.central.radius();
                let (pe, ap) = ((el.periapsis() - re) / 1e3, (el.apoapsis() - re) / 1e3);
                if pe > 120.0 {
                    out.events.push((Level::Good, format!("Fim da propulsão do lançador: órbita {:.0} × {:.0} km, {:.2}°", pe, ap, el.i.to_degrees())));
                    v.guidance.deploy_timer = Some(0.0);
                } else {
                    out.events.push((Level::Alarm, format!("Órbita não alcançada: periastro {:.0} km", pe)));
                }
            }
        }
        let delay = v.guidance.sounding.as_ref().map_or(5.0, |s| s.stage2_delay.max(2.0));
        if v.guidance.pending_ignition && elapsed >= 2.0 + delay {
            v.guidance.pending_ignition = false;
            v.guidance.stage_timer = None;
            // A carga do foguete de sondagem não tem motor: aí acabou a propulsão.
            if v.stages.first().is_some_and(|s| !s.engines.is_empty()) {
                if v.ignite(out) {
                    out.events.push((Level::Info, format!("Ignição do estágio {}", v.stage_number())));
                }
            }
        }
    }
    // Coifa: fora da atmosfera densa.
    if in_ascent && v.derived.alt > 105e3 && v.derived.q < 100.0 {
        if v.drop_item("fairing").is_some() {
            out.events.push((Level::Info, "Separação da coifa".into()));
        }
    }
    // Torre de escape: depois de 120 km ou na ignição do estágio superior.
    let (alt, stage_no) = (v.derived.alt, v.stage_number());
    if let Some(les) = &mut v.les {
        if les.available && les.tower_mass > 0.0 && in_ascent && (alt > 90e3 || stage_no >= 2) && les.firing <= 0.0 {
            les.available = false;
            out.events.push((Level::Info, "Torre de escape descartada".into()));
        }
    }
    // Carga útil: depois da inserção (e das queimas do estágio superior).
    if v.guidance.payload_after_burns && v.guidance.nodes.iter().all(|n| n.done) && !v.any_engine_lit() {
        v.guidance.payload_after_burns = false;
        v.guidance.payload_timer = Some(30.0);
    }
    if let Some(timer) = v.guidance.payload_timer.as_mut() {
        *timer -= h;
        if *timer <= 0.0 {
            v.guidance.payload_timer = None;
            if let Some(d) = v.separate_payload(t) {
                out.jettisoned.push(d);
                out.events.push((Level::Good, format!("Separação da carga útil: {}", v.name)));
                v.guidance.deploy_timer = Some(0.0);
            }
        }
    }
    // Foguete de sondagem: separa a carga depois do último motor.
    if v.guidance.program == Program::Sounding && v.stages.len() > 1 {
        let s0 = &v.stages[0];
        if s0.propellant <= 0.0 && !s0.any_lit() && v.stages[1].engines.is_empty() && v.guidance.stage_timer.is_none() {
            let name = s0.name.clone();
            if let Some(d) = v.jettison_bottom(t) {
                out.jettisoned.push(d);
                out.events.push((Level::Info, format!("Separação da carga útil ({} largado)", name)));
            }
        }
    }
}

/// Ativação de satélite/sonda depois de separar: amortece, abre os painéis e
/// aponta ao Sol.
fn post_separation(v: &mut Vehicle, h: f64, out: &mut StepOutput) {
    let Some(timer) = v.guidance.deploy_timer.as_mut() else { return };
    let before = *timer;
    *timer += h;
    let now = *timer;
    let Some(sys) = &mut v.systems else {
        v.guidance.deploy_timer = None;
        return;
    };
    if before == 0.0 {
        sys.adcs.mode = AttMode::Detumble;
    }
    if before < 60.0 && now >= 60.0 {
        if let Some(sa) = &mut sys.power.solar {
            if !sa.deployed {
                sa.deployed = true;
                out.events.push((Level::Good, "Painéis solares abertos".into()));
            }
        }
        sys.comms.hga_deployed = true;
        sys.adcs.mode = if v.kind == Kind::Probe { AttMode::EarthPoint } else { AttMode::SunPoint };
        v.guidance.deploy_timer = None;
    }
}

// ───────────────────────── Queimas planejadas ─────────────────────────

/// Empuxo e vazão dos motores que fariam a queima (estágios ativos).
pub fn burn_capacity(v: &Vehicle) -> (f64, f64) {
    let mut f = 0.0;
    let mut m = 0.0;
    for i in v.active_stages() {
        let s = &v.stages[i];
        if s.propellant <= 0.0 {
            continue;
        }
        for e in &s.engines {
            if e.state != EngineState::Failed && (e.ignitions > 0 || e.lit()) {
                f += e.thrust_vac;
                m += e.mdot_max();
            }
        }
    }
    (f, m)
}

/// Duração estimada de uma queima de `dv` (s).
pub fn burn_duration(v: &Vehicle, dv: f64, rcs: bool) -> f64 {
    if rcs {
        let f = v.systems.as_ref().map_or(0.0, |s| s.adcs.rcs_force);
        return if f > 0.0 { dv * v.mass() / f } else { f64::INFINITY };
    }
    let (f, mdot) = burn_capacity(v);
    if f <= 0.0 {
        return f64::INFINITY;
    }
    let ve = f / mdot;
    let m0 = v.mass();
    m0 * (1.0 - (-dv / ve).exp()) / mdot
}

fn ignition_time(v: &Vehicle, n: &Node) -> f64 {
    if let Some(t) = n.ignite_at {
        return t;
    }
    let dur = burn_duration(v, n.dv_rtn.norm(), n.rcs);
    n.t - if dur.is_finite() { dur / 2.0 } else { 0.0 }
}

/// Antecedência da orientação para a queima (s).
const ORIENT_LEAD: f64 = 150.0;

/// Tempo até a próxima ignição (com a orientação antes).
pub fn time_to_next_node(v: &Vehicle, t: f64) -> f64 {
    v.guidance
        .nodes
        .iter()
        .filter(|n| !n.done && !n.executing)
        .map(|n| ignition_time(v, n) - ORIENT_LEAD - t)
        .fold(f64::INFINITY, f64::min)
        .max(0.0)
        .min(if v.guidance.nodes.iter().any(|n| n.executing) { 0.0 } else { f64::INFINITY })
}

fn nodes(v: &mut Vehicle, t: f64, h: f64, out: &mut StepOutput) {
    if v.guidance.nodes.is_empty() || v.phase.is_final() {
        return;
    }
    let mass = v.mass();
    let (f_max, _) = burn_capacity(v);
    let mut idx = None;
    for (i, n) in v.guidance.nodes.iter().enumerate() {
        if !n.done {
            idx = Some(i);
            break;
        }
    }
    let Some(i) = idx else { return };
    let t_ign = ignition_time(v, &v.guidance.nodes[i]);
    let n = v.guidance.nodes[i].clone();
    if !n.executing {
        // O RCS de translação empurra em qualquer eixo: o pulso não gira a nave.
        if !n.rcs && t >= t_ign - ORIENT_LEAD {
            // Orientação para a queima.
            let dir = rtn_to_inertial(v.r, v.v, n.dv_rtn).unit();
            v.guidance.att_cmd = Some(Quat::look(dir, -v.r.unit()));
            // Na pilotagem manual quem aponta é o piloto (o diretor mostra para onde).
            if let Some(sys) = v.systems.as_mut().filter(|_| !v.guidance.manual) {
                if sys.adcs.mode != AttMode::Guidance {
                    sys.adcs.mode = AttMode::Guidance;
                }
            }
        }
        let dir_now = rtn_to_inertial(v.r, v.v, n.dv_rtn).unit();
        let aligned = n.rcs || v.att.forward().angle(dir_now) < 8f64.to_radians();
        if t >= t_ign && !aligned && t < t_ign + 90.0 {
            // Ainda girando: espera alinhar (até 90 s de atraso).
            return;
        }
        if t >= t_ign {
            let dir = rtn_to_inertial(v.r, v.v, n.dv_rtn).unit();
            let node = &mut v.guidance.nodes[i];
            node.dir = Some(dir);
            node.remaining = n.dv_rtn.norm();
            node.executing = true;
            let ok = if n.rcs { true } else { v.ignite(out) };
            if ok {
                out.events.push((Level::Info, format!("Ignição: {} (Δv {:.1} m/s)", n.label, n.dv_rtn.norm())));
            } else {
                let node = &mut v.guidance.nodes[i];
                node.done = true;
                node.executing = false;
                out.events.push((Level::Alarm, format!("Queima cancelada: {}", n.label)));
            }
        }
        return;
    }
    // Queimas longas seguem a direção no referencial orbital (RTN), que gira
    // com a órbita: uma retroqueima continua retrógrada até o fim.
    let dir = rtn_to_inertial(v.r, v.v, n.dv_rtn).unit();
    if !n.rcs {
        v.guidance.att_cmd = Some(Quat::look(dir, -v.r.unit()));
    }
    let accel = if n.rcs {
        let f = v.systems.as_ref().map_or(0.0, |s| s.adcs.rcs_force);
        v.guidance.rcs_accel = dir * (f / mass);
        f / mass
    } else {
        let thrust: f64 = v.stages.iter().flat_map(|s| &s.engines).map(|e| e.thrust).sum();
        // Quanto da aceleração está na direção certa (a atitude ainda pode estar girando).
        let along = v.att.forward().dot(dir).max(0.0);
        // Fim da queima: reduz a aceleração para cortar com precisão.
        if f_max > 0.0 {
            let k = (n.remaining * mass / (f_max * 2.0)).clamp(0.05, 1.0);
            v.set_throttle(k);
        }
        thrust / mass * along
    };
    let stalled = !n.rcs && !v.any_engine_lit() && t > t_ign + 10.0;
    // Queima de escape: corta pela energia (contando o empuxo que ainda vem no desligamento).
    let energy_reached = n.target_energy.is_some_and(|e| {
        let vn = v.v.norm();
        vn * vn / 2.0 - v.central.mu() / v.r.norm() + vn * accel * 0.5 >= e
    });
    let node = &mut v.guidance.nodes[i];
    node.remaining -= accel * h;
    if n.target_energy.is_some() {
        node.remaining = node.remaining.max(1.0);
    }
    // O motor ainda empurra ~0,5 s enquanto desliga.
    let tail = if n.rcs { 0.0 } else { accel * 0.5 };
    let dv_done = node.remaining <= tail && n.target_energy.is_none();
    if dv_done || stalled || energy_reached {
        let residual = node.remaining.max(0.0);
        node.done = true;
        node.executing = false;
        if !n.rcs {
            v.shutdown();
        }
        v.guidance.rcs_accel = Vec3::ZERO;
        v.guidance.att_cmd = None;
        if let Some(sys) = &mut v.systems {
            if sys.adcs.mode == AttMode::Guidance {
                sys.adcs.mode = AttMode::Hold;
            }
        }
        out.events.push((
            if stalled { Level::Warn } else { Level::Good },
            if stalled {
                format!("Queima interrompida: {} (faltaram {:.1} m/s)", n.label, residual)
            } else {
                format!("Corte: {} concluída", n.label)
            },
        ));
    }
}

// ───────────────────────── Reentrada ─────────────────────────

/// Liga o programa de reentrada sozinho quando uma cápsula ou o ônibus espacial
/// começa a descer na atmosfera.
fn auto_entry(v: &mut Vehicle, t: f64, out: &mut StepOutput) {
    if !matches!(v.kind, Kind::Capsule | Kind::Shuttle | Kind::SoundingRocket) || !v.guidance.auto_chutes {
        return;
    }
    let d = &v.derived;
    if v.central != Body::Earth && !v.central.has_atmosphere() {
        return;
    }
    let top = if v.central == Body::Earth { 140e3 } else { v.central.atmosphere_top() };
    if d.alt < top && d.vspeed < 0.0 && v.guidance.program == Program::None && v.launch_time.is_some()
        && !v.any_engine_lit() && v.kind != Kind::SoundingRocket
        && !v.guidance.nodes.iter().any(|n| n.executing)
    {
        // A cápsula larga o módulo de serviço antes.
        if v.stages.len() > 1 && v.guidance.auto_stage {
            let name = v.stages[0].name.clone();
            if let Some(dbr) = v.jettison_bottom(t) {
                out.jettisoned.push(dbr);
                out.events.push((Level::Info, format!("Separação: {}", name)));
            }
        }
        if v.stages.len() == 1 {
            v.guidance.program = Program::Entry;
            if let Some(sys) = &mut v.systems {
                sys.adcs.mode = AttMode::Guidance;
            }
            out.events.push((Level::Info, "Programa de reentrada ativo".into()));
        }
    }
}

/// Coeficientes de reentrada da nave (sem estágios).
fn entry_aero(v: &Vehicle, mach: f64) -> (f64, f64) {
    let cda = aero::drag_coefficient(v.shape, mach) * v.ref_area();
    let ld = if v.guidance.ballistic { 0.0 } else { aero::lift_to_drag(v.shape, mach) };
    (cda, ld)
}

/// Prevê onde a nave chega ao solo com rolamento `bank` (graus): integra em
/// passos de 1 a 4 s, sem vento, até 3 km de altitude. Só a componente
/// vertical da sustentação conta: o guiamento alterna o lado do rolamento e
/// a componente lateral se cancela.
pub fn predict_landing(v: &Vehicle, t: f64, bank_deg: f64) -> Option<(f64, f64, f64)> {
    let body = v.central;
    let mass = v.mass();
    let (mut r, mut vel) = (v.r, v.v);
    let bank = bank_deg.to_radians();
    let mut tt = 0.0;
    for _ in 0..20_000 {
        let alt = body.altitude(r);
        if alt < 3_000.0 {
            let (lat, lon, _) = body.to_geodetic(r, t + tt);
            return Some((lat, lon, tt));
        }
        let air = atmosphere::air(body, alt);
        let vrel = vel - body.surface_velocity(r);
        let vn = vrel.norm();
        let mach = if air.sound > 0.0 { vn / air.sound } else { 30.0 };
        let (cda, ld) = entry_aero(v, mach);
        let accel = |r: Vec3, vel: Vec3| -> Vec3 {
            let mut a = gravity(body, r);
            let alt = body.altitude(r);
            let air = atmosphere::air(body, alt);
            if air.density > 0.0 {
                let vrel = vel - body.surface_velocity(r);
                let vn = vrel.norm();
                if vn > 1.0 {
                    let drag = 0.5 * air.density * vn * vn * cda / mass;
                    let vh = vrel / vn;
                    a -= vh * drag;
                    if ld > 0.0 {
                        let up = r.unit().reject(vh).unit();
                        a += up * (bank.cos() * drag * ld);
                    }
                }
            }
            a
        };
        let dt = if air.density > 1e-6 { 1.0 } else if alt > 200e3 { 20.0 } else { 4.0 };
        // Ponto médio.
        let a1 = accel(r, vel);
        let rm = r + vel * (dt / 2.0);
        let vm = vel + a1 * (dt / 2.0);
        let a2 = accel(rm, vm);
        r += vm * dt;
        vel += a2 * dt;
        tt += dt;
        if r.norm() > body.radius() * 3.0 {
            return None;
        }
        let _ = vn;
    }
    None
}

/// Reentrada: atitude de escudo à frente (cápsula) ou nariz a 40° (ônibus), e
/// rolamento guiado para o alvo pelo preditor.
fn entry(v: &mut Vehicle, t: f64) {
    let d = v.derived.clone();
    let up = v.r.unit();
    let vrel = v.v - v.central.surface_velocity(v.r);
    let vhat = vrel.unit();
    match v.kind {
        Kind::Shuttle => {
            let aoa: f64 = if d.mach > 4.0 { 40.0 } else if d.mach > 1.0 { 15.0 + 25.0 * (d.mach - 1.0) / 3.0 } else { 8.0 };
            let side = vhat.cross(up).unit();
            let fwd = (vhat * aoa.to_radians().cos() + side.cross(vhat).unit() * aoa.to_radians().sin()).unit();
            v.guidance.att_cmd = Some(Quat::look(fwd, -up));
        }
        _ => {
            // Escudo térmico (−X) contra o vento.
            v.guidance.att_cmd = Some(Quat::look(-vhat, -up));
        }
    }
    if v.kind == Kind::Shuttle && d.mach < 2.5 && d.alt < 30e3 {
        approach_and_landing(v);
        return;
    }
    v.guidance.ld_cmd = None;
    let Some((lat_t, lon_t)) = v.guidance.landing_target else {
        v.guidance.bank_deg = if v.kind == Kind::Shuttle { 45.0 } else { 0.0 };
        return;
    };
    if !v.guidance.entry_guided || d.air_speed < 600.0 {
        return;
    }
    if t < v.guidance.next_entry_update {
        return;
    }
    v.guidance.next_entry_update = t + 4.0;
    // Distância a percorrer até o alvo e o rumo para ele.
    let dist_km = great_circle_km(d.lat, d.lon, lat_t, lon_t);
    let bearing = bearing_deg(d.lat, d.lon, lat_t, lon_t);
    // Busca o |rolamento| que faz o alcance previsto bater, medindo tudo ao
    // longo do rumo atual (o desvio lateral é com o sinal do rolamento).
    let along = |la: f64, lo: f64| {
        great_circle_km(d.lat, d.lon, la, lo) * ((bearing_deg(d.lat, d.lon, la, lo) - d.heading_deg).to_radians()).cos()
    };
    let to_go = along(lat_t, lon_t);
    let range_at = |bank: f64| -> Option<f64> { predict_landing(v, t, bank).map(|(la, lo, _)| along(la, lo)) };
    let (mut lo, mut hi) = (0.0_f64, 150.0_f64);
    let mut bank = v.guidance.bank_deg.abs();
    match (range_at(lo), range_at(hi)) {
        (Some(r_lo), _) if r_lo < to_go => bank = 0.0,
        (_, Some(r_hi)) if r_hi > to_go => bank = hi,
        (Some(_), Some(_)) => {
            for _ in 0..7 {
                let mid = 0.5 * (lo + hi);
                match range_at(mid) {
                    Some(r) if r > to_go => lo = mid,
                    Some(_) => hi = mid,
                    None => break,
                }
            }
            bank = 0.5 * (lo + hi);
        }
        _ => {}
    }
    // Sinal: vira para o lado do alvo, com histerese para não ficar trocando.
    let heading_err = ((bearing - d.heading_deg + 540.0) % 360.0) - 180.0;
    // Rolamento positivo põe a sustentação à direita (rumo crescente).
    let sign = v.guidance.bank_deg.signum();
    let deadband = (2.0 + 0.002 * dist_km).min(8.0);
    let sign = if sign == 0.0 || heading_err.abs() > deadband { heading_err.signum() } else { sign };
    v.guidance.bank_deg = bank * if sign == 0.0 { 1.0 } else { sign };
}

/// Aproximação e pouso do ônibus espacial: planeio com L/D ajustado pela
/// razão distância/altura, curva para a pista, trem e arredondamento.
fn approach_and_landing(v: &mut Vehicle) {
    let d = v.derived.clone();
    let (lat_t, lon_t) = v.guidance.landing_target.unwrap_or((d.lat, d.lon));
    let dist = great_circle_km(d.lat, d.lon, lat_t, lon_t) * 1e3;
    let bearing = bearing_deg(d.lat, d.lon, lat_t, lon_t);
    let heading_err = ((bearing - d.heading_deg + 540.0) % 360.0) - 180.0;
    let mut bank = (heading_err * 1.5).clamp(-50.0, 50.0);
    // Sobre a pista: nivela as asas.
    if dist < 1_500.0 {
        bank = 0.0;
    }
    v.guidance.bank_deg = bank;
    let max_ld = aero::lift_to_drag(v.shape, d.mach);
    let mut ld = (dist / d.alt.max(1.0)).clamp(0.6, max_ld);
    // Freios aerodinâmicos para chegar a 300 m com ~150 m/s.
    if d.alt < 600.0 {
        if !v.gear_down {
            v.gear_down = true;
        }
    }
    if d.alt < 120.0 {
        // Arredondamento: mais sustentação que peso por alguns segundos.
        ld = (max_ld * 2.2).min(9.0);
    }
    v.guidance.ld_cmd = Some(ld);
}

// ───────────────────────── Paraquedas ─────────────────────────

fn chutes(v: &mut Vehicle, out: &mut StepOutput) {
    if !v.guidance.auto_chutes || v.chutes.is_empty() || v.launch_time.is_none() {
        return;
    }
    let d = v.derived.clone();
    if d.vspeed >= 0.0 || v.any_engine_lit() {
        return;
    }
    // Cápsula ainda presa a estágios não abre paraquedas (salvo foguete de sondagem).
    if v.stages.len() > 1 {
        return;
    }
    let mut main_opened = false;
    for c in &mut v.chutes {
        if c.state == ChuteState::Stowed && d.alt < c.deploy_alt && d.mach < c.max_mach && d.q < 25_000.0 {
            c.state = ChuteState::Deployed;
            c.inflation = 0.0;
            out.events.push((Level::Good, format!("{} abertos a {:.1} km", c.name, d.alt / 1e3)));
            if !c.drogue {
                main_opened = true;
            }
        }
    }
    if main_opened {
        for c in &mut v.chutes {
            if c.drogue && c.state == ChuteState::Deployed {
                c.state = ChuteState::Cut;
                out.events.push((Level::Info, format!("{} cortados", c.name)));
            }
        }
    }
    if let Some(hs) = &v.heat_shield {
        let _ = hs;
    }
}

// ───────────────────────── Pouso propulsado ─────────────────────────

/// Liga a descida propulsada: acende o motor e o guiamento assume a atitude.
pub fn start_powered_descent(v: &mut Vehicle, out: &mut StepOutput) -> Result<(), String> {
    if v.landing != LandingMode::Legs {
        return Err("este veículo não pousa com motor".into());
    }
    if burn_capacity(v).0 <= 0.0 {
        return Err("sem motor ou propelente para a descida".into());
    }
    v.guidance.program = Program::PoweredDescent;
    v.guidance.nodes.retain(|n| n.done);
    if let Some(sys) = &mut v.systems {
        sys.adcs.mode = AttMode::Guidance;
    }
    if !v.any_engine_lit() {
        v.ignite(out);
    }
    let ground_speed = (v.v - v.central.surface_velocity(v.r)).norm();
    out.events.push((Level::Info, format!("Descida propulsada a {:.1} km, {:.0} m/s", v.derived.alt / 1e3, ground_speed)));
    Ok(())
}

/// Marte: sob o paraquedas, abaixo da altitude marcada, corta o paraquedas e desce no motor.
fn auto_pdi(v: &mut Vehicle, out: &mut StepOutput) {
    let Some(alt) = v.guidance.auto_pdi_alt else { return };
    let d = &v.derived;
    if v.guidance.program == Program::PoweredDescent || v.phase.is_final() || d.vspeed >= 0.0 {
        return;
    }
    if !v.chutes.iter().any(|c| c.state == ChuteState::Deployed) {
        return;
    }
    if d.alt > alt {
        // Pendurada no paraquedas, já com o motor para baixo.
        let up = v.r.unit();
        let down = v.att.rotate(Vec3::Z);
        let down = if down.cross(up).norm() < 0.3 { Vec3::Z.cross(up).unit() } else { down };
        v.guidance.att_cmd = Some(Quat::look(up, down));
        if let Some(sys) = &mut v.systems {
            sys.adcs.mode = AttMode::Guidance;
        }
        return;
    }
    for c in &mut v.chutes {
        if c.state == ChuteState::Deployed {
            c.state = ChuteState::Cut;
        }
    }
    v.heat_shield = None;
    out.events.push((Level::Info, "Paraquedas e escudo largados".into()));
    if let Err(e) = start_powered_descent(v, out) {
        out.events.push((Level::Alarm, format!("Descida propulsada não começou: {e}")));
    }
}

/// Descida propulsada até pousar nas pernas, como o P63/P64/P66 do Apollo
/// numa lei só. A velocidade de descida alvo é a menor entre a que dá para
/// frear com folga até 15 m (√(2·a·h), mais 1,5 m/s para o toque) e a que
/// chega ao chão junto com o fim da frenagem horizontal; o empuxo atende
/// primeiro a vertical (gravidade menos centrífuga, mais a correção da
/// descida) e o que sobra mata a velocidade horizontal. Lá em cima isso é a frenagem de costas para o movimento; perto
/// do chão, a descida vertical devagar.
fn powered_descent(v: &mut Vehicle, out: &mut StepOutput) {
    let body = v.central;
    let r = v.r;
    let up = r.unit();
    let rn = r.norm();
    let v_surf = v.v - body.surface_velocity(r);
    let vz = v_surf.dot(up);
    let vh = v_surf - up * vz;
    let alt = v.derived.alt.max(0.0);
    let (f_max, _) = burn_capacity(v);
    if f_max <= 0.0 {
        v.guidance.program = Program::None;
        out.events.push((Level::Alarm, "Descida propulsada: acabou o propelente".into()));
        return;
    }
    if !v.any_engine_lit() {
        v.ignite(out);
    }
    let mass = v.mass();
    let a_max = f_max / mass;
    let g_eff = body.mu() / (rn * rn) - vh.norm2() / rn;
    // Velocidade de descida alvo.
    let a_brake = (0.6 * (a_max - g_eff)).max(0.2);
    let t_brake = 1.3 * vh.norm() / (0.8 * a_max) + 5.0;
    let profile = |h: f64| (h / t_brake).min((2.0 * a_brake * (h - 15.0).max(0.0)).sqrt() + 1.5).clamp(1.5, 80.0);
    let v_down = profile(alt);
    // Quanto o alvo cai por segundo descendo na velocidade atual: entra direto
    // (senão o controle fica τ·a atrás e chega rápido demais ao chão).
    let ff = ((profile(alt) - profile((alt - 1.0).max(0.0))) * (-vz).max(0.0)).clamp(0.0, a_max);
    // Vertical primeiro; o resto, para a horizontal (mais suave na fase final,
    // em que a nave gira devagar e não pode inclinar muito).
    let az = (((-v_down) - vz) / 2.5 + g_eff + ff).clamp(0.0, a_max * 0.97);
    let ah_avail = (a_max * a_max - az * az).max(0.0).sqrt();
    // Perto do chão a inclinação máxima cai aos poucos (de 75° a 2,5 km para
    // 20° no fim), como o "pitch-up" do Apollo na portão alta.
    let tilt = (20.0 + (alt / 2_500.0).min(1.0) * 55.0).to_radians();
    let ah = (vh.norm() / if alt < 2_500.0 { 5.0 } else { 2.5 }).min(ah_avail).min(az.max(0.5) * tilt.tan());
    let dir_h = if vh.norm() > 1e-3 { -vh.unit() } else { Vec3::ZERO };
    let a_cmd = up * az + dir_h * ah;
    let a_mag = a_cmd.norm();
    let dir = if a_mag > 1e-6 { a_cmd / a_mag } else { up };
    // Enquanto o eixo não aponta para onde deve, o motor alivia (senão empurra
    // para o lado errado durante o giro).
    let err = v.att.forward().angle(dir).to_degrees();
    let align = (1.0 - (err - 8.0).max(0.0) / 30.0).clamp(0.15, 1.0);
    v.set_throttle((a_mag / a_max * align).clamp(0.0, 1.0));
    // Fica com o rolamento atual (o "baixo" da nave, tirado da frente), para
    // o giro gastar tudo na direção do empuxo.
    let mut down = v.att.rotate(Vec3::Z);
    if down.cross(dir).norm() < 0.3 {
        down = if dir.dot(up).abs() < 0.9 { -up } else { Vec3::Z.cross(up).unit() };
    }
    v.guidance.att_cmd = Some(Quat::look(dir, down));
    v.guidance.t_go = alt / v_down.max(0.1);
}

// ───────────────────────── Encontro e acoplamento ─────────────────────────

/// Aproximação automática pela V-bar: vai a um ponto de espera 200 m atrás do
/// alvo, depois avança devagar até a porta. Captura com menos de 0,3 m e
/// 0,25 m/s.
fn docking(v: &mut Vehicle, env: &Env, out: &mut StepOutput) {
    let Some(tid) = v.guidance.dock_target else {
        v.guidance.program = Program::None;
        return;
    };
    let Some(tg) = env.targets.iter().find(|x| x.id == tid).copied() else {
        v.guidance.program = Program::None;
        out.events.push((Level::Warn, "Alvo de acoplamento não encontrado".into()));
        return;
    };
    if tg.central != v.central {
        v.guidance.program = Program::None;
        return;
    }
    let (x, y, z) = lvlh_axes(tg.r, tg.v);
    let n = (v.central.mu() / tg.r.norm().powi(3)).sqrt();
    let (rho, rho_dot) = relative_lvlh(v, &tg);
    let port = Vec3::new(0.0, -tg.port_offset, 0.0);
    let range = (rho - port).norm();
    if rho.norm() > 8_000.0 {
        v.guidance.program = Program::None;
        v.guidance.rcs_accel = Vec3::ZERO;
        out.events.push((Level::Warn, format!("Alvo a {:.1} km: longe demais para a aproximação automática", rho.norm() / 1e3)));
        return;
    }
    // Ponto alvo: espera em −200 m na V-bar, depois a porta.
    let hold = Vec3::new(0.0, -tg.port_offset - 200.0, 0.0);
    let aim = if (rho - hold).norm() > 20.0 && rho.y < hold.y + 10.0 || rho.x.abs() > 30.0 || rho.z.abs() > 30.0 {
        hold
    } else {
        port
    };
    let to = aim - rho;
    let dist = to.norm();
    let speed = (0.002 * dist + 0.04).min(if aim == port { 0.3 } else { 2.0 });
    let v_des = if dist > 0.05 { to / dist * speed } else { Vec3::ZERO };
    // Controle: corrige a velocidade relativa; compensa a dinâmica de Hill.
    let hill = Vec3::new(3.0 * n * n * rho.x + 2.0 * n * rho_dot.y, -2.0 * n * rho_dot.x, -n * n * rho.z);
    let mut a_cmd = (v_des - rho_dot) * 0.05 - hill;
    let f = v.systems.as_ref().map_or(0.0, |s| s.adcs.rcs_force);
    let has_prop = v.systems.as_ref().is_some_and(|s| s.adcs.rcs_prop > 0.0);
    let a_max = if has_prop { f / v.mass() } else { 0.0 };
    if a_cmd.norm() > a_max {
        a_cmd = a_cmd.unit() * a_max;
    }
    v.guidance.rcs_accel = x * a_cmd.x + y * a_cmd.y + z * a_cmd.z;
    // Aponta a porta de acoplamento (nariz, +X) para o alvo.
    v.guidance.att_cmd = Some(Quat::look((tg.r - v.r).unit(), -v.r.unit()));
    if range < 0.3 && rho_dot.norm() < 0.25 {
        v.guidance.rcs_accel = Vec3::ZERO;
        v.guidance.program = Program::None;
        out.docked_to = Some(tid);
    }
}

/// Posição e velocidade da nave no LVLH do alvo (que gira com a órbita dele).
fn relative_lvlh(v: &Vehicle, tg: &TargetInfo) -> (Vec3, Vec3) {
    let (x, y, z) = lvlh_axes(tg.r, tg.v);
    let n = (v.central.mu() / tg.r.norm().powi(3)).sqrt();
    let dr = v.r - tg.r;
    let dv = v.v - tg.v - (z * n).cross(dr);
    (Vec3::new(dr.dot(x), dr.dot(y), dr.dot(z)), Vec3::new(dv.dot(x), dv.dot(y), dv.dot(z)))
}

/// Acoplamento pilotado: sem o programa automático, a nave que chega à porta
/// livre de outra (a menos de 1,5 m, se aproximando a menos de 0,4 m/s) é
/// capturada, como na sonda e no cone de uma porta de verdade.
fn manual_capture(v: &Vehicle, env: &Env, out: &mut StepOutput) {
    if v.attached_to.is_some() || v.guidance.program != Program::None || v.systems.is_none() || v.phase.is_final() {
        return;
    }
    for tg in env.targets.iter().filter(|x| x.id != v.id && x.central == v.central && x.port_free) {
        if (tg.r - v.r).norm() > 100.0 {
            continue;
        }
        let (rho, rho_dot) = relative_lvlh(v, tg);
        let to_port = rho - Vec3::new(0.0, -tg.port_offset, 0.0);
        if to_port.norm() < 1.5 && rho_dot.norm() < 0.4 && to_port.dot(rho_dot) < 0.0 {
            out.docked_to = Some(tg.id);
            out.events.push((Level::Info, format!("Contato a {:.2} m/s", rho_dot.norm())));
            return;
        }
    }
}

// ───────────────────────── RTLS ─────────────────────────

/// Retorno ao local de lançamento: vira e empurra contra a velocidade horizontal
/// até ganhar a velocidade de volta, corta e plana para o local.
fn rtls(v: &mut Vehicle, t: f64, out: &mut StepOutput) {
    let Some(site) = v.site.clone() else {
        v.guidance.program = Program::None;
        return;
    };
    let d = v.derived.clone();
    let (up, east, north) = local_frame(v.r);
    let brg = bearing_deg(d.lat, d.lon, site.lat, site.lon).to_radians();
    let home = north * brg.cos() + east * brg.sin();
    let dist = great_circle_km(d.lat, d.lon, site.lat, site.lon) * 1e3;
    // Velocidade horizontal necessária para planar de volta (~ distância/120 s, máx. 2 km/s).
    let v_need = (dist / 140.0).clamp(400.0, 2_000.0);
    let v_surf = v.v - v.central.surface_velocity(v.r);
    let vh = v_surf.reject(up);
    let want = home * v_need;
    let gap = want - vh;
    if gap.norm() < 40.0 || !v.any_engine_lit() {
        if v.any_engine_lit() {
            v.shutdown();
            out.events.push((Level::Info, "RTLS: corte do motor, separação do tanque externo".into()));
        }
        // Larga o tanque e plana para casa.
        if v.stages.len() > 1 {
            while v.stages.len() > 1 {
                if let Some(dbr) = v.jettison_bottom(t) {
                    out.jettisoned.push(dbr);
                }
            }
        }
        v.guidance.landing_target = Some((site.lat, site.lon));
        v.guidance.program = Program::Entry;
        return;
    }
    // Mantém a altitude com um pouco de componente vertical.
    let lift = if d.vspeed < 0.0 { 0.35 } else { 0.1 };
    let dir = (gap.unit() + up * lift).unit();
    v.guidance.att_cmd = Some(Quat::look(dir, -up));
}
