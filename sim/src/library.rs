//! Veículos, cenários e procedimentos em arquivos TOML, para criar missões sem
//! recompilar. Ficam em `missoes/veiculos`, `missoes/cenarios` e
//! `missoes/procedimentos` (veja os exemplos que vêm com o Jaci).
//!
//! Um veículo parte de um do catálogo (`base`) com ajustes, ou monta uma pilha
//! nova de estágios. Um cenário põe veículos na plataforma ou em órbita, com
//! hora, clima, satélites reais e falhas planejadas. Os procedimentos rodam na
//! interface; aqui só são lidos e conferidos.

use crate::aero::Shape;
use crate::bodies::Body;
use crate::catalog;
use crate::scenarios;
use crate::time::parse_utc;
use crate::training::FaultWhen;
use crate::vehicle::*;
use crate::world::World;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

// ── Veículos ──

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleFile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Veículo do catálogo (ou de outro arquivo) de onde partir.
    pub base: Option<String>,
    /// Tipo, quando não há base: sounding, satellite, capsule, probe, station.
    pub kind: Option<String>,
    /// Pilha nova de estágios, de baixo para cima (troca a da base).
    pub stages: Option<Vec<StageFile>>,
    /// Massa seca (kg) do último estágio, a carga útil.
    pub payload_kg: Option<f64>,
    /// Multiplica o propelente de todos os estágios.
    pub propellant_scale: Option<f64>,
    /// Coifa (kg), largada na subida; 0 tira.
    pub fairing_kg: Option<f64>,
    /// Ajustes no perfil de subida (órbita alvo, curva, limite de g…).
    pub ascent: Option<AscentPatch>,
    /// Perfil de foguete de sondagem (elevação, azimute, espera do 2º estágio).
    pub sounding: Option<SoundingProfile>,
    /// Copia os subsistemas (energia, térmico, atitude, rádio…) deste veículo do catálogo.
    pub systems_from: Option<String>,
    /// Tripulação (precisa de suporte de vida).
    pub crew: Option<u32>,
    /// Alvo de pouso [latitude, longitude] (graus).
    pub landing_target: Option<[f64; 2]>,
    /// Terminação de voo instalada.
    pub fts: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StageFile {
    pub name: String,
    /// Massa seca e de propelente (kg).
    pub dry_kg: f64,
    #[serde(default)]
    pub propellant_kg: f64,
    #[serde(default = "one")]
    pub diameter_m: f64,
    #[serde(default = "ten")]
    pub length_m: f64,
    #[serde(default)]
    pub cryogenic: bool,
    /// Boosters laterais: queimam junto com o estágio de cima.
    #[serde(default)]
    pub boosters: bool,
    /// A nave (fica depois da separação).
    #[serde(default)]
    pub spacecraft: bool,
    #[serde(default)]
    pub engines: Vec<EngineFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EngineFile {
    pub name: String,
    /// Empuxo no vácuo (kN) e impulsos específicos (s).
    pub thrust_kn: f64,
    pub isp_vac: f64,
    pub isp_sl: f64,
    #[serde(default)]
    pub solid: bool,
    #[serde(default = "one")]
    pub min_throttle: f64,
    #[serde(default = "one_u")]
    pub ignitions: u32,
    #[serde(default = "seventy")]
    pub pc_bar: f64,
    /// Quantos motores iguais (numerados).
    #[serde(default = "one_u")]
    pub count: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AscentPatch {
    pub target_pe_km: Option<f64>,
    pub target_ap_km: Option<f64>,
    pub inclination_deg: Option<f64>,
    pub north: Option<bool>,
    pub turn_start_speed: Option<f64>,
    pub turn_end_km: Option<f64>,
    pub end_pitch_deg: Option<f64>,
    pub turn_shape: Option<f64>,
    pub closed_loop_stage: Option<usize>,
    pub max_g: Option<f64>,
    pub payload_sep: Option<bool>,
    pub kick_stage: Option<usize>,
    /// Malha fechada pelo PEG (padrão) ou, com false, pela lei antiga.
    pub peg: Option<bool>,
}

fn one() -> f64 {
    1.0
}
fn ten() -> f64 {
    10.0
}
fn one_u() -> u32 {
    1
}
fn seventy() -> f64 {
    70.0
}

fn kind_from(id: &str) -> Result<Kind, String> {
    Ok(match id {
        "sounding" => Kind::SoundingRocket,
        "satellite" => Kind::Satellite,
        "capsule" => Kind::Capsule,
        "shuttle" => Kind::Shuttle,
        "station" => Kind::Station,
        "probe" => Kind::Probe,
        _ => return Err(format!("tipo desconhecido: {id} (sounding, satellite, capsule, probe, station)")),
    })
}

fn stage_of(s: &StageFile) -> Stage {
    let engines = s
        .engines
        .iter()
        .flat_map(|e| {
            (1..=e.count.max(1)).map(move |i| Engine {
                name: if e.count > 1 { format!("{} {i}", e.name) } else { e.name.clone() },
                thrust_vac: e.thrust_kn * 1e3,
                isp_vac: e.isp_vac,
                isp_sl: e.isp_sl,
                min_throttle: if e.solid { 1.0 } else { e.min_throttle },
                solid: e.solid,
                ignitions: if e.solid { 1 } else { e.ignitions },
                pc_nominal: e.pc_bar,
                state: EngineState::Off,
                spool: 0.0,
                throttle: 1.0,
                burn_time: 0.0,
                thrust: 0.0,
            })
        })
        .collect();
    Stage {
        name: s.name.clone(),
        dry_mass: s.dry_kg,
        propellant: s.propellant_kg,
        capacity: s.propellant_kg,
        engines,
        boosters: s.boosters,
        diameter: s.diameter_m,
        length: s.length_m,
        cryogenic: s.cryogenic,
        spacecraft: s.spacecraft,
        leak: 0.0,
        tank_bar: 3.5,
    }
}

impl Library {
    /// Monta um veículo: do catálogo ou de um arquivo (que pode partir de outro).
    pub fn vehicle(&self, id: &str) -> Result<Vehicle, String> {
        self.vehicle_depth(id, 0)
    }

    fn vehicle_depth(&self, id: &str, depth: u32) -> Result<Vehicle, String> {
        if depth > 8 {
            return Err(format!("{id}: bases em ciclo"));
        }
        let Some(f) = self.vehicles.get(id) else {
            return catalog::build(id).ok_or_else(|| format!("veículo desconhecido: {id}"));
        };
        let mut v = match &f.base {
            Some(b) => self.vehicle_depth(b, depth + 1)?,
            None => {
                let kind = kind_from(f.kind.as_deref().ok_or(format!("{id}: sem base, diga o tipo (kind)"))?)?;
                if f.stages.is_none() {
                    return Err(format!("{id}: sem base, defina os estágios ([[stages]])"));
                }
                let shape = match kind {
                    Kind::SoundingRocket => Shape::Rocket,
                    Kind::Capsule => Shape::Capsule,
                    _ => Shape::Box,
                };
                let mut v = catalog::build("vsb30").expect("vsb30 no catálogo");
                v.kind = kind;
                v.shape = shape;
                v.systems = None;
                v.chutes.clear();
                v.landing = LandingMode::None;
                v.guidance = Guidance::default();
                v.items.clear();
                v
            }
        };
        v.name = f.name.clone();
        v.def_id = id.to_string();
        if let Some(st) = &f.stages {
            if st.is_empty() {
                return Err(format!("{id}: pilha de estágios vazia"));
            }
            v.stages = st.iter().map(stage_of).collect();
            v.initial_stages = v.stages.len();
        }
        if let Some(k) = f.propellant_scale {
            for s in &mut v.stages {
                s.propellant *= k;
                s.capacity *= k;
            }
        }
        if let Some(kg) = f.payload_kg {
            v.stages.last_mut().ok_or(format!("{id}: sem estágios"))?.dry_mass = kg;
        }
        if let Some(kg) = f.fairing_kg {
            v.items.retain(|i| i.id != "fairing");
            if kg > 0.0 {
                v.items.push(Item { id: "fairing".into(), name: "Coifa".into(), mass: kg });
            }
        }
        if let Some(src) = &f.systems_from {
            v.systems = catalog::build(src).ok_or(format!("{id}: systems_from desconhecido: {src}"))?.systems;
        }
        if let Some(n) = f.crew {
            let e = v.systems.as_mut().and_then(|s| s.eclss.as_mut()).ok_or(format!("{id}: tripulação precisa de suporte de vida (systems_from de uma cápsula ou estação)"))?;
            e.crew = n;
        }
        if let Some(p) = &f.ascent {
            let a = v.guidance.ascent.get_or_insert(AscentProfile {
                target_pe_km: 200.0,
                target_ap_km: 200.0,
                inclination_deg: 0.0,
                north: true,
                raan_deg: None,
                turn_start_speed: 55.0,
                turn_end_km: 80.0,
                end_pitch_deg: 15.0,
                turn_shape: 0.5,
                closed_loop_stage: 1,
                max_g: 4.5,
                throttle_bucket: None,
                payload_sep: true,
                kick_stage: None,
            });
            macro_rules! set {
                ($($f:ident),*) => { $( if let Some(x) = p.$f { a.$f = x; } )* };
            }
            set!(target_pe_km, target_ap_km, inclination_deg, north, turn_start_speed, turn_end_km, end_pitch_deg, turn_shape, closed_loop_stage, max_g, payload_sep);
            if p.kick_stage.is_some() {
                a.kick_stage = p.kick_stage;
            }
            if let Some(peg) = p.peg {
                v.guidance.legacy_ascent = !peg;
            }
            if a.target_ap_km < a.target_pe_km {
                return Err(format!("{id}: apoastro alvo abaixo do periastro"));
            }
        }
        if let Some(s) = &f.sounding {
            v.guidance.sounding = Some(s.clone());
        }
        if let Some([lat, lon]) = f.landing_target {
            v.guidance.landing_target = Some((lat, lon));
        }
        if let Some(fts) = f.fts {
            v.fts.installed = fts;
        }
        if v.kind == Kind::SoundingRocket && v.guidance.sounding.is_none() {
            return Err(format!("{id}: foguete de sondagem sem [sounding]"));
        }
        Ok(v)
    }
}

// ── Cenários ──

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioFile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Hora de início (UTC, "AAAA-MM-DD HH:MM:SS"); sem ela, a pedida ou agora.
    pub start: Option<String>,
    /// Cenário pronto de onde partir (os veículos daqui são acrescentados).
    pub base: Option<String>,
    #[serde(default)]
    pub vehicles: Vec<ScenarioVehicle>,
    pub weather: Option<WeatherPatch>,
    /// Aceleração do tempo ao começar.
    pub warp: Option<f64>,
    /// Satélites reais (TLEs colados, duas ou três linhas cada).
    pub tle: Option<String>,
    /// Falhas planejadas para o treino.
    #[serde(default)]
    pub faults: Vec<FaultFile>,
    /// Scripts (missoes/scripts/<nome>.rhai) ligados ao carregar o cenário.
    #[serde(default)]
    pub scripts: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScenarioVehicle {
    /// Id do catálogo ou de um arquivo de veículo.
    pub vehicle: String,
    pub name: Option<String>,
    /// "pad" (na plataforma) ou "orbit".
    pub place: String,
    /// Plataforma: local de lançamento (começo do nome, ex. "Alcântara").
    pub site: Option<String>,
    /// Plataforma: começa a contagem em T- (s); padrão 600.
    pub t_minus: Option<f64>,
    /// Plataforma: janela de lançamento no plano deste veículo do cenário (nome).
    pub window_to: Option<String>,
    /// Órbita: corpo central (padrão: earth).
    pub body: Option<String>,
    pub pe_km: Option<f64>,
    pub ap_km: Option<f64>,
    pub inc_deg: Option<f64>,
    pub raan_deg: Option<f64>,
    pub argp_deg: Option<f64>,
    pub nu_deg: Option<f64>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherPatch {
    pub surface_wind: Option<f64>,
    pub wind_from: Option<f64>,
    pub jet_wind: Option<f64>,
    pub lightning_km: Option<f64>,
    pub ceiling_m: Option<f64>,
    pub rain: Option<bool>,
    pub evolve: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FaultFile {
    /// Nome do veículo no cenário.
    pub vehicle: String,
    /// Id da falha (veja o console do instrutor) ou "random".
    pub fault: String,
    pub arg: Option<String>,
    /// Daqui a N s do começo, em T+N s do lançamento ou numa hora UTC (um dos três).
    pub in_s: Option<f64>,
    pub met_s: Option<f64>,
    pub at: Option<String>,
    #[serde(default)]
    pub window_s: f64,
    #[serde(default = "one")]
    pub probability: f64,
    #[serde(default)]
    pub announce: bool,
}

impl Library {
    /// Monta um cenário de arquivo no instante `t` (se o arquivo não diz a hora).
    pub fn scenario(&self, id: &str, t: f64, seed: u64) -> Result<World, String> {
        let f = self.scenarios.get(id).ok_or_else(|| format!("cenário desconhecido: {id}"))?;
        let t = match &f.start {
            Some(s) => parse_utc(s).ok_or(format!("{id}: start inválido (AAAA-MM-DD HH:MM:SS)"))?,
            None => t,
        };
        let mut w = match &f.base {
            Some(b) => scenarios::build(b, t, seed).ok_or(format!("{id}: cenário base desconhecido: {b}"))?,
            None => World::new(t, seed),
        };
        w.scenario = id.to_string();
        w.mission = f.name.clone();
        let mut pad: Option<(VehicleId, f64, Option<String>)> = None;
        for (i, sv) in f.vehicles.iter().enumerate() {
            let mut v = self.vehicle(&sv.vehicle).map_err(|e| format!("{id}, veículo {}: {e}", i + 1))?;
            if let Some(n) = &sv.name {
                v.name = n.clone();
            }
            match sv.place.as_str() {
                "pad" => {
                    if pad.is_some() {
                        return Err(format!("{id}: só um veículo na plataforma"));
                    }
                    let site = catalog::sites().into_iter().find(|s| s.name.starts_with(sv.site.as_deref().unwrap_or("Alcântara"))).ok_or(format!("{id}: local desconhecido: {:?}", sv.site))?;
                    scenarios::on_pad(&mut v, site, w.t);
                    let vid = w.add_vehicle(v);
                    pad = Some((vid, sv.t_minus.unwrap_or(600.0), sv.window_to.clone()));
                }
                "orbit" => {
                    let body = match &sv.body {
                        Some(b) => Body::from_id(b).ok_or(format!("{id}: corpo desconhecido: {b}"))?,
                        None => Body::Earth,
                    };
                    let pe = sv.pe_km.ok_or(format!("{id}: órbita sem pe_km"))?;
                    let ap = sv.ap_km.unwrap_or(pe);
                    if ap < pe || pe < 0.0 {
                        return Err(format!("{id}: órbita inválida ({pe} × {ap} km)"));
                    }
                    scenarios::place_in_orbit(&mut v, body, pe, ap, sv.inc_deg.unwrap_or(0.0), sv.raan_deg.unwrap_or(0.0), sv.argp_deg.unwrap_or(0.0), sv.nu_deg.unwrap_or(0.0));
                    v.launch_time = Some(w.t - 86_400.0);
                    w.add_vehicle(v);
                }
                p => return Err(format!("{id}: place deve ser \"pad\" ou \"orbit\", não \"{p}\"")),
            }
        }
        if let Some((vid, t_minus, window)) = pad {
            let v = w.vehicle(vid).cloned().ok_or("veículo sumiu")?;
            w.launch.setup(&v, t_minus);
            if let Some(target) = window {
                let tid = w.vehicles.iter().find(|x| x.name == target).map(|x| x.id).ok_or(format!("{id}: window_to: não há veículo {target}"))?;
                let cmd = crate::commands::Command::new("window").with("target", tid);
                w.execute(vid, &cmd).map_err(|e| format!("{id}: janela: {e}"))?;
            }
        }
        if let Some(p) = &f.weather {
            let wx = &mut w.weather;
            macro_rules! set {
                ($($f:ident),*) => { $( if let Some(x) = p.$f { wx.$f = x; } )* };
            }
            set!(surface_wind, wind_from, jet_wind, lightning_km, ceiling_m, rain, evolve);
        }
        if let Some(k) = f.warp {
            w.warp = k.clamp(0.1, 1_000_000.0);
        }
        if let Some(text) = &f.tle {
            let found = crate::tracked::parse(text);
            if found.is_empty() {
                return Err(format!("{id}: tle sem nenhum TLE"));
            }
            for (name, l1, l2) in found {
                w.track(name.as_deref(), &l1, &l2).map_err(|e| format!("{id}: {e}"))?;
            }
        }
        for (i, ff) in f.faults.iter().enumerate() {
            let vid = w.vehicles.iter().find(|x| x.name == ff.vehicle).map(|x| x.id).ok_or(format!("{id}, falha {}: não há veículo {}", i + 1, ff.vehicle))?;
            if ff.fault != "random" && !crate::world::FAULTS.iter().any(|(k, _, _)| *k == ff.fault) {
                return Err(format!("{id}, falha {}: falha desconhecida: {}", i + 1, ff.fault));
            }
            let when = match (ff.in_s, ff.met_s, &ff.at) {
                (Some(s), None, None) => FaultWhen::At(w.t + s),
                (None, Some(s), None) => FaultWhen::Met(s),
                (None, None, Some(at)) => FaultWhen::At(parse_utc(at).ok_or(format!("{id}, falha {}: at inválido", i + 1))?),
                _ => return Err(format!("{id}, falha {}: diga um (e só um) de in_s, met_s ou at", i + 1)),
            };
            w.plan_fault(vid, &ff.fault, ff.arg.clone(), when, ff.window_s, ff.probability, ff.announce);
        }
        Ok(w)
    }
}

// ── Procedimentos ──

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProcedureFile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Tipos de veículo a que se aplica (vazio: todos).
    #[serde(default)]
    pub kinds: Vec<String>,
    pub steps: Vec<StepFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StepFile {
    /// note, check, cmd ou wait.
    pub kind: String,
    pub text: String,
    pub cmd: Option<String>,
    pub args: Option<toml::Table>,
    #[serde(default)]
    pub hazardous: bool,
    /// Espera: condição sobre a telemetria, ex. "eps.soc > 80 and phase == 'orbit'".
    pub until: Option<String>,
}

impl ProcedureFile {
    fn check(&self, id: &str) -> Result<(), String> {
        if self.steps.is_empty() {
            return Err(format!("{id}: procedimento sem passos"));
        }
        for (i, s) in self.steps.iter().enumerate() {
            let n = i + 1;
            match s.kind.as_str() {
                "note" | "check" => {}
                "cmd" => {
                    let c = s.cmd.as_deref().ok_or(format!("{id}, passo {n}: cmd sem o comando"))?;
                    if !crate::commands::dictionary().iter().any(|d| d.id == c) && !c.starts_with("ksp_") {
                        return Err(format!("{id}, passo {n}: comando desconhecido: {c}"));
                    }
                }
                "wait" => {
                    if s.until.as_deref().is_none_or(|u| u.trim().is_empty()) {
                        return Err(format!("{id}, passo {n}: wait sem until"));
                    }
                }
                k => return Err(format!("{id}, passo {n}: kind deve ser note, check, cmd ou wait, não {k}")),
            }
        }
        Ok(())
    }
}

// ── Escola de controle ──

/// Lição da escola de controle: um cenário e objetivos em ordem, cada um com
/// a condição que a interface confere a cada instantâneo.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LessonFile {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Ordem na lista (menor primeiro).
    #[serde(default)]
    pub order: i32,
    /// Cenário que a lição carrega (pronto ou de arquivo); sem ele, segue no atual.
    pub scenario: Option<String>,
    /// Texto de abertura.
    #[serde(default)]
    pub intro: String,
    /// Texto do fim.
    #[serde(default)]
    pub done: String,
    /// Veículo das condições (nome), se não for o da plataforma ou o selecionado.
    pub vehicle: Option<String>,
    /// Tempo de referência (s de relógio) para as três estrelas.
    pub par_s: Option<f64>,
    pub objectives: Vec<ObjectiveFile>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectiveFile {
    pub text: String,
    #[serde(default)]
    pub hint: String,
    /// Condição de cumprido (a mesma linguagem do `until` dos procedimentos).
    pub until: String,
    /// Console que o "me leve lá" abre.
    pub console: Option<String>,
    /// Veículo desta condição (nome), se for outro.
    pub vehicle: Option<String>,
}

impl LessonFile {
    fn check(&self, id: &str) -> Result<(), String> {
        if self.objectives.is_empty() {
            return Err(format!("{id}: lição sem objetivos"));
        }
        for (i, o) in self.objectives.iter().enumerate() {
            if o.until.trim().is_empty() {
                return Err(format!("{id}, objetivo {}: sem until", i + 1));
            }
        }
        Ok(())
    }
}

// ── A biblioteca ──

/// Os arquivos lidos da pasta de missões.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub vehicles: BTreeMap<String, VehicleFile>,
    pub scenarios: BTreeMap<String, ScenarioFile>,
    pub procedures: BTreeMap<String, ProcedureFile>,
    pub lessons: BTreeMap<String, LessonFile>,
    /// Arquivos com problema: (pasta/arquivo, erro).
    pub errors: Vec<(String, String)>,
}

pub const KINDS: [(&str, &str); 4] = [("veiculos", "vehicle"), ("cenarios", "scenario"), ("procedimentos", "procedure"), ("escola", "lesson")];

/// Nome de arquivo seguro (sem extensão): letras, números, - e _.
pub fn clean_id(s: &str) -> String {
    s.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect::<String>().trim_matches('-').to_string()
}

impl Library {
    /// Lê `<dir>/veiculos`, `<dir>/cenarios` e `<dir>/procedimentos`, e confere tudo.
    pub fn load(dir: &Path) -> Library {
        let mut lib = Library::default();
        for (sub, _) in KINDS {
            let Ok(rd) = std::fs::read_dir(dir.join(sub)) else { continue };
            let mut files: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "toml")).collect();
            files.sort();
            for path in files {
                let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
                let label = format!("{sub}/{id}.toml");
                let text = match std::fs::read_to_string(&path) {
                    Ok(t) => t,
                    Err(e) => {
                        lib.errors.push((label, e.to_string()));
                        continue;
                    }
                };
                if let Err(e) = lib.insert(sub, &id, &text) {
                    lib.errors.push((label, e));
                }
            }
        }
        // Veículos e cenários só valem se montam.
        let bad: Vec<(String, String)> = lib.vehicles.keys().filter_map(|id| lib.vehicle(id).err().map(|e| (id.clone(), e))).collect();
        for (id, e) in bad {
            lib.vehicles.remove(&id);
            lib.errors.push((format!("veiculos/{id}.toml"), e));
        }
        let bad: Vec<(String, String)> = lib.scenarios.keys().filter_map(|id| lib.scenario(id, 0.0, 1).err().map(|e| (id.clone(), e))).collect();
        for (id, e) in bad {
            lib.scenarios.remove(&id);
            lib.errors.push((format!("cenarios/{id}.toml"), e));
        }
        // Lições só valem com um cenário que existe.
        let bad: Vec<(String, String)> = lib.lessons.iter().filter_map(|(id, l)| l.scenario.as_ref().filter(|sc| !lib.has_scenario(sc)).map(|sc| (id.clone(), format!("{id}: cenário desconhecido: {sc}")))).collect();
        for (id, e) in bad {
            lib.lessons.remove(&id);
            lib.errors.push((format!("escola/{id}.toml"), e));
        }
        lib
    }

    /// Lê um arquivo e põe na biblioteca (sem montar).
    pub fn insert(&mut self, sub: &str, id: &str, text: &str) -> Result<(), String> {
        let err = |e: toml::de::Error| e.to_string();
        match sub {
            "veiculos" => {
                if catalog::ENTRIES.iter().any(|e| e.id == id) {
                    return Err(format!("{id}: já é um veículo do catálogo; use outro nome e base = \"{id}\""));
                }
                self.vehicles.insert(id.into(), toml::from_str(text).map_err(err)?);
            }
            "cenarios" => {
                if scenarios::SCENARIOS.iter().any(|s| s.id == id) {
                    return Err(format!("{id}: já é um cenário pronto; use outro nome e base = \"{id}\""));
                }
                self.scenarios.insert(id.into(), toml::from_str(text).map_err(err)?);
            }
            "procedimentos" => {
                let p: ProcedureFile = toml::from_str(text).map_err(err)?;
                p.check(id)?;
                self.procedures.insert(id.into(), p);
            }
            "escola" => {
                let l: LessonFile = toml::from_str(text).map_err(err)?;
                l.check(id)?;
                self.lessons.insert(id.into(), l);
            }
            _ => return Err(format!("pasta desconhecida: {sub}")),
        }
        Ok(())
    }

    /// Cenário pronto ou de arquivo com este id.
    pub fn has_scenario(&self, id: &str) -> bool {
        scenarios::SCENARIOS.iter().any(|s| s.id == id) || self.scenarios.contains_key(id)
    }

    /// Confere um texto sem gravar: lê e, para veículos e cenários, monta.
    pub fn validate(&self, sub: &str, id: &str, text: &str) -> Result<(), String> {
        let mut lib = self.clone();
        lib.insert(sub, id, text)?;
        match sub {
            "veiculos" => lib.vehicle(id).map(|_| ()),
            "cenarios" => lib.scenario(id, 0.0, 1).map(|_| ()),
            "escola" => match lib.lessons[id].scenario.as_deref() {
                Some(sc) if !lib.has_scenario(sc) => Err(format!("{id}: cenário desconhecido: {sc}")),
                _ => Ok(()),
            },
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::from_calendar;

    #[test]
    fn veiculo_com_base_e_ajustes() {
        let mut lib = Library::default();
        lib.insert(
            "veiculos",
            "vsb30_pesado",
            r#"
                name = "VSB-30 pesado"
                base = "vsb30"
                payload_kg = 600
                landing_target = [-1.5, -40.0]
            "#,
        )
        .unwrap();
        let v = lib.vehicle("vsb30_pesado").unwrap();
        assert_eq!(v.name, "VSB-30 pesado");
        assert_eq!(v.stages.last().unwrap().dry_mass, 600.0);
        assert_eq!(v.guidance.landing_target, Some((-1.5, -40.0)));
        assert!(v.guidance.sounding.is_some());
    }

    #[test]
    fn pilha_nova_e_erros_claros() {
        let mut lib = Library::default();
        let text = r#"
            name = "Lançador leve"
            kind = "satellite"
            systems_from = "amazonia"
            fairing_kg = 150
            [ascent]
            target_pe_km = 500
            target_ap_km = 500
            inclination_deg = 97.5
            north = false
            [[stages]]
            name = "1º estágio"
            dry_kg = 2500
            propellant_kg = 26000
            [[stages.engines]]
            name = "Motor"
            thrust_kn = 120
            isp_vac = 311
            isp_sl = 285
            count = 4
            min_throttle = 0.6
            [[stages]]
            name = "2º estágio"
            dry_kg = 600
            propellant_kg = 4500
            [[stages.engines]]
            name = "Motor de vácuo"
            thrust_kn = 30
            isp_vac = 340
            isp_sl = 200
            ignitions = 3
            [[stages]]
            name = "Satélite"
            dry_kg = 300
            spacecraft = true
        "#;
        lib.insert("veiculos", "leve", text).unwrap();
        let v = lib.vehicle("leve").unwrap();
        assert_eq!(v.stages.len(), 3);
        assert_eq!(v.stages[0].engines.len(), 4);
        assert_eq!(v.stages[0].engines[3].name, "Motor 4");
        assert!(v.systems.is_some());
        assert_eq!(v.guidance.ascent.as_ref().unwrap().inclination_deg, 97.5);
        // Campo errado: o erro diz qual.
        let e = lib.validate("veiculos", "x", "name = \"X\"\nbase = \"vsb30\"\npayload = 3\n").unwrap_err();
        assert!(e.contains("payload"), "{e}");
        assert!(lib.validate("veiculos", "vsb30", "name = \"X\"\nbase = \"vsb30\"").is_err(), "sobrescreveu o catálogo");
        assert!(lib.validate("veiculos", "y", "name = \"Y\"\nbase = \"y\"").unwrap_err().contains("ciclo"));
    }

    #[test]
    fn cenario_com_orbita_plataforma_e_falha() {
        let mut lib = Library::default();
        lib.insert("veiculos", "leve", "name = \"Leve\"\nbase = \"medio_sso\"\n").unwrap();
        lib.insert(
            "cenarios",
            "treino",
            r#"
                name = "Treino com falha de motor"
                start = "2026-11-05 12:00:00"
                warp = 1
                [[vehicles]]
                vehicle = "amazonia"
                name = "Alvo"
                place = "orbit"
                pe_km = 700
                inc_deg = 98
                [[vehicles]]
                vehicle = "leve"
                place = "pad"
                site = "Alcântara"
                t_minus = 300
                [weather]
                surface_wind = 9
                [[faults]]
                vehicle = "Leve"
                fault = "engine"
                arg = "3"
                met_s = 40
            "#,
        )
        .unwrap();
        let w = lib.scenario("treino", 0.0, 1).unwrap();
        assert_eq!(w.mission, "Treino com falha de motor");
        assert_eq!(w.t, from_calendar(2026, 11, 5, 12, 0, 0.0));
        assert_eq!(w.vehicles.len(), 2);
        assert_eq!(w.launch.t_minus, 300.0);
        assert_eq!(w.weather.surface_wind, 9.0);
        assert_eq!(w.training.plan.len(), 1);
        let alvo = w.vehicles.iter().find(|v| v.name == "Alvo").unwrap();
        assert!((alvo.derived.alt / 1e3 - 700.0).abs() < 30.0 || alvo.r.norm() > 7.0e6);
        let e = lib.validate("cenarios", "ruim", "name = \"R\"\n[[vehicles]]\nvehicle = \"nada\"\nplace = \"pad\"\n").unwrap_err();
        assert!(e.contains("nada"), "{e}");
    }

    #[test]
    fn procedimento_conferido() {
        let mut lib = Library::default();
        let ok = r#"
            name = "Bateria baixa"
            kinds = ["satellite"]
            [[steps]]
            kind = "cmd"
            text = "Modo de segurança"
            cmd = "safe_mode"
            args = { on = "on" }
            [[steps]]
            kind = "wait"
            text = "Bateria acima de 60 %"
            until = "eps.soc > 60"
        "#;
        lib.insert("procedimentos", "bateria", ok).unwrap();
        assert!(lib.insert("procedimentos", "x", "name = \"X\"\n[[steps]]\nkind = \"cmd\"\ntext = \"t\"\ncmd = \"nao_existe\"\n").unwrap_err().contains("nao_existe"));
        assert!(lib.insert("procedimentos", "x", "name = \"X\"\n[[steps]]\nkind = \"wait\"\ntext = \"t\"\n").is_err());
    }

    #[test]
    fn os_exemplos_do_repositorio_carregam() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../missoes");
        let lib = Library::load(&dir);
        assert!(lib.errors.is_empty(), "{:?}", lib.errors);
        assert!(!lib.scenarios.is_empty() && !lib.vehicles.is_empty() && !lib.procedures.is_empty() && lib.lessons.len() >= 5);
    }
}
