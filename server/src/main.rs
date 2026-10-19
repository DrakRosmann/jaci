//! jaci: o servidor do centro de controle. Roda a simulação, fala WebSocket
//! com a interface, serve os arquivos dela e, se pedido, troca pacotes CCSDS
//! por UDP com computadores de bordo de verdade.

mod snapshot;

use anyhow::Context;
use axum::{
    Router,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
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
    /// Cenário inicial (veja --list).
    #[arg(long, default_value = "sondagem")]
    scenario: String,
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
    let world = scenarios::build(&args.scenario, t0, args.seed).with_context(|| format!("cenário desconhecido: {}", args.scenario))?;
    tracing::info!("cenário {} em {}", world.mission, format_utc(world.t));
    let app = Arc::new(Mutex::new(App {
        world,
        states_dir: states_dir(&args),
        sent_events: 0,
        dict_sig: String::new(),
        tc_routes: HashMap::new(),
        tc_seq: 0,
        tc_socket: None,
        history: HashMap::new(),
    }));
    {
        let mut a = app.lock().unwrap();
        a.sent_events = a.world.event_seq;
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
            loop {
                tick.tick().await;
                let now = Instant::now();
                let dt = (now - last).as_secs_f64().min(0.25);
                last = now;
                let mut a = app.lock().unwrap();
                a.world.advance(dt);
                if now - last_snap >= Duration::from_millis(200) {
                    last_snap = now;
                    if let Some(dict) = snapshot::dictionary_if_changed(&mut a) {
                        let _ = tx.send(dict.into());
                    }
                    let s = snapshot::snapshot(&mut a);
                    let _ = tx.send(s.into());
                }
                if now - last_hist >= Duration::from_millis(500) {
                    last_hist = now;
                    snapshot::record_history(&mut a);
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
    let router = Router::new().route("/ws", get(ws_handler)).fallback_service(static_files).with_state(ctx);
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

async fn client(socket: WebSocket, ctx: Ctx) {
    let (mut sink, mut stream) = socket.split();
    let (hello, history) = {
        let mut a = ctx.app.lock().unwrap();
        (snapshot::hello(&mut a), snapshot::history(&a))
    };
    if sink.send(Message::Text(hello.into())).await.is_err() {
        return;
    }
    if sink.send(Message::Text(history.into())).await.is_err() {
        return;
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
        let reply = handle(&ctx, &v);
        if let Some(r) = reply {
            if reply_tx.send(r.to_string()).is_err() {
                break;
            }
        }
    }
    writer.abort();
}

fn vid(v: &Value) -> u32 {
    v.get("vehicle").and_then(|x| x.as_u64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0) as u32
}

/// Trata uma mensagem da interface; devolve a resposta (só para quem pediu).
fn handle(ctx: &Ctx, msg: &Value) -> Option<Value> {
    let kind = msg.get("type")?.as_str()?;
    let req = msg.get("req").cloned().unwrap_or(Value::Null);
    let mut a = ctx.app.lock().unwrap();
    let out = match kind {
        "command" => {
            let id = vid(msg);
            let cmd: Command = serde_json::from_value(msg.get("cmd")?.clone()).ok()?;
            let armed = msg.get("armed").and_then(|x| x.as_bool()).unwrap_or(false);
            let at = msg.get("at").and_then(|x| x.as_str()).and_then(parse_utc).or_else(|| msg.get("in").and_then(|x| x.as_f64()).map(|d| a.world.t + d));
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
        "sim" => sim(&mut a, msg, &req),
        "plan" => {
            let id = vid(msg);
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
            let pts: Vec<Value> = a.world.predict(id, span, step).into_iter().map(|(t, la, lo, al, r)| json!([t, la, lo, al / 1e3, r.x, r.y, r.z])).collect();
            let central = a.world.vehicle(id).map(|v| v.central.id());
            json!({"type": "predict", "req": req, "vehicle": id, "central": central, "points": pts})
        }
        "passes" => {
            let id = vid(msg);
            let span = msg.get("span").and_then(|x| x.as_f64()).unwrap_or(86_400.0).min(3.0 * 86_400.0);
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
        "dictionary" => snapshot::dictionary(&a),
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
                    *w = nw;
                    a.sent_events = a.world.event_seq;
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
                    a.world = nw;
                    a.sent_events = a.world.event_seq;
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
            match w.inject(id, f, arg.as_deref()) {
                Ok(m) => ok(&m),
                Err(e) => json!({"type": "sim_result", "req": req, "ok": false, "error": e}),
            }
        }
        "clear_faults" => {
            w.clear_faults(vid(msg));
            ok("falhas removidas")
        }
        "ack" => {
            let which = msg.get("value");
            for al in &mut w.alarms {
                if which.and_then(|x| x.as_u64()).is_none_or(|n| n == al.id) {
                    al.acked = true;
                }
            }
            ok("reconhecido")
        }
        "station" => {
            let id = msg.get("id").and_then(|x| x.as_str()).unwrap_or("");
            let en = msg.get("enabled").and_then(|x| x.as_bool()).unwrap_or(true);
            if let Some(s) = w.stations.iter_mut().find(|s| s.id == id) {
                s.enabled = en;
            }
            ok("ok")
        }
        "remove_debris" => {
            w.vehicles.retain(|v| !(v.kind == jaci_sim::vehicle::Kind::Debris && v.phase.is_final()));
            ok("detritos removidos")
        }
        _ => json!({"type": "sim_result", "req": req, "ok": false, "error": "ação desconhecida"}),
    }
}
