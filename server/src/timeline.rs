//! Linha do tempo da missão (console FAO): o que vai acontecer com um veículo
//! nas próximas horas — contagem, janela, queimas, comandos agendados,
//! passagens nas estações, eclipses, periastros e apoastros e, para o
//! instrutor, as falhas planejadas.

use crate::App;
use jaci_sim::{
    bodies::{Body, sun_direction, sunlit},
    ground,
    launch::CountState,
    world::CmdStatus,
};
use serde_json::{Value, json};

/// Passo das amostras da trajetória (s): fino o bastante para eclipses de minutos.
fn step_for(span: f64) -> f64 {
    (span / 2_000.0).clamp(10.0, 120.0)
}

pub fn build(a: &App, id: u32, span: f64, show_faults: bool) -> Value {
    let w = &a.world;
    let t = w.t;
    let span = span.clamp(600.0, 3.0 * 86_400.0);
    let central = w.vehicle(id).map(|v| v.central).or_else(|| w.tracked.iter().any(|s| s.id == id).then_some(Body::Earth));

    // Trajetória prevista: eclipses e apsides.
    let mut eclipses: Vec<[f64; 2]> = Vec::new();
    let mut apsides: Vec<Value> = Vec::new();
    let flying = w.vehicle(id).is_none_or(|v| !matches!(v.phase, jaci_sim::vehicle::Phase::Prelaunch | jaci_sim::vehicle::Phase::Landed) && !v.phase.is_final());
    if let (Some(c), true) = (central, flying) {
        let samples = w.predict(id, span, step_for(span));
        let mut start: Option<f64> = None;
        for (tt, _, _, _, r) in &samples {
            let lit = sunlit(c, *r, sun_direction(c, *r, *tt).0) >= 0.5;
            match (lit, start) {
                (false, None) => start = Some(*tt),
                (true, Some(s)) => {
                    eclipses.push([s, *tt]);
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(s) = start {
            eclipses.push([s, t + span]);
        }
        // Pelos elementos osculadores: com órbita quase circular, a oscilação do J2
        // no raio criaria extremos falsos nas amostras.
        let state = w.vehicle(id).map(|v| (v.r, v.v)).or_else(|| w.tracked.iter().find(|s| s.id == id).and_then(|s| s.rv(t)));
        if let Some((r, vel)) = state {
            let mu = c.mu();
            let el = jaci_sim::orbit::elements_from_state(r, vel, mu);
            if el.e < 1.0 && el.e > 1e-4 {
                let period = el.period(mu);
                let first_pe = t + jaci_sim::orbit::time_to_true_anomaly(&el, mu, 0.0);
                let first_ap = t + jaci_sim::orbit::time_to_true_anomaly(&el, mu, std::f64::consts::PI);
                for (first, kind, rr) in [(first_pe, "pe", el.periapsis()), (first_ap, "ap", el.apoapsis())] {
                    let mut tt = first;
                    while tt <= t + span && period > 60.0 {
                        apsides.push(json!({"t": tt, "kind": kind, "alt": (rr - c.radius()) / 1e3}));
                        tt += period;
                    }
                }
                apsides.sort_by(|x, y| x["t"].as_f64().partial_cmp(&y["t"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
            }
        }
    }

    // Passagens nas estações.
    let passes = if let Some(s) = w.tracked.iter().find(|s| s.id == id) {
        s.passes(&w.stations, t, span)
    } else if let Some(v) = w.vehicle(id).filter(|v| v.central == Body::Earth && flying) {
        ground::passes(v, &w.stations, t, span)
    } else {
        vec![]
    };

    // Queimas planejadas.
    let burns: Vec<Value> = w
        .vehicle(id)
        .map(|v| {
            v.guidance
                .nodes
                .iter()
                .filter(|n| !n.done)
                .map(|n| {
                    let dur = jaci_sim::guidance::burn_duration(v, n.dv_rtn.norm(), n.rcs);
                    json!({"t": n.t, "label": n.label, "dv": n.dv_rtn.norm(), "duration": dur, "executing": n.executing})
                })
                .collect()
        })
        .unwrap_or_default();

    // Comandos com hora marcada (ou a caminho, pelo atraso da luz).
    let commands: Vec<Value> = w.commands.iter().filter(|c| c.vehicle == id && c.status == CmdStatus::Sent && c.exec_at > t).map(|c| json!({"t": c.exec_at, "text": c.text})).collect();

    // Contagem: a sequência em hora absoluta (parada, como se retomasse agora).
    let lc = &w.launch;
    let countdown: Vec<Value> = if lc.vehicle == Some(id) && matches!(lc.state, CountState::Counting | CountState::Hold) {
        lc.sequence.iter().filter(|s| !s.done && s.at <= lc.t_minus).map(|s| json!({"t": t + (lc.t_minus - s.at), "name": s.name, "at": s.at})).collect()
    } else {
        vec![]
    };
    let window = if lc.vehicle == Some(id) { lc.window } else { None };

    // Falhas planejadas: só para o instrutor.
    let faults: Vec<Value> = if show_faults {
        w.training
            .plan
            .iter()
            .filter(|f| f.vehicle == id)
            .filter_map(|f| w.plan_due(f).map(|due| json!({"t": due, "fault": f.fault, "announce": f.announce})))
            .collect()
    } else {
        vec![]
    };

    json!({
        "type": "timeline",
        "vehicle": id,
        "t": t,
        "span": span,
        "eclipses": eclipses,
        "apsides": apsides,
        "passes": passes,
        "burns": burns,
        "commands": commands,
        "countdown": countdown,
        "window": window,
        "hold": matches!(lc.state, CountState::Hold) && lc.vehicle == Some(id),
        "faults": faults,
    })
}
