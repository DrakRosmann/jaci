//! Sol, planetas e Lua: constantes físicas e efemérides aproximadas.
//!
//! Os planetas seguem os elementos keplerianos médios de E. M. Standish (JPL,
//! "Approximate Positions of the Planets", válidos de 1800 a 2050, erro de
//! minutos de arco). A Lua usa elementos médios com a regressão dos nodos. Tudo
//! é devolvido no referencial equatorial J2000, relativo ao corpo pai.

use crate::math::{DEG, Vec3, wrap_2pi};
use crate::orbit::{Elements, mean_to_true, state_from_elements};
use crate::time::{DAY, centuries, gmst, tt};
use std::f64::consts::FRAC_PI_2;
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

    /// Elementos orbitais médios na eclíptica J2000. `t` é o relógio da
    /// simulação (UTC); os elementos são calculados em TT.
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
        let tc = centuries(tt(t));
        let v: Vec<f64> = (0..6).map(|i| k[0][i] + k[1][i] * tc).collect();
        let (a, e, inc, l, varpi, node) = (v[0] * AU, v[1], v[2] * DEG, v[3] * DEG, v[4] * DEG, v[5] * DEG);
        let argp = varpi - node;
        let m = l - varpi;
        Some(Elements { a, e, i: inc, raan: node, argp, nu: mean_to_true(m, e) })
    }

    /// Orientação pela IAU (WGCCRE): ascensão reta e declinação do polo norte
    /// e ângulo do meridiano principal W, em radianos, no instante `t` (UTC;
    /// calculada em TT). A Terra não entra aqui: gira pelo GMST em torno do Z.
    pub fn iau(self, t: f64) -> Option<(f64, f64, f64)> {
        let d = tt(t) / DAY;
        let tc = centuries(tt(t));
        let (a, dec, w) = match self {
            Body::Earth => return None,
            Body::Moon => {
                // Termos periódicos principais (o polo da Lua oscila alguns graus).
                let e = |c: f64, r: f64| ((c + r * d) * DEG);
                let (e1, e2, e3, e4, e5, e6, e7, e8, e9, e10, e11, e12, e13) = (
                    e(125.045, -0.052_992_1),
                    e(250.089, -0.105_984_2),
                    e(260.008, 13.012_000_9),
                    e(176.625, 13.340_715_4),
                    e(357.529, 0.985_600_3),
                    e(311.589, 26.405_708_4),
                    e(134.963, 13.064_993_0),
                    e(276.617, 0.328_714_6),
                    e(34.226, 1.748_487_7),
                    e(15.134, -0.158_976_3),
                    e(119.743, 0.003_609_6),
                    e(239.961, 0.164_357_3),
                    e(25.053, 12.959_008_8),
                );
                let a = 269.9949 + 0.0031 * tc - 3.8787 * e1.sin() - 0.1204 * e2.sin() + 0.0700 * e3.sin() - 0.0172 * e4.sin() + 0.0072 * e6.sin() - 0.0052 * e10.sin() + 0.0043 * e13.sin();
                let dec = 66.5392 + 0.0130 * tc + 1.5419 * e1.cos() + 0.0239 * e2.cos() - 0.0278 * e3.cos() + 0.0068 * e4.cos() - 0.0029 * e6.cos() + 0.0009 * e7.cos() + 0.0008 * e10.cos() - 0.0009 * e13.cos();
                let w = 38.3213 + 13.176_358_15 * d - 1.4e-12 * d * d + 3.5610 * e1.sin() + 0.1208 * e2.sin() - 0.0642 * e3.sin() + 0.0158 * e4.sin() + 0.0252 * e5.sin() - 0.0066 * e6.sin() - 0.0047 * e7.sin() - 0.0046 * e8.sin()
                    + 0.0028 * e9.sin()
                    + 0.0052 * e10.sin()
                    + 0.0040 * e11.sin()
                    + 0.0019 * e12.sin()
                    - 0.0044 * e13.sin();
                (a, dec, w)
            }
            Body::Mars => (317.681_43 - 0.1061 * tc, 52.886_50 - 0.0609 * tc, 176.630 + 350.891_982_26 * d),
            Body::Mercury => (281.0103 - 0.0328 * tc, 61.4155 - 0.0049 * tc, 329.5988 + 6.138_510_8 * d),
            Body::Venus => (272.76, 67.16, 160.20 - 1.481_368_8 * d),
            Body::Jupiter => (268.056_595 - 0.006_499 * tc, 64.495_303 + 0.002_413 * tc, 284.95 + 870.536 * d),
            Body::Saturn => (40.589 - 0.036 * tc, 83.537 - 0.004 * tc, 38.90 + 810.793_902_4 * d),
            Body::Uranus => (257.311, -15.175, 203.81 - 501.160_092_8 * d),
            Body::Neptune => {
                let n = (357.85 + 52.316 * tc) * DEG;
                (299.36 + 0.70 * n.sin(), 43.46 - 0.51 * n.cos(), 249.978 + 541.139_775_7 * d - 0.48 * n.sin())
            }
            Body::Sun => (286.13, 63.87, 84.176 + 14.1844 * d),
        };
        Some((a * DEG, dec * DEG, wrap_2pi(w * DEG)))
    }

    /// Polo norte de rotação (unitário, inercial). Sem `t`, o polo médio de J2000.
    pub fn pole(self, t: Option<f64>) -> Vec3 {
        match self.iau(t.unwrap_or(0.0)) {
            None => Vec3::Z,
            Some((a, d, _)) => Vec3::new(d.cos() * a.cos(), d.cos() * a.sin(), d.sin()),
        }
    }

    /// Inercial → fixo no corpo (gira com ele; Z no polo norte, X no meridiano principal).
    pub fn to_fixed(self, r: Vec3, t: f64) -> Vec3 {
        match self.iau(t) {
            None => r.rot_z(-gmst(t)),
            Some((a, d, w)) => r.rot_z(-(FRAC_PI_2 + a)).rot_x(-(FRAC_PI_2 - d)).rot_z(-w),
        }
    }

    /// Fixo no corpo → inercial.
    pub fn to_inertial(self, rf: Vec3, t: f64) -> Vec3 {
        match self.iau(t) {
            None => rf.rot_z(gmst(t)),
            Some((a, d, w)) => rf.rot_z(w).rot_x(FRAC_PI_2 - d).rot_z(FRAC_PI_2 + a),
        }
    }

    /// Converte posição relativa ao corpo (inercial) em latitude, longitude
    /// (graus) e altitude (m), no referencial fixo do corpo (polos da IAU).
    /// Esfera, exceto para a Terra (elipsoide WGS-84).
    pub fn to_geodetic(self, r: Vec3, t: f64) -> (f64, f64, f64) {
        let rf = self.to_fixed(r, t);
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
        let fixed = if self == Body::Earth {
            geodetic_to_ecef(lat, lon, alt)
        } else {
            let (la, lo) = (lat * DEG, lon * DEG);
            Vec3::new(la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()) * (self.radius() + alt)
        };
        self.to_inertial(fixed, t)
    }

    /// Ângulo do meridiano principal: o GMST na Terra, o W da IAU nos outros.
    pub fn rotation_angle(self, t: f64) -> f64 {
        match self.iau(t) {
            None => gmst(t),
            Some((_, _, w)) => w,
        }
    }

    /// Velocidade da superfície girando, num ponto inercial (em torno do polo médio).
    pub fn surface_velocity(self, r: Vec3) -> Vec3 {
        self.pole(None).cross(r) * self.rotation_rate()
    }

    /// Inclinação de uma órbita no equador do corpo (rad), pelo momento angular.
    pub fn equatorial_inclination(self, h: Vec3, t: f64) -> f64 {
        let n = h.unit();
        n.dot(self.pole(Some(t))).clamp(-1.0, 1.0).acos()
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
    let d = tt(t) / DAY;
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
    fn polos_da_iau() {
        let t = from_calendar(2026, 10, 19, 0, 0, 0.0);
        for b in Body::ALL {
            // O polo vai para o Z do corpo, e ida e volta não muda o vetor.
            let p = b.pole(Some(t));
            assert!((b.to_fixed(p, t) - Vec3::Z).norm() < 1e-12, "{b:?}");
            let r = Vec3::new(1.0, 2.0, 3.0);
            assert!((b.to_inertial(b.to_fixed(r, t), t) - r).norm() < 1e-12, "{b:?}");
        }
        // Marte: polo a 52,9° de declinação; a Terra continua com o Z.
        assert!((Body::Mars.pole(Some(t)).z.asin().to_degrees() - 52.87).abs() < 0.1);
        assert_eq!(Body::Earth.pole(Some(t)), Vec3::Z);
    }

    #[test]
    fn a_lua_mostra_sempre_a_mesma_face() {
        // Vista da Lua, a Terra fica perto de 0°, 0° (libração de até ~8°).
        for dias in [0.0, 7.0, 13.0, 20.0, 100.0, 1_000.0, 3_000.0] {
            let t = from_calendar(2026, 1, 1, 0, 0, 0.0) + dias * DAY;
            let (moon, _) = Body::Moon.state(t);
            let (lat, lon, _) = Body::Moon.to_geodetic(-moon, t);
            assert!(lat.abs() < 9.0 && lon.abs() < 10.0, "dia {dias}: {lat:.1}°, {lon:.1}°");
        }
    }

    #[test]
    fn geodesicas_na_lua_e_inclinacao_no_equador() {
        let t = 5.0e8;
        let r = Body::Moon.from_geodetic(-45.0, 120.0, 1_000.0, t);
        let (lat, lon, alt) = Body::Moon.to_geodetic(r, t);
        assert!((lat + 45.0).abs() < 1e-9 && (lon - 120.0).abs() < 1e-9 && (alt - 1_000.0).abs() < 1e-6);
        // Órbita polar da Lua: 90° no equador dela, qualquer coisa no da Terra.
        let pole = Body::Moon.pole(Some(t));
        let h = pole.any_perpendicular();
        assert!((Body::Moon.equatorial_inclination(h, t).to_degrees() - 90.0).abs() < 1e-9);
        assert!(Body::Moon.equatorial_inclination(pole, t).to_degrees() < 1e-6);
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
