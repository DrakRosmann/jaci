//! Aerodinâmica simplificada: coeficientes por número de Mach e aquecimento.

use crate::math::interp;
use serde::{Deserialize, Serialize};

/// Forma do veículo, para os coeficientes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shape {
    /// Foguete esbelto com coifa.
    Rocket,
    /// Cápsula com escudo térmico na frente (rombuda).
    Capsule,
    /// Corpo sustentador com asas (ônibus espacial).
    Winged,
    /// Satélite, estação: só arrasto, Cd ~2,2 em fluxo molecular livre.
    Box,
}

/// Coeficiente de arrasto em função do Mach.
pub fn drag_coefficient(shape: Shape, mach: f64) -> f64 {
    match shape {
        Shape::Rocket => interp(
            &[(0.0, 0.30), (0.8, 0.32), (1.05, 0.55), (1.3, 0.50), (2.0, 0.40), (4.0, 0.30), (8.0, 0.25)],
            mach,
        ),
        Shape::Capsule => interp(&[(0.0, 0.85), (0.8, 1.0), (1.2, 1.35), (3.0, 1.45), (10.0, 1.3)], mach),
        Shape::Winged => interp(&[(0.0, 0.08), (0.9, 0.12), (1.2, 0.25), (3.0, 0.6), (10.0, 0.85)], mach),
        Shape::Box => 2.2,
    }
}

/// Razão sustentação/arrasto (no ângulo de ataque de projeto).
pub fn lift_to_drag(shape: Shape, mach: f64) -> f64 {
    match shape {
        Shape::Capsule => interp(&[(0.0, 0.0), (1.5, 0.15), (3.0, 0.3)], mach),
        Shape::Winged => interp(&[(0.0, 4.5), (0.9, 4.0), (1.5, 2.0), (3.0, 1.3), (6.0, 1.1), (20.0, 1.0)], mach),
        _ => 0.0,
    }
}

/// Fluxo de calor convectivo no ponto de estagnação (Sutton-Graves), W/m².
pub fn stagnation_heat_flux(density: f64, speed: f64, nose_radius: f64) -> f64 {
    1.7415e-4 * (density / nose_radius.max(0.05)).sqrt() * speed.powi(3)
}

/// Temperatura de equilíbrio radiativo de uma superfície recebendo `q` W/m².
pub fn radiative_equilibrium(q: f64, emissivity: f64) -> f64 {
    (q / (emissivity * 5.670_374e-8)).max(0.0).powf(0.25)
}
