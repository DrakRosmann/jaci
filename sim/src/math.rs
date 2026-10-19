//! Vetores, quatérnios e utilidades numéricas.

use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

pub const DEG: f64 = std::f64::consts::PI / 180.0;
pub const TAU: f64 = std::f64::consts::TAU;
pub const PI: f64 = std::f64::consts::PI;
/// Gravidade padrão, para impulso específico e cargas em g.
pub const G0: f64 = 9.80665;
/// Velocidade da luz (m/s).
pub const C: f64 = 299_792_458.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const X: Vec3 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Vec3 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }
    pub fn dot(self, o: Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn norm2(self) -> f64 {
        self.dot(self)
    }
    /// O vetor unitário; o vetor nulo continua nulo.
    pub fn unit(self) -> Vec3 {
        let n = self.norm();
        if n > 0.0 { self / n } else { Vec3::ZERO }
    }
    /// Ângulo entre dois vetores, em radianos.
    pub fn angle(self, o: Vec3) -> f64 {
        let c = self.unit().dot(o.unit()).clamp(-1.0, 1.0);
        c.acos()
    }
    /// A parte perpendicular a `n` (unitário).
    pub fn reject(self, n: Vec3) -> Vec3 {
        self - n * self.dot(n)
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
    /// Gira em torno do eixo Z.
    pub fn rot_z(self, a: f64) -> Vec3 {
        let (s, c) = a.sin_cos();
        Vec3::new(c * self.x - s * self.y, s * self.x + c * self.y, self.z)
    }
    /// Gira em torno do eixo X.
    pub fn rot_x(self, a: f64) -> Vec3 {
        let (s, c) = a.sin_cos();
        Vec3::new(self.x, c * self.y - s * self.z, s * self.y + c * self.z)
    }
    /// Um vetor qualquer perpendicular a este.
    pub fn any_perpendicular(self) -> Vec3 {
        let a = if self.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
        self.cross(a).unit()
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}
impl Mul<f64> for Vec3 {
    type Output = Vec3;
    fn mul(self, k: f64) -> Vec3 {
        Vec3::new(self.x * k, self.y * k, self.z * k)
    }
}
impl Mul<Vec3> for f64 {
    type Output = Vec3;
    fn mul(self, v: Vec3) -> Vec3 {
        v * self
    }
}
impl Div<f64> for Vec3 {
    type Output = Vec3;
    fn div(self, k: f64) -> Vec3 {
        Vec3::new(self.x / k, self.y / k, self.z / k)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}

/// Quatérnio unitário de rotação (w, x, y, z): leva o referencial do corpo ao inercial.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Quat {
    pub w: f64,
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Default for Quat {
    fn default() -> Self {
        Quat::IDENTITY
    }
}

impl Quat {
    pub const IDENTITY: Quat = Quat { w: 1.0, x: 0.0, y: 0.0, z: 0.0 };

    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Quat {
        let a = axis.unit();
        let (s, c) = (angle / 2.0).sin_cos();
        Quat { w: c, x: a.x * s, y: a.y * s, z: a.z * s }
    }

    pub fn mul(self, o: Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }

    pub fn conj(self) -> Quat {
        Quat { w: self.w, x: -self.x, y: -self.y, z: -self.z }
    }

    pub fn normalized(self) -> Quat {
        let n = (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if n == 0.0 || !n.is_finite() {
            return Quat::IDENTITY;
        }
        Quat { w: self.w / n, x: self.x / n, y: self.y / n, z: self.z / n }
    }

    /// Leva um vetor do corpo para o inercial.
    pub fn rotate(self, v: Vec3) -> Vec3 {
        let q = Vec3::new(self.x, self.y, self.z);
        let t = q.cross(v) * 2.0;
        v + t * self.w + q.cross(t)
    }

    /// Leva um vetor do inercial para o corpo.
    pub fn inv_rotate(self, v: Vec3) -> Vec3 {
        self.conj().rotate(v)
    }

    /// A atitude cujo eixo X do corpo aponta para `forward` e o Z fica o mais perto
    /// possível de `down` (o "chão", para naves com face voltada à Terra).
    pub fn look(forward: Vec3, down: Vec3) -> Quat {
        let x = forward.unit();
        let mut z = down.reject(x).unit();
        if z.norm2() < 1e-12 {
            z = x.any_perpendicular();
        }
        let y = z.cross(x);
        Quat::from_matrix([x, y, z])
    }

    /// A partir das colunas da matriz de rotação (eixos do corpo no inercial).
    pub fn from_matrix(cols: [Vec3; 3]) -> Quat {
        let [x, y, z] = cols;
        let (m00, m01, m02) = (x.x, y.x, z.x);
        let (m10, m11, m12) = (x.y, y.y, z.y);
        let (m20, m21, m22) = (x.z, y.z, z.z);
        let tr = m00 + m11 + m22;
        let q = if tr > 0.0 {
            let s = (tr + 1.0).sqrt() * 2.0;
            Quat { w: 0.25 * s, x: (m21 - m12) / s, y: (m02 - m20) / s, z: (m10 - m01) / s }
        } else if m00 > m11 && m00 > m22 {
            let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
            Quat { w: (m21 - m12) / s, x: 0.25 * s, y: (m01 + m10) / s, z: (m02 + m20) / s }
        } else if m11 > m22 {
            let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
            Quat { w: (m02 - m20) / s, x: (m01 + m10) / s, y: 0.25 * s, z: (m12 + m21) / s }
        } else {
            let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
            Quat { w: (m10 - m01) / s, x: (m02 + m20) / s, y: (m12 + m21) / s, z: 0.25 * s }
        };
        q.normalized()
    }

    /// O eixo X do corpo (para onde o motor empurra) no inercial.
    pub fn forward(self) -> Vec3 {
        self.rotate(Vec3::X)
    }

    /// Erro de atitude como vetor de rotação no corpo (eixo × ângulo) de `self` até `target`.
    pub fn error_to(self, target: Quat) -> Vec3 {
        let mut e = self.conj().mul(target);
        if e.w < 0.0 {
            e = Quat { w: -e.w, x: -e.x, y: -e.y, z: -e.z };
        }
        let v = Vec3::new(e.x, e.y, e.z);
        let s = v.norm();
        if s < 1e-12 {
            return Vec3::ZERO;
        }
        let angle = 2.0 * s.atan2(e.w);
        v / s * angle
    }

    /// Ângulo total entre duas atitudes.
    pub fn angle_to(self, target: Quat) -> f64 {
        self.error_to(target).norm()
    }

    /// Integra uma velocidade angular no corpo durante `dt`.
    pub fn integrate(self, omega_body: Vec3, dt: f64) -> Quat {
        let a = omega_body.norm() * dt;
        if a < 1e-15 {
            return self;
        }
        self.mul(Quat::from_axis_angle(omega_body, a)).normalized()
    }

    /// Gira de `self` em direção a `target` no máximo `max_angle`.
    pub fn slew_toward(self, target: Quat, max_angle: f64) -> Quat {
        let e = self.error_to(target);
        let a = e.norm();
        if a <= max_angle || a < 1e-12 {
            return target;
        }
        self.mul(Quat::from_axis_angle(e, max_angle)).normalized()
    }
}

/// Ângulo em [0, 2π).
pub fn wrap_2pi(a: f64) -> f64 {
    let r = a % TAU;
    if r < 0.0 { r + TAU } else { r }
}

/// Ângulo em (-π, π].
pub fn wrap_pi(a: f64) -> f64 {
    let r = wrap_2pi(a);
    if r > PI { r - TAU } else { r }
}

/// Interpolação linear numa tabela (x crescente); fora dela, as pontas.
pub fn interp(table: &[(f64, f64)], x: f64) -> f64 {
    if table.is_empty() {
        return 0.0;
    }
    if x <= table[0].0 {
        return table[0].1;
    }
    for w in table.windows(2) {
        let (a, b) = (w[0], w[1]);
        if x <= b.0 {
            let t = (x - a.0) / (b.0 - a.0);
            return a.1 + (b.1 - a.1) * t;
        }
    }
    table[table.len() - 1].1
}

/// Gerador pseudoaleatório pequeno e determinístico (xorshift64*), que vai junto no
/// estado salvo: a mesma simulação, com a mesma semente, dá o mesmo resultado.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniforme em [0, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// Normal padrão (Box-Muller).
    pub fn normal(&mut self) -> f64 {
        let u1 = self.uniform().max(1e-12);
        let u2 = self.uniform();
        (-2.0 * u1.ln()).sqrt() * (TAU * u2).cos()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quaternio_gira_e_desgira() {
        let q = Quat::from_axis_angle(Vec3::Z, 90.0 * DEG);
        let v = q.rotate(Vec3::X);
        assert!((v - Vec3::Y).norm() < 1e-12);
        assert!((q.inv_rotate(v) - Vec3::X).norm() < 1e-12);
    }

    #[test]
    fn look_aponta_o_eixo_x() {
        let f = Vec3::new(1.0, 2.0, -0.5).unit();
        let q = Quat::look(f, -Vec3::Z);
        assert!((q.forward() - f).norm() < 1e-9);
        assert!(q.rotate(Vec3::Z).dot(-Vec3::Z) > 0.0);
    }

    #[test]
    fn erro_de_atitude() {
        let a = Quat::IDENTITY;
        let b = Quat::from_axis_angle(Vec3::Y, 30.0 * DEG);
        assert!((a.angle_to(b) - 30.0 * DEG).abs() < 1e-9);
        let c = a.slew_toward(b, 10.0 * DEG);
        assert!((c.angle_to(b) - 20.0 * DEG).abs() < 1e-9);
    }

    #[test]
    fn tabela_interpola() {
        let t = [(0.0, 0.0), (10.0, 100.0)];
        assert_eq!(interp(&t, 5.0), 50.0);
        assert_eq!(interp(&t, -1.0), 0.0);
        assert_eq!(interp(&t, 11.0), 100.0);
    }
}
