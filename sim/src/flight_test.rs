//! Teste de voo do hangar: voa um veículo de arquivo da plataforma até a
//! órbita (ou até cair) num mundo à parte, sem tocar no da sala, e conta como
//! foi: a órbita, os picos de pressão dinâmica e de carga g, os eventos e a
//! trajetória.

use crate::commands::Command;
use crate::launch::CountState;
use crate::library::Library;
use crate::vehicle::{Level, Phase};
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct FlightPoint {
    /// Segundos desde a decolagem.
    pub t: f64,
    pub alt_km: f64,
    pub downrange_km: f64,
    pub speed: f64,
    pub q_kpa: f64,
    pub g: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct FlightReport {
    pub ok: bool,
    /// O que aconteceu, em uma frase.
    pub result: String,
    pub pe_km: f64,
    pub ap_km: f64,
    pub inc_deg: f64,
    pub max_q_kpa: f64,
    pub max_q_t: f64,
    pub max_g: f64,
    /// Segundos da decolagem à inserção.
    pub time_to_orbit: Option<f64>,
    pub dv_left: f64,
    pub events: Vec<(f64, Level, String)>,
    pub track: Vec<FlightPoint>,
}

/// Voa o veículo (TOML de missoes/veiculos) a partir do local de lançamento dado.
pub fn test_flight(lib: &Library, vehicle_toml: &str, site: &str, t0: f64) -> Result<FlightReport, String> {
    let mut lib = lib.clone();
    lib.insert("veiculos", "_teste_de_voo", vehicle_toml)?;
    let scenario = format!("name = \"Teste de voo\"\n[[vehicles]]\nvehicle = \"_teste_de_voo\"\nplace = \"pad\"\nsite = {}\nt_minus = 30\n", toml_str(site));
    lib.insert("cenarios", "_teste_de_voo_missao", &scenario)?;
    let mut w = lib.scenario("_teste_de_voo_missao", t0, 7)?;
    // Tempo bom e área livre: o teste é do foguete, não do clima.
    w.weather.evolve = false;
    w.weather.surface_wind = 3.0;
    w.weather.lightning_km = 150.0;
    w.weather.ceiling_m = 5_000.0;
    w.weather.rain = false;
    let vid = w.launch.vehicle.ok_or("sem veículo na plataforma")?;
    w.execute(vid, &Command::new("count_start")).map_err(|e| format!("contagem: {e}"))?;
    let fail = |w: &crate::world::World, why: String| -> FlightReport {
        FlightReport { ok: false, result: why, pe_km: 0.0, ap_km: 0.0, inc_deg: 0.0, max_q_kpa: 0.0, max_q_t: 0.0, max_g: 0.0, time_to_orbit: None, dv_left: 0.0, events: events(w, vid, w.t), track: vec![] }
    };
    let mut guard = 0;
    while w.vehicle(vid).map(|v| v.phase) == Some(Phase::Prelaunch) {
        w.launch.range_fouled = 0.0;
        w.step(0.5);
        if w.launch.state == CountState::Hold {
            let _ = w.execute(vid, &Command::new("poll"));
            if let Err(e) = w.execute(vid, &Command::new("count_start")) {
                let bad: Vec<String> = w.launch.criteria.iter().filter(|c| !c.ok).map(|c| format!("{} ({})", c.name, c.value)).collect();
                return Ok(fail(&w, format!("A contagem não passou: {e}{}", if bad.is_empty() { String::new() } else { format!(": {}", bad.join(", ")) })));
            }
        }
        if w.launch.state == CountState::Scrubbed {
            return Ok(fail(&w, format!("Lançamento cancelado: {}", w.launch.hold_reason)));
        }
        guard += 1;
        if guard > 4_000 {
            return Ok(fail(&w, "A contagem não terminou".into()));
        }
    }
    let lift = w.t;
    let mut track = Vec::new();
    let mut next = lift;
    let mut inserted_at = None;
    // Subida: amostra a cada 2 s até a inserção (ou 30 min).
    while w.t < lift + 1_800.0 {
        let v = w.vehicle(vid).ok_or("o veículo sumiu")?;
        if w.t >= next {
            let d = &v.derived;
            track.push(FlightPoint { t: w.t - lift, alt_km: d.alt / 1e3, downrange_km: d.downrange_km, speed: d.speed, q_kpa: d.q / 1e3, g: d.accel_g });
            next = w.t + 2.0;
        }
        if v.guidance.inserted {
            inserted_at = Some(w.t - lift);
            break;
        }
        if v.phase.is_final() {
            break;
        }
        w.step(0.25);
    }
    // Depois da inserção: as queimas que ainda faltam (circularização, apogeu) e a separação.
    if inserted_at.is_some() {
        let end = w.t + 4.0 * 3_600.0;
        while w.t < end {
            let v = w.vehicle(vid).ok_or("o veículo sumiu")?;
            if v.guidance.nodes.iter().all(|n| n.done) && !v.guidance.payload_after_burns && v.guidance.payload_timer.is_none() && !v.any_engine_lit() {
                break;
            }
            w.step(1.0);
        }
    }
    let v = w.vehicle(vid).ok_or("o veículo sumiu")?;
    let el = &v.derived.elements;
    let re = v.central.radius();
    let (pe, ap) = ((el.periapsis() - re) / 1e3, (el.apoapsis() - re) / 1e3);
    let result = match v.phase {
        Phase::Destroyed => "O veículo foi destruído.".to_string(),
        p if p.is_final() => "Voltou ao chão sem chegar à órbita.".to_string(),
        _ if inserted_at.is_some() && pe > 120.0 => format!("Em órbita: {pe:.0} × {ap:.0} km, {:.1}°.", el.i.to_degrees()),
        _ if inserted_at.is_some() && pe < 0.0 => format!("Ficou suborbital: o último estágio apagou com o apoastro em {ap:.0} km. Falta Δv."),
        _ if inserted_at.is_some() => format!("O último estágio apagou com o periastro em {pe:.0} km: a órbita cai. Falta um pouco de Δv."),
        _ if pe > 0.0 => format!("Não completou a subida: {pe:.0} × {ap:.0} km. Falta Δv ou empuxo."),
        _ => format!("Não chegou à órbita (apoastro {ap:.0} km): falta Δv ou empuxo."),
    };
    Ok(FlightReport {
        ok: inserted_at.is_some() && pe > 120.0 && !v.phase.is_final(),
        result,
        pe_km: pe,
        ap_km: ap,
        inc_deg: el.i.to_degrees(),
        max_q_kpa: v.stats.max_q / 1e3,
        max_q_t: v.stats.max_q_met,
        max_g: v.stats.max_g,
        time_to_orbit: inserted_at,
        dv_left: v.derived.dv_remaining,
        events: events(&w, vid, lift),
        track,
    })
}

/// Eventos do veículo (e os gerais), com o tempo desde a decolagem.
fn events(w: &crate::world::World, vid: u32, lift: f64) -> Vec<(f64, Level, String)> {
    w.events.iter().filter(|e| e.vehicle.is_none_or(|x| x == vid) && e.from.is_none()).map(|e| (e.t - lift, e.level, e.text.clone())).collect()
}

fn toml_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lib() -> Library {
        Library::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../missoes"))
    }
    const T0: f64 = 846_000_000.0;

    #[test]
    fn o_lancador_leve_do_hangar_chega_a_orbita() {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/hangar/hangar-leve.toml")).unwrap();
        let r = test_flight(&lib(), &text, "Alcântara", T0).unwrap();
        println!("{} · max q {:.1} kPa em {:.0} s · {:.1} g · {} pontos", r.result, r.max_q_kpa, r.max_q_t, r.max_g, r.track.len());
        assert!(r.ok, "{}", r.result);
        assert!(r.track.len() > 50 && r.max_q_kpa > 5.0 && r.max_g > 1.0);
        assert!(r.events.iter().any(|e| e.2.contains("Separação") || e.2.contains("estágio")), "sem eventos de estágio");
    }

    #[test]
    fn um_foguete_fraco_nao_chega() {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/hangar/hangar-leve.toml")).unwrap();
        // Carga de 2 t no lugar de 150 kg.
        let heavy = text.replace("name = \"Satélite\"\ndry_kg = 150", "name = \"Satélite\"\ndry_kg = 2000");
        assert_ne!(heavy, text);
        let r = test_flight(&lib(), &heavy, "Alcântara", T0).unwrap();
        println!("{}", r.result);
        assert!(!r.ok);
    }
}
