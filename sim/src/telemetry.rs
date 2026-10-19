//! Telemetria: o dicionário de pontos de cada veículo (nome, unidade, grupo e
//! limites) e os valores do instante.

use crate::bodies::Body;
use crate::subsystems::AttMode;
use crate::vehicle::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize)]
pub struct TmDef {
    pub id: String,
    pub name: String,
    pub unit: &'static str,
    pub group: &'static str,
    /// Faixa amarela e vermelha (mín, máx); fora delas, alarme.
    pub yellow: Option<(f64, f64)>,
    pub red: Option<(f64, f64)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LimitState {
    Nominal,
    Yellow,
    Red,
}

impl TmDef {
    pub fn check(&self, v: f64) -> LimitState {
        if !v.is_finite() {
            return LimitState::Nominal;
        }
        if let Some((lo, hi)) = self.red {
            if v < lo || v > hi {
                return LimitState::Red;
            }
        }
        if let Some((lo, hi)) = self.yellow {
            if v < lo || v > hi {
                return LimitState::Yellow;
            }
        }
        LimitState::Nominal
    }
}

fn def(id: &str, name: &str, unit: &'static str, group: &'static str) -> TmDef {
    TmDef { id: id.into(), name: name.into(), unit, group, yellow: None, red: None }
}

fn lim(mut d: TmDef, yellow: (f64, f64), red: (f64, f64)) -> TmDef {
    d.yellow = Some(yellow);
    d.red = Some(red);
    d
}

/// Dicionário de telemetria do veículo.
pub fn defs(v: &Vehicle) -> Vec<TmDef> {
    let mut d = vec![
        def("nav.met", "Tempo de missão", "s", "Navegação"),
        def("nav.alt", "Altitude", "km", "Navegação"),
        def("nav.lat", "Latitude", "°", "Navegação"),
        def("nav.lon", "Longitude", "°", "Navegação"),
        def("nav.speed", "Velocidade inercial", "m/s", "Navegação"),
        def("nav.vspeed", "Velocidade vertical", "m/s", "Navegação"),
        def("nav.hspeed", "Velocidade horizontal (solo)", "m/s", "Navegação"),
        def("nav.mach", "Mach", "", "Navegação"),
        def("nav.q", "Pressão dinâmica", "kPa", "Navegação"),
        {
            let (y, r) = match v.kind {
                _ if v.crew() > 0 => (4.5, 6.5),
                Kind::SoundingRocket => (15.0, 25.0),
                _ => (7.0, 12.0),
            };
            lim(def("nav.accel", "Aceleração", "g", "Navegação"), (-1.0, y), (-1.0, r))
        },
        def("nav.pitch", "Arfagem (sobre o horizonte)", "°", "Navegação"),
        def("nav.heading", "Rumo", "°", "Navegação"),
        def("nav.aoa", "Ângulo de ataque", "°", "Navegação"),
        def("nav.downrange", "Distância percorrida", "km", "Navegação"),
        def("nav.mass", "Massa", "t", "Navegação"),
        def("nav.dv", "Δv restante", "m/s", "Navegação"),
        def("nav.twr", "Empuxo/peso", "", "Navegação"),
        def("orb.ap", "Apoastro", "km", "Órbita"),
        def("orb.pe", "Periastro", "km", "Órbita"),
        def("orb.inc", "Inclinação", "°", "Órbita"),
        def("orb.ecc", "Excentricidade", "", "Órbita"),
        def("orb.period", "Período", "min", "Órbita"),
        def("orb.raan", "Nodo ascendente", "°", "Órbita"),
        def("orb.argp", "Argumento do periastro", "°", "Órbita"),
        def("orb.ta", "Anomalia verdadeira", "°", "Órbita"),
        def("orb.sma", "Semieixo maior", "km", "Órbita"),
    ];
    for (i, s) in v.stages.iter().enumerate() {
        let n = i + 1;
        let mut pd = def(&format!("prop.s{n}.prop"), &format!("{}: propelente", s.name), "%", "Propulsão");
        if s.capacity > 0.0 {
            pd.yellow = Some((5.0, 101.0));
        }
        d.push(pd);
        if s.tank_bar > 0.0 && s.capacity > 0.0 {
            d.push(lim(
                def(&format!("prop.s{n}.tank"), &format!("{}: pressão do tanque", s.name), "bar", "Propulsão"),
                (s.tank_bar * 0.85, s.tank_bar * 1.15),
                (s.tank_bar * 0.7, s.tank_bar * 1.3),
            ));
        }
        for (j, e) in s.engines.iter().enumerate() {
            let m = j + 1;
            d.push(def(&format!("prop.s{n}.e{m}.thrust"), &format!("{} {}: empuxo", s.name, e.name), "kN", "Propulsão"));
            d.push(def(&format!("prop.s{n}.e{m}.pc"), &format!("{} {}: pressão de câmara", s.name, e.name), "bar", "Propulsão"));
            d.push(lim(
                def(&format!("prop.s{n}.e{m}.state"), &format!("{} {}: estado", s.name, e.name), "", "Propulsão"),
                (-1.0, 3.5),
                (-1.0, 3.5),
            ));
        }
    }
    if let Some(sys) = &v.systems {
        d.push(lim(def("eps.soc", "Carga da bateria", "%", "Energia"), (30.0, 101.0), (15.0, 101.0)));
        d.push(def("eps.gen", "Geração", "W", "Energia"));
        d.push(def("eps.load", "Consumo", "W", "Energia"));
        d.push(lim(def("eps.bus", "Tensão do barramento", "V", "Energia"), (25.5, 33.0), (24.0, 35.0)));
        d.push(def("eps.battery", "Potência da bateria", "W", "Energia"));
        d.push(def("eps.sun_angle", "Ângulo do Sol nos painéis", "°", "Energia"));
        d.push(def("eps.sunlit", "Iluminado", "", "Energia"));
        if sys.power.fuel_cell.is_some() {
            d.push(lim(def("eps.reactant", "Reagentes da célula a combustível", "kg", "Energia"), (300.0, 1e9), (100.0, 1e9)));
        }
        for n in &sys.thermal.nodes {
            d.push(lim(
                def(&format!("tcs.{}", n.id), &format!("Temperatura: {}", n.name), "°C", "Térmico"),
                (n.min_c + 5.0, n.max_c - 5.0),
                (n.min_c, n.max_c),
            ));
            if n.heater_w > 0.0 {
                d.push(def(&format!("tcs.{}.heater", n.id), &format!("Aquecedor: {}", n.name), "", "Térmico"));
            }
        }
        d.push(def("adcs.mode", "Modo de atitude", "", "Atitude"));
        d.push(lim(def("adcs.error", "Erro de apontamento", "°", "Atitude"), (-1.0, 5.0), (-1.0, 30.0)));
        d.push(lim(def("adcs.rate", "Velocidade angular", "°/s", "Atitude"), (-1.0, 1.5), (-1.0, 5.0)));
        if sys.adcs.wheels.is_some() {
            d.push(lim(def("adcs.wheels", "Saturação das rodas", "%", "Atitude"), (-1.0, 80.0), (-1.0, 98.0)));
        }
        if sys.adcs.rcs_torque > 0.0 {
            d.push(lim(def("adcs.rcs_prop", "Propelente do RCS", "kg", "Atitude"), (sys.adcs.rcs_prop * 0.1, 1e9), (sys.adcs.rcs_prop * 0.02, 1e9)));
        }
        d.push(def("com.link", "Enlace", "", "Comunicações"));
        d.push(def("com.margin", "Margem do enlace", "dB", "Comunicações"));
        d.push(def("com.rate", "Taxa de dados", "kbit/s", "Comunicações"));
        d.push(def("com.elevation", "Elevação na estação", "°", "Comunicações"));
        d.push(def("com.range", "Distância à estação", "km", "Comunicações"));
        d.push(def("com.owlt", "Atraso da luz (ida)", "s", "Comunicações"));
        d.push(lim(def("com.stored", "Gravador ocupado", "%", "Comunicações"), (-1.0, 85.0), (-1.0, 98.0)));
        if let Some(e) = &sys.eclss {
            d.push(lim(def("ecl.pressure", "Pressão da cabine", "kPa", "Suporte de vida"), (95.0, 104.0), (80.0, 110.0)));
            d.push(lim(def("ecl.ppo2", "Pressão parcial de O₂", "kPa", "Suporte de vida"), (19.5, 23.5), (16.0, 26.0)));
            d.push(lim(def("ecl.ppco2", "Pressão parcial de CO₂", "kPa", "Suporte de vida"), (-1.0, 0.53), (-1.0, 1.0)));
            d.push(def("ecl.temp", "Temperatura da cabine", "°C", "Suporte de vida"));
            d.push(lim(def("ecl.o2_tank", "O₂ de reserva", "kg", "Suporte de vida"), (e.o2_tank_kg * 0.2, 1e9), (e.o2_tank_kg * 0.05, 1e9)));
            d.push(def("ecl.n2_tank", "N₂ de reserva", "kg", "Suporte de vida"));
            d.push(def("ecl.water", "Água", "kg", "Suporte de vida"));
            d.push(def("ecl.crew", "Tripulação", "", "Suporte de vida"));
            d.push(lim(def("ecl.hr", "Frequência cardíaca média", "bpm", "Suporte de vida"), (45.0, 130.0), (35.0, 160.0)));
            if e.scrubber_capacity.is_finite() {
                d.push(lim(def("ecl.scrubber", "Capacidade dos cartuchos de LiOH", "kg CO₂", "Suporte de vida"), (2.0, 1e9), (0.5, 1e9)));
            }
        }
        for i in &sys.instruments {
            d.push(def(&format!("pl.{}", i.id), &i.name, "", "Carga útil"));
        }
    }
    if let Some(hs) = &v.heat_shield {
        d.push(def("ent.flux", "Fluxo de calor no escudo", "kW/m²", "Reentrada"));
        d.push(lim(def("ent.temp", "Temperatura do escudo", "K", "Reentrada"), (-1.0, 2_300.0), (-1.0, 2_900.0)));
        d.push(lim(def("ent.ablator", "Ablativo restante", "%", "Reentrada"), (30.0, 101.0), (10.0, 101.0)));
        d.push(def("ent.bank", "Ângulo de rolamento", "°", "Reentrada"));
        let _ = hs;
    }
    for (i, c) in v.chutes.iter().enumerate() {
        d.push(def(&format!("ent.chute{}", i + 1), &c.name, "", "Reentrada"));
    }
    if v.fts.installed {
        d.push(def("rso.fts_armed", "Terminação de voo armada", "", "Segurança de voo"));
    }
    d
}

/// Valores do instante, alinhados por id.
pub fn values(v: &Vehicle, t: f64) -> Vec<(String, f64)> {
    let dd = &v.derived;
    let re = v.central.radius();
    let el = &dd.elements;
    let mut out: Vec<(String, f64)> = vec![
        ("nav.met".into(), v.launch_time.map_or(0.0, |l| t - l)),
        ("nav.alt".into(), dd.alt / 1e3),
        ("nav.lat".into(), dd.lat),
        ("nav.lon".into(), dd.lon),
        ("nav.speed".into(), dd.speed),
        ("nav.vspeed".into(), dd.vspeed),
        ("nav.hspeed".into(), dd.hspeed),
        ("nav.mach".into(), dd.mach),
        ("nav.q".into(), dd.q / 1e3),
        ("nav.accel".into(), dd.accel_g),
        ("nav.pitch".into(), dd.pitch_deg),
        ("nav.heading".into(), dd.heading_deg),
        ("nav.aoa".into(), dd.aoa_deg),
        ("nav.downrange".into(), dd.downrange_km),
        ("nav.mass".into(), dd.mass / 1e3),
        ("nav.dv".into(), dd.dv_remaining),
        ("nav.twr".into(), dd.twr),
        ("orb.ap".into(), if el.e < 1.0 { (el.apoapsis() - re) / 1e3 } else { f64::INFINITY }),
        ("orb.pe".into(), (el.periapsis() - re) / 1e3),
        ("orb.inc".into(), el.i.to_degrees()),
        ("orb.ecc".into(), el.e),
        ("orb.period".into(), el.period(v.central.mu()) / 60.0),
        ("orb.raan".into(), el.raan.to_degrees()),
        ("orb.argp".into(), el.argp.to_degrees()),
        ("orb.ta".into(), el.nu.to_degrees()),
        ("orb.sma".into(), el.a / 1e3),
    ];
    for (i, s) in v.stages.iter().enumerate() {
        let n = i + 1;
        out.push((format!("prop.s{n}.prop"), s.prop_fraction() * 100.0));
        if s.tank_bar > 0.0 && s.capacity > 0.0 {
            // Tanque pressurizado: cai quando vaza; sobe um pouco com o motor aceso.
            let p = if s.leak > 0.0 { s.tank_bar * (0.6 + 0.4 * s.prop_fraction()) } else { s.tank_bar * if s.any_lit() { 1.02 } else { 1.0 } };
            out.push((format!("prop.s{n}.tank"), p));
        }
        for (j, e) in s.engines.iter().enumerate() {
            let m = j + 1;
            out.push((format!("prop.s{n}.e{m}.thrust"), e.thrust / 1e3));
            out.push((format!("prop.s{n}.e{m}.pc"), e.pc()));
            let st = match e.state {
                EngineState::Off => 0.0,
                EngineState::Starting => 1.0,
                EngineState::Running => 2.0,
                EngineState::Stopping => 3.0,
                EngineState::Failed => 4.0,
            };
            out.push((format!("prop.s{n}.e{m}.state"), st));
        }
    }
    if let Some(sys) = &v.systems {
        let p = &sys.power;
        out.push(("eps.soc".into(), p.soc * 100.0));
        out.push(("eps.gen".into(), p.gen_w));
        out.push(("eps.load".into(), p.load_w));
        out.push(("eps.bus".into(), p.bus_v));
        out.push(("eps.battery".into(), p.battery_w));
        out.push(("eps.sun_angle".into(), p.sun_angle));
        out.push(("eps.sunlit".into(), dd.sunlit));
        if let Some(fc) = &p.fuel_cell {
            out.push(("eps.reactant".into(), fc.reactant_kg));
        }
        for n in &sys.thermal.nodes {
            out.push((format!("tcs.{}", n.id), n.celsius()));
            if n.heater_w > 0.0 {
                out.push((format!("tcs.{}.heater", n.id), if n.heater_on { 1.0 } else { 0.0 }));
            }
        }
        let a = &sys.adcs;
        out.push(("adcs.mode".into(), att_mode_code(a.mode)));
        out.push(("adcs.error".into(), a.error_deg));
        out.push(("adcs.rate".into(), v.omega.norm().to_degrees()));
        if a.wheels.is_some() {
            out.push(("adcs.wheels".into(), a.wheel_saturation() * 100.0));
        }
        if a.rcs_torque > 0.0 {
            out.push(("adcs.rcs_prop".into(), a.rcs_prop));
        }
        let c = &sys.comms;
        match &c.link {
            Some(l) => {
                out.push(("com.link".into(), 1.0));
                out.push(("com.margin".into(), l.margin_db));
                out.push(("com.rate".into(), l.rate_bps / 1e3));
                out.push(("com.elevation".into(), l.elevation_deg));
                out.push(("com.range".into(), l.range_m / 1e3));
                out.push(("com.owlt".into(), l.owlt));
            }
            None => {
                out.push(("com.link".into(), 0.0));
                for k in ["com.margin", "com.rate", "com.elevation", "com.range", "com.owlt"] {
                    out.push((k.into(), f64::NAN));
                }
            }
        }
        out.push(("com.stored".into(), c.stored_mb / c.storage_mb.max(1e-9) * 100.0));
        if let Some(e) = &sys.eclss {
            out.push(("ecl.pressure".into(), e.pressure()));
            out.push(("ecl.ppo2".into(), e.ppo2()));
            out.push(("ecl.ppco2".into(), e.ppco2()));
            out.push(("ecl.temp".into(), e.temp_c));
            out.push(("ecl.o2_tank".into(), e.o2_tank_kg));
            out.push(("ecl.n2_tank".into(), e.n2_tank_kg));
            out.push(("ecl.water".into(), e.water_kg));
            out.push(("ecl.crew".into(), e.crew as f64));
            out.push(("ecl.hr".into(), e.heart_rate));
            if e.scrubber_capacity.is_finite() {
                out.push(("ecl.scrubber".into(), e.scrubber_capacity));
            }
        }
        for i in &sys.instruments {
            out.push((format!("pl.{}", i.id), if i.on { 1.0 } else { 0.0 }));
        }
    }
    if let Some(hs) = &v.heat_shield {
        out.push(("ent.flux".into(), hs.flux / 1e3));
        out.push(("ent.temp".into(), hs.temp));
        out.push(("ent.ablator".into(), hs.ablator_kg / hs.initial_kg.max(1e-9) * 100.0));
        out.push(("ent.bank".into(), v.guidance.bank_deg));
    }
    for (i, c) in v.chutes.iter().enumerate() {
        let s = match c.state {
            ChuteState::Stowed => 0.0,
            ChuteState::Deployed => 1.0,
            ChuteState::Cut => 2.0,
            ChuteState::Failed => 3.0,
        };
        out.push((format!("ent.chute{}", i + 1), s));
    }
    if v.fts.installed {
        out.push(("rso.fts_armed".into(), if v.fts.armed { 1.0 } else { 0.0 }));
    }
    let _ = Body::Earth;
    out
}

pub fn att_mode_code(m: AttMode) -> f64 {
    match m {
        AttMode::Free => 0.0,
        AttMode::Detumble => 1.0,
        AttMode::Hold => 2.0,
        AttMode::Prograde => 3.0,
        AttMode::Retrograde => 4.0,
        AttMode::Normal => 5.0,
        AttMode::AntiNormal => 6.0,
        AttMode::RadialOut => 7.0,
        AttMode::RadialIn => 8.0,
        AttMode::SunPoint => 9.0,
        AttMode::Nadir => 10.0,
        AttMode::EarthPoint => 11.0,
        AttMode::Target(_) => 12.0,
        AttMode::Inertial(_) => 13.0,
        AttMode::Guidance => 14.0,
    }
}
