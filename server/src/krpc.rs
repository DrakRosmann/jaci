//! Integração com o Kerbal Space Program pelo kRPC, a alternativa ao kOS.
//!
//! O kRPC é um mod que abre um servidor RPC dentro do jogo (TCP, porta 50000
//! por padrão). Uma thread conecta, lê a nave ativa cinco vezes por segundo e
//! produz as mesmas linhas do script do kOS (H, B, T, R e E, veja
//! docs/protocolo.md); os comandos chegam no mesmo texto (`stage`,
//! `throttle 0.5`…) e viram chamadas de RPC. A subida automática, a
//! circularização e a execução de manobras rodam aqui, com a mesma lógica do
//! script. Assim o resto do Jaci (dicionário, telas, comandos) vale para os
//! dois caminhos.
//!
//! O protocolo do kRPC é protobuf com o tamanho de cada mensagem em varint na
//! frente; as poucas mensagens que usamos são escritas à mão, sem dependência.

use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

// ───────────────────────── Protobuf mínimo ─────────────────────────

pub mod wire {
    pub fn put_varint(buf: &mut Vec<u8>, mut v: u64) {
        loop {
            let b = (v & 0x7f) as u8;
            v >>= 7;
            if v == 0 {
                buf.push(b);
                return;
            }
            buf.push(b | 0x80);
        }
    }

    pub fn get_varint(buf: &[u8], pos: &mut usize) -> Option<u64> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = *buf.get(*pos)?;
            *pos += 1;
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }

    pub fn zigzag(i: i64) -> u64 {
        ((i << 1) ^ (i >> 63)) as u64
    }

    pub fn unzigzag(u: u64) -> i64 {
        ((u >> 1) as i64) ^ -((u & 1) as i64)
    }

    /// Um campo lido de uma mensagem.
    #[derive(Clone, Debug, PartialEq)]
    pub enum Field {
        Varint(u64),
        Bytes(Vec<u8>),
        Fixed64(u64),
        Fixed32(u32),
    }

    pub fn field_varint(buf: &mut Vec<u8>, n: u32, v: u64) {
        put_varint(buf, u64::from(n) << 3);
        put_varint(buf, v);
    }

    pub fn field_bytes(buf: &mut Vec<u8>, n: u32, b: &[u8]) {
        put_varint(buf, (u64::from(n) << 3) | 2);
        put_varint(buf, b.len() as u64);
        buf.extend_from_slice(b);
    }

    pub fn parse(msg: &[u8]) -> Option<Vec<(u32, Field)>> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos < msg.len() {
            let key = get_varint(msg, &mut pos)?;
            let n = (key >> 3) as u32;
            let f = match key & 7 {
                0 => Field::Varint(get_varint(msg, &mut pos)?),
                1 => {
                    let b = msg.get(pos..pos + 8)?;
                    pos += 8;
                    Field::Fixed64(u64::from_le_bytes(b.try_into().ok()?))
                }
                2 => {
                    let len = get_varint(msg, &mut pos)? as usize;
                    let b = msg.get(pos..pos + len)?.to_vec();
                    pos += len;
                    Field::Bytes(b)
                }
                5 => {
                    let b = msg.get(pos..pos + 4)?;
                    pos += 4;
                    Field::Fixed32(u32::from_le_bytes(b.try_into().ok()?))
                }
                _ => return None,
            };
            out.push((n, f));
        }
        Some(out)
    }

    pub fn bytes_of(fields: &[(u32, Field)], n: u32) -> Vec<&[u8]> {
        fields.iter().filter(|(k, _)| *k == n).filter_map(|(_, f)| if let Field::Bytes(b) = f { Some(b.as_slice()) } else { None }).collect()
    }

    pub fn varint_of(fields: &[(u32, Field)], n: u32) -> Option<u64> {
        fields.iter().find(|(k, _)| *k == n).and_then(|(_, f)| if let Field::Varint(v) = f { Some(*v) } else { None })
    }
}

/// Valores no formato do kRPC (sem a etiqueta do campo).
pub mod val {
    use super::wire::*;

    pub fn f64(x: f64) -> Vec<u8> {
        x.to_le_bytes().to_vec()
    }
    pub fn f32(x: f64) -> Vec<u8> {
        (x as f32).to_le_bytes().to_vec()
    }
    pub fn boolean(b: bool) -> Vec<u8> {
        vec![u8::from(b)]
    }
    pub fn obj(id: u64) -> Vec<u8> {
        let mut b = Vec::new();
        put_varint(&mut b, id);
        b
    }
    pub fn uint(x: u64) -> Vec<u8> {
        obj(x)
    }
    pub fn sint(i: i64) -> Vec<u8> {
        obj(zigzag(i))
    }
    pub fn string(s: &str) -> Vec<u8> {
        let mut b = Vec::new();
        put_varint(&mut b, s.len() as u64);
        b.extend_from_slice(s.as_bytes());
        b
    }
    /// Tupla (ou lista): mensagem com os itens no campo 1.
    pub fn tuple(items: &[Vec<u8>]) -> Vec<u8> {
        let mut b = Vec::new();
        for it in items {
            field_bytes(&mut b, 1, it);
        }
        b
    }

    pub fn as_f64(b: &[u8]) -> f64 {
        b.get(..8).and_then(|x| x.try_into().ok()).map_or(f64::NAN, f64::from_le_bytes)
    }
    pub fn as_f32(b: &[u8]) -> f64 {
        b.get(..4).and_then(|x| x.try_into().ok()).map_or(f64::NAN, |x| f64::from(f32::from_le_bytes(x)))
    }
    pub fn as_u64(b: &[u8]) -> u64 {
        get_varint(b, &mut 0).unwrap_or(0)
    }
    pub fn as_i64(b: &[u8]) -> i64 {
        unzigzag(as_u64(b))
    }
    pub fn as_bool(b: &[u8]) -> bool {
        as_u64(b) != 0
    }
    pub fn as_string(b: &[u8]) -> String {
        let mut pos = 0;
        let Some(len) = get_varint(b, &mut pos) else { return String::new() };
        b.get(pos..pos + len as usize).map(|s| String::from_utf8_lossy(s).into_owned()).unwrap_or_default()
    }
    /// Itens de uma lista ou tupla.
    pub fn items(b: &[u8]) -> Vec<Vec<u8>> {
        parse(b).map(|f| bytes_of(&f, 1).into_iter().map(<[u8]>::to_vec).collect()).unwrap_or_default()
    }
    /// Entradas de um dicionário: (chave, valor).
    pub fn dict(b: &[u8]) -> Vec<(Vec<u8>, Vec<u8>)> {
        let Some(f) = parse(b) else { return vec![] };
        bytes_of(&f, 1)
            .into_iter()
            .filter_map(|e| {
                let ef = parse(e)?;
                Some((bytes_of(&ef, 1).first().map_or(vec![], |x| x.to_vec()), bytes_of(&ef, 2).first().map_or(vec![], |x| x.to_vec())))
            })
            .collect()
    }
    pub fn vec3(b: &[u8]) -> [f64; 3] {
        let it = items(b);
        [0, 1, 2].map(|i| it.get(i).map_or(f64::NAN, |x| as_f64(x)))
    }
}

// ───────────────────────── Cliente ─────────────────────────

/// Uma chamada de procedimento: serviço, nome e argumentos já codificados.
#[derive(Clone, Debug)]
pub struct Call {
    pub service: &'static str,
    pub procedure: String,
    pub args: Vec<Vec<u8>>,
}

pub fn call(service: &'static str, procedure: impl Into<String>, args: Vec<Vec<u8>>) -> Call {
    Call { service, procedure: procedure.into(), args }
}

/// Atalho para o serviço SpaceCenter, onde mora quase tudo.
fn sc(procedure: impl Into<String>, args: Vec<Vec<u8>>) -> Call {
    call("SpaceCenter", procedure, args)
}

pub struct Client {
    stream: TcpStream,
}

fn error_text(b: &[u8]) -> String {
    let f = wire::parse(b).unwrap_or_default();
    let s = |n| wire::bytes_of(&f, n).first().map(|x| String::from_utf8_lossy(x).into_owned()).unwrap_or_default();
    let (name, desc) = (s(2), s(3));
    if desc.is_empty() { name } else { desc }
}

impl Client {
    /// Conecta e faz o aperto de mão do kRPC (pedido de conexão RPC).
    pub fn connect(addr: &str, name: &str) -> Result<Self, String> {
        let sa = addr.to_socket_addrs().map_err(|e| format!("endereço inválido {addr}: {e}"))?.next().ok_or_else(|| format!("endereço inválido {addr}"))?;
        let stream = TcpStream::connect_timeout(&sa, Duration::from_secs(3)).map_err(|e| format!("sem servidor kRPC em {addr}: {e}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
        stream.set_nodelay(true).ok();
        let mut c = Client { stream };
        let mut req = Vec::new();
        wire::field_varint(&mut req, 1, 0); // tipo RPC
        wire::field_bytes(&mut req, 2, name.as_bytes());
        c.send(&req)?;
        let resp = c.recv()?;
        let f = wire::parse(&resp).ok_or("resposta de conexão ilegível")?;
        let status = wire::varint_of(&f, 1).unwrap_or(0);
        if status != 0 {
            let msg = wire::bytes_of(&f, 2).first().map(|x| String::from_utf8_lossy(x).into_owned()).unwrap_or_default();
            return Err(format!("o kRPC recusou a conexão: {msg}"));
        }
        Ok(c)
    }

    fn send(&mut self, msg: &[u8]) -> Result<(), String> {
        let mut b = Vec::with_capacity(msg.len() + 4);
        wire::put_varint(&mut b, msg.len() as u64);
        b.extend_from_slice(msg);
        self.stream.write_all(&b).map_err(|e| format!("conexão com o kRPC caiu: {e}"))
    }

    fn recv(&mut self) -> Result<Vec<u8>, String> {
        let mut len = 0u64;
        for shift in (0..64).step_by(7) {
            let mut byte = [0u8];
            self.stream.read_exact(&mut byte).map_err(|e| format!("conexão com o kRPC caiu: {e}"))?;
            len |= u64::from(byte[0] & 0x7f) << shift;
            if byte[0] & 0x80 == 0 {
                break;
            }
        }
        if len > 64 << 20 {
            return Err("mensagem grande demais do kRPC".into());
        }
        let mut buf = vec![0u8; len as usize];
        self.stream.read_exact(&mut buf).map_err(|e| format!("conexão com o kRPC caiu: {e}"))?;
        Ok(buf)
    }

    /// Várias chamadas numa requisição só. O erro de fora é da conexão; os de
    /// dentro, de cada chamada.
    pub fn batch(&mut self, calls: &[Call]) -> Result<Vec<Result<Vec<u8>, String>>, String> {
        if calls.is_empty() {
            return Ok(vec![]);
        }
        let mut req = Vec::new();
        for c in calls {
            let mut pc = Vec::new();
            wire::field_bytes(&mut pc, 1, c.service.as_bytes());
            wire::field_bytes(&mut pc, 2, c.procedure.as_bytes());
            for (i, a) in c.args.iter().enumerate() {
                let mut arg = Vec::new();
                if i > 0 {
                    wire::field_varint(&mut arg, 1, i as u64);
                }
                wire::field_bytes(&mut arg, 2, a);
                wire::field_bytes(&mut pc, 3, &arg);
            }
            wire::field_bytes(&mut req, 1, &pc);
        }
        self.send(&req)?;
        let resp = self.recv()?;
        let f = wire::parse(&resp).ok_or("resposta ilegível do kRPC")?;
        if let Some(e) = wire::bytes_of(&f, 1).first() {
            return Err(format!("kRPC: {}", error_text(e)));
        }
        let results: Vec<Result<Vec<u8>, String>> = wire::bytes_of(&f, 2)
            .into_iter()
            .map(|r| {
                let rf = wire::parse(r).unwrap_or_default();
                match wire::bytes_of(&rf, 1).first() {
                    Some(e) => Err(error_text(e)),
                    None => Ok(wire::bytes_of(&rf, 2).first().map_or(vec![], |b| b.to_vec())),
                }
            })
            .collect();
        if results.len() != calls.len() {
            return Err(format!("o kRPC respondeu {} de {} chamadas", results.len(), calls.len()));
        }
        Ok(results)
    }

    pub fn call(&mut self, c: Call) -> Result<Vec<u8>, String> {
        self.batch(std::slice::from_ref(&c))?.pop().unwrap_or_else(|| Err("sem resposta".into()))
    }
}

// ───────────────────────── Ponte ─────────────────────────

/// Estado da ligação, para a interface.
#[derive(Clone, Debug, Default)]
pub struct Status {
    pub connected: bool,
    pub message: String,
    pub vessel: String,
}

/// A thread do kRPC vista pelo servidor: manda comandos, recebe linhas.
pub struct Bridge {
    pub addr: String,
    pub status: Arc<Mutex<Status>>,
    cmd_tx: mpsc::Sender<(u64, String)>,
    lines_rx: mpsc::Receiver<(String, String)>,
    stop: Arc<AtomicBool>,
}

impl Bridge {
    pub fn start(addr: &str) -> Bridge {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (lines_tx, lines_rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status { connected: false, message: format!("conectando a {addr}…"), vessel: String::new() }));
        let stop = Arc::new(AtomicBool::new(false));
        let (a, st, sp) = (addr.to_string(), status.clone(), stop.clone());
        thread::Builder::new()
            .name("krpc".into())
            .spawn(move || run(&a, &st, &sp, &cmd_rx, &lines_tx))
            .expect("thread do kRPC");
        Bridge { addr: addr.to_string(), status, cmd_tx, lines_rx, stop }
    }

    pub fn send(&self, seq: u64, text: &str) {
        let _ = self.cmd_tx.send((seq, text.to_string()));
    }

    /// Linhas novas: (uid da nave, linha).
    pub fn drain(&self) -> Vec<(String, String)> {
        self.lines_rx.try_iter().collect()
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Conecta, roda a sessão e reconecta quando cai.
fn run(addr: &str, status: &Mutex<Status>, stop: &AtomicBool, cmds: &mpsc::Receiver<(u64, String)>, out: &mpsc::Sender<(String, String)>) {
    let set = |connected: bool, message: String| {
        if let Ok(mut s) = status.lock() {
            s.connected = connected;
            s.message = message;
        }
    };
    while !stop.load(Ordering::Relaxed) {
        match Client::connect(addr, "Jaci") {
            Ok(client) => {
                set(true, format!("ligado ao kRPC em {addr}"));
                let mut s = Session::new(client);
                let e = s.run(stop, cmds, out, status);
                set(false, e);
            }
            Err(e) => set(false, e),
        }
        // Tenta de novo daqui a pouco (o jogo pode estar carregando).
        for _ in 0..20 {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
}

/// Situação da nave (enum VesselSituation) no vocabulário do kOS.
fn situation(i: i64) -> &'static str {
    match i {
        0 => "PRELAUNCH",
        1 => "ORBITING",
        2 => "SUB_ORBITAL",
        3 => "ESCAPING",
        4 => "FLYING",
        5 => "LANDED",
        6 => "SPLASHED",
        7 => "DOCKED",
        _ => "UNKNOWN",
    }
}

/// Modos do SAS (enum SASMode) para os modos de atitude do Jaci.
fn sas_mode(m: &str) -> Option<i64> {
    Some(match m {
        "hold" => 0,
        "prograde" => 2,
        "retrograde" => 3,
        "normal" => 4,
        "antinormal" => 5,
        "radial_out" => 6,
        "radial_in" => 7,
        "target" => 8,
        _ => return None,
    })
}

/// Objetos da nave ativa e do corpo em que ela está (ids do kRPC).
#[derive(Clone, Debug, Default)]
struct Objects {
    vessel: u64,
    orbit: u64,
    body: u64,
    control: u64,
    resources: u64,
    autopilot: u64,
    surface_rf: u64,
    orbital_rf: u64,
    body_rf: u64,
    body_nrf: u64,
    flight_surface: u64,
    flight_body: u64,
}

/// Uma leitura da nave.
#[derive(Clone, Debug, Default)]
struct Reading {
    ut: f64,
    met: f64,
    situation: &'static str,
    alt: f64,
    lat: f64,
    lon: f64,
    vorb: f64,
    vsurf: f64,
    vvert: f64,
    ap: f64,
    pe: f64,
    ap_radius: f64,
    inc: f64,
    ecc: f64,
    period: f64,
    lan: f64,
    argp: f64,
    ta: f64,
    sma: f64,
    eta_ap: f64,
    eta_pe: f64,
    mass: f64,
    thrust: f64,
    avail: f64,
    throttle: f64,
    q: f64,
    stage: i64,
    res: [(f64, f64); 5],
    pitch: f64,
    heading: f64,
    flags: [bool; 5],
    node: Option<u64>,
    node_eta: f64,
    node_dv: f64,
    target: String,
    target_dist: f64,
    warp: i64,
    crew: i64,
    mu: f64,
    atm: f64,
}

const RESOURCES: [&str; 5] = ["LiquidFuel", "Oxidizer", "MonoPropellant", "ElectricCharge", "SolidFuel"];

struct Session {
    c: Client,
    o: Objects,
    uid: String,
    body_name: String,
    tick: u64,
    seq: u64,
    mode: &'static str,
    steer: String,
    ascent_apo: f64,
    ascent_hdg: f64,
    burning: bool,
    burn_vec0: [f64; 3],
    last_stage: Instant,
    last: Reading,
}

impl Session {
    fn new(c: Client) -> Self {
        Session {
            c,
            o: Objects::default(),
            uid: String::new(),
            body_name: String::new(),
            tick: 0,
            seq: 0,
            mode: "manual",
            steer: "free".into(),
            ascent_apo: 80_000.0,
            ascent_hdg: 90.0,
            burning: false,
            burn_vec0: [0.0; 3],
            last_stage: Instant::now(),
            last: Reading::default(),
        }
    }

    fn run(&mut self, stop: &AtomicBool, cmds: &mpsc::Receiver<(u64, String)>, out: &mpsc::Sender<(String, String)>, status: &Mutex<Status>) -> String {
        let mut emit = |uid: &str, line: String| {
            let _ = out.send((uid.to_string(), line));
        };
        while !stop.load(Ordering::Relaxed) {
            let t0 = Instant::now();
            if let Err(e) = self.step(cmds, &mut emit, status) {
                return e;
            }
            let dt = t0.elapsed();
            if dt < Duration::from_millis(100) {
                thread::sleep(Duration::from_millis(100) - dt);
            }
        }
        "desligado".into()
    }

    fn step(&mut self, cmds: &mpsc::Receiver<(u64, String)>, emit: &mut impl FnMut(&str, String), status: &Mutex<Status>) -> Result<(), String> {
        self.tick += 1;
        // Nave ativa e órbita (mudam ao trocar de nave ou de esfera de influência).
        let r = self.c.batch(&[sc("get_ActiveVessel", vec![])])?;
        let vessel = r[0].as_ref().map(|b| val::as_u64(b)).unwrap_or(0);
        if vessel == 0 {
            // Fora do voo (VAB, KSC, menu): nada a ler.
            self.o = Objects::default();
            if let Ok(mut s) = status.lock() {
                s.vessel.clear();
            }
            return Ok(());
        }
        if vessel != self.o.vessel {
            self.attach(vessel)?;
            let name = val::as_string(&self.c.call(sc("Vessel_get_Name", vec![val::obj(vessel)]))?);
            if let Ok(mut s) = status.lock() {
                s.vessel = name.clone();
            }
            emit(&self.uid, format!("H {} {}", self.uid, name));
            emit(&self.uid, "E Ponte do Jaci ativa pelo kRPC".into());
            self.body_name.clear();
        }
        let orbit = val::as_u64(&self.c.call(sc("Vessel_get_Orbit", vec![val::obj(vessel)]))?);
        if orbit != self.o.orbit {
            self.o.orbit = orbit;
            let body = val::as_u64(&self.c.call(sc("Orbit_get_Body", vec![val::obj(orbit)]))?);
            if body != self.o.body {
                self.attach_body(body)?;
            }
        }
        if self.body_name.is_empty() {
            let line = self.body_line()?;
            emit(&self.uid, line);
        }
        // Comandos novos.
        for (seq, text) in cmds.try_iter().collect::<Vec<_>>() {
            let (ok, msg) = match self.command(&text, emit) {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            };
            emit(&self.uid, format!("R {seq} {} {msg}", if ok { "True" } else { "False" }));
        }
        let rd = self.read()?;
        self.last = rd;
        self.autopilot(emit)?;
        if self.tick.is_multiple_of(2) {
            let line = self.telemetry_line();
            emit(&self.uid, line);
        }
        Ok(())
    }

    /// Pega os objetos da nave nova.
    fn attach(&mut self, vessel: u64) -> Result<(), String> {
        let v = || vec![val::obj(vessel)];
        let r = self.c.batch(&[
            sc("Vessel_get_Control", v()),
            sc("Vessel_get_Resources", v()),
            sc("Vessel_get_AutoPilot", v()),
            sc("Vessel_get_SurfaceReferenceFrame", v()),
            sc("Vessel_get_OrbitalReferenceFrame", v()),
            sc("Vessel_get_Orbit", v()),
        ])?;
        let id = |i: usize| r[i].as_ref().map(|b| val::as_u64(b)).unwrap_or(0);
        self.o = Objects { vessel, control: id(0), resources: id(1), autopilot: id(2), surface_rf: id(3), orbital_rf: id(4), orbit: id(5), ..Objects::default() };
        let fs = self.c.call(sc("Vessel_Flight", vec![val::obj(vessel), val::obj(self.o.surface_rf)]))?;
        self.o.flight_surface = val::as_u64(&fs);
        let body = val::as_u64(&self.c.call(sc("Orbit_get_Body", vec![val::obj(self.o.orbit)]))?);
        self.attach_body(body)?;
        self.uid = format!("krpc-{vessel}");
        self.mode = "manual";
        self.steer = "free".into();
        self.burning = false;
        Ok(())
    }

    fn attach_body(&mut self, body: u64) -> Result<(), String> {
        let b = || vec![val::obj(body)];
        let r = self.c.batch(&[sc("CelestialBody_get_ReferenceFrame", b()), sc("CelestialBody_get_NonRotatingReferenceFrame", b())])?;
        self.o.body = body;
        self.o.body_rf = r[0].as_ref().map(|x| val::as_u64(x)).unwrap_or(0);
        self.o.body_nrf = r[1].as_ref().map(|x| val::as_u64(x)).unwrap_or(0);
        let f = self.c.call(sc("Vessel_Flight", vec![val::obj(self.o.vessel), val::obj(self.o.body_rf)]))?;
        self.o.flight_body = val::as_u64(&f);
        self.body_name.clear();
        Ok(())
    }

    fn body_line(&mut self) -> Result<String, String> {
        let b = || vec![val::obj(self.o.body)];
        let r = self.c.batch(&[
            sc("CelestialBody_get_Name", b()),
            sc("CelestialBody_get_EquatorialRadius", b()),
            sc("CelestialBody_get_GravitationalParameter", b()),
            sc("CelestialBody_get_HasAtmosphere", b()),
            sc("CelestialBody_get_AtmosphereDepth", b()),
            sc("CelestialBody_get_RotationalPeriod", b()),
        ])?;
        let ok = |i: usize| r[i].clone().unwrap_or_default();
        let name = val::as_string(&ok(0));
        let atm = if val::as_bool(&ok(3)) { val::as_f32(&ok(4)) } else { 0.0 };
        self.body_name = name.clone();
        self.last.mu = val::as_f32(&ok(2));
        self.last.atm = atm;
        Ok(format!("B {} {} {} {} {}", name.replace(' ', "_"), val::as_f32(&ok(1)), val::as_f32(&ok(2)), atm, val::as_f32(&ok(5))))
    }

    /// Lê tudo numa requisição.
    fn read(&mut self) -> Result<Reading, String> {
        let o = self.o.clone();
        let v = || vec![val::obj(o.vessel)];
        let orb = |p: &str| sc(format!("Orbit_get_{p}"), vec![val::obj(o.orbit)]);
        let ctl = |p: &str| sc(format!("Control_get_{p}"), vec![val::obj(o.control)]);
        let fs = |p: &str| sc(format!("Flight_get_{p}"), vec![val::obj(o.flight_surface)]);
        let fb = |p: &str| sc(format!("Flight_get_{p}"), vec![val::obj(o.flight_body)]);
        let mut calls = vec![
            sc("get_UT", vec![]),
            sc("Vessel_get_MET", v()),
            sc("Vessel_get_Situation", v()),
            sc("Vessel_get_Mass", v()),
            sc("Vessel_get_Thrust", v()),
            sc("Vessel_get_AvailableThrust", v()),
            sc("Vessel_get_CrewCount", v()),
            orb("ApoapsisAltitude"),
            orb("PeriapsisAltitude"),
            orb("Apoapsis"),
            orb("Inclination"),
            orb("Eccentricity"),
            orb("Period"),
            orb("LongitudeOfAscendingNode"),
            orb("ArgumentOfPeriapsis"),
            orb("TrueAnomaly"),
            orb("SemiMajorAxis"),
            orb("TimeToApoapsis"),
            orb("TimeToPeriapsis"),
            orb("Speed"),
            ctl("Throttle"),
            ctl("SAS"),
            ctl("RCS"),
            ctl("Gear"),
            ctl("Lights"),
            ctl("Brakes"),
            ctl("CurrentStage"),
            ctl("Nodes"),
            fs("MeanAltitude"),
            fs("Latitude"),
            fs("Longitude"),
            fs("Pitch"),
            fs("Heading"),
            fs("DynamicPressure"),
            fb("Speed"),
            fb("VerticalSpeed"),
            sc("get_RailsWarpFactor", vec![]),
            sc("get_TargetVessel", vec![]),
        ];
        let n0 = calls.len();
        for name in RESOURCES {
            calls.push(sc("Resources_Amount", vec![val::obj(o.resources), val::string(name)]));
            calls.push(sc("Resources_Max", vec![val::obj(o.resources), val::string(name)]));
        }
        let r = self.c.batch(&calls)?;
        let b = |i: usize| r[i].clone().unwrap_or_default();
        let d = |i: usize| r[i].as_ref().map_or(f64::NAN, |x| val::as_f64(x));
        let f = |i: usize| r[i].as_ref().map_or(f64::NAN, |x| val::as_f32(x));
        let ecc = d(11);
        let closed = ecc < 1.0;
        let nodes = val::items(&b(27));
        let mut rd = Reading {
            ut: d(0),
            met: d(1),
            situation: situation(val::as_i64(&b(2))),
            mass: f(3) / 1e3,
            thrust: f(4) / 1e3,
            avail: f(5) / 1e3,
            crew: val::as_i64(&b(6)),
            ap: d(7),
            pe: d(8),
            ap_radius: d(9),
            inc: d(10).to_degrees(),
            ecc,
            period: if closed { d(12) } else { -1.0 },
            lan: d(13).to_degrees(),
            argp: d(14).to_degrees(),
            ta: d(15).to_degrees(),
            sma: d(16),
            eta_ap: if closed { d(17) } else { -1.0 },
            eta_pe: d(18),
            vorb: d(19),
            throttle: f(20),
            flags: [21, 22, 23, 24, 25].map(|i| val::as_bool(&b(i))),
            stage: val::as_i64(&b(26)),
            node: nodes.first().map(|x| val::as_u64(x)),
            alt: d(28),
            lat: d(29),
            lon: d(30),
            pitch: f(31),
            heading: f(32),
            q: f(33) / 1e3,
            vsurf: d(34),
            vvert: d(35),
            warp: val::as_i64(&b(36)),
            target: "-".into(),
            target_dist: -1.0,
            node_eta: -1.0,
            node_dv: 0.0,
            mu: self.last.mu,
            atm: self.last.atm,
            ..Reading::default()
        };
        for (k, res) in rd.res.iter_mut().enumerate() {
            *res = (f(n0 + 2 * k), f(n0 + 2 * k + 1));
        }
        // Hipérbole saindo: não há periastro à frente.
        if !closed && rd.ta > 0.0 {
            rd.eta_pe = -1.0;
        }
        let target = val::as_u64(&b(37));
        let mut extra = Vec::new();
        if let Some(n) = rd.node {
            extra.push(sc("Node_get_TimeTo", vec![val::obj(n)]));
            extra.push(sc("Node_get_RemainingDeltaV", vec![val::obj(n)]));
        }
        if target != 0 {
            extra.push(sc("Vessel_get_Name", vec![val::obj(target)]));
            extra.push(sc("Vessel_Position", vec![val::obj(target), val::obj(o.surface_rf)]));
        }
        let r2 = self.c.batch(&extra)?;
        let mut i = 0;
        if rd.node.is_some() {
            rd.node_eta = r2[0].as_ref().map_or(-1.0, |x| val::as_f64(x));
            rd.node_dv = r2[1].as_ref().map_or(0.0, |x| val::as_f64(x));
            i = 2;
        }
        if target != 0 {
            rd.target = r2[i].as_ref().map(|x| val::as_string(x)).unwrap_or_else(|_| "-".into());
            let p = r2[i + 1].as_ref().map(|x| val::vec3(x)).unwrap_or([f64::NAN; 3]);
            rd.target_dist = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        }
        Ok(rd)
    }

    /// A linha T, na ordem dos campos do script do kOS.
    fn telemetry_line(&mut self) -> String {
        let r = &self.last;
        let tf = |b: bool| if b { "True" } else { "False" };
        let tgt = if r.target.is_empty() { "-".to_string() } else { r.target.replace(' ', "_") };
        let fields: Vec<String> = vec![
            self.seq.to_string(),
            format!("{:.2}", r.ut),
            format!("{:.2}", r.met),
            r.situation.to_string(),
            format!("{:.1}", r.alt),
            format!("{:.5}", r.lat),
            format!("{:.5}", r.lon),
            format!("{:.2}", r.vorb),
            format!("{:.2}", r.vsurf),
            format!("{:.2}", r.vvert),
            format!("{:.1}", r.ap),
            format!("{:.1}", r.pe),
            format!("{:.4}", r.inc),
            format!("{:.6}", r.ecc),
            format!("{:.1}", r.period),
            format!("{:.4}", r.lan),
            format!("{:.4}", r.argp),
            format!("{:.4}", r.ta),
            format!("{:.1}", r.sma),
            format!("{:.4}", r.mass),
            format!("{:.2}", r.thrust),
            format!("{:.2}", r.avail),
            format!("{:.3}", r.throttle),
            format!("{:.3}", r.q),
            r.stage.to_string(),
            format!("{:.2}", r.res[0].0),
            format!("{:.2}", r.res[0].1),
            format!("{:.2}", r.res[1].0),
            format!("{:.2}", r.res[1].1),
            format!("{:.2}", r.res[2].0),
            format!("{:.2}", r.res[2].1),
            format!("{:.2}", r.res[3].0),
            format!("{:.2}", r.res[3].1),
            format!("{:.2}", r.res[4].0),
            format!("{:.2}", r.res[4].1),
            format!("{:.2}", r.pitch),
            format!("{:.2}", (r.heading + 360.0) % 360.0),
            tf(r.flags[0]).into(),
            tf(r.flags[1]).into(),
            tf(r.flags[2]).into(),
            tf(r.flags[3]).into(),
            tf(r.flags[4]).into(),
            format!("{:.1}", r.eta_ap),
            format!("{:.1}", r.eta_pe),
            format!("{:.1}", r.node_eta),
            format!("{:.2}", r.node_dv),
            tgt,
            format!("{:.1}", r.target_dist),
            r.warp.to_string(),
            r.crew.to_string(),
            self.mode.to_string(),
            self.steer.clone(),
        ];
        self.seq += 1;
        format!("T {}", fields.join(" "))
    }

    // ── Controle ──

    fn release(&mut self) -> Result<(), String> {
        self.c.call(sc("AutoPilot_Disengage", vec![val::obj(self.o.autopilot)]))?;
        self.steer = "free".into();
        self.burning = false;
        Ok(())
    }

    fn set_throttle(&mut self, x: f64) -> Result<(), String> {
        self.c.call(sc("Control_set_Throttle", vec![val::obj(self.o.control), val::f32(x.clamp(0.0, 1.0))])).map(|_| ())
    }

    /// Aponta o piloto automático para uma direção num referencial.
    fn point(&mut self, frame: u64, dir: [f64; 3]) -> Result<(), String> {
        let ap = val::obj(self.o.autopilot);
        self.c.batch(&[
            sc("AutoPilot_set_ReferenceFrame", vec![ap.clone(), val::obj(frame)]),
            sc("AutoPilot_set_TargetDirection", vec![ap.clone(), val::tuple(&dir.map(val::f64))]),
            sc("AutoPilot_Engage", vec![ap]),
        ])?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map(|_| ())
    }

    fn pitch_heading(&mut self, pitch: f64, heading: f64) -> Result<(), String> {
        let ap = val::obj(self.o.autopilot);
        self.c.batch(&[
            sc("AutoPilot_set_ReferenceFrame", vec![ap.clone(), val::obj(self.o.surface_rf)]),
            sc("AutoPilot_TargetPitchAndHeading", vec![ap.clone(), val::f32(pitch), val::f32(heading)]),
            sc("AutoPilot_Engage", vec![ap]),
        ])?
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map(|_| ())
    }

    fn stage(&mut self) -> Result<(), String> {
        self.last_stage = Instant::now();
        self.c.call(sc("Control_ActivateNextStage", vec![val::obj(self.o.control)])).map(|_| ())
    }

    /// Separa o estágio quando acaba o empuxo (com um segundo de folga entre um e outro).
    fn auto_stage(&mut self, emit: &mut impl FnMut(&str, String)) -> Result<(), String> {
        if self.last.avail <= 0.0 && self.last.stage > 0 && self.last_stage.elapsed() > Duration::from_secs(1) {
            self.stage()?;
            emit(&self.uid, format!("E Estágio separado ({} restantes)", self.last.stage - 1));
        }
        Ok(())
    }

    fn make_circ(&mut self, at_pe: bool, emit: &mut impl FnMut(&str, String)) -> Result<(), String> {
        let r = &self.last;
        let (dt, radius) = if at_pe { (r.eta_pe, r.sma * (1.0 - r.ecc)) } else { (r.eta_ap, r.ap_radius) };
        // NaN também cai aqui.
        let valid = dt >= 0.0 && radius > 0.0 && r.mu > 0.0;
        if !valid {
            return Err("órbita sem esse ponto".into());
        }
        let v_now = (r.mu * (2.0 / radius - 1.0 / r.sma)).sqrt();
        let dv = (r.mu / radius).sqrt() - v_now;
        let ut = r.ut + dt;
        self.c.call(sc("Control_AddNode", vec![val::obj(self.o.control), val::f64(ut), val::f32(dv), val::f32(0.0), val::f32(0.0)]))?;
        emit(&self.uid, format!("E Manobra de circularização criada: {dv:.1} m/s"));
        Ok(())
    }

    fn command(&mut self, text: &str, emit: &mut impl FnMut(&str, String)) -> Result<String, String> {
        let p: Vec<&str> = text.split_whitespace().collect();
        let c = p.first().copied().unwrap_or("").to_lowercase();
        let num = |i: usize, d: f64| p.get(i).and_then(|s| s.replace(',', ".").parse().ok()).unwrap_or(d);
        let word = |i: usize, d: &str| p.get(i).map_or(d.to_string(), |s| s.to_lowercase());
        let ctl = val::obj(self.o.control);
        let set_bool = |s: &mut Self, prop: &str, b: bool| s.c.call(sc(format!("Control_set_{prop}"), vec![ctl.clone(), val::boolean(b)])).map(|_| ());
        match c.as_str() {
            "stage" => self.stage()?,
            "throttle" => self.set_throttle(num(1, 0.0))?,
            "sas" => {
                let on = word(1, "on") == "on";
                if on {
                    self.release()?;
                }
                set_bool(self, "SAS", on)?;
            }
            "rcs" => set_bool(self, "RCS", word(1, "on") == "on")?,
            "gear" | "lights" | "brakes" => {
                let prop = match c.as_str() {
                    "gear" => "Gear",
                    "lights" => "Lights",
                    _ => "Brakes",
                };
                let i = ["gear", "lights", "brakes"].iter().position(|x| *x == c).unwrap_or(0) + 2;
                set_bool(self, prop, !self.last.flags[i])?;
            }
            "abort" => {
                self.mode = "manual";
                self.release()?;
                set_bool(self, "Abort", true)?;
            }
            "ag" => {
                let n = num(1, 0.0);
                if !(1.0..=10.0).contains(&n) {
                    return Err("grupo inválido".into());
                }
                // No kRPC o grupo 10 é o 0.
                self.c.call(sc("Control_ToggleActionGroup", vec![ctl, val::uint(n as u64 % 10)]))?;
            }
            "chutes" => set_bool(self, "Parachutes", true)?,
            "panels" => set_bool(self, "SolarPanels", word(1, "on") == "on")?,
            "steer" => {
                let m = word(1, "free");
                self.mode = "manual";
                if m == "free" {
                    self.release()?;
                    set_bool(self, "SAS", false)?;
                } else if let Some(k) = sas_mode(&m) {
                    // Os modos do navball são do SAS.
                    self.release()?;
                    set_bool(self, "SAS", true)?;
                    self.c.call(sc("Control_set_SASMode", vec![ctl, val::sint(k)])).map_err(|e| format!("o SAS desta nave não tem esse modo: {e}"))?;
                    self.steer = m;
                } else if m == "heading" || m == "up" {
                    set_bool(self, "SAS", false)?;
                    let (h, pt) = if m == "up" { (num(2, 90.0), 90.0) } else { (num(2, 90.0), num(3, 0.0)) };
                    self.pitch_heading(pt, h)?;
                    self.steer = m;
                } else {
                    return Err(format!("atitude {m} não disponível pelo kRPC"));
                }
            }
            "warp" => {
                self.c.call(sc("set_RailsWarpFactor", vec![val::sint(num(1, 0.0).clamp(0.0, 7.0) as i64)]))?;
            }
            "node" => {
                // node t radial normal prograde (como o NODE do kOS); o kRPC quer prógrado, normal, radial.
                let ut = self.last.ut + num(1, 60.0);
                self.c.call(sc("Control_AddNode", vec![ctl, val::f64(ut), val::f32(num(4, 0.0)), val::f32(num(3, 0.0)), val::f32(num(2, 0.0))]))?;
            }
            "delnodes" => {
                self.c.call(sc("Control_RemoveNodes", vec![ctl]))?;
            }
            "exec" => {
                if self.last.node.is_none() {
                    return Err("não há manobra".into());
                }
                self.mode = "node";
                self.burning = false;
            }
            "circ" => self.make_circ(word(1, "ap") == "pe", emit)?,
            "launch" => {
                self.ascent_apo = num(1, 80.0) * 1e3;
                self.ascent_hdg = num(2, 90.0);
                self.mode = "ascent";
                set_bool(self, "SAS", false)?;
                self.pitch_heading(90.0, self.ascent_hdg)?;
                self.set_throttle(1.0)?;
                if matches!(self.last.situation, "PRELAUNCH" | "LANDED") {
                    self.stage()?;
                }
                emit(&self.uid, format!("E Subida automática: apoastro {:.1} km, rumo {}", self.ascent_apo / 1e3, self.ascent_hdg));
            }
            "stop" => {
                self.mode = "manual";
                self.set_throttle(0.0)?;
                self.release()?;
            }
            "target" => {
                let name = p.get(1).map_or(String::new(), |s| s.replace('_', " "));
                self.set_target(&name)?;
            }
            _ => return Err("comando desconhecido".into()),
        }
        Ok(c)
    }

    /// Alvo: um corpo celeste ou outra nave, pelo nome.
    fn set_target(&mut self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("alvo não encontrado".into());
        }
        let bodies = self.c.call(sc("get_Bodies", vec![]))?;
        if let Some((_, b)) = val::dict(&bodies).into_iter().find(|(k, _)| val::as_string(k) == name) {
            self.c.call(sc("set_TargetBody", vec![b]))?;
            return Ok(());
        }
        let vessels = val::items(&self.c.call(sc("get_Vessels", vec![]))?);
        let names = self.c.batch(&vessels.iter().map(|v| sc("Vessel_get_Name", vec![v.clone()])).collect::<Vec<_>>())?;
        let found = vessels.iter().zip(names).find(|(_, n)| n.as_ref().is_ok_and(|n| val::as_string(n) == name));
        match found {
            Some((v, _)) => self.c.call(sc("set_TargetVessel", vec![v.clone()])).map(|_| ()),
            None => Err("alvo não encontrado".into()),
        }
    }

    /// Piloto automático: subida, saída da atmosfera e execução de manobra.
    fn autopilot(&mut self, emit: &mut impl FnMut(&str, String)) -> Result<(), String> {
        match self.mode {
            "ascent" => {
                self.auto_stage(emit)?;
                let r = self.last.clone();
                let turn_end = if r.atm > 0.0 { r.atm * 0.65 } else { 10_000.0 };
                let pitch = if r.vsurf > 60.0 { (90.0 - 90.0 * (r.alt / turn_end).clamp(0.0, 1.0).sqrt()).max(0.0) } else { 90.0 };
                self.pitch_heading(pitch, self.ascent_hdg)?;
                if r.ap >= self.ascent_apo {
                    self.set_throttle(0.0)?;
                    self.mode = "coast";
                    emit(&self.uid, format!("E Corte do motor: apoastro {:.1} km", r.ap / 1e3));
                }
            }
            "coast" => {
                let r = self.last.clone();
                // No referencial orbital da nave, o eixo y é o prógrado.
                self.point(self.o.orbital_rf, [0.0, 1.0, 0.0])?;
                // O arrasto rouba apoastro: repõe até sair da atmosfera.
                self.set_throttle(if r.ap < self.ascent_apo * 0.995 && r.alt < r.atm { 0.3 } else { 0.0 })?;
                if r.alt > r.atm {
                    self.set_throttle(0.0)?;
                    self.make_circ(false, emit)?;
                    self.mode = "node";
                    self.burning = false;
                }
            }
            "node" => {
                let Some(node) = self.last.node else {
                    self.mode = "manual";
                    return self.release();
                };
                let r = self.last.clone();
                let n = val::obj(node);
                let rf = val::as_u64(&self.c.call(sc("Node_get_ReferenceFrame", vec![n.clone()]))?);
                // No referencial da manobra, o eixo y é a direção da queima.
                self.point(rf, [0.0, 1.0, 0.0])?;
                let res = self.c.batch(&[sc("AutoPilot_get_Error", vec![val::obj(self.o.autopilot)]), sc("Node_RemainingBurnVector", vec![n.clone(), val::obj(self.o.body_nrf)])])?;
                let err = res[0].as_ref().map_or(f64::NAN, |x| val::as_f32(x));
                let bv = res[1].as_ref().map(|x| val::vec3(x)).unwrap_or([f64::NAN; 3]);
                let acc = if r.mass > 0.0 { r.avail / r.mass } else { 0.0 };
                if acc <= 0.0 {
                    if !self.burning {
                        emit(&self.uid, "E Sem empuxo disponível para a manobra".into());
                        self.mode = "manual";
                        return self.release();
                    }
                    // O estágio acabou no meio da queima: separa e segue.
                    return self.auto_stage(emit);
                }
                let bt = r.node_dv / acc;
                if !self.burning {
                    self.set_throttle(0.0)?;
                    if r.node_eta <= bt / 2.0 && err < 3.0 {
                        self.burning = true;
                        self.burn_vec0 = bv;
                        emit(&self.uid, format!("E Ignição: manobra de {:.1} m/s", r.node_dv));
                    }
                } else {
                    self.auto_stage(emit)?;
                    self.set_throttle((r.node_dv / acc).clamp(0.02, 1.0))?;
                    let dot = self.burn_vec0[0] * bv[0] + self.burn_vec0[1] * bv[1] + self.burn_vec0[2] * bv[2];
                    if dot < 0.0 || r.node_dv < 0.1 {
                        self.set_throttle(0.0)?;
                        self.c.call(sc("Node_Remove", vec![n]))?;
                        self.mode = "manual";
                        self.release()?;
                        emit(&self.uid, "E Corte: manobra concluída".into());
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

// ───────────────────────── Testes ─────────────────────────

/// Servidor kRPC de mentira, para os testes: aperto de mão, requisições em
/// lote e uma nave com estado simples.
#[cfg(test)]
pub mod fake {
    use super::{val, wire};
    use std::{
        collections::HashMap,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::{Arc, Mutex},
        thread,
    };

    #[derive(Debug, Default)]
    pub struct Ship {
        pub alt: f64,
        pub throttle: f64,
        pub stage: i64,
        pub sas: bool,
        pub sas_mode: i64,
        pub gear: bool,
        pub nodes: Vec<(f64, f64, f64, f64)>,
        pub calls: Vec<String>,
        pub warp: i64,
        /// Relógio do jogo; com `clock`, anda 0,1 s a cada leitura e a queima
        /// gasta o Δv da manobra (24 m/s² na potência máxima).
        pub ut: f64,
        pub clock: bool,
    }

    fn read_msg(s: &mut TcpStream) -> Option<Vec<u8>> {
        let mut len = 0u64;
        for shift in (0..64).step_by(7) {
            let mut b = [0u8];
            s.read_exact(&mut b).ok()?;
            len |= u64::from(b[0] & 0x7f) << shift;
            if b[0] & 0x80 == 0 {
                break;
            }
        }
        let mut buf = vec![0u8; len as usize];
        s.read_exact(&mut buf).ok()?;
        Some(buf)
    }

    fn write_msg(s: &mut TcpStream, m: &[u8]) {
        let mut b = Vec::new();
        wire::put_varint(&mut b, m.len() as u64);
        b.extend_from_slice(m);
        let _ = s.write_all(&b);
    }

    /// Ids dos objetos de mentira.
    const VESSEL: u64 = 7;
    const ORBIT: u64 = 8;
    const BODY: u64 = 9;
    const CONTROL: u64 = 10;
    const NODE: u64 = 11;

    fn answer(ship: &mut Ship, proc_: &str, args: &[Vec<u8>]) -> Result<Vec<u8>, String> {
        ship.calls.push(proc_.to_string());
        let d = val::f64;
        let f = val::f32;
        Ok(match proc_ {
            "get_UT" => {
                if ship.clock {
                    ship.ut += 0.1;
                    let thr = ship.throttle;
                    if let Some(n) = ship.nodes.first_mut() {
                        n.1 -= thr * 24.0 * 0.1;
                    }
                }
                d(ship.ut)
            }
            "get_ActiveVessel" => val::obj(VESSEL),
            "Vessel_get_Name" => val::string("Kerbal X"),
            "Vessel_get_Orbit" => val::obj(ORBIT),
            "Orbit_get_Body" => val::obj(BODY),
            "Vessel_get_Control" => val::obj(CONTROL),
            "Vessel_get_Resources" | "Vessel_get_AutoPilot" | "Vessel_get_SurfaceReferenceFrame" | "Vessel_get_OrbitalReferenceFrame" | "Vessel_Flight" => val::obj(20 + ship.calls.len() as u64),
            "CelestialBody_get_ReferenceFrame" | "CelestialBody_get_NonRotatingReferenceFrame" | "Node_get_ReferenceFrame" => val::obj(50),
            "CelestialBody_get_Name" => val::string("Kerbin"),
            "CelestialBody_get_EquatorialRadius" => f(600_000.0),
            "CelestialBody_get_GravitationalParameter" => f(3.5316e12),
            "CelestialBody_get_HasAtmosphere" => val::boolean(true),
            "CelestialBody_get_AtmosphereDepth" => f(70_000.0),
            "CelestialBody_get_RotationalPeriod" => f(21_549.425),
            "Vessel_get_MET" => d(12.3),
            "Vessel_get_Situation" => val::sint(4),
            "Vessel_get_Mass" => f(12_500.0),
            "Vessel_get_Thrust" => f(250_000.0),
            "Vessel_get_AvailableThrust" => f(300_000.0),
            "Vessel_get_CrewCount" => val::sint(1),
            "Orbit_get_ApoapsisAltitude" => d(60_000.0),
            "Orbit_get_PeriapsisAltitude" => d(-500_000.0),
            "Orbit_get_Apoapsis" => d(660_000.0),
            "Orbit_get_Inclination" => d(0.1f64.to_radians()),
            "Orbit_get_Eccentricity" => d(0.5),
            "Orbit_get_Period" => d(1200.0),
            "Orbit_get_LongitudeOfAscendingNode" | "Orbit_get_ArgumentOfPeriapsis" => d(0.2),
            "Orbit_get_TrueAnomaly" => d(-0.5),
            "Orbit_get_SemiMajorAxis" => d(440_000.0),
            "Orbit_get_TimeToApoapsis" => d(45.0),
            "Orbit_get_TimeToPeriapsis" => d(1500.0),
            "Orbit_get_Speed" => d(2300.0),
            "Control_get_Throttle" => f(ship.throttle),
            "Control_get_SAS" => val::boolean(ship.sas),
            "Control_get_RCS" => val::boolean(true),
            "Control_get_Gear" => val::boolean(ship.gear),
            "Control_get_Lights" | "Control_get_Brakes" => val::boolean(false),
            "Control_get_CurrentStage" => val::sint(ship.stage),
            "Control_get_Nodes" => val::tuple(&ship.nodes.iter().map(|_| val::obj(NODE)).collect::<Vec<_>>()),
            "Flight_get_MeanAltitude" => d(ship.alt),
            "Flight_get_Latitude" => d(-0.09),
            "Flight_get_Longitude" => d(-74.55),
            "Flight_get_Pitch" => f(60.0),
            "Flight_get_Heading" => f(90.0),
            "Flight_get_DynamicPressure" => f(18_200.0),
            "Flight_get_Speed" => d(1800.0),
            "Flight_get_VerticalSpeed" => d(350.0),
            "get_RailsWarpFactor" => val::sint(ship.warp),
            "get_TargetVessel" => val::obj(0),
            "Resources_Amount" => f(if val::as_string(&args[1]) == "LiquidFuel" { 100.0 } else { 0.0 }),
            "Resources_Max" => f(if val::as_string(&args[1]) == "LiquidFuel" { 200.0 } else { 0.0 }),
            "Node_get_TimeTo" => d(ship.nodes.first().map_or(0.0, |n| n.0 - ship.ut)),
            "Node_get_RemainingDeltaV" => d(ship.nodes.first().map_or(0.0, |n| n.1.abs())),
            "Node_RemainingBurnVector" => val::tuple(&[d(ship.nodes.first().map_or(0.0, |n| n.1)), d(0.0), d(0.0)]),
            "AutoPilot_get_Error" => f(1.0),
            "Node_Remove" => {
                if !ship.nodes.is_empty() {
                    ship.nodes.remove(0);
                }
                vec![]
            }
            "Control_RemoveNodes" => {
                ship.nodes.clear();
                vec![]
            }
            "Control_set_Throttle" => {
                ship.throttle = val::as_f32(&args[1]);
                vec![]
            }
            "Control_set_SAS" => {
                ship.sas = val::as_bool(&args[1]);
                vec![]
            }
            "Control_set_SASMode" => {
                ship.sas_mode = val::as_i64(&args[1]);
                vec![]
            }
            "Control_set_Gear" => {
                ship.gear = val::as_bool(&args[1]);
                vec![]
            }
            "Control_ActivateNextStage" => {
                ship.stage -= 1;
                val::tuple(&[])
            }
            "Control_AddNode" => {
                ship.nodes.push((val::as_f64(&args[1]), val::as_f32(&args[2]), val::as_f32(&args[3]), val::as_f32(&args[4])));
                val::obj(NODE)
            }
            "set_RailsWarpFactor" => {
                ship.warp = val::as_i64(&args[0]);
                vec![]
            }
            "AutoPilot_Disengage" | "AutoPilot_Engage" | "AutoPilot_set_ReferenceFrame" | "AutoPilot_set_TargetDirection" | "AutoPilot_TargetPitchAndHeading" => vec![],
            other => return Err(format!("procedimento desconhecido {other}")),
        })
    }

    fn serve(mut s: TcpStream, ship: Arc<Mutex<Ship>>) {
        // Aperto de mão.
        let Some(req) = read_msg(&mut s) else { return };
        let f = wire::parse(&req).unwrap();
        assert_eq!(wire::varint_of(&f, 1).unwrap_or(0), 0, "pedido de conexão RPC");
        let mut resp = Vec::new();
        wire::field_bytes(&mut resp, 3, &[1; 16]);
        write_msg(&mut s, &resp);
        while let Some(req) = read_msg(&mut s) {
            let f = wire::parse(&req).unwrap();
            let mut out = Vec::new();
            for pc in wire::bytes_of(&f, 1) {
                let pf = wire::parse(pc).unwrap();
                let name = String::from_utf8_lossy(wire::bytes_of(&pf, 2)[0]).into_owned();
                let mut args: HashMap<u64, Vec<u8>> = HashMap::new();
                for a in wire::bytes_of(&pf, 3) {
                    let af = wire::parse(a).unwrap();
                    args.insert(wire::varint_of(&af, 1).unwrap_or(0), wire::bytes_of(&af, 2).first().map_or(vec![], |b| b.to_vec()));
                }
                let args: Vec<Vec<u8>> = (0..args.len() as u64).map(|i| args.get(&i).cloned().unwrap_or_default()).collect();
                let mut pr = Vec::new();
                match answer(&mut ship.lock().unwrap(), &name, &args) {
                    Ok(v) => wire::field_bytes(&mut pr, 2, &v),
                    Err(e) => {
                        let mut err = Vec::new();
                        wire::field_bytes(&mut err, 3, e.as_bytes());
                        wire::field_bytes(&mut pr, 1, &err);
                    }
                }
                wire::field_bytes(&mut out, 2, &pr);
            }
            write_msg(&mut s, &out);
        }
    }

    /// Sobe o servidor numa porta livre; devolve o endereço e a nave.
    pub fn start() -> (String, Arc<Mutex<Ship>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap().to_string();
        let ship = Arc::new(Mutex::new(Ship { alt: 15_000.0, stage: 3, ut: 1000.0, ..Ship::default() }));
        let sh = ship.clone();
        thread::spawn(move || {
            for s in l.incoming().flatten() {
                let sh = sh.clone();
                thread::spawn(move || serve(s, sh));
            }
        });
        (addr, ship)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_e_zigzag() {
        for v in [0u64, 1, 127, 128, 300, u64::from(u32::MAX), u64::MAX] {
            let mut b = Vec::new();
            wire::put_varint(&mut b, v);
            assert_eq!(wire::get_varint(&b, &mut 0), Some(v));
        }
        for i in [0i64, -1, 1, -64, 63, i64::from(i32::MIN), i64::MAX] {
            assert_eq!(wire::unzigzag(wire::zigzag(i)), i);
        }
        assert_eq!(wire::zigzag(-1), 1);
        assert_eq!(wire::zigzag(1), 2);
        // Tupla de três doubles, ida e volta.
        let t = val::tuple(&[val::f64(1.5), val::f64(-2.0), val::f64(3.25)]);
        assert_eq!(val::vec3(&t), [1.5, -2.0, 3.25]);
        assert_eq!(val::as_string(&val::string("Kerbin")), "Kerbin");
    }

    /// A ponte inteira: a nave do servidor de mentira aparece no Jaci com a
    /// telemetria, e os comandos viram chamadas de RPC.
    #[test]
    fn nave_pelo_krpc_aparece_e_obedece() {
        use crate::ksp::KspLink;
        use jaci_sim::world::{CmdStatus, World};
        let (addr, ship) = fake::start();
        let mut link = KspLink::without_kos();
        link.connect_krpc(&addr);
        let mut w = World::new(0.0, 1);
        let wait = |link: &mut KspLink, w: &mut World, ok: &dyn Fn(&KspLink, &World) -> bool| {
            for _ in 0..100 {
                link.poll(w);
                if ok(link, w) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            false
        };
        assert!(wait(&mut link, &mut w, &|l, _| l.vessels.first().is_some_and(|v| v.packets > 0)), "a nave não apareceu: {:?}", link.krpc.as_ref().map(|b| b.status()));
        let v = &link.vessels[0];
        assert_eq!(v.via, "kRPC");
        assert_eq!(v.name, "Kerbal X");
        assert_eq!(v.body.name, "Kerbin");
        assert_eq!(v.situation, "FLYING");
        assert!((v.tm_value("nav.alt") - 15.0).abs() < 1e-6);
        assert!((v.tm_value("nav.mass") - 12.5).abs() < 1e-3);
        assert!((v.tm_value("res.lf") - 50.0).abs() < 1e-6);
        assert!((v.tm_value("orb.inc") - 0.1).abs() < 1e-3);
        assert!(link.krpc.as_ref().unwrap().status().connected);
        let id = v.id;
        let send = |link: &mut KspLink, w: &mut World, text: &str| {
            let rec = link.send(w, id, text, text).unwrap();
            assert!(wait(link, w, &|_, w| w.commands.iter().any(|c| c.id == rec && c.status != CmdStatus::Queued)), "{text} sem resposta");
            let c = w.commands.iter().find(|c| c.id == rec).unwrap();
            (c.status, c.message.clone())
        };
        assert_eq!(send(&mut link, &mut w, "throttle 0.5").0, CmdStatus::Executed);
        assert!((ship.lock().unwrap().throttle - 0.5).abs() < 1e-6);
        assert_eq!(send(&mut link, &mut w, "stage").0, CmdStatus::Executed);
        assert_eq!(ship.lock().unwrap().stage, 2);
        assert_eq!(send(&mut link, &mut w, "steer prograde 90 0").0, CmdStatus::Executed);
        assert!(ship.lock().unwrap().sas);
        assert_eq!(ship.lock().unwrap().sas_mode, 2);
        assert_eq!(send(&mut link, &mut w, "gear").0, CmdStatus::Executed);
        assert!(ship.lock().unwrap().gear);
        assert_eq!(send(&mut link, &mut w, "warp 3").0, CmdStatus::Executed);
        assert_eq!(ship.lock().unwrap().warp, 3);
        // node t radial normal prograde → AddNode(ut, prógrado, normal, radial).
        assert_eq!(send(&mut link, &mut w, "node 60 1 2 100").0, CmdStatus::Executed);
        assert_eq!(ship.lock().unwrap().nodes[0], (1060.0, 100.0, 2.0, 1.0));
        // Circularização no apoastro: Δv positivo pela vis-viva.
        assert_eq!(send(&mut link, &mut w, "circ ap").0, CmdStatus::Executed);
        let (ut, dv, _, _) = ship.lock().unwrap().nodes[1];
        assert!((ut - 1045.0).abs() < 1e-6 && dv > 100.0, "{ut} {dv}");
        let (st, msg) = send(&mut link, &mut w, "voar");
        assert_eq!(st, CmdStatus::Failed);
        assert!(msg.contains("desconhecido"));
        // A telemetria mostra o que mudou.
        assert!(wait(&mut link, &mut w, &|l, _| (l.vessels[0].tm_value("prop.throttle") - 50.0).abs() < 1e-3 && l.vessels[0].flags[0]));
        // O piloto automático executa a manobra: aponta, espera, queima e corta.
        assert_eq!(send(&mut link, &mut w, "delnodes").0, CmdStatus::Executed);
        assert!(ship.lock().unwrap().nodes.is_empty());
        ship.lock().unwrap().clock = true;
        assert_eq!(send(&mut link, &mut w, "node 3 0 0 50").0, CmdStatus::Executed);
        assert_eq!(send(&mut link, &mut w, "exec").0, CmdStatus::Executed);
        let mut done = false;
        for _ in 0..400 {
            link.poll(&mut w);
            if w.events.iter().any(|e| e.text.contains("Corte: manobra concluída")) {
                done = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let sh = ship.lock().unwrap();
        assert!(done, "a manobra não terminou: nós {:?}, potência {}", sh.nodes, sh.throttle);
        assert!(w.events.iter().any(|e| e.text.contains("Ignição: manobra de")));
        assert!(sh.nodes.is_empty() && sh.throttle == 0.0);
    }

    #[test]
    fn chamadas_em_lote_e_erros() {
        let (addr, _) = fake::start();
        let mut c = Client::connect(&addr, "teste").unwrap();
        let r = c.batch(&[sc("get_UT", vec![]), sc("NaoExiste", vec![]), sc("CelestialBody_get_Name", vec![val::obj(9)])]).unwrap();
        assert_eq!(val::as_f64(r[0].as_ref().unwrap()), 1000.0);
        assert!(r[1].as_ref().unwrap_err().contains("desconhecido"));
        assert_eq!(val::as_string(r[2].as_ref().unwrap()), "Kerbin");
    }
}
