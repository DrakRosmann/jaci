//! Gravação e reprodução de missões, para o debriefing.
//!
//! A gravação guarda os instantâneos que vão à interface, um por segundo de
//! relógio (com os eventos do intervalo), num arquivo JSON Lines comprimido
//! (`<nome>.jaci.gz`). A primeira linha traz a saudação (dicionários de
//! comandos, estações, corpos); as mudanças do dicionário de telemetria entram
//! no meio.
//!
//! Na reprodução a simulação fica parada e o servidor reenvia os quadros
//! gravados a todas as interfaces, que funcionam como ao vivo, só que sem
//! comandos. 1× reproduz no ritmo em que foi gravado (um quadro por segundo).

use flate2::{Compression, read::DeflateDecoder, read::MultiGzDecoder, write::DeflateEncoder, write::GzEncoder};
use jaci_sim::{
    bodies::Body,
    ground,
    math::Vec3,
    orbit::propagate_kepler,
    time::format_utc,
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

pub const EXT: &str = ".jaci.gz";

/// Nome seguro para arquivo: letras, números, espaço, - e _.
pub fn clean_name(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric() || "-_ ".contains(*c)).collect::<String>().trim().to_string()
}

pub fn path_of(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{}{EXT}", clean_name(name)))
}

// ── Gravação ──

pub struct Recorder {
    pub name: String,
    pub path: PathBuf,
    out: GzEncoder<BufWriter<File>>,
    pub frames: u64,
    /// Instante (simulação) do primeiro quadro.
    pub t0: Option<f64>,
    pub last_t: f64,
    /// Último evento já gravado (contador do mundo).
    pub event_seq: u64,
    since_flush: u32,
}

impl Recorder {
    /// Abre o arquivo e escreve o cabeçalho com a saudação.
    pub fn create(dir: &Path, name: &str, hello: &str, event_seq: u64) -> io::Result<Self> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "nome vazio"));
        }
        std::fs::create_dir_all(dir)?;
        let path = path_of(dir, &name);
        let file = File::create(&path)?;
        let mut out = GzEncoder::new(BufWriter::new(file), Compression::default());
        let hello: Value = serde_json::from_str(hello).unwrap_or(Value::Null);
        let header = json!({"type": "header", "format": 1, "name": name, "created": crate::now_j2000(), "hello": hello});
        writeln!(out, "{header}")?;
        out.flush()?;
        Ok(Recorder { name, path, out, frames: 0, t0: None, last_t: f64::NAN, event_seq, since_flush: 0 })
    }

    /// Grava um instantâneo (já com os eventos desde o quadro anterior).
    pub fn frame(&mut self, snapshot: &Value) -> io::Result<()> {
        let t = snapshot["t"].as_f64().unwrap_or(f64::NAN);
        let has_events = snapshot["events"].as_array().is_some_and(|e| !e.is_empty());
        // Simulação parada e nada novo: não repete o quadro.
        if t == self.last_t && !has_events {
            return Ok(());
        }
        self.t0.get_or_insert(t);
        self.last_t = t;
        self.frames += 1;
        self.line(snapshot)
    }

    /// Grava um dicionário de telemetria novo.
    pub fn dictionary(&mut self, dict: &str) -> io::Result<()> {
        let v: Value = serde_json::from_str(dict).unwrap_or(Value::Null);
        self.line(&v)
    }

    fn line(&mut self, v: &Value) -> io::Result<()> {
        writeln!(self.out, "{v}")?;
        // Descarrega de vez em quando: se o servidor cair, o arquivo é lido até ali.
        self.since_flush += 1;
        if self.since_flush >= 10 {
            self.since_flush = 0;
            self.out.flush()?;
        }
        Ok(())
    }

    pub fn bytes(&self) -> u64 {
        self.out.get_ref().get_ref().metadata().map_or(0, |m| m.len())
    }

    pub fn status(&self) -> Value {
        json!({
            "name": self.name,
            "frames": self.frames,
            "span": self.t0.map_or(0.0, |t0| self.last_t - t0),
            "bytes": self.bytes(),
        })
    }

    /// Fecha o arquivo.
    pub fn finish(self) -> io::Result<PathBuf> {
        let mut w = self.out.finish()?;
        w.flush()?;
        Ok(self.path)
    }
}

/// Gravações da pasta: nome, tamanho, data e o que diz o cabeçalho.
pub fn list(dir: &Path) -> Vec<Value> {
    let mut out: Vec<Value> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|e| {
            let file = e.file_name().to_str()?.to_string();
            let name = file.strip_suffix(EXT)?.to_string();
            let meta = e.metadata().ok()?;
            let header = read_header(&e.path()).unwrap_or(Value::Null);
            let hello = &header["hello"];
            let modified = meta.modified().ok().and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64() - 946_728_000.0);
            Some(json!({
                "name": name,
                "bytes": meta.len(),
                "modified": modified.map(format_utc),
                "created": header["created"].as_f64().map(format_utc),
                "mission": hello["mission"],
                "scenario": hello["scenario"],
            }))
        })
        .collect();
    out.sort_by(|a, b| b["modified"].as_str().cmp(&a["modified"].as_str()));
    out
}

fn read_header(path: &Path) -> Option<Value> {
    let r = BufReader::new(MultiGzDecoder::new(BufReader::new(File::open(path).ok()?)));
    let line = r.lines().next()?.ok()?;
    let v: Value = serde_json::from_str(&line).ok()?;
    (v["type"] == "header").then_some(v)
}

// ── Reprodução ──

struct Frame {
    t: f64,
    /// O instantâneo, comprimido sozinho (acesso direto a qualquer quadro).
    data: Vec<u8>,
}

/// Telemetria de um veículo ao longo da gravação, em colunas.
#[derive(Default)]
struct Columns {
    keys: Vec<String>,
    rows: Vec<(usize, f64, Vec<f32>)>,
}

pub struct Replay {
    pub name: String,
    hello: Value,
    frames: Vec<Frame>,
    events: Vec<(usize, Value)>,
    dictionaries: Vec<(usize, Value)>,
    tm: HashMap<u32, Columns>,
    markers: Vec<Value>,
    pub cursor: usize,
    pub playing: bool,
    pub speed: f64,
    acc: f64,
    /// Até que quadro os eventos já foram às interfaces.
    delivered: usize,
    dirty: bool,
    current: Option<(usize, Value)>,
}

/// Quantas amostras de telemetria vão para os gráficos ao conectar ou pular.
const HISTORY_ROWS: usize = 3_600;

impl Replay {
    pub fn load(path: &Path) -> anyhow::Result<Replay> {
        let name = path.file_name().and_then(|f| f.to_str()).and_then(|f| f.strip_suffix(EXT)).unwrap_or("gravação").to_string();
        let r = BufReader::new(MultiGzDecoder::new(BufReader::new(File::open(path)?)));
        let mut rp = Replay {
            name,
            hello: Value::Null,
            frames: Vec::new(),
            events: Vec::new(),
            dictionaries: Vec::new(),
            tm: HashMap::new(),
            markers: Vec::new(),
            cursor: 0,
            playing: false,
            speed: 1.0,
            acc: 0.0,
            delivered: 0,
            dirty: true,
            current: None,
        };
        for line in r.lines() {
            // Arquivo cortado (servidor caiu gravando): usa o que deu para ler.
            let Ok(line) = line else { break };
            let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
            match v["type"].as_str() {
                Some("header") => rp.hello = v["hello"].clone(),
                Some("dictionary") => rp.dictionaries.push((rp.frames.len(), v["dictionary"].clone())),
                Some("snapshot") => rp.add_frame(&line, &v)?,
                _ => {}
            }
        }
        anyhow::ensure!(!rp.frames.is_empty(), "a gravação não tem nenhum quadro");
        Ok(rp)
    }

    fn add_frame(&mut self, line: &str, v: &Value) -> anyhow::Result<()> {
        let idx = self.frames.len();
        let t = v["t"].as_f64().unwrap_or(0.0);
        for e in v["events"].as_array().into_iter().flatten() {
            if self.markers.len() < 3_000 {
                self.markers.push(json!([idx, e["level"], e["text"]]));
            }
            self.events.push((idx, e.clone()));
        }
        for veh in v["vehicles"].as_array().into_iter().flatten() {
            let (Some(id), Some(tm)) = (veh["id"].as_u64(), veh["tm"].as_object()) else { continue };
            let col = self.tm.entry(id as u32).or_default();
            let mut row = vec![f32::NAN; col.keys.len()];
            for (k, x) in tm {
                let i = match col.keys.iter().position(|kk| kk == k) {
                    Some(i) => i,
                    None => {
                        col.keys.push(k.clone());
                        row.push(f32::NAN);
                        col.keys.len() - 1
                    }
                };
                row[i] = x.as_f64().map_or(f32::NAN, |x| x as f32);
            }
            col.rows.push((idx, t, row));
        }
        let mut enc = DeflateEncoder::new(Vec::new(), Compression::fast());
        enc.write_all(line.as_bytes())?;
        self.frames.push(Frame { t, data: enc.finish()? });
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    fn frame(&mut self, idx: usize) -> Value {
        if let Some((i, v)) = &self.current {
            if *i == idx {
                return v.clone();
            }
        }
        let mut s = String::new();
        let _ = DeflateDecoder::new(&self.frames[idx].data[..]).read_to_string(&mut s);
        let v: Value = serde_json::from_str(&s).unwrap_or(Value::Null);
        self.current = Some((idx, v.clone()));
        v
    }

    /// O estado da reprodução, que vai em cada quadro.
    pub fn info(&self) -> Value {
        json!({
            "name": self.name,
            "frames": self.frames.len(),
            "index": self.cursor,
            "playing": self.playing,
            "speed": self.speed,
            "t0": self.frames[0].t,
            "t1": self.frames[self.frames.len() - 1].t,
            "t": self.frames[self.cursor].t,
        })
    }

    fn events_until(&self, idx: usize) -> Vec<Value> {
        let end = self.events.partition_point(|(i, _)| *i <= idx);
        self.events[end.saturating_sub(400)..end].iter().map(|(_, e)| e.clone()).collect()
    }

    fn dictionary_at(&self, idx: usize) -> Value {
        self.dictionaries.iter().rev().find(|(i, _)| *i <= idx).map_or_else(|| self.hello["dictionary"].clone(), |(_, d)| d.clone())
    }

    /// Saudação para quem conecta durante a reprodução.
    pub fn hello(&self) -> String {
        let mut h = self.hello.clone();
        h["events"] = json!(self.events_until(self.cursor));
        h["roles"] = crate::room::roles_json();
        h["dictionary"] = self.dictionary_at(self.cursor);
        h["replay"] = self.info();
        h["replay"]["markers"] = json!(self.markers);
        h["replay"]["times"] = json!(self.frame_times());
        h.to_string()
    }

    /// Instantes de alguns quadros, para a linha do tempo mostrar a hora.
    fn frame_times(&self) -> Vec<Value> {
        let n = self.frames.len();
        let step = (n / 200).max(1);
        (0..n).step_by(step).map(|i| json!([i, self.frames[i].t])).collect()
    }

    /// Telemetria até o quadro atual, no formato da mensagem "history".
    pub fn history(&self) -> String {
        let mut out = serde_json::Map::new();
        for (id, col) in &self.tm {
            let end = col.rows.partition_point(|(i, _, _)| *i <= self.cursor);
            let rows = &col.rows[end.saturating_sub(HISTORY_ROWS)..end];
            if rows.is_empty() {
                continue;
            }
            let ts: Vec<f64> = rows.iter().map(|(_, t, _)| *t).collect();
            let mut cols = serde_json::Map::new();
            for (k, key) in col.keys.iter().enumerate() {
                let c: Vec<Value> = rows.iter().map(|(_, _, r)| r.get(k).filter(|x| x.is_finite()).map_or(Value::Null, |x| json!(x))).collect();
                cols.insert(key.clone(), Value::Array(c));
            }
            out.insert(id.to_string(), json!({"t": ts, "points": cols}));
        }
        json!({"type": "history", "vehicles": out}).to_string()
    }

    /// Vai para um quadro: a mensagem que recomeça eventos e gráficos nas interfaces.
    pub fn seek(&mut self, idx: usize) -> String {
        self.cursor = idx.min(self.frames.len() - 1);
        self.acc = 0.0;
        self.delivered = self.cursor;
        self.dirty = true;
        json!({
            "type": "rewind",
            "events": self.events_until(self.cursor),
            "dictionary": self.dictionary_at(self.cursor),
            "history": serde_json::from_str::<Value>(&self.history()).unwrap_or(Value::Null)["vehicles"],
        })
        .to_string()
    }

    pub fn control(&mut self, playing: Option<bool>, speed: Option<f64>) {
        if let Some(s) = speed {
            self.speed = s.clamp(0.25, 600.0);
        }
        if let Some(p) = playing {
            // No fim, tocar de novo começa do início.
            if p && self.cursor + 1 >= self.frames.len() {
                self.cursor = 0;
                self.delivered = 0;
            }
            self.playing = p;
        }
        self.dirty = true;
    }

    /// O quadro atual sem eventos (já foram na saudação), para quem acabou de conectar.
    pub fn current(&mut self) -> String {
        let mut v = self.frame(self.cursor);
        v["events"] = json!([]);
        v["paused"] = json!(!self.playing);
        v["warp"] = json!(self.speed);
        v["replay"] = self.info();
        v["recording"] = Value::Null;
        v.to_string()
    }

    /// Avança o relógio da reprodução; devolve o quadro a enviar, se mudou algo.
    pub fn tick(&mut self, wall_dt: f64) -> Option<String> {
        if self.playing {
            self.acc += wall_dt * self.speed;
            while self.acc >= 1.0 && self.cursor + 1 < self.frames.len() {
                self.cursor += 1;
                self.acc -= 1.0;
            }
            if self.cursor + 1 >= self.frames.len() {
                self.playing = false;
                self.acc = 0.0;
            }
            self.dirty = true;
        }
        if !self.dirty {
            return None;
        }
        self.dirty = false;
        let idx = self.cursor;
        let mut v = self.frame(idx);
        // Eventos dos quadros pulados entram juntos; um quadro reenviado não repete os dele.
        let from = self.events.partition_point(|(i, _)| *i <= self.delivered);
        let to = self.events.partition_point(|(i, _)| *i <= idx);
        v["events"] = if idx > self.delivered { json!(self.events[from..to].iter().map(|(_, e)| e).collect::<Vec<_>>()) } else { json!([]) };
        self.delivered = idx.max(self.delivered);
        v["paused"] = json!(!self.playing);
        v["warp"] = json!(self.speed);
        v["replay"] = self.info();
        v["recording"] = Value::Null;
        Some(v.to_string())
    }

    /// Traço previsto de um veículo a partir do estado gravado no quadro atual.
    pub fn predict(&mut self, id: u32, span: f64, step: f64) -> (Option<String>, Vec<Value>) {
        let f = self.frame(self.cursor);
        let Some(v) = f["vehicles"].as_array().and_then(|vs| vs.iter().find(|v| v["id"].as_u64() == Some(id as u64))) else { return (None, vec![]) };
        let vec3 = |x: &Value| Some(Vec3::new(x[0].as_f64()?, x[1].as_f64()?, x[2].as_f64()?));
        let (Some(r), Some(vel), Some(central)) = (vec3(&v["r"]), vec3(&v["v"]), v["central"].as_str().and_then(Body::from_id)) else { return (None, vec![]) };
        let t = f["t"].as_f64().unwrap_or(0.0);
        let samples = if central == Body::Sun || r.norm() > 2e9 {
            let mut out = Vec::new();
            let mut tt = 0.0;
            while tt <= span {
                out.push((t + tt, propagate_kepler(r, vel, tt, central.mu()).0));
                tt += step;
            }
            out
        } else {
            ground::propagate_samples(central, r, vel, t, span, step)
        };
        let pts = samples
            .into_iter()
            .map(|(tt, r)| {
                let (la, lo, al) = central.to_geodetic(r, tt);
                json!([tt, la, lo, al / 1e3, r.x, r.y, r.z])
            })
            .collect();
        (Some(central.id().to_string()), pts)
    }

    /// Telemetria de um veículo em CSV (a gravação inteira).
    pub fn csv(&self, id: u32, dict: &Value) -> Option<String> {
        let col = self.tm.get(&id)?;
        Some(to_csv(&col.keys, col.rows.iter().map(|(_, t, r)| (*t, r.as_slice())), dict, id))
    }

    /// Veículos com telemetria na gravação.
    pub fn vehicles(&self) -> Vec<u32> {
        let mut v: Vec<u32> = self.tm.keys().copied().collect();
        v.sort();
        v
    }
}

/// Monta o CSV: hora UTC, segundos desde J2000 e uma coluna por ponto, com a unidade.
pub fn to_csv<'a>(keys: &[String], rows: impl Iterator<Item = (f64, &'a [f32])>, dict: &Value, vehicle: u32) -> String {
    let defs = dict[vehicle.to_string()].as_array();
    let unit = |k: &str| defs.and_then(|d| d.iter().find(|x| x["id"] == k)).and_then(|x| x["unit"].as_str()).filter(|u| !u.is_empty()).map(|u| format!(" [{u}]")).unwrap_or_default();
    let mut out = String::from("utc,t_j2000_s");
    for k in keys {
        out.push(',');
        out.push_str(&format!("{k}{}", unit(k)).replace(',', ";"));
    }
    out.push('\n');
    for (t, row) in rows {
        out.push_str(&format_utc(t).replace(" UTC", ""));
        out.push_str(&format!(",{t:.3}"));
        for k in 0..keys.len() {
            out.push(',');
            if let Some(x) = row.get(k).filter(|x| x.is_finite()) {
                out.push_str(&x.to_string());
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(t: f64, alt: f64, events: Value) -> Value {
        json!({"type": "snapshot", "t": t, "events": events, "vehicles": [{"id": 1, "name": "X", "r": [7.0e6, 0.0, 0.0], "v": [0.0, 7546.0, 0.0], "central": "earth", "tm": {"nav.alt": alt}}]})
    }

    #[test]
    fn grava_e_reproduz() {
        let dir = std::env::temp_dir().join(format!("jaci-grav-{}", std::process::id()));
        let hello = json!({"type": "hello", "mission": "Teste", "dictionary": {"1": [{"id": "nav.alt", "unit": "km"}]}}).to_string();
        let mut rec = Recorder::create(&dir, "../teste:1", &hello, 0).unwrap();
        assert_eq!(rec.name, "teste1");
        rec.frame(&snap(10.0, 100.0, json!([]))).unwrap();
        // Mesmo instante sem eventos: não grava de novo.
        rec.frame(&snap(10.0, 100.0, json!([]))).unwrap();
        rec.frame(&snap(11.0, 101.0, json!([{"t": 11.0, "level": "alarm", "text": "Alarme"}]))).unwrap();
        rec.frame(&snap(12.0, 102.0, json!([{"t": 12.0, "level": "info", "text": "Info"}]))).unwrap();
        assert_eq!(rec.frames, 3);
        let path = rec.finish().unwrap();
        assert_eq!(list(&dir)[0]["mission"], "Teste");

        let mut rp = Replay::load(&path).unwrap();
        assert_eq!(rp.len(), 3);
        assert_eq!(rp.markers.len(), 2);
        // Quem conecta no começo vê só o que aconteceu até ali, e já recebe o quadro.
        let h: Value = serde_json::from_str(&rp.hello()).unwrap();
        assert_eq!(h["events"].as_array().unwrap().len(), 0);
        let c: Value = serde_json::from_str(&rp.current()).unwrap();
        assert_eq!(c["t"], 10.0);
        assert_eq!(c["replay"]["index"], 0);
        // Tocando a 10×, um passo pula os dois quadros e leva os dois eventos.
        rp.control(Some(true), Some(10.0));
        let f: Value = serde_json::from_str(&rp.tick(0.2).unwrap()).unwrap();
        assert_eq!(f["t"], 12.0);
        assert_eq!(f["events"].as_array().unwrap().len(), 2);
        assert_eq!(f["replay"]["playing"], false);
        // Reenviado, o quadro não repete os eventos.
        rp.control(None, Some(2.0));
        let f: Value = serde_json::from_str(&rp.tick(0.2).unwrap()).unwrap();
        assert_eq!(f["events"].as_array().unwrap().len(), 0);
        // Voltar ao meio: eventos e gráfico até ali.
        let w: Value = serde_json::from_str(&rp.seek(1)).unwrap();
        assert_eq!(w["events"].as_array().unwrap().len(), 1);
        assert_eq!(w["history"]["1"]["t"].as_array().unwrap().len(), 2);
        // Traço previsto a partir do estado gravado.
        let (central, pts) = rp.predict(1, 600.0, 60.0);
        assert_eq!(central.as_deref(), Some("earth"));
        assert!(pts.len() >= 10);
        let csv = rp.csv(1, &serde_json::from_str::<Value>(&hello).unwrap()["dictionary"]).unwrap();
        assert!(csv.starts_with("utc,t_j2000_s,nav.alt [km]\n"));
        assert_eq!(csv.lines().count(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn arquivo_cortado_ainda_le() {
        let dir = std::env::temp_dir().join(format!("jaci-grav-cortado-{}", std::process::id()));
        let mut rec = Recorder::create(&dir, "cortado", "{}", 0).unwrap();
        for i in 0..25 {
            rec.frame(&snap(i as f64, 100.0, json!([]))).unwrap();
        }
        // Sem finish(): como se o servidor tivesse caído (só o que foi descarregado).
        rec.out.flush().unwrap();
        let path = rec.path.clone();
        std::mem::forget(rec);
        let rp = Replay::load(&path).unwrap();
        assert!(rp.len() >= 20, "{}", rp.len());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
