//! Subsistemas de bordo: energia (EPS), controle térmico (TCS), atitude (ADCS),
//! comunicações e suporte de vida (ECLSS).

use crate::math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

// ───────────────────────────── Energia ─────────────────────────────

/// "Sem limite" que sobrevive ao JSON (infinito vira null).
pub const UNLIMITED: f64 = 1e300;

/// Lê null como NaN (o JSON não tem NaN).
pub fn nan_default<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
}

/// Constante solar a 1 UA (W/m²).
pub const SOLAR_CONSTANT: f64 = 1361.0;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolarArray {
    /// Área de células (m²).
    pub area: f64,
    pub efficiency: f64,
    pub deployed: bool,
    /// Gira para o Sol sozinho (asas com mecanismo); senão, fica na normal do corpo.
    pub tracking: bool,
    /// Normal dos painéis fixos, no corpo.
    pub normal: Vec3,
    /// Fração perdida por degradação ou falha (0..1).
    #[serde(default)]
    pub degraded: f64,
    /// Travada sem acompanhar o Sol (falha do mecanismo).
    #[serde(default)]
    pub stuck: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FuelCell {
    pub max_w: f64,
    /// Reagentes (O₂ + H₂) restantes, kg.
    pub reactant_kg: f64,
    /// Consumo de reagente por kWh.
    pub kg_per_kwh: f64,
    pub on: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Load {
    pub id: String,
    pub name: String,
    pub watts: f64,
    pub on: bool,
    /// Fica ligada no modo de segurança (corte de cargas).
    pub essential: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Power {
    pub solar: Option<SolarArray>,
    /// Gerador termoelétrico de radioisótopo (W).
    #[serde(default)]
    pub rtg_w: f64,
    pub fuel_cell: Option<FuelCell>,
    pub battery_wh: f64,
    pub soc: f64,
    /// Células da bateria perdidas (cada uma tira 1/8 da capacidade).
    #[serde(default)]
    pub failed_cells: u32,
    pub loads: Vec<Load>,
    /// Aquecimento pedido pelo controle térmico neste passo (W).
    #[serde(skip)]
    pub heater_w: f64,
    // Saídas do último passo:
    #[serde(default)]
    pub gen_w: f64,
    #[serde(default)]
    pub load_w: f64,
    #[serde(default)]
    pub bus_v: f64,
    #[serde(default)]
    pub battery_w: f64,
    #[serde(default)]
    pub sun_angle: f64,
    /// Sem energia: a bateria zerou e a geração não cobre a carga.
    #[serde(default)]
    pub brownout: bool,
    /// Energia externa (umbilical na plataforma, ou a estação quando acoplado).
    #[serde(default)]
    pub external: bool,
}

impl Power {
    pub fn capacity_wh(&self) -> f64 {
        self.battery_wh * (1.0 - self.failed_cells.min(8) as f64 / 8.0)
    }

    pub fn load(&self, id: &str) -> Option<&Load> {
        self.loads.iter().find(|l| l.id == id)
    }

    pub fn is_on(&self, id: &str) -> bool {
        self.load(id).is_some_and(|l| l.on) && !self.brownout
    }

    pub fn set_load(&mut self, id: &str, on: bool) -> bool {
        match self.loads.iter_mut().find(|l| l.id == id) {
            Some(l) => {
                l.on = on;
                true
            }
            None => false,
        }
    }

    /// Corta as cargas não essenciais (modo de segurança).
    pub fn shed(&mut self) {
        for l in &mut self.loads {
            if !l.essential {
                l.on = false;
            }
        }
    }

    /// Avança `dt` segundos. `sun` é o fluxo solar no veículo (W/m², 0 em
    /// eclipse) e `sun_body` a direção do Sol no corpo.
    pub fn update(&mut self, dt: f64, sun: f64, sun_body: Vec3, extra_load_w: f64) {
        let mut gen_w = self.rtg_w;
        let ext_w = if self.external { self.loads.iter().filter(|l| l.on).map(|l| l.watts).sum::<f64>() + self.heater_w + extra_load_w + 0.2 * self.capacity_wh() } else { 0.0 };
        gen_w += ext_w;
        if let Some(sa) = &self.solar {
            let cos = if !sa.deployed {
                // Recolhidos: só uma fração das células vê o Sol.
                0.15 * sun_body.dot(sa.normal).max(0.0)
            } else if sa.tracking && !sa.stuck {
                // O mecanismo acompanha o Sol num eixo: perde a componente ao longo dele.
                let axis = sa.normal.any_perpendicular();
                (1.0 - sun_body.dot(axis).powi(2)).max(0.0).sqrt()
            } else {
                sun_body.dot(sa.normal).max(0.0)
            };
            self.sun_angle = cos.clamp(-1.0, 1.0).acos().to_degrees();
            gen_w += sun * sa.area * sa.efficiency * cos * (1.0 - sa.degraded);
        }
        let mut load_w: f64 = self.loads.iter().filter(|l| l.on).map(|l| l.watts).sum::<f64>()
            + self.heater_w
            + extra_load_w;
        if let Some(fc) = &mut self.fuel_cell {
            if fc.on && fc.reactant_kg > 0.0 {
                let need = (load_w - gen_w).clamp(0.0, fc.max_w);
                // A célula a combustível segue a carga; um mínimo de 20 % fica sempre.
                let out = need.max(0.2 * fc.max_w);
                fc.reactant_kg = (fc.reactant_kg - out * dt / 3.6e6 * fc.kg_per_kwh).max(0.0);
                gen_w += out;
            }
        }
        let cap = self.capacity_wh().max(1e-6);
        let mut net = gen_w - load_w;
        // Carga limitada (C/2) e eficiência de 90 % ao carregar.
        let max_charge = cap * 0.5;
        if net > 0.0 {
            let charge = net.min(max_charge) * 0.9;
            if self.soc >= 1.0 {
                net = 0.0;
            }
            self.soc = (self.soc + charge * dt / 3600.0 / cap).min(1.0);
        } else {
            self.soc = (self.soc + net * dt / 3600.0 / cap).max(0.0);
        }
        self.brownout = self.soc <= 0.0 && gen_w < load_w;
        if self.brownout {
            load_w = gen_w;
        }
        self.gen_w = gen_w;
        self.load_w = load_w;
        self.battery_w = net;
        // Barramento de 28 V: a tensão da bateria segue a carga dela.
        self.bus_v = if self.brownout {
            0.0
        } else if net > 0.0 {
            31.0 + 1.5 * self.soc
        } else {
            25.0 + 6.5 * self.soc
        };
    }
}

// ───────────────────────────── Térmico ─────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ThermalNode {
    pub id: String,
    pub name: String,
    /// Temperatura (K).
    pub temp: f64,
    /// Capacidade térmica (J/K).
    pub capacity: f64,
    /// Área que irradia para o espaço (m²); zero nos nós internos.
    pub area: f64,
    pub absorptivity: f64,
    pub emissivity: f64,
    /// Fração da potência das cargas dissipada aqui.
    pub load_share: f64,
    /// Condutância até a estrutura (W/K).
    pub conductance: f64,
    pub heater_w: f64,
    pub heater_on: bool,
    /// Termostato automático entre `set_lo` e `set_hi` (°C).
    pub auto: bool,
    pub set_lo: f64,
    pub set_hi: f64,
    /// Limites operacionais (°C), para os alarmes.
    pub min_c: f64,
    pub max_c: f64,
    /// Falha do aquecedor.
    #[serde(default)]
    pub heater_failed: bool,
}

impl ThermalNode {
    pub fn celsius(&self) -> f64 {
        self.temp - 273.15
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Thermal {
    /// O primeiro nó é a estrutura externa: troca calor com o espaço e com os outros.
    pub nodes: Vec<ThermalNode>,
    /// Radiador com persianas ou bomba de fluido (ônibus espacial, estação): rejeita
    /// calor da estrutura quando ligado (W/K acima de 0 °C).
    #[serde(default)]
    pub radiator_w_per_k: f64,
    #[serde(default)]
    pub radiator_on: bool,
}

const SIGMA: f64 = 5.670_374e-8;

impl Thermal {
    pub fn node_mut(&mut self, id: &str) -> Option<&mut ThermalNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// Potência dos aquecedores ligados (para o EPS).
    pub fn heater_power(&self) -> f64 {
        self.nodes.iter().filter(|n| n.heater_on && !n.heater_failed).map(|n| n.heater_w).sum()
    }

    /// `sun_flux` em W/m² (já com eclipse), `planet_ir` em W/m² (aquecimento pelo
    /// planeta), `load_w` a potência elétrica dissipada, `aero_w` o aquecimento
    /// aerodinâmico na estrutura.
    pub fn update(&mut self, dt: f64, sun_flux: f64, planet_ir: f64, load_w: f64, aero_w: f64, powered: bool) {
        if self.nodes.is_empty() {
            return;
        }
        // Passos internos de no máximo 10 s: os nós pequenos são rápidos.
        let n = (dt / 10.0).ceil().max(1.0) as usize;
        let h = dt / n as f64;
        for _ in 0..n {
            let t_struct = self.nodes[0].temp;
            let mut flows = vec![0.0; self.nodes.len()];
            for (i, node) in self.nodes.iter_mut().enumerate() {
                if node.auto && powered && !node.heater_failed {
                    let c = node.celsius();
                    if c < node.set_lo {
                        node.heater_on = true;
                    } else if c > node.set_hi {
                        node.heater_on = false;
                    }
                }
                let mut q = load_w * node.load_share;
                if node.heater_on && powered && !node.heater_failed {
                    q += node.heater_w;
                }
                if node.area > 0.0 {
                    // Metade da área vê o Sol, em média; o planeta aquece pela outra metade.
                    q += node.absorptivity * node.area * 0.25 * sun_flux;
                    q += node.emissivity * node.area * 0.25 * planet_ir;
                    q -= node.emissivity * node.area * SIGMA * node.temp.powi(4);
                }
                if i == 0 {
                    q += aero_w;
                } else {
                    let f = node.conductance * (node.temp - t_struct);
                    q -= f;
                    flows[0] += f;
                }
                flows[i] += q;
            }
            if self.radiator_on && powered {
                flows[0] -= self.radiator_w_per_k * (self.nodes[0].temp - 273.15).max(0.0);
            }
            for (node, q) in self.nodes.iter_mut().zip(flows) {
                node.temp = (node.temp + q * h / node.capacity).max(3.0);
            }
        }
    }
}

// ───────────────────────────── Atitude ─────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", content = "arg")]
pub enum AttMode {
    /// Sem controle: mantém a rotação que tiver.
    Free,
    /// Amortece a rotação.
    Detumble,
    /// Mantém a atitude inercial atual.
    Hold,
    Prograde,
    Retrograde,
    Normal,
    AntiNormal,
    RadialOut,
    RadialIn,
    /// Painéis para o Sol.
    SunPoint,
    /// Face +Z para o planeta, X para a frente.
    Nadir,
    /// Antena de alto ganho para a Terra (sondas).
    EarthPoint,
    /// Aponta para outro veículo (id).
    Target(u32),
    /// Atitude inercial dada (quatérnio).
    Inertial(Quat),
    /// Comandada pelo guiamento (subida, queima, reentrada).
    Guidance,
}

impl AttMode {
    pub fn name(&self) -> &'static str {
        match self {
            AttMode::Free => "Livre",
            AttMode::Detumble => "Amortecimento",
            AttMode::Hold => "Manter",
            AttMode::Prograde => "Prógrado",
            AttMode::Retrograde => "Retrógrado",
            AttMode::Normal => "Normal",
            AttMode::AntiNormal => "Antinormal",
            AttMode::RadialOut => "Radial para fora",
            AttMode::RadialIn => "Radial para dentro",
            AttMode::SunPoint => "Apontado ao Sol",
            AttMode::Nadir => "Nadir",
            AttMode::EarthPoint => "Apontado à Terra",
            AttMode::Target(_) => "Alvo",
            AttMode::Inertial(_) => "Inercial",
            AttMode::Guidance => "Guiamento",
        }
    }

    pub fn from_id(s: &str, arg: Option<u32>) -> Option<AttMode> {
        Some(match s {
            "free" => AttMode::Free,
            "detumble" => AttMode::Detumble,
            "hold" => AttMode::Hold,
            "prograde" => AttMode::Prograde,
            "retrograde" => AttMode::Retrograde,
            "normal" => AttMode::Normal,
            "antinormal" => AttMode::AntiNormal,
            "radial_out" => AttMode::RadialOut,
            "radial_in" => AttMode::RadialIn,
            "sun" => AttMode::SunPoint,
            "nadir" => AttMode::Nadir,
            "earth" => AttMode::EarthPoint,
            "target" => AttMode::Target(arg?),
            "guidance" => AttMode::Guidance,
            _ => return None,
        })
    }

    pub fn id(&self) -> &'static str {
        match self {
            AttMode::Free => "free",
            AttMode::Detumble => "detumble",
            AttMode::Hold => "hold",
            AttMode::Prograde => "prograde",
            AttMode::Retrograde => "retrograde",
            AttMode::Normal => "normal",
            AttMode::AntiNormal => "antinormal",
            AttMode::RadialOut => "radial_out",
            AttMode::RadialIn => "radial_in",
            AttMode::SunPoint => "sun",
            AttMode::Nadir => "nadir",
            AttMode::EarthPoint => "earth",
            AttMode::Target(_) => "target",
            AttMode::Inertial(_) => "inertial",
            AttMode::Guidance => "guidance",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Wheels {
    /// Momento angular armazenado em cada eixo (N·m·s).
    pub h: Vec3,
    pub h_max: f64,
    pub torque_max: f64,
    /// Roda falhada em cada eixo.
    #[serde(default)]
    pub failed: [bool; 3],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Adcs {
    pub mode: AttMode,
    /// Momentos de inércia principais (kg·m²).
    pub inertia: Vec3,
    pub wheels: Option<Wheels>,
    /// Torque dos propulsores de controle (N·m), se houver.
    pub rcs_torque: f64,
    /// Propelente dos propulsores de controle (kg) e consumo por N·m·s.
    pub rcs_prop: f64,
    pub rcs_kg_per_nms: f64,
    /// Força de translação do RCS (N), para aproximação e acoplamento.
    #[serde(default)]
    pub rcs_force: f64,
    /// Bobinas magnéticas para descarregar as rodas (órbita baixa da Terra).
    pub magnetorquers: bool,
    /// Velocidade máxima de giro (rad/s).
    pub max_rate: f64,
    /// Torque de perturbação ambiente (N·m), que enche as rodas.
    pub disturbance: f64,
    /// Viés dos girômetros (falha), rad/s.
    #[serde(default)]
    pub gyro_bias: Vec3,
    #[serde(default = "yes")]
    pub star_tracker: bool,
    /// Descarregando as rodas agora.
    #[serde(default)]
    pub desaturating: bool,
    // Saídas:
    #[serde(default, deserialize_with = "nan_default")]
    pub error_deg: f64,
    #[serde(default)]
    pub torque_cmd: Vec3,
}

fn yes() -> bool {
    true
}

impl Adcs {
    /// Controle PD de atitude com rodas e/ou RCS, integrando a dinâmica de corpo
    /// rígido por `dt`. Devolve a nova atitude e velocidade angular.
    pub fn control(&mut self, att: Quat, omega: Vec3, target: Option<Quat>, dt: f64, powered: bool) -> (Quat, Vec3) {
        let i = self.inertia;
        let measured = omega + self.gyro_bias;
        let mut torque = Vec3::ZERO;
        if powered {
            match (self.mode, target) {
                (AttMode::Free, _) => {}
                (_, Some(q)) if self.star_tracker || matches!(self.mode, AttMode::Detumble) => {
                    let err = att.error_to(q);
                    self.error_deg = err.norm().to_degrees();
                    // Banda de 0,5 rad/s² equivalente, limitada pela taxa máxima.
                    let wn = 0.35;
                    let kp = Vec3::new(i.x * wn * wn, i.y * wn * wn, i.z * wn * wn);
                    let kd = Vec3::new(i.x * 2.0 * wn, i.y * 2.0 * wn, i.z * 2.0 * wn);
                    let mut rate_cmd = Vec3::new(err.x * kp.x / kd.x, err.y * kp.y / kd.y, err.z * kp.z / kd.z);
                    if rate_cmd.norm() > self.max_rate {
                        rate_cmd = rate_cmd.unit() * self.max_rate;
                    }
                    let re = rate_cmd - measured;
                    torque = Vec3::new(re.x * kd.x, re.y * kd.y, re.z * kd.z);
                }
                _ => {
                    // Sem referência (ou sem rastreador de estrelas): só amortece.
                    torque = Vec3::new(-measured.x * i.x, -measured.y * i.y, -measured.z * i.z) * 0.7;
                    self.error_deg = f64::NAN;
                }
            }
        }
        // Atuadores: rodas primeiro, RCS para o que sobrar.
        let mut applied = Vec3::ZERO;
        if let Some(w) = &mut self.wheels {
            let comps = [torque.x, torque.y, torque.z];
            let hs = [w.h.x, w.h.y, w.h.z];
            let mut out = [0.0; 3];
            for k in 0..3 {
                if w.failed[k] || !powered {
                    continue;
                }
                let mut tq = comps[k].clamp(-w.torque_max, w.torque_max);
                // Saturada: não consegue mais absorver no mesmo sentido.
                if (hs[k] >= w.h_max && -tq > 0.0) || (hs[k] <= -w.h_max && -tq < 0.0) {
                    tq = 0.0;
                }
                out[k] = tq;
            }
            let wheel_t = Vec3::new(out[0], out[1], out[2]);
            applied += wheel_t;
            w.h = w.h - wheel_t * dt;
        }
        let rest = torque - applied;
        if self.rcs_torque > 0.0 && self.rcs_prop > 0.0 && powered {
            let mut t = rest;
            if t.norm() > self.rcs_torque {
                t = t.unit() * self.rcs_torque;
            }
            // Zona morta: o RCS só pulsa para erros apreciáveis.
            if t.norm() > self.rcs_torque * 0.02 || self.wheels.is_none() {
                self.rcs_prop = (self.rcs_prop - t.norm() * dt * self.rcs_kg_per_nms).max(0.0);
                applied += t;
            }
        }
        // Descarga das rodas: bobinas ou RCS aplicam torque externo oposto.
        if self.desaturating {
            if let Some(w) = &mut self.wheels {
                let rate = if self.magnetorquers { 0.002 } else { self.rcs_torque.min(1.0) };
                let dh = w.h.unit() * (rate * dt).min(w.h.norm());
                w.h = w.h - dh;
                if !self.magnetorquers {
                    self.rcs_prop = (self.rcs_prop - dh.norm() * self.rcs_kg_per_nms).max(0.0);
                }
                if w.h.norm() < 0.01 * w.h_max {
                    self.desaturating = false;
                }
            }
        }
        self.torque_cmd = applied;
        // Perturbação: sempre no mesmo sentido do corpo, como o gradiente de gravidade médio.
        let dist = Vec3::new(1.0, 0.6, -0.4).unit() * self.disturbance;
        let h_w = self.wheels.as_ref().map_or(Vec3::ZERO, |w| w.h);
        let total = applied + dist;
        // Euler: I ω̇ = τ − ω × (I ω + h)
        let iw = Vec3::new(i.x * omega.x, i.y * omega.y, i.z * omega.z) + h_w;
        let gyro = omega.cross(iw);
        let wdot = Vec3::new(
            (total.x - gyro.x) / i.x,
            (total.y - gyro.y) / i.y,
            (total.z - gyro.z) / i.z,
        );
        let new_omega = omega + wdot * dt;
        (att.integrate(new_omega, dt), new_omega)
    }

    pub fn wheel_saturation(&self) -> f64 {
        self.wheels.as_ref().map_or(0.0, |w| {
            let m = w.h.x.abs().max(w.h.y.abs()).max(w.h.z.abs());
            m / w.h_max
        })
    }
}

// ───────────────────────────── Comunicações ─────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Comms {
    pub tx_on: bool,
    /// Potência de RF (W).
    pub tx_power_w: f64,
    pub freq_hz: f64,
    pub lga_gain_dbi: f64,
    /// Antena de alto ganho: ganho (dBi) e largura do feixe (graus).
    pub hga_gain_dbi: f64,
    pub hga_beam_deg: f64,
    #[serde(default)]
    pub hga_deployed: bool,
    /// Taxas de dados disponíveis (bit/s).
    pub rates: Vec<f64>,
    /// Escolhe a maior taxa com margem ≥ 3 dB.
    pub auto_rate: bool,
    pub rate: f64,
    /// Gravador de bordo (MB).
    pub storage_mb: f64,
    pub stored_mb: f64,
    /// Geração de dados dos instrumentos ligados (Mbit/s), atualizada pelo veículo.
    #[serde(default)]
    pub data_gen_mbps: f64,
    /// Cobertura por satélites de retransmissão (tipo TDRS).
    #[serde(default)]
    pub relay: bool,
    /// Transmissor em pane.
    #[serde(default)]
    pub failed: bool,
    // Estado do enlace:
    #[serde(default)]
    pub link: Option<LinkStatus>,
    #[serde(default)]
    pub blackout: bool,
    /// Transmitindo o gravador para o solo.
    #[serde(default)]
    pub dumping: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LinkStatus {
    pub station: String,
    pub elevation_deg: f64,
    pub azimuth_deg: f64,
    pub range_m: f64,
    pub margin_db: f64,
    pub rate_bps: f64,
    /// Atraso de luz, ida (s).
    pub owlt: f64,
    pub via_hga: bool,
    pub via_relay: bool,
}

impl Comms {
    pub fn update_storage(&mut self, dt: f64) {
        self.stored_mb = (self.stored_mb + self.data_gen_mbps * dt / 8.0).min(self.storage_mb);
        if let Some(l) = &self.link {
            if self.dumping && self.tx_on && !self.blackout {
                self.stored_mb = (self.stored_mb - l.rate_bps * 0.9 * dt / 8e6).max(0.0);
                if self.stored_mb <= 0.0 {
                    self.dumping = false;
                }
            }
        }
    }
}

// ───────────────────────────── Suporte de vida ─────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CabinModule {
    pub name: String,
    pub volume: f64,
    /// Escotilha fechada: fora do volume comum.
    pub isolated: bool,
    /// Área do furo (m²), se vazando.
    #[serde(default)]
    pub leak_area: f64,
    /// Pressão própria quando isolado (kPa).
    #[serde(default)]
    pub pressure_kpa: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Eclss {
    pub crew: u32,
    pub modules: Vec<CabinModule>,
    /// Gás no volume comum (kg).
    pub o2_kg: f64,
    pub n2_kg: f64,
    pub co2_kg: f64,
    pub water_kg: f64,
    pub temp_c: f64,
    pub humidity: f64,
    /// Reservas pressurizadas.
    pub o2_tank_kg: f64,
    pub n2_tank_kg: f64,
    /// Gerador de O₂ por eletrólise (kg/dia), consome água e energia.
    pub o2_gen_rate: f64,
    pub o2_gen_on: bool,
    /// Remoção de CO₂ (kg/dia).
    pub scrubber_rate: f64,
    pub scrubber_on: bool,
    /// Cartuchos de LiOH: capacidade restante (kg de CO₂); infinito se regenerativo.
    pub scrubber_capacity: f64,
    /// Repressurização automática pelos tanques.
    pub auto_press: bool,
    pub fans_on: bool,
    // Saídas:
    #[serde(default)]
    pub heart_rate: f64,
}

const R_GAS: f64 = 8.314;
const M_O2: f64 = 0.032;
const M_N2: f64 = 0.028;
const M_CO2: f64 = 0.044;

impl Eclss {
    pub fn volume(&self) -> f64 {
        self.modules.iter().filter(|m| !m.isolated).map(|m| m.volume).sum::<f64>().max(1.0)
    }

    fn partial_kpa(&self, kg: f64, molar: f64) -> f64 {
        kg / molar * R_GAS * (self.temp_c + 273.15) / self.volume() / 1000.0
    }

    pub fn ppo2(&self) -> f64 {
        self.partial_kpa(self.o2_kg, M_O2)
    }
    pub fn ppn2(&self) -> f64 {
        self.partial_kpa(self.n2_kg, M_N2)
    }
    pub fn ppco2(&self) -> f64 {
        self.partial_kpa(self.co2_kg, M_CO2)
    }
    pub fn pressure(&self) -> f64 {
        self.ppo2() + self.ppn2() + self.ppco2()
    }

    /// Fecha ou abre a escotilha de um módulo. Fechando, o ar dele sai do volume
    /// comum e fica preso no módulo, na pressão do momento; abrindo, volta e se
    /// mistura (com a composição atual da cabine).
    pub fn set_isolated(&mut self, k: usize, closed: bool) {
        let Some(m) = self.modules.get(k) else { return };
        if m.isolated == closed {
            return;
        }
        let t = self.temp_c + 273.15;
        if closed {
            let frac = (m.volume / self.volume()).min(1.0);
            let p = self.pressure();
            self.o2_kg *= 1.0 - frac;
            self.n2_kg *= 1.0 - frac;
            self.co2_kg *= 1.0 - frac;
            self.modules[k].pressure_kpa = p;
            self.modules[k].isolated = true;
        } else {
            let n = self.modules[k].pressure_kpa * 1e3 * m.volume / (R_GAS * t);
            let p = self.pressure();
            let (xo, xn, xc) = if p > 1.0 { (self.ppo2() / p, self.ppn2() / p, self.ppco2() / p) } else { (0.21, 0.786, 0.0004) };
            self.o2_kg += n * xo * M_O2;
            self.n2_kg += n * xn * M_N2;
            self.co2_kg += n * xc * M_CO2;
            self.modules[k].isolated = false;
        }
    }

    /// Enche as massas de gás para 101,3 kPa, 21 % de O₂, nos módulos abertos.
    pub fn fill_standard(&mut self) {
        let n = 101.325e3 * self.volume() / (R_GAS * (self.temp_c + 273.15));
        self.o2_kg = n * 0.21 * M_O2;
        self.n2_kg = n * 0.786 * M_N2;
        self.co2_kg = n * 0.0004 * M_CO2;
        for m in &mut self.modules {
            m.pressure_kpa = 101.325;
        }
    }

    /// Avança `dt`. `powered` diz se os equipamentos têm energia; `g` a carga.
    pub fn update(&mut self, dt: f64, powered: bool, g: f64) {
        let days = dt / 86_400.0;
        let crew = self.crew as f64;
        // Metabolismo: 0,84 kg O₂ e 1,0 kg CO₂ por pessoa por dia.
        self.o2_kg = (self.o2_kg - 0.84 * crew * days).max(0.0);
        self.co2_kg += 1.0 * crew * days;
        if self.scrubber_on && powered && self.scrubber_capacity > 0.0 {
            // Remove proporcional à concentração, até a taxa nominal.
            let want = self.scrubber_rate * days * (self.ppco2() / 0.4).min(1.5);
            let removed = want.min(self.co2_kg).min(self.scrubber_capacity);
            self.co2_kg -= removed;
            if self.scrubber_capacity < UNLIMITED {
                self.scrubber_capacity -= removed;
            }
        }
        if self.o2_gen_on && powered && self.water_kg > 0.0 && self.ppo2() < 22.0 {
            let made = self.o2_gen_rate * days;
            self.o2_kg += made;
            self.water_kg = (self.water_kg - made * 1.125).max(0.0);
        }
        // Vazamento: escoamento crítico pelo furo, levando a mistura.
        let leak: f64 = self.modules.iter().filter(|m| !m.isolated).map(|m| m.leak_area).sum();
        if leak > 0.0 {
            let p = self.pressure() * 1000.0;
            let t = self.temp_c + 273.15;
            let mdot = 0.6 * leak * p * (1.4 / (287.0 * t)).sqrt() * 0.5787;
            let total = self.o2_kg + self.n2_kg + self.co2_kg;
            if total > 0.0 {
                let f = ((mdot * dt) / total).min(1.0);
                self.o2_kg *= 1.0 - f;
                self.n2_kg *= 1.0 - f;
                self.co2_kg *= 1.0 - f;
            }
        }
        for m in &mut self.modules {
            if m.isolated && m.leak_area > 0.0 {
                // O módulo isolado esvazia sozinho (constante de tempo do volume).
                let tau = m.volume / (m.leak_area * 400.0);
                m.pressure_kpa *= (-dt / tau.max(1.0)).exp();
            }
        }
        if self.auto_press && powered {
            let ppo2 = self.ppo2();
            if ppo2 < 20.0 && self.o2_tank_kg > 0.0 {
                let need = (21.0 - ppo2) / ppo2.max(1.0) * self.o2_kg;
                let give = need.min(self.o2_tank_kg).min(0.01 * dt.max(1.0) * (crew + 1.0));
                self.o2_kg += give;
                self.o2_tank_kg -= give;
            }
            let p = self.pressure();
            if p < 97.0 && self.n2_tank_kg > 0.0 {
                let need = (99.0 - p) / self.ppn2().max(1.0) * self.n2_kg;
                let give = need.min(self.n2_tank_kg).min(0.02 * dt.max(1.0));
                self.n2_kg += give;
                self.n2_tank_kg -= give;
            }
        }
        // A tripulação reage à carga e ao CO₂.
        self.heart_rate = if self.crew == 0 {
            0.0
        } else {
            (68.0 + 9.0 * (g - 1.0).max(0.0) + 12.0 * (self.ppco2() - 0.5).max(0.0)
                + 25.0 * (16.0 - self.ppo2()).max(0.0) / 4.0)
                .min(190.0)
        };
        let _ = self.fans_on;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolar_um_modulo_nao_muda_a_pressao() {
        let mut e = Eclss {
            crew: 0,
            modules: vec![
                CabinModule { name: "A".into(), volume: 100.0, isolated: false, leak_area: 0.0, pressure_kpa: 0.0 },
                CabinModule { name: "B".into(), volume: 50.0, isolated: false, leak_area: 0.0, pressure_kpa: 0.0 },
            ],
            o2_kg: 0.0,
            n2_kg: 0.0,
            co2_kg: 0.0,
            water_kg: 0.0,
            temp_c: 22.0,
            humidity: 0.4,
            o2_tank_kg: 0.0,
            n2_tank_kg: 0.0,
            o2_gen_rate: 0.0,
            o2_gen_on: false,
            scrubber_rate: 0.0,
            scrubber_on: false,
            scrubber_capacity: UNLIMITED,
            auto_press: false,
            fans_on: true,
            heart_rate: 70.0,
        };
        e.fill_standard();
        let p0 = e.pressure();
        e.set_isolated(1, true);
        assert!((e.pressure() - p0).abs() < 0.01, "{} → {}", p0, e.pressure());
        assert!((e.modules[1].pressure_kpa - p0).abs() < 0.01);
        // O módulo despressurizado e reaberto baixa a pressão de todos.
        e.modules[1].pressure_kpa = 0.0;
        e.set_isolated(1, false);
        assert!((e.pressure() - p0 * 100.0 / 150.0).abs() < 0.05, "{}", e.pressure());
    }

    #[test]
    fn bateria_descarrega_no_eclipse() {
        let mut p = Power {
            solar: Some(SolarArray {
                area: 2.0,
                efficiency: 0.3,
                deployed: true,
                tracking: false,
                normal: Vec3::X,
                degraded: 0.0,
                stuck: false,
            }),
            rtg_w: 0.0,
            fuel_cell: None,
            battery_wh: 100.0,
            soc: 0.5,
            failed_cells: 0,
            loads: vec![Load { id: "obc".into(), name: "OBC".into(), watts: 50.0, on: true, essential: true }],
            heater_w: 0.0,
            gen_w: 0.0,
            load_w: 0.0,
            bus_v: 0.0,
            battery_w: 0.0,
            sun_angle: 0.0,
            brownout: false,
            external: false,
        };
        p.update(3600.0, 0.0, Vec3::X, 0.0);
        assert!((p.soc - 0.0).abs() < 1e-9);
        p.soc = 0.5;
        p.update(60.0, SOLAR_CONSTANT, Vec3::X, 0.0);
        assert!(p.gen_w > 800.0 && p.soc > 0.5);
    }

    #[test]
    fn cabine_padrao_e_vazamento() {
        let mut e = Eclss {
            crew: 3,
            modules: vec![CabinModule { name: "A".into(), volume: 100.0, isolated: false, leak_area: 0.0, pressure_kpa: 0.0 }],
            o2_kg: 0.0,
            n2_kg: 0.0,
            co2_kg: 0.0,
            water_kg: 100.0,
            temp_c: 22.0,
            humidity: 0.45,
            o2_tank_kg: 0.0,
            n2_tank_kg: 0.0,
            o2_gen_rate: 0.0,
            o2_gen_on: false,
            scrubber_rate: 4.0,
            scrubber_on: true,
            scrubber_capacity: UNLIMITED,
            auto_press: false,
            fans_on: true,
            heart_rate: 0.0,
        };
        e.fill_standard();
        assert!((e.pressure() - 101.3).abs() < 0.5);
        assert!((e.ppo2() - 21.3).abs() < 0.5);
        e.modules[0].leak_area = 1e-4;
        let p0 = e.pressure();
        e.update(600.0, true, 1.0);
        assert!(e.pressure() < p0 - 1.0, "{}", e.pressure());
    }
}
