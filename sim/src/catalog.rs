//! Catálogo de veículos e locais de lançamento. Os números são aproximações
//! de dados públicos dos veículos em que cada um se inspira: servem para voar
//! de forma plausível, não para projeto.

use crate::aero::Shape;
use crate::bodies::Body;
use crate::math::{Quat, Vec3};
use crate::subsystems::*;
use crate::vehicle::*;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub kind: &'static str,
    pub description: &'static str,
}

pub const ENTRIES: &[CatalogEntry] = &[
    CatalogEntry { id: "vsb30", name: "VSB-30", kind: "sounding", description: "Foguete de sondagem de dois estágios sólidos (IAE/DLR): ~6 min de microgravidade, apogeu ~250 km, carga recuperada no mar" },
    CatalogEntry { id: "vlm1", name: "VLM-1", kind: "satellite", description: "Microlançador de três estágios sólidos com satélite de 150 kg" },
    CatalogEntry { id: "medio_sso", name: "Lançador médio + satélite de observação", kind: "satellite", description: "Dois estágios querosene/LOX (tipo Falcon 9) com um satélite tipo Amazonia-1 para órbita heliossíncrona" },
    CatalogEntry { id: "medio_geo", name: "Lançador médio + satélite geoestacionário", kind: "satellite", description: "Inserção em órbita de transferência (GTO) e motor de apogeu do satélite (tipo SGDC)" },
    CatalogEntry { id: "medio_capsula", name: "Lançador médio + cápsula tripulada", kind: "capsule", description: "Cápsula para 4 tripulantes com motores de aborto, escudo térmico e paraquedas (tipo Crew Dragon)" },
    CatalogEntry { id: "soyuz", name: "Lançador com boosters + nave tripulada", kind: "capsule", description: "Quatro boosters laterais, núcleo e terceiro estágio, nave com módulo de serviço e torre de escape (tipo Soyuz)" },
    CatalogEntry { id: "sts", name: "Ônibus espacial", kind: "shuttle", description: "Dois foguetes sólidos, tanque externo, orbitador alado com OMS que pousa em pista (tipo STS)" },
    CatalogEntry { id: "estacao", name: "Estação espacial", kind: "station", description: "Estação modular de 420 t a 420 km, 7 tripulantes, giroscópios de controle e portas de acoplamento (tipo ISS)" },
    CatalogEntry { id: "amazonia", name: "Satélite de observação", kind: "satellite", description: "Satélite de 640 kg com câmera, rodas de reação e bobinas magnéticas, já em órbita" },
    CatalogEntry { id: "sonda_marte", name: "Sonda marciana + estágio de escape", kind: "probe", description: "Estágio superior hidrogênio/LOX em órbita de estacionamento e sonda de 2,2 t (tipo MRO)" },
    CatalogEntry { id: "sonda_lua", name: "Sonda lunar + estágio de escape", kind: "probe", description: "Estágio superior em órbita de estacionamento e sonda de 1,9 t (tipo LRO)" },
    CatalogEntry { id: "modulo_lunar", name: "Módulo lunar", kind: "capsule", description: "Estágio de descida com motor de 45 kN que regula até 10 % e estágio de subida, para 2 tripulantes (tipo LM do Apollo)" },
    CatalogEntry { id: "sonda_pouso_marte", name: "Sonda de pouso em Marte", kind: "probe", description: "Escudo térmico, paraquedas supersônico e descida propulsada até as pernas (tipo Phoenix/InSight)" },
];

/// Locais de lançamento conhecidos.
pub fn sites() -> Vec<Site> {
    let s = |name: &str, lat: f64, lon: f64, alt: f64| Site { name: name.into(), lat, lon, alt };
    vec![
        s("Alcântara (CLA)", -2.3730, -44.3964, 40.0),
        s("Barreira do Inferno (CLBI)", -5.9196, -35.1638, 30.0),
        s("Cabo Canaveral LC-39A", 28.6082, -80.6041, 15.0),
        s("Kourou ELA-3", 5.2390, -52.7680, 15.0),
        s("Baikonur Sítio 31", 45.9960, 63.5640, 100.0),
        s("Vandenberg SLC-4E", 34.6320, -120.6110, 150.0),
        s("Wenchang", 19.6145, 110.9510, 20.0),
        s("Sriharikota", 13.7199, 80.2304, 10.0),
        s("Tanegashima", 30.4000, 130.9700, 30.0),
    ]
}

pub fn site(name_prefix: &str) -> Site {
    sites().into_iter().find(|s| s.name.starts_with(name_prefix)).unwrap_or_else(|| sites().remove(0))
}

// ───────────────────────── Peças ─────────────────────────

fn engine(name: &str, thrust_vac_kn: f64, isp_vac: f64, isp_sl: f64, min_throttle: f64, ignitions: u32, pc: f64) -> Engine {
    Engine {
        name: name.into(),
        thrust_vac: thrust_vac_kn * 1e3,
        isp_vac,
        isp_sl,
        min_throttle,
        solid: false,
        ignitions,
        pc_nominal: pc,
        state: EngineState::Off,
        spool: 0.0,
        throttle: 1.0,
        burn_time: 0.0,
        thrust: 0.0,
    }
}

fn solid(name: &str, thrust_vac_kn: f64, isp_vac: f64, isp_sl: f64, pc: f64) -> Engine {
    Engine { solid: true, ignitions: 1, ..engine(name, thrust_vac_kn, isp_vac, isp_sl, 1.0, 1, pc) }
}

fn stage(name: &str, dry: f64, prop: f64, engines: Vec<Engine>, diameter: f64, length: f64) -> Stage {
    Stage {
        name: name.into(),
        dry_mass: dry,
        propellant: prop,
        capacity: prop,
        engines,
        boosters: false,
        diameter,
        length,
        cryogenic: false,
        spacecraft: false,
        leak: 0.0,
        tank_bar: 3.5,
    }
}

fn load(id: &str, name: &str, w: f64, essential: bool) -> Load {
    Load { id: id.into(), name: name.into(), watts: w, on: true, essential }
}

fn node(id: &str, name: &str, cap: f64, area: f64, share: f64, cond: f64, heater: f64, lo: f64, hi: f64, min: f64, max: f64) -> ThermalNode {
    ThermalNode {
        id: id.into(),
        name: name.into(),
        temp: 293.15,
        capacity: cap,
        area,
        absorptivity: if area > 0.0 { 0.35 } else { 0.0 },
        emissivity: if area > 0.0 { 0.8 } else { 0.0 },
        load_share: share,
        conductance: cond,
        heater_w: heater,
        heater_on: false,
        auto: heater > 0.0,
        set_lo: lo,
        set_hi: hi,
        min_c: min,
        max_c: max,
        heater_failed: false,
    }
}

/// Térmico padrão em escala com o tamanho: estrutura, aviônica, bateria, tanques.
fn thermal(mass: f64, area: f64, crewed: bool) -> Thermal {
    let k = (mass / 1000.0).max(0.2);
    let mut nodes = vec![
        node("struct", "Estrutura", 900.0 * mass * 0.5, area, 0.1, 0.0, 0.0, 0.0, 0.0, -100.0, 120.0),
        node("avionics", "Aviônica", 900.0 * mass * 0.05, 0.0, 0.6, 3.0 * k, 8.0 * k, 0.0, 10.0, -20.0, 60.0),
        node("battery", "Bateria", 900.0 * mass * 0.03, 0.0, 0.1, 0.8 * k, 15.0 * k, 5.0, 15.0, 0.0, 40.0),
        node("tanks", "Tanques", 900.0 * mass * 0.1, 0.0, 0.0, 1.5 * k, 20.0 * k, 8.0, 18.0, 5.0, 50.0),
    ];
    if crewed {
        nodes.push(node("cabin", "Cabine", 900.0 * mass * 0.1, 0.0, 0.2, 6.0 * k, 150.0 * k, 19.0, 24.0, 16.0, 30.0));
    }
    // Radiadores para levar o calor das cargas embora.
    for n in &mut nodes {
        if n.id == "struct" {
            n.emissivity = 0.85;
        }
    }
    Thermal { nodes, radiator_w_per_k: 0.0, radiator_on: false }
}

fn power(solar_area: f64, eff: f64, tracking: bool, battery_wh: f64, loads: Vec<Load>) -> Power {
    Power {
        solar: (solar_area > 0.0).then(|| SolarArray {
            area: solar_area,
            efficiency: eff,
            deployed: true,
            tracking,
            normal: Vec3::Z * -1.0,
            degraded: 0.0,
            stuck: false,
        }),
        rtg_w: 0.0,
        fuel_cell: None,
        battery_wh,
        soc: 1.0,
        failed_cells: 0,
        loads,
        heater_w: 0.0,
        gen_w: 0.0,
        load_w: 0.0,
        bus_v: 28.0,
        battery_w: 0.0,
        sun_angle: 0.0,
        brownout: false,
        external: false,
    }
}

fn adcs(inertia: Vec3, wheels: Option<(f64, f64)>, rcs_torque: f64, rcs_prop: f64, rcs_force: f64, magnetorquers: bool) -> Adcs {
    Adcs {
        mode: AttMode::Hold,
        inertia,
        wheels: wheels.map(|(h, t)| Wheels { h: Vec3::ZERO, h_max: h, torque_max: t, failed: [false; 3] }),
        rcs_torque,
        rcs_prop,
        rcs_kg_per_nms: 0.0005,
        rcs_force,
        magnetorquers,
        max_rate: 1.0f64.to_radians(),
        disturbance: inertia.norm() * 2e-9 + 1e-5,
        gyro_bias: Vec3::ZERO,
        star_tracker: true,
        desaturating: false,
        error_deg: 0.0,
        torque_cmd: Vec3::ZERO,
    }
}

fn comms(tx_w: f64, freq_ghz: f64, lga: f64, hga: f64, beam: f64, rates_kbps: &[f64], storage_mb: f64, relay: bool) -> Comms {
    Comms {
        tx_on: true,
        tx_power_w: tx_w,
        freq_hz: freq_ghz * 1e9,
        lga_gain_dbi: lga,
        hga_gain_dbi: hga,
        hga_beam_deg: beam,
        hga_deployed: hga > 0.0,
        rates: rates_kbps.iter().map(|k| k * 1e3).collect(),
        auto_rate: true,
        rate: rates_kbps.first().copied().unwrap_or(1.0) * 1e3,
        storage_mb,
        stored_mb: 0.0,
        data_gen_mbps: 0.0,
        relay,
        failed: false,
        link: None,
        blackout: false,
        dumping: false,
    }
}

fn eclss(crew: u32, modules: &[(&str, f64)], o2: f64, n2: f64, scrubber: f64, capacity: f64, o2gen: f64, water: f64) -> Eclss {
    let mut e = Eclss {
        crew,
        modules: modules
            .iter()
            .map(|(n, v)| CabinModule { name: (*n).into(), volume: *v, isolated: false, leak_area: 0.0, pressure_kpa: 101.3 })
            .collect(),
        o2_kg: 0.0,
        n2_kg: 0.0,
        co2_kg: 0.0,
        water_kg: water,
        temp_c: 22.0,
        humidity: 0.45,
        o2_tank_kg: o2,
        n2_tank_kg: n2,
        o2_gen_rate: o2gen,
        o2_gen_on: o2gen > 0.0,
        scrubber_rate: scrubber,
        scrubber_on: true,
        scrubber_capacity: capacity,
        auto_press: true,
        fans_on: true,
        heart_rate: 70.0,
    };
    e.fill_standard();
    e
}

fn instrument(id: &str, name: &str, w: f64, mbps: f64, on: bool) -> Instrument {
    Instrument { id: id.into(), name: name.into(), watts: w, mbps, on }
}

fn base(kind: Kind, def_id: &str, name: &str, stages: Vec<Stage>, systems: Option<Systems>, shape: Shape) -> Vehicle {
    let n = stages.len();
    Vehicle {
        id: 0,
        name: name.into(),
        kind,
        def_id: def_id.into(),
        stages,
        initial_stages: n,
        items: vec![],
        shape,
        nose_radius: 1.0,
        heat_shield: None,
        chutes: vec![],
        landing: LandingMode::None,
        gear_down: false,
        les: None,
        ports: vec![],
        systems,
        fts: Fts::default(),
        central: Body::Earth,
        r: Vec3::new(Body::Earth.radius(), 0.0, 0.0),
        v: Vec3::ZERO,
        att: Quat::IDENTITY,
        omega: Vec3::ZERO,
        phase: Phase::Orbit,
        site: None,
        launch_time: None,
        released: false,
        guidance: Guidance::default(),
        attached_to: None,
        derived: Derived::default(),
        stats: Stats::default(),
        safe_mode: false,
        faults: vec![],
        parent: None,
        ground_power: false,
    }
}

fn spacecraft_stage(name: &str, dry: f64, prop: f64, engines: Vec<Engine>, diameter: f64) -> Stage {
    Stage { spacecraft: true, ..stage(name, dry, prop, engines, diameter, 3.0) }
}

// ───────────────────────── Naves ─────────────────────────

fn sat_observacao() -> (Stage, Systems) {
    let core = spacecraft_stage(
        "Satélite de observação",
        580.0,
        60.0,
        vec![engine("Propulsores de hidrazina 4×1 N", 0.004, 220.0, 200.0, 1.0, 500, 15.0)],
        2.5,
    );
    let sys = Systems {
        power: power(
            6.0,
            0.28,
            true,
            1_200.0,
            vec![
                load("obc", "Computador de bordo", 35.0, true),
                load("ttc", "Transponder TT&C", 20.0, true),
                load("adcs", "Eletrônica do ADCS", 40.0, true),
                load("gps", "Receptor GPS", 6.0, false),
            ],
        ),
        thermal: thermal(640.0, 9.0, false),
        adcs: adcs(Vec3::new(310.0, 360.0, 420.0), Some((12.0, 0.2)), 0.5, 0.0, 0.0, true),
        comms: comms(2.0, 2.2, 0.0, 6.0, 60.0, &[2.0, 32.0, 256.0, 100_000.0], 64_000.0, false),
        eclss: None,
        instruments: vec![
            instrument("wfi", "Câmera de campo largo (WFI)", 120.0, 160.0, false),
            instrument("dcs", "Coleta de dados ambientais", 15.0, 0.01, true),
        ],
    };
    (core, sys)
}

fn sat_geo() -> (Stage, Systems) {
    let core = spacecraft_stage(
        "Satélite geoestacionário",
        2_700.0,
        3_000.0,
        vec![engine("Motor de apogeu 400 N", 0.42, 318.0, 280.0, 1.0, 20, 9.0)],
        3.7,
    );
    let mut p = power(
        40.0,
        0.29,
        true,
        8_000.0,
        vec![
            load("obc", "Computador de bordo", 60.0, true),
            load("ttc", "Transponder TT&C", 30.0, true),
            load("adcs", "Eletrônica do ADCS", 80.0, true),
        ],
    );
    if let Some(sa) = &mut p.solar {
        sa.deployed = false;
    }
    let sys = Systems {
        power: p,
        thermal: thermal(5_700.0, 30.0, false),
        adcs: adcs(Vec3::new(9_000.0, 4_000.0, 10_000.0), Some((70.0, 0.3)), 20.0, 120.0, 40.0, false),
        comms: comms(20.0, 4.0, 0.0, 30.0, 4.0, &[1.0, 4.0, 64.0], 1_000.0, false),
        eclss: None,
        instruments: vec![instrument("ka", "Transponders banda Ka (carga útil)", 6_000.0, 0.0, false)],
    };
    (core, sys)
}

fn capsula() -> Vehicle {
    let core = spacecraft_stage(
        "Cápsula",
        9_000.0,
        2_400.0,
        vec![engine("Propulsores de manobra 8×400 N", 3.2, 300.0, 250.0, 1.0, 100, 10.0)],
        3.7,
    );
    let trunk = Stage { spacecraft: true, ..stage("Tronco", 1_300.0, 0.0, vec![], 3.7, 3.7) };
    let sys = Systems {
        power: power(
            12.0,
            0.3,
            false,
            15_000.0,
            vec![
                load("obc", "Computadores de voo", 400.0, true),
                load("ttc", "Comunicações", 150.0, true),
                load("adcs", "Controle de atitude", 200.0, true),
                load("eclss", "Suporte de vida", 900.0, true),
                load("displays", "Telas da cabine", 300.0, false),
                load("lights", "Iluminação", 120.0, false),
            ],
        ),
        thermal: thermal(12_000.0, 25.0, true),
        adcs: adcs(Vec3::new(15_000.0, 22_000.0, 22_000.0), None, 800.0, 350.0, 400.0, false),
        comms: comms(15.0, 2.2, 3.0, 20.0, 15.0, &[2.0, 64.0, 2_000.0], 32_000.0, true),
        eclss: Some(eclss(4, &[("Cabine", 9.3)], 70.0, 30.0, 4.5, UNLIMITED, 0.0, 120.0)),
        instruments: vec![],
    };
    let mut v = base(Kind::Capsule, "capsula", "Cápsula Jaci", vec![trunk, core], Some(sys), Shape::Capsule);
    v.nose_radius = 4.0;
    v.heat_shield = Some(HeatShield {
        ablator_kg: 420.0,
        initial_kg: 420.0,
        ablation_heat: 30e6,
        area: 10.8,
        temp: 293.0,
        flux: 0.0,
        peak_flux: 0.0,
        heat_load: 0.0,
    });
    v.chutes = vec![
        Chute { name: "Paraquedas drogue".into(), cda: 46.0, deploy_alt: 5_700.0, max_mach: 1.6, drogue: true, state: ChuteState::Stowed, inflation: 0.0 },
        Chute { name: "Paraquedas principais".into(), cda: 2_900.0, deploy_alt: 1_800.0, max_mach: 0.6, drogue: false, state: ChuteState::Stowed, inflation: 0.0 },
    ];
    v.landing = LandingMode::Splash;
    v.les = Some(Les { thrust: 8.0 * 71e3, burn: 6.0, tower_mass: 0.0, available: true, firing: 0.0 });
    v.guidance.landing_target = Some((29.3, -79.8));
    v
}

// ───────────────────────── Construção ─────────────────────────

/// Constrói um veículo do catálogo. `None` se o id não existe.
pub fn build(id: &str) -> Option<Vehicle> {
    Some(match id {
        "vsb30" => {
            let s1 = stage("S31 (1º estágio)", 284.0, 650.0, vec![solid("Motor S31", 108.0, 262.0, 240.0, 70.0)], 0.557, 3.3);
            let s2 = stage("S30 (2º estágio)", 370.0, 870.0, vec![solid("Motor S30", 80.0, 272.0, 248.0, 60.0)], 0.557, 3.4);
            let pl = Stage { spacecraft: true, ..stage("Carga útil", 400.0, 0.0, vec![], 0.438, 3.0) };
            let sys = Systems {
                power: power(0.0, 0.0, false, 500.0, vec![load("obc", "Sistema de serviço", 80.0, true), load("tm", "Telemetria", 40.0, true)]),
                thermal: thermal(400.0, 4.0, false),
                adcs: adcs(Vec3::new(5.0, 300.0, 300.0), None, 5.0, 3.0, 0.0, false),
                comms: comms(5.0, 2.2, 0.0, 0.0, 0.0, &[256.0, 1_000.0], 4_000.0, false),
                eclss: None,
                instruments: vec![
                    instrument("exp1", "Experimento de microgravidade 1", 60.0, 1.0, true),
                    instrument("exp2", "Forno de solidificação", 300.0, 0.5, true),
                ],
            };
            let mut v = base(Kind::SoundingRocket, id, "VSB-30", vec![s1, s2, pl], Some(sys), Shape::Rocket);
            v.chutes = vec![
                Chute { name: "Paraquedas de frenagem".into(), cda: 3.0, deploy_alt: 8_000.0, max_mach: 1.2, drogue: true, state: ChuteState::Stowed, inflation: 0.0 },
                Chute { name: "Paraquedas principal".into(), cda: 110.0, deploy_alt: 4_000.0, max_mach: 0.8, drogue: false, state: ChuteState::Stowed, inflation: 0.0 },
            ];
            v.landing = LandingMode::Splash;
            v.fts.installed = true;
            v.guidance.sounding = Some(SoundingProfile { elevation_deg: 84.0, azimuth_deg: 45.0, stage2_delay: 6.0 });
            v
        }
        "vlm1" => {
            let s1 = stage("S50 (1º estágio)", 1_500.0, 11_800.0, vec![solid("Motor S50", 400.0, 272.0, 250.0, 60.0)], 1.46, 6.0);
            let s2 = stage("S50 (2º estágio)", 1_350.0, 11_800.0, vec![solid("Motor S50 vácuo", 400.0, 292.0, 230.0, 60.0)], 1.46, 6.0);
            let s3 = stage("S44 (3º estágio)", 170.0, 615.0, vec![solid("Motor S44", 30.0, 285.0, 220.0, 50.0)], 1.0, 2.0);
            let (mut core, sys) = small_sat();
            core.name = "Satélite de 150 kg".into();
            let mut v = base(Kind::Satellite, id, "VLM-1 / satélite", vec![s1, s2, s3, core], Some(sys), Shape::Box);
            v.items.push(Item { id: "fairing".into(), name: "Coifa".into(), mass: 120.0 });
            v.fts.installed = true;
            v.guidance.ascent = Some(AscentProfile {
                target_pe_km: 300.0,
                target_ap_km: 300.0,
                inclination_deg: 5.0,
                north: true,
                raan_deg: None,
                turn_start_speed: 60.0,
                turn_end_km: 95.0,
                end_pitch_deg: 8.0,
                turn_shape: 0.55,
                closed_loop_stage: 99,
                max_g: 12.0,
                throttle_bucket: None,
                payload_sep: true,
                kick_stage: Some(2),
            });
            v
        }
        "medio_sso" | "medio_geo" | "medio_capsula" => {
            let mut s1 = stage(
                "1º estágio",
                25_600.0,
                395_700.0,
                (1..=9).map(|i| engine(&format!("Motor {i}"), 914.0, 311.0, 282.0, 0.4, 3, 97.0)).collect(),
                3.66,
                41.0,
            );
            s1.cryogenic = true;
            let mut s2 = stage("2º estágio", 4_000.0, 92_670.0, vec![engine("Motor de vácuo", 981.0, 348.0, 200.0, 0.39, 4, 97.0)], 3.66, 13.8);
            s2.cryogenic = true;
            match id {
                "medio_sso" => {
                    let (core, sys) = sat_observacao();
                    let mut v = base(Kind::Satellite, id, "Satélite de observação", vec![s1, s2, core], Some(sys), Shape::Box);
                    v.items.push(Item { id: "fairing".into(), name: "Coifa".into(), mass: 1_900.0 });
                    v.fts.installed = true;
                    v.guidance.ascent = Some(AscentProfile {
                        target_pe_km: 752.0,
                        target_ap_km: 752.0,
                        inclination_deg: 98.4,
                        north: false,
                        raan_deg: None,
                        turn_start_speed: 55.0,
                        turn_end_km: 80.0,
                        end_pitch_deg: 18.0,
                        turn_shape: 0.5,
                        closed_loop_stage: 1,
                        max_g: 4.5,
                        throttle_bucket: Some((28.0, 0.75)),
                        payload_sep: true,
                        kick_stage: None,
                    });
                    v
                }
                "medio_geo" => {
                    let (core, sys) = sat_geo();
                    let mut v = base(Kind::Satellite, id, "Satélite geoestacionário", vec![s1, s2, core], Some(sys), Shape::Box);
                    v.items.push(Item { id: "fairing".into(), name: "Coifa".into(), mass: 1_900.0 });
                    v.fts.installed = true;
                    v.guidance.ascent = Some(AscentProfile {
                        target_pe_km: 250.0,
                        target_ap_km: 35_786.0,
                        inclination_deg: 28.6,
                        north: true,
                        raan_deg: None,
                        turn_start_speed: 55.0,
                        turn_end_km: 75.0,
                        end_pitch_deg: 15.0,
                        turn_shape: 0.5,
                        closed_loop_stage: 1,
                        max_g: 4.5,
                        throttle_bucket: Some((28.0, 0.75)),
                        payload_sep: true,
                        kick_stage: None,
                    });
                    v
                }
                _ => {
                    let mut cap = capsula();
                    let mut stages = vec![s1, s2];
                    stages.append(&mut cap.stages);
                    cap.initial_stages = stages.len();
                    cap.stages = stages;
                    cap.def_id = id.into();
                    cap.fts.installed = true;
                    cap.guidance.ascent = Some(AscentProfile {
                        target_pe_km: 210.0,
                        target_ap_km: 210.0,
                        inclination_deg: 51.6,
                        north: true,
                        raan_deg: None,
                        turn_start_speed: 55.0,
                        turn_end_km: 75.0,
                        end_pitch_deg: 15.0,
                        turn_shape: 0.5,
                        closed_loop_stage: 1,
                        max_g: 4.0,
                        throttle_bucket: Some((28.0, 0.72)),
                        payload_sep: true,
                        kick_stage: None,
                    });
                    cap
                }
            }
        }
        "soyuz" => {
            let mut boosters = stage(
                "Blocos laterais (4)",
                4.0 * 3_784.0,
                4.0 * 39_160.0,
                (1..=4).map(|i| engine(&format!("Booster {i}"), 1_019.0, 319.0, 263.0, 1.0, 1, 61.0)).collect(),
                2.68,
                19.6,
            );
            boosters.boosters = true;
            let core = stage("Bloco central", 6_545.0, 90_100.0, vec![engine("Motor central", 990.0, 320.0, 255.0, 1.0, 1, 59.0)], 2.95, 27.1);
            let s3 = stage("3º estágio", 2_355.0, 25_400.0, vec![engine("Motor do 3º estágio", 298.0, 326.0, 200.0, 1.0, 1, 68.0)], 2.66, 6.7);
            let sm = Stage {
                spacecraft: true,
                ..stage("Módulo de serviço", 2_000.0, 880.0, vec![engine("Motor de manobra 3 kN", 2.95, 302.0, 250.0, 1.0, 30, 9.0)], 2.72, 2.6)
            };
            let mut cap = capsula();
            let mut cm = cap.stages.pop().unwrap();
            cm.name = "Módulo de descida e orbital".into();
            cm.dry_mass = 4_250.0;
            cm.propellant = 0.0;
            cm.capacity = 0.0;
            cm.engines.clear();
            cm.diameter = 2.2;
            cap.stages = vec![boosters, core, s3, sm, cm];
            cap.initial_stages = 5;
            cap.def_id = id.into();
            cap.name = "Nave tripulada".into();
            cap.items.push(Item { id: "fairing".into(), name: "Coifa".into(), mass: 1_800.0 });
            cap.les = Some(Les { thrust: 800e3, burn: 4.0, tower_mass: 2_400.0, available: true, firing: 0.0 });
            if let Some(hs) = &mut cap.heat_shield {
                hs.area = 3.8;
                hs.ablator_kg = 250.0;
                hs.initial_kg = 250.0;
            }
            cap.nose_radius = 2.2;
            if let Some(sys) = &mut cap.systems {
                if let Some(e) = &mut sys.eclss {
                    e.crew = 3;
                }
            }
            cap.chutes = vec![
                Chute { name: "Paraquedas de frenagem".into(), cda: 24.0, deploy_alt: 9_500.0, max_mach: 1.2, drogue: true, state: ChuteState::Stowed, inflation: 0.0 },
                Chute { name: "Paraquedas principal".into(), cda: 1_000.0, deploy_alt: 7_500.0, max_mach: 0.5, drogue: false, state: ChuteState::Stowed, inflation: 0.0 },
            ];
            cap.fts.installed = false;
            cap.guidance.landing_target = Some((47.4, 69.6));
            cap.guidance.ascent = Some(AscentProfile {
                target_pe_km: 200.0,
                target_ap_km: 240.0,
                inclination_deg: 51.6,
                north: true,
                raan_deg: None,
                turn_start_speed: 50.0,
                turn_end_km: 85.0,
                end_pitch_deg: 12.0,
                turn_shape: 0.55,
                closed_loop_stage: 1,
                max_g: 4.0,
                throttle_bucket: None,
                payload_sep: true,
                kick_stage: None,
            });
            cap
        }
        "sts" => {
            let mut srb = stage(
                "Foguetes sólidos (2)",
                2.0 * 91_000.0,
                2.0 * 502_000.0,
                vec![solid("SRB esquerdo", 13_000.0, 268.0, 242.0, 62.0), solid("SRB direito", 13_000.0, 268.0, 242.0, 62.0)],
                3.71,
                45.5,
            );
            srb.boosters = true;
            let mut et = stage(
                "Tanque externo",
                26_500.0,
                735_600.0,
                (1..=3).map(|i| engine(&format!("SSME {i}"), 2_279.0, 452.0, 366.0, 0.67, 1, 206.0)).collect(),
                8.4,
                46.9,
            );
            et.cryogenic = true;
            let orbiter = Stage {
                spacecraft: true,
                ..stage("Orbitador", 88_000.0, 10_800.0, vec![engine("OMS (2×27 kN)", 53.4, 316.0, 250.0, 1.0, 20, 8.6)], 5.0, 37.2)
            };
            let mut p = power(
                0.0,
                0.0,
                false,
                2_000.0,
                vec![
                    load("gpc", "Computadores (GPC)", 2_500.0, true),
                    load("ttc", "Comunicações", 600.0, true),
                    load("eclss", "Suporte de vida", 3_000.0, true),
                    load("payload", "Carga útil no compartimento", 2_000.0, false),
                    load("cabin", "Cabine e iluminação", 1_200.0, false),
                ],
            );
            p.fuel_cell = Some(FuelCell { max_w: 21_000.0, reactant_kg: 3_000.0, kg_per_kwh: 0.36, on: true });
            let mut th = thermal(100_000.0, 600.0, true);
            th.radiator_w_per_k = 400.0;
            th.radiator_on = false;
            let sys = Systems {
                power: p,
                thermal: th,
                adcs: adcs(Vec3::new(1.0e6, 8.0e6, 8.5e6), None, 30_000.0, 1_300.0, 3_900.0, false),
                comms: comms(100.0, 2.2, 3.0, 30.0, 4.0, &[32.0, 192.0, 50_000.0], 100_000.0, true),
                eclss: Some(eclss(7, &[("Convés de voo", 33.0), ("Convés intermediário", 41.0)], 280.0, 200.0, 8.0, 60.0, 0.0, 600.0)),
                instruments: vec![instrument("bay", "Experimentos no compartimento de carga", 1_500.0, 2.0, false)],
            };
            let mut v = base(Kind::Shuttle, id, "Ônibus espacial", vec![srb, et, orbiter], Some(sys), Shape::Winged);
            v.nose_radius = 0.65;
            v.heat_shield = Some(HeatShield {
                ablator_kg: 1_000.0,
                initial_kg: 1_000.0,
                ablation_heat: 5e9,
                area: 400.0,
                temp: 293.0,
                flux: 0.0,
                peak_flux: 0.0,
                heat_load: 0.0,
            });
            v.landing = LandingMode::Runway;
            v.fts.installed = true;
            v.guidance.landing_target = Some((28.615, -80.695));
            v.guidance.ascent = Some(AscentProfile {
                target_pe_km: 300.0,
                target_ap_km: 300.0,
                inclination_deg: 51.6,
                north: true,
                raan_deg: None,
                turn_start_speed: 45.0,
                turn_end_km: 80.0,
                end_pitch_deg: 12.0,
                turn_shape: 0.55,
                closed_loop_stage: 1,
                max_g: 3.0,
                throttle_bucket: Some((30.0, 0.72)),
                payload_sep: false,
                kick_stage: None,
            });
            v
        }
        "estacao" => {
            let core = spacecraft_stage("Estação", 412_000.0, 6_000.0, vec![engine("Motores de reimpulso 2×2,9 kN", 5.9, 302.0, 250.0, 1.0, 1_000, 9.0)], 10.0);
            let mut th = thermal(420_000.0, 2_000.0, true);
            th.radiator_w_per_k = 900.0;
            th.radiator_on = true;
            let sys = Systems {
                power: power(
                    2_000.0,
                    0.10,
                    true,
                    100_000.0,
                    vec![
                        load("cmd", "Comando e controle", 4_000.0, true),
                        load("comms", "Comunicações (banda S e Ku)", 2_500.0, true),
                        load("eclss", "Suporte de vida", 25_000.0, true),
                        load("lab", "Laboratórios", 35_000.0, false),
                        load("crew", "Áreas da tripulação", 6_000.0, false),
                        load("thermal", "Bombas do controle térmico", 8_000.0, true),
                    ],
                ),
                thermal: th,
                adcs: Adcs {
                    max_rate: 0.1f64.to_radians(),
                    ..adcs(Vec3::new(1.2e8, 4.1e7, 1.6e8), Some((19_000.0, 258.0)), 2_000.0, 800.0, 0.0, false)
                },
                comms: comms(50.0, 2.2, 3.0, 35.0, 3.0, &[192.0, 2_000.0, 300_000.0], 2_000_000.0, true),
                eclss: Some(eclss(
                    7,
                    &[
                        ("Módulo de serviço", 89.0),
                        ("Módulo de carga e energia", 72.0),
                        ("Nó 1", 55.0),
                        ("Laboratório", 106.0),
                        ("Nó 2", 75.0),
                        ("Laboratório europeu", 75.0),
                        ("Laboratório japonês", 150.0),
                        ("Nó 3", 75.0),
                    ],
                    300.0,
                    300.0,
                    6.0,
                    UNLIMITED,
                    5.4,
                    2_000.0,
                )),
                instruments: vec![
                    instrument("exp", "Experimentos externos", 3_000.0, 50.0, true),
                    instrument("earthcam", "Câmeras de observação da Terra", 200.0, 20.0, false),
                ],
            };
            let mut v = base(Kind::Station, id, "Estação espacial", vec![core], Some(sys), Shape::Box);
            v.ports = vec![
                Port { name: "Nó 2 frontal".into(), offset: 55.0, occupant: None },
                Port { name: "Nó 2 zênite".into(), offset: 55.0, occupant: None },
                Port { name: "Módulo de serviço popa".into(), offset: 60.0, occupant: None },
            ];
            v
        }
        "amazonia" => {
            let (core, sys) = sat_observacao();
            base(Kind::Satellite, id, "Satélite de observação", vec![core], Some(sys), Shape::Box)
        }
        "modulo_lunar" => {
            let descent = stage("Estágio de descida", 2_150.0, 8_200.0, vec![engine("Motor de descida", 45.0, 311.0, 250.0, 0.10, 3, 7.4)], 4.2, 3.2);
            let ascent = spacecraft_stage("Estágio de subida", 2_200.0, 2_350.0, vec![engine("Motor de subida", 16.0, 311.0, 250.0, 1.0, 2, 8.4)], 4.0);
            let sys = Systems {
                power: power(
                    0.0,
                    0.0,
                    false,
                    30_000.0,
                    vec![
                        load("obc", "Computador de guiamento", 150.0, true),
                        load("ttc", "Rádio em banda S", 120.0, true),
                        load("adcs", "Controle de atitude", 200.0, true),
                        load("eclss", "Suporte de vida", 300.0, true),
                        load("radar", "Radar de pouso", 120.0, true),
                    ],
                ),
                thermal: thermal(4_700.0, 20.0, true),
                adcs: adcs(Vec3::new(9_000.0, 12_000.0, 12_000.0), None, 900.0, 280.0, 400.0, false),
                comms: comms(20.0, 2.2, 3.0, 25.0, 12.0, &[1.6, 51.2], 100.0, false),
                eclss: Some(eclss(2, &[("Cabine", 6.7)], 48.0, 10.0, 2.0, UNLIMITED, 0.0, 150.0)),
                instruments: vec![],
            };
            let mut v = base(Kind::Capsule, id, "Módulo lunar Jaci", vec![descent, ascent], Some(sys), Shape::Box);
            v.landing = LandingMode::Legs;
            v.guidance.auto_stage = false;
            v.guidance.auto_chutes = false;
            v
        }
        "sonda_pouso_marte" => {
            let core = spacecraft_stage("Sonda de pouso", 350.0, 80.0, vec![engine("Propulsores de descida 12×293 N", 3.5, 220.0, 200.0, 0.10, 5, 15.0)], 2.64);
            let mut p = power(4.0, 0.28, false, 1_000.0, vec![load("obc", "Computador de bordo", 40.0, true), load("ttc", "Rádio UHF (relé)", 30.0, true), load("adcs", "Controle de atitude", 30.0, true)]);
            if let Some(sa) = &mut p.solar {
                sa.deployed = false;
            }
            let sys = Systems {
                power: p,
                thermal: thermal(600.0, 6.0, false),
                adcs: adcs(Vec3::new(150.0, 150.0, 150.0), None, 20.0, 25.0, 30.0, false),
                comms: comms(8.0, 0.4, 2.0, 0.0, 0.0, &[8.0, 128.0], 2_000.0, true),
                eclss: None,
                instruments: vec![instrument("met", "Estação meteorológica", 15.0, 0.01, true), instrument("cam", "Câmera estéreo", 20.0, 2.0, false)],
            };
            let mut v = base(Kind::Probe, id, "Sonda de pouso Jaci", vec![core], Some(sys), Shape::Capsule);
            v.nose_radius = 1.3;
            v.heat_shield = Some(HeatShield { ablator_kg: 60.0, initial_kg: 60.0, ablation_heat: 30e6, area: 5.5, temp: 293.0, flux: 0.0, peak_flux: 0.0, heat_load: 0.0 });
            v.chutes = vec![Chute { name: "Paraquedas supersônico".into(), cda: 65.0, deploy_alt: 12_000.0, max_mach: 2.2, drogue: false, state: ChuteState::Stowed, inflation: 0.0 }];
            v.landing = LandingMode::Legs;
            v.guidance.ballistic = true;
            v.guidance.auto_stage = false;
            v.guidance.auto_chutes = true;
            v.guidance.auto_pdi_alt = Some(1_200.0);
            v
        }
        "sonda_marte" | "sonda_lua" => {
            let mut upper = stage("Estágio de escape", 2_250.0, 20_830.0, vec![engine("Motor H₂/LOX", 99.2, 451.0, 200.0, 1.0, 3, 42.0)], 3.05, 12.7);
            upper.cryogenic = true;
            let mars = id == "sonda_marte";
            let core = spacecraft_stage(
                if mars { "Sonda marciana" } else { "Sonda lunar" },
                if mars { 1_031.0 } else { 1_018.0 },
                if mars { 1_149.0 } else { 898.0 },
                vec![if mars {
                    engine("Motores principais 6×170 N", 1.02, 318.0, 280.0, 1.0, 40, 9.0)
                } else {
                    engine("Motores 4×80 N", 0.32, 220.0, 200.0, 1.0, 60, 15.0)
                }],
                3.0,
            );
            let mut p = power(
                if mars { 20.0 } else { 10.7 },
                0.25,
                true,
                if mars { 2_800.0 } else { 2_400.0 },
                vec![
                    load("obc", "Computador de bordo", 60.0, true),
                    load("ttc", "Transponder de espaço profundo", 40.0, true),
                    load("adcs", "Controle de atitude", 90.0, true),
                ],
            );
            if let Some(sa) = &mut p.solar {
                sa.deployed = false;
            }
            let sys = Systems {
                power: p,
                thermal: thermal(2_200.0, 14.0, false),
                adcs: adcs(Vec3::new(2_000.0, 3_000.0, 3_500.0), Some((50.0, 0.2)), 10.0, 60.0, 20.0, false),
                comms: comms(if mars { 100.0 } else { 40.0 }, 8.4, 3.0, if mars { 46.0 } else { 33.0 }, if mars { 0.8 } else { 3.0 }, &[0.01, 0.5, 30.0, 500.0, 6_000.0], 160_000.0, false),
                eclss: None,
                instruments: if mars {
                    vec![
                        instrument("hirise", "Câmera de alta resolução", 60.0, 20.0, false),
                        instrument("spectro", "Espectrômetro de imagem", 45.0, 5.0, false),
                        instrument("radar", "Radar de subsuperfície", 120.0, 2.0, false),
                    ]
                } else {
                    vec![
                        instrument("lroc", "Câmera de reconhecimento", 30.0, 10.0, false),
                        instrument("lola", "Altímetro laser", 34.0, 0.2, false),
                    ]
                },
            };
            let kind = Kind::Probe;
            let mut v = base(kind, id, if mars { "Sonda marciana" } else { "Sonda lunar" }, vec![upper, core], Some(sys), Shape::Box);
            v.guidance.auto_stage = false;
            v
        }
        _ => return None,
    })
}

/// Satélite pequeno do VLM-1.
fn small_sat() -> (Stage, Systems) {
    let core = spacecraft_stage("Satélite", 140.0, 10.0, vec![engine("Propulsor 1 N", 0.001, 210.0, 180.0, 1.0, 200, 10.0)], 0.8);
    let sys = Systems {
        power: power(1.2, 0.29, false, 200.0, vec![load("obc", "Computador de bordo", 8.0, true), load("ttc", "Rádio", 6.0, true), load("adcs", "ADCS", 10.0, true)]),
        thermal: thermal(150.0, 2.5, false),
        adcs: adcs(Vec3::new(8.0, 9.0, 10.0), Some((0.4, 0.01)), 0.0, 0.0, 0.0, true),
        comms: comms(1.0, 2.2, 0.0, 0.0, 0.0, &[9.6, 64.0, 1_000.0], 8_000.0, false),
        eclss: None,
        instruments: vec![instrument("cam", "Câmera", 12.0, 8.0, false)],
    };
    (core, sys)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_constroem() {
        for e in ENTRIES {
            let v = build(e.id).unwrap_or_else(|| panic!("{}", e.id));
            assert!(v.mass() > 0.0);
            assert!(v.stages.last().unwrap().spacecraft || v.kind == Kind::SoundingRocket, "{}", e.id);
        }
    }

    #[test]
    fn dv_do_lancador_medio() {
        let v = build("medio_sso").unwrap();
        let dv = v.dv_remaining();
        // ~13,7 km/s com só 640 kg de carga (e a coifa junto no cálculo).
        assert!(dv > 11_000.0 && dv < 15_000.0, "{dv}");
    }
}
