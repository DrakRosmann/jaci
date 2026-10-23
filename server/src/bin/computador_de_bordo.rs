//! Computador de bordo de demonstração: voa um VSB-30 por conta própria e manda
//! a navegação em pacotes CCSDS (APID 0x200) para o Jaci, como faria um
//! computador de bordo de verdade. Aceita telecomandos CCSDS de volta:
//! `count_start` (lança), `fts_terminate` (termina o voo) e `chute`.
//!
//!     jaci --ccsds-in 0.0.0.0:10015
//!     computador-de-bordo --server 127.0.0.1:10015
//!
//! Com `--xtce missoes/ccsds/cubesat.xml`, manda também os pacotes desse
//! dicionário (layout fixo): os pontos do voo pelo id, e os que o VSB-30 não
//! tem (tensão da bateria, temperatura, modo) sintetizados.

use clap::Parser;
use jaci_sim::{
    ccsds::{APID_NAV, Packet},
    commands::Command,
    launch::CountState,
    scenarios,
    vehicle::{ChuteState, Phase},
};
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;

#[derive(Parser, Debug)]
#[command(version, about = "Computador de bordo de demonstração (VSB-30) com telemetria CCSDS")]
struct Args {
    /// Endereço UDP do Jaci (--ccsds-in do servidor).
    #[arg(long, default_value = "127.0.0.1:10015")]
    server: String,
    /// Lança sozinho depois desta espera (s), sem telecomando.
    #[arg(long)]
    auto: Option<f64>,
    /// Pacotes por segundo.
    #[arg(long, default_value_t = 10.0)]
    rate: f64,
    /// Dicionário XTCE: manda também os pacotes dele.
    #[arg(long)]
    xtce: Option<std::path::PathBuf>,
}

/// Valor de um ponto do dicionário: do voo, ou inventado para a demonstração.
fn demo_value(id: &str, tm: &[(String, f64)], met: f64, phase: f64) -> f64 {
    if let Some((_, x)) = tm.iter().find(|(k, _)| k == id) {
        return *x;
    }
    match id {
        // Bateria que vai descarregando: aviso em ~2,5 min de voo, crítico em ~3,5 min.
        s if s.contains("batt_v") => 8.2 - 0.01 * met.max(0.0),
        s if s.contains("temp") => 20.0 + 0.05 * met.max(0.0),
        s if s.contains("mode") => phase,
        _ => 0.0,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let defs = match &args.xtce {
        Some(p) => {
            let d = jaci_sim::xtce::parse(&std::fs::read_to_string(p)?).map_err(|e| anyhow::anyhow!(e))?;
            for x in &d {
                println!("XTCE: APID 0x{:03X} = {} ({} pontos)", x.apid, x.container, x.fields.len());
            }
            d
        }
        None => vec![],
    };
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    let t0 = jaci_sim::time::from_calendar(2026, 1, 1, 12, 0, 0.0);
    let mut w = scenarios::build("sondagem", t0, 1).expect("cenário");
    w.weather.evolve = false;
    let vid = w.vehicles[0].id;
    println!("Computador de bordo pronto; telemetria para {}", args.server);
    let start = Instant::now();
    let mut seq: u16 = 0;
    let mut buf = vec![0u8; 4096];
    let period = Duration::from_secs_f64(1.0 / args.rate.clamp(0.5, 100.0));
    let mut last = Instant::now();
    loop {
        // Telecomandos.
        while let Ok(Ok((n, _))) = tokio::time::timeout(Duration::from_millis(1), sock.recv_from(&mut buf)).await {
            let Some(p) = Packet::decode(&buf[..n]) else { continue };
            if !p.telecommand {
                continue;
            }
            let Ok(cmd) = serde_json::from_slice::<Command>(&p.data) else { continue };
            println!("telecomando: {}", cmd.describe());
            match cmd.id.as_str() {
                "count_start" => {
                    let _ = w.execute(vid, &Command::new("count_start").with("t_minus", 30.0));
                }
                "fts_terminate" => {
                    if let Some(v) = w.vehicle_mut(vid) {
                        v.phase = Phase::Destroyed;
                    }
                }
                "chute" => {
                    if let Some(v) = w.vehicle_mut(vid) {
                        for c in &mut v.chutes {
                            if c.state == ChuteState::Stowed {
                                c.state = ChuteState::Deployed;
                                break;
                            }
                        }
                    }
                }
                _ => println!("  (não sei fazer isso)"),
            }
        }
        if let Some(a) = args.auto {
            if start.elapsed().as_secs_f64() > a && w.launch.state == CountState::Idle {
                let _ = w.execute(vid, &Command::new("count_start").with("t_minus", 10.0));
            }
        }
        // Sem esperas programadas: a demonstração pula a votação.
        if w.launch.state == CountState::Hold {
            w.launch.state = CountState::Counting;
        }
        w.launch.range_fouled = 0.0;
        let dt = last.elapsed().as_secs_f64();
        last = Instant::now();
        w.advance(dt);
        let v = w.vehicle(vid).expect("veículo");
        let d = &v.derived;
        let phase = match v.phase {
            Phase::Prelaunch => 0.0,
            Phase::Ascent => 1.0,
            Phase::Coast | Phase::Entry | Phase::Orbit | Phase::Escape => 2.0,
            Phase::Descent => 3.0,
            _ => 4.0,
        };
        let bus = v.systems.as_ref().map_or(0.0, |s| s.power.bus_v);
        let vals = [
            v.launch_time.map_or(0.0, |l| w.t - l),
            d.lat,
            d.lon,
            d.alt / 1e3,
            d.speed,
            d.vspeed,
            d.accel_g,
            d.q / 1e3,
            d.mach,
            bus,
            d.pitch_deg,
            phase,
        ];
        let pts: Vec<(u16, f32)> = vals.iter().enumerate().map(|(i, x)| (i as u16, *x as f32)).collect();
        seq = seq.wrapping_add(1);
        sock.send_to(&Packet::tm(APID_NAV, seq, w.t, &pts).encode(), &args.server).await?;
        if !defs.is_empty() {
            let mut tm = jaci_sim::telemetry::values(v, w.t);
            tm.extend([("nav.lat", d.lat), ("nav.lon", d.lon), ("nav.alt", d.alt / 1e3), ("nav.speed", d.speed), ("nav.vspeed", d.vspeed)].map(|(k, x)| (k.to_string(), x)));
            let met = v.launch_time.map_or(0.0, |l| w.t - l);
            for def in &defs {
                let vals: Vec<(String, f64)> = def.fields.iter().map(|f| (f.id.clone(), demo_value(&f.id, &tm, met, phase))).collect();
                let refs: Vec<(&str, f64)> = vals.iter().map(|(k, x)| (k.as_str(), *x)).collect();
                let packet = Packet { telecommand: false, apid: def.apid, seq, time: Some(w.t), data: def.encode(&refs) };
                sock.send_to(&packet.encode(), &args.server).await?;
            }
        }
        tokio::time::sleep(period).await;
    }
}
