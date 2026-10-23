//! O veículo: uma pilha de estágios (de baixo para cima) cujo último é a nave
//! propriamente dita, com seus subsistemas. O mesmo modelo serve ao foguete de
//! sondagem, ao lançador com satélite, à cápsula tripulada, ao ônibus espacial,
//! à estação e à sonda; estágios largados viram detritos com vida própria.

use crate::aero::{self, Shape};
use crate::atmosphere::{self, Air, P0};
use crate::bodies::Body;
use crate::math::{G0, Quat, Vec3};
use crate::orbit::{Elements, elements_from_state};
use crate::subsystems::{Adcs, AttMode, Comms, Eclss, Power, Thermal};
use serde::{Deserialize, Serialize};

pub type VehicleId = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    SoundingRocket,
    Satellite,
    Capsule,
    Shuttle,
    Station,
    Probe,
    /// Estágio largado, coifa, módulo descartado.
    Debris,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::SoundingRocket => "Foguete de sondagem",
            Kind::Satellite => "Satélite",
            Kind::Capsule => "Cápsula",
            Kind::Shuttle => "Ônibus espacial",
            Kind::Station => "Estação espacial",
            Kind::Probe => "Sonda",
            Kind::Debris => "Detrito",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Kind::SoundingRocket => "sounding",
            Kind::Satellite => "satellite",
            Kind::Capsule => "capsule",
            Kind::Shuttle => "shuttle",
            Kind::Station => "station",
            Kind::Probe => "probe",
            Kind::Debris => "debris",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Prelaunch,
    Ascent,
    /// Balística, fora de órbita (inclusive um foguete de sondagem subindo).
    Coast,
    Orbit,
    /// Órbita aberta (hipérbole), saindo da esfera de influência.
    Escape,
    Entry,
    /// Sob paraquedas ou planando em baixa velocidade.
    Descent,
    Landed,
    Splashdown,
    Docked,
    Destroyed,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Phase::Prelaunch => "Pré-lançamento",
            Phase::Ascent => "Subida",
            Phase::Coast => "Voo balístico",
            Phase::Orbit => "Órbita",
            Phase::Escape => "Escape",
            Phase::Entry => "Reentrada",
            Phase::Descent => "Descida",
            Phase::Landed => "Pousado",
            Phase::Splashdown => "Amerissado",
            Phase::Docked => "Acoplado",
            Phase::Destroyed => "Perdido",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            Phase::Prelaunch => "prelaunch",
            Phase::Ascent => "ascent",
            Phase::Coast => "coast",
            Phase::Orbit => "orbit",
            Phase::Escape => "escape",
            Phase::Entry => "entry",
            Phase::Descent => "descent",
            Phase::Landed => "landed",
            Phase::Splashdown => "splashdown",
            Phase::Docked => "docked",
            Phase::Destroyed => "destroyed",
        }
    }
    pub fn is_final(self) -> bool {
        matches!(self, Phase::Landed | Phase::Splashdown | Phase::Destroyed)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineState {
    #[default]
    Off,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Engine {
    pub name: String,
    /// Empuxo no vácuo (N).
    pub thrust_vac: f64,
    pub isp_vac: f64,
    pub isp_sl: f64,
    /// Menor aceleração do motor (fração); 1 se não regula.
    pub min_throttle: f64,
    /// Motor sólido: uma vez aceso, queima até acabar.
    pub solid: bool,
    /// Ignições que restam.
    pub ignitions: u32,
    /// Pressão de câmara nominal (bar).
    pub pc_nominal: f64,
    #[serde(default)]
    pub state: EngineState,
    /// Rampa de partida/parada (0..1).
    #[serde(default)]
    pub spool: f64,
    /// Aceleração comandada (0..1).
    #[serde(default = "one")]
    pub throttle: f64,
    #[serde(default)]
    pub burn_time: f64,
    /// Empuxo do último passo (N).
    #[serde(default)]
    pub thrust: f64,
}

fn one() -> f64 {
    1.0
}

impl Engine {
    pub fn mdot_max(&self) -> f64 {
        self.thrust_vac / (self.isp_vac * G0)
    }
    /// Área de saída do bocal, que tira empuxo ao nível do mar.
    fn exit_area(&self) -> f64 {
        let f_sl = self.mdot_max() * self.isp_sl * G0;
        (self.thrust_vac - f_sl) / P0
    }
    pub fn lit(&self) -> bool {
        matches!(self.state, EngineState::Starting | EngineState::Running | EngineState::Stopping)
    }
    /// Empuxo e vazão agora, com a pressão ambiente dada.
    pub fn output(&self, p_amb: f64) -> (f64, f64) {
        if !self.lit() {
            return (0.0, 0.0);
        }
        let k = self.spool * if self.solid { 1.0 } else { self.throttle };
        let mdot = self.mdot_max() * k;
        let thrust = (self.thrust_vac * k - p_amb * self.exit_area() * self.spool).max(0.0);
        (thrust, mdot)
    }
    pub fn pc(&self) -> f64 {
        if self.lit() {
            self.pc_nominal * self.spool * if self.solid { 1.0 } else { self.throttle }
        } else {
            0.0
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stage {
    pub name: String,
    pub dry_mass: f64,
    pub propellant: f64,
    pub capacity: f64,
    pub engines: Vec<Engine>,
    /// Boosters laterais: queimam junto com o estágio de cima.
    #[serde(default)]
    pub boosters: bool,
    pub diameter: f64,
    #[serde(default)]
    pub length: f64,
    /// Propelente criogênico: evapora parado no solo.
    #[serde(default)]
    pub cryogenic: bool,
    /// Faz parte da nave (não do lançador): fica depois da separação da carga.
    #[serde(default)]
    pub spacecraft: bool,
    /// Vazamento de propelente (kg/s), por falha.
    #[serde(default)]
    pub leak: f64,
    /// Pressão do tanque (bar) nominal.
    #[serde(default)]
    pub tank_bar: f64,
}

impl Stage {
    pub fn mass(&self) -> f64 {
        self.dry_mass + self.propellant
    }
    pub fn thrust_vac(&self) -> f64 {
        self.engines.iter().filter(|e| e.state != EngineState::Failed).map(|e| e.thrust_vac).sum()
    }
    pub fn isp_vac(&self) -> f64 {
        let f: f64 = self.engines.iter().map(|e| e.thrust_vac).sum();
        let m: f64 = self.engines.iter().map(|e| e.mdot_max()).sum();
        if m > 0.0 { f / m / G0 } else { 0.0 }
    }
    pub fn any_lit(&self) -> bool {
        self.engines.iter().any(|e| e.lit())
    }
    pub fn prop_fraction(&self) -> f64 {
        if self.capacity > 0.0 { self.propellant / self.capacity } else { 0.0 }
    }
}

/// Peça descartável que não é estágio: coifa, torre de escape, tampa.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub name: String,
    pub mass: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChuteState {
    #[default]
    Stowed,
    Deployed,
    Cut,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Chute {
    pub name: String,
    /// Cd·A total (m²).
    pub cda: f64,
    pub deploy_alt: f64,
    /// Abre só abaixo deste Mach.
    pub max_mach: f64,
    /// Drogue: cortado quando o principal abre.
    pub drogue: bool,
    #[serde(default)]
    pub state: ChuteState,
    #[serde(default)]
    pub inflation: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HeatShield {
    pub ablator_kg: f64,
    pub initial_kg: f64,
    /// Calor de ablação (J/kg).
    pub ablation_heat: f64,
    /// Área do escudo (m²).
    pub area: f64,
    #[serde(default)]
    pub temp: f64,
    #[serde(default)]
    pub flux: f64,
    #[serde(default)]
    pub peak_flux: f64,
    #[serde(default)]
    pub heat_load: f64,
}

/// Sistema de escape (torre ou motores de aborto).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Les {
    pub thrust: f64,
    pub burn: f64,
    /// Massa da torre (zero se os motores ficam na cápsula).
    pub tower_mass: f64,
    pub available: bool,
    #[serde(default)]
    pub firing: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LandingMode {
    /// Amerissagem sob paraquedas.
    Splash,
    /// Pista, com trem de pouso (ônibus espacial).
    Runway,
    /// Sem pouso previsto.
    None,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Port {
    pub name: String,
    /// Distância do centro, ao longo da direção de aproximação (m).
    pub offset: f64,
    pub occupant: Option<VehicleId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Instrument {
    pub id: String,
    pub name: String,
    pub watts: f64,
    /// Dados gerados (Mbit/s) ligado.
    pub mbps: f64,
    pub on: bool,
}

/// Os subsistemas de uma nave (os detritos não têm).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Systems {
    pub power: Power,
    pub thermal: Thermal,
    pub adcs: Adcs,
    pub comms: Comms,
    pub eclss: Option<Eclss>,
    pub instruments: Vec<Instrument>,
}

/// Local de lançamento.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Site {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

/// Perfil de subida.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AscentProfile {
    /// Periastro e apoastro alvo (km). A inserção é no periastro.
    pub target_pe_km: f64,
    pub target_ap_km: f64,
    pub inclination_deg: f64,
    /// Lança para o norte (true) ou para o sul.
    pub north: bool,
    /// Nodo ascendente alvo (graus), para encontro; livre se None.
    #[serde(default)]
    pub raan_deg: Option<f64>,
    /// Programa de arfagem: velocidade em que começa a curva (m/s), altitude em
    /// que termina (km), arfagem final (graus sobre o horizonte) e o expoente.
    pub turn_start_speed: f64,
    pub turn_end_km: f64,
    pub end_pitch_deg: f64,
    pub turn_shape: f64,
    /// Índice do estágio (na pilha original) em que o guiamento fecha a malha.
    pub closed_loop_stage: usize,
    /// Aceleração máxima (g).
    pub max_g: f64,
    /// Redução de empuxo na pressão dinâmica máxima: q (kPa) e fração.
    #[serde(default)]
    pub throttle_bucket: Option<(f64, f64)>,
    /// Separa a carga útil depois da inserção.
    pub payload_sep: bool,
    /// Estágio (índice na pilha original) aceso no apogeu, na horizontal:
    /// lançadores sólidos sobem em balística e circularizam com ele.
    #[serde(default)]
    pub kick_stage: Option<usize>,
}

/// Perfil de foguete de sondagem: trilho e rotação.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundingProfile {
    pub elevation_deg: f64,
    pub azimuth_deg: f64,
    /// Espera entre o fim do 1º estágio e a ignição do 2º (s).
    pub stage2_delay: f64,
}

/// Uma manobra planejada: Δv no referencial RTN em torno de um instante.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: u32,
    pub label: String,
    /// Centro da queima.
    pub t: f64,
    pub dv_rtn: Vec3,
    /// Direção inercial congelada na ignição.
    #[serde(default)]
    pub dir: Option<Vec3>,
    #[serde(default)]
    pub remaining: f64,
    #[serde(default)]
    pub executing: bool,
    #[serde(default)]
    pub done: bool,
    /// Usa só o RCS (Δv pequeno).
    #[serde(default)]
    pub rcs: bool,
    /// Corta quando a energia específica (J/kg) chegar a este valor, em vez de
    /// contar o Δv: queimas longas de escape.
    #[serde(default)]
    pub target_energy: Option<f64>,
    /// Instante de ignição escolhido pelo planejador (senão, centrado em `t`).
    #[serde(default)]
    pub ignite_at: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Program {
    None,
    Ascent,
    Sounding,
    Entry,
    Docking,
    /// Retorno ao local de lançamento (aborto do ônibus espacial).
    Rtls,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Guidance {
    pub program: Program,
    pub ascent: Option<AscentProfile>,
    pub sounding: Option<SoundingProfile>,
    /// Normal do plano alvo (inercial), fixada no lançamento.
    #[serde(default)]
    pub plane: Option<Vec3>,
    /// Azimute de lançamento relativo ao solo (graus), para o programa de arfagem.
    #[serde(default)]
    pub azimuth: Option<f64>,
    /// Altitude em que a curva começou.
    #[serde(default)]
    pub turn_start_alt: Option<f64>,
    /// L/D comandado (aproximação do ônibus espacial), em vez do da tabela.
    #[serde(default)]
    pub ld_cmd: Option<f64>,
    /// Reentrada balística (cápsula girando, sem sustentação líquida).
    #[serde(default)]
    pub ballistic: bool,
    /// Relógio depois da separação da carga (ativação automática do satélite).
    #[serde(default)]
    pub deploy_timer: Option<f64>,
    /// Separa a carga quando as queimas do estágio superior terminarem.
    #[serde(default)]
    pub payload_after_burns: bool,
    /// Esperando o apogeu para acender o motor de apogeu.
    #[serde(default)]
    pub kick_wait: bool,
    /// Malha fechada ativa.
    #[serde(default)]
    pub closed_loop: bool,
    pub nodes: Vec<Node>,
    /// Local de pouso alvo (lat, lon).
    pub landing_target: Option<(f64, f64)>,
    /// Ângulo de rolamento na reentrada (graus).
    pub bank_deg: f64,
    #[serde(default)]
    pub entry_guided: bool,
    #[serde(default)]
    pub next_entry_update: f64,
    pub dock_target: Option<VehicleId>,
    pub auto_stage: bool,
    pub auto_chutes: bool,
    /// Aceleração manual (0..1) para os motores acesos fora do guiamento.
    pub throttle: f64,
    /// Relógio do sequenciador de estágios (s desde o corte).
    #[serde(default)]
    pub stage_timer: Option<f64>,
    /// Estágio a acender quando o relógio chegar.
    #[serde(default)]
    pub pending_ignition: bool,
    /// Inserção feita (SECO).
    #[serde(default)]
    pub inserted: bool,
    #[serde(default)]
    pub payload_timer: Option<f64>,
    /// Atitude comandada pelo guiamento.
    #[serde(default)]
    pub att_cmd: Option<Quat>,
    /// Translação comandada pelo RCS (m/s², inercial).
    #[serde(default)]
    pub rcs_accel: Vec3,
    /// Tempo para o fim da queima estimado pelo guiamento (s).
    #[serde(default)]
    pub t_go: f64,
}

impl Default for Guidance {
    fn default() -> Self {
        Guidance {
            program: Program::None,
            ascent: None,
            sounding: None,
            plane: None,
            azimuth: None,
            turn_start_alt: None,
            ld_cmd: None,
            ballistic: false,
            deploy_timer: None,
            payload_after_burns: false,
            kick_wait: false,
            closed_loop: false,
            nodes: Vec::new(),
            landing_target: None,
            bank_deg: 0.0,
            entry_guided: true,
            next_entry_update: 0.0,
            dock_target: None,
            auto_stage: true,
            auto_chutes: true,
            throttle: 1.0,
            stage_timer: None,
            pending_ignition: false,
            inserted: false,
            payload_timer: None,
            att_cmd: None,
            rcs_accel: Vec3::ZERO,
            t_go: 0.0,
        }
    }
}

/// Sistema de terminação de voo (segurança de voo).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Fts {
    pub installed: bool,
    pub armed: bool,
    /// Termina sozinho se o ponto de impacto sair do corredor.
    pub autonomous: bool,
}

/// Grandezas derivadas, recalculadas a cada passo.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Derived {
    pub alt: f64,
    pub lat: f64,
    pub lon: f64,
    pub speed: f64,
    pub vspeed: f64,
    pub hspeed: f64,
    pub air_speed: f64,
    pub mach: f64,
    pub q: f64,
    pub accel_g: f64,
    pub density: f64,
    pub pitch_deg: f64,
    pub heading_deg: f64,
    pub aoa_deg: f64,
    pub thrust: f64,
    pub mass: f64,
    pub downrange_km: f64,
    pub elements: Elements,
    pub sunlit: f64,
    pub sun_dir: Vec3,
    pub sun_dist: f64,
    pub twr: f64,
    pub dv_remaining: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Stats {
    pub max_q: f64,
    pub max_q_met: f64,
    pub max_g: f64,
    pub max_alt: f64,
    pub max_speed: f64,
    pub passed_max_q: bool,
    pub passed_mach1: bool,
    pub entry_interface: bool,
    pub microgravity_start: Option<f64>,
    pub apogee_reported: bool,
    pub blackout: bool,
    /// Calor acumulado sem escudo (J/m²).
    pub heat_load: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Level {
    Info,
    Good,
    Warn,
    Alarm,
    /// Conversa nos loops da sala de controle.
    Chat,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub t: f64,
    pub vehicle: Option<VehicleId>,
    pub level: Level,
    pub text: String,
    /// Conversa: quem falou ("Ana (EECOM)").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// Conversa: o loop ("geral" ou o código de uma posição).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
}

/// O que sai de um passo além do próprio veículo: eventos e pedaços largados.
#[derive(Default)]
pub struct StepOutput {
    pub events: Vec<(Level, String)>,
    pub jettisoned: Vec<Vehicle>,
    /// Pediu acoplamento a este alvo.
    pub docked_to: Option<VehicleId>,
}

/// Estado de outro veículo, para o guiamento de encontro.
#[derive(Clone, Copy, Debug)]
pub struct TargetInfo {
    pub id: VehicleId,
    pub central: Body,
    pub r: Vec3,
    pub v: Vec3,
    pub port_offset: f64,
}

/// Contexto de um passo.
pub struct Env<'a> {
    pub t: f64,
    pub wind_speed: f64,
    pub wind_jet: f64,
    pub wind_from: f64,
    pub targets: &'a [TargetInfo],
    /// Passo longo demais para a dinâmica de atitude (alta aceleração do tempo).
    pub coarse: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vehicle {
    pub id: VehicleId,
    pub name: String,
    pub kind: Kind,
    /// De qual definição do catálogo veio.
    pub def_id: String,
    pub stages: Vec<Stage>,
    /// Quantos estágios a pilha tinha no começo (para numerar os eventos).
    pub initial_stages: usize,
    pub items: Vec<Item>,
    pub shape: Shape,
    pub nose_radius: f64,
    pub heat_shield: Option<HeatShield>,
    pub chutes: Vec<Chute>,
    pub landing: LandingMode,
    #[serde(default)]
    pub gear_down: bool,
    pub les: Option<Les>,
    pub ports: Vec<Port>,
    pub systems: Option<Systems>,
    pub fts: Fts,
    pub central: Body,
    pub r: Vec3,
    pub v: Vec3,
    pub att: Quat,
    pub omega: Vec3,
    pub phase: Phase,
    pub site: Option<Site>,
    pub launch_time: Option<f64>,
    /// Liberado da plataforma (T-0).
    #[serde(default)]
    pub released: bool,
    pub guidance: Guidance,
    pub attached_to: Option<VehicleId>,
    #[serde(skip)]
    pub derived: Derived,
    #[serde(default)]
    pub stats: Stats,
    #[serde(default)]
    pub safe_mode: bool,
    /// Falhas injetadas ativas.
    #[serde(default)]
    pub faults: Vec<String>,
    /// Pai dos detritos (para o nome).
    #[serde(default)]
    pub parent: Option<VehicleId>,
    /// Alimentado de fora (umbilical ou anfitrião), definido pelo mundo.
    #[serde(default)]
    pub ground_power: bool,
}

// ───────────────────────── Massa, empuxo e capacidades ─────────────────────────

impl Vehicle {
    pub fn mass(&self) -> f64 {
        let stages: f64 = self.stages.iter().map(|s| s.mass()).sum();
        let items: f64 = self.items.iter().map(|i| i.mass).sum();
        let les = self.les.as_ref().map_or(0.0, |l| if l.available { l.tower_mass } else { 0.0 });
        let rcs = self.systems.as_ref().map_or(0.0, |s| s.adcs.rcs_prop);
        let shield = self.heat_shield.as_ref().map_or(0.0, |h| h.ablator_kg);
        stages + items + les + rcs + shield
    }

    pub fn core(&self) -> &Stage {
        self.stages.last().expect("pilha vazia")
    }

    pub fn crew(&self) -> u32 {
        self.systems.as_ref().and_then(|s| s.eclss.as_ref()).map_or(0, |e| e.crew)
    }

    /// Índices dos estágios que queimam agora: o de baixo e, se ele é de
    /// boosters, também o seguinte.
    pub fn active_stages(&self) -> Vec<usize> {
        // O de baixo; se ele não tem motores (tronco da cápsula), o primeiro que tem.
        let i = self.stages.iter().position(|s| !s.engines.is_empty()).unwrap_or(0);
        let i = if self.stages.first().is_some_and(|s| s.engines.is_empty() && !s.spacecraft) { 0 } else { i };
        match self.stages.get(i) {
            None => vec![],
            Some(s) if s.boosters && self.stages.len() > i + 1 => vec![i, i + 1],
            Some(_) => vec![i],
        }
    }

    /// Número do estágio de baixo na pilha original (1, 2, 3…).
    pub fn stage_number(&self) -> usize {
        self.initial_stages - self.stages.len() + 1
    }

    pub fn any_engine_lit(&self) -> bool {
        self.stages.iter().any(|s| s.any_lit())
    }

    /// Δv que resta na pilha (Tsiolkovsky, estágio a estágio, no vácuo).
    pub fn dv_remaining(&self) -> f64 {
        let mut dv = 0.0;
        let mut above: f64 = self.mass();
        let mut i = 0;
        while i < self.stages.len() {
            let s = &self.stages[i];
            let isp = s.isp_vac();
            if s.boosters && i + 1 < self.stages.len() {
                // Booster e núcleo juntos: aproximação pela isp média.
                let core = &self.stages[i + 1];
                let isp2 = core.isp_vac();
                let mix = if s.thrust_vac() + core.thrust_vac() > 0.0 {
                    (isp * s.thrust_vac() + isp2 * core.thrust_vac()) / (s.thrust_vac() + core.thrust_vac())
                } else {
                    0.0
                };
                let burn_core = s.propellant * core.engines.iter().map(|e| e.mdot_max()).sum::<f64>()
                    / s.engines.iter().map(|e| e.mdot_max()).sum::<f64>().max(1e-9);
                let used = s.propellant + burn_core.min(core.propellant);
                if mix > 0.0 && above - used > 0.0 {
                    dv += mix * G0 * (above / (above - used)).ln();
                }
                above -= used + s.dry_mass;
                // O núcleo segue com o que sobrou.
                let left = core.propellant - burn_core.min(core.propellant);
                if isp2 > 0.0 && left > 0.0 {
                    dv += isp2 * G0 * (above / (above - left)).ln();
                }
                above -= left + core.dry_mass;
                i += 2;
                continue;
            }
            if isp > 0.0 && s.propellant > 0.0 && s.engines.iter().any(|e| e.state != EngineState::Failed) {
                dv += isp * G0 * (above / (above - s.propellant)).ln();
            }
            above -= s.mass();
            i += 1;
        }
        dv
    }

    /// Área de referência (m²): a seção da pilha mais larga, mais os boosters.
    pub fn ref_area(&self) -> f64 {
        let d = self.stages.iter().map(|s| s.diameter).fold(0.0, f64::max).max(0.3);
        let mut a = std::f64::consts::PI * d * d / 4.0;
        if self.stages.first().is_some_and(|s| s.boosters) {
            a *= 1.6;
        }
        a
    }

    /// Corpo sem estabilização caindo de lado (detrito, carga de sondagem):
    /// arrasto pela área lateral.
    pub fn tumbling_cda(&self) -> Option<f64> {
        let tumbling = self.kind == Kind::Debris || (self.kind == Kind::SoundingRocket && self.stages.len() == 1);
        if !tumbling {
            return None;
        }
        let side: f64 = self.stages.iter().map(|s| s.length.max(1.0) * s.diameter).sum();
        Some(1.1 * side.max(self.ref_area()))
    }

    fn shape_now(&self) -> Shape {
        if self.kind == Kind::Debris {
            return Shape::Rocket;
        }
        // Enquanto há estágios do lançador embaixo, voa como foguete.
        if self.stages.iter().any(|s| !s.spacecraft) {
            Shape::Rocket
        } else {
            self.shape
        }
    }

    pub fn air(&self) -> Air {
        atmosphere::air(self.central, self.derived.alt)
    }
}

// ───────────────────────── Dinâmica ─────────────────────────

/// Aceleração da gravidade com J2.
pub fn gravity(body: Body, r: Vec3) -> Vec3 {
    let mu = body.mu();
    let rn = r.norm();
    let mut a = r * (-mu / (rn * rn * rn));
    let j2 = body.j2();
    if j2 > 0.0 {
        let re = body.radius();
        let z2 = r.z * r.z / (rn * rn);
        let k = -1.5 * j2 * mu * re * re / rn.powi(5);
        a += Vec3::new(r.x * (1.0 - 5.0 * z2), r.y * (1.0 - 5.0 * z2), r.z * (3.0 - 5.0 * z2)) * k;
    }
    a
}

/// Propaga só com a gravidade (com J2), em RK4 de passos de até `step` s.
pub fn propagate_gravity(body: Body, r0: Vec3, v0: Vec3, dt: f64, step: f64) -> (Vec3, Vec3) {
    let (mut r, mut v) = (r0, v0);
    let n = (dt.abs() / step).ceil().max(1.0) as usize;
    let h = dt / n as f64;
    for _ in 0..n {
        let k1v = gravity(body, r);
        let k1r = v;
        let k2v = gravity(body, r + k1r * (h / 2.0));
        let k2r = v + k1v * (h / 2.0);
        let k3v = gravity(body, r + k2r * (h / 2.0));
        let k3r = v + k2v * (h / 2.0);
        let k4v = gravity(body, r + k3r * h);
        let k4r = v + k3v * h;
        r += (k1r + k2r * 2.0 + k3r * 2.0 + k4r) * (h / 6.0);
        v += (k1v + k2v * 2.0 + k3v * 2.0 + k4v) * (h / 6.0);
    }
    (r, v)
}

/// Forças que não dependem do estado ao longo de um passo curto.
#[derive(Clone, Copy)]
struct Forces {
    thrust: Vec3,
    mass: f64,
    /// Coeficientes aerodinâmicos: Cd·A total e L/D com o rolamento.
    cda: f64,
    lift_over_drag: f64,
    bank: f64,
    wind_speed: f64,
    wind_jet: f64,
    wind_from: f64,
    extra: Vec3,
}

impl Vehicle {
    fn accel(&self, r: Vec3, v: Vec3, f: &Forces) -> Vec3 {
        let mut a = gravity(self.central, r) + f.thrust / f.mass + f.extra;
        if f.cda > 0.0 && self.central.has_atmosphere() {
            let alt = self.central.altitude(r);
            if alt < self.central.atmosphere_top() {
                let air = atmosphere::air(self.central, alt);
                if air.density > 0.0 {
                    let wind = atmosphere::wind_profile(f.wind_speed, f.wind_jet, alt);
                    let vrel = atmosphere::air_relative_velocity(self.central, r, v, wind, f.wind_from);
                    let vn = vrel.norm();
                    if vn > 1e-3 {
                        let q = 0.5 * air.density * vn * vn;
                        let drag = q * f.cda;
                        let vhat = vrel / vn;
                        a -= vhat * (drag / f.mass);
                        if f.lift_over_drag > 0.0 {
                            let up = r.unit().reject(vhat).unit();
                            let side = vhat.cross(up);
                            let dir = up * f.bank.cos() + side * f.bank.sin();
                            a += dir * (drag * f.lift_over_drag / f.mass);
                        }
                    }
                }
            }
        }
        a
    }

    /// Integra a translação por `h` com Runge-Kutta 4.
    fn integrate(&mut self, h: f64, f: &Forces) {
        let (r, v) = (self.r, self.v);
        let k1v = self.accel(r, v, f);
        let k1r = v;
        let k2v = self.accel(r + k1r * (h / 2.0), v + k1v * (h / 2.0), f);
        let k2r = v + k1v * (h / 2.0);
        let k3v = self.accel(r + k2r * (h / 2.0), v + k2v * (h / 2.0), f);
        let k3r = v + k2v * (h / 2.0);
        let k4v = self.accel(r + k3r * h, v + k3v * h, f);
        let k4r = v + k3v * h;
        self.r = r + (k1r + k2r * 2.0 + k3r * 2.0 + k4r) * (h / 6.0);
        self.v = v + (k1v + k2v * 2.0 + k3v * 2.0 + k4v) * (h / 6.0);
    }

    /// Passo máximo da integração no estado atual.
    pub fn max_step(&self, t: f64) -> f64 {
        match self.phase {
            Phase::Prelaunch => return 0.5,
            Phase::Landed | Phase::Splashdown | Phase::Destroyed => return 60.0,
            Phase::Docked => return 30.0,
            _ => {}
        }
        if self.attached_to.is_some() {
            return 30.0;
        }
        let alt = self.derived.alt;
        let in_air = self.central.has_atmosphere() && alt < self.central.atmosphere_top().min(160e3);
        if self.any_engine_lit() || self.les.as_ref().is_some_and(|l| l.firing > 0.0) {
            return if in_air { 0.05 } else { 0.2 };
        }
        if self.guidance.program == Program::Docking {
            return 0.5;
        }
        if in_air && self.derived.speed > 50.0 {
            return if alt < 100e3 { 0.1 } else { 0.5 };
        }
        if self.chutes.iter().any(|c| c.state == ChuteState::Deployed) || alt < 20e3 {
            return 0.2;
        }
        // Coasting: uma fração do período (ou do tempo até a borda da influência).
        let mu = self.central.mu();
        let rn = self.r.norm();
        let p = std::f64::consts::TAU * (rn.powi(3) / mu).sqrt();
        let mut h = (p / 720.0).clamp(1.0, 21_600.0);
        // Antes de uma queima programada, não pula por cima dela.
        h = h.min((crate::guidance::time_to_next_node(self, t) - 1.0).max(0.2));
        // Perto de um corpo com atmosfera, mais devagar.
        if self.central.has_atmosphere() && alt < 400e3 {
            h = h.min(10.0);
        }
        h
    }

    /// Ponto da superfície ao qual a pilha está presa na plataforma.
    pub fn pad_state(&self, t: f64) -> Option<(Vec3, Vec3)> {
        let s = self.site.as_ref()?;
        let r = self.central.from_geodetic(s.lat, s.lon, s.alt + 2.0, t);
        Some((r, self.central.surface_velocity(r)))
    }

    /// Atitude "em pé" na plataforma: X para cima.
    pub fn upright(&self) -> Quat {
        let up = self.r.unit();
        Quat::look(up, Vec3::Z.cross(up).unit())
    }

    /// Recalcula as grandezas derivadas.
    pub fn update_derived(&mut self, t: f64, env_wind: (f64, f64, f64)) {
        let (lat, lon, alt) = self.central.to_geodetic(self.r, t);
        let mass = self.mass();
        let dv_remaining = self.dv_remaining();
        let d = &mut self.derived;
        d.lat = lat;
        d.lon = lon;
        d.alt = alt;
        d.speed = self.v.norm();
        let up = self.r.unit();
        d.vspeed = self.v.dot(up);
        // Velocidade horizontal relativa à superfície (girando).
        let v_surf = self.v - self.central.surface_velocity(self.r);
        d.hspeed = v_surf.reject(up).norm();
        let air = atmosphere::air(self.central, alt);
        d.density = air.density;
        let wind = atmosphere::wind_profile(env_wind.0, env_wind.1, alt);
        let vrel = atmosphere::air_relative_velocity(self.central, self.r, self.v, wind, env_wind.2);
        d.air_speed = vrel.norm();
        d.mach = if air.sound > 0.0 { d.air_speed / air.sound } else { 0.0 };
        d.q = 0.5 * air.density * d.air_speed * d.air_speed;
        let fwd = self.att.forward();
        d.pitch_deg = fwd.dot(up).clamp(-1.0, 1.0).asin().to_degrees();
        let east = Vec3::Z.cross(up).unit();
        let north = up.cross(east);
        let hv = if d.hspeed > 1.0 { v_surf.reject(up) } else { fwd.reject(up) };
        d.heading_deg = hv.dot(east).atan2(hv.dot(north)).to_degrees().rem_euclid(360.0);
        d.aoa_deg = if d.air_speed > 5.0 { fwd.angle(vrel).to_degrees() } else { 0.0 };
        d.mass = mass;
        d.elements = elements_from_state(self.r, self.v, self.central.mu());
        let (sd, dist) = crate::bodies::sun_direction(self.central, self.r, t);
        d.sun_dir = sd;
        d.sun_dist = dist;
        d.sunlit = crate::bodies::sunlit(self.central, self.r, sd);
        d.dv_remaining = dv_remaining;
        let g_local = self.central.mu() / self.r.norm2();
        let thrust_now: f64 = self.stages.iter().flat_map(|s| &s.engines).map(|e| e.thrust).sum();
        d.thrust = thrust_now;
        d.twr = thrust_now / (d.mass * g_local);
        if let (Some(site), Some(_)) = (&self.site, self.launch_time) {
            d.downrange_km = great_circle_km(site.lat, site.lon, lat, lon);
        }
        let s = &mut self.stats;
        if d.alt > s.max_alt {
            s.max_alt = d.alt;
        }
        if d.speed > s.max_speed {
            s.max_speed = d.speed;
        }
    }
}

/// Distância de grande círculo na Terra (km).
pub fn great_circle_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let a = ((p2 - p1) / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (dl / 2.0).sin().powi(2);
    2.0 * 6_371.0 * a.sqrt().min(1.0).asin()
}

/// Rumo inicial de grande círculo (graus).
pub fn bearing_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    y.atan2(x).to_degrees().rem_euclid(360.0)
}

// ───────────────────────── Motores e estágios ─────────────────────────

impl Vehicle {
    /// Acende os motores dos estágios ativos.
    pub fn ignite(&mut self, out: &mut StepOutput) -> bool {
        let mut any = false;
        for i in self.active_stages() {
            let stage = &mut self.stages[i];
            if stage.propellant <= 0.0 {
                continue;
            }
            for e in &mut stage.engines {
                if e.state == EngineState::Off && e.ignitions > 0 {
                    e.ignitions -= 1;
                    e.throttle = 1.0;
                    e.state = EngineState::Starting;
                    e.spool = if e.solid { 1.0 } else { 0.0 };
                    if e.solid {
                        e.state = EngineState::Running;
                    }
                    any = true;
                } else if e.lit() {
                    any = true;
                }
            }
        }
        if !any {
            out.events.push((Level::Warn, "Ignição recusada: sem propelente ou sem ignições".into()));
        }
        any
    }

    /// Corta os motores líquidos acesos (os sólidos seguem até o fim).
    pub fn shutdown(&mut self) {
        for s in &mut self.stages {
            for e in &mut s.engines {
                if e.lit() && !e.solid {
                    e.state = EngineState::Stopping;
                }
            }
        }
    }

    /// Aplica a aceleração a todos os motores líquidos acesos.
    pub fn set_throttle(&mut self, k: f64) {
        for s in &mut self.stages {
            for e in &mut s.engines {
                if !e.solid {
                    e.throttle = k.clamp(e.min_throttle, 1.0);
                }
            }
        }
    }

    /// Larga o estágio de baixo (ou os boosters). Devolve o detrito.
    pub fn jettison_bottom(&mut self, t: f64) -> Option<Vehicle> {
        if self.stages.len() <= 1 {
            return None;
        }
        let stage = self.stages.remove(0);
        Some(self.make_debris(vec![stage], t, Vec3::ZERO))
    }

    /// Larga todos os estágios do lançador abaixo da nave (separação da carga).
    pub fn separate_payload(&mut self, t: f64) -> Option<Vehicle> {
        let n = self.stages.iter().take_while(|s| !s.spacecraft).count();
        if n == 0 || n >= self.stages.len() {
            return None;
        }
        let below: Vec<Stage> = self.stages.drain(..n).collect();
        // Mola de separação: 0,5 m/s para a nave, o resto fica para trás.
        let push = self.v.unit() * 0.5;
        self.v += push;
        let debris = self.make_debris(below, t, -push);
        Some(debris)
    }

    fn make_debris(&self, mut stages: Vec<Stage>, _t: f64, dv: Vec3) -> Vehicle {
        for s in &mut stages {
            for e in &mut s.engines {
                // Um sólido largado ainda queimando continua (boosters gastos não têm nada).
                if !e.solid {
                    e.state = EngineState::Off;
                }
                e.thrust = 0.0;
            }
        }
        let name = if stages.len() == 1 {
            format!("{} — {}", self.name, stages[0].name)
        } else {
            format!("{} — estágios do lançador", self.name)
        };
        Vehicle {
            id: 0,
            name,
            kind: Kind::Debris,
            def_id: String::new(),
            initial_stages: stages.len(),
            stages,
            items: vec![],
            shape: Shape::Rocket,
            nose_radius: 1.0,
            heat_shield: None,
            chutes: vec![],
            landing: LandingMode::None,
            gear_down: false,
            les: None,
            ports: vec![],
            systems: None,
            fts: Fts::default(),
            central: self.central,
            r: self.r,
            v: self.v + dv,
            att: self.att,
            omega: Vec3::new(0.02, 0.05, 0.01),
            phase: self.phase,
            site: self.site.clone(),
            launch_time: self.launch_time,
            released: true,
            guidance: Guidance { auto_stage: false, auto_chutes: false, ..Guidance::default() },
            attached_to: None,
            derived: self.derived.clone(),
            stats: Stats::default(),
            safe_mode: false,
            faults: vec![],
            parent: Some(self.id),
            ground_power: false,
        }
    }

    /// Larga uma peça (coifa, torre, ...) pelo id.
    pub fn drop_item(&mut self, id: &str) -> Option<Item> {
        let i = self.items.iter().position(|it| it.id == id)?;
        Some(self.items.remove(i))
    }

    /// Partida, rampa e queima dos motores; consome propelente.
    fn engines_step(&mut self, h: f64, p_amb: f64, out: &mut StepOutput) -> (f64, Vec3) {
        let mut thrust_total = 0.0;
        let mut mdot_total = 0.0;
        let dir = self.att.forward();
        for s in &mut self.stages {
            let mut stage_mdot = 0.0;
            for e in &mut s.engines {
                match e.state {
                    EngineState::Starting => {
                        e.spool = (e.spool + h / 2.0).min(1.0);
                        if e.spool >= 1.0 {
                            e.state = EngineState::Running;
                        }
                    }
                    EngineState::Stopping => {
                        e.spool = (e.spool - h / 1.0).max(0.0);
                        if e.spool <= 0.0 {
                            e.state = EngineState::Off;
                        }
                    }
                    _ => {}
                }
                let (f, m) = e.output(p_amb);
                e.thrust = f;
                if e.lit() {
                    e.burn_time += h;
                }
                thrust_total += f;
                stage_mdot += m;
            }
            let used = (stage_mdot * h + s.leak * h).min(s.propellant);
            s.propellant -= used;
            mdot_total += stage_mdot;
            if s.propellant <= 0.0 && s.any_lit() {
                for e in &mut s.engines {
                    if e.lit() {
                        e.state = EngineState::Off;
                        e.spool = 0.0;
                        e.thrust = 0.0;
                    }
                }
                out.events.push((Level::Info, format!("Fim da queima: {} sem propelente", s.name)));
            }
        }
        let _ = mdot_total;
        (thrust_total, dir * thrust_total)
    }
}

// ───────────────────────── Passo principal ─────────────────────────

impl Vehicle {
    /// Avança o veículo `h` segundos.
    pub fn step(&mut self, env: &Env, h: f64) -> StepOutput {
        let mut out = StepOutput::default();
        let t = env.t;
        let wind = (env.wind_speed, env.wind_jet, env.wind_from);
        if self.phase == Phase::Destroyed {
            return out;
        }
        if self.phase.is_final() {
            // Parado na superfície, girando com o corpo.
            let (lat, lon) = (self.derived.lat, self.derived.lon);
            self.r = self.central.from_geodetic(lat, lon, 0.0, t + h);
            self.v = self.central.surface_velocity(self.r);
            self.systems_step(h, env, false, 0.0);
            return out;
        }
        if self.attached_to.is_some() {
            // O mundo copia o estado do anfitrião; aqui só os subsistemas.
            self.systems_step(h, env, false, 0.0);
            return out;
        }
        if self.phase == Phase::Prelaunch {
            if let Some((r, v)) = self.pad_state(t + h) {
                self.r = r;
                self.v = v;
            }
            self.att = self.upright();
            self.omega = Vec3::ZERO;
            let air = atmosphere::air(self.central, self.derived.alt);
            let (thrust, _) = self.engines_step(h, air.pressure, &mut out);
            self.update_derived(t + h, wind);
            if self.released {
                let weight = self.mass() * G0;
                if thrust > weight * 1.02 {
                    self.phase = Phase::Ascent;
                    self.launch_time = Some(t + h);
                    out.events.push((Level::Good, format!("Decolagem! Empuxo/peso {:.2}", thrust / weight)));
                    if self.guidance.sounding.is_some() {
                        self.guidance.program = Program::Sounding;
                    } else if self.guidance.ascent.is_some() {
                        self.guidance.program = Program::Ascent;
                        self.setup_ascent(t + h);
                    }
                } else if thrust > 0.0 && self.derived.alt < 50.0 {
                    // Liberado sem empuxo suficiente: fica preso esperando.
                }
            }
            self.systems_step(h, env, false, 0.0);
            return out;
        }

        // Guiamento e sequenciador.
        crate::guidance::run(self, env, h, &mut out);

        // Atitude.
        self.attitude_step(env, h);

        // Motores.
        let air = atmosphere::air(self.central, self.derived.alt.max(0.0));
        let (thrust_mag, mut thrust) = self.engines_step(h, air.pressure, &mut out);
        // Sistema de escape disparando.
        if let Some(les) = &mut self.les {
            if les.firing > 0.0 {
                thrust += self.att.forward() * les.thrust;
                les.firing = (les.firing - h).max(0.0);
            }
        }
        let mass = self.mass();
        // Aerodinâmica do estado.
        let shape = self.shape_now();
        let mach = self.derived.mach;
        let mut cda = match self.tumbling_cda() {
            Some(c) => c,
            None => aero::drag_coefficient(shape, mach) * self.ref_area(),
        };
        for c in &mut self.chutes {
            if c.state == ChuteState::Deployed {
                c.inflation = (c.inflation + h / 3.0).min(1.0);
                cda += c.cda * c.inflation;
            }
        }
        let ld = if self.stages.len() == 1
            && matches!(shape, Shape::Capsule | Shape::Winged)
            && !self.chutes.iter().any(|c| c.state == ChuteState::Deployed)
            && !self.guidance.ballistic
        {
            self.guidance.ld_cmd.unwrap_or_else(|| aero::lift_to_drag(shape, mach))
        } else {
            0.0
        };
        let extra = self.guidance.rcs_accel;
        if extra.norm2() > 0.0 {
            if let Some(sys) = &mut self.systems {
                // RCS de translação: isp ~220 s.
                let used = mass * extra.norm() * h / (220.0 * G0);
                sys.adcs.rcs_prop = (sys.adcs.rcs_prop - used).max(0.0);
            }
        }
        let forces = Forces {
            thrust,
            mass,
            cda,
            lift_over_drag: ld,
            bank: self.guidance.bank_deg.to_radians(),
            wind_speed: wind.0,
            wind_jet: wind.1,
            wind_from: wind.2,
            extra,
        };
        let v_before = self.v;
        self.integrate(h, &forces);
        let dv = self.v - v_before - gravity(self.central, self.r) * h;
        self.derived.accel_g = dv.norm() / h / G0;
        if self.derived.accel_g > self.stats.max_g {
            self.stats.max_g = self.derived.accel_g;
        }
        let _ = thrust_mag;

        // Mudança de esfera de influência.
        self.check_soi(t + h, &mut out);
        self.update_derived(t + h, wind);
        self.heating_step(h, &mut out);
        self.flight_events(t + h, &mut out);
        self.ground_contact(t + h, &mut out);
        self.update_phase();
        let aero_w = if let Some(hs) = &self.heat_shield { hs.flux * hs.area * 0.02 } else { 0.0 };
        self.systems_step(h, env, true, aero_w);
        out
    }

    /// Troca de corpo central ao cruzar a esfera de influência.
    fn check_soi(&mut self, t: f64, out: &mut StepOutput) {
        let rn = self.r.norm();
        if let Some(parent) = self.central.parent() {
            if rn > self.central.soi() {
                let (rb, vb) = self.central.state(t);
                out.events.push((
                    Level::Info,
                    format!("Saiu da esfera de influência de {}: agora em torno de {}", self.central.name(), parent.name()),
                ));
                self.r += rb;
                self.v += vb;
                self.central = parent;
                return;
            }
        }
        for child in self.central.children() {
            let (rc, vc) = child.state(t);
            if (self.r - rc).norm() < child.soi() {
                out.events.push((Level::Good, format!("Entrou na esfera de influência de {}", child.name())));
                self.r -= rc;
                self.v -= vc;
                self.central = child;
                return;
            }
        }
    }

    /// Aquecimento aerodinâmico, escudo térmico e queima de quem não tem escudo.
    fn heating_step(&mut self, h: f64, out: &mut StepOutput) {
        let d = &self.derived;
        if d.density <= 0.0 || d.air_speed < 1000.0 {
            if let Some(hs) = &mut self.heat_shield {
                hs.flux = 0.0;
                hs.temp += (300.0 - hs.temp) * (h / 600.0).min(1.0);
            }
            return;
        }
        let nose = self.nose_radius;
        let q = aero::stagnation_heat_flux(d.density, d.air_speed, nose);
        match &mut self.heat_shield {
            Some(hs) => {
                hs.flux = q;
                hs.peak_flux = hs.peak_flux.max(q);
                hs.heat_load += q * h;
                let t_eq = aero::radiative_equilibrium(q, 0.85).min(3200.0);
                hs.temp += (t_eq - hs.temp) * (h / 8.0).min(1.0);
                // Ablação: o que não irradia é consumido pelo material.
                if hs.temp > 1500.0 {
                    let absorbed = (q - 0.85 * 5.670_374e-8 * hs.temp.powi(4)).max(0.05 * q);
                    hs.ablator_kg = (hs.ablator_kg - absorbed * hs.area * h / hs.ablation_heat).max(0.0);
                    if hs.ablator_kg <= 0.0 && q > 3e5 {
                        out.events.push((Level::Alarm, "Escudo térmico consumido: perda do veículo".into()));
                        self.phase = Phase::Destroyed;
                    }
                }
            }
            None => {
                // Sem escudo: carga térmica acumulada até desintegrar.
                self.stats.heat_load += q * h;
                if q > 4e5 && self.stats.heat_load > 2.5e8 && self.kind != Kind::SoundingRocket {
                    out.events.push((Level::Info, "Desintegrou-se na reentrada".into()));
                    self.phase = Phase::Destroyed;
                }
            }
        }
    }

    /// Marcos do voo: Max-Q, Mach 1, interface de reentrada, apogeu, microgravidade.
    fn flight_events(&mut self, t: f64, out: &mut StepOutput) {
        if self.kind == Kind::Debris {
            return;
        }
        let d = self.derived.clone();
        let met = self.launch_time.map_or(0.0, |l| t - l);
        if self.phase == Phase::Ascent || (self.phase == Phase::Coast && d.vspeed > 0.0) {
            if d.q > self.stats.max_q {
                self.stats.max_q = d.q;
                self.stats.max_q_met = met;
            } else if !self.stats.passed_max_q && self.stats.max_q > 5_000.0 && d.q < self.stats.max_q * 0.95 {
                self.stats.passed_max_q = true;
                out.events.push((Level::Info, format!("Max-Q: {:.1} kPa em T+{:.0} s", self.stats.max_q / 1e3, self.stats.max_q_met)));
            }
            if !self.stats.passed_mach1 && d.mach >= 1.0 {
                self.stats.passed_mach1 = true;
                out.events.push((Level::Info, "Supersônico (Mach 1)".into()));
            }
        }
        if self.kind == Kind::SoundingRocket || (self.phase == Phase::Coast && self.kind != Kind::Debris) {
            if !self.any_engine_lit() && d.vspeed < 0.0 && !self.stats.apogee_reported && self.launch_time.is_some() {
                self.stats.apogee_reported = true;
                out.events.push((Level::Info, format!("Apogeu: {:.1} km", self.stats.max_alt / 1e3)));
            }
            if self.kind == Kind::SoundingRocket && self.stats.microgravity_start.is_none()
                && !self.any_engine_lit() && d.q < 1.0 && d.alt > 100e3 && self.stages.len() == 1
            {
                self.stats.microgravity_start = Some(t);
                out.events.push((Level::Good, "Início da microgravidade".into()));
            }
        }
        let ei = if self.central == Body::Earth { 120e3 } else { self.central.atmosphere_top() * 0.8 };
        if self.central.has_atmosphere() && d.vspeed < 0.0 && d.alt < ei && !self.stats.entry_interface
            && d.speed > 2_000.0 && self.kind != Kind::Debris
        {
            self.stats.entry_interface = true;
            out.events.push((Level::Info, format!("Interface de reentrada ({} km), {:.0} m/s", ei / 1e3, d.air_speed)));
        }
        if let Some(sys) = &mut self.systems {
            let black = self.central == Body::Earth && d.alt > 40e3 && d.alt < 90e3 && d.air_speed > 5_000.0;
            if black != sys.comms.blackout {
                sys.comms.blackout = black;
                out.events.push((
                    if black { Level::Warn } else { Level::Good },
                    if black { "Início do apagão de comunicações (plasma)".into() } else { "Fim do apagão de comunicações".into() },
                ));
            }
        }
        if self.kind == Kind::SoundingRocket && self.stats.microgravity_start.is_some() && d.q > 50.0 && d.vspeed < 0.0 && !self.stats.blackout {
            self.stats.blackout = true;
            let dur = t - self.stats.microgravity_start.unwrap_or(t);
            out.events.push((Level::Info, format!("Fim da microgravidade: {:.0} s de experimento", dur)));
        }
    }

    /// Contato com a superfície: pouso, amerissagem ou impacto.
    fn ground_contact(&mut self, t: f64, out: &mut StepOutput) {
        if self.derived.alt > 0.0 || self.phase == Phase::Prelaunch {
            return;
        }
        let up = self.r.unit();
        let v_surf = self.v - self.central.surface_velocity(self.r);
        let sink = -v_surf.dot(up);
        let horiz = v_surf.reject(up).norm();
        let chutes = self.chutes.iter().any(|c| c.state == ChuteState::Deployed && !c.drogue);
        let landed = match self.landing {
            LandingMode::Splash => chutes && sink < 15.0,
            LandingMode::Runway => self.gear_down && sink < 6.0 && horiz < 160.0,
            LandingMode::None => false,
        };
        let (lat, lon) = (self.derived.lat, self.derived.lon);
        if landed {
            self.phase = if self.landing == LandingMode::Splash { Phase::Splashdown } else { Phase::Landed };
            out.events.push((
                Level::Good,
                format!(
                    "{} em {:.2}°, {:.2}° a {:.1} m/s",
                    if self.phase == Phase::Splashdown { "Amerissagem" } else { "Pouso" },
                    lat,
                    lon,
                    sink
                ),
            ));
            for e in self.stages.iter_mut().flat_map(|s| s.engines.iter_mut()) {
                e.state = EngineState::Off;
            }
        } else {
            self.phase = Phase::Destroyed;
            let level = if self.kind == Kind::Debris { Level::Info } else { Level::Alarm };
            out.events.push((level, format!("Impacto em {:.2}°, {:.2}° a {:.0} m/s", lat, lon, v_surf.norm())));
        }
        self.r = self.central.from_geodetic(lat, lon, 0.0, t);
        self.v = self.central.surface_velocity(self.r);
        self.derived.alt = 0.0;
    }

    /// Classifica a fase pelo estado.
    pub fn update_phase(&mut self) {
        if self.phase.is_final() || self.phase == Phase::Prelaunch {
            return;
        }
        if self.attached_to.is_some() {
            self.phase = Phase::Docked;
            return;
        }
        let d = &self.derived;
        let el = &d.elements;
        let atm_top = if self.central == Body::Earth { 100e3 } else { self.central.atmosphere_top() * 0.6 };
        let surface = self.central.radius();
        let chutes = self.chutes.iter().any(|c| c.state == ChuteState::Deployed);
        self.phase = if self.guidance.program == Program::Ascent && !self.guidance.inserted {
            Phase::Ascent
        } else if self.any_engine_lit() && self.launch_time.is_some() && self.phase == Phase::Ascent {
            Phase::Ascent
        } else if chutes || (self.landing == LandingMode::Runway && d.alt < 25e3 && d.mach < 1.0 && d.vspeed < 0.0) {
            Phase::Descent
        } else if self.central.has_atmosphere() && d.alt < atm_top + 20e3 && d.vspeed < 0.0 && el.periapsis() < surface + atm_top {
            Phase::Entry
        } else if el.e >= 1.0 {
            Phase::Escape
        } else if el.periapsis() > surface + atm_top.max(0.0) + if self.central.has_atmosphere() { 0.0 } else { 1_000.0 } {
            Phase::Orbit
        } else {
            Phase::Coast
        };
    }

    /// Atitude: cinemática para foguetes e queimas, dinâmica com o ADCS em órbita.
    fn attitude_step(&mut self, env: &Env, h: f64) {
        let target = self.attitude_target(env);
        // Com estágios do lançador presos, quem gira a pilha é o RCS do estágio.
        let kinematic = self.any_engine_lit()
            || self.stages.iter().any(|s| !s.spacecraft)
            || self.guidance.program != Program::None && self.guidance.program != Program::Docking
            || self.derived.q > 50.0
            || self.systems.is_none()
            || env.coarse
            || h > 1.0;
        if self.systems.is_none() {
            // Detrito: gira solto.
            self.att = self.att.integrate(self.omega, h);
            return;
        }
        if kinematic {
            let rate = (if self.any_engine_lit() { 4.0f64 } else { 3.0 }).to_radians();
            if let Some(q) = target {
                let mode = self.systems.as_ref().map(|s| s.adcs.mode);
                if env.coarse && mode != Some(AttMode::Free) {
                    self.att = q;
                } else {
                    self.att = self.att.slew_toward(q, rate * h);
                }
                self.omega = Vec3::ZERO;
            } else {
                self.att = self.att.integrate(self.omega, h);
            }
            if let Some(sys) = &mut self.systems {
                // Para foguetes, o que importa é para onde o eixo aponta (o rolamento é livre).
                sys.adcs.error_deg = target.map_or(f64::NAN, |q| self.att.forward().angle(q.forward()).to_degrees());
            }
        } else {
            let powered = self.systems.as_ref().is_some_and(|s| !s.power.brownout);
            let sys = self.systems.as_mut().unwrap();
            let (q, w) = sys.adcs.control(self.att, self.omega, target, h, powered);
            self.att = q;
            self.omega = w;
        }
    }

    /// Atitude alvo pelo modo do ADCS ou pelo guiamento.
    pub fn attitude_target(&self, env: &Env) -> Option<Quat> {
        let r = self.r;
        let v = self.v;
        let up = r.unit();
        let normal = r.cross(v).unit();
        let mode = match &self.systems {
            Some(s) => s.adcs.mode,
            None => return None,
        };
        if let Some(q) = self.guidance.att_cmd {
            if mode == AttMode::Guidance || self.guidance.program != Program::None || self.guidance.nodes.iter().any(|n| n.executing) {
                return Some(q);
            }
        }
        let look = |fwd: Vec3| Some(Quat::look(fwd, -up));
        match mode {
            AttMode::Free | AttMode::Detumble => None,
            AttMode::Hold => Some(self.att),
            AttMode::Prograde => look(v.unit()),
            AttMode::Retrograde => look(-v.unit()),
            AttMode::Normal => look(normal),
            AttMode::AntiNormal => look(-normal),
            AttMode::RadialOut => Some(Quat::look(up, v.unit())),
            AttMode::RadialIn => Some(Quat::look(-up, v.unit())),
            AttMode::SunPoint => {
                // Painéis (normal no corpo) voltados para o Sol.
                let n = self.systems.as_ref().and_then(|s| s.power.solar.as_ref()).map_or(Vec3::X, |sa| sa.normal);
                let sun = self.derived.sun_dir;
                Some(align_body_axis(n, sun, up))
            }
            AttMode::Nadir => Some(Quat::look(v.reject(up).unit(), -up)),
            AttMode::EarthPoint => {
                let (re, _) = Body::Earth.helio_state(env.t);
                let (rb, _) = self.central.helio_state(env.t);
                let to_earth = (re - (rb + r)).unit();
                Some(Quat::look(to_earth, normal))
            }
            AttMode::Target(id) => env
                .targets
                .iter()
                .find(|x| x.id == id && x.central == self.central)
                .map(|x| Quat::look((x.r - r).unit(), -up)),
            AttMode::Inertial(q) => Some(q),
            AttMode::Guidance => self.guidance.att_cmd,
        }
    }

    /// Energia, térmico, suporte de vida, armazenamento.
    pub fn systems_step(&mut self, h: f64, env: &Env, _flying: bool, aero_w: f64) {
        let d = self.derived.clone();
        let att = self.att;
        let g = d.accel_g.max(if self.phase.is_final() || self.phase == Phase::Prelaunch { 1.0 } else { 0.0 });
        let external = self.ground_power || self.attached_to.is_some();
        let Some(sys) = &mut self.systems else { return };
        sys.power.external = external;
        let sun_flux = crate::subsystems::SOLAR_CONSTANT * (crate::bodies::AU / d.sun_dist.max(1e9)).powi(2) * d.sunlit;
        let sun_body = att.inv_rotate(d.sun_dir);
        // Cargas que dependem de outros sistemas.
        let instr_w: f64 = sys.instruments.iter().filter(|i| i.on).map(|i| i.watts).sum();
        sys.comms.data_gen_mbps = sys.instruments.iter().filter(|i| i.on).map(|i| i.mbps).sum();
        let tx_w = if sys.comms.tx_on && !sys.comms.failed { sys.comms.tx_power_w * 3.0 } else { 0.0 };
        let wheels_w = sys.adcs.wheels.as_ref().map_or(0.0, |w| 5.0 + 20.0 * (w.h.norm() / w.h_max.max(1e-9)));
        sys.power.heater_w = sys.thermal.heater_power();
        sys.power.update(h, sun_flux, sun_body, instr_w + tx_w + wheels_w);
        let powered = !sys.power.brownout;
        let planet_ir = if self.central == Body::Earth && d.alt < 2_000e3 { 150.0 } else { 0.0 };
        let on_ground = self.phase == Phase::Prelaunch || self.phase.is_final();
        // No solo, o ar troca calor com a nave (aproximação: puxa para 20 °C).
        sys.thermal.update(h, if on_ground { 0.0 } else { sun_flux }, planet_ir, sys.power.load_w, aero_w, powered);
        if on_ground {
            for n in &mut sys.thermal.nodes {
                n.temp += (293.15 - n.temp) * (h / 3600.0).min(1.0);
            }
        }
        if let Some(e) = &mut sys.eclss {
            e.update(h, powered, g);
            // A cabine segue o nó térmico da cabine, se houver.
            if let Some(n) = sys.thermal.nodes.iter().find(|n| n.id == "cabin") {
                e.temp_c = n.celsius();
            }
        }
        sys.comms.update_storage(h);
        let _ = env;
    }
}

/// Atitude que leva o eixo `body_axis` do corpo à direção `dir`, com o resto
/// orientado por `secondary`.
pub fn align_body_axis(body_axis: Vec3, dir: Vec3, secondary: Vec3) -> Quat {
    // Atitude base com X em `dir`; depois corrige pelo eixo do corpo.
    let base = Quat::look(dir, secondary);
    // Rotação no corpo que leva body_axis para X.
    let a = body_axis.unit();
    let axis = a.cross(Vec3::X);
    let s = axis.norm();
    let c = a.dot(Vec3::X);
    if s < 1e-9 {
        if c > 0.0 {
            return base;
        }
        return base.mul(Quat::from_axis_angle(Vec3::Z, std::f64::consts::PI));
    }
    let rot = Quat::from_axis_angle(axis, s.atan2(c));
    base.mul(rot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eixo_alinhado() {
        let q = align_body_axis(Vec3::Z, Vec3::new(1.0, 1.0, 0.0).unit(), Vec3::Z);
        assert!((q.rotate(Vec3::Z) - Vec3::new(1.0, 1.0, 0.0).unit()).norm() < 1e-9);
    }

    #[test]
    fn gravidade_na_superficie() {
        let g = gravity(Body::Earth, Vec3::new(6_378_137.0, 0.0, 0.0)).norm();
        assert!((g - 9.81).abs() < 0.05, "{g}");
    }

    #[test]
    fn distancias() {
        // Alcântara a Natal: ~ 1 240 km? (CLA -2,32/-44,37; CLBI -5,92/-35,16)
        let d = great_circle_km(-2.32, -44.37, -5.92, -35.16);
        assert!((d - 1_097.0).abs() < 30.0, "{d}");
    }
}
