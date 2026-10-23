//! O mundo: relógio, veículos, estações, controle de lançamento, clima,
//! eventos, alarmes, comandos e falhas.

use crate::bodies::Body;
use crate::commands::{self, Command};
use crate::ground::{self, Station};
use crate::launch::{CountState, LaunchControl, Weather};
use crate::math::{Quat, Rng, Vec3};
use crate::orbit::{propagate_kepler, rtn_to_inertial};
use crate::subsystems::AttMode;
use crate::telemetry::{self, LimitState};
use crate::tracked::Tracked;
use crate::training::{AlarmLog, TrainingLog};
use crate::vehicle::*;
use serde::{Deserialize, Serialize};

/// Falhas que o instrutor pode injetar: id, nome e o que o argumento escolhe.
pub const FAULTS: &[(&str, &str, &str)] = &[
    ("engine", "Falha de motor", "motor nº"),
    ("leak", "Vazamento de propelente", ""),
    ("battery_cell", "Célula da bateria perdida", ""),
    ("solar_stuck", "Mecanismo do painel solar travado", ""),
    ("solar_degrade", "Metade de uma asa solar perdida", ""),
    ("wheel", "Roda de reação falhou", "eixo (1–3)"),
    ("gyro", "Viés nos girômetros", ""),
    ("star_tracker", "Rastreador de estrelas cego", ""),
    ("comms", "Transmissor em pane", ""),
    ("heater", "Aquecedor falhou", ""),
    ("cabin_leak", "Vazamento na cabine", "módulo nº"),
    ("scrubber", "Remoção de CO₂ degradada", ""),
    ("chute", "Paraquedas não abre", "paraquedas nº"),
    ("fuel_cell", "Célula a combustível degradada", ""),
    ("range_fouled", "Barco na área de exclusão", ""),
    ("ground_fault", "Falha de sistema de solo", ""),
    ("storm", "Tempestade com raios", ""),
];

/// Veículo de verdade, acompanhado pela telemetria CCSDS que chega por UDP.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct External {
    pub id: VehicleId,
    pub name: String,
    pub apid: u16,
    pub tm: Vec<(String, f64)>,
    /// Instante (simulação) do último pacote.
    pub last: f64,
    pub packets: u64,
    /// Pontos do dicionário XTCE (vazio: o pacote de navegação 0x200).
    #[serde(default)]
    pub points: Vec<ExternalPoint>,
}

/// Ponto de telemetria de um veículo externo, do dicionário XTCE.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalPoint {
    pub id: String,
    pub name: String,
    pub unit: String,
    pub yellow: Option<(f64, f64)>,
    pub red: Option<(f64, f64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alarm {
    pub id: u64,
    pub t: f64,
    pub vehicle: VehicleId,
    pub point: String,
    pub name: String,
    pub level: LimitState,
    pub value: f64,
    pub unit: String,
    pub acked: bool,
    /// Ainda fora do limite.
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CmdStatus {
    Queued,
    Sent,
    Executed,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CmdRecord {
    pub id: u64,
    pub vehicle: VehicleId,
    pub text: String,
    pub sent: f64,
    /// Quando executa a bordo (atraso da luz ou hora marcada).
    pub exec_at: f64,
    pub status: CmdStatus,
    pub message: String,
    #[serde(skip)]
    pub cmd: Option<Command>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Corridor {
    pub lat: f64,
    pub lon: f64,
    pub azimuth: f64,
    pub half_width_km: f64,
}

/// Ponto de impacto instantâneo (balístico, sem arrasto).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Iip {
    pub lat: f64,
    pub lon: f64,
    pub t: f64,
    pub cross_km: f64,
    pub inside: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct World {
    pub t: f64,
    pub warp: f64,
    pub paused: bool,
    pub scenario: String,
    pub mission: String,
    pub vehicles: Vec<Vehicle>,
    pub next_id: VehicleId,
    pub stations: Vec<Station>,
    pub launch: LaunchControl,
    pub weather: Weather,
    pub events: Vec<Event>,
    pub alarms: Vec<Alarm>,
    pub commands: Vec<CmdRecord>,
    pub rng: Rng,
    pub corridor: Option<Corridor>,
    #[serde(default)]
    pub iip: Vec<(VehicleId, Iip)>,
    /// Estado de limite de cada ponto, para gerar alarmes nas transições.
    #[serde(default)]
    pub limit_states: Vec<(VehicleId, String, LimitState)>,
    pub next_seq: u64,
    pub next_node: u32,
    /// Volta para 1× num alarme.
    pub warp_stop: bool,
    #[serde(default)]
    pub last_slow: f64,
    /// Contador de eventos já lidos pelos clientes.
    #[serde(default)]
    pub event_seq: u64,
    #[serde(default)]
    pub next_periodic: f64,
    #[serde(default)]
    pub externals: Vec<External>,
    /// Satélites reais acompanhados por TLE (SGP4).
    #[serde(default)]
    pub tracked: Vec<Tracked>,
    /// Falhas planejadas e o registro do treino (para o relatório).
    #[serde(default)]
    pub training: TrainingLog,
    /// Perturbação dos terceiros corpos (Sol, Lua…) ligada.
    #[serde(default = "yes")]
    pub third_body: bool,
    /// Injetando falha sem avisar a equipe (veja `inject_as`).
    #[serde(skip)]
    pub(crate) quiet_fault: bool,
}

fn yes() -> bool {
    true
}

impl World {
    pub fn new(t: f64, seed: u64) -> Self {
        World {
            t,
            warp: 1.0,
            paused: false,
            scenario: String::new(),
            mission: String::new(),
            vehicles: Vec::new(),
            next_id: 1,
            stations: ground::default_stations(),
            launch: LaunchControl::default(),
            weather: Weather::default(),
            events: Vec::new(),
            alarms: Vec::new(),
            commands: Vec::new(),
            rng: Rng::new(seed),
            corridor: None,
            iip: Vec::new(),
            limit_states: Vec::new(),
            next_seq: 1,
            next_node: 1,
            warp_stop: true,
            last_slow: -1e9,
            event_seq: 0,
            next_periodic: 0.0,
            externals: Vec::new(),
            tracked: Vec::new(),
            training: TrainingLog { start: Some(t), ..Default::default() },
            third_body: true,
            quiet_fault: false,
        }
    }

    /// Passa a acompanhar um satélite real pelo TLE; se o número NORAD já
    /// estiver na lista, só troca os elementos (o id continua o mesmo).
    pub fn track(&mut self, name: Option<&str>, line1: &str, line2: &str) -> Result<VehicleId, String> {
        let mut s = Tracked::from_tle(0, name, line1, line2)?;
        if let Some(old) = self.tracked.iter_mut().find(|x| x.norad == s.norad) {
            s.id = old.id;
            *old = s;
            return Ok(old.id);
        }
        s.id = self.next_id;
        self.next_id += 1;
        let id = s.id;
        self.event(Some(id), Level::Info, format!("Acompanhando {} (NORAD {})", s.name, s.norad));
        self.tracked.push(s);
        Ok(id)
    }

    /// Deixa de acompanhar um satélite real.
    pub fn untrack(&mut self, id: VehicleId) -> bool {
        let before = self.tracked.len();
        self.tracked.retain(|s| s.id != id);
        self.tracked.len() != before
    }

    /// Atualiza (ou cria) um veículo externo com um pacote de telemetria.
    pub fn external_update(&mut self, apid: u16, points: Vec<(String, f64)>) -> VehicleId {
        self.external_update_with(apid, points, None)
    }

    /// Como `external_update`, com o pacote do dicionário XTCE (nome e limites).
    pub fn external_update_with(&mut self, apid: u16, points: Vec<(String, f64)>, def: Option<&crate::xtce::PacketDef>) -> VehicleId {
        let t = self.t;
        if let Some(e) = self.externals.iter_mut().find(|e| e.apid == apid) {
            for (k, v) in points {
                match e.tm.iter_mut().find(|(id, _)| *id == k) {
                    Some(slot) => slot.1 = v,
                    None => e.tm.push((k, v)),
                }
            }
            e.last = t;
            e.packets += 1;
            return e.id;
        }
        let id = self.next_id;
        self.next_id += 1;
        let (name, defs) = match def {
            Some(d) => (
                format!("{} ({})", d.system, d.container),
                d.fields.iter().map(|f| ExternalPoint { id: f.id.clone(), name: f.name.clone(), unit: f.unit.clone(), yellow: f.yellow, red: f.red }).collect(),
            ),
            None => (format!("Veículo externo (APID 0x{apid:03X})"), Vec::new()),
        };
        self.externals.push(External { id, name, apid, tm: points, last: t, packets: 1, points: defs });
        self.event(Some(id), Level::Good, format!("Telemetria CCSDS recebida: novo veículo externo, APID 0x{apid:03X}"));
        id
    }

    pub fn add_vehicle(&mut self, mut v: Vehicle) -> VehicleId {
        v.id = self.next_id;
        self.next_id += 1;
        let t = self.t;
        v.update_derived(t, (0.0, 0.0, 0.0));
        let id = v.id;
        self.vehicles.push(v);
        id
    }

    pub fn vehicle(&self, id: VehicleId) -> Option<&Vehicle> {
        self.vehicles.iter().find(|v| v.id == id)
    }

    pub fn vehicle_mut(&mut self, id: VehicleId) -> Option<&mut Vehicle> {
        self.vehicles.iter_mut().find(|v| v.id == id)
    }

    pub fn event(&mut self, vehicle: Option<VehicleId>, level: Level, text: impl Into<String>) {
        self.events.push(Event { t: self.t, vehicle, level, text: text.into(), from: None, channel: None });
        self.event_seq += 1;
        if self.events.len() > 3_000 {
            self.events.drain(..500);
        }
        if level == Level::Alarm && self.warp_stop && self.warp > 1.0 {
            self.warp = 1.0;
            self.last_slow = self.t;
        }
    }

    /// Mensagem num loop da sala de controle (entra no registro, nas gravações e na reprodução).
    pub fn chat(&mut self, from: &str, channel: &str, text: &str) {
        self.events.push(Event { t: self.t, vehicle: None, level: Level::Chat, text: text.into(), from: Some(from.into()), channel: Some(channel.into()) });
        self.event_seq += 1;
        if self.events.len() > 3_000 {
            self.events.drain(..500);
        }
    }

    /// Avança o mundo `real_dt` segundos de relógio (multiplicados pela aceleração).
    /// Para não travar, limita o trabalho por chamada.
    pub fn advance(&mut self, real_dt: f64) {
        if self.paused {
            return;
        }
        let mut remaining = real_dt * self.warp;
        let mut iterations = 0;
        let started = std::time::Instant::now();
        while remaining > 1e-9 {
            let mut h = remaining;
            for v in &self.vehicles {
                if v.attached_to.is_none() && !v.phase.is_final() {
                    h = h.min(v.max_step(self.t));
                }
            }
            if matches!(self.launch.state, CountState::Counting) {
                let tm = self.launch.t_minus;
                h = h.min(if tm < 15.0 { 0.05 } else { 1.0 });
                // Não pula itens da sequência.
                if let Some(next) = self.launch.sequence.iter().filter(|s| !s.done && s.at < tm).map(|s| tm - s.at).reduce(f64::min) {
                    h = h.min(next.max(0.01));
                }
            }
            if let Some(next) = self.commands.iter().filter(|c| c.status == CmdStatus::Sent).map(|c| c.exec_at - self.t).reduce(f64::min) {
                h = h.min(next.max(0.01));
            }
            h = h.max(1e-3);
            self.step(h);
            remaining -= h;
            iterations += 1;
            if iterations % 64 == 0 && started.elapsed().as_secs_f64() > 0.08 {
                // Não deu tempo: a simulação fica para trás e a aceleração cai.
                if self.warp > 1.0 && remaining > 0.0 {
                    self.warp = (self.warp / 2.0).max(1.0);
                }
                break;
            }
        }
    }

    /// Um passo de `h` segundos.
    pub fn step(&mut self, h: f64) {
        let t = self.t;
        self.weather.evolve(h, &mut self.rng);
        // Falhas planejadas pelo instrutor que chegaram na hora.
        if !self.training.plan.is_empty() {
            self.run_fault_plan();
        }
        // Contagem regressiva.
        if let Some(id) = self.launch.vehicle {
            let link = self.vehicle(id).is_some_and(|v| v.phase == Phase::Prelaunch);
            if let Some(idx) = self.vehicles.iter().position(|v| v.id == id) {
                if self.vehicles[idx].phase == Phase::Prelaunch {
                    let v = &mut self.vehicles[idx];
                    let out = self.launch.tick(v, &self.weather, &mut self.rng, h, link);
                    if out.ignite_liquids {
                        ignite_liquids(v);
                    }
                    if out.shutdown {
                        for s in &mut v.stages {
                            for e in &mut s.engines {
                                if e.lit() && !e.solid {
                                    e.state = EngineState::Stopping;
                                }
                            }
                        }
                    }
                    if out.release {
                        // Sólidos acendem no T-0.
                        for i in v.active_stages() {
                            for e in &mut v.stages[i].engines {
                                if e.solid && e.state == EngineState::Off && e.ignitions > 0 {
                                    e.ignitions -= 1;
                                    e.state = EngineState::Running;
                                    e.spool = 1.0;
                                }
                            }
                        }
                        v.released = true;
                        let az = v.guidance.sounding.as_ref().map(|s| s.azimuth_deg).or(v.guidance.azimuth);
                        if let Some(site) = &v.site {
                            self.corridor = Some(Corridor { lat: site.lat, lon: site.lon, azimuth: az.unwrap_or(90.0), half_width_km: 120.0 });
                        }
                    }
                    for (l, txt) in out.events {
                        self.event(Some(id), l, txt);
                    }
                }
            }
        }
        // Energia de solo até o veículo passar para a interna (T-5 min).
        let internal = self.launch.internal_power;
        let pad = self.launch.vehicle;
        for v in &mut self.vehicles {
            v.ground_power = v.phase == Phase::Prelaunch && !(Some(v.id) == pad && internal);
        }
        // Estado dos outros, para encontro e apontamento.
        let targets: Vec<TargetInfo> = self
            .vehicles
            .iter()
            .map(|v| TargetInfo {
                id: v.id,
                central: v.central,
                r: v.r,
                v: v.v,
                port_offset: v.ports.iter().find(|p| p.occupant.is_none()).map_or(15.0, |p| p.offset),
            })
            .collect();
        // Terceiros corpos, uma vez por passo para cada corpo central em uso.
        let perturbers = if self.third_body {
            let mut centrals: Vec<Body> = self.vehicles.iter().filter(|v| !matches!(v.phase, Phase::Prelaunch | Phase::Landed) && !v.phase.is_final()).map(|v| v.central).collect();
            centrals.sort_by_key(|b| b.id());
            centrals.dedup();
            crate::vehicle::perturbers_at(&centrals, t)
        } else {
            Vec::new()
        };
        let env = Env {
            t,
            wind_speed: self.weather.surface_wind,
            wind_jet: self.weather.jet_wind,
            wind_from: self.weather.wind_from,
            targets: &targets,
            coarse: self.warp >= 50.0,
            perturbers: &perturbers,
        };
        let mut spawned = Vec::new();
        let mut events = Vec::new();
        let mut docks = Vec::new();
        for v in &mut self.vehicles {
            let before_az = v.guidance.azimuth;
            let out = v.step(&env, h);
            if before_az.is_none() && v.guidance.azimuth.is_some() {
                if let (Some(c), Some(az)) = (&mut self.corridor, v.guidance.azimuth) {
                    c.azimuth = az;
                }
            }
            for (l, txt) in out.events {
                events.push((v.id, l, txt));
            }
            for d in out.jettisoned {
                spawned.push(d);
            }
            if let Some(target) = out.docked_to {
                docks.push((v.id, target));
            }
        }
        self.t += h;
        for (id, l, txt) in events {
            self.event(Some(id), l, txt);
        }
        for d in spawned {
            self.add_vehicle(d);
        }
        for (id, target) in docks {
            self.dock(id, target);
        }
        // Veículos acoplados seguem o anfitrião.
        let hosts: Vec<(VehicleId, Vec3, Vec3, crate::math::Quat, Body)> = self.vehicles.iter().map(|v| (v.id, v.r, v.v, v.att, v.central)).collect();
        for v in &mut self.vehicles {
            if let Some(host) = v.attached_to {
                if let Some(&(_, r, vel, att, central)) = hosts.iter().find(|x| x.0 == host) {
                    v.central = central;
                    v.r = r;
                    v.v = vel;
                    v.att = att;
                    v.update_derived(self.t, (0.0, 0.0, 0.0));
                }
            }
        }
        self.periodic();
        self.run_commands();
    }

    /// O que roda uma vez por segundo de simulação: enlaces, limites, proteção,
    /// segurança de voo.
    fn periodic(&mut self) {
        let t = self.t;
        if t < self.next_periodic {
            return;
        }
        // Uma vez por segundo de simulação; em alta aceleração, mais espaçado.
        self.next_periodic = t + (self.warp / 20.0).clamp(1.0, 600.0);
        // Enlaces.
        let stations = self.stations.clone();
        for v in &mut self.vehicles {
            if v.systems.is_none() {
                continue;
            }
            let link = if v.phase == Phase::Prelaunch {
                // Umbilical na plataforma.
                Some(crate::subsystems::LinkStatus {
                    station: "Umbilical da plataforma".into(),
                    elevation_deg: 90.0,
                    azimuth_deg: 0.0,
                    range_m: 100.0,
                    margin_db: 30.0,
                    rate_bps: 1e6,
                    owlt: 0.0,
                    via_hga: false,
                    via_relay: false,
                })
            } else if let Some(host) = v.attached_to {
                let _ = host;
                v.systems.as_ref().and_then(|s| s.comms.link.clone()).or_else(|| ground::best_link(v, &stations, t))
            } else {
                ground::best_link(v, &stations, t)
            };
            if let Some(sys) = &mut v.systems {
                sys.comms.link = link;
            }
        }
        self.limits();
        self.fdir();
        self.range_safety();
    }

    fn limits(&mut self) {
        let mut new_alarms = Vec::new();
        let mut checks: Vec<(VehicleId, String, String, String, f64, LimitState)> = Vec::new();
        for v in &self.vehicles {
            if v.kind == Kind::Debris || v.phase == Phase::Destroyed {
                continue;
            }
            let defs = telemetry::defs(v);
            let vals = telemetry::values(v, self.t);
            for d in defs.iter().filter(|d| d.yellow.is_some() || d.red.is_some()) {
                let Some((_, val)) = vals.iter().find(|(k, _)| *k == d.id) else { continue };
                // Propelente acabando é normal na subida; não alarma estágios que já queimaram.
                if d.id.starts_with("prop.") && d.id.ends_with(".prop") {
                    continue;
                }
                // Apontamento só conta em órbita, com o ADCS no controle, e não
                // enquanto a nave gira de propósito para uma queima.
                let slewing = v.guidance.nodes.iter().any(|n| !n.done && (n.executing || n.t - self.t < 900.0))
                    || v.guidance.program != Program::None;
                if d.id.starts_with("adcs.") && (!matches!(v.phase, Phase::Orbit | Phase::Escape) || slewing) {
                    continue;
                }
                checks.push((v.id, d.id.clone(), d.name.clone(), d.unit.to_string(), *val, d.check(*val)));
            }
        }
        // Veículos externos com limites no dicionário (XTCE).
        for e in &self.externals {
            for p in e.points.iter().filter(|p| p.yellow.is_some() || p.red.is_some()) {
                let Some((_, val)) = e.tm.iter().find(|(k, _)| *k == p.id) else { continue };
                let def = telemetry::TmDef { id: String::new(), name: String::new(), unit: "", group: "", yellow: p.yellow, red: p.red };
                checks.push((e.id, p.id.clone(), p.name.clone(), p.unit.clone(), *val, def.check(*val)));
            }
        }
        for (vid, point, name, unit, val, st) in checks {
            let prev = self.limit_states.iter().position(|(id, p, _)| *id == vid && *p == point);
            let old = prev.map(|i| self.limit_states[i].2).unwrap_or(LimitState::Nominal);
            if st == old {
                continue;
            }
            match prev {
                Some(i) => self.limit_states[i].2 = st,
                None => self.limit_states.push((vid, point.clone(), st)),
            }
            if st != LimitState::Nominal {
                new_alarms.push((vid, point, name, st, val, unit));
            } else {
                for a in self.alarms.iter_mut().filter(|a| a.vehicle == vid && a.point == point && a.active) {
                    a.active = false;
                    if let Some(l) = self.training.alarms.iter_mut().find(|l| l.id == a.id) {
                        l.cleared_at.get_or_insert(self.t);
                    }
                }
            }
        }
        for (vid, point, name, st, val, unit) in new_alarms {
            let id = self.next_seq;
            self.next_seq += 1;
            for a in self.alarms.iter_mut().filter(|a| a.vehicle == vid && a.point == point) {
                a.active = false;
            }
            self.alarms.push(Alarm { id, t: self.t, vehicle: vid, point: point.clone(), name: name.clone(), level: st, value: val, unit: unit.clone(), acked: false, active: true });
            self.training.alarms.push(AlarmLog { id, t: self.t, vehicle: vid, name: name.clone(), level: st, acked_at: None, cleared_at: None });
            let level = if st == LimitState::Red { Level::Alarm } else { Level::Warn };
            self.event(Some(vid), level, format!("{}: {} {:.2} {}", if st == LimitState::Red { "LIMITE VERMELHO" } else { "Limite amarelo" }, name, val, unit));
        }
        // Os reconhecidos e já normais saem da lista.
        self.alarms.retain(|a| a.active || !a.acked);
        if self.alarms.len() > 300 {
            let n = self.alarms.len() - 300;
            self.alarms.drain(..n);
        }
    }

    /// Detecção, isolamento e recuperação de falhas a bordo.
    fn fdir(&mut self) {
        let mut ev = Vec::new();
        for v in &mut self.vehicles {
            if v.phase == Phase::Prelaunch || v.phase == Phase::Destroyed {
                continue;
            }
            let quiet = !v.any_engine_lit() && v.guidance.program == Program::None;
            let kind = v.kind;
            let Some(sys) = &mut v.systems else { continue };
            // Bateria baixa: modo de segurança.
            if sys.power.soc < 0.2 && !v.safe_mode && sys.power.battery_wh > 0.0 {
                v.safe_mode = true;
                sys.power.shed();
                for i in &mut sys.instruments {
                    i.on = false;
                }
                if quiet {
                    sys.adcs.mode = if kind == Kind::Probe { AttMode::EarthPoint } else { AttMode::SunPoint };
                }
                ev.push((v.id, Level::Alarm, "Proteção: bateria abaixo de 20 %, modo de segurança (cargas cortadas, painéis ao Sol)".to_string()));
            }
            // Rodas quase saturadas: descarga automática.
            if sys.adcs.wheel_saturation() > 0.9 && !sys.adcs.desaturating {
                sys.adcs.desaturating = true;
                ev.push((v.id, Level::Warn, "Proteção: rodas de reação perto da saturação, descarga automática".into()));
            }
            // Gravador cheio: começa a descarregar quando houver enlace.
            if sys.comms.stored_mb > 0.9 * sys.comms.storage_mb && !sys.comms.dumping && sys.comms.link.is_some() {
                sys.comms.dumping = true;
                ev.push((v.id, Level::Info, "Gravador quase cheio: descarregando para o solo".into()));
            }
            if let Some(e) = &mut sys.eclss {
                let p = e.pressure();
                if p < 70.0 && e.crew > 0 && !v.faults.iter().any(|f| f == "depress_alarm") {
                    v.faults.push("depress_alarm".into());
                    ev.push((v.id, Level::Alarm, format!("DESPRESSURIZAÇÃO: cabine a {:.0} kPa — isole o módulo com vazamento", p)));
                }
                if e.ppco2() > 1.0 && !v.faults.iter().any(|f| f == "co2_alarm") {
                    v.faults.push("co2_alarm".into());
                    ev.push((v.id, Level::Alarm, format!("CO₂ alto: {:.2} kPa", e.ppco2())));
                }
            }
        }
        for (id, l, txt) in ev {
            self.event(Some(id), l, txt);
        }
    }

    /// Ponto de impacto instantâneo dos veículos em subida e terminação autônoma.
    fn range_safety(&mut self) {
        self.iip.clear();
        let corridor = self.corridor.clone();
        let mut terminate = Vec::new();
        for v in &self.vehicles {
            if v.central != Body::Earth || v.launch_time.is_none() || v.kind == Kind::Debris || v.phase.is_final() {
                continue;
            }
            if !matches!(v.phase, Phase::Ascent | Phase::Coast) {
                continue;
            }
            let Some((lat, lon, ti)) = impact_point(v, self.t) else { continue };
            let (cross, inside) = match &corridor {
                Some(c) => {
                    let x = cross_track_km(c.lat, c.lon, c.azimuth, lat, lon);
                    (x, x.abs() <= c.half_width_km)
                }
                None => (0.0, true),
            };
            if !inside && v.fts.installed && v.fts.armed && v.fts.autonomous && v.phase == Phase::Ascent {
                terminate.push(v.id);
            }
            self.iip.push((v.id, Iip { lat, lon, t: ti, cross_km: cross, inside }));
        }
        for id in terminate {
            self.event(Some(id), Level::Alarm, "Terminação autônoma: ponto de impacto fora do corredor");
            self.terminate(id);
        }
    }

    // ───────────────────────── Comandos ─────────────────────────

    /// Recebe um comando da interface. `armed` confirma os perigosos. `at` é a
    /// hora marcada (absoluta), opcional.
    pub fn send(&mut self, vehicle: VehicleId, cmd: Command, armed: bool, at: Option<f64>) -> Result<u64, String> {
        let def = commands::find(&cmd.id).ok_or_else(|| format!("comando desconhecido: {}", cmd.id))?;
        if def.hazardous && !armed {
            return Err("comando perigoso: arme antes de executar".into());
        }
        let v = self.vehicle(vehicle).ok_or("veículo não existe")?.clone();
        if !def.kinds.is_empty() && !def.kinds.contains(&v.kind.id()) {
            return Err(format!("{} não se aplica a {}", def.name, v.kind.name()));
        }
        let id = self.next_seq;
        self.next_seq += 1;
        let mut exec_at = at.unwrap_or(self.t);
        if !def.ground {
            if v.phase == Phase::Destroyed {
                return Err("veículo perdido".into());
            }
            let link = v.systems.as_ref().and_then(|s| s.comms.link.clone());
            match link {
                Some(l) => exec_at = exec_at.max(self.t + l.owlt),
                None => return Err("sem enlace com o veículo".into()),
            }
        }
        let text = cmd.describe();
        let status = if exec_at > self.t + 1e-6 { CmdStatus::Sent } else { CmdStatus::Sent };
        self.commands.push(CmdRecord { id, vehicle, text, sent: self.t, exec_at, status, message: String::new(), cmd: Some(cmd) });
        if self.commands.len() > 500 {
            self.commands.drain(..100);
        }
        self.run_commands();
        Ok(id)
    }

    fn run_commands(&mut self) {
        let due: Vec<usize> = self
            .commands
            .iter()
            .enumerate()
            .filter(|(_, c)| c.status == CmdStatus::Sent && c.exec_at <= self.t + 1e-9)
            .map(|(i, _)| i)
            .collect();
        for i in due {
            let (vid, cmd) = {
                let c = &mut self.commands[i];
                (c.vehicle, c.cmd.clone())
            };
            let Some(cmd) = cmd else { continue };
            let result = self.execute(vid, &cmd);
            let c = &mut self.commands[i];
            match result {
                Ok(msg) => {
                    c.status = CmdStatus::Executed;
                    c.message = msg;
                }
                Err(msg) => {
                    c.status = CmdStatus::Failed;
                    c.message = msg.clone();
                    let text = c.text.clone();
                    self.event(Some(vid), Level::Warn, format!("Comando {} falhou: {}", text, msg));
                }
            }
        }
    }

    /// Executa um comando no veículo.
    pub fn execute(&mut self, vid: VehicleId, cmd: &Command) -> Result<String, String> {
        let t = self.t;
        let idx = self.vehicles.iter().position(|v| v.id == vid).ok_or("veículo não existe")?;
        let on = cmd.str("on").map(|s| s == "on").or(cmd.flag("on"));
        macro_rules! veh {
            () => {
                self.vehicles[idx]
            };
        }
        macro_rules! sys {
            () => {
                self.vehicles[idx].systems.as_mut().ok_or("o veículo não tem esse sistema")?
            };
        }
        let msg: String = match cmd.id.as_str() {
            // ── Lançamento ──
            "count_start" => {
                if veh!().phase != Phase::Prelaunch {
                    return Err("o veículo não está na plataforma".into());
                }
                if self.launch.vehicle != Some(vid) {
                    let tm = cmd.num("t_minus").unwrap_or(3_600.0);
                    let v = veh!().clone();
                    self.launch.setup(&v, tm);
                } else if let Some(tm) = cmd.num("t_minus") {
                    if self.launch.state == CountState::Idle {
                        let v = veh!().clone();
                        self.launch.setup(&v, tm);
                    }
                }
                if self.launch.state == CountState::Hold && self.launch.t_minus <= 600.0 + 1e-6 && !self.launch.all_go() {
                    return Err("há posições NO-GO".into());
                }
                if self.launch.state == CountState::Scrubbed {
                    let v = veh!().clone();
                    self.launch.setup(&v, 3_600.0);
                }
                self.launch.state = CountState::Counting;
                self.launch.hold_reason.clear();
                self.event(Some(vid), Level::Info, format!("Contagem em andamento: T-{:.0} s", self.launch.t_minus));
                "contagem em andamento".into()
            }
            "count_hold" => {
                self.launch.hold("Espera comandada pelo diretor de lançamento");
                self.event(Some(vid), Level::Warn, "Espera comandada");
                "em espera".into()
            }
            "count_recycle" => {
                self.launch.recycle(600.0);
                self.event(Some(vid), Level::Warn, "Contagem volta a T-10 min");
                "recomeço".into()
            }
            "count_scrub" => {
                self.launch.state = CountState::Scrubbed;
                self.launch.loading = false;
                self.launch.hold_reason = "Lançamento cancelado".into();
                for s in veh!().stages.iter_mut().filter(|s| s.cryogenic) {
                    s.propellant = 0.0;
                }
                for e in veh!().stages.iter_mut().flat_map(|s| s.engines.iter_mut()) {
                    if !e.solid {
                        e.state = EngineState::Off;
                        e.spool = 0.0;
                    }
                }
                self.event(Some(vid), Level::Warn, "Lançamento cancelado: tanques esvaziados");
                "cancelado".into()
            }
            "poll" => {
                let v = veh!().clone();
                self.launch.evaluate(&v, &self.weather, true);
                for (l, txt) in self.launch.poll() {
                    self.event(Some(vid), l, txt);
                }
                if self.launch.all_go() { "GO".into() } else { "NO-GO".into() }
            }
            "vote" => {
                let p = cmd.str("position").ok_or("falta a posição")?.to_string();
                let go = match cmd.str("go") {
                    Some("go") => true,
                    Some("nogo") => false,
                    _ => return Err("voto: go ou nogo".into()),
                };
                let ev = self.launch.vote(&p, go)?;
                for (l, txt) in ev {
                    self.event(Some(vid), l, txt);
                }
                if go { "GO".into() } else { "NO-GO".into() }
            }
            "position" => {
                let p = cmd.str("position").ok_or("falta a posição")?.to_string();
                let go = match cmd.str("go") {
                    Some("go") => Some(true),
                    Some("nogo") => Some(false),
                    _ => None,
                };
                let pos = self.launch.position_mut(&p).ok_or("posição desconhecida")?;
                pos.manual = go;
                format!("{}: {}", pos.name, match go { Some(true) => "GO", Some(false) => "NO-GO", None => "automático" })
            }
            "load" => {
                self.launch.loading = on.unwrap_or(true);
                if self.launch.loading { "carregando".into() } else { "carregamento parado".into() }
            }
            "window" => {
                let site = veh!().site.clone().ok_or("sem local de lançamento")?;
                let t0 = if let Some(target) = cmd.str("target").and_then(|s| s.parse::<u32>().ok()).or(cmd.num("target").map(|x| x as u32)) {
                    let tv = self.vehicle(target).ok_or("alvo não existe")?;
                    let n = tv.r.cross(tv.v).unit();
                    let north = veh!().guidance.ascent.as_ref().is_none_or(|a| a.north);
                    let crossings = crate::launch::plane_crossings(&site, n, t + 900.0, 2.0 * 86_400.0, north);
                    // Lança um pouco antes de cruzar o plano (o alvo segue girando).
                    let first = *crossings.first().ok_or("sem cruzamento do plano nas próximas 48 h")?;
                    let el = crate::orbit::elements_from_state(tv.r, tv.v, tv.central.mu());
                    if let Some(a) = &mut self.vehicles[idx].guidance.ascent {
                        a.raan_deg = Some(el.raan.to_degrees());
                        a.inclination_deg = el.i.to_degrees();
                    }
                    first - 240.0
                } else if let Some(s) = cmd.str("utc") {
                    crate::time::parse_utc(s).ok_or("data inválida")?
                } else {
                    return Err("informe um alvo ou uma hora".into());
                };
                if t0 <= t + 60.0 {
                    return Err("a janela já passou".into());
                }
                self.launch.window = Some((t0 - 300.0, t0 + 300.0));
                let v = veh!().clone();
                self.launch.setup(&v, t0 - t);
                self.event(Some(vid), Level::Info, format!("Janela de lançamento: T-0 em {}", crate::time::format_utc(t0)));
                format!("T-0 {}", crate::time::format_utc(t0))
            }
            "fts_arm" => {
                veh!().fts.armed = true;
                "terminação armada".into()
            }
            "fts_safe" => {
                veh!().fts.armed = false;
                "terminação em segurança".into()
            }
            "fts_auto" => {
                veh!().fts.autonomous = on.unwrap_or(true);
                "ok".into()
            }
            "fts_terminate" => {
                if !veh!().fts.installed {
                    return Err("sem sistema de terminação".into());
                }
                if !veh!().fts.armed {
                    return Err("terminação não armada".into());
                }
                self.terminate(vid);
                "voo terminado".into()
            }
            "abort" => {
                let mode = cmd.str("mode").unwrap_or("les").to_string();
                self.abort(vid, &mode)?
            }
            // ── Voo ──
            "stage" => {
                let name = veh!().stages.first().map(|s| s.name.clone()).unwrap_or_default();
                let d = veh!().jettison_bottom(t).ok_or("não há estágio para separar")?;
                self.add_vehicle(d);
                self.event(Some(vid), Level::Info, format!("Separação manual: {}", name));
                "separado".into()
            }
            "ignite" => {
                let mut out = StepOutput::default();
                let ok = veh!().ignite(&mut out);
                for (l, txt) in out.events {
                    self.event(Some(vid), l, txt);
                }
                if !ok {
                    return Err("ignição recusada".into());
                }
                "ignição".into()
            }
            "shutdown" => {
                veh!().shutdown();
                "corte".into()
            }
            "throttle" => {
                let k = cmd.num("value").ok_or("falta a aceleração")? / 100.0;
                veh!().set_throttle(k);
                veh!().guidance.throttle = k;
                format!("{:.0} %", k * 100.0)
            }
            "engine_off" => {
                let n = cmd.num("engine").ok_or("falta o motor")? as usize;
                let stages = veh!().active_stages();
                let e = stages
                    .into_iter()
                    .flat_map(|i| (0..self.vehicles[idx].stages[i].engines.len()).map(move |j| (i, j)))
                    .nth(n.saturating_sub(1))
                    .ok_or("motor não existe")?;
                let eng = &mut veh!().stages[e.0].engines[e.1];
                if eng.solid {
                    return Err("motor sólido não desliga".into());
                }
                eng.state = EngineState::Stopping;
                format!("{} desligado", eng.name)
            }
            "jettison" => {
                let item = cmd.str("item").unwrap_or("fairing");
                if item == "les" {
                    let les = veh!().les.as_mut().ok_or("sem torre de escape")?;
                    if !les.available {
                        return Err("já descartada".into());
                    }
                    les.available = false;
                    "torre descartada".into()
                } else {
                    veh!().drop_item(item).ok_or("peça não existe")?;
                    "descartada".into()
                }
            }
            "payload_sep" => {
                let d = veh!().separate_payload(t).ok_or("nada a separar")?;
                self.add_vehicle(d);
                veh!().guidance.deploy_timer = Some(0.0);
                self.event(Some(vid), Level::Good, "Separação da carga útil");
                "separado".into()
            }
            "auto_stage" => {
                veh!().guidance.auto_stage = on.unwrap_or(true);
                "ok".into()
            }
            // ── Atitude ──
            "att_nudge" => {
                // Pilotagem manual: gira a atitude alvo no referencial do corpo
                // (arfagem em Y, guinada em Z, rolamento em X) e o ADCS persegue.
                if veh!().guidance.program != Program::None {
                    return Err("o guiamento está no controle".into());
                }
                let (p, y, r) = (cmd.num("pitch").unwrap_or(0.0), cmd.num("yaw").unwrap_or(0.0), cmd.num("roll").unwrap_or(0.0));
                let att = veh!().att;
                let s = sys!();
                let base = match s.adcs.mode {
                    AttMode::Inertial(q) => q,
                    _ => att,
                };
                let q = base
                    .mul(Quat::from_axis_angle(Vec3::Y, p.to_radians()))
                    .mul(Quat::from_axis_angle(Vec3::Z, y.to_radians()))
                    .mul(Quat::from_axis_angle(Vec3::X, r.to_radians()))
                    .normalized();
                s.adcs.mode = AttMode::Inertial(q);
                format!("atitude manual: arfagem {p:+.1}°, guinada {y:+.1}°, rolamento {r:+.1}°")
            }
            "att_mode" => {
                let target = cmd.str("target").and_then(|s| s.parse().ok()).or(cmd.num("target").map(|x| x as u32));
                let mode = AttMode::from_id(cmd.str("mode").ok_or("falta o modo")?, target).ok_or("modo inválido")?;
                sys!().adcs.mode = mode;
                if matches!(mode, AttMode::Detumble | AttMode::Free) {
                    // Nada.
                }
                mode.name().into()
            }
            "desat" => {
                let s = sys!();
                if s.adcs.wheels.is_none() {
                    return Err("sem rodas de reação".into());
                }
                s.adcs.desaturating = true;
                "descarregando".into()
            }
            "rcs_pulse" => {
                let dv = cmd.num("dv").ok_or("falta o Δv")?;
                let dir = match cmd.str("dir").unwrap_or("prograde") {
                    "prograde" => Vec3::new(0.0, 1.0, 0.0),
                    "retrograde" => Vec3::new(0.0, -1.0, 0.0),
                    "radial_out" => Vec3::new(1.0, 0.0, 0.0),
                    "radial_in" => Vec3::new(-1.0, 0.0, 0.0),
                    "normal" => Vec3::new(0.0, 0.0, 1.0),
                    _ => Vec3::new(0.0, 0.0, -1.0),
                };
                let id = self.next_node;
                self.next_node += 1;
                veh!().guidance.nodes.push(Node { id, label: "Pulso de RCS".into(), t: t + 5.0, dv_rtn: dir * dv, dir: None, remaining: 0.0, executing: false, done: false, rcs: true, target_energy: None, ignite_at: None });
                "pulso programado".into()
            }
            // ── Manobras ──
            "burn" => {
                let dv = Vec3::new(cmd.num("dv_r").unwrap_or(0.0), cmd.num("dv_t").unwrap_or(0.0), cmd.num("dv_n").unwrap_or(0.0));
                let dt = cmd.num("t").unwrap_or(60.0).max(0.0);
                let label = cmd.str("label").unwrap_or("Queima").to_string();
                self.add_node(vid, label.clone(), t + dt, dv);
                format!("{} em {:.0} s", label, dt)
            }
            "burn_delete" => {
                let n = cmd.num("node").ok_or("falta a queima")? as u32;
                let before = veh!().guidance.nodes.len();
                veh!().guidance.nodes.retain(|x| x.id != n || x.executing);
                if veh!().guidance.nodes.len() == before {
                    return Err("queima não encontrada (ou em execução)".into());
                }
                "apagada".into()
            }
            "burn_clear" => {
                veh!().guidance.nodes.retain(|x| x.executing);
                "fila vazia".into()
            }
            "dock" => {
                let target = cmd.str("target").and_then(|s| s.parse().ok()).or(cmd.num("target").map(|x| x as u32)).ok_or("falta o alvo")?;
                let tv = self.vehicle(target).ok_or("alvo não existe")?;
                let dist = (tv.r - self.vehicles[idx].r).norm();
                if tv.central != self.vehicles[idx].central || dist > 8_000.0 {
                    return Err(format!("alvo a {:.1} km: faça o encontro antes", dist / 1e3));
                }
                let g = &mut veh!().guidance;
                g.dock_target = Some(target);
                g.program = Program::Docking;
                sys!().adcs.mode = AttMode::Guidance;
                self.event(Some(vid), Level::Info, format!("Aproximação automática iniciada a {:.0} m", dist));
                "aproximação".into()
            }
            "undock" => {
                let host = veh!().attached_to.ok_or("não está acoplado")?;
                veh!().attached_to = None;
                veh!().phase = Phase::Orbit;
                let away = -veh!().v.unit() * 0.1;
                veh!().v += away;
                if let Some(h) = self.vehicle_mut(host) {
                    for p in &mut h.ports {
                        if p.occupant == Some(vid) {
                            p.occupant = None;
                        }
                    }
                }
                self.event(Some(vid), Level::Info, "Desacoplado");
                "desacoplado".into()
            }
            "crew_transfer" => {
                let host = veh!().attached_to.ok_or("não está acoplado")?;
                let n = cmd.num("count").unwrap_or(1.0) as u32;
                let have = veh!().crew();
                if n > have {
                    return Err("não há tantos tripulantes".into());
                }
                if let Some(e) = veh!().systems.as_mut().and_then(|s| s.eclss.as_mut()) {
                    e.crew -= n;
                }
                let hv = self.vehicle_mut(host).ok_or("anfitrião sumiu")?;
                let e = hv.systems.as_mut().and_then(|s| s.eclss.as_mut()).ok_or("o anfitrião não tem cabine")?;
                e.crew += n;
                format!("{} tripulantes transferidos", n)
            }
            // ── Energia, térmico, comunicações ──
            "load_switch" => {
                let id = cmd.str("load").ok_or("falta a carga")?.to_string();
                let s = sys!();
                if !s.power.set_load(&id, on.unwrap_or(true)) {
                    return Err("carga não existe".into());
                }
                format!("{} {}", id, if on.unwrap_or(true) { "ligada" } else { "desligada" })
            }
            "solar_deploy" => {
                let s = sys!();
                let sa = s.power.solar.as_mut().ok_or("sem painéis")?;
                sa.deployed = true;
                "painéis abertos".into()
            }
            "fuel_cell" => {
                let s = sys!();
                let fc = s.power.fuel_cell.as_mut().ok_or("sem célula a combustível")?;
                fc.on = on.unwrap_or(true);
                "ok".into()
            }
            "heater" => {
                let node = cmd.str("node").ok_or("falta o nó")?.to_string();
                let mode = cmd.str("mode").unwrap_or("auto").to_string();
                let s = sys!();
                let n = s.thermal.node_mut(&node).ok_or("nó não existe")?;
                match mode.as_str() {
                    "auto" => n.auto = true,
                    "on" => {
                        n.auto = false;
                        n.heater_on = true;
                    }
                    _ => {
                        n.auto = false;
                        n.heater_on = false;
                    }
                }
                format!("aquecedor {} {}", node, mode)
            }
            "radiator" => {
                sys!().thermal.radiator_on = on.unwrap_or(true);
                "ok".into()
            }
            "tx" => {
                sys!().comms.tx_on = on.unwrap_or(true);
                "ok".into()
            }
            "rate" => {
                let s = sys!();
                match cmd.num("kbps") {
                    Some(k) => {
                        s.comms.auto_rate = false;
                        s.comms.rate = k * 1e3;
                    }
                    None => s.comms.auto_rate = true,
                }
                "ok".into()
            }
            "dump" => {
                sys!().comms.dumping = true;
                "descarregando".into()
            }
            "hga_deploy" => {
                sys!().comms.hga_deployed = true;
                "antena aberta".into()
            }
            "instrument" => {
                let id = cmd.str("id").ok_or("falta o instrumento")?.to_string();
                let s = sys!();
                let i = s.instruments.iter_mut().find(|i| i.id == id).ok_or("instrumento não existe")?;
                i.on = on.unwrap_or(true);
                format!("{} {}", i.name, if i.on { "ligado" } else { "desligado" })
            }
            // ── Suporte de vida ──
            "o2_gen" | "scrubber" | "auto_press" => {
                let s = sys!();
                let e = s.eclss.as_mut().ok_or("sem cabine")?;
                let val = on.unwrap_or(true);
                match cmd.id.as_str() {
                    "o2_gen" => e.o2_gen_on = val,
                    "scrubber" => e.scrubber_on = val,
                    _ => e.auto_press = val,
                }
                "ok".into()
            }
            "hatch" => {
                let m = cmd.str("module").ok_or("falta o módulo")?.to_string();
                let closed = cmd.str("closed").map(|s| s == "on").or(cmd.flag("closed")).unwrap_or(true);
                let s = sys!();
                let e = s.eclss.as_mut().ok_or("sem cabine")?;
                let k = e.modules.iter().position(|x| x.name == m).ok_or("módulo não existe")?;
                e.set_isolated(k, closed);
                format!("escotilha de {} {}", m, if closed { "fechada" } else { "aberta" })
            }
            // ── Reentrada ──
            "chute" => {
                let i = cmd.num("index").ok_or("falta o paraquedas")? as usize;
                let action = cmd.str("action").unwrap_or("deploy").to_string();
                let c = veh!().chutes.get_mut(i.saturating_sub(1)).ok_or("paraquedas não existe")?;
                if c.state == ChuteState::Failed {
                    return Err("paraquedas em falha".into());
                }
                c.state = if action == "cut" { ChuteState::Cut } else { ChuteState::Deployed };
                format!("{} {}", c.name, if action == "cut" { "cortado" } else { "aberto" })
            }
            "auto_chutes" => {
                veh!().guidance.auto_chutes = on.unwrap_or(true);
                "ok".into()
            }
            "landing_target" => {
                let lat = cmd.num("lat").ok_or("falta a latitude")?;
                let lon = cmd.num("lon").ok_or("falta a longitude")?;
                veh!().guidance.landing_target = Some((lat, lon));
                format!("alvo {:.2}°, {:.2}°", lat, lon)
            }
            "pdi" => {
                let mut out = StepOutput::default();
                crate::guidance::start_powered_descent(&mut self.vehicles[idx], &mut out)?;
                for (l, txt) in out.events {
                    self.event(Some(vid), l, txt);
                }
                "descida propulsada".into()
            }
            "entry_mode" => {
                let ballistic = cmd.str("mode") == Some("ballistic");
                veh!().guidance.ballistic = ballistic;
                veh!().guidance.entry_guided = !ballistic;
                "ok".into()
            }
            "gear" => {
                veh!().gear_down = true;
                "trem baixado".into()
            }
            "safe_mode" => {
                let val = on.unwrap_or(true);
                veh!().safe_mode = val;
                if val {
                    let kind = veh!().kind;
                    let s = sys!();
                    s.power.shed();
                    for i in &mut s.instruments {
                        i.on = false;
                    }
                    s.adcs.mode = if kind == Kind::Probe { AttMode::EarthPoint } else { AttMode::SunPoint };
                } else {
                    for l in &mut sys!().power.loads {
                        l.on = true;
                    }
                }
                if val { "modo de segurança".into() } else { "operação normal".into() }
            }
            other => return Err(format!("comando sem implementação: {other}")),
        };
        Ok(msg)
    }

    pub fn add_node(&mut self, vid: VehicleId, label: String, t: f64, dv_rtn: Vec3) -> Option<u32> {
        let id = self.next_node;
        self.next_node += 1;
        let v = self.vehicle_mut(vid)?;
        v.guidance.nodes.push(Node {
            id,
            label,
            t,
            dv_rtn,
            dir: None,
            remaining: 0.0,
            executing: false,
            done: false,
            rcs: dv_rtn.norm() < 2.0 && v.systems.as_ref().is_some_and(|s| s.adcs.rcs_force > 0.0),
            target_energy: None,
            ignite_at: None,
        });
        v.guidance.nodes.sort_by(|a, b| a.t.total_cmp(&b.t));
        Some(id)
    }

    /// Acrescenta um plano do planejador (tempos relativos a agora).
    pub fn add_plan(&mut self, vid: VehicleId, plan: &crate::planner::Plan) -> Vec<u32> {
        let t = self.t;
        let mut ids = Vec::new();
        for n in &plan.nodes {
            if let Some(id) = self.add_node(vid, n.label.clone(), t + n.t, n.dv_rtn) {
                ids.push(id);
                // Corte por energia e ignição escolhida passam junto.
                if let Some(node) = self.vehicle_mut(vid).and_then(|v| v.guidance.nodes.iter_mut().find(|x| x.id == id)) {
                    node.target_energy = n.target_energy;
                    node.ignite_at = n.ignite_at.map(|x| x + t);
                }
            }
        }
        if let Some(l) = plan.landing {
            if let Some(v) = self.vehicle_mut(vid) {
                if v.guidance.landing_target.is_none() {
                    v.guidance.landing_target = Some(l);
                }
            }
        }
        ids
    }

    fn dock(&mut self, id: VehicleId, target: VehicleId) {
        let name = self.vehicle(target).map(|v| v.name.clone()).unwrap_or_default();
        let mut port_name = String::from("porta");
        if let Some(tv) = self.vehicle_mut(target) {
            if let Some(p) = tv.ports.iter_mut().find(|p| p.occupant.is_none()) {
                p.occupant = Some(id);
                port_name = p.name.clone();
            }
        }
        if let Some(v) = self.vehicle_mut(id) {
            v.attached_to = Some(target);
            v.phase = Phase::Docked;
            v.guidance.rcs_accel = Vec3::ZERO;
            if let Some(s) = &mut v.systems {
                s.adcs.mode = AttMode::Free;
            }
        }
        self.event(Some(id), Level::Good, format!("Captura e acoplamento: {} ({})", name, port_name));
    }

    /// Terminação de voo: com tripulação e escape disponível, aborta antes.
    pub fn terminate(&mut self, vid: VehicleId) {
        let crewed_les = self.vehicle(vid).is_some_and(|v| v.crew() > 0 && v.les.as_ref().is_some_and(|l| l.available));
        if crewed_les {
            let _ = self.abort(vid, "les");
            // O que sobrou do lançador é o detrito criado no aborto; destrói-o.
            if let Some(d) = self.vehicles.iter_mut().rev().find(|d| d.parent == Some(vid) && d.kind == Kind::Debris) {
                d.phase = Phase::Destroyed;
            }
        } else if let Some(v) = self.vehicle_mut(vid) {
            v.phase = Phase::Destroyed;
        }
        self.event(Some(vid), Level::Alarm, "TERMINAÇÃO DE VOO executada");
    }

    /// Abortos da subida.
    pub fn abort(&mut self, vid: VehicleId, mode: &str) -> Result<String, String> {
        let t = self.t;
        let v = self.vehicle_mut(vid).ok_or("veículo não existe")?;
        let msg = match mode {
            "les" => {
                let les = v.les.clone().ok_or("sem sistema de escape")?;
                if !les.available {
                    return Err("sistema de escape já descartado".into());
                }
                // Larga tudo abaixo da cápsula.
                let n = v.stages.len() - 1;
                if n == 0 {
                    return Err("nada a separar".into());
                }
                let below: Vec<Stage> = v.stages.drain(..n).collect();
                let mut d = v.clone();
                d.kind = Kind::Debris;
                d.name = format!("{} — lançador abortado", v.name);
                d.stages = below;
                d.initial_stages = d.stages.len();
                d.systems = None;
                d.les = None;
                d.heat_shield = None;
                d.chutes.clear();
                d.landing = LandingMode::None;
                d.guidance = Guidance { auto_stage: false, auto_chutes: false, ..Guidance::default() };
                d.parent = Some(v.id);
                d.items.clear();
                for e in d.stages.iter_mut().flat_map(|s| s.engines.iter_mut()) {
                    if !e.solid {
                        e.state = EngineState::Stopping;
                    }
                }
                if v.phase == Phase::Prelaunch {
                    v.phase = Phase::Ascent;
                    v.launch_time = Some(t);
                    v.released = true;
                    d.phase = Phase::Prelaunch;
                    d.released = false;
                }
                v.items.retain(|i| i.id != "fairing");
                if let Some(l) = &mut v.les {
                    l.firing = l.burn;
                }
                // Escape para cima e um pouco para o lado do mar.
                let up = v.r.unit();
                let east = Vec3::Z.cross(up).unit();
                v.guidance.att_cmd = Some(crate::math::Quat::look((up * 0.94 + east * 0.34).unit(), -up));
                v.guidance.program = Program::None;
                v.guidance.ascent = None;
                v.guidance.inserted = true;
                v.guidance.nodes.clear();
                if let Some(s) = &mut v.systems {
                    s.adcs.mode = AttMode::Guidance;
                }
                v.guidance.landing_target = None;
                self.add_vehicle(d);
                self.launch.state = CountState::Scrubbed;
                "sistema de escape disparado"
            }
            "separate" => {
                if v.les.as_ref().is_some_and(|l| l.available) && v.phase == Phase::Ascent {
                    return Err("use o sistema de escape enquanto ele está disponível".into());
                }
                v.shutdown();
                let d = v.separate_payload(t).ok_or("nada a separar")?;
                v.guidance.program = Program::None;
                v.guidance.ascent = None;
                v.guidance.inserted = true;
                v.guidance.landing_target = None;
                self.add_vehicle(d);
                "nave separada: reentrada balística"
            }
            "ato" | "aoa" | "tal" => {
                let a = v.guidance.ascent.as_mut().ok_or("sem subida em curso")?;
                if v.guidance.inserted {
                    return Err("já em órbita".into());
                }
                match mode {
                    "ato" => {
                        a.target_pe_km = 160.0;
                        a.target_ap_km = 200.0;
                        "aborto para órbita: alvo 160 × 200 km"
                    }
                    "aoa" => {
                        a.target_pe_km = 110.0;
                        a.target_ap_km = 110.0;
                        a.payload_sep = false;
                        // Corte cedo: suborbital, uma volta e reentrada.
                        "uma volta e reentrada"
                    }
                    _ => {
                        a.target_pe_km = 100.0;
                        a.target_ap_km = 100.0;
                        v.guidance.landing_target = Some((41.67, -1.04));
                        "pouso transoceânico: alvo Saragoça"
                    }
                }
            }
            "rtls" => {
                if v.kind != Kind::Shuttle {
                    return Err("RTLS é do ônibus espacial".into());
                }
                if v.stages.first().is_some_and(|s| s.boosters) {
                    return Err("espere a separação dos foguetes sólidos".into());
                }
                v.guidance.program = Program::Rtls;
                v.guidance.ascent = None;
                v.guidance.inserted = true;
                "retorno ao local de lançamento"
            }
            _ => return Err("modo de aborto desconhecido".into()),
        };
        self.event(Some(vid), Level::Alarm, format!("ABORTO: {}", msg));
        Ok(msg.into())
    }

    // ───────────────────────── Falhas ─────────────────────────

    /// Injeta uma falha (instrutor).
    pub fn inject(&mut self, vid: VehicleId, fault: &str, arg: Option<&str>) -> Result<String, String> {
        let t = self.t;
        let _ = t;
        let msg: String = match fault {
            "range_fouled" => {
                self.launch.range_fouled = 600.0;
                "barco na área".into()
            }
            "ground_fault" => {
                self.launch.ground_fault = 900.0;
                "falha de solo".into()
            }
            "storm" => {
                self.weather.lightning_km = 6.0;
                self.weather.surface_wind = 18.0;
                "tempestade".into()
            }
            _ => {
                let v = self.vehicle_mut(vid).ok_or("veículo não existe")?;
                let n: usize = arg.and_then(|s| s.parse().ok()).unwrap_or(1);
                match fault {
                    "engine" => {
                        let stages = v.active_stages();
                        let mut list: Vec<(usize, usize)> = stages.into_iter().flat_map(|i| (0..v.stages[i].engines.len()).map(move |j| (i, j))).collect();
                        if list.is_empty() {
                            return Err("sem motores".into());
                        }
                        let (i, j) = list.remove((n.max(1) - 1).min(list.len() - 1));
                        let e = &mut v.stages[i].engines[j];
                        e.state = EngineState::Failed;
                        e.spool = 0.0;
                        e.thrust = 0.0;
                        format!("falha no motor {}", e.name)
                    }
                    "leak" => {
                        let s = v.stages.first_mut().ok_or("sem estágio")?;
                        s.leak = (s.capacity * 0.002).max(0.05);
                        format!("vazamento em {}", s.name)
                    }
                    "battery_cell" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        s.power.failed_cells += 1;
                        "célula da bateria perdida".into()
                    }
                    "solar_stuck" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        s.power.solar.as_mut().ok_or("sem painéis")?.stuck = true;
                        "mecanismo do painel travado".into()
                    }
                    "solar_degrade" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        s.power.solar.as_mut().ok_or("sem painéis")?.degraded = 0.5;
                        "metade de uma asa solar perdida".into()
                    }
                    "wheel" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        let w = s.adcs.wheels.as_mut().ok_or("sem rodas")?;
                        w.failed[(n.max(1) - 1).min(2)] = true;
                        format!("roda de reação do eixo {} falhou", ["X", "Y", "Z"][(n.max(1) - 1).min(2)])
                    }
                    "gyro" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        s.adcs.gyro_bias = Vec3::new(0.05f64.to_radians(), -0.03f64.to_radians(), 0.0);
                        "viés nos girômetros".into()
                    }
                    "star_tracker" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        s.adcs.star_tracker = false;
                        "rastreador de estrelas cego".into()
                    }
                    "comms" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        s.comms.failed = true;
                        "transmissor em pane".into()
                    }
                    "heater" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        let node = s.thermal.nodes.iter_mut().find(|x| x.heater_w > 0.0).ok_or("sem aquecedores")?;
                        node.heater_failed = true;
                        format!("aquecedor de {} falhou", node.name)
                    }
                    "cabin_leak" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        let e = s.eclss.as_mut().ok_or("sem cabine")?;
                        let k = (n.max(1) - 1).min(e.modules.len().saturating_sub(1));
                        let m = e.modules.get_mut(k).ok_or("sem módulos")?;
                        m.leak_area = 3e-4;
                        format!("vazamento em {}", m.name)
                    }
                    "scrubber" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        let e = s.eclss.as_mut().ok_or("sem cabine")?;
                        e.scrubber_rate *= 0.2;
                        "remoção de CO₂ degradada".into()
                    }
                    "chute" => {
                        let k = (n.max(1) - 1).min(v.chutes.len().saturating_sub(1));
                        let c = v.chutes.get_mut(k).ok_or("sem paraquedas")?;
                        c.state = ChuteState::Failed;
                        format!("{} em falha", c.name)
                    }
                    "fuel_cell" => {
                        let s = v.systems.as_mut().ok_or("sem sistemas")?;
                        s.power.fuel_cell.as_mut().ok_or("sem célula")?.max_w *= 0.4;
                        "célula a combustível degradada".into()
                    }
                    _ => return Err("falha desconhecida".into()),
                }
            }
        };
        if let Some(v) = self.vehicle_mut(vid) {
            v.faults.push(fault.into());
        }
        if !self.quiet_fault {
            self.event(Some(vid), Level::Warn, format!("[Instrutor] Falha injetada: {}", msg));
        }
        Ok(msg)
    }

    /// Desfaz todas as falhas injetadas de um veículo (o que dá para desfazer).
    pub fn clear_faults(&mut self, vid: VehicleId) {
        if let Some(v) = self.vehicle_mut(vid) {
            v.faults.clear();
            for s in &mut v.stages {
                s.leak = 0.0;
                for e in &mut s.engines {
                    if e.state == EngineState::Failed {
                        e.state = EngineState::Off;
                    }
                }
            }
            for c in &mut v.chutes {
                if c.state == ChuteState::Failed {
                    c.state = ChuteState::Stowed;
                }
            }
            if let Some(s) = &mut v.systems {
                s.power.failed_cells = 0;
                if let Some(sa) = &mut s.power.solar {
                    sa.stuck = false;
                    sa.degraded = 0.0;
                }
                if let Some(w) = &mut s.adcs.wheels {
                    w.failed = [false; 3];
                }
                s.adcs.gyro_bias = Vec3::ZERO;
                s.adcs.star_tracker = true;
                s.comms.failed = false;
                for n in &mut s.thermal.nodes {
                    n.heater_failed = false;
                }
                if let Some(e) = &mut s.eclss {
                    for m in &mut e.modules {
                        m.leak_area = 0.0;
                    }
                }
            }
        }
        self.launch.range_fouled = 0.0;
        self.launch.ground_fault = 0.0;
        self.event(Some(vid), Level::Info, "[Instrutor] Falhas removidas");
    }

    /// Posições futuras (para o traço no mapa): pontos (t, lat, lon, alt) e o
    /// estado inercial, por propagação de dois corpos com J2.
    pub fn predict(&self, vid: VehicleId, span: f64, step: f64) -> Vec<(f64, f64, f64, f64, Vec3)> {
        if let Some(s) = self.tracked.iter().find(|s| s.id == vid) {
            return s
                .samples(self.t, span, step)
                .into_iter()
                .map(|(t, r)| {
                    let (lat, lon, alt) = Body::Earth.to_geodetic(r, t);
                    (t, lat, lon, alt, r)
                })
                .collect();
        }
        let Some(v) = self.vehicle(vid) else { return vec![] };
        if v.phase.is_final() {
            return vec![];
        }
        let samples = if v.central == Body::Sun || v.r.norm() > 2e9 {
            // Heliocêntrico: Kepler puro em passos.
            let mut out = Vec::new();
            let mut tt = 0.0;
            while tt <= span {
                let (r, _) = propagate_kepler(v.r, v.v, tt, v.central.mu());
                out.push((self.t + tt, r));
                tt += step;
            }
            out
        } else {
            ground::propagate_samples(v.central, v.r, v.v, self.t, span, step)
        };
        samples
            .into_iter()
            .map(|(t, r)| {
                let (lat, lon, alt) = v.central.to_geodetic(r, t);
                (t, lat, lon, alt, r)
            })
            .collect()
    }

    pub fn burn_vector(&self, vid: VehicleId, node: &Node) -> Option<Vec3> {
        let v = self.vehicle(vid)?;
        let (r, vel) = propagate_kepler(v.r, v.v, node.t - self.t, v.central.mu());
        Some(rtn_to_inertial(r, vel, node.dv_rtn))
    }

    /// Modos de aborto disponíveis agora.
    pub fn abort_modes(&self, vid: VehicleId) -> Vec<(&'static str, bool)> {
        let Some(v) = self.vehicle(vid) else { return vec![] };
        let ascending = matches!(v.phase, Phase::Ascent | Phase::Prelaunch) && !v.guidance.inserted;
        let les = v.les.as_ref().is_some_and(|l| l.available);
        let mut out = vec![];
        if v.les.is_some() {
            out.push(("les", ascending && les));
            out.push(("separate", ascending && !les && v.stages.iter().any(|s| !s.spacecraft)));
        }
        if v.kind == Kind::Shuttle {
            let boosters = v.stages.first().is_some_and(|s| s.boosters);
            let speed = v.derived.hspeed;
            out.push(("rtls", ascending && v.phase == Phase::Ascent && !boosters && speed < 2_300.0));
            out.push(("tal", ascending && v.phase == Phase::Ascent && speed > 1_500.0 && speed < 6_000.0));
            out.push(("aoa", ascending && v.phase == Phase::Ascent && speed > 5_500.0));
            out.push(("ato", ascending && v.phase == Phase::Ascent && speed > 5_000.0));
        } else if v.guidance.ascent.is_some() && v.crew() > 0 {
            out.push(("ato", ascending && v.phase == Phase::Ascent && v.derived.hspeed > 5_000.0));
        }
        out
    }
}

fn ignite_liquids(v: &mut Vehicle) {
    for i in v.active_stages() {
        for e in &mut v.stages[i].engines {
            if !e.solid && e.state == EngineState::Off && e.ignitions > 0 {
                e.ignitions -= 1;
                e.state = EngineState::Starting;
                e.spool = 0.0;
            }
        }
    }
}

/// Onde o veículo cairia agora sem propulsão nem arrasto (Terra).
pub fn impact_point(v: &Vehicle, t: f64) -> Option<(f64, f64, f64)> {
    let mu = v.central.mu();
    let el = crate::orbit::elements_from_state(v.r, v.v, mu);
    if el.periapsis() > v.central.radius() + 30e3 {
        return None;
    }
    let alt = |dt: f64| {
        let (r, _) = propagate_kepler(v.r, v.v, dt, mu);
        v.central.altitude(r)
    };
    // Procura o primeiro instante abaixo do solo, de 10 em 10 s.
    let mut prev = 0.0;
    let mut dt = 10.0;
    while dt < 7_200.0 {
        if alt(dt) < 0.0 {
            let (mut a, mut b) = (prev, dt);
            for _ in 0..40 {
                let m = 0.5 * (a + b);
                if alt(m) < 0.0 { b = m } else { a = m }
            }
            let (r, _) = propagate_kepler(v.r, v.v, b, mu);
            let (lat, lon, _) = v.central.to_geodetic(r, t + b);
            return Some((lat, lon, t + b));
        }
        prev = dt;
        dt += 10.0;
    }
    None
}

/// Distância (km) de um ponto à trajetória de grande círculo que sai de
/// (lat0, lon0) no rumo `az`; positiva à direita.
pub fn cross_track_km(lat0: f64, lon0: f64, az: f64, lat: f64, lon: f64) -> f64 {
    let r = 6_371.0;
    let d13 = great_circle_km(lat0, lon0, lat, lon) / r;
    let th13 = bearing_deg(lat0, lon0, lat, lon).to_radians();
    let th12 = az.to_radians();
    (d13.sin() * (th13 - th12).sin()).asin() * r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distancia_lateral() {
        // Ponto a 1° para o norte de uma trajetória para o leste: ~111 km à esquerda.
        let x = cross_track_km(0.0, 0.0, 90.0, 1.0, 5.0);
        assert!((x + 111.0).abs() < 2.0, "{x}");
    }
}
