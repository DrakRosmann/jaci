//! jaci: o servidor do centro de controle. Roda a simulação, fala WebSocket
//! com a interface, serve os arquivos dela e, se pedido, troca pacotes CCSDS
//! por UDP com computadores de bordo de verdade.

mod ksp;
mod recording;
mod room;
mod snapshot;

use anyhow::Context;
use axum::{
    Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use jaci_sim::{
    bodies::Body,
    ccsds::{self, Packet},
    commands::Command,
    planner, scenarios,
    time::{format_utc, parse_utc},
    world::World,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{net::UdpSocket, sync::broadcast};
use tower_http::services::{ServeDir, ServeFile};

#[derive(Parser, Debug, Clone)]
#[command(version, about = "Jaci: centro de controle de missões espaciais")]
struct Args {
    /// Endereço do servidor web. Só esta máquina por padrão; 0.0.0.0:8080 para a rede.
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,
    /// Senha exigida das interfaces (ws://host/ws?token=...). Escutando na rede
    /// sem ela, o servidor gera uma e mostra no log.
    #[arg(long, env = "JACI_TOKEN")]
    token: Option<String>,
    /// Cenário inicial (veja --list); padrão: sondagem, ou ksp com --ksp.
    #[arg(long)]
    scenario: Option<String>,
    /// Liga ao Kerbal Space Program pelo kOS. Sem a pasta, procura a do Steam.
    #[arg(long, num_args = 0..=1, value_name = "PASTA_DO_KSP", env = "JACI_KSP")]
    ksp: Option<Option<PathBuf>>,
    /// Lista os cenários e sai.
    #[arg(long)]
    list: bool,
    /// Pasta da interface compilada (padrão: web/dist, ou a instalada em
    /// <prefixo>/share/jaci/web).
    #[arg(long)]
    web_dir: Option<PathBuf>,
    /// Abre o navegador na interface (se o Jaci já estiver rodando, só abre).
    #[arg(long)]
    open: bool,
    /// Pasta dos estados salvos.
    /// (padrão: ./estados no código, ~/.local/share/jaci/estados instalado)
    #[arg(long)]
    states_dir: Option<PathBuf>,
    /// Data e hora de início (UTC); padrão: agora.
    #[arg(long)]
    start: Option<String>,
    /// Semente do gerador aleatório (clima, falhas de solo).
    #[arg(long, default_value_t = 2026)]
    seed: u64,
    /// Recebe telemetria CCSDS de computadores de bordo nesta porta UDP
    /// (ex.: 0.0.0.0:10015).
    #[arg(long)]
    ccsds_in: Option<String>,
    /// Envia a telemetria dos veículos simulados em CCSDS para este endereço UDP.
    #[arg(long)]
    ccsds_out: Option<String>,
    /// Começa gravando a missão (para reproduzir depois no Instrutor).
    #[arg(long)]
    record: bool,
    /// Pasta das gravações (padrão: ao lado da pasta dos estados salvos).
    #[arg(long)]
    recordings_dir: Option<PathBuf>,
    /// Sala com posições: cada tela entra numa (FLIGHT, FIDO, EECOM, …) e só
    /// comanda o que é dela. Também se liga pelo console do instrutor.
    #[arg(long)]
    room: bool,
}

pub struct App {
    pub world: World,
    pub states_dir: PathBuf,
    /// Último evento já enviado (contador do mundo).
    pub sent_events: u64,
    /// Assinatura dos dicionários de telemetria (muda quando um veículo muda de forma).
    pub dict_sig: String,
    /// Para onde mandar telecomandos de cada veículo externo.
    pub tc_routes: HashMap<u32, SocketAddr>,
    pub tc_seq: u16,
    pub tc_socket: Option<Arc<UdpSocket>>,
    /// Telemetria recente de cada veículo (uma amostra por segundo de relógio),
    /// para quem conecta no meio da missão ver os gráficos desde antes.
    pub history: HashMap<u32, snapshot::Hist>,
    /// Ligação com o KSP (kOS), se ativa.
    pub ksp: Option<ksp::KspLink>,
    pub recordings_dir: PathBuf,
    /// Lista dos satélites reais (TLE), guardada a cada mudança e lida ao iniciar.
    pub satellites_file: PathBuf,
    /// Gravação em andamento.
    pub recorder: Option<recording::Recorder>,
    /// Reprodução de uma gravação: a simulação fica parada enquanto isso.
    pub replay: Option<recording::Replay>,
    /// Sala com posições e permissões.
    pub room: bool,
    /// Quem está em qual posição, por conexão.
    pub members: HashMap<u64, room::Member>,
    pub next_conn: u64,
}

impl App {
    /// Começa a gravar (nome automático se vazio).
    fn start_recording(&mut self, name: Option<&str>) -> Result<String, String> {
        if self.replay.is_some() {
            return Err("em reprodução: saia dela para gravar".into());
        }
        if let Some(r) = &self.recorder {
            return Err(format!("já gravando: {}", r.name));
        }
        let name = name.map(recording::clean_name).filter(|n| !n.is_empty()).unwrap_or_else(|| {
            let (y, mo, d, h, mi, s) = jaci_sim::time::to_calendar(now_j2000());
            format!("{}-{y:04}{mo:02}{d:02}-{h:02}{mi:02}{:02}", self.world.scenario, s.floor() as u32)
        });
        let hello = snapshot::hello(self);
        let mut rec = recording::Recorder::create(&self.recordings_dir, &name, &hello, self.world.event_seq).map_err(|e| e.to_string())?;
        // O dicionário atual entra logo no começo.
        let _ = rec.dictionary(&snapshot::dictionary(self).to_string());
        let msg = format!("Gravação iniciada: {}", rec.name);
        self.world.event(None, jaci_sim::vehicle::Level::Info, &msg);
        self.recorder = Some(rec);
        Ok(msg)
    }

    /// Passa ao controle de lançamento quais posições da votação têm gente.
    fn sync_staffed(&mut self) {
        let ids: Vec<String> = if self.room {
            let mut v: Vec<String> = self.members.values().filter_map(|m| room::role(&m.position)).flat_map(|r| r.launch.iter().map(|s| s.to_string())).collect();
            v.sort();
            v.dedup();
            v
        } else {
            vec![]
        };
        self.world.launch.set_staffed(ids);
    }

    /// Na sala com posições, confere se quem mandou a mensagem pode.
    fn check(&self, conn: u64, msg: &Value) -> Result<(), String> {
        if !self.room {
            return Ok(());
        }
        let position = self.members.get(&conn).map_or("OBS", |m| m.position.as_str());
        let kind = msg.get("type").and_then(|x| x.as_str()).unwrap_or("");
        match kind {
            "chat" if position == "OBS" => Err("observador não fala nos loops".into()),
            "plan" | "plan_apply" if !matches!(position, "FLIGHT" | "FIDO") => Err(format!("{position} não aplica planos de manobra (FIDO ou FLIGHT)")),
            "command" => {
                let id = msg.pointer("/cmd/id").and_then(|x| x.as_str()).unwrap_or("");
                if id == "vote" || id == "position" {
                    let lp = msg.pointer("/cmd/args/position").and_then(|x| x.as_str()).unwrap_or("");
                    return if room::can_vote(position, lp) { Ok(()) } else { Err(format!("{position} não vota pela posição {lp}")) };
                }
                let group = jaci_sim::commands::dictionary().into_iter().chain(ksp::commands()).find(|c| c.id == id).map_or("", |c| c.group);
                if room::can_command(position, group) { Ok(()) } else { Err(format!("{position} não comanda {group}")) }
            }
            "sim" => {
                let action = msg.get("action").and_then(|x| x.as_str()).unwrap_or("");
                if room::can_sim(position, action) { Ok(()) } else { Err(format!("{position} não pode pedir isso ao simulador ({action})")) }
            }
            _ => Ok(()),
        }
    }

    /// Guarda a lista de satélites reais.
    fn save_satellites(&self) {
        if let Some(dir) = self.satellites_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = std::fs::write(&self.satellites_file, jaci_sim::tracked::to_text(&self.world.tracked)) {
            tracing::warn!("não guardei os satélites em {}: {e}", self.satellites_file.display());
        }
    }

    /// Mundo novo (cenário ou estado): os satélites reais continuam na lista.
    fn replace_world(&mut self, nw: World) {
        let sats = std::mem::take(&mut self.world.tracked);
        self.world = nw;
        let have: Vec<u64> = self.world.tracked.iter().map(|s| s.norad).collect();
        for s in sats.iter().filter(|s| !have.contains(&s.norad)) {
            let _ = self.world.track(Some(&s.name), &s.line1, &s.line2);
        }
        self.sent_events = self.world.event_seq;
        self.sync_staffed();
    }

    /// Termina a gravação em andamento, se houver.
    fn stop_recording(&mut self) -> Option<String> {
        let rec = self.recorder.take()?;
        let (name, frames) = (rec.name.clone(), rec.frames);
        let msg = match rec.finish() {
            Ok(_) => format!("Gravação encerrada: {name} ({frames} quadros)"),
            Err(e) => format!("Gravação {name} encerrada com erro: {e}"),
        };
        self.world.event(None, jaci_sim::vehicle::Level::Info, &msg);
        Some(msg)
    }
}

type Shared = Arc<Mutex<App>>;

#[derive(Clone)]
struct Ctx {
    app: Shared,
    tx: broadcast::Sender<Arc<str>>,
    token: Option<String>,
}

/// A interface: a pedida, a do código (web/dist) ou a instalada ao lado do binário.
fn web_dir(args: &Args) -> PathBuf {
    if let Some(d) = &args.web_dir {
        return d.clone();
    }
    let local = PathBuf::from("web/dist");
    if local.join("index.html").exists() {
        return local;
    }
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent()?.parent().map(|p| p.join("share/jaci/web")))
        .filter(|d| d.join("index.html").exists())
        .unwrap_or(local)
}

/// Estados salvos: o pedido, ./estados no código ou ~/.local/share/jaci/estados.
fn states_dir(args: &Args) -> PathBuf {
    if let Some(d) = &args.states_dir {
        return d.clone();
    }
    if PathBuf::from("web/dist").exists() || PathBuf::from("Cargo.toml").exists() {
        return PathBuf::from("estados");
    }
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    data.join("jaci/estados")
}

/// Gravações: a pedida ou a pasta irmã da dos estados salvos.
fn recordings_dir(args: &Args, states: &std::path::Path) -> PathBuf {
    args.recordings_dir.clone().unwrap_or_else(|| states.with_file_name("gravacoes"))
}

fn open_browser(url: &str) {
    let cmd = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(cmd).arg(url).spawn() {
        tracing::warn!("não abri o navegador ({e}): abra {url}");
    }
}

pub fn now_j2000() -> f64 {
    let unix = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    unix - 946_728_000.0
}

fn random_token() -> String {
    let mut b = [0u8; 12];
    getrandom::fill(&mut b).expect("gerador do sistema");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let args = Args::parse();
    if args.list {
        for s in scenarios::SCENARIOS {
            println!("{:<12} {} — {}", s.id, s.name, s.description);
        }
        return Ok(());
    }
    let t0 = match &args.start {
        Some(s) => parse_utc(s).context("data de início inválida (use AAAA-MM-DD HH:MM:SS)")?,
        None => now_j2000(),
    };
    let scenario = args.scenario.clone().unwrap_or_else(|| if args.ksp.is_some() { "ksp" } else { "sondagem" }.into());
    let world = scenarios::build(&scenario, t0, args.seed).with_context(|| format!("cenário desconhecido: {scenario}"))?;
    let ksp_link = match &args.ksp {
        Some(dir) => Some(connect_ksp(dir.clone())?),
        None => None,
    };
    tracing::info!("cenário {} em {}", world.mission, format_utc(world.t));
    let states = states_dir(&args);
    let mut world = world;
    let satellites_file = states.with_file_name("satelites.tle");
    if let Ok(text) = std::fs::read_to_string(&satellites_file) {
        for (name, l1, l2) in jaci_sim::tracked::parse(&text) {
            if let Err(e) = world.track(name.as_deref(), &l1, &l2) {
                tracing::warn!("{e}");
            }
        }
        if !world.tracked.is_empty() {
            tracing::info!("{} satélite(s) real(is) de {}", world.tracked.len(), satellites_file.display());
        }
    }
    let app = Arc::new(Mutex::new(App {
        world,
        satellites_file,
        recordings_dir: recordings_dir(&args, &states),
        recorder: None,
        replay: None,
        room: args.room,
        members: HashMap::new(),
        next_conn: 0,
        states_dir: states,
        sent_events: 0,
        dict_sig: String::new(),
        tc_routes: HashMap::new(),
        tc_seq: 0,
        tc_socket: None,
        history: HashMap::new(),
        ksp: ksp_link,
    }));
    {
        let mut a = app.lock().unwrap();
        a.sent_events = a.world.event_seq;
        if args.record {
            match a.start_recording(None) {
                Ok(m) => tracing::info!("{m} em {}", a.recordings_dir.display()),
                Err(e) => tracing::warn!("não deu para gravar: {e}"),
            }
        }
    }
    let (tx, _) = broadcast::channel::<Arc<str>>(64);

    let addr: SocketAddr = args.addr.parse().context("endereço inválido")?;
    let token = match (&args.token, addr.ip().is_loopback()) {
        (Some(t), _) => Some(t.clone()),
        (None, true) => None,
        (None, false) => {
            let t = random_token();
            tracing::warn!("escutando na rede sem --token: use http://<ip>:{}/?token={t}", addr.port());
            Some(t)
        }
    };

    // Laço da simulação: 50 Hz, instantâneo a 5 Hz.
    {
        let app = app.clone();
        let tx = tx.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(20));
            let mut last = Instant::now();
            let mut last_snap = Instant::now();
            let mut last_hist = Instant::now();
            let mut last_rec = Instant::now();
            loop {
                tick.tick().await;
                let now = Instant::now();
                let dt = (now - last).as_secs_f64().min(0.25);
                last = now;
                let mut a = app.lock().unwrap();
                // Reproduzindo: a simulação espera; vão os quadros gravados.
                if let Some(rp) = a.replay.as_mut() {
                    if now - last_snap >= Duration::from_millis(200) {
                        let wall = (now - last_snap).as_secs_f64();
                        last_snap = now;
                        if let Some(f) = rp.tick(wall) {
                            let _ = tx.send(f.into());
                        }
                    }
                    continue;
                }
                a.world.advance(dt);
                if now - last_snap >= Duration::from_millis(200) {
                    last_snap = now;
                    if let Some(dict) = snapshot::dictionary_if_changed(&mut a) {
                        if let Some(rec) = a.recorder.as_mut() {
                            let _ = rec.dictionary(&dict);
                        }
                        let _ = tx.send(dict.into());
                    }
                    let s = snapshot::snapshot(&mut a);
                    let _ = tx.send(s.into());
                }
                // Gravação: um quadro por segundo, com os eventos do intervalo.
                if a.recorder.is_some() && now - last_rec >= Duration::from_secs(1) {
                    last_rec = now;
                    let since = a.recorder.as_ref().map_or(0, |r| r.event_seq);
                    let frame = snapshot::snapshot_value(&a, since);
                    let seq = a.world.event_seq;
                    let rec = a.recorder.as_mut().unwrap();
                    rec.event_seq = seq;
                    if let Err(e) = rec.frame(&frame) {
                        tracing::warn!("gravação interrompida: {e}");
                        a.stop_recording();
                    }
                }
                if now - last_hist >= Duration::from_millis(500) {
                    last_hist = now;
                    snapshot::record_history(&mut a);
                }
            }
        });
    }

    // Arquivos do kOS: lidos quatro vezes por segundo.
    {
        let app = app.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(250));
            loop {
                tick.tick().await;
                let mut a = app.lock().unwrap();
                let App { ksp, world, .. } = &mut *a;
                if let Some(link) = ksp {
                    link.poll(world);
                }
            }
        });
    }

    if let Some(addr) = &args.ccsds_in {
        let sock = Arc::new(UdpSocket::bind(addr).await.with_context(|| format!("UDP {addr}"))?);
        tracing::info!("recebendo telemetria CCSDS em {addr}");
        app.lock().unwrap().tc_socket = Some(sock.clone());
        let app = app.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65_536];
            loop {
                let Ok((n, from)) = sock.recv_from(&mut buf).await else { continue };
                let Some(p) = Packet::decode(&buf[..n]) else { continue };
                if p.telecommand {
                    // Telecomando vindo de outro centro: executa no simulado.
                    let Ok(cmd) = serde_json::from_slice::<Command>(&p.data) else { continue };
                    let vid = (p.apid as u32).saturating_sub(0x100);
                    let mut a = app.lock().unwrap();
                    if let Err(e) = a.world.send(vid, cmd, true, None) {
                        tracing::warn!("telecomando CCSDS recusado: {e}");
                    }
                    continue;
                }
                let names: Vec<(String, f64)> = p
                    .points()
                    .into_iter()
                    .filter_map(|(i, v)| ccsds::NAV_POINTS.get(i as usize).map(|(id, _, _)| (id.to_string(), v as f64)))
                    .collect();
                let mut a = app.lock().unwrap();
                let id = a.world.external_update(p.apid, names);
                a.tc_routes.insert(id, from);
            }
        });
    }

    if let Some(dest) = args.ccsds_out.clone() {
        let app = app.clone();
        tokio::spawn(async move {
            let Ok(sock) = UdpSocket::bind("0.0.0.0:0").await else { return };
            tracing::info!("enviando telemetria CCSDS para {dest}");
            let mut seq: u16 = 0;
            let mut tick = tokio::time::interval(Duration::from_secs(1));
            loop {
                tick.tick().await;
                let packets: Vec<Vec<u8>> = {
                    let a = app.lock().unwrap();
                    a.world
                        .vehicles
                        .iter()
                        .filter(|v| v.kind != jaci_sim::vehicle::Kind::Debris)
                        .map(|v| {
                            let vals = jaci_sim::telemetry::values(v, a.world.t);
                            let pts: Vec<(u16, f32)> = vals.iter().enumerate().map(|(i, (_, x))| (i as u16, *x as f32)).collect();
                            seq = seq.wrapping_add(1);
                            Packet::tm(0x100 + v.id as u16, seq, a.world.t, &pts).encode()
                        })
                        .collect()
                };
                for p in packets {
                    let _ = sock.send_to(&p, &dest).await;
                }
            }
        });
    }

    let url_token = token.clone();
    let ctx = Ctx { app, tx, token };
    let web_dir = web_dir(&args);
    let index = web_dir.join("index.html");
    let static_files = ServeDir::new(&web_dir).not_found_service(ServeFile::new(&index));
    let router = Router::new().route("/ws", get(ws_handler)).route("/api/csv", get(csv_handler)).fallback_service(static_files).with_state(ctx);
    let url = format!("http://{addr}/{}", url_token.as_ref().map(|t| format!("?token={t}")).unwrap_or_default());
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        // Já rodando (o atalho clicado de novo): só abre o navegador.
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse && args.open => {
            open_browser(&url);
            return Ok(());
        }
        Err(e) => return Err(e).with_context(|| format!("porta {addr}")),
    };
    if args.open {
        open_browser(&url);
    }
    if !index.exists() {
        tracing::warn!("interface não encontrada em {} (compile com `npm run build` em web/)", web_dir.display());
    }
    tracing::info!("Jaci em http://{addr}/");
    axum::serve(listener, router).await?;
    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade, Query(q): Query<HashMap<String, String>>, State(ctx): State<Ctx>) -> Response {
    if let Some(t) = &ctx.token {
        if q.get("token") != Some(t) {
            return (StatusCode::UNAUTHORIZED, "token inválido").into_response();
        }
    }
    ws.on_upgrade(move |s| client(s, ctx))
}

/// Telemetria de um veículo em CSV: a gravação inteira, na reprodução, ou o
/// histórico recente (30 min de relógio), ao vivo.
async fn csv_handler(Query(q): Query<HashMap<String, String>>, State(ctx): State<Ctx>) -> Response {
    if let Some(t) = &ctx.token {
        if q.get("token") != Some(t) {
            return (StatusCode::UNAUTHORIZED, "token inválido").into_response();
        }
    }
    let Some(id) = q.get("vehicle").and_then(|v| v.parse::<u32>().ok()) else {
        return (StatusCode::BAD_REQUEST, "falta ?vehicle=").into_response();
    };
    let (body, name) = {
        let a = ctx.app.lock().unwrap();
        match &a.replay {
            Some(rp) => {
                let dict: Value = serde_json::from_str::<Value>(&rp.hello()).map(|h| h["dictionary"].clone()).unwrap_or(Value::Null);
                (rp.csv(id, &dict), rp.name.clone())
            }
            None => (snapshot::csv(&a, id), a.world.scenario.clone()),
        }
    };
    match body {
        Some(b) => (
            [(header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"jaci-{}-{id}.csv\"", recording::clean_name(&name).replace(' ', "_")))],
            b,
        )
            .into_response(),
        None => (StatusCode::NOT_FOUND, "sem telemetria desse veículo").into_response(),
    }
}

async fn client(socket: WebSocket, ctx: Ctx) {
    let (mut sink, mut stream) = socket.split();
    let (conn, hello, history, frame) = {
        let mut a = ctx.app.lock().unwrap();
        a.next_conn += 1;
        let conn = a.next_conn;
        match a.replay.as_mut() {
            // Na reprodução, o quadro atual vai junto: parada, ela não manda outro.
            Some(rp) => (conn, rp.hello(), rp.history(), Some(rp.current())),
            None => (conn, snapshot::hello(&mut a), snapshot::history(&a), None),
        }
    };
    for m in [Some(hello), Some(history), frame].into_iter().flatten() {
        if sink.send(Message::Text(m.into())).await.is_err() {
            return;
        }
    }
    let mut rx = ctx.tx.subscribe();
    let (reply_tx, mut reply_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        loop {
            tokio::select! {
                m = rx.recv() => match m {
                    Ok(s) => if sink.send(Message::Text(s.to_string().into())).await.is_err() { break },
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                },
                r = reply_rx.recv() => match r {
                    Some(s) => if sink.send(Message::Text(s.into())).await.is_err() { break },
                    None => break,
                },
            }
        }
    });
    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(text) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
        let reply = handle(&ctx, conn, &v);
        if let Some(r) = reply {
            if reply_tx.send(r.to_string()).is_err() {
                break;
            }
        }
    }
    writer.abort();
    // Saiu da sala.
    let mut a = ctx.app.lock().unwrap();
    if let Some(m) = a.members.remove(&conn) {
        if a.replay.is_none() {
            a.world.event(None, jaci_sim::vehicle::Level::Info, format!("{} saiu da sala", m.label()));
        }
        a.sync_staffed();
    }
}

/// Ações do Instrutor durante a reprodução: controlam a reprodução. Devolve a
/// resposta e, se for o caso, uma mensagem para todas as interfaces.
fn replay_sim(a: &mut App, msg: &Value, req: &Value) -> (Value, Option<String>) {
    let action = msg.get("action").and_then(|x| x.as_str()).unwrap_or("");
    let ok = |m: &str| json!({"type": "sim_result", "req": req, "ok": true, "message": m});
    let Some(rp) = a.replay.as_mut() else { return (ok(""), None) };
    match action {
        "pause" => rp.control(Some(false), None),
        "resume" => rp.control(Some(true), None),
        "warp" => rp.control(None, msg.get("value").and_then(|x| x.as_f64())),
        "replay_control" => {
            rp.control(msg.get("playing").and_then(|x| x.as_bool()), msg.get("speed").and_then(|x| x.as_f64()));
            if let Some(i) = msg.get("seek").and_then(|x| x.as_u64()) {
                return (ok("ok"), Some(rp.seek(i as usize)));
            }
        }
        "replay_stop" => {
            a.replay = None;
            a.dict_sig.clear();
            a.sent_events = a.world.event_seq;
            return (json!({"type": "reload", "req": req}), Some(json!({"type": "reload"}).to_string()));
        }
        "recordings" => return (json!({"type": "recordings", "req": req, "list": recording::list(&a.recordings_dir), "dir": a.recordings_dir.display().to_string()}), None),
        "saves" | "ack" => return (json!({"type": "sim_result", "req": req, "ok": false, "error": "em reprodução"}), None),
        _ => return (json!({"type": "sim_result", "req": req, "ok": false, "error": "em reprodução: saia dela primeiro"}), None),
    }
    (ok("ok"), None)
}

/// Liga ao KSP: acha a pasta, instala os scripts do kOS.
fn connect_ksp(dir: Option<PathBuf>) -> anyhow::Result<ksp::KspLink> {
    let dir = dir.or_else(ksp::find_ksp).context("não achei o Kerbal Space Program; passe a pasta: --ksp <pasta>")?;
    anyhow::ensure!(dir.join("GameData").is_dir(), "{} não parece ser a pasta do KSP (falta GameData)", dir.display());
    let link = ksp::KspLink::new(dir.clone()).with_context(|| format!("não instalei os scripts em {}", dir.display()))?;
    tracing::info!("KSP em {}: scripts em {}", dir.display(), link.dir.display());
    if !link.kos_installed {
        tracing::warn!("o kOS não está em GameData: instale pelo CKAN (ckan install kOS)");
    }
    Ok(link)
}

fn vid(v: &Value) -> u32 {
    v.get("vehicle").and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0) as u32
}

/// Trata uma mensagem da interface; devolve a resposta (só para quem pediu).
fn handle(ctx: &Ctx, conn: u64, msg: &Value) -> Option<Value> {
    let kind = msg.get("type")?.as_str()?;
    let req = msg.get("req").cloned().unwrap_or(Value::Null);
    let mut a = ctx.app.lock().unwrap();
    if let Err(e) = a.check(conn, msg) {
        let reply = match kind {
            "command" => "command_result",
            "sim" => "sim_result",
            "plan_apply" => "plan_applied",
            "chat" => "chat_result",
            k => k,
        };
        return Some(json!({"type": reply, "req": req, "ok": false, "error": e}));
    }
    // Entrar na sala, sair e falar nos loops.
    match kind {
        "join" => {
            let name = room::clean(msg.get("name").and_then(|x| x.as_str()).unwrap_or(""), 40);
            let position = msg.get("position").and_then(|x| x.as_str()).unwrap_or("OBS");
            if name.is_empty() || room::role(position).is_none() {
                return Some(json!({"type": "joined", "req": req, "ok": false, "error": "nome e posição"}));
            }
            let m = room::Member { name, position: position.to_string() };
            let changed = a.members.get(&conn).is_none_or(|old| old.position != m.position || old.name != m.name);
            if changed && a.replay.is_none() {
                a.world.event(None, jaci_sim::vehicle::Level::Info, format!("{} entrou na sala", m.label()));
            }
            a.members.insert(conn, m.clone());
            a.sync_staffed();
            return Some(json!({"type": "joined", "req": req, "ok": true, "name": m.name, "position": m.position}));
        }
        "leave" => {
            if let Some(m) = a.members.remove(&conn) {
                a.world.event(None, jaci_sim::vehicle::Level::Info, format!("{} saiu da sala", m.label()));
                a.sync_staffed();
            }
            return Some(json!({"type": "joined", "req": req, "ok": true, "position": null}));
        }
        "chat" => {
            if a.replay.is_some() {
                return Some(json!({"type": "chat_result", "req": req, "ok": false, "error": "em reprodução"}));
            }
            let text = room::clean(msg.get("text").and_then(|x| x.as_str()).unwrap_or(""), 500);
            let channel = room::clean(msg.get("channel").and_then(|x| x.as_str()).unwrap_or("geral"), 20);
            if text.is_empty() {
                return Some(json!({"type": "chat_result", "req": req, "ok": false, "error": "mensagem vazia"}));
            }
            let from = a.members.get(&conn).map_or_else(|| "Console".to_string(), |m| m.label());
            a.world.chat(&from, if channel.is_empty() { "geral" } else { &channel }, &text);
            return Some(json!({"type": "chat_result", "req": req, "ok": true}));
        }
        _ => {}
    }
    // Na reprodução não há o que comandar; o traço previsto vem do estado gravado.
    if let Some(rp) = a.replay.as_mut() {
        match kind {
            "command" => return Some(json!({"type": "command_result", "req": req, "ok": false, "error": "em reprodução: comandos desligados"})),
            "plan" | "plan_apply" => return Some(json!({"type": kind, "req": req, "ok": false, "error": "em reprodução: comandos desligados"})),
            "passes" => return Some(json!({"type": "passes", "req": req, "vehicle": vid(msg), "passes": []})),
            "predict" => {
                let id = vid(msg);
                let span = msg.get("span").and_then(|x| x.as_f64()).unwrap_or(6_000.0).clamp(60.0, 3.0e8);
                let step = msg.get("step").and_then(|x| x.as_f64()).unwrap_or(span / 400.0).clamp(span / 5_000.0, span).max(1.0);
                let (central, pts) = rp.predict(id, span, step);
                return Some(json!({"type": "predict", "req": req, "vehicle": id, "central": central, "points": pts}));
            }
            "sim" => {
                let (reply, broadcast) = replay_sim(&mut a, msg, &req);
                if let Some(b) = broadcast {
                    let _ = ctx.tx.send(b.into());
                }
                return Some(reply);
            }
            _ => {}
        }
    }
    let out = match kind {
        "command" => {
            let id = vid(msg);
            let cmd: Command = serde_json::from_value(msg.get("cmd")?.clone()).ok()?;
            let armed = msg.get("armed").and_then(|x| x.as_bool()).unwrap_or(false);
            let at = msg.get("at").and_then(|x| x.as_str()).and_then(parse_utc).or_else(|| msg.get("in").and_then(|x| x.as_f64()).map(|d| a.world.t + d));
            if a.world.tracked.iter().any(|s| s.id == id) {
                return Some(json!({"type": "command_result", "req": req, "ok": false, "error": "satélite real: só acompanhamento"}));
            }
            // Nave do KSP: o comando vai para o kOS.
            let App { ksp, world, .. } = &mut *a;
            if let Some(link) = ksp.as_mut().filter(|l| l.vessel(id).is_some()) {
                return Some(match link.command(world, id, &cmd, armed) {
                    Ok(n) => json!({"type": "command_result", "req": req, "ok": true, "id": n}),
                    Err(e) => json!({"type": "command_result", "req": req, "ok": false, "error": e}),
                });
            }
            // Veículo externo: o comando vai por CCSDS.
            if let Some(dest) = a.tc_routes.get(&id).copied() {
                a.tc_seq = a.tc_seq.wrapping_add(1);
                let apid = a.world.externals.iter().find(|e| e.id == id).map_or(0x200, |e| e.apid);
                let p = Packet::tc(apid, a.tc_seq, &serde_json::to_string(&cmd).unwrap_or_default());
                if let Some(sock) = a.tc_socket.clone() {
                    let bytes = p.encode();
                    tokio::spawn(async move {
                        let _ = sock.send_to(&bytes, dest).await;
                    });
                }
                a.world.event(Some(id), jaci_sim::vehicle::Level::Info, format!("Telecomando CCSDS enviado: {}", cmd.describe()));
                return Some(json!({"type": "command_result", "req": req, "ok": true, "message": "enviado por CCSDS"}));
            }
            match a.world.send(id, cmd, armed, at) {
                Ok(n) => json!({"type": "command_result", "req": req, "ok": true, "id": n}),
                Err(e) => json!({"type": "command_result", "req": req, "ok": false, "error": e}),
            }
        }
        "sim" => {
            let out = sim(&mut a, msg, &req);
            // Mundo novo (cenário, estado ou reprodução): todas as interfaces recomeçam.
            if out["type"] == "reload" {
                let _ = ctx.tx.send(Arc::from(json!({"type": "reload"}).to_string()));
            }
            out
        }
        "plan" => {
            let id = vid(msg);
            if a.ksp.as_ref().is_some_and(|l| l.vessel(id).is_some()) {
                return Some(json!({"type": "plan", "req": req, "vehicle": id, "ok": false, "error": "nave do KSP: use os comandos de manobra do KSP"}));
            }
            if a.world.tracked.iter().any(|s| s.id == id) {
                return Some(json!({"type": "plan", "req": req, "vehicle": id, "ok": false, "error": "satélite real: só acompanhamento"}));
            }
            let v = a.world.vehicle(id)?.clone();
            let p = msg.get("params").cloned().unwrap_or(json!({}));
            let num = |k: &str, d: f64| p.get(k).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.replace(',', ".").parse().ok()))).unwrap_or(d);
            let target = p.get("target").and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).and_then(|t| a.world.vehicle(t as u32).cloned());
            let body = p.get("body").and_then(|x| x.as_str()).and_then(Body::from_id).unwrap_or(Body::Mars);
            let t = a.world.t;
            let plan = match msg.get("kind")?.as_str()? {
                "circularize_ap" => planner::circularize(&v, true),
                "circularize_pe" => planner::circularize(&v, false),
                "apoapsis" => planner::change_apsis(&v, true, num("alt", 400.0)),
                "periapsis" => planner::change_apsis(&v, false, num("alt", 400.0)),
                "hohmann" => planner::hohmann_to(&v, num("alt", 400.0)),
                "inclination" => planner::change_inclination(&v, num("inc", 51.6)),
                "match_plane" => target.as_ref().and_then(|tg| planner::match_plane(&v, tg)),
                "deorbit" => {
                    let lt = v.guidance.landing_target.or_else(|| p.get("lat").and_then(|_| Some((num("lat", 0.0), num("lon", 0.0)))));
                    planner::deorbit(&v, t, num("pe", if v.kind == jaci_sim::vehicle::Kind::Shuttle { 30.0 } else { 40.0 }), lt)
                }
                "rendezvous" => target.as_ref().and_then(|tg| planner::rendezvous(&v, tg)),
                "injection" => planner::injection(&v, t, body, num("depart_in_h", 1.0) * 3600.0, num("tof_days", 200.0) * 86_400.0),
                "tcm" => {
                    let arrival = p.get("arrival").and_then(|x| x.as_str()).and_then(parse_utc).unwrap_or(t + num("arrive_in_days", 100.0) * 86_400.0);
                    planner::tcm(&v, t, body, arrival, num("pe", 300.0))
                }
                "capture" => planner::capture(&v, num("apo", 10_000.0)),
                _ => None,
            };
            match plan {
                Some(p) => json!({"type": "plan", "req": req, "vehicle": id, "ok": true, "plan": p}),
                None => json!({"type": "plan", "req": req, "vehicle": id, "ok": false, "error": "sem solução para esse plano nesta órbita"}),
            }
        }
        "plan_apply" => {
            let id = vid(msg);
            let nodes: Vec<jaci_sim::vehicle::Node> = serde_json::from_value(msg.get("nodes")?.clone()).ok()?;
            let landing = msg.get("landing").and_then(|l| serde_json::from_value::<(f64, f64)>(l.clone()).ok());
            let plan = planner::Plan { nodes, total_dv: 0.0, summary: String::new(), landing };
            let ids = a.world.add_plan(id, &plan);
            a.world.event(Some(id), jaci_sim::vehicle::Level::Info, format!("{} queima(s) acrescentada(s) ao plano de voo", ids.len()));
            json!({"type": "plan_applied", "req": req, "ids": ids})
        }
        "predict" => {
            let id = vid(msg);
            let span = msg.get("span").and_then(|x| x.as_f64()).unwrap_or(6_000.0).clamp(60.0, 3.0e8);
            let step = msg.get("step").and_then(|x| x.as_f64()).unwrap_or((span / 400.0).max(10.0));
            if let Some(link) = a.ksp.as_ref().filter(|l| l.vessel(id).is_some()) {
                let central = link.vessel(id).map(|v| v.body.name.to_lowercase());
                return Some(json!({"type": "predict", "req": req, "vehicle": id, "central": central, "points": link.predict(id, a.world.t, span, step)}));
            }
            let pts: Vec<Value> = a.world.predict(id, span, step).into_iter().map(|(t, la, lo, al, r)| json!([t, la, lo, al / 1e3, r.x, r.y, r.z])).collect();
            let central = a.world.vehicle(id).map(|v| v.central.id()).or_else(|| a.world.tracked.iter().any(|s| s.id == id).then_some("earth"));
            json!({"type": "predict", "req": req, "vehicle": id, "central": central, "points": pts})
        }
        "passes" => {
            let id = vid(msg);
            let span = msg.get("span").and_then(|x| x.as_f64()).unwrap_or(86_400.0).min(3.0 * 86_400.0);
            if let Some(s) = a.world.tracked.iter().find(|s| s.id == id) {
                let p = s.passes(&a.world.stations, a.world.t, span);
                return Some(json!({"type": "passes", "req": req, "vehicle": id, "passes": p}));
            }
            let v = a.world.vehicle(id)?.clone();
            let p = jaci_sim::ground::passes(&v, &a.world.stations, a.world.t, span);
            json!({"type": "passes", "req": req, "vehicle": id, "passes": p})
        }
        "porkchop" => {
            let from = msg.get("from").and_then(|x| x.as_str()).and_then(Body::from_id).unwrap_or(Body::Earth);
            let to = msg.get("to").and_then(|x| x.as_str()).and_then(Body::from_id).unwrap_or(Body::Mars);
            let start = msg.get("start").and_then(|x| x.as_str()).and_then(parse_utc).unwrap_or(a.world.t);
            let g = |k: &str, d: f64| msg.get(k).and_then(|x| x.as_f64()).unwrap_or(d);
            drop(a);
            let pc = planner::porkchop(from, to, start, g("span_days", 400.0), g("tof_min", 100.0), g("tof_max", 400.0), g("n", 40.0) as usize);
            return Some(json!({"type": "porkchop", "req": req, "data": pc}));
        }
        "dictionary" => {
            let mut d = snapshot::dictionary(&a);
            d["req"] = req;
            d
        }
        _ => return None,
    };
    Some(out)
}

fn sim(a: &mut App, msg: &Value, req: &Value) -> Value {
    let action = msg.get("action").and_then(|x| x.as_str()).unwrap_or("");
    let w = &mut a.world;
    let ok = |m: &str| json!({"type": "sim_result", "req": req, "ok": true, "message": m});
    match action {
        "warp" => {
            let k = msg.get("value").and_then(|x| x.as_f64()).unwrap_or(1.0).clamp(0.1, 1_000_000.0);
            w.warp = k;
            ok(&format!("{k}×"))
        }
        "pause" => {
            w.paused = true;
            ok("pausado")
        }
        "resume" => {
            w.paused = false;
            ok("rodando")
        }
        "warp_stop" => {
            w.warp_stop = msg.get("value").and_then(|x| x.as_bool()).unwrap_or(true);
            ok("ok")
        }
        "scenario" => {
            let id = msg.get("value").and_then(|x| x.as_str()).unwrap_or("sondagem");
            let t = msg.get("start").and_then(|x| x.as_str()).and_then(parse_utc).unwrap_or_else(now_j2000);
            match scenarios::build(id, t, 2026) {
                Some(nw) => {
                    a.stop_recording();
                    a.replace_world(nw);
                    a.dict_sig.clear();
                    a.tc_routes.clear();
                    a.history.clear();
                    json!({"type": "reload", "req": req})
                }
                None => json!({"type": "sim_result", "req": req, "ok": false, "error": "cenário desconhecido"}),
            }
        }
        "save" => {
            let name: String = msg.get("value").and_then(|x| x.as_str()).unwrap_or("estado").chars().filter(|c| c.is_alphanumeric() || "-_ ".contains(*c)).collect();
            let name = if name.trim().is_empty() { "estado".to_string() } else { name.trim().to_string() };
            let _ = std::fs::create_dir_all(&a.states_dir);
            let path = a.states_dir.join(format!("{name}.json"));
            match serde_json::to_string(&a.world).map_err(|e| e.to_string()).and_then(|s| std::fs::write(&path, s).map_err(|e| e.to_string())) {
                Ok(()) => ok(&format!("salvo em {}", path.display())),
                Err(e) => json!({"type": "sim_result", "req": req, "ok": false, "error": e}),
            }
        }
        "saves" => {
            let mut list: Vec<String> = std::fs::read_dir(&a.states_dir)
                .map(|d| d.filter_map(|e| e.ok()).filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".json")).map(String::from)).collect())
                .unwrap_or_default();
            list.sort();
            json!({"type": "saves", "req": req, "list": list})
        }
        "load" => {
            let name = msg.get("value").and_then(|x| x.as_str()).unwrap_or("");
            let path = a.states_dir.join(format!("{name}.json"));
            match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|s| serde_json::from_str::<World>(&s).map_err(|e| e.to_string())) {
                Ok(nw) => {
                    a.stop_recording();
                    a.replace_world(nw);
                    a.dict_sig.clear();
                    a.history.clear();
                    json!({"type": "reload", "req": req})
                }
                Err(e) => json!({"type": "sim_result", "req": req, "ok": false, "error": e}),
            }
        }
        "weather" => {
            let wx = &mut w.weather;
            let g = |k: &str| msg.get(k).and_then(|x| x.as_f64());
            if let Some(x) = g("surface_wind") {
                wx.surface_wind = x;
            }
            if let Some(x) = g("wind_from") {
                wx.wind_from = x;
            }
            if let Some(x) = g("jet_wind") {
                wx.jet_wind = x;
            }
            if let Some(x) = g("lightning_km") {
                wx.lightning_km = x;
            }
            if let Some(x) = g("ceiling_m") {
                wx.ceiling_m = x;
            }
            if let Some(x) = msg.get("evolve").and_then(|x| x.as_bool()) {
                wx.evolve = x;
            }
            if let Some(x) = msg.get("rain").and_then(|x| x.as_bool()) {
                wx.rain = x;
            }
            ok("clima ajustado")
        }
        "fault" => {
            let id = vid(msg);
            let f = msg.get("fault").and_then(|x| x.as_str()).unwrap_or("");
            let arg = msg.get("arg").and_then(|x| x.as_str().map(String::from).or_else(|| x.as_f64().map(|n| n.to_string())));
            let announce = msg.get("announce").and_then(|x| x.as_bool()).unwrap_or(true);
            match w.inject_as(id, f, arg.as_deref(), announce, false) {
                Ok(m) => ok(&m),
                Err(e) => json!({"type": "sim_result", "req": req, "ok": false, "error": e}),
            }
        }
        "clear_faults" => {
            w.clear_faults(vid(msg));
            ok("falhas removidas")
        }
        "ack" => {
            w.ack(msg.get("value").and_then(|x| x.as_u64()));
            ok("reconhecido")
        }
        "plan_fault" => {
            let id = vid(msg);
            let f = msg.get("fault").and_then(|x| x.as_str()).unwrap_or("random");
            if f != "random" && !jaci_sim::world::FAULTS.iter().any(|(k, _, _)| *k == f) {
                return json!({"type": "sim_result", "req": req, "ok": false, "error": "falha desconhecida"});
            }
            let arg = msg.get("arg").and_then(|x| x.as_str().map(String::from).or_else(|| x.as_f64().map(|n| n.to_string()))).filter(|s| !s.is_empty());
            let num = |k: &str| msg.get(k).and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.replace(',', ".").parse().ok())));
            let value = num("value").unwrap_or(0.0);
            let when = match msg.get("when").and_then(|x| x.as_str()).unwrap_or("in") {
                "met" => jaci_sim::training::FaultWhen::Met(value),
                "at" => match msg.get("at").and_then(|x| x.as_str()).and_then(parse_utc) {
                    Some(t) => jaci_sim::training::FaultWhen::At(t),
                    None => return json!({"type": "sim_result", "req": req, "ok": false, "error": "hora inválida (AAAA-MM-DD HH:MM:SS)"}),
                },
                _ => jaci_sim::training::FaultWhen::At(w.t + value),
            };
            let window = num("window").unwrap_or(0.0);
            let probability = num("probability").unwrap_or(1.0);
            let announce = msg.get("announce").and_then(|x| x.as_bool()).unwrap_or(false);
            let n = w.plan_fault(id, f, arg, when, window, probability, announce);
            ok(&format!("falha planejada nº {n}"))
        }
        "unplan_fault" => {
            if w.unplan_fault(msg.get("id").and_then(|x| x.as_u64()).unwrap_or(0) as u32) {
                ok("falha retirada do plano")
            } else {
                json!({"type": "sim_result", "req": req, "ok": false, "error": "não está no plano"})
            }
        }
        "fault_plan" => {
            // Só para o console do instrutor (não vai no instantâneo: a equipe não vê).
            let plan: Vec<Value> = w
                .training
                .plan
                .iter()
                .map(|f| {
                    let name = w.vehicle(f.vehicle).map(|v| v.name.clone()).unwrap_or_else(|| format!("#{}", f.vehicle));
                    json!({"plan": f, "vehicle_name": name, "due": w.plan_due(f)})
                })
                .collect();
            json!({"type": "fault_plan", "req": req, "plan": plan, "t": w.t})
        }
        "report" => json!({"type": "report", "req": req, "report": w.report()}),
        "report_reset" => {
            w.reset_training();
            ok("relatório recomeçado")
        }
        "proc_log" => {
            let id = vid(msg);
            let name = msg.get("procedure").and_then(|x| x.as_str()).unwrap_or("?");
            let kind = msg.get("kind").and_then(|x| x.as_str()).unwrap_or("step");
            let step = msg.get("step").and_then(|x| x.as_u64()).map(|x| x as u32);
            let total = msg.get("total").and_then(|x| x.as_u64()).map(|x| x as u32);
            w.log_procedure(id, name, kind, step, total);
            ok("ok")
        }
        "station" => {
            let id = msg.get("id").and_then(|x| x.as_str()).unwrap_or("");
            let en = msg.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true);
            if let Some(s) = w.stations.iter_mut().find(|s| s.id == id) {
                s.enabled = en;
            }
            ok("ok")
        }
        "ksp_connect" => {
            let dir = msg.get("value").and_then(|x| x.as_str()).filter(|s| !s.trim().is_empty()).map(|s| PathBuf::from(s.trim()));
            match connect_ksp(dir) {
                Ok(link) => {
                    let m = format!("KSP ligado: scripts em {}", link.dir.display());
                    a.world.event(None, jaci_sim::vehicle::Level::Good, &m);
                    a.ksp = Some(link);
                    a.dict_sig.clear();
                    json!({"type": "sim_result", "req": req, "ok": true, "message": m})
                }
                Err(e) => json!({"type": "sim_result", "req": req, "ok": false, "error": format!("{e:#}")}),
            }
        }
        "ksp_patch" => match a.ksp.as_mut().map(|l| l.install_patch()) {
            Some(Ok(())) => ok("patch instalado: reinicie o KSP para pôr o kOS nas peças de comando"),
            Some(Err(e)) => json!({"type": "sim_result", "req": req, "ok": false, "error": e.to_string()}),
            None => json!({"type": "sim_result", "req": req, "ok": false, "error": "KSP não ligado"}),
        },
        "track" => {
            let text = msg.get("value").and_then(|x| x.as_str()).unwrap_or("");
            let found = jaci_sim::tracked::parse(text);
            if found.is_empty() {
                return json!({"type": "sim_result", "req": req, "ok": false, "error": "nenhum TLE no texto (linhas 1 e 2, com o nome antes)"});
            }
            let (mut ok, mut errors) = (0, Vec::new());
            for (name, l1, l2) in &found {
                match a.world.track(name.as_deref(), l1, l2) {
                    Ok(_) => ok += 1,
                    Err(e) => errors.push(e),
                }
            }
            a.save_satellites();
            a.dict_sig.clear();
            if ok == 0 {
                json!({"type": "sim_result", "req": req, "ok": false, "error": errors.join("; ")})
            } else {
                json!({"type": "sim_result", "req": req, "ok": true, "message": format!("{ok} satélite(s) acompanhado(s)"), "errors": errors})
            }
        }
        "untrack" => {
            let id = vid(msg);
            if a.world.untrack(id) {
                a.save_satellites();
                a.dict_sig.clear();
                a.history.remove(&id);
                ok("satélite removido")
            } else {
                json!({"type": "sim_result", "req": req, "ok": false, "error": "satélite não encontrado"})
            }
        }
        "room_mode" => {
            a.room = msg.get("value").and_then(|x| x.as_bool()).unwrap_or(!a.room);
            a.sync_staffed();
            let m = if a.room { "Sala com posições ligada: cada tela entra numa posição" } else { "Sala com posições desligada: todas as telas comandam tudo" };
            a.world.event(None, jaci_sim::vehicle::Level::Info, m);
            json!({"type": "sim_result", "req": req, "ok": true, "message": m})
        }
        "record_start" => match a.start_recording(msg.get("value").and_then(|x| x.as_str())) {
            Ok(m) => json!({"type": "sim_result", "req": req, "ok": true, "message": m}),
            Err(e) => json!({"type": "sim_result", "req": req, "ok": false, "error": e}),
        },
        "record_stop" => match a.stop_recording() {
            Some(m) => json!({"type": "sim_result", "req": req, "ok": true, "message": m}),
            None => json!({"type": "sim_result", "req": req, "ok": false, "error": "não há gravação em andamento"}),
        },
        "recordings" => json!({"type": "recordings", "req": req, "list": recording::list(&a.recordings_dir), "dir": a.recordings_dir.display().to_string()}),
        "replay_start" => {
            if a.recorder.is_some() {
                return json!({"type": "sim_result", "req": req, "ok": false, "error": "pare a gravação antes de reproduzir"});
            }
            let name = msg.get("value").and_then(|x| x.as_str()).unwrap_or("");
            match recording::Replay::load(&recording::path_of(&a.recordings_dir, name)) {
                Ok(rp) => {
                    a.replay = Some(rp);
                    json!({"type": "reload", "req": req})
                }
                Err(e) => json!({"type": "sim_result", "req": req, "ok": false, "error": format!("{e:#}")}),
            }
        }
        "remove_debris" => {
            w.vehicles.retain(|v| !(v.kind == jaci_sim::vehicle::Kind::Debris && v.phase.is_final()));
            ok("detritos removidos")
        }
        _ => json!({"type": "sim_result", "req": req, "ok": false, "error": "ação desconhecida"}),
    }
}
