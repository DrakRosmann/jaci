//! Integração com o Kerbal Space Program pelo kOS (e pelo kRPC, em krpc.rs).
//!
//! O script `jaci.ks` roda no computador kOS de cada nave e conversa com o
//! Jaci por arquivos na pasta `Ships/Script/jaci` do KSP (o volume 0 do kOS):
//! `tm-<id>.txt`, que ele escreve (telemetria, respostas e eventos), e
//! `cmd-<id>.txt`, que o Jaci reescreve (comandos numerados). O formato está
//! em docs/protocolo.md. A ponte do kRPC produz as mesmas linhas e aceita os
//! mesmos comandos, então as naves das duas pontes são tratadas igual.

use jaci_sim::{
    commands::{ArgDef, CmdDef, Command},
    math::Vec3,
    orbit::{Elements, propagate_kepler, state_from_elements},
    telemetry::TmDef,
    vehicle::Level,
    world::{CmdRecord, CmdStatus, World},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    time::Instant,
};

pub const FIRST_ID: u32 = 10_000;

const SCRIPT: &str = include_str!("../../ksp/Script/jaci/jaci.ks");
const BOOT: &str = include_str!("../../ksp/Script/boot/jaci.ks");
pub const MM_PATCH: &str = include_str!("../../ksp/GameData/JaciKOS/jaci-kos.cfg");

/// Pastas onde o Steam costuma instalar o KSP.
pub fn find_ksp() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let rel = "steamapps/common/Kerbal Space Program";
    let mut cands = vec![
        home.join(".steam/steam").join(rel),
        home.join(".local/share/Steam").join(rel),
        home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam").join(rel),
        home.join("Library/Application Support/Steam").join(rel),
        PathBuf::from("C:/Program Files (x86)/Steam").join(rel),
        PathBuf::from("C:/Program Files/Steam").join(rel),
    ];
    // Bibliotecas extras do Steam.
    for vdf in [home.join(".steam/steam/steamapps/libraryfolders.vdf"), home.join(".local/share/Steam/steamapps/libraryfolders.vdf")] {
        if let Ok(txt) = fs::read_to_string(vdf) {
            for line in txt.lines() {
                let l = line.trim();
                if let Some(rest) = l.strip_prefix("\"path\"") {
                    let p = rest.trim().trim_matches('"').replace("\\\\", "/");
                    cands.push(PathBuf::from(p).join(rel));
                }
            }
        }
    }
    cands.into_iter().find(|p| p.join("GameData").is_dir())
}

/// Corpo celeste do KSP como o kOS informa.
#[derive(Clone, Debug, Default)]
pub struct KspBody {
    pub name: String,
    pub radius: f64,
    pub mu: f64,
    pub atm_height: f64,
    pub rotation_period: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Countdown {
    pub t_minus: f64,
    pub apo_km: f64,
    pub heading: f64,
    pub announced: Vec<i64>,
}

#[derive(Clone, Debug)]
pub struct KspVessel {
    pub id: u32,
    pub uid: String,
    pub name: String,
    pub body: KspBody,
    /// Valores da última linha T, por nome.
    pub tm: Vec<(String, f64)>,
    pub situation: String,
    pub mode: String,
    pub steer: String,
    pub target: String,
    pub flags: [bool; 5],
    pub ut: f64,
    pub last_rx: Option<Instant>,
    pub packets: u64,
    pub elements: Elements,
    pub r: Vec3,
    pub v: Vec3,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    /// Para estimar a aceleração.
    prev: Option<(f64, Vec3)>,
    pub accel_g: f64,
    /// Comandos escritos: (número, texto, id do registro no mundo).
    pending: Vec<(u64, String, u64)>,
    next_seq: u64,
    offset: u64,
    pub countdown: Option<Countdown>,
    pub resources: Vec<(String, f64, f64)>,
    /// Por qual ponte a nave fala: "kOS" ou "kRPC".
    pub via: &'static str,
}

pub struct KspLink {
    pub ksp_dir: PathBuf,
    pub dir: PathBuf,
    pub vessels: Vec<KspVessel>,
    pub session: String,
    pub patch_installed: bool,
    pub kos_installed: bool,
    pub mm_installed: bool,
    /// A ponte do kOS (por arquivos) está ligada.
    pub kos: bool,
    /// A ponte do kRPC (pela rede), se ligada.
    pub krpc: Option<crate::krpc::Bridge>,
    last_count: Instant,
    /// Ids próprios, longe dos veículos simulados (o mundo recomeça ao trocar de cenário).
    next_id: u32,
}

/// Campos da linha T, na ordem do script.
const FIELDS: [&str; 52] = [
    "seq", "ut", "met", "status", "alt", "lat", "lon", "vorb", "vsurf", "vvert", "ap", "pe", "inc", "ecc", "period", "lan", "argp", "ta", "sma",
    "mass", "thrust", "maxthrust", "throttle", "q", "stage", "lf", "lfc", "ox", "oxc", "mp", "mpc", "ec", "ecc_cap", "sf", "sfc", "pitch", "heading",
    "sas", "rcs", "gear", "lights", "brakes", "eta_ap", "eta_pe", "node_eta", "node_dv", "target", "target_dist", "warp", "crew", "mode", "steer",
];

impl KspLink {
    /// Prepara a ligação: instala os scripts na pasta do KSP.
    pub fn new(ksp_dir: PathBuf) -> anyhow::Result<Self> {
        let script_dir = ksp_dir.join("Ships/Script");
        let dir = script_dir.join("jaci");
        fs::create_dir_all(&dir)?;
        fs::create_dir_all(script_dir.join("boot"))?;
        write_if_changed(&dir.join("jaci.ks"), SCRIPT)?;
        write_if_changed(&script_dir.join("boot/jaci.ks"), BOOT)?;
        let mut session = [0u8; 4];
        getrandom::fill(&mut session).ok();
        let session: String = session.iter().map(|b| format!("{b:02x}")).collect();
        let gd = ksp_dir.join("GameData");
        Ok(KspLink {
            patch_installed: gd.join("JaciKOS/jaci-kos.cfg").exists(),
            kos_installed: gd.join("kOS").is_dir(),
            mm_installed: fs::read_dir(&gd).map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with("ModuleManager") && e.file_name().to_string_lossy().ends_with(".dll"))).unwrap_or(false),
            ksp_dir,
            dir,
            vessels: Vec::new(),
            session,
            kos: true,
            krpc: None,
            last_count: Instant::now(),
            next_id: FIRST_ID,
        })
    }

    /// Só com o kRPC: sem pasta do jogo nem scripts do kOS.
    pub fn without_kos() -> Self {
        KspLink {
            ksp_dir: PathBuf::new(),
            dir: PathBuf::new(),
            vessels: Vec::new(),
            session: String::new(),
            patch_installed: false,
            kos_installed: false,
            mm_installed: false,
            kos: false,
            krpc: None,
            last_count: Instant::now(),
            next_id: FIRST_ID,
        }
    }

    /// Fica com as naves e a ponte do kRPC de uma ligação anterior.
    pub fn adopt(&mut self, old: KspLink) {
        self.krpc = old.krpc;
        self.vessels = old.vessels;
        self.next_id = self.next_id.max(old.next_id);
    }

    /// Liga (ou religa em outro endereço) a ponte do kRPC.
    pub fn connect_krpc(&mut self, addr: &str) {
        self.krpc = Some(crate::krpc::Bridge::start(addr));
    }

    /// Instala o patch do Module Manager que põe o kOS nas peças de comando.
    pub fn install_patch(&mut self) -> anyhow::Result<()> {
        let p = self.ksp_dir.join("GameData/JaciKOS/jaci-kos.cfg");
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(p, MM_PATCH)?;
        self.patch_installed = true;
        Ok(())
    }

    pub fn vessel(&self, id: u32) -> Option<&KspVessel> {
        self.vessels.iter().find(|v| v.id == id)
    }

    /// Lê o que as naves escreveram desde a última vez.
    pub fn poll(&mut self, world: &mut World) {
        if self.kos {
            self.poll_files(world);
        }
        self.poll_krpc(world);
        self.countdowns(world);
        // Comandos sem resposta: a ponte parou, ou a nave perdeu o sinal.
        for v in &mut self.vessels {
            for (_, _, rec) in &v.pending {
                if let Some(c) = world.commands.iter_mut().find(|c| c.id == *rec && c.status == CmdStatus::Queued) {
                    if world.t - c.sent > 30.0 && v.age() > 5.0 {
                        c.status = CmdStatus::Failed;
                        c.message = format!("sem resposta do {}", v.via);
                    }
                }
            }
        }
    }

    /// Linhas que a thread do kRPC produziu.
    fn poll_krpc(&mut self, world: &mut World) {
        let Some(b) = &self.krpc else { return };
        for (uid, line) in b.drain() {
            let idx = match self.vessels.iter().position(|v| v.uid == uid) {
                Some(i) => i,
                None => {
                    let id = self.next_id;
                    self.next_id += 1;
                    let mut v = KspVessel::new(id, &uid);
                    v.via = "kRPC";
                    self.vessels.push(v);
                    world.event(Some(id), Level::Good, "KSP: nave ativa pelo kRPC");
                    self.vessels.len() - 1
                }
            };
            self.vessels[idx].line(&line, world);
        }
    }

    /// Arquivos que os scripts do kOS escreveram.
    fn poll_files(&mut self, world: &mut World) {
        let Ok(entries) = fs::read_dir(&self.dir) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(uid) = name.strip_prefix("tm-").and_then(|s| s.strip_suffix(".txt")) else { continue };
            let idx = match self.vessels.iter().position(|v| v.uid == uid) {
                Some(i) => i,
                None => {
                    // Arquivo velho de outra sessão do jogo: ignora.
                    let fresh = e.metadata().and_then(|m| m.modified()).ok().and_then(|m| m.elapsed().ok()).is_some_and(|d| d.as_secs() < 120);
                    if !fresh {
                        continue;
                    }
                    let id = self.next_id;
                    self.next_id += 1;
                    self.vessels.push(KspVessel::new(id, uid));
                    world.event(Some(id), Level::Good, format!("KSP: nave com a ponte do kOS (peça {uid})"));
                    self.vessels.len() - 1
                }
            };
            let path = e.path();
            let lines = read_new(&path, &mut self.vessels[idx].offset);
            for line in lines {
                self.vessels[idx].line(&line, world);
            }
        }
    }

    /// Executa um comando do dicionário na nave.
    pub fn command(&mut self, world: &mut World, id: u32, cmd: &Command, armed: bool) -> Result<u64, String> {
        let def = commands().into_iter().find(|d| d.id == cmd.id).ok_or_else(|| format!("comando desconhecido para nave do KSP: {}", cmd.id))?;
        if def.hazardous && !armed {
            return Err("comando perigoso: arme antes de enviar".into());
        }
        let label = cmd.describe();
        let v = self.vessels.iter_mut().find(|v| v.id == id).ok_or("nave do KSP não encontrada")?;
        match cmd.id.as_str() {
            "ksp_count" => {
                let t = cmd.num("t_minus").unwrap_or(30.0).clamp(1.0, 3_600.0);
                v.countdown = Some(Countdown { t_minus: t, apo_km: cmd.num("apo").unwrap_or(80.0), heading: cmd.num("heading").unwrap_or(90.0), announced: vec![] });
                world.event(Some(id), Level::Info, format!("KSP: contagem iniciada em T-{t:.0} s"));
                Ok(0)
            }
            "ksp_count_abort" => {
                if v.countdown.take().is_some() {
                    world.event(Some(id), Level::Warn, "KSP: contagem cancelada");
                }
                Ok(0)
            }
            _ => {
                let text = translate(cmd)?;
                self.send(world, id, &text, &label)
            }
        }
    }

    /// Manda um comando para a nave (texto do protocolo do script).
    pub fn send(&mut self, world: &mut World, id: u32, text: &str, label: &str) -> Result<u64, String> {
        let session = self.session.clone();
        let dir = self.dir.clone();
        let v = self.vessels.iter_mut().find(|v| v.id == id).ok_or("nave do KSP não encontrada")?;
        let seq = v.next_seq;
        v.next_seq += 1;
        let rec = world.next_seq;
        world.next_seq += 1;
        world.commands.push(CmdRecord {
            id: rec,
            vehicle: id,
            text: label.to_string(),
            sent: world.t,
            exec_at: world.t,
            status: CmdStatus::Queued,
            message: format!("aguardando o {}", v.via),
            cmd: None,
        });
        v.pending.push((seq, text.to_string(), rec));
        if v.pending.len() > 40 {
            v.pending.drain(..v.pending.len() - 40);
        }
        if v.via == "kRPC" {
            let b = self.krpc.as_ref().ok_or("a ponte do kRPC está desligada")?;
            b.send(seq, text);
            return Ok(rec);
        }
        // Reescreve o arquivo inteiro e troca de uma vez (o kOS nunca lê pela metade).
        let mut body = format!("S {session}\n");
        for (s, t, _) in &v.pending {
            body.push_str(&format!("{s} {t}\n"));
        }
        let path = dir.join(format!("cmd-{}.txt", v.uid));
        let tmp = dir.join(format!(".cmd-{}.tmp", v.uid));
        fs::write(&tmp, body).and_then(|_| fs::rename(&tmp, &path)).map_err(|e| format!("não escrevi o comando: {e}"))?;
        Ok(rec)
    }

    /// Contagens regressivas para o lançamento automático.
    fn countdowns(&mut self, world: &mut World) {
        let dt = self.last_count.elapsed().as_secs_f64();
        self.last_count = Instant::now();
        let mut launches = Vec::new();
        for v in &mut self.vessels {
            let Some(c) = &mut v.countdown else { continue };
            c.t_minus -= dt;
            let whole = c.t_minus.ceil() as i64;
            if (whole <= 10 || whole % 10 == 0) && whole > 0 && !c.announced.contains(&whole) {
                c.announced.push(whole);
                world.event(Some(v.id), Level::Info, format!("KSP: T-{whole}"));
            }
            if c.t_minus <= 0.0 {
                launches.push((v.id, c.apo_km, c.heading));
                v.countdown = None;
            }
        }
        for (id, apo, hdg) in launches {
            world.event(Some(id), Level::Good, "KSP: T-0, ignição e subida automática");
            let _ = self.send(world, id, &format!("launch {apo} {hdg}"), &format!("launch {apo} {hdg}"));
        }
    }

    /// Previsão do traço no solo e da órbita (dois corpos) de uma nave.
    pub fn predict(&self, id: u32, world_t: f64, span: f64, step: f64) -> Vec<Value> {
        let Some(v) = self.vessel(id) else { return vec![] };
        if v.body.mu <= 0.0 || v.elements.a == 0.0 {
            return vec![];
        }
        let w = if v.body.rotation_period != 0.0 { std::f64::consts::TAU / v.body.rotation_period } else { 0.0 };
        let lam0 = v.r.y.atan2(v.r.x);
        let mut out = Vec::new();
        let mut t = 0.0;
        while t <= span {
            let (r, _) = propagate_kepler(v.r, v.v, t, v.body.mu);
            let rn = r.norm();
            if rn < v.body.radius {
                break;
            }
            let lat = (r.z / rn).asin().to_degrees();
            let lon = wrap180(v.lon + (r.y.atan2(r.x) - lam0).to_degrees() - (w * t).to_degrees());
            out.push(json!([world_t + t, lat, lon, (rn - v.body.radius) / 1e3, r.x, r.y, r.z]));
            t += step;
        }
        out
    }
}

fn wrap180(x: f64) -> f64 {
    (x + 180.0).rem_euclid(360.0) - 180.0
}

fn write_if_changed(path: &Path, content: &str) -> std::io::Result<()> {
    if fs::read_to_string(path).ok().as_deref() != Some(content) {
        fs::write(path, content)?;
    }
    Ok(())
}

/// Linhas completas novas de um arquivo; recomeça se ele encolheu (o script o
/// apaga de tempos em tempos).
fn read_new(path: &Path, offset: &mut u64) -> Vec<String> {
    let Ok(mut f) = fs::File::open(path) else { return vec![] };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    if len < *offset {
        *offset = 0;
    }
    if len == *offset || f.seek(SeekFrom::Start(*offset)).is_err() {
        return vec![];
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return vec![];
    }
    // Só até a última quebra de linha: o resto ainda está sendo escrito.
    let Some(end) = buf.iter().rposition(|&b| b == b'\n') else { return vec![] };
    *offset += end as u64 + 1;
    String::from_utf8_lossy(&buf[..end]).lines().map(|l| l.trim_end_matches('\r').to_string()).filter(|l| !l.is_empty()).collect()
}

/// O script manda -1 para o que não existe (período de hipérbole etc.).
fn positive(x: f64) -> f64 {
    if x >= 0.0 { x } else { f64::NAN }
}

fn number(s: &str) -> f64 {
    s.replace(',', ".").parse().unwrap_or(f64::NAN)
}

impl KspVessel {
    fn new(id: u32, uid: &str) -> Self {
        KspVessel {
            id,
            uid: uid.to_string(),
            name: format!("Nave {uid}"),
            body: KspBody::default(),
            tm: Vec::new(),
            situation: String::new(),
            mode: "manual".into(),
            steer: "free".into(),
            target: String::new(),
            flags: [false; 5],
            ut: 0.0,
            last_rx: None,
            packets: 0,
            elements: Elements::default(),
            r: Vec3::ZERO,
            v: Vec3::ZERO,
            lat: 0.0,
            lon: 0.0,
            alt: 0.0,
            prev: None,
            accel_g: 0.0,
            pending: Vec::new(),
            next_seq: 1,
            offset: 0,
            countdown: None,
            resources: Vec::new(),
            via: "kOS",
        }
    }

    pub fn age(&self) -> f64 {
        self.last_rx.map_or(f64::INFINITY, |t| t.elapsed().as_secs_f64())
    }

    fn line(&mut self, line: &str, world: &mut World) {
        let (kind, rest) = line.split_once(' ').unwrap_or((line, ""));
        match kind {
            "H" => {
                if let Some((_, name)) = rest.split_once(' ') {
                    if self.name != name {
                        self.name = name.to_string();
                    }
                }
            }
            "B" => {
                let p: Vec<&str> = rest.split_whitespace().collect();
                if p.len() >= 5 {
                    let name = p[0].replace('_', " ");
                    if !self.body.name.is_empty() && self.body.name != name {
                        world.event(Some(self.id), Level::Info, format!("KSP: {} entrou na esfera de influência de {}", self.name, name));
                    }
                    self.body = KspBody { name, radius: number(p[1]), mu: number(p[2]), atm_height: number(p[3]), rotation_period: number(p[4]) };
                }
            }
            "T" => self.telemetry(rest),
            "R" => {
                let p: Vec<&str> = rest.splitn(3, ' ').collect();
                if p.len() >= 2 {
                    let seq: u64 = p[0].parse().unwrap_or(0);
                    let ok = p[1].eq_ignore_ascii_case("true");
                    let msg = p.get(2).copied().unwrap_or("").to_string();
                    if let Some(&(_, _, rec)) = self.pending.iter().find(|x| x.0 == seq) {
                        if let Some(c) = world.commands.iter_mut().find(|c| c.id == rec) {
                            c.status = if ok { CmdStatus::Executed } else { CmdStatus::Failed };
                            c.message = msg.clone();
                        }
                    }
                    if !ok {
                        world.event(Some(self.id), Level::Warn, format!("KSP: comando recusado pelo kOS: {msg}"));
                    }
                }
            }
            "E" => world.event(Some(self.id), Level::Info, format!("KSP: {rest}")),
            _ => {}
        }
    }

    fn telemetry(&mut self, rest: &str) {
        let p: Vec<&str> = rest.split_whitespace().collect();
        if p.len() < FIELDS.len() {
            return;
        }
        let f = |name: &str| -> f64 { FIELDS.iter().position(|x| *x == name).map_or(f64::NAN, |i| number(p[i])) };
        let s = |name: &str| -> String { FIELDS.iter().position(|x| *x == name).map_or(String::new(), |i| p[i].to_string()) };
        self.last_rx = Some(Instant::now());
        self.packets += 1;
        let ut = f("ut");
        self.situation = s("status");
        self.mode = s("mode");
        self.steer = s("steer");
        self.target = s("target").replace('_', " ");
        self.flags = ["sas", "rcs", "gear", "lights", "brakes"].map(|k| s(k).eq_ignore_ascii_case("true"));
        self.lat = f("lat");
        self.lon = f("lon");
        self.alt = f("alt");
        // Estado inercial pelos elementos (o referencial cru do KSP gira).
        let el = Elements {
            a: f("sma"),
            e: f("ecc"),
            i: f("inc").to_radians(),
            raan: f("lan").to_radians(),
            argp: f("argp").to_radians(),
            nu: f("ta").to_radians(),
        };
        if el.a.is_finite() && el.a != 0.0 && self.body.mu > 0.0 && el.e.is_finite() {
            let (r, v) = state_from_elements(&el, self.body.mu);
            if r.is_finite() && v.is_finite() {
                if let Some((t0, v0)) = self.prev {
                    let dt = ut - t0;
                    if dt > 0.05 {
                        // Aceleração não gravitacional aproximada.
                        let g = r * (-self.body.mu / r.norm().powi(3));
                        self.accel_g = ((v - v0) / dt - g).norm() / 9.80665;
                    }
                }
                self.prev = Some((ut, v));
                self.r = r;
                self.v = v;
                self.elements = el;
            }
        }
        self.ut = ut;
        let pct = |a: f64, c: f64| if c > 0.0 { a / c * 100.0 } else { f64::NAN };
        let mass = f("mass");
        let thrust = f("thrust");
        let g_local = if self.body.mu > 0.0 { self.body.mu / (self.body.radius + self.alt).powi(2) } else { 9.81 };
        self.resources = vec![
            ("LiquidFuel".into(), f("lf"), f("lfc")),
            ("Oxidizer".into(), f("ox"), f("oxc")),
            ("MonoPropellant".into(), f("mp"), f("mpc")),
            ("ElectricCharge".into(), f("ec"), f("ecc_cap")),
            ("SolidFuel".into(), f("sf"), f("sfc")),
        ];
        self.tm = vec![
            ("nav.met".into(), f("met")),
            ("nav.alt".into(), self.alt / 1e3),
            ("nav.lat".into(), self.lat),
            ("nav.lon".into(), self.lon),
            ("nav.speed".into(), f("vorb")),
            ("nav.hspeed".into(), f("vsurf")),
            ("nav.vspeed".into(), f("vvert")),
            ("nav.q".into(), f("q")),
            ("nav.accel".into(), self.accel_g),
            ("nav.pitch".into(), f("pitch")),
            ("nav.heading".into(), f("heading")),
            ("nav.mass".into(), mass),
            ("nav.twr".into(), if mass > 0.0 { thrust / (mass * g_local) } else { f64::NAN }),
            ("orb.ap".into(), f("ap") / 1e3),
            ("orb.pe".into(), f("pe") / 1e3),
            ("orb.inc".into(), f("inc")),
            ("orb.ecc".into(), f("ecc")),
            ("orb.period".into(), positive(f("period")) / 60.0),
            ("orb.raan".into(), f("lan")),
            ("orb.argp".into(), f("argp")),
            ("orb.ta".into(), f("ta")),
            ("orb.sma".into(), f("sma") / 1e3),
            ("orb.eta_ap".into(), positive(f("eta_ap"))),
            ("orb.eta_pe".into(), positive(f("eta_pe"))),
            ("prop.thrust".into(), thrust),
            ("prop.max_thrust".into(), f("maxthrust")),
            ("prop.throttle".into(), f("throttle") * 100.0),
            ("ksp.stage".into(), f("stage")),
            ("res.lf".into(), pct(f("lf"), f("lfc"))),
            ("res.ox".into(), pct(f("ox"), f("oxc"))),
            ("res.mp".into(), pct(f("mp"), f("mpc"))),
            ("res.ec".into(), pct(f("ec"), f("ecc_cap"))),
            ("res.sf".into(), pct(f("sf"), f("sfc"))),
            ("node.eta".into(), if f("node_eta") >= 0.0 { f("node_eta") } else { f64::NAN }),
            ("node.dv".into(), if f("node_eta") >= 0.0 { f("node_dv") } else { f64::NAN }),
            ("tgt.dist".into(), if f("target_dist") >= 0.0 { f("target_dist") / 1e3 } else { f64::NAN }),
            ("ksp.warp".into(), f("warp")),
            ("ksp.crew".into(), f("crew")),
        ];
    }

    /// Fase no vocabulário do Jaci.
    pub fn phase(&self) -> (&'static str, &'static str) {
        if self.age() > 15.0 {
            return ("external", "Sem sinal");
        }
        match self.situation.as_str() {
            "PRELAUNCH" => ("prelaunch", "Pré-lançamento"),
            "FLYING" => ("ascent", "Voando"),
            "SUB_ORBITAL" => ("coast", "Voo balístico"),
            "ORBITING" => ("orbit", "Órbita"),
            "ESCAPING" => ("escape", "Escape"),
            "LANDED" => ("landed", "Pousado"),
            "SPLASHED" => ("splashdown", "Amerissado"),
            "DOCKED" => ("docked", "Acoplado"),
            _ => ("external", "Desconhecida"),
        }
    }

    pub fn tm_value(&self, k: &str) -> f64 {
        self.tm.iter().find(|(id, _)| id == k).map_or(f64::NAN, |x| x.1)
    }
}

// ───────────────────────── Dicionários ─────────────────────────

fn tm(id: &str, name: &str, unit: &'static str, group: &'static str) -> TmDef {
    TmDef { id: id.into(), name: name.into(), unit, group, yellow: None, red: None }
}

pub fn tm_defs() -> Vec<TmDef> {
    let mut ec = tm("res.ec", "Carga elétrica", "%", "Recursos");
    ec.yellow = Some((25.0, 101.0));
    ec.red = Some((8.0, 101.0));
    vec![
        tm("nav.met", "Tempo de missão", "s", "Navegação"),
        tm("nav.alt", "Altitude", "km", "Navegação"),
        tm("nav.lat", "Latitude", "°", "Navegação"),
        tm("nav.lon", "Longitude", "°", "Navegação"),
        tm("nav.speed", "Velocidade orbital", "m/s", "Navegação"),
        tm("nav.hspeed", "Velocidade em relação à superfície", "m/s", "Navegação"),
        tm("nav.vspeed", "Velocidade vertical", "m/s", "Navegação"),
        tm("nav.q", "Pressão dinâmica", "kPa", "Navegação"),
        tm("nav.accel", "Aceleração", "g", "Navegação"),
        tm("nav.pitch", "Arfagem (sobre o horizonte)", "°", "Navegação"),
        tm("nav.heading", "Rumo", "°", "Navegação"),
        tm("nav.mass", "Massa", "t", "Navegação"),
        tm("nav.twr", "Empuxo/peso", "", "Navegação"),
        tm("orb.ap", "Apoastro", "km", "Órbita"),
        tm("orb.pe", "Periastro", "km", "Órbita"),
        tm("orb.inc", "Inclinação", "°", "Órbita"),
        tm("orb.ecc", "Excentricidade", "", "Órbita"),
        tm("orb.period", "Período", "min", "Órbita"),
        tm("orb.raan", "Nodo ascendente", "°", "Órbita"),
        tm("orb.argp", "Argumento do periastro", "°", "Órbita"),
        tm("orb.ta", "Anomalia verdadeira", "°", "Órbita"),
        tm("orb.sma", "Semieixo maior", "km", "Órbita"),
        tm("orb.eta_ap", "Tempo até o apoastro", "s", "Órbita"),
        tm("orb.eta_pe", "Tempo até o periastro", "s", "Órbita"),
        tm("prop.thrust", "Empuxo", "kN", "Propulsão"),
        tm("prop.max_thrust", "Empuxo máximo", "kN", "Propulsão"),
        tm("prop.throttle", "Potência", "%", "Propulsão"),
        tm("ksp.stage", "Estágio atual", "", "Propulsão"),
        tm("res.lf", "Combustível líquido", "%", "Recursos"),
        tm("res.ox", "Oxidante", "%", "Recursos"),
        tm("res.mp", "Monopropelente", "%", "Recursos"),
        ec,
        tm("res.sf", "Combustível sólido", "%", "Recursos"),
        tm("node.eta", "Tempo até a manobra", "s", "Manobras"),
        tm("node.dv", "Δv da manobra", "m/s", "Manobras"),
        tm("tgt.dist", "Distância ao alvo", "km", "Encontro"),
        tm("ksp.warp", "Aceleração do tempo", "", "KSP"),
        tm("ksp.crew", "Tripulação", "", "KSP"),
    ]
}

fn a(name: &'static str, label: &'static str, kind: &'static str, unit: Option<&'static str>, options: Vec<(&'static str, &'static str)>, optional: bool) -> ArgDef {
    ArgDef { name, label, kind, unit, options, optional }
}

const ON_OFF: [(&str, &str); 2] = [("on", "Ligar"), ("off", "Desligar")];

/// Comandos para naves do KSP.
pub fn commands() -> Vec<CmdDef> {
    let k = vec!["ksp"];
    let d = |id, name, group, description, args: Vec<ArgDef>, hazardous| CmdDef { id, name, group, description, args, hazardous, ground: false, kinds: k.clone() };
    vec![
        d("ksp_count", "Contagem e lançamento automático", "KSP: lançamento", "Contagem regressiva no Jaci; no T-0 o kOS acende, separa os estágios, faz a curva de gravidade e circulariza", vec![a("t_minus", "Contagem", "number", Some("s"), vec![], false), a("apo", "Apoastro alvo", "number", Some("km"), vec![], false), a("heading", "Rumo", "number", Some("°"), vec![], false)], true),
        d("ksp_count_abort", "Cancelar a contagem", "KSP: lançamento", "Para a contagem do lançamento automático", vec![], false),
        d("ksp_launch", "Lançar agora (automático)", "KSP: lançamento", "Subida automática pelo kOS até o apoastro e circularização", vec![a("apo", "Apoastro alvo", "number", Some("km"), vec![], false), a("heading", "Rumo", "number", Some("°"), vec![], false)], true),
        d("ksp_stage", "Separar estágio", "KSP: voo", "Ativa o próximo estágio (barra de espaço)", vec![], true),
        d("ksp_abort", "ABORTO", "KSP: voo", "Aciona o grupo de ação de aborto", vec![], true),
        d("ksp_throttle", "Potência", "KSP: voo", "Potência dos motores", vec![a("value", "Potência", "number", Some("%"), vec![], false)], false),
        d("ksp_stop", "Parar o piloto automático", "KSP: voo", "Solta a atitude e a potência", vec![], false),
        d("ksp_steer", "Atitude", "KSP: atitude", "Para onde o kOS aponta a nave", vec![a("mode", "Modo", "enum", None, vec![("prograde", "Prógrado"), ("retrograde", "Retrógrado"), ("normal", "Normal"), ("antinormal", "Antinormal"), ("radial_out", "Radial para fora"), ("radial_in", "Radial para dentro"), ("up", "Para cima"), ("heading", "Rumo e arfagem"), ("hold", "Manter"), ("sun", "Apontado ao Sol"), ("target", "Alvo"), ("free", "Livre")], false), a("heading", "Rumo", "number", Some("°"), vec![], true), a("pitch", "Arfagem", "number", Some("°"), vec![], true)], false),
        d("ksp_sas", "SAS", "KSP: atitude", "Liga ou desliga o SAS", vec![a("on", "Estado", "enum", None, ON_OFF.to_vec(), false)], false),
        d("ksp_rcs", "RCS", "KSP: atitude", "Liga ou desliga o RCS", vec![a("on", "Estado", "enum", None, ON_OFF.to_vec(), false)], false),
        d("ksp_gear", "Trem de pouso", "KSP: sistemas", "Alterna o trem de pouso", vec![], false),
        d("ksp_lights", "Luzes", "KSP: sistemas", "Alterna as luzes", vec![], false),
        d("ksp_brakes", "Freios", "KSP: sistemas", "Alterna os freios", vec![], false),
        d("ksp_ag", "Grupo de ação", "KSP: sistemas", "Alterna um grupo de ação (1 a 10)", vec![a("n", "Grupo", "number", None, vec![], false)], false),
        d("ksp_chutes", "Paraquedas", "KSP: sistemas", "Abre os paraquedas quando for seguro", vec![], false),
        d("ksp_panels", "Painéis solares", "KSP: sistemas", "Abre ou fecha os painéis", vec![a("on", "Estado", "enum", None, vec![("on", "Abrir"), ("off", "Fechar")], false)], false),
        d("ksp_warp", "Aceleração do tempo", "KSP: sistemas", "Nível de aceleração do tempo do KSP (0 a 7)", vec![a("level", "Nível", "number", None, vec![], false)], false),
        d("ksp_target", "Alvo", "KSP: sistemas", "Escolhe o alvo pelo nome", vec![a("name", "Nome", "text", None, vec![], false)], false),
        d("ksp_circ", "Circularizar", "KSP: manobras", "Cria a manobra de circularização no apoastro ou periastro", vec![a("at", "Onde", "enum", None, vec![("ap", "Apoastro"), ("pe", "Periastro")], false)], false),
        d("ksp_node", "Manobra", "KSP: manobras", "Cria uma manobra (nó) daqui a t segundos", vec![a("t", "Daqui a", "number", Some("s"), vec![], false), a("prograde", "Prógrado", "number", Some("m/s"), vec![], false), a("normal", "Normal", "number", Some("m/s"), vec![], false), a("radial", "Radial", "number", Some("m/s"), vec![], false)], false),
        d("ksp_exec", "Executar a próxima manobra", "KSP: manobras", "O kOS aponta, espera, queima e corta no Δv do nó", vec![], false),
        d("ksp_delnodes", "Apagar manobras", "KSP: manobras", "Remove todos os nós de manobra", vec![], false),
    ]
}

/// Traduz um comando do Jaci para o protocolo do script.
pub fn translate(cmd: &Command) -> Result<String, String> {
    let n = |k: &str, d: f64| cmd.num(k).unwrap_or(d);
    let w = |k: &str, d: &str| cmd.str(k).unwrap_or(d).to_string();
    Ok(match cmd.id.as_str() {
        "ksp_launch" => format!("launch {} {}", n("apo", 80.0), n("heading", 90.0)),
        "ksp_stage" => "stage".into(),
        "ksp_abort" => "abort".into(),
        "ksp_throttle" => format!("throttle {}", n("value", 0.0) / 100.0),
        "ksp_stop" => "stop".into(),
        "ksp_steer" => format!("steer {} {} {}", w("mode", "free"), n("heading", 90.0), n("pitch", 0.0)),
        "ksp_sas" => format!("sas {}", w("on", "on")),
        "ksp_rcs" => format!("rcs {}", w("on", "on")),
        "ksp_gear" => "gear".into(),
        "ksp_lights" => "lights".into(),
        "ksp_brakes" => "brakes".into(),
        "ksp_ag" => format!("ag {}", n("n", 1.0)),
        "ksp_chutes" => "chutes".into(),
        "ksp_panels" => format!("panels {}", w("on", "on")),
        "ksp_warp" => format!("warp {}", n("level", 0.0)),
        "ksp_target" => format!("target {}", w("name", "").replace(' ', "_")),
        "ksp_circ" => format!("circ {}", w("at", "ap")),
        "ksp_node" => format!("node {} {} {} {}", n("t", 60.0), n("radial", 0.0), n("normal", 0.0), n("prograde", 0.0)),
        "ksp_exec" => "exec".into(),
        "ksp_delnodes" => "delnodes".into(),
        other => return Err(format!("comando sem tradução para o kOS: {other}")),
    })
}

/// A nave no formato de veículo do instantâneo.
pub fn vehicle_json(v: &KspVessel, world_t: f64) -> Value {
    let (phase, phase_name) = v.phase();
    let tm: serde_json::Map<String, Value> = v
        .tm
        .iter()
        .map(|(k, x)| (k.clone(), if x.is_finite() { json!(x) } else { Value::Null }))
        .collect();
    let node = if v.tm_value("node.dv").is_finite() {
        json!([{
            "id": 0, "label": "Manobra do KSP", "t": world_t + v.tm_value("node.eta"), "dv": [0, 0, 0],
            "dv_total": v.tm_value("node.dv"), "executing": v.mode == "node", "done": false, "remaining": v.tm_value("node.dv"), "rcs": false, "burn": null,
        }])
    } else {
        json!([])
    };
    let pe = v.tm_value("orb.pe");
    let ap = v.tm_value("orb.ap");
    let fin = |x: f64| if x.is_finite() { json!(x) } else { Value::Null };
    let ksp = json!({
        "uid": v.uid,
        "via": v.via,
        "situation": v.situation,
        "mode": v.mode,
        "steer": v.steer,
        "target": v.target,
        "sas": v.flags[0], "rcs": v.flags[1], "gear": v.flags[2], "lights": v.flags[3], "brakes": v.flags[4],
        "stage": fin(v.tm_value("ksp.stage")),
        "warp": fin(v.tm_value("ksp.warp")),
        "age": fin(v.age()),
        "packets": v.packets,
        "ut": v.ut,
        "body": {"name": v.body.name, "radius": v.body.radius, "mu": v.body.mu, "atm": v.body.atm_height, "rotation": v.body.rotation_period},
        "resources": v.resources.iter().map(|(n, a, c)| json!({"name": n, "amount": fin(*a), "capacity": fin(*c)})).collect::<Vec<_>>(),
        "countdown": v.countdown.as_ref().map(|c| json!({"t_minus": c.t_minus, "apo": c.apo_km, "heading": c.heading})),
    });
    json!({
        "id": v.id,
        "name": v.name,
        "kind": "ksp",
        "kind_name": if v.via == "kRPC" { "Nave do KSP (kRPC)" } else { "Nave do KSP (kOS)" },
        "phase": phase,
        "phase_name": phase_name,
        "central": v.body.name.to_lowercase(),
        "central_name": v.body.name,
        "central_radius": v.body.radius,
        "central_rotation": v.body.rotation_period,
        "crew": v.tm_value("ksp.crew").max(0.0) as u32,
        "met": fin(v.tm_value("nav.met")),
        "lat": fin(v.lat), "lon": fin(v.lon), "alt": fin(v.alt / 1e3),
        "speed": fin(v.tm_value("nav.speed")),
        "r": [v.r.x, v.r.y, v.r.z], "v": [v.v.x, v.v.y, v.v.z],
        "pe": fin(pe), "ap": fin(ap), "inc": fin(v.tm_value("orb.inc")), "ecc": fin(v.tm_value("orb.ecc")),
        "tm": tm,
        "nodes": node,
        "ksp": ksp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linha_de_telemetria_tem_todos_os_campos() {
        // A linha T do script (sem o "T") tem os campos na ordem de FIELDS.
        let script = SCRIPT;
        let start = script.find("out(\"T \" + seq").expect("linha T");
        let end = script[start..].find(").\n").unwrap() + start;
        let n = script[start..end].matches("\" \" +").count() + 1;
        assert_eq!(n, FIELDS.len(), "o script e o Jaci discordam no número de campos");
    }

    #[test]
    fn le_telemetria_e_responde_comandos() {
        let dir = std::env::temp_dir().join(format!("jaci-ksp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("GameData")).unwrap();
        let mut link = KspLink::new(dir.clone()).unwrap();
        assert!(dir.join("Ships/Script/jaci/jaci.ks").exists());
        assert!(dir.join("Ships/Script/boot/jaci.ks").exists());
        let mut w = World::new(0.0, 1);
        let tm = link.dir.join("tm-123.txt");
        let fields = [
            "0", "1000.5", "12.3", "FLYING", "15000", "-0.09", "-74.55", "2300", "1800", "350", "60000", "-500000", "0.1", "0.01", "1200", "10", "20", "30",
            "680000", "12.5", "250", "300", "0.8", "18.2", "3", "100", "200", "120", "240", "5", "10", "150", "200", "0", "0", "60", "90",
            "False", "True", "False", "False", "False", "45", "1500", "-1", "0", "-", "-1", "0", "1", "ascent", "free",
        ];
        assert_eq!(fields.len(), FIELDS.len());
        fs::write(&tm, format!("H 123 Kerbal X\nB Kerbin 600000 3531600000000 70000 21549.425\nT {}\nE Teste\n", fields.join(" "))).unwrap();
        link.poll(&mut w);
        assert_eq!(link.vessels.len(), 1);
        let v = &link.vessels[0];
        assert_eq!(v.name, "Kerbal X");
        assert_eq!(v.body.name, "Kerbin");
        assert!((v.tm_value("nav.alt") - 15.0).abs() < 1e-9);
        assert!((v.tm_value("res.lf") - 50.0).abs() < 1e-9);
        assert!(v.flags[1]);
        let id = v.id;
        let rec = link.send(&mut w, id, "stage", "ksp_stage").unwrap();
        let cmd = fs::read_to_string(link.dir.join("cmd-123.txt")).unwrap();
        assert!(cmd.starts_with("S ") && cmd.contains("\n1 stage\n"), "{cmd}");
        fs::write(&tm, format!("{}R 1 True stage\n", fs::read_to_string(&tm).unwrap())).unwrap();
        link.poll(&mut w);
        assert_eq!(w.commands.iter().find(|c| c.id == rec).unwrap().status, CmdStatus::Executed);
        assert!(!link.predict(id, 0.0, 3000.0, 60.0).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
