//! Sol, planetas e Lua: constantes físicas e efemérides aproximadas.
//!
//! Os planetas seguem os elementos keplerianos médios de E. M. Standish (JPL,
//! "Approximate Positions of the Planets", válidos de 1800 a 2050, erro de
//! minutos de arco). A Lua usa elementos médios com a regressão dos nodos. Tudo
//! é devolvido no referencial equatorial J2000, relativo ao corpo pai.

use crate::math::{DEG, Vec3};
use crate::orbit::{Elements, mean_to_true, state_from_elements};
use crate::time::{DAY, centuries, gmst};
use serde::{Deserialize, Serialize};

pub const AU: f64 = 149_597_870_700.0;
/// Obliquidade da eclíptica em J2000.
pub const OBLIQUITY: f64 = 23.439_281 * DEG;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Body {
    Sun,
    Mercury,
    Venus,
    Earth,
    Moon,
    Mars,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
}

/// Atmosfera exponencial simples (para os corpos sem modelo próprio).
#[derive(Clone, Copy, Debug)]
pub struct ExpAtmosphere {
    pub rho0: f64,
    pub scale_height: f64,
    pub top: f64,
    pub temperature: f64,
    /// Constante do gás (J/kg·K), para a velocidade do som.
    pub gas_r: f64,
    pub gamma: f64,
}

impl Body {
    pub const ALL: [Body; 10] = [
        Body::Sun,
        Body::Mercury,
        Body::Venus,
        Body::Earth,
        Body::Moon,
        Body::Mars,
        Body::Jupiter,
        Body::Saturn,
        Body::Uranus,
        Body::Neptune,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Body::Sun => "Sol",
            Body::Mercury => "Mercúrio",
            Body::Venus => "Vênus",
            Body::Earth => "Terra",
            Body::Moon => "Lua",
            Body::Mars => "Marte",
            Body::Jupiter => "Júpiter",
            Body::Saturn => "Saturno",
            Body::Uranus => "Urano",
            Body::Neptune => "Netuno",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Body::Sun => "sun",
            Body::Mercury => "mercury",
            Body::Venus => "venus",
            Body::Earth => "earth",
            Body::Moon => "moon",
            Body::Mars => "mars",
            Body::Jupiter => "jupiter",
            Body::Saturn => "saturn",
            Body::Uranus => "uranus",
            Body::Neptune => "neptune",
        }
    }

    pub fn from_id(s: &str) -> Option<Body> {
        Body::ALL.into_iter().find(|b| b.id() == s)
    }

    /// Parâmetro gravitacional μ = GM (m³/s²).
    pub fn mu(self) -> f64 {
        match self {
            Body::Sun => 1.327_124_400_18e20,
            Body::Mercury => 2.2032e13,
            Body::Venus => 3.248_59e14,
            Body::Earth => 3.986_004_418e14,
            Body::Moon => 4.904_869_5e12,
            Body::Mars => 4.282_837e13,
            Body::Jupiter => 1.266_865_34e17,
            Body::Saturn => 3.793_118_7e16,
            Body::Uranus => 5.793_939e15,
            Body::Neptune => 6.836_529e15,
        }
    }

    /// Raio equatorial (m).
    pub fn radius(self) -> f64 {
        match self {
            Body::Sun => 695_700e3,
            Body::Mercury => 2_439.7e3,
            Body::Venus => 6_051.8e3,
            Body::Earth => 6_378_137.0,
            Body::Moon => 1_737.4e3,
            Body::Mars => 3_396.2e3,
            Body::Jupiter => 71_492e3,
            Body::Saturn => 60_268e3,
            Body::Uranus => 25_559e3,
            Body::Neptune => 24_764e3,
        }
    }

    /// Achatamento J2.
    pub fn j2(self) -> f64 {
        match self {
            Body::Earth => 1.082_626_68e-3,
            Body::Mars => 1.960_45e-3,
            Body::Moon => 2.027e-4,
            Body::Jupiter => 1.4736e-2,
            Body::Saturn => 1.6298e-2,
            _ => 0.0,
        }
    }

    /// Velocidade de rotação (rad/s).
    pub fn rotation_rate(self) -> f64 {
        match self {
            Body::Earth => crate::time::EARTH_ROTATION,
            Body::Mars => 7.088_218e-5,
            Body::Moon => 2.661_7e-6,
            Body::Venus => -2.992_4e-7,
            Body::Mercury => 1.240_0e-6,
            Body::Jupiter => 1.758_5e-4,
            Body::Saturn => 1.637_9e-4,
            Body::Uranus => -1.012_4e-4,
            Body::Neptune => 1.083_4e-4,
            Body::Sun => 2.865e-6,
        }
    }

    /// O corpo em torno do qual este gira.
    pub fn parent(self) -> Option<Body> {
        match self {
            Body::Sun => None,
            Body::Moon => Some(Body::Earth),
            _ => Some(Body::Sun),
        }
    }

    /// Raio da esfera de influência (Laplace), em metros.
    pub fn soi(self) -> f64 {
        match self {
            Body::Sun => f64::INFINITY,
            Body::Mercury => 1.124e8,
            Body::Venus => 6.162e8,
            Body::Earth => 9.246e8,
            Body::Moon => 6.616e7,
            Body::Mars => 5.774e8,
            Body::Jupiter => 4.821e10,
            Body::Saturn => 5.479e10,
            Body::Uranus => 5.177e10,
            Body::Neptune => 8.664e10,
        }
    }

    /// Os corpos que giram em torno deste.
    pub fn children(self) -> Vec<Body> {
        Body::ALL.into_iter().filter(|b| b.parent() == Some(self)).collect()
    }

    /// Atmosfera para arrasto, quando há (a da Terra tem modelo próprio).
    pub fn atmosphere(self) -> Option<ExpAtmosphere> {
        match self {
            Body::Mars => Some(ExpAtmosphere {
                rho0: 0.020,
                scale_height: 11_100.0,
                top: 150e3,
                temperature: 210.0,
                gas_r: 188.9,
                gamma: 1.29,
            }),
            Body::Venus => Some(ExpAtmosphere {
                rho0: 65.0,
                scale_height: 15_900.0,
                top: 250e3,
                temperature: 400.0,
                gas_r: 188.9,
                gamma: 1.29,
            }),
            Body::Jupiter | Body::Saturn | Body::Uranus | Body::Neptune => Some(ExpAtmosphere {
                rho0: 0.16,
                scale_height: 27_000.0,
                top: 1_000e3,
                temperature: 165.0,
                gas_r: 3_745.0,
                gamma: 1.4,
            }),
            _ => None,
        }
    }

    pub fn has_atmosphere(self) -> bool {
        self == Body::Earth || self.atmosphere().is_some()
    }

    /// Altura acima da qual o arrasto é desprezível.
    pub fn atmosphere_top(self) -> f64 {
        match self {
            Body::Earth => 1_000e3,
            b => b.atmosphere().map_or(0.0, |a| a.top),
        }
    }

    /// Posição e velocidade em relação ao pai, no equatorial J2000.
    pub fn state(self, t: f64) -> (Vec3, Vec3) {
        let el = match self.elements(t) {
            Some(el) => el,
            None => return (Vec3::ZERO, Vec3::ZERO),
        };
        let mu = self.parent().map_or(0.0, |p| p.mu()) + self.mu();
        let (r, v) = state_from_elements(&el, mu);
        (r.rot_x(OBLIQUITY), v.rot_x(OBLIQUITY))
    }

    /// Posição heliocêntrica (somando a cadeia de pais).
    pub fn helio_state(self, t: f64) -> (Vec3, Vec3) {
        let mut b = self;
        let (mut r, mut v) = (Vec3::ZERO, Vec3::ZERO);
        while let Some(p) = b.parent() {
            let (rb, vb) = b.state(t);
            r += rb;
            v += vb;
            b = p;
        }
        (r, v)
    }

    /// Estado de `self` relativo a `other` (ambos em qualquer ponto da hierarquia).
    pub fn state_relative_to(self, other: Body, t: f64) -> (Vec3, Vec3) {
        let (ra, va) = self.helio_state(t);
        let (rb, vb) = other.helio_state(t);
        (ra - rb, va - vb)
    }

    /// Elementos orbitais médios na eclíptica J2000.
    pub fn elements(self, t: f64) -> Option<Elements> {
        if self == Body::Moon {
            return Some(moon_elements(t));
        }
        // a (UA), e, I, L, ϖ, Ω e as taxas por século.
        let k: [[f64; 6]; 2] = match self {
            Body::Mercury => [
                [0.387_099_27, 0.205_635_93, 7.004_979_02, 252.250_323_50, 77.457_796_28, 48.330_765_93],
                [0.000_000_37, 0.000_019_06, -0.005_947_49, 149_472.674_111_75, 0.160_476_89, -0.125_340_81],
            ],
            Body::Venus => [
                [0.723_335_66, 0.006_776_72, 3.394_676_05, 181.979_099_50, 131.602_467_18, 76.679_842_55],
                [0.000_003_90, -0.000_041_07, -0.000_788_90, 58_517.815_387_29, 0.002_683_29, -0.277_694_18],
            ],
            Body::Earth => [
                [1.000_002_61, 0.016_711_23, -0.000_015_31, 100.464_571_66, 102.937_681_93, 0.0],
                [0.000_005_62, -0.000_043_92, -0.012_946_68, 35_999.372_449_81, 0.323_273_64, 0.0],
            ],
            Body::Mars => [
                [1.523_710_34, 0.093_394_10, 1.849_691_42, -4.553_432_05, -23.943_629_59, 49.559_538_91],
                [0.000_018_47, 0.000_078_82, -0.008_131_31, 19_140.302_684_99, 0.444_410_88, -0.292_573_43],
            ],
            Body::Jupiter => [
                [5.202_887_00, 0.048_386_24, 1.304_396_95, 34.396_440_51, 14.728_479_83, 100.473_909_09],
                [-0.000_116_07, -0.000_132_53, -0.001_837_14, 3_034.746_127_75, 0.212_526_68, 0.204_691_06],
            ],
            Body::Saturn => [
                [9.536_675_94, 0.053_861_79, 2.485_991_87, 49.954_244_23, 92.598_878_31, 113.662_424_48],
                [-0.001_250_60, -0.000_509_91, 0.001_936_09, 1_222.493_622_01, -0.418_972_16, -0.288_677_94],
            ],
            Body::Uranus => [
                [19.189_164_64, 0.047_257_44, 0.772_637_83, 313.238_104_51, 170.954_276_30, 74.016_925_03],
                [-0.001_961_76, -0.000_043_97, -0.002_429_39, 428.482_027_85, 0.408_052_81, 0.042_405_89],
            ],
            Body::Neptune => [
                [30.069_922_76, 0.008_590_48, 1.770_043_47, -55.120_029_69, 44.964_762_27, 131.784_225_74],
                [0.000_262_91, 0.000_051_05, 0.000_353_72, 218.459_453_25, -0.322_414_64, -0.005_086_64],
            ],
            Body::Sun | Body::Moon => return None,
        };
        let tc = centuries(t);
        let v: Vec<f64> = (0..6).map(|i| k[0][i] + k[1][i] * tc).collect();
        let (a, e, inc, l, varpi, node) = (v[0] * AU, v[1], v[2] * DEG, v[3] * DEG, v[4] * DEG, v[5] * DEG);
        let argp = varpi - node;
        let m = l - varpi;
        Some(Elements { a, e, i: inc, raan: node, argp, nu: mean_to_true(m, e) })
    }

    /// Converte posição relativa ao corpo (inercial) em latitude, longitude
    /// (graus) e altitude (m), girando com o corpo. Esfera, exceto para a Terra
    /// (elipsoide WGS-84).
    pub fn to_geodetic(self, r: Vec3, t: f64) -> (f64, f64, f64) {
        let rf = r.rot_z(-self.rotation_angle(t));
        if self == Body::Earth {
            return ecef_to_geodetic(rf);
        }
        let rn = rf.norm();
        let lat = (rf.z / rn).asin().to_degrees();
        let lon = rf.y.atan2(rf.x).to_degrees();
        (lat, lon, rn - self.radius())
    }

    /// Posição inercial de um ponto da superfície (lat, lon em graus, alt em m).
    pub fn from_geodetic(self, lat: f64, lon: f64, alt: f64, t: f64) -> Vec3 {
        let ecef = if self == Body::Earth {
            geodetic_to_ecef(lat, lon, alt)
        } else {
            let (la, lo) = (lat * DEG, lon * DEG);
            Vec3::new(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()) * (self.radius() + alt)
        };
        ecef.rot_z(self.rotation_angle(t))
    }

    /// Ângulo de rotação do corpo em torno do eixo Z (simplificação: todos os
    /// polos alinhados ao Z equatorial; exato só para a Terra).
    pub fn rotation_angle(self, t: f64) -> f64 {
        match self {
            Body::Earth => gmst(t),
            Body::Mars => (176.630 * DEG) + self.rotation_rate() * t,
            b => b.rotation_rate() * t,
        }
    }

    /// Velocidade da superfície girando, num ponto inercial.
    pub fn surface_velocity(self, r: Vec3) -> Vec3 {
        Vec3::Z.cross(r) * self.rotation_rate()
    }

    /// Altitude acima da superfície (elipsoide na Terra), sem a longitude.
    pub fn altitude(self, r: Vec3) -> f64 {
        if self == Body::Earth {
            let (_, _, h) = ecef_to_geodetic(r);
            h
        } else {
            r.norm() - self.radius()
        }
    }
}

fn moon_elements(t: f64) -> Elements {
    let d = t / DAY;
    let l = (218.316 + 13.176_396 * d) * DEG;
    let m = (134.963 + 13.064_993 * d) * DEG;
    let f = (93.272 + 13.229_350 * d) * DEG;
    let node = l - f;
    let varpi = l - m;
    Elements {
        a: 384_400e3,
        e: 0.0549,
        i: 5.145 * DEG,
        raan: node,
        argp: varpi - node,
        nu: mean_to_true(m, 0.0549),
    }
}

const WGS84_A: f64 = 6_378_137.0;
const WGS84_F: f64 = 1.0 / 298.257_223_563;

/// ECEF → geodésicas (graus, graus, metros), método de Bowring.
pub fn ecef_to_geodetic(r: Vec3) -> (f64, f64, f64) {
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let b = WGS84_A * (1.0 - WGS84_F);
    let ep2 = (WGS84_A * WGS84_A - b * b) / (b * b);
    let p = (r.x * r.x + r.y * r.y).sqrt();
    let lon = r.y.atan2(r.x);
    if p < 1e-6 {
        let lat = 90f64.copysign(r.z);
        return (lat, lon.to_degrees(), r.z.abs() - b);
    }
    let th = (r.z * WGS84_A).atan2(p * b);
    let lat = (r.z + ep2 * b * th.sin().powi(3)).atan2(p - e2 * WGS84_A * th.cos().powi(3));
    let n = WGS84_A / (1.0 - e2 * lat.sin().powi(2)).sqrt();
    let h = p / lat.cos() - n;
    (lat.to_degrees(), lon.to_degrees(), h)
}

pub fn geodetic_to_ecef(lat: f64, lon: f64, alt: f64) -> Vec3 {
    let e2 = WGS84_F * (2.0 - WGS84_F);
    let (la, lo) = (lat * DEG, lon * DEG);
    let n = WGS84_A / (1.0 - e2 * la.sin().powi(2)).sqrt();
    Vec3::new(
        (n + alt) * la.cos() * lo.cos(),
        (n + alt) * la.cos() * lo.sin(),
        (n * (1.0 - e2) + alt) * la.sin(),
    )
}

/// Direção do Sol vista de um corpo, no inercial (unitário), e a distância.
pub fn sun_direction(from: Body, r_rel: Vec3, t: f64) -> (Vec3, f64) {
    let (rb, _) = from.helio_state(t);
    let to_sun = -(rb + r_rel);
    (to_sun.unit(), to_sun.norm())
}

/// Sombra cilíndrica do corpo central: 0 em eclipse, 1 ao sol.
pub fn sunlit(central: Body, r: Vec3, sun_dir: Vec3) -> f64 {
    if central == Body::Sun {
        return 1.0;
    }
    let along = r.dot(sun_dir);
    if along > 0.0 {
        return 1.0;
    }
    let perp = (r - sun_dir * along).norm();
    let rb = central.radius();
    // Penumbra simplificada: transição suave numa faixa de 0,5% do raio.
    let band = rb * 0.005;
    ((perp - rb) / band + 0.5).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::from_calendar;

    #[test]
    fn terra_a_uma_ua() {
        let t = from_calendar(2026, 1, 3, 0, 0, 0.0);
        let (r, v) = Body::Earth.state(t);
        // Periélio no começo de janeiro: ~0,983 UA, ~30,3 km/s.
        assert!((r.norm() / AU - 0.983).abs() < 0.002, "{}", r.norm() / AU);
        assert!((v.norm() - 30_290.0).abs() < 150.0, "{}", v.norm());
        // No referencial equatorial a Terra fica fora do plano XY.
        assert!(r.z.abs() > 1e9 || r.y.abs() < 2e10);
    }

    #[test]
    fn lua_a_384_mil_km() {
        let (r, v) = Body::Moon.state(1.0e8);
        assert!((r.norm() / 1e3 - 384_400.0).abs() < 25_000.0);
        assert!((v.norm() - 1_022.0).abs() < 100.0);
    }

    #[test]
    fn marte_em_oposicao_2027() {
        // Oposição de Marte em fev/2027: Terra e Marte do mesmo lado do Sol.
        let t = from_calendar(2027, 2, 19, 0, 0, 0.0);
        let (e, _) = Body::Earth.state(t);
        let (m, _) = Body::Mars.state(t);
        assert!(e.angle(m).to_degrees() < 3.0, "{}", e.angle(m).to_degrees());
    }

    #[test]
    fn geodesicas_ida_e_volta() {
        let t = 12_345.0;
        let r = Body::Earth.from_geodetic(-2.3167, -44.3667, 50.0, t);
        let (lat, lon, alt) = Body::Earth.to_geodetic(r, t);
        assert!((lat + 2.3167).abs() < 1e-7);
        assert!((lon + 44.3667).abs() < 1e-7);
        assert!((alt - 50.0).abs() < 1e-4);
    }
}
