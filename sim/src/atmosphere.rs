//! Atmosfera: a Padrão dos EUA de 1976 até 86 km, uma tabela exponencial por
//! faixas até 1000 km (Vallado), e um modelo exponencial para os outros corpos.

use crate::bodies::Body;
use crate::math::{Vec3, interp};

/// Condições do ar num ponto.
#[derive(Clone, Copy, Debug, Default)]
pub struct Air {
    /// Densidade (kg/m³).
    pub density: f64,
    /// Pressão (Pa).
    pub pressure: f64,
    /// Temperatura (K).
    pub temperature: f64,
    /// Velocidade do som (m/s).
    pub sound: f64,
}

pub const P0: f64 = 101_325.0;
const R_AIR: f64 = 287.053;
const GAMMA: f64 = 1.4;
const G0: f64 = 9.80665;
const R_EARTH: f64 = 6_356_766.0;

/// Camadas da Padrão de 1976: base geopotencial (m), temperatura (K), gradiente (K/m).
const LAYERS: [(f64, f64, f64); 7] = [
    (0.0, 288.15, -0.0065),
    (11_000.0, 216.65, 0.0),
    (20_000.0, 216.65, 0.001),
    (32_000.0, 228.65, 0.0028),
    (47_000.0, 270.65, 0.0),
    (51_000.0, 270.65, -0.0028),
    (71_000.0, 214.65, -0.002),
];

/// Pressões na base de cada camada, calculadas uma vez.
fn base_pressures() -> [f64; 7] {
    let mut p = [0.0; 7];
    p[0] = P0;
    for i in 1..7 {
        let (h0, t0, l) = LAYERS[i - 1];
        let h1 = LAYERS[i].0;
        p[i] = if l == 0.0 {
            p[i - 1] * (-G0 * (h1 - h0) / (R_AIR * t0)).exp()
        } else {
            let t1 = t0 + l * (h1 - h0);
            p[i - 1] * (t1 / t0).powf(-G0 / (R_AIR * l))
        };
    }
    p
}

/// Densidades acima de 86 km: altitude (km), densidade de referência, escala (km).
const HIGH: [(f64, f64, f64); 21] = [
    (86.0, 6.958e-6, 5.877),
    (100.0, 5.297e-7, 5.877),
    (110.0, 9.661e-8, 7.263),
    (120.0, 2.438e-8, 9.473),
    (130.0, 8.484e-9, 12.636),
    (140.0, 3.845e-9, 16.149),
    (150.0, 2.070e-9, 22.523),
    (180.0, 5.464e-10, 29.740),
    (200.0, 2.789e-10, 37.105),
    (250.0, 7.248e-11, 45.546),
    (300.0, 2.418e-11, 53.628),
    (350.0, 9.518e-12, 53.298),
    (400.0, 3.725e-12, 58.515),
    (450.0, 1.585e-12, 60.828),
    (500.0, 6.967e-13, 63.822),
    (600.0, 1.454e-13, 71.835),
    (700.0, 3.614e-14, 88.667),
    (800.0, 1.170e-14, 124.64),
    (900.0, 5.245e-15, 181.05),
    (1000.0, 3.019e-15, 268.00),
    (1_000_000.0, 0.0, 1.0),
];

/// Ar da Terra numa altitude geométrica (m).
pub fn earth(alt: f64) -> Air {
    if alt < 86_000.0 {
        let alt = alt.max(-500.0);
        let h = R_EARTH * alt / (R_EARTH + alt);
        let bp = base_pressures();
        let mut i = 0;
        while i + 1 < LAYERS.len() && h >= LAYERS[i + 1].0 {
            i += 1;
        }
        let (h0, t0, l) = LAYERS[i];
        let t = t0 + l * (h - h0);
        let p = if l == 0.0 {
            bp[i] * (-G0 * (h - h0) / (R_AIR * t0)).exp()
        } else {
            bp[i] * (t / t0).powf(-G0 / (R_AIR * l))
        };
        let rho = p / (R_AIR * t);
        return Air { density: rho, pressure: p, temperature: t, sound: (GAMMA * R_AIR * t).sqrt() };
    }
    if alt > 1_000e3 {
        return Air { density: 0.0, pressure: 0.0, temperature: 1000.0, sound: 0.0 };
    }
    let km = alt / 1e3;
    let mut i = 0;
    while i + 1 < HIGH.len() && km >= HIGH[i + 1].0 {
        i += 1;
    }
    let (h0, rho0, scale) = HIGH[i];
    let rho = rho0 * (-(km - h0) / scale).exp();
    // A termosfera esquenta até ~1000 K; a temperatura só serve ao som e à pressão.
    let t = interp(&[(86.0, 186.87), (120.0, 360.0), (200.0, 855.0), (300.0, 976.0), (500.0, 999.0)], km);
    Air { density: rho, pressure: rho * R_AIR * t, temperature: t, sound: (GAMMA * R_AIR * t).sqrt() }
}

/// Ar num corpo qualquer.
pub fn air(body: Body, alt: f64) -> Air {
    if body == Body::Earth {
        return earth(alt);
    }
    match body.atmosphere() {
        Some(a) if alt < a.top => {
            let rho = a.rho0 * (-alt.max(0.0) / a.scale_height).exp();
            Air {
                density: rho,
                pressure: rho * a.gas_r * a.temperature,
                temperature: a.temperature,
                sound: (a.gamma * a.gas_r * a.temperature).sqrt(),
            }
        }
        _ => Air::default(),
    }
}

/// Vento: velocidade horizontal do ar em relação ao chão, em função da altitude.
/// O perfil sobe até a corrente de jato (~12 km) e some acima de 60 km.
pub fn wind_profile(surface: f64, jet: f64, alt: f64) -> f64 {
    let km = alt / 1e3;
    if km < 0.0 {
        surface
    } else if km < 12.0 {
        surface + (jet - surface) * (km / 12.0)
    } else if km < 60.0 {
        jet * (1.0 - (km - 12.0) / 48.0)
    } else {
        0.0
    }
}

/// Velocidade do ar em relação ao veículo, no inercial: o ar gira com o corpo e
/// sopra com o vento local (direção `wind_from_deg`, de onde vem, em graus).
pub fn air_relative_velocity(body: Body, r: Vec3, v: Vec3, wind_speed: f64, wind_from_deg: f64) -> Vec3 {
    let mut v_air = body.surface_velocity(r);
    if wind_speed > 0.0 {
        let up = r.unit();
        let east = Vec3::Z.cross(up).unit();
        let north = up.cross(east);
        let to = (wind_from_deg + 180.0).to_radians();
        v_air += (north * to.cos() + east * to.sin()) * wind_speed;
    }
    v - v_air
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nivel_do_mar() {
        let a = earth(0.0);
        assert!((a.density - 1.225).abs() < 0.001);
        assert!((a.sound - 340.3).abs() < 0.5);
    }

    #[test]
    fn padrao_1976() {
        // 11 km: 0,3639 kg/m³; 30 km: 0,01841; 80 km: 1,846e-5.
        assert!((earth(11_000.0).density - 0.3639).abs() < 0.002);
        assert!((earth(30_000.0).density / 0.01841 - 1.0).abs() < 0.02);
        assert!((earth(80_000.0).density / 1.846e-5 - 1.0).abs() < 0.05);
        // 400 km: ordem de 3e-12.
        let d = earth(400e3).density;
        assert!(d > 2e-12 && d < 6e-12, "{d}");
        // Contínua na troca de modelo, em 86 km.
        let (a, b) = (earth(85_999.0).density, earth(86_001.0).density);
        assert!((a / b - 1.0).abs() < 0.2, "{a} {b}");
    }
}
