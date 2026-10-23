//! O que vai para a interface: a saudação, os instantâneos e os dicionários.

use crate::App;
use jaci_sim::{
    bodies::Body,
    catalog, commands,
    launch::CountState,
    scenarios,
    telemetry,
    time::{format_met, format_utc},
    vehicle::{EngineState, Kind, Vehicle},
    world::{FAULTS, World},
};
use serde_json::{Value, json};

fn v3(v: jaci_sim::math::Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

fn num(x: f64) -> Value {
    if x.is_finite() { json!(x) } else { Value::Null }
}

pub fn hello(a: &mut App) -> String {
    let w = &a.world;
    let events: Vec<Value> = w.events.iter().rev().take(400).rev().map(event).collect();
    json!({
        "type": "hello",
        "version": env!("CARGO_PKG_VERSION"),
        "t": w.t,
        "scenarios": scenario_list(a),
        "catalog": catalog_list(a),
        "procedures": procedure_list(a),
        "lessons": lesson_list(a),
        "commands": commands::dictionary().into_iter().chain(crate::ksp::commands()).collect::<Vec<_>>(),
        "faults": FAULTS.iter().map(|(id, name, arg)| json!({"id": id, "name": name, "arg": arg})).collect::<Vec<_>>(),
        "stations": w.stations,
        "sites": catalog::sites(),
        "bodies": Body::ALL.iter().map(|b| json!({"id": b.id(), "name": b.name(), "radius": b.radius(), "parent": b.parent().map(|p| p.id()), "soi": num(b.soi())})).collect::<Vec<_>>(),
        "events": events,
        "dictionary": dictionary_map(a),
        "roles": crate::room::roles_json(),
    })
    .to_string()
}

/// Cenários prontos e os dos arquivos.
fn scenario_list(a: &App) -> Value {
    let mut v: Vec<Value> = scenarios::SCENARIOS.iter().map(|s| json!(s)).collect();
    v.extend(a.library.scenarios.iter().map(|(id, s)| json!({"id": id, "name": s.name, "description": s.description, "file": true})));
    json!(v)
}

fn catalog_list(a: &App) -> Value {
    let mut v: Vec<Value> = catalog::ENTRIES.iter().map(|e| json!(e)).collect();
    v.extend(a.library.vehicles.iter().map(|(id, f)| {
        let kind = a.library.vehicle(id).map(|x| x.kind.id()).unwrap_or("");
        json!({"id": id, "name": f.name, "kind": kind, "description": f.description, "file": true})
    }));
    json!(v)
}

fn procedure_list(a: &App) -> Value {
    json!(a.library.procedures.iter().map(|(id, p)| {
        let mut v = json!(p);
        v["id"] = json!(format!("arquivo:{id}"));
        v
    }).collect::<Vec<_>>())
}

fn lesson_list(a: &App) -> Value {
    let mut v: Vec<Value> = a.library.lessons.iter().map(|(id, l)| {
        let mut x = json!(l);
        x["id"] = json!(id);
        x
    }).collect();
    v.sort_by_key(|x| (x["order"].as_i64().unwrap_or(0), x["id"].as_str().unwrap_or("").to_string()));
    json!(v)
}

/// As listas que mudam quando um arquivo de missão é salvo.
pub fn library(a: &App) -> Value {
    json!({"type": "library", "scenarios": scenario_list(a), "catalog": catalog_list(a), "procedures": procedure_list(a), "lessons": lesson_list(a)})
}

fn event(e: &jaci_sim::vehicle::Event) -> Value {
    let mut v = json!({"t": e.t, "vehicle": e.vehicle, "level": format!("{:?}", e.level).to_lowercase(), "text": e.text});
    if let Some(f) = &e.from {
        v["from"] = json!(f);
        v["channel"] = json!(e.channel);
    }
    v
}

fn dictionary_map(a: &App) -> Value {
    let w = &a.world;
    let mut m = serde_json::Map::new();
    if let Some(link) = &a.ksp {
        let defs = json!(crate::ksp::tm_defs());
        for v in &link.vessels {
            m.insert(v.id.to_string(), defs.clone());
        }
    }
    for v in &w.vehicles {
        if v.kind == Kind::Debris {
            continue;
        }
        m.insert(v.id.to_string(), json!(telemetry::defs(v)));
    }
    if !w.tracked.is_empty() {
        let defs = json!(jaci_sim::tracked::tm_defs());
        for s in &w.tracked {
            m.insert(s.id.to_string(), defs.clone());
        }
    }
    for e in &w.externals {
        let defs: Vec<Value> = if e.points.is_empty() {
            jaci_sim::ccsds::NAV_POINTS.iter().map(|(id, name, unit)| json!({"id": id, "name": name, "unit": unit, "group": "Navegação", "yellow": null, "red": null})).collect()
        } else {
            // Dicionário XTCE: nomes, unidades e limites de lá.
            e.points.iter().map(|p| json!({"id": p.id, "name": p.name, "unit": p.unit, "group": "XTCE", "yellow": p.yellow, "red": p.red})).collect()
        };
        m.insert(e.id.to_string(), json!(defs));
    }
    Value::Object(m)
}

pub fn dictionary(a: &App) -> Value {
    json!({"type": "dictionary", "dictionary": dictionary_map(a)})
}

fn signature(a: &App) -> String {
    let w = &a.world;
    let mut s = String::new();
    for v in a.ksp.iter().flat_map(|l| &l.vessels) {
        s.push_str(&format!("k{};", v.id));
    }
    for v in w.vehicles.iter().filter(|v| v.kind != Kind::Debris) {
        s.push_str(&format!("{}:{}:{}:{};", v.id, v.stages.len(), v.chutes.len(), v.crew()));
    }
    for e in &w.externals {
        s.push_str(&format!("x{};", e.id));
    }
    for t in &w.tracked {
        s.push_str(&format!("s{};", t.id));
    }
    s
}

pub fn dictionary_if_changed(a: &mut App) -> Option<String> {
    let sig = signature(a);
    if sig == a.dict_sig {
        return None;
    }
    a.dict_sig = sig;
    Some(dictionary(a).to_string())
}

fn vehicle(w: &World, v: &Vehicle) -> Value {
    let d = &v.derived;
    let el = &d.elements;
    let re = v.central.radius();
    let debris = v.kind == Kind::Debris;
    let mut o = json!({
        "id": v.id,
        "name": v.name,
        "kind": v.kind.id(),
        "kind_name": v.kind.name(),
        "def": v.def_id,
        "phase": v.phase.id(),
        "phase_name": v.phase.name(),
        "central": v.central.id(),
        "parent": v.parent,
        "attached_to": v.attached_to,
        "crew": v.crew(),
        "met": v.launch_time.map(|l| w.t - l),
        "lat": d.lat, "lon": d.lon, "alt": d.alt / 1e3,
        "speed": d.speed,
        "r": v3(v.r), "v": v3(v.v),
        "q": [v.att.w, v.att.x, v.att.y, v.att.z],
        "pe": num((el.periapsis() - re) / 1e3),
        "ap": num((el.apoapsis() - re) / 1e3),
        "inc": el.i.to_degrees(),
        "ecc": el.e,
    });
    if debris {
        o["stages"] = json!(v.stages.iter().map(|s| json!({"name": s.name})).collect::<Vec<_>>());
        return o;
    }
    let tm: serde_json::Map<String, Value> = telemetry::values(v, w.t).into_iter().map(|(k, x)| (k, num(x))).collect();
    o["tm"] = Value::Object(tm);
    o["stages"] = json!(v
        .stages
        .iter()
        .map(|s| json!({
            "name": s.name,
            "prop": s.propellant,
            "capacity": s.capacity,
            "dry": s.dry_mass,
            "boosters": s.boosters,
            "spacecraft": s.spacecraft,
            "cryogenic": s.cryogenic,
            "diameter": s.diameter,
            "length": s.length,
            "leak": s.leak > 0.0,
            "engines": s.engines.iter().map(|e| json!({
                "name": e.name,
                "state": match e.state { EngineState::Off => "off", EngineState::Starting => "starting", EngineState::Running => "running", EngineState::Stopping => "stopping", EngineState::Failed => "failed" },
                "thrust": e.thrust / 1e3,
                "rated": e.thrust_vac / 1e3,
                "pc": e.pc(),
                "throttle": e.throttle,
                "solid": e.solid,
                "ignitions": e.ignitions,
                "burn": e.burn_time,
            })).collect::<Vec<_>>(),
        }))
        .collect::<Vec<_>>());
    o["shape"] = json!(format!("{:?}", v.shape).to_lowercase());
    o["items"] = json!(v.items.iter().map(|i| json!({"id": i.id, "name": i.name})).collect::<Vec<_>>());
    o["chutes"] = json!(v.chutes.iter().map(|c| json!({"name": c.name, "state": format!("{:?}", c.state).to_lowercase(), "inflation": c.inflation, "alt": c.deploy_alt})).collect::<Vec<_>>());
    o["les"] = v.les.as_ref().map_or(Value::Null, |l| json!({"available": l.available, "firing": l.firing > 0.0, "tower": l.tower_mass > 0.0}));
    o["heat_shield"] = v.heat_shield.as_ref().map_or(Value::Null, |h| json!({"ablator": h.ablator_kg / h.initial_kg.max(1e-9), "temp": h.temp, "flux": h.flux / 1e3, "peak": h.peak_flux / 1e3}));
    o["landing"] = json!(format!("{:?}", v.landing).to_lowercase());
    o["gear_down"] = json!(v.gear_down);
    o["ports"] = json!(v.ports.iter().map(|p| json!({"name": p.name, "occupant": p.occupant})).collect::<Vec<_>>());
    let g = &v.guidance;
    o["nodes"] = json!(g
        .nodes
        .iter()
        .map(|n| json!({
            "id": n.id, "label": n.label, "t": n.t, "dv": [n.dv_rtn.x, n.dv_rtn.y, n.dv_rtn.z], "dv_total": n.dv_rtn.norm(),
            "executing": n.executing, "done": n.done, "remaining": n.remaining, "rcs": n.rcs,
            "burn": jaci_sim::guidance::burn_duration(v, n.dv_rtn.norm(), n.rcs),
        }))
        .collect::<Vec<_>>());
    o["guidance"] = json!({
        "program": format!("{:?}", g.program).to_lowercase(),
        "closed_loop": g.closed_loop,
        "t_go": g.t_go,
        "bank": g.bank_deg,
        "landing_target": g.landing_target,
        "dock_target": g.dock_target,
        "inserted": g.inserted,
        "ballistic": g.ballistic,
        "auto_stage": g.auto_stage,
        "auto_chutes": g.auto_chutes,
        "kick_wait": g.kick_wait,
        "ascent": g.ascent.as_ref().map(|p| json!({"pe": p.target_pe_km, "ap": p.target_ap_km, "inc": p.inclination_deg})),
        "azimuth": g.azimuth,
    });
    o["fts"] = json!({"installed": v.fts.installed, "armed": v.fts.armed, "autonomous": v.fts.autonomous});
    o["aborts"] = json!(w.abort_modes(v.id));
    o["iip"] = w.iip.iter().find(|(id, _)| *id == v.id).map_or(Value::Null, |(_, i)| json!(i));
    o["safe_mode"] = json!(v.safe_mode);
    o["faults"] = json!(v.faults);
    o["site"] = json!(v.site);
    if let Some(sys) = &v.systems {
        o["systems"] = json!({
            "loads": sys.power.loads,
            "solar": sys.power.solar.as_ref().map(|s| json!({"deployed": s.deployed, "tracking": s.tracking, "stuck": s.stuck, "degraded": s.degraded, "area": s.area})),
            "fuel_cell": sys.power.fuel_cell.as_ref().map(|f| json!({"on": f.on, "reactant": f.reactant_kg, "max": f.max_w})),
            "battery_wh": sys.power.capacity_wh(),
            "brownout": sys.power.brownout,
            "thermal": sys.thermal.nodes.iter().map(|n| json!({"id": n.id, "name": n.name, "temp": n.celsius(), "heater": n.heater_on, "heater_w": n.heater_w, "auto": n.auto, "failed": n.heater_failed, "min": n.min_c, "max": n.max_c})).collect::<Vec<_>>(),
            "radiator": sys.thermal.radiator_on,
            "has_radiator": sys.thermal.radiator_w_per_k > 0.0,
            "adcs": {
                "mode": sys.adcs.mode.id(),
                "mode_name": sys.adcs.mode.name(),
                "target": if let jaci_sim::subsystems::AttMode::Target(t) = sys.adcs.mode { json!(t) } else { Value::Null },
                "wheels": sys.adcs.wheels.as_ref().map(|wh| json!({"h": v3(wh.h), "max": wh.h_max, "failed": wh.failed})),
                "rcs_prop": sys.adcs.rcs_prop,
                "desat": sys.adcs.desaturating,
                "star_tracker": sys.adcs.star_tracker,
                "rate": v.omega.norm().to_degrees(),
            },
            "comms": {
                "tx_on": sys.comms.tx_on,
                "auto_rate": sys.comms.auto_rate,
                "rate": sys.comms.rate,
                "rates": sys.comms.rates,
                "dumping": sys.comms.dumping,
                "relay": sys.comms.relay,
                "hga": sys.comms.hga_gain_dbi > 0.0,
                "hga_deployed": sys.comms.hga_deployed,
                "stored": sys.comms.stored_mb,
                "storage": sys.comms.storage_mb,
                "gen": sys.comms.data_gen_mbps,
                "blackout": sys.comms.blackout,
                "failed": sys.comms.failed,
                "link": sys.comms.link,
            },
            "eclss": sys.eclss.as_ref().map(|e| json!({
                "crew": e.crew,
                "modules": e.modules.iter().map(|m| json!({"name": m.name, "volume": m.volume, "isolated": m.isolated, "leak": m.leak_area > 0.0, "pressure": m.pressure_kpa})).collect::<Vec<_>>(),
                "o2_gen": e.o2_gen_on, "scrubber": e.scrubber_on, "auto_press": e.auto_press,
                "has_o2_gen": e.o2_gen_rate > 0.0,
                "lioh": e.scrubber_capacity < jaci_sim::subsystems::UNLIMITED,
            })),
            "instruments": sys.instruments,
        });
    }
    o
}

/// Satélite real (TLE + SGP4) no formato dos veículos.
fn tracked(s: &jaci_sim::tracked::Tracked, t: f64) -> Option<Value> {
    let Some(st) = s.state(t) else {
        return Some(json!({
            "id": s.id, "name": s.name, "kind": "tracked", "kind_name": "Satélite real (TLE)",
            "phase": "destroyed", "phase_name": "SGP4 sem solução (reentrou ou TLE velho demais)",
            "central": "earth", "lat": 0, "lon": 0, "alt": 0, "speed": 0, "crew": 0,
            "tle": {"norad": s.norad, "line1": s.line1, "line2": s.line2, "epoch": s.epoch(), "age": s.age_days(t)},
        }));
    };
    let el = &st.elements;
    let re = Body::Earth.radius();
    let tm: serde_json::Map<String, Value> = s.telemetry(&st, t).into_iter().map(|(k, x)| (k, num(x))).collect();
    Some(json!({
        "id": s.id, "name": s.name, "kind": "tracked", "kind_name": "Satélite real (TLE)",
        "phase": "orbit", "phase_name": "Em órbita (SGP4)",
        "central": "earth", "lat": st.lat, "lon": st.lon, "alt": st.alt / 1e3, "speed": st.v.norm(),
        "r": v3(st.r), "v": v3(st.v),
        "pe": num((el.periapsis() - re) / 1e3), "ap": num((el.apoapsis() - re) / 1e3), "inc": el.i.to_degrees(), "ecc": el.e,
        "crew": 0, "tm": tm,
        "tle": {"norad": s.norad, "line1": s.line1, "line2": s.line2, "epoch": s.epoch(), "age": s.age_days(t)},
    }))
}

pub fn snapshot(a: &mut App) -> String {
    let v = snapshot_value(a, a.sent_events);
    a.sent_events = a.world.event_seq;
    v.to_string()
}

/// O instantâneo, com os eventos depois de `since` (contador do mundo).
pub fn snapshot_value(a: &App, since: u64) -> Value {
    let w = &a.world;
    let new = (w.event_seq.saturating_sub(since)).min(w.events.len() as u64) as usize;
    let events: Vec<Value> = w.events[w.events.len() - new..].iter().map(event).collect();
    let lc = &w.launch;
    let launch = json!({
        "vehicle": lc.vehicle,
        "state": match lc.state { CountState::Idle => "idle", CountState::Counting => "counting", CountState::Hold => "hold", CountState::Scrubbed => "scrubbed", CountState::Launched => "launched" },
        "t_minus": lc.t_minus,
        "clock": format_met(-lc.t_minus),
        "hold_reason": lc.hold_reason,
        "sequence": lc.sequence,
        "positions": lc.positions.iter().map(|p| json!({"id": p.id, "name": p.name, "go": p.go(), "manual": p.manual, "reason": p.reason, "polled": p.polled, "awaiting": p.awaiting})).collect::<Vec<_>>(),
        "criteria": lc.criteria,
        "window": lc.window,
        "loading": lc.loading,
        "all_go": lc.all_go(),
        "range_fouled": lc.range_fouled > 0.0,
        "auto_sequence": lc.auto_sequence,
    });
    // Planetas e Lua (heliocêntrico, e a Lua relativa à Terra) para as vistas.
    let bodies: Vec<Value> = Body::ALL
        .iter()
        .map(|b| {
            let (r, _) = b.helio_state(w.t);
            let rel = if *b == Body::Moon { b.state(w.t).0 } else { r };
            json!({"id": b.id(), "r": v3(r), "rel": v3(rel)})
        })
        .collect();
    let sun = jaci_sim::bodies::sun_direction(Body::Earth, jaci_sim::math::Vec3::ZERO, w.t).0;
    let ext: Vec<Value> = w
        .externals
        .iter()
        .map(|e| {
            let tm: serde_json::Map<String, Value> = e.tm.iter().map(|(k, x)| (k.clone(), num(*x))).collect();
            let get = |k: &str| e.tm.iter().find(|(id, _)| id == k).map_or(0.0, |x| x.1);
            json!({
                "id": e.id, "name": e.name, "kind": "external", "kind_name": "Veículo externo (CCSDS)",
                "phase": "external", "phase_name": if w.t - e.last < 5.0 { "Recebendo" } else { "Sem sinal" },
                "central": "earth", "lat": get("nav.lat"), "lon": get("nav.lon"), "alt": get("nav.alt"),
                "speed": get("nav.speed"), "tm": tm, "packets": e.packets, "age": w.t - e.last, "crew": 0,
            })
        })
        .collect();
    let mut vehicles: Vec<Value> = w.vehicles.iter().map(|v| vehicle(w, v)).collect();
    vehicles.extend(ext);
    vehicles.extend(w.tracked.iter().filter_map(|s| tracked(s, w.t)));
    let ksp = a.ksp.as_ref().map(|l| {
        vehicles.extend(l.vessels.iter().map(|v| crate::ksp::vehicle_json(v, w.t)));
        json!({
            "dir": l.ksp_dir.display().to_string(),
            "scripts": l.dir.display().to_string(),
            "kos": l.kos_installed,
            "module_manager": l.mm_installed,
            "patch": l.patch_installed,
            "vessels": l.vessels.len(),
            "live": l.vessels.iter().filter(|v| v.age() < 5.0).count(),
        })
    });
    json!({
        "type": "snapshot",
        "t": w.t,
        "utc": format_utc(w.t),
        "gmst": jaci_sim::time::gmst(w.t),
        "warp": w.warp,
        "paused": w.paused,
        "warp_stop": w.warp_stop,
        "third_body": w.third_body,
        "scenario": w.scenario,
        "mission": w.mission,
        "vehicles": vehicles,
        "launch": launch,
        "weather": w.weather,
        "alarms": w.alarms,
        "commands": w.commands.iter().rev().take(80).collect::<Vec<_>>(),
        "events": events,
        "corridor": w.corridor,
        "bodies": bodies,
        "sun": v3(sun),
        "stations_live": w.stations.iter().map(|s| json!({"id": s.id, "enabled": s.enabled})).collect::<Vec<_>>(),
        "ksp": ksp,
        "recording": a.recorder.as_ref().map(|r| r.status()),
        "room": room(a),
        "scripts": a.scripts.status(),
    })
}

/// A sala: ligada ou não, e quem está em cada posição.
fn room(a: &App) -> Value {
    let mut members: Vec<&crate::room::Member> = a.members.values().collect();
    members.sort_by(|x, y| (&x.position, &x.name).cmp(&(&y.position, &y.name)));
    json!({"on": a.room, "members": members.iter().map(|m| json!({"name": m.name, "position": m.position})).collect::<Vec<_>>()})
}

/// Histórico de telemetria de um veículo: os nomes uma vez, os valores em f32.
#[derive(Default)]
pub struct Hist {
    pub keys: Vec<String>,
    pub rows: std::collections::VecDeque<(f64, Vec<f32>)>,
}

const HIST_LEN: usize = 3_600;

/// Guarda uma amostra da telemetria de cada veículo (até 30 min de relógio).
pub fn record_history(a: &mut App) {
    let t = a.world.t;
    let sim = a.world.vehicles.iter().filter(|v| v.kind != Kind::Debris && v.phase != jaci_sim::vehicle::Phase::Destroyed).map(|v| (v.id, telemetry::values(v, t)));
    // Naves do KSP só enquanto chega telemetria (o gráfico não repete o último valor).
    let game = a.ksp.iter().flat_map(|l| &l.vessels).filter(|v| v.age() < 5.0).map(|v| (v.id, v.tm.clone()));
    let real = a.world.tracked.iter().filter_map(|s| s.state(t).map(|st| (s.id, s.telemetry(&st, t))));
    let all: Vec<(u32, Vec<(String, f64)>)> = sim.chain(game).chain(real).collect();
    for (id, vals) in all {
        let h = a.history.entry(id).or_default();
        if h.rows.back().is_some_and(|(last, _)| *last == t) {
            continue;
        }
        let mut row = vec![f32::NAN; h.keys.len()];
        for (k, x) in vals {
            let i = match h.keys.iter().position(|kk| *kk == k) {
                Some(i) => i,
                None => {
                    h.keys.push(k);
                    row.push(f32::NAN);
                    h.keys.len() - 1
                }
            };
            row[i] = x as f32;
        }
        h.rows.push_back((t, row));
        while h.rows.len() > HIST_LEN {
            h.rows.pop_front();
        }
    }
}

/// O histórico recente de um veículo em CSV.
pub fn csv(a: &App, id: u32) -> Option<String> {
    let h = a.history.get(&id)?;
    Some(crate::recording::to_csv(&h.keys, h.rows.iter().map(|(t, r)| (*t, r.as_slice())), &dictionary_map(a), id))
}

/// O histórico em colunas: por veículo, os instantes e cada ponto.
pub fn history(a: &App) -> String {
    let mut out = serde_json::Map::new();
    for (id, h) in &a.history {
        let ts: Vec<f64> = h.rows.iter().map(|(t, _)| *t).collect();
        let mut cols = serde_json::Map::new();
        for (k, key) in h.keys.iter().enumerate() {
            let col: Vec<Value> = h.rows.iter().map(|(_, r)| r.get(k).filter(|x| x.is_finite()).map_or(Value::Null, |x| json!(x))).collect();
            cols.insert(key.clone(), Value::Array(col));
        }
        out.insert(id.to_string(), json!({"t": ts, "points": cols}));
    }
    json!({"type": "history", "vehicles": out}).to_string()
}
