//! Satélites reais acompanhados pelos elementos NORAD (TLE, do CelesTrak ou de
//! outra fonte), propagados com o SGP4/SDP4.
//!
//! O SGP4 dá posição e velocidade no referencial TEME, que é o equatorial do
//! Jaci girado pela Terra pelo tempo sideral (GMST): os vetores entram direto,
//! e latitude, longitude e passagens saem como as dos veículos simulados.

use crate::{
    bodies::Body,
    ground::{Pass, Station, passes_from_samples},
    math::Vec3,
    orbit::{Elements, elements_from_state},
    telemetry::TmDef,
    time::DAY,
    vehicle::VehicleId,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tracked {
    pub id: VehicleId,
    pub name: String,
    pub norad: u64,
    pub line1: String,
    pub line2: String,
    /// O modelo do SGP4, montado na primeira vez que é usado.
    #[serde(skip)]
    model: OnceLock<Option<Model>>,
}

#[derive(Clone, Debug)]
struct Model {
    constants: sgp4::Constants,
    /// Época do TLE no relógio da simulação (s desde J2000, UTC).
    epoch: f64,
}

/// Estado num instante, com o que se mostra de um satélite.
pub struct TrackedState {
    pub r: Vec3,
    pub v: Vec3,
    pub lat: f64,
    pub lon: f64,
    /// Altitude (m) sobre o elipsoide.
    pub alt: f64,
    pub elements: Elements,
}

impl Tracked {
    /// Monta a partir das duas linhas do TLE (o nome é opcional: sem ele, o da linha 0 ou o número).
    pub fn from_tle(id: VehicleId, name: Option<&str>, line1: &str, line2: &str) -> Result<Tracked, String> {
        let (l1, l2) = (line1.trim_end(), line2.trim_end());
        let el = sgp4::Elements::from_tle(name.map(|s| s.trim().to_string()), l1.as_bytes(), l2.as_bytes()).map_err(|e| format!("TLE inválido: {e}"))?;
        let norad = el.norad_id;
        let name = el.object_name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| format!("NORAD {norad}"));
        let t = Tracked { id, name, norad, line1: l1.to_string(), line2: l2.to_string(), model: OnceLock::new() };
        t.model().ok_or_else(|| format!("{}: o SGP4 não aceitou estes elementos", t.name))?;
        Ok(t)
    }

    fn model(&self) -> Option<&Model> {
        self.model
            .get_or_init(|| {
                let el = sgp4::Elements::from_tle(None, self.line1.as_bytes(), self.line2.as_bytes()).ok()?;
                let constants = sgp4::Constants::from_elements(&el).ok()?;
                Some(Model { constants, epoch: el.epoch() * 365.25 * DAY })
            })
            .as_ref()
    }

    /// Época do TLE (relógio da simulação).
    pub fn epoch(&self) -> Option<f64> {
        self.model().map(|m| m.epoch)
    }

    /// Posição e velocidade (m, m/s) no inercial da Terra.
    pub fn rv(&self, t: f64) -> Option<(Vec3, Vec3)> {
        let m = self.model()?;
        let p = m.constants.propagate(sgp4::MinutesSinceEpoch((t - m.epoch) / 60.0)).ok()?;
        let r = Vec3::new(p.position[0], p.position[1], p.position[2]) * 1e3;
        let v = Vec3::new(p.velocity[0], p.velocity[1], p.velocity[2]) * 1e3;
        Some((r, v))
    }

    pub fn state(&self, t: f64) -> Option<TrackedState> {
        let (r, v) = self.rv(t)?;
        let (lat, lon, alt) = Body::Earth.to_geodetic(r, t);
        Some(TrackedState { r, v, lat, lon, alt, elements: elements_from_state(r, v, Body::Earth.mu()) })
    }

    /// Idade dos elementos (dias desde a época do TLE).
    pub fn age_days(&self, t: f64) -> f64 {
        self.epoch().map_or(f64::NAN, |e| (t - e) / DAY)
    }

    /// Pontos de telemetria calculados (mesmos ids dos veículos simulados).
    pub fn telemetry(&self, st: &TrackedState, t: f64) -> Vec<(String, f64)> {
        let el = &st.elements;
        let re = Body::Earth.radius();
        let mu = Body::Earth.mu();
        let r = st.r.norm();
        [
            ("nav.alt", st.alt / 1e3),
            ("nav.lat", st.lat),
            ("nav.lon", st.lon),
            ("nav.speed", st.v.norm()),
            ("nav.vspeed", st.v.dot(st.r) / r),
            ("orb.pe", (el.periapsis() - re) / 1e3),
            ("orb.ap", (el.apoapsis() - re) / 1e3),
            ("orb.sma", el.a / 1e3),
            ("orb.ecc", el.e),
            ("orb.inc", el.i.to_degrees()),
            ("orb.raan", el.raan.to_degrees()),
            ("orb.argp", el.argp.to_degrees()),
            ("orb.ta", el.nu.to_degrees()),
            ("orb.period", el.period(mu) / 60.0),
            ("tle.age", self.age_days(t)),
        ]
        .into_iter()
        .map(|(k, x)| (k.to_string(), x))
        .collect()
    }

    /// Posições a cada `step` segundos por `span` segundos.
    pub fn samples(&self, t0: f64, span: f64, step: f64) -> Vec<(f64, Vec3)> {
        let step = step.max(1.0);
        let n = (span / step).ceil().min(100_000.0) as usize;
        (0..=n).filter_map(|i| {
            let t = t0 + i as f64 * step;
            self.rv(t).map(|(r, _)| (t, r))
        })
        .collect()
    }

    /// Passagens sobre as estações.
    pub fn passes(&self, stations: &[Station], t0: f64, span: f64) -> Vec<Pass> {
        passes_from_samples(&self.samples(t0, span, 20.0), stations, t0, span)
    }
}

/// Dicionário de telemetria de um satélite real.
pub fn tm_defs() -> Vec<TmDef> {
    let d = |id: &str, name: &str, unit: &'static str, group: &'static str| TmDef { id: id.into(), name: name.into(), unit, group, yellow: None, red: None };
    let mut age = d("tle.age", "Idade do TLE", "dias", "Elementos");
    // Elementos velhos erram cada vez mais (km por dia em órbita baixa).
    age.yellow = Some((-3.0, 7.0));
    age.red = Some((-30.0, 30.0));
    vec![
        d("nav.alt", "Altitude", "km", "Navegação"),
        d("nav.lat", "Latitude", "°", "Navegação"),
        d("nav.lon", "Longitude", "°", "Navegação"),
        d("nav.speed", "Velocidade inercial", "m/s", "Navegação"),
        d("nav.vspeed", "Velocidade vertical", "m/s", "Navegação"),
        d("orb.pe", "Periastro", "km", "Órbita"),
        d("orb.ap", "Apoastro", "km", "Órbita"),
        d("orb.sma", "Semieixo maior", "km", "Órbita"),
        d("orb.ecc", "Excentricidade", "", "Órbita"),
        d("orb.inc", "Inclinação", "°", "Órbita"),
        d("orb.raan", "Nodo ascendente", "°", "Órbita"),
        d("orb.argp", "Argumento do periastro", "°", "Órbita"),
        d("orb.ta", "Anomalia verdadeira", "°", "Órbita"),
        d("orb.period", "Período", "min", "Órbita"),
        age,
    ]
}

/// A lista em texto, três linhas por satélite (o formato do CelesTrak).
pub fn to_text(list: &[Tracked]) -> String {
    list.iter().map(|s| format!("{}\n{}\n{}\n", s.name, s.line1, s.line2)).collect()
}

/// Lê um texto com TLEs: pares de linhas "1 …" e "2 …", cada um com o nome na
/// linha de antes, se houver (o formato do CelesTrak). Devolve (nome, linha 1, linha 2).
pub fn parse(text: &str) -> Vec<(Option<String>, String, String)> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches(['\r', ' '])).filter(|l| !l.trim().is_empty()).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < lines.len() {
        let (a, b) = (lines[i].trim_start(), lines[i + 1].trim_start());
        if a.starts_with("1 ") && b.starts_with("2 ") {
            let name = (i > 0 && !lines[i - 1].trim_start().starts_with("1 ") && !lines[i - 1].trim_start().starts_with("2 ")).then(|| lines[i - 1].trim().trim_start_matches("0 ").to_string());
            out.push((name, a.to_string(), b.to_string()));
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::from_calendar;

    // ISS em 2026-10-21 (CelesTrak).
    const ISS: &str = "ISS (ZARYA)
1 25544U 98067A   26294.04791053  .00009575  00000+0  18188-3 0  9996
2 25544  51.6327  37.2623 0006759 289.5321  70.4938 15.48967427591263";

    #[test]
    fn le_tles() {
        let two = format!("{ISS}\n\n1 25544U 98067A   26294.04791053  .00009575  00000+0  18188-3 0  9996\n2 25544  51.6327  37.2623 0006759 289.5321  70.4938 15.48967427591263\n");
        let p = parse(&two);
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].0.as_deref(), Some("ISS (ZARYA)"));
        assert_eq!(p[1].0, None);
        assert!(parse("lixo\n1 abc").is_empty());
        // Ida e volta pelo texto.
        let s = Tracked::from_tle(1, p[0].0.as_deref(), &p[0].1, &p[0].2).unwrap();
        let back = parse(&to_text(&[s]));
        assert_eq!(back[0].0.as_deref(), Some("ISS (ZARYA)"));
        assert_eq!(back[0].2, p[0].2);
    }

    #[test]
    fn iss_em_orbita() {
        let p = parse(ISS);
        let s = Tracked::from_tle(1, p[0].0.as_deref(), &p[0].1, &p[0].2).unwrap();
        assert_eq!(s.norad, 25544);
        assert_eq!(s.name, "ISS (ZARYA)");
        // Época: dia 294,0479 de 2026.
        let ep = from_calendar(2026, 1, 1, 0, 0, 0.0) + 293.047_910_53 * DAY;
        assert!((s.epoch().unwrap() - ep).abs() < 1e-3, "{}", s.epoch().unwrap() - ep);
        let st = s.state(ep + 3600.0).unwrap();
        // ~420 km, 51,6°, ~7,66 km/s.
        assert!((st.alt / 1e3 - 420.0).abs() < 25.0, "{}", st.alt / 1e3);
        assert!((st.elements.i.to_degrees() - 51.63).abs() < 0.2);
        assert!((st.v.norm() - 7_660.0).abs() < 60.0, "{}", st.v.norm());
        assert!(st.lat.abs() <= 51.7);
        // Uma volta em ~93 min.
        let period = st.elements.period(Body::Earth.mu()) / 60.0;
        assert!((period - 92.9).abs() < 0.5, "{period}");
    }

    #[test]
    fn passagens_sobre_alcantara() {
        let p = parse(ISS);
        let s = Tracked::from_tle(1, None, &p[0].1, &p[0].2).unwrap();
        let st = Station { id: "alc".into(), name: "Alcântara".into(), lat: -2.3167, lon: -44.3667, alt: 50.0, dish_m: 5.0, tsys: 150.0, min_el: 5.0, deep_space: false, enabled: true };
        let t0 = s.epoch().unwrap();
        let ps = s.passes(&[st], t0, DAY);
        // A ISS passa sobre o equador várias vezes por dia: algumas visíveis de Alcântara.
        assert!(!ps.is_empty() && ps.len() < 10, "{}", ps.len());
        for p in &ps {
            assert!(p.los > p.aos && p.los - p.aos < 15.0 * 60.0);
            assert!(p.max_el >= 5.0);
        }
    }

    #[test]
    fn tle_invalido() {
        assert!(Tracked::from_tle(1, None, "1 xxx", "2 yyy").is_err());
    }
}
