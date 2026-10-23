//! Estações de solo: visada, orçamento de enlace e previsão de passagens.

use crate::bodies::Body;
use crate::math::{C, Vec3};
use crate::subsystems::{AttMode, Comms, LinkStatus};
use crate::vehicle::{Vehicle, gravity};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
    /// Diâmetro da antena (m).
    pub dish_m: f64,
    /// Temperatura de ruído do sistema (K).
    pub tsys: f64,
    /// Elevação mínima (graus).
    pub min_el: f64,
    /// Rede de espaço profundo (só fala com sondas e naves longe).
    pub deep_space: bool,
    pub enabled: bool,
}

pub fn default_stations() -> Vec<Station> {
    let s = |id: &str, name: &str, lat: f64, lon: f64, dish: f64, tsys: f64, deep: bool| Station {
        id: id.into(),
        name: name.into(),
        lat,
        lon,
        alt: 50.0,
        dish_m: dish,
        tsys,
        min_el: if deep { 10.0 } else { 5.0 },
        deep_space: deep,
        enabled: true,
    };
    vec![
        s("cla", "Alcântara", -2.336, -44.400, 7.3, 180.0, false),
        s("clbi", "Natal (Barreira do Inferno)", -5.924, -35.162, 6.0, 180.0, false),
        s("cba", "Cuiabá", -15.555, -56.070, 11.0, 150.0, false),
        s("kou", "Kourou", 5.252, -52.805, 10.0, 160.0, false),
        s("ccafs", "Cabo Canaveral", 28.48, -80.57, 9.0, 160.0, false),
        s("wff", "Wallops", 37.94, -75.46, 11.0, 150.0, false),
        s("sval", "Svalbard", 78.23, 15.39, 13.0, 140.0, false),
        s("kir", "Kiruna", 67.86, 20.96, 13.0, 140.0, false),
        s("hbk", "Hartebeesthoek", -25.89, 27.69, 12.0, 150.0, false),
        s("sant", "Santiago", -33.15, -70.67, 9.0, 160.0, false),
        s("haw", "Havaí", 19.01, -155.66, 11.0, 150.0, false),
        s("guam", "Guam", 13.62, 144.86, 11.0, 150.0, false),
        s("perth", "Perth", -31.80, 115.89, 13.0, 150.0, false),
        s("mcm", "McMurdo", -77.84, 166.67, 10.0, 160.0, false),
        s("gds", "Goldstone (espaço profundo)", 35.43, -116.89, 70.0, 25.0, true),
        s("mad", "Madri (espaço profundo)", 40.43, -4.25, 70.0, 25.0, true),
        s("cbr", "Camberra (espaço profundo)", -35.40, 148.98, 70.0, 25.0, true),
    ]
}

/// Azimute, elevação (graus) e distância (m) de um ponto (relativo ao centro da
/// Terra, inercial) visto da estação.
pub fn look_angles(st: &Station, r_eci: Vec3, t: f64) -> (f64, f64, f64) {
    let site = Body::Earth.from_geodetic(st.lat, st.lon, st.alt, t);
    let d = r_eci - site;
    let up = site.unit();
    let east = Vec3::Z.cross(up).unit();
    let north = up.cross(east);
    let range = d.norm();
    let el = (d.dot(up) / range).clamp(-1.0, 1.0).asin().to_degrees();
    let az = d.dot(east).atan2(d.dot(north)).to_degrees().rem_euclid(360.0);
    (az, el, range)
}

/// Posição do veículo relativa ao centro da Terra (inercial), qualquer que seja
/// o corpo central.
pub fn earth_relative(v: &Vehicle, t: f64) -> Vec3 {
    if v.central == Body::Earth {
        return v.r;
    }
    let (rc, _) = v.central.state_relative_to(Body::Earth, t);
    rc + v.r
}

/// O corpo central (que não a Terra) tapa a linha de visada?
fn occulted(v: &Vehicle, site_rel_central: Vec3) -> bool {
    if v.central == Body::Earth {
        return false;
    }
    // Segmento do veículo (v.r) até a estação; o corpo está na origem.
    let a = v.r;
    let b = site_rel_central;
    let ab = b - a;
    let tq = (-a.dot(ab) / ab.norm2()).clamp(0.0, 1.0);
    let closest = a + ab * tq;
    closest.norm() < v.central.radius()
}

/// Ganho de antena parabólica (dBi).
pub fn dish_gain_dbi(diameter: f64, freq_hz: f64) -> f64 {
    let lambda = C / freq_hz;
    let g = 0.6 * (std::f64::consts::PI * diameter / lambda).powi(2);
    10.0 * g.max(1.0).log10()
}

/// Perda no espaço livre (dB).
pub fn fspl_db(range: f64, freq_hz: f64) -> f64 {
    20.0 * (4.0 * std::f64::consts::PI * range * freq_hz / C).log10()
}

/// Margem do enlace de descida (dB) para uma taxa: Eb/N0 menos 2,5 dB exigidos
/// (código turbo) e 3 dB de perdas diversas.
pub fn link_margin(c: &Comms, use_hga: bool, st: &Station, range: f64, rate: f64) -> f64 {
    let p_dbw = 10.0 * c.tx_power_w.max(1e-6).log10();
    let g_tx = if use_hga { c.hga_gain_dbi } else { c.lga_gain_dbi };
    let eirp = p_dbw + g_tx - 1.0;
    let g_rx = dish_gain_dbi(st.dish_m, c.freq_hz);
    let gt = g_rx - 10.0 * st.tsys.log10();
    let cn0 = eirp - fspl_db(range, c.freq_hz) - 0.5 + gt + 228.6;
    let ebn0 = cn0 - 10.0 * rate.max(1.0).log10();
    ebn0 - 2.5 - 3.0
}

/// A antena de alto ganho está apontada para a Terra?
fn hga_on_earth(v: &Vehicle) -> bool {
    let Some(sys) = &v.systems else { return false };
    sys.comms.hga_gain_dbi > 0.0
        && sys.comms.hga_deployed
        && matches!(sys.adcs.mode, AttMode::EarthPoint)
        && sys.adcs.error_deg.is_finite()
        && sys.adcs.error_deg < sys.comms.hga_beam_deg / 2.0
}

/// Melhor enlace disponível agora (estação, relé ou nenhum).
pub fn best_link(v: &Vehicle, stations: &[Station], t: f64) -> Option<LinkStatus> {
    let sys = v.systems.as_ref()?;
    let c = &sys.comms;
    if !c.tx_on || c.failed || c.blackout || sys.power.brownout {
        return None;
    }
    let r = earth_relative(v, t);
    let dist = r.norm();
    let use_hga = hga_on_earth(v);
    // Relé: cobertura quase total para quem tem, até 40 000 km da Terra.
    if c.relay && v.central == Body::Earth && dist < 40_000e3 {
        let rate = c.rates.iter().copied().fold(0.0, f64::max);
        return Some(LinkStatus {
            station: "Rede de relé (GEO)".into(),
            elevation_deg: 90.0,
            azimuth_deg: 0.0,
            range_m: 38_000e3,
            margin_db: 6.0,
            rate_bps: rate,
            owlt: 38_000e3 / C,
            via_hga: false,
            via_relay: true,
        });
    }
    let mut best: Option<LinkStatus> = None;
    for st in stations.iter().filter(|s| s.enabled) {
        // Estação de espaço profundo só para quem está longe; as outras, perto.
        if st.deep_space != (dist > 2_000_000e3) && !(st.deep_space && v.central != Body::Earth) {
            continue;
        }
        let (az, el, range) = look_angles(st, r, t);
        if el < st.min_el {
            continue;
        }
        if v.central != Body::Earth {
            let site = Body::Earth.from_geodetic(st.lat, st.lon, st.alt, t);
            let (rc, _) = v.central.state_relative_to(Body::Earth, t);
            if occulted(v, site - rc) {
                continue;
            }
        }
        let rate = if c.auto_rate {
            c.rates
                .iter()
                .copied()
                .filter(|&rb| link_margin(c, use_hga, st, range, rb) >= 3.0)
                .fold(f64::NAN, f64::max)
        } else {
            c.rate
        };
        let rate = if rate.is_nan() { c.rates.first().copied().unwrap_or(1.0) } else { rate };
        let margin = link_margin(c, use_hga, st, range, rate);
        if margin < 0.0 {
            continue;
        }
        let better = best.as_ref().is_none_or(|b| rate > b.rate_bps || (rate == b.rate_bps && margin > b.margin_db));
        if better {
            best = Some(LinkStatus {
                station: st.name.clone(),
                elevation_deg: el,
                azimuth_deg: az,
                range_m: range,
                margin_db: margin,
                rate_bps: rate,
                owlt: range / C,
                via_hga: use_hga,
                via_relay: false,
            });
        }
    }
    best
}

#[derive(Clone, Debug, Serialize)]
pub struct Pass {
    pub station: String,
    pub aos: f64,
    pub los: f64,
    pub max_el: f64,
    pub t_max: f64,
}

/// Propaga uma órbita da Terra (gravidade com J2, sem arrasto) e devolve os
/// pontos (t, r) a cada `step` segundos.
pub fn propagate_samples(central: Body, r0: Vec3, v0: Vec3, t0: f64, span: f64, step: f64) -> Vec<(f64, Vec3)> {
    let mut out = Vec::new();
    let (mut r, mut v) = (r0, v0);
    let mut t = 0.0;
    let h_max = (step / 4.0).min(30.0);
    out.push((t0, r));
    let mut next = step;
    while t < span {
        let h = h_max.min(next - t).max(1e-3);
        // RK4 só com gravidade.
        let a = |r: Vec3| gravity(central, r);
        let k1v = a(r);
        let k1r = v;
        let k2v = a(r + k1r * (h / 2.0));
        let k2r = v + k1v * (h / 2.0);
        let k3v = a(r + k2r * (h / 2.0));
        let k3r = v + k2v * (h / 2.0);
        let k4v = a(r + k3r * h);
        let k4r = v + k3v * h;
        r += (k1r + k2r * 2.0 + k3r * 2.0 + k4r) * (h / 6.0);
        v += (k1v + k2v * 2.0 + k3v * 2.0 + k4v) * (h / 6.0);
        t += h;
        if t >= next - 1e-9 {
            out.push((t0 + t, r));
            next += step;
        }
        if central.altitude(r) < 0.0 {
            break;
        }
    }
    out
}

/// Passagens sobre as estações nas próximas `span` segundos.
pub fn passes(v: &Vehicle, stations: &[Station], t0: f64, span: f64) -> Vec<Pass> {
    if v.central != Body::Earth {
        return vec![];
    }
    let samples = propagate_samples(v.central, v.r, v.v, t0, span, 20.0);
    passes_from_samples(&samples, stations, t0, span)
}

/// Passagens a partir de posições (t, r inercial na Terra) já calculadas.
pub fn passes_from_samples(samples: &[(f64, Vec3)], stations: &[Station], t0: f64, span: f64) -> Vec<Pass> {
    let mut out = Vec::new();
    for st in stations.iter().filter(|s| s.enabled && !s.deep_space) {
        let mut in_pass: Option<(f64, f64, f64)> = None;
        let mut prev: Option<(f64, f64)> = None;
        for &(t, r) in samples {
            let (_, el, _) = look_angles(st, r, t);
            let up = el >= st.min_el;
            match (&mut in_pass, up) {
                (None, true) => {
                    // Interpola o instante de subida.
                    let aos = match prev {
                        Some((tp, ep)) if ep < st.min_el => tp + (t - tp) * (st.min_el - ep) / (el - ep),
                        _ => t,
                    };
                    in_pass = Some((aos, el, t));
                }
                (Some(p), true) => {
                    if el > p.1 {
                        p.1 = el;
                        p.2 = t;
                    }
                }
                (Some(p), false) => {
                    let (tp, ep) = prev.unwrap_or((t, el));
                    let los = tp + (t - tp) * (ep - st.min_el) / (ep - el).max(1e-9);
                    out.push(Pass { station: st.name.clone(), aos: p.0, los, max_el: p.1, t_max: p.2 });
                    in_pass = None;
                }
                (None, false) => {}
            }
            prev = Some((t, el));
        }
        if let Some(p) = in_pass {
            out.push(Pass { station: st.name.clone(), aos: p.0, los: t0 + span, max_el: p.1, t_max: p.2 });
        }
    }
    out.sort_by(|a, b| a.aos.total_cmp(&b.aos));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ganho_da_dsn() {
        // 70 m em banda X: ~74 dBi.
        let g = dish_gain_dbi(70.0, 8.4e9);
        assert!((g - 74.0).abs() < 1.0, "{g}");
    }

    #[test]
    fn zenite() {
        let st = &default_stations()[0];
        let t = 1000.0;
        let site = Body::Earth.from_geodetic(st.lat, st.lon, st.alt, t);
        let (_, el, range) = look_angles(st, site * 1.1, t);
        assert!(el > 89.0, "{el}");
        assert!((range - site.norm() * 0.1).abs() < 1.0);
    }
}
