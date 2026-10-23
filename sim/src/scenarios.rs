//! Cenários prontos: cada um monta um mundo com veículos, local e hora.

use crate::bodies::Body;
use crate::catalog;
use crate::math::{DEG, Quat};
use crate::orbit::{Elements, state_from_elements};
use crate::subsystems::AttMode;
use crate::vehicle::*;
use crate::world::World;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct ScenarioInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub const SCENARIOS: &[ScenarioInfo] = &[
    ScenarioInfo { id: "sondagem", name: "Foguete de sondagem VSB-30 (Alcântara)", description: "Contagem a partir de T-10 min, voo suborbital de microgravidade e recuperação da carga no mar" },
    ScenarioInfo { id: "vlm", name: "Microlançador VLM-1 (Alcântara)", description: "Três estágios sólidos levando um satélite de 150 kg a órbita equatorial" },
    ScenarioInfo { id: "sso", name: "Satélite de observação em órbita heliossíncrona", description: "Lançador médio partindo de Alcântara para 752 km, 98,4°; carregamento de propelente na contagem" },
    ScenarioInfo { id: "geo", name: "Satélite geoestacionário", description: "Inserção em GTO a partir de Alcântara e queimas de apogeu até a órbita geoestacionária" },
    ScenarioInfo { id: "tripulada", name: "Tripulação para a estação espacial", description: "Cápsula em Cabo Canaveral com janela no plano da estação, encontro e acoplamento" },
    ScenarioInfo { id: "soyuz", name: "Nave com boosters em Baikonur", description: "Quatro boosters laterais, torre de escape e encontro com a estação" },
    ScenarioInfo { id: "sts", name: "Ônibus espacial", description: "Subida com foguetes sólidos e tanque externo, abortos RTLS/TAL/ATO/AOA, reentrada e pouso em pista" },
    ScenarioInfo { id: "estacao", name: "Operações da estação espacial", description: "Estação com 7 tripulantes e uma cápsula acoplada: energia, suporte de vida, reimpulso e emergências" },
    ScenarioInfo { id: "retorno", name: "Retorno de cápsula", description: "Cápsula em órbita: saída de órbita mirando o mar da Flórida, reentrada guiada e paraquedas" },
    ScenarioInfo { id: "observacao", name: "Satélite de observação em operação", description: "Imageamento, gravador de bordo, passagens nas estações e modo de segurança" },
    ScenarioInfo { id: "marte", name: "Sonda a Marte", description: "Pilha em órbita de estacionamento na janela de 2026: porkchop, injeção, cruzeiro com correções e captura" },
    ScenarioInfo { id: "lua", name: "Sonda lunar", description: "Injeção translunar da órbita de estacionamento e inserção em órbita da Lua" },
    ScenarioInfo { id: "ksp", name: "Kerbal Space Program (kOS)", description: "Sem veículos simulados: o Jaci acompanha e comanda as naves do KSP pelo kOS" },
];

/// Coloca um veículo numa órbita.
pub fn place_in_orbit(v: &mut Vehicle, central: Body, pe_km: f64, ap_km: f64, inc_deg: f64, raan_deg: f64, argp_deg: f64, nu_deg: f64) {
    let re = central.radius();
    let rp = re + pe_km * 1e3;
    let ra = re + ap_km * 1e3;
    let el = Elements {
        a: (rp + ra) / 2.0,
        e: (ra - rp) / (ra + rp),
        i: inc_deg * DEG,
        raan: raan_deg * DEG,
        argp: argp_deg * DEG,
        nu: nu_deg * DEG,
    };
    let (r, vel) = state_from_elements(&el, central.mu());
    v.central = central;
    v.r = r;
    v.v = vel;
    v.phase = Phase::Orbit;
    v.released = true;
    v.att = Quat::look(vel.unit(), -r.unit());
    v.guidance.ascent = None;
    v.guidance.program = Program::None;
    v.guidance.inserted = true;
}

pub fn on_pad(v: &mut Vehicle, site: Site, t: f64) {
    v.site = Some(site);
    v.phase = Phase::Prelaunch;
    v.released = false;
    if let Some((r, vel)) = v.pad_state(t) {
        v.r = r;
        v.v = vel;
    }
    v.att = v.upright();
    // Criogênicos vazios: o carregamento faz parte da contagem.
    for s in &mut v.stages {
        if s.cryogenic {
            s.propellant = 0.0;
        }
    }
}

fn station(w: &mut World, t: f64) -> VehicleId {
    let mut st = catalog::build("estacao").unwrap();
    place_in_orbit(&mut st, Body::Earth, 415.0, 425.0, 51.64, ((t / 86_400.0) * 13.0 + 120.0) % 360.0, 0.0, 30.0);
    if let Some(s) = &mut st.systems {
        s.adcs.mode = AttMode::Nadir;
    }
    st.att = st.attitude_target(&Env { t, wind_speed: 0.0, wind_jet: 0.0, wind_from: 0.0, targets: &[], coarse: true }).unwrap_or(st.att);
    st.launch_time = Some(t - 86_400.0 * 900.0);
    w.add_vehicle(st)
}

/// Monta o cenário no instante `t`. `None` se não existe.
pub fn build(id: &str, t: f64, seed: u64) -> Option<World> {
    let mut w = World::new(t, seed);
    w.scenario = id.into();
    let info = SCENARIOS.iter().find(|s| s.id == id)?;
    w.mission = info.name.into();
    match id {
        "sondagem" => {
            let mut v = catalog::build("vsb30")?;
            on_pad(&mut v, catalog::site("Alcântara"), t);
            let vid = w.add_vehicle(v);
            let v = w.vehicle(vid)?.clone();
            w.launch.setup(&v, 600.0);
        }
        "vlm" => {
            let mut v = catalog::build("vlm1")?;
            on_pad(&mut v, catalog::site("Alcântara"), t);
            let vid = w.add_vehicle(v);
            let v = w.vehicle(vid)?.clone();
            w.launch.setup(&v, 900.0);
        }
        "sso" | "geo" => {
            let mut v = catalog::build(if id == "sso" { "medio_sso" } else { "medio_geo" })?;
            on_pad(&mut v, catalog::site("Alcântara"), t);
            let vid = w.add_vehicle(v);
            let v = w.vehicle(vid)?.clone();
            w.launch.setup(&v, 3_000.0);
        }
        "tripulada" | "soyuz" => {
            let sid = station(&mut w, t);
            let (def, site) = if id == "tripulada" { ("medio_capsula", "Cabo Canaveral") } else { ("soyuz", "Baikonur") };
            let mut v = catalog::build(def)?;
            on_pad(&mut v, catalog::site(site), t);
            let vid = w.add_vehicle(v);
            // Janela no plano da estação.
            let cmd = crate::commands::Command::new("window").with("target", sid);
            if w.execute(vid, &cmd).is_err() {
                let v = w.vehicle(vid)?.clone();
                w.launch.setup(&v, 3_000.0);
            }
            // Começa a 50 min do T-0 (o resto do tempo passa acelerado, se quiser).
            if w.launch.t_minus > 3_000.0 {
                let skip = w.launch.t_minus - 3_000.0;
                w.t += skip;
                w.launch.t_minus = 3_000.0;
                let t_now = w.t;
                for v in &mut w.vehicles {
                    if v.phase == Phase::Orbit {
                        let (r, vel) = crate::orbit::propagate_kepler(v.r, v.v, skip, v.central.mu());
                        v.r = r;
                        v.v = vel;
                        // A atitude acompanha (senão a estação começa fora do apontamento).
                        let env = Env { t: t_now, wind_speed: 0.0, wind_jet: 0.0, wind_from: 0.0, targets: &[], coarse: true };
                        if let Some(q) = v.attitude_target(&env) {
                            v.att = q;
                        }
                    }
                }
                if let Some(v) = w.vehicles.iter_mut().find(|v| v.id == vid) {
                    if let Some((r, vel)) = v.pad_state(w.t) {
                        v.r = r;
                        v.v = vel;
                    }
                }
                let v = w.vehicle(vid)?.clone();
                let window = w.launch.window;
                w.launch.setup(&v, 3_000.0);
                w.launch.window = window;
            }
        }
        "sts" => {
            let mut v = catalog::build("sts")?;
            on_pad(&mut v, catalog::site("Cabo Canaveral"), t);
            let vid = w.add_vehicle(v);
            let v = w.vehicle(vid)?.clone();
            w.launch.setup(&v, 3_000.0);
        }
        "estacao" => {
            let sid = station(&mut w, t);
            let mut cap = catalog::build("medio_capsula")?;
            cap.stages.retain(|s| s.spacecraft);
            cap.initial_stages = cap.stages.len();
            let (r, vel) = (w.vehicle(sid)?.r, w.vehicle(sid)?.v);
            cap.r = r;
            cap.v = vel;
            cap.phase = Phase::Docked;
            cap.released = true;
            cap.launch_time = Some(t - 86_400.0 * 20.0);
            cap.guidance.inserted = true;
            cap.guidance.ascent = None;
            if let Some(s) = &mut cap.systems {
                s.adcs.mode = AttMode::Free;
                if let Some(e) = &mut s.eclss {
                    e.crew = 0;
                }
            }
            let cid = w.add_vehicle(cap);
            if let Some(c) = w.vehicle_mut(cid) {
                c.attached_to = Some(sid);
            }
            if let Some(s) = w.vehicle_mut(sid) {
                s.ports[0].occupant = Some(cid);
            }
        }
        "retorno" => {
            let mut cap = catalog::build("medio_capsula")?;
            cap.stages.retain(|s| s.spacecraft);
            cap.initial_stages = cap.stages.len();
            place_in_orbit(&mut cap, Body::Earth, 400.0, 410.0, 51.6, 200.0, 0.0, 0.0);
            cap.launch_time = Some(t - 86_400.0 * 30.0);
            if let Some(s) = &mut cap.systems {
                s.adcs.mode = AttMode::Hold;
            }
            w.add_vehicle(cap);
        }
        "observacao" => {
            let mut sat = catalog::build("amazonia")?;
            place_in_orbit(&mut sat, Body::Earth, 752.0, 752.0, 98.4, 10.0, 0.0, 200.0);
            sat.launch_time = Some(t - 86_400.0 * 400.0);
            if let Some(s) = &mut sat.systems {
                s.adcs.mode = AttMode::Nadir;
                s.comms.stored_mb = 20_000.0;
            }
            w.add_vehicle(sat);
        }
        "marte" | "lua" => {
            let mut v = catalog::build(if id == "marte" { "sonda_marte" } else { "sonda_lua" })?;
            place_in_orbit(&mut v, Body::Earth, 185.0, 185.0, 28.5, 40.0, 0.0, 0.0);
            v.launch_time = Some(t - 3_000.0);
            v.site = Some(catalog::site("Cabo Canaveral"));
            if let Some(s) = &mut v.systems {
                s.adcs.mode = AttMode::Prograde;
            }
            w.add_vehicle(v);
        }
        // As naves vêm do jogo (server/src/ksp.rs).
        "ksp" => {}
        _ => return None,
    }
    Some(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_os_cenarios_montam() {
        let t = crate::time::from_calendar(2026, 10, 19, 12, 0, 0.0);
        for s in SCENARIOS {
            let w = build(s.id, t, 1).unwrap_or_else(|| panic!("{}", s.id));
            assert!(!w.vehicles.is_empty() || s.id == "ksp");
        }
    }
}

#[cfg(test)]
mod salvar {
    #[test]
    fn estado_vai_e_volta_em_json() {
        let t = crate::time::from_calendar(2026, 10, 19, 12, 0, 0.0);
        for s in super::SCENARIOS {
            let mut w = super::build(s.id, t, 1).unwrap();
            w.advance(2.0);
            let j = serde_json::to_string(&w).unwrap();
            let mut back: crate::world::World = serde_json::from_str(&j).unwrap_or_else(|e| panic!("{}: {e}", s.id));
            back.advance(1.0);
            assert_eq!(back.vehicles.len(), w.vehicles.len());
        }
    }
}
