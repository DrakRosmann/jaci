//! Pacotes CCSDS (Space Packet Protocol, CCSDS 133.0-B): telemetria e
//! telecomando entre o Jaci e um computador de bordo de verdade (ou outro
//! centro de controle), por UDP.
//!
//! Telemetria (tipo 0): cabeçalho secundário com o tempo em CUC (4 bytes de
//! segundos desde J2000 e 2 de fração) e, nos dados, pares (índice u16, valor
//! f32) big-endian. O índice é a posição do ponto no dicionário publicado em
//! `docs/protocolo.md` (APID 0x200, navegação) ou no dicionário do veículo
//! simulado (APID = 0x100 + id).
//!
//! Telecomando (tipo 1): os dados são o comando em JSON (UTF-8), o mesmo da
//! interface: `{"id": "abort", "args": {"mode": "les"}}`.

#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    pub telecommand: bool,
    pub apid: u16,
    pub seq: u16,
    /// Tempo do cabeçalho secundário (s desde J2000), quando há.
    pub time: Option<f64>,
    pub data: Vec<u8>,
}

pub const APID_NAV: u16 = 0x200;

/// Pontos do pacote de navegação de um computador de bordo externo (APID 0x200).
pub const NAV_POINTS: [(&str, &str, &str); 12] = [
    ("nav.met", "Tempo de missão", "s"),
    ("nav.lat", "Latitude", "°"),
    ("nav.lon", "Longitude", "°"),
    ("nav.alt", "Altitude", "km"),
    ("nav.speed", "Velocidade", "m/s"),
    ("nav.vspeed", "Velocidade vertical", "m/s"),
    ("nav.accel", "Aceleração", "g"),
    ("nav.q", "Pressão dinâmica", "kPa"),
    ("nav.mach", "Mach", ""),
    ("eps.bus", "Tensão do barramento", "V"),
    ("nav.pitch", "Arfagem", "°"),
    ("nav.phase", "Fase (0 solo, 1 subida, 2 balístico, 3 descida, 4 pousado)", ""),
];

impl Packet {
    pub fn tm(apid: u16, seq: u16, t: f64, points: &[(u16, f32)]) -> Self {
        let mut data = Vec::with_capacity(points.len() * 6);
        for (i, v) in points {
            data.extend_from_slice(&i.to_be_bytes());
            data.extend_from_slice(&v.to_be_bytes());
        }
        Packet { telecommand: false, apid, seq, time: Some(t), data }
    }

    pub fn tc(apid: u16, seq: u16, json: &str) -> Self {
        Packet { telecommand: true, apid, seq, time: None, data: json.as_bytes().to_vec() }
    }

    pub fn encode(&self) -> Vec<u8> {
        let sec = self.time.is_some();
        let mut body = Vec::new();
        if let Some(t) = self.time {
            let secs = t.floor().max(0.0) as u32;
            let frac = ((t - t.floor()) * 65_536.0) as u16;
            body.extend_from_slice(&secs.to_be_bytes());
            body.extend_from_slice(&frac.to_be_bytes());
        }
        body.extend_from_slice(&self.data);
        let w0: u16 = ((self.telecommand as u16) << 12) | ((sec as u16) << 11) | (self.apid & 0x7FF);
        let w1: u16 = (0b11 << 14) | (self.seq & 0x3FFF);
        let len = (body.len().max(1) - 1) as u16;
        let mut out = Vec::with_capacity(6 + body.len());
        out.extend_from_slice(&w0.to_be_bytes());
        out.extend_from_slice(&w1.to_be_bytes());
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&body);
        out
    }

    pub fn decode(buf: &[u8]) -> Option<Packet> {
        if buf.len() < 7 {
            return None;
        }
        let w0 = u16::from_be_bytes([buf[0], buf[1]]);
        let w1 = u16::from_be_bytes([buf[2], buf[3]]);
        let len = u16::from_be_bytes([buf[4], buf[5]]) as usize + 1;
        if (w0 >> 13) != 0 || buf.len() < 6 + len {
            return None;
        }
        let telecommand = (w0 >> 12) & 1 == 1;
        let sec = (w0 >> 11) & 1 == 1;
        let apid = w0 & 0x7FF;
        let seq = w1 & 0x3FFF;
        let mut body = &buf[6..6 + len];
        let mut time = None;
        if sec {
            if body.len() < 6 {
                return None;
            }
            let secs = u32::from_be_bytes([body[0], body[1], body[2], body[3]]) as f64;
            let frac = u16::from_be_bytes([body[4], body[5]]) as f64 / 65_536.0;
            time = Some(secs + frac);
            body = &body[6..];
        }
        Some(Packet { telecommand, apid, seq, time, data: body.to_vec() })
    }

    /// Pares (índice, valor) de um pacote de telemetria.
    pub fn points(&self) -> Vec<(u16, f32)> {
        self.data
            .chunks_exact(6)
            .map(|c| (u16::from_be_bytes([c[0], c[1]]), f32::from_be_bytes([c[2], c[3], c[4], c[5]])))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_e_volta() {
        let p = Packet::tm(APID_NAV, 1234, 845_000_123.5, &[(0, 12.5), (3, -7.25)]);
        let b = p.encode();
        // Versão 0, tipo TM, com cabeçalho secundário, APID 0x200.
        assert_eq!(b[0], 0x0A);
        assert_eq!(b[1], 0x00);
        let q = Packet::decode(&b).unwrap();
        assert_eq!(q.apid, APID_NAV);
        assert_eq!(q.seq, 1234);
        assert!((q.time.unwrap() - 845_000_123.5).abs() < 1e-4);
        assert_eq!(q.points(), vec![(0, 12.5), (3, -7.25)]);
        let tc = Packet::tc(0x101, 1, r#"{"id":"tx"}"#);
        let d = Packet::decode(&tc.encode()).unwrap();
        assert!(d.telecommand);
        assert_eq!(std::str::from_utf8(&d.data).unwrap(), r#"{"id":"tx"}"#);
    }
}
