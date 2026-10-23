//! Automação por script (Rhai): sequências automáticas e regras de bordo
//! escritas pelo usuário, em `missoes/scripts/*.rhai`.
//!
//! Cada script ligado roda inteiro a cada segundo de simulação (no máximo uma
//! vez por volta do laço do servidor). Ele lê o estado (telemetria, fases,
//! alarmes, contagem) e pede ações: `cmd` é automação de solo e passa pelo
//! enlace, com o atraso da luz; `onboard` é regra de bordo e executa direto no
//! veículo. `mem` guarda o que o script quiser de uma vez para a outra;
//! `once` e `every` ajudam a montar sequências. O Rhai não tem acesso a
//! arquivos nem à rede, e cada execução tem limite de operações: um script
//! que erra ou passa do limite é desligado, com o motivo no registro.

use jaci_sim::{
    commands::{self, Command},
    vehicle::{Level, Phase, VehicleId},
    world::World,
};
use rhai::{AST, Dynamic, Engine, Map, Scope};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

pub const EXT: &str = "rhai";

/// O que o script vê e o que ele pede, durante uma execução.
#[derive(Default)]
struct RunCtx {
    t: f64,
    names: Vec<(String, VehicleId)>,
    tm: HashMap<VehicleId, HashMap<String, f64>>,
    phase: HashMap<VehicleId, String>,
    met: HashMap<VehicleId, f64>,
    link: HashMap<VehicleId, bool>,
    alarms: Vec<(VehicleId, String, String)>,
    count: (String, f64),
    /// Marcas de once/every do script que roda.
    flags: HashMap<String, f64>,
    out: Vec<Action>,
}

enum Action {
    Cmd { vehicle: VehicleId, cmd: Command, onboard: bool, armed: bool },
    Log(Level, String),
}

pub struct Script {
    pub name: String,
    pub source: String,
    ast: Option<AST>,
    pub enabled: bool,
    mem: Map,
    flags: HashMap<String, f64>,
    pub error: Option<String>,
    pub runs: u64,
}

pub struct Scripts {
    engine: Engine,
    ctx: Arc<Mutex<RunCtx>>,
    pub list: Vec<Script>,
    last_t: f64,
}

fn to_json(d: &Dynamic) -> Value {
    if let Some(i) = d.clone().try_cast::<rhai::INT>() {
        json!(i)
    } else if let Some(f) = d.clone().try_cast::<rhai::FLOAT>() {
        json!(f)
    } else if let Some(b) = d.clone().try_cast::<bool>() {
        json!(b)
    } else if let Some(m) = d.clone().try_cast::<Map>() {
        Value::Object(m.iter().map(|(k, v)| (k.to_string(), to_json(v))).collect())
    } else if let Some(a) = d.clone().try_cast::<rhai::Array>() {
        Value::Array(a.iter().map(to_json).collect())
    } else {
        json!(d.to_string())
    }
}

fn command(name: &str, args: &Map) -> Command {
    let mut c = Command::new(name);
    for (k, v) in args {
        c.args.insert(k.to_string(), to_json(v));
    }
    c
}

impl Scripts {
    pub fn new() -> Scripts {
        let ctx = Arc::new(Mutex::new(RunCtx::default()));
        let mut e = Engine::new();
        e.set_max_operations(200_000);
        e.set_max_call_levels(32);
        e.set_max_expr_depths(64, 32);
        e.set_max_string_size(10_000);
        e.set_max_array_size(10_000);
        e.set_max_map_size(10_000);
        e.disable_symbol("eval");
        {
            let c = ctx.clone();
            e.on_print(move |s| c.lock().unwrap().out.push(Action::Log(Level::Info, s.to_string())));
        }
        macro_rules! reg {
            ($name:expr, $f:expr) => {{
                let c = ctx.clone();
                #[allow(clippy::redundant_closure_call)]
                e.register_fn($name, $f(c));
            }};
        }
        reg!("t", |c: Arc<Mutex<RunCtx>>| move || c.lock().unwrap().t);
        reg!("vehicle", |c: Arc<Mutex<RunCtx>>| move |name: &str| c.lock().unwrap().names.iter().find(|(n, _)| n == name).map_or(-1, |(_, id)| *id as rhai::INT));
        reg!("vehicles", |c: Arc<Mutex<RunCtx>>| move || c.lock().unwrap().names.iter().map(|(n, _)| Dynamic::from(n.clone())).collect::<rhai::Array>());
        reg!("tm", |c: Arc<Mutex<RunCtx>>| move |id: rhai::INT, p: &str| c.lock().unwrap().tm.get(&(id as VehicleId)).and_then(|m| m.get(p)).copied().unwrap_or(f64::NAN));
        reg!("phase", |c: Arc<Mutex<RunCtx>>| move |id: rhai::INT| c.lock().unwrap().phase.get(&(id as VehicleId)).cloned().unwrap_or_default());
        reg!("met", |c: Arc<Mutex<RunCtx>>| move |id: rhai::INT| c.lock().unwrap().met.get(&(id as VehicleId)).copied().unwrap_or(f64::NAN));
        reg!("link", |c: Arc<Mutex<RunCtx>>| move |id: rhai::INT| c.lock().unwrap().link.get(&(id as VehicleId)).copied().unwrap_or(false));
        reg!("alarm", |c: Arc<Mutex<RunCtx>>| move |id: rhai::INT, p: &str| {
            c.lock().unwrap().alarms.iter().find(|(v, pt, _)| *v == id as VehicleId && pt == p).map(|(_, _, l)| l.clone()).unwrap_or_default()
        });
        reg!("alarms", |c: Arc<Mutex<RunCtx>>| move |id: rhai::INT| c.lock().unwrap().alarms.iter().filter(|(v, _, _)| *v == id as VehicleId).count() as rhai::INT);
        reg!("count_state", |c: Arc<Mutex<RunCtx>>| move || c.lock().unwrap().count.0.clone());
        reg!("t_minus", |c: Arc<Mutex<RunCtx>>| move || c.lock().unwrap().count.1);
        reg!("log", |c: Arc<Mutex<RunCtx>>| move |s: &str| c.lock().unwrap().out.push(Action::Log(Level::Info, s.to_string())));
        reg!("warn", |c: Arc<Mutex<RunCtx>>| move |s: &str| c.lock().unwrap().out.push(Action::Log(Level::Warn, s.to_string())));
        reg!("once", |c: Arc<Mutex<RunCtx>>| move |k: &str| {
            let mut c = c.lock().unwrap();
            let t = c.t;
            c.flags.insert(k.to_string(), t).is_none()
        });
        reg!("every", |c: Arc<Mutex<RunCtx>>| move |secs: rhai::FLOAT, k: &str| {
            let mut c = c.lock().unwrap();
            let t = c.t;
            match c.flags.get(k) {
                Some(last) if t - last < secs => false,
                _ => {
                    c.flags.insert(k.to_string(), t);
                    true
                }
            }
        });
        // Comandos: de solo (enlace e atraso da luz) ou de bordo (na hora).
        for (name, onboard, armed) in [("cmd", false, false), ("cmd_armed", false, true), ("onboard", true, false)] {
            let c1 = ctx.clone();
            e.register_fn(name, move |id: rhai::INT, cmd: &str| c1.lock().unwrap().out.push(Action::Cmd { vehicle: id as VehicleId, cmd: Command::new(cmd), onboard, armed }));
            let c2 = ctx.clone();
            e.register_fn(name, move |id: rhai::INT, cmd: &str, args: Map| c2.lock().unwrap().out.push(Action::Cmd { vehicle: id as VehicleId, cmd: command(cmd, &args), onboard, armed }));
        }
        Scripts { engine: e, ctx, list: Vec::new(), last_t: f64::NEG_INFINITY }
    }

    /// Confere a sintaxe.
    pub fn compile(&self, source: &str) -> Result<AST, String> {
        self.engine.compile(source).map_err(|e| e.to_string())
    }

    /// Lê `<dir>/*.rhai`, mantendo ligados os que já estavam.
    pub fn load(&mut self, dir: &Path) {
        let on: Vec<String> = self.list.iter().filter(|s| s.enabled).map(|s| s.name.clone()).collect();
        self.list.clear();
        let mut files: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == EXT)).collect();
        files.sort();
        for p in files {
            let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let source = std::fs::read_to_string(&p).unwrap_or_default();
            let (ast, error) = match self.compile(&source) {
                Ok(a) => (Some(a), None),
                Err(e) => (None, Some(e)),
            };
            let enabled = ast.is_some() && on.contains(&name);
            self.list.push(Script { name, source, ast, enabled, mem: Map::new(), flags: HashMap::new(), error, runs: 0 });
        }
    }

    pub fn set_enabled(&mut self, name: &str, on: bool) -> Result<(), String> {
        let s = self.list.iter_mut().find(|s| s.name == name).ok_or(format!("script desconhecido: {name}"))?;
        if on && s.ast.is_none() {
            return Err(format!("{name}: {}", s.error.clone().unwrap_or_else(|| "não compila".into())));
        }
        s.enabled = on;
        if on {
            // Ligar de novo começa do zero.
            s.mem = Map::new();
            s.flags.clear();
            s.error = None;
        }
        Ok(())
    }

    /// Roda os scripts ligados se passou um segundo de simulação.
    pub fn tick(&mut self, w: &mut World) {
        // Voltou no tempo (estado carregado): recomeça a contagem.
        if w.t < self.last_t {
            self.last_t = f64::NEG_INFINITY;
        }
        if !self.list.iter().any(|s| s.enabled) || w.t - self.last_t < 1.0 {
            return;
        }
        self.last_t = w.t;
        self.prepare(w);
        for i in 0..self.list.len() {
            if !self.list[i].enabled {
                continue;
            }
            let s = &mut self.list[i];
            {
                let mut c = self.ctx.lock().unwrap();
                c.flags = std::mem::take(&mut s.flags);
                c.out.clear();
            }
            let mut scope = Scope::new();
            scope.push("mem", std::mem::take(&mut s.mem));
            let result = self.engine.run_ast_with_scope(&mut scope, s.ast.as_ref().unwrap());
            s.mem = scope.get_value::<Map>("mem").unwrap_or_default();
            s.runs += 1;
            let (flags, out) = {
                let mut c = self.ctx.lock().unwrap();
                (std::mem::take(&mut c.flags), std::mem::take(&mut c.out))
            };
            s.flags = flags;
            let name = s.name.clone();
            if let Err(e) = result {
                s.enabled = false;
                s.error = Some(e.to_string());
                w.event(None, Level::Warn, format!("[script {name}] parou: {e}"));
                continue;
            }
            apply(w, &name, out);
        }
    }

    /// Copia do mundo o que os scripts podem ler.
    fn prepare(&self, w: &World) {
        let mut c = self.ctx.lock().unwrap();
        c.t = w.t;
        c.names = w.vehicles.iter().filter(|v| v.kind != jaci_sim::vehicle::Kind::Debris).map(|v| (v.name.clone(), v.id)).collect();
        c.tm = w.vehicles.iter().filter(|v| v.kind != jaci_sim::vehicle::Kind::Debris).map(|v| (v.id, jaci_sim::telemetry::values(v, w.t).into_iter().collect())).collect();
        c.phase = w.vehicles.iter().map(|v| (v.id, v.phase.id().to_string())).collect();
        c.met = w.vehicles.iter().filter_map(|v| v.launch_time.map(|l| (v.id, w.t - l))).collect();
        c.link = w.vehicles.iter().map(|v| (v.id, v.systems.as_ref().is_some_and(|s| s.comms.link.is_some()) || v.phase == Phase::Prelaunch)).collect();
        c.alarms = w.alarms.iter().filter(|a| a.active).map(|a| (a.vehicle, a.point.clone(), format!("{:?}", a.level).to_lowercase())).collect();
        c.count = (format!("{:?}", w.launch.state).to_lowercase(), w.launch.t_minus);
    }

    pub fn status(&self) -> Value {
        json!(self.list.iter().map(|s| json!({"name": s.name, "enabled": s.enabled, "error": s.error, "runs": s.runs})).collect::<Vec<_>>())
    }
}

/// Executa o que o script pediu.
fn apply(w: &mut World, name: &str, out: Vec<Action>) {
    for a in out {
        match a {
            Action::Log(level, text) => w.event(None, level, format!("[script {name}] {text}")),
            Action::Cmd { vehicle, cmd, onboard, armed } => {
                let Some(def) = commands::find(&cmd.id) else {
                    w.event(None, Level::Warn, format!("[script {name}] comando desconhecido: {}", cmd.id));
                    continue;
                };
                let result = if onboard {
                    if def.hazardous {
                        Err("regra de bordo não manda comando perigoso".to_string())
                    } else {
                        w.execute(vehicle, &cmd).map(|m| {
                            let vn = w.vehicle(vehicle).map(|v| v.name.clone()).unwrap_or_default();
                            w.event(Some(vehicle), Level::Info, format!("[script {name}] a bordo de {vn}: {} — {m}", cmd.describe()));
                        })
                    }
                } else {
                    w.send(vehicle, cmd.clone(), armed, None).map(|_| ())
                };
                if let Err(e) = result {
                    w.event(Some(vehicle), Level::Warn, format!("[script {name}] {}: {e}", cmd.id));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jaci_sim::{scenarios, time::from_calendar};

    fn world() -> (World, VehicleId) {
        let w = scenarios::build("estacao", from_calendar(2026, 10, 19, 15, 0, 0.0), 7).unwrap();
        let id = w.vehicles.iter().find(|v| v.kind == jaci_sim::vehicle::Kind::Station).unwrap().id;
        (w, id)
    }

    fn with(src: &str) -> Scripts {
        let mut s = Scripts::new();
        let ast = s.compile(src).unwrap();
        s.list.push(Script { name: "teste".into(), source: src.into(), ast: Some(ast), enabled: true, mem: Map::new(), flags: HashMap::new(), error: None, runs: 0 });
        s
    }

    fn step(s: &mut Scripts, w: &mut World, secs: usize) {
        for _ in 0..secs {
            w.step(1.0);
            s.tick(w);
        }
    }

    #[test]
    fn le_o_estado_e_lembra_entre_execucoes() {
        let (mut w, _) = world();
        let mut s = with(
            r#"
            let est = vehicle("Estação espacial");
            if !mem.contains("n") { mem.n = 0; }
            mem.n += 1;
            if once("inicio") { log(`começou; altitude ${tm(est, "nav.alt").to_int()} km, fase ${phase(est)}`); }
            if every(10.0, "tique") { log("tique"); }
            "#,
        );
        step(&mut s, &mut w, 25);
        assert_eq!(s.list[0].runs, 25);
        assert_eq!(s.list[0].mem.get("n").unwrap().as_int().unwrap(), 25);
        let ours: Vec<&str> = w.events.iter().filter(|e| e.text.starts_with("[script teste]")).map(|e| e.text.as_str()).collect();
        assert!(ours[0].contains("começou; altitude 4"), "{ours:?}");
        assert!(ours[0].contains("fase orbit"), "{ours:?}");
        assert_eq!(ours.iter().filter(|t| t.ends_with("tique")).count(), 3);
    }

    #[test]
    fn comando_de_solo_e_regra_de_bordo() {
        let (mut w, id) = world();
        let mut s = with(
            r#"
            let est = vehicle("Estação espacial");
            if once("solo") { cmd(est, "att_mode", #{ mode: "sun" }); }
            if once("bordo") { onboard(est, "safe_mode", #{ on: "on" }); }
            "#,
        );
        step(&mut s, &mut w, 3);
        assert!(w.vehicle(id).unwrap().safe_mode, "regra de bordo não executou");
        assert!(w.commands.iter().any(|c| c.text.contains("tude") || c.vehicle == id), "comando de solo não foi enviado");
    }

    #[test]
    fn erro_desliga_e_laco_infinito_para() {
        let (mut w, _) = world();
        let mut s = with("let x = 1; x.nao_existe();");
        step(&mut s, &mut w, 2);
        assert!(!s.list[0].enabled);
        assert!(s.list[0].error.is_some());
        assert!(w.events.iter().any(|e| e.text.contains("[script teste] parou")));
        let mut s = with("loop { }");
        step(&mut s, &mut w, 1);
        assert!(!s.list[0].enabled, "laço infinito não foi parado");
        // Sem eval.
        assert!(Scripts::new().compile("eval(\"1\")").is_err());
    }

    #[test]
    fn perigoso_so_armado_e_nunca_de_bordo() {
        let (mut w, id) = world();
        let mut s = with(r#"let v = vehicle("Estação espacial"); if once("a") { onboard(v, "fts_terminate"); cmd(v, "fts_terminate"); }"#);
        step(&mut s, &mut w, 2);
        assert_ne!(w.vehicle(id).unwrap().phase, Phase::Destroyed);
        assert_eq!(w.events.iter().filter(|e| e.text.contains("fts_terminate")).count(), 2);
    }

    #[test]
    fn exemplos_compilam_e_a_regra_de_bordo_age() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../missoes/scripts");
        let mut s = Scripts::new();
        s.load(&dir);
        assert!(s.list.len() >= 2);
        for sc in &s.list {
            assert!(sc.error.is_none(), "{}: {:?}", sc.name, sc.error);
        }
        let mut w = scenarios::build("observacao", from_calendar(2026, 10, 19, 15, 0, 0.0), 7).unwrap();
        let id = w.vehicles.iter().find(|v| v.systems.is_some()).unwrap().id;
        s.set_enabled("bateria-de-bordo", true).unwrap();
        step(&mut s, &mut w, 2);
        assert!(!w.vehicle(id).unwrap().safe_mode);
        // Bateria quase vazia: a regra de bordo entra em modo de segurança sozinha.
        w.vehicles.iter_mut().find(|v| v.id == id).unwrap().systems.as_mut().unwrap().power.soc = 0.2;
        step(&mut s, &mut w, 2);
        assert!(w.vehicle(id).unwrap().safe_mode, "{:?}", w.events.iter().rev().take(5).map(|e| &e.text).collect::<Vec<_>>());
        assert!(s.list.iter().find(|x| x.name == "bateria-de-bordo").unwrap().enabled);
    }
}
