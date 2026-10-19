//! Controle de lançamento: contagem regressiva, critérios de lançamento (LCC),
//! votação GO/NO-GO, carregamento de propelente, clima e janela.

use crate::bodies::Body;
use crate::math::{Rng, Vec3};
use crate::vehicle::{EngineState, Level, Site, Vehicle, VehicleId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Weather {
    /// Vento de superfície (m/s) e de onde vem (graus).
    pub surface_wind: f64,
    pub wind_from: f64,
    /// Vento na corrente de jato (m/s).
    pub jet_wind: f64,
    /// Raio mais próximo (km).
    pub lightning_km: f64,
    /// Teto de nuvens (m).
    pub ceiling_m: f64,
    pub rain: bool,
    pub temperature_c: f64,
    /// O clima muda sozinho.
    pub evolve: bool,
}

impl Default for Weather {
    fn default() -> Self {
        Weather {
            surface_wind: 5.0,
            wind_from: 90.0,
            jet_wind: 30.0,
            lightning_km: 80.0,
            ceiling_m: 3_000.0,
            rain: false,
            temperature_c: 27.0,
            evolve: true,
        }
    }
}

impl Weather {
    pub fn evolve(&mut self, dt: f64, rng: &mut Rng) {
        if !self.evolve || dt <= 0.0 {
            return;
        }
        let k = (dt / 600.0).sqrt();
        self.surface_wind = (self.surface_wind + rng.normal() * 1.2 * k + (6.0 - self.surface_wind) * 0.02 * k).clamp(0.0, 30.0);
        self.wind_from = (self.wind_from + rng.normal() * 8.0 * k).rem_euclid(360.0);
        self.jet_wind = (self.jet_wind + rng.normal() * 3.0 * k + (35.0 - self.jet_wind) * 0.02 * k).clamp(0.0, 90.0);
        // Tempestades vêm e vão.
        self.lightning_km = (self.lightning_km + rng.normal() * 6.0 * k + (60.0 - self.lightning_km) * 0.01 * k).clamp(0.0, 150.0);
        self.ceiling_m = (self.ceiling_m + rng.normal() * 150.0 * k).clamp(150.0, 6_000.0);
        if rng.uniform() < 0.002 * dt / 60.0 {
            self.rain = !self.rain;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CountState {
    Idle,
    Counting,
    Hold,
    Scrubbed,
    Launched,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeqAction {
    Note,
    PropellantLoad,
    CrewArm,
    InternalPower,
    ArmFts,
    BuiltInHold,
    AutoSequence,
    Ignition,
    Liftoff,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SeqItem {
    /// Segundos antes do T-0.
    pub at: f64,
    pub name: String,
    pub action: SeqAction,
    pub done: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub id: String,
    pub name: String,
    /// Forçado pelo operador (GO = true, NO-GO = false).
    pub manual: Option<bool>,
    /// Motivo de NO-GO automático pelos critérios.
    pub reason: Option<String>,
    /// Resposta na última votação.
    pub polled: Option<bool>,
}

impl Position {
    pub fn go(&self) -> bool {
        self.manual.unwrap_or(self.reason.is_none())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Criterion {
    pub id: String,
    pub name: String,
    pub position: String,
    pub ok: bool,
    pub value: String,
    pub limit: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchControl {
    pub vehicle: Option<VehicleId>,
    pub state: CountState,
    /// Segundos até o T-0.
    pub t_minus: f64,
    pub hold_reason: String,
    pub sequence: Vec<SeqItem>,
    pub positions: Vec<Position>,
    pub criteria: Vec<Criterion>,
    /// Janela: abre e fecha (instantes absolutos).
    pub window: Option<(f64, f64)>,
    pub loading: bool,
    /// Taxa de carregamento: fração da capacidade por segundo.
    pub load_rate: f64,
    /// Área do lançamento interditada (barco ou avião).
    pub range_fouled: f64,
    /// Falha de sistema de solo (s restantes).
    pub ground_fault: f64,
    pub crew_arm_retracted: bool,
    pub internal_power: bool,
    pub auto_sequence: bool,
    /// Limite de vento de superfície (m/s).
    pub wind_limit: f64,
}

pub const POSITIONS: [(&str, &str); 11] = [
    ("ld", "Diretor de lançamento"),
    ("fd", "Diretor de voo"),
    ("wx", "Meteorologia"),
    ("rso", "Segurança de voo"),
    ("lv", "Veículo lançador"),
    ("prop", "Propulsão"),
    ("pl", "Carga útil"),
    ("com", "Comunicações"),
    ("gs", "Sistemas de solo"),
    ("crew", "Tripulação"),
    ("surg", "Médico de voo"),
];

impl Default for LaunchControl {
    fn default() -> Self {
        LaunchControl {
            vehicle: None,
            state: CountState::Idle,
            t_minus: 3_600.0,
            hold_reason: String::new(),
            sequence: vec![],
            positions: vec![],
            criteria: vec![],
            window: None,
            loading: false,
            load_rate: 1.0 / 1_500.0,
            range_fouled: 0.0,
            ground_fault: 0.0,
            crew_arm_retracted: false,
            internal_power: false,
            auto_sequence: false,
            wind_limit: 15.0,
        }
    }
}

/// O que o controle de lançamento pede ao mundo neste passo.
#[derive(Default)]
pub struct LaunchOutput {
    pub events: Vec<(Level, String)>,
    pub ignite_liquids: bool,
    pub release: bool,
    pub shutdown: bool,
}

impl LaunchControl {
    /// Prepara a contagem para um veículo na plataforma.
    pub fn setup(&mut self, v: &Vehicle, t_minus: f64) {
        let crewed = v.crew() > 0;
        let solid_first = v.stages.first().is_some_and(|s| s.engines.iter().all(|e| e.solid));
        let cryo = v.stages.iter().any(|s| s.cryogenic);
        self.vehicle = Some(v.id);
        self.state = CountState::Idle;
        self.t_minus = t_minus;
        self.crew_arm_retracted = !crewed;
        self.internal_power = false;
        self.auto_sequence = false;
        self.wind_limit = if crewed { 12.0 } else { 15.0 };
        let mut seq: Vec<(f64, &str, SeqAction)> = vec![
            (t_minus.max(3_600.0), "Início da contagem: equipes em posição", SeqAction::Note),
        ];
        if cryo {
            seq.push((2_700.0, "Início do carregamento de propelentes", SeqAction::PropellantLoad));
        }
        seq.push((1_200.0, "Fechamento da área de lançamento", SeqAction::Note));
        if crewed {
            seq.push((900.0, "Recolhimento do braço de acesso da tripulação", SeqAction::CrewArm));
        }
        seq.push((600.0, "Espera programada: votação GO/NO-GO", SeqAction::BuiltInHold));
        seq.push((300.0, "Veículo em energia interna", SeqAction::InternalPower));
        seq.push((240.0, "Resfriamento dos motores", SeqAction::Note));
        seq.push((120.0, "Terminação de voo armada", SeqAction::ArmFts));
        seq.push((60.0, "Sequência automática: computador de bordo no controle", SeqAction::AutoSequence));
        if !solid_first {
            let ign = if v.stages.first().is_some_and(|s| s.boosters) && v.stages.get(1).is_some_and(|s| s.engines.iter().all(|e| !e.solid)) {
                6.6
            } else {
                3.0
            };
            seq.push((ign, "Sequência de ignição", SeqAction::Ignition));
        } else if v.stages.first().is_some_and(|s| s.boosters) {
            seq.push((6.6, "Sequência de ignição dos motores líquidos", SeqAction::Ignition));
        }
        seq.push((0.0, "T-0: liberação", SeqAction::Liftoff));
        self.sequence = seq
            .into_iter()
            .map(|(at, name, action)| SeqItem { at, name: name.into(), action, done: at > t_minus + 1e-6 && action != SeqAction::Note })
            .collect();
        // O que já passou (contagem começando tarde) conta como feito, menos o carregamento.
        for s in &mut self.sequence {
            if s.at > t_minus && s.action != SeqAction::PropellantLoad {
                s.done = true;
            }
            if s.action == SeqAction::PropellantLoad && s.at > t_minus {
                s.done = false;
            }
        }
        self.positions = POSITIONS
            .iter()
            .filter(|(id, _)| crewed || (*id != "crew" && *id != "surg"))
            .map(|(id, name)| Position { id: (*id).into(), name: (*name).into(), manual: None, reason: None, polled: None })
            .collect();
    }

    pub fn position_mut(&mut self, id: &str) -> Option<&mut Position> {
        self.positions.iter_mut().find(|p| p.id == id)
    }

    /// Avalia os critérios de lançamento e marca as posições.
    pub fn evaluate(&mut self, v: &Vehicle, wx: &Weather, has_link: bool) {
        let mut c: Vec<Criterion> = Vec::new();
        let mut add = |id: &str, name: &str, pos: &str, ok: bool, value: String, limit: &str| {
            c.push(Criterion { id: id.into(), name: name.into(), position: pos.into(), ok, value, limit: limit.into() })
        };
        let crewed = v.crew() > 0;
        add("wind", "Vento de superfície", "wx", wx.surface_wind <= self.wind_limit, format!("{:.1} m/s", wx.surface_wind), &format!("≤ {:.0} m/s", self.wind_limit));
        add("jet", "Vento em altitude", "wx", wx.jet_wind <= 65.0, format!("{:.0} m/s", wx.jet_wind), "≤ 65 m/s");
        add("lightning", "Raios", "wx", wx.lightning_km >= 18.0, format!("{:.0} km", wx.lightning_km), "≥ 18 km");
        add("ceiling", "Teto de nuvens", "wx", wx.ceiling_m >= if crewed { 1_800.0 } else { 900.0 }, format!("{:.0} m", wx.ceiling_m), if crewed { "≥ 1800 m" } else { "≥ 900 m" });
        add("range", "Área de segurança livre", "rso", self.range_fouled <= 0.0, if self.range_fouled > 0.0 { "barco na área".into() } else { "livre".into() }, "livre");
        if v.fts.installed {
            let need = self.t_minus <= 120.0;
            add("fts", "Terminação de voo armada", "rso", !need || v.fts.armed, if v.fts.armed { "armada".into() } else { "segura".into() }, "armada após T-2 min");
        }
        let load_ok = v.stages.iter().filter(|s| s.cryogenic).all(|s| s.prop_fraction() >= 0.985);
        let min_load = v.stages.iter().filter(|s| s.cryogenic).map(|s| s.prop_fraction()).fold(1.0, f64::min);
        let need_load = self.t_minus <= 600.0;
        add("loaded", "Propelentes carregados", "prop", !need_load || load_ok, format!("{:.1} %", min_load * 100.0), "≥ 98,5 % a partir de T-10 min");
        let leak = v.stages.iter().any(|s| s.leak > 0.0);
        add("tanks", "Pressão dos tanques", "prop", !leak, if leak { "queda de pressão".into() } else { "nominal".into() }, "nominal");
        let failed = v.stages.iter().flat_map(|s| &s.engines).any(|e| e.state == EngineState::Failed);
        add("engines", "Motores", "lv", !failed, if failed { "motor em falha".into() } else { "nominal".into() }, "sem falhas");
        if let Some(sys) = &v.systems {
            add("battery", "Bateria da nave", "pl", sys.power.soc >= 0.8, format!("{:.0} %", sys.power.soc * 100.0), "≥ 80 %");
            let temps_ok = sys.thermal.nodes.iter().all(|n| n.celsius() > n.min_c && n.celsius() < n.max_c);
            add("thermal", "Temperaturas da nave", "pl", temps_ok, if temps_ok { "nominais".into() } else { "fora da faixa".into() }, "dentro da faixa");
            add("telemetry", "Telemetria do veículo", "com", has_link || sys.comms.tx_on, if sys.comms.tx_on { "recebendo".into() } else { "sem sinal".into() }, "recebendo");
            if let Some(e) = &sys.eclss {
                let p = e.pressure();
                add("cabin", "Pressão da cabine", "crew", (95.0..=104.0).contains(&p), format!("{:.1} kPa", p), "95–104 kPa");
                if self.t_minus <= 900.0 && crewed {
                    add("arm", "Braço de acesso recolhido", "crew", self.crew_arm_retracted, if self.crew_arm_retracted { "recolhido".into() } else { "estendido".into() }, "recolhido após T-15 min");
                }
                add("vitals", "Sinais vitais da tripulação", "surg", e.heart_rate < 130.0, format!("{:.0} bpm", e.heart_rate), "< 130 bpm");
            }
        }
        add("ground", "Sistemas de solo", "gs", self.ground_fault <= 0.0, if self.ground_fault > 0.0 { "falha em reparo".into() } else { "nominais".into() }, "nominais");
        if let Some((open, close)) = self.window {
            let _ = (open, close);
        }
        for p in &mut self.positions {
            p.reason = c.iter().find(|x| x.position == p.id && !x.ok).map(|x| format!("{}: {}", x.name, x.value));
        }
        // Diretor de voo e de lançamento: NO-GO se qualquer outra posição está.
        let any_nogo = self.positions.iter().filter(|p| p.id != "ld" && p.id != "fd").any(|p| !p.go());
        for id in ["fd", "ld"] {
            if let Some(p) = self.position_mut(id) {
                p.reason = any_nogo.then(|| "aguardando as outras posições".into());
            }
        }
        self.criteria = c;
    }

    pub fn all_go(&self) -> bool {
        self.positions.iter().all(|p| p.go())
    }

    /// Votação: cada posição responde.
    pub fn poll(&mut self) -> Vec<(Level, String)> {
        let mut ev = vec![(Level::Info, "Votação GO/NO-GO para o lançamento".to_string())];
        for p in &mut self.positions {
            let go = p.go();
            p.polled = Some(go);
            ev.push((
                if go { Level::Good } else { Level::Warn },
                format!("{}: {}{}", p.name, if go { "GO" } else { "NO-GO" }, p.reason.as_ref().map(|r| format!(" ({r})")).unwrap_or_default()),
            ));
        }
        let all = self.all_go();
        ev.push((if all { Level::Good } else { Level::Warn }, if all { "Todas as posições GO para o lançamento".into() } else { "Lançamento NO-GO".into() }));
        ev
    }

    pub fn hold(&mut self, reason: &str) {
        if matches!(self.state, CountState::Counting) {
            self.state = CountState::Hold;
            self.hold_reason = reason.into();
        }
    }

    /// Volta a contagem a T-10 min, desfazendo o que vem depois.
    pub fn recycle(&mut self, to: f64) {
        self.t_minus = to;
        self.state = CountState::Hold;
        self.hold_reason = "Recomeço da contagem".into();
        self.auto_sequence = false;
        for s in &mut self.sequence {
            if s.at < to {
                s.done = false;
            }
        }
        for p in &mut self.positions {
            p.polled = None;
        }
    }

    /// Avança a contagem `dt` segundos.
    pub fn tick(&mut self, v: &mut Vehicle, wx: &Weather, rng: &mut Rng, dt: f64, has_link: bool) -> LaunchOutput {
        let mut out = LaunchOutput::default();
        if self.vehicle != Some(v.id) {
            return out;
        }
        if self.range_fouled > 0.0 {
            self.range_fouled -= dt;
            if self.range_fouled <= 0.0 {
                out.events.push((Level::Good, "Área de segurança livre novamente".into()));
            }
        }
        if self.ground_fault > 0.0 {
            self.ground_fault -= dt;
            if self.ground_fault <= 0.0 {
                out.events.push((Level::Good, "Sistemas de solo restabelecidos".into()));
            }
        }
        // Carregamento e evaporação.
        for s in v.stages.iter_mut().filter(|s| s.cryogenic) {
            if self.loading {
                s.propellant = (s.propellant + s.capacity * self.load_rate * dt).min(s.capacity);
            } else if s.propellant > 0.0 && !v.released {
                // Evaporação sem reposição: ~0,5 % por hora.
                s.propellant = (s.propellant - s.capacity * 0.005 / 3600.0 * dt).max(0.0);
            }
        }
        self.evaluate(v, wx, has_link);
        if self.state != CountState::Counting {
            return out;
        }
        // Barco entrando na área de vez em quando.
        if rng.uniform() < dt / 7_200.0 && self.t_minus > 60.0 {
            self.range_fouled = 300.0 + rng.uniform() * 600.0;
            out.events.push((Level::Warn, "Segurança de voo: barco na área de exclusão".into()));
        }
        let before = self.t_minus;
        self.t_minus -= dt;
        let now = self.t_minus;
        // Critérios em violação no fim da contagem: espera automática ou aborto.
        let violated = self.criteria.iter().find(|c| !c.ok).cloned();
        if let Some(c) = &violated {
            if now <= 600.0 && now > 31.0 {
                self.t_minus = before;
                self.hold(&format!("LCC: {} ({})", c.name, c.value));
                out.events.push((Level::Warn, format!("Espera automática em T-{:.0} s: {} fora do limite ({})", before, c.name, c.value)));
                return out;
            }
            if now <= 31.0 && now > 0.0 {
                out.events.push((Level::Alarm, format!("Aborto da sequência automática em T-{:.0} s: {}", before, c.name)));
                out.shutdown = true;
                self.recycle(600.0);
                return out;
            }
        }
        let mut items: Vec<usize> = (0..self.sequence.len()).filter(|&i| !self.sequence[i].done && self.sequence[i].at >= now).collect();
        items.sort_by(|&a, &b| self.sequence[b].at.total_cmp(&self.sequence[a].at));
        for i in items {
            let item = self.sequence[i].clone();
            match item.action {
                SeqAction::BuiltInHold => {
                    self.t_minus = item.at;
                    self.sequence[i].done = true;
                    self.state = CountState::Hold;
                    self.hold_reason = item.name.clone();
                    out.events.push((Level::Info, format!("T-{} — {}", fmt_t(item.at), item.name)));
                    return out;
                }
                SeqAction::PropellantLoad => {
                    self.loading = true;
                }
                SeqAction::CrewArm => self.crew_arm_retracted = true,
                SeqAction::InternalPower => self.internal_power = true,
                SeqAction::ArmFts => {
                    if v.fts.installed {
                        v.fts.armed = true;
                    }
                }
                SeqAction::AutoSequence => self.auto_sequence = true,
                SeqAction::Ignition => out.ignite_liquids = true,
                SeqAction::Liftoff => {
                    // Confere os motores líquidos antes de soltar.
                    let liquids: Vec<_> = v.active_stages().into_iter().flat_map(|i| v.stages[i].engines.iter()).filter(|e| !e.solid).collect();
                    let bad = liquids.iter().any(|e| e.state == EngineState::Failed || (e.lit() && e.spool < 0.9));
                    let need_liquids = !liquids.is_empty();
                    let lit = liquids.iter().filter(|e| e.lit()).count();
                    if bad || (need_liquids && lit == 0) {
                        out.events.push((Level::Alarm, "Desligamento na plataforma: motores fora do nominal".into()));
                        out.shutdown = true;
                        self.recycle(600.0);
                        self.state = CountState::Scrubbed;
                        self.hold_reason = "Lançamento cancelado: desligamento na plataforma".into();
                        return out;
                    }
                    out.release = true;
                    self.state = CountState::Launched;
                    self.loading = false;
                }
                SeqAction::Note => {}
            }
            self.sequence[i].done = true;
            out.events.push((Level::Info, format!("T-{} — {}", fmt_t(item.at), item.name)));
        }
        out
    }
}

fn fmt_t(s: f64) -> String {
    let s = s.max(0.0).round() as i64;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// Instantes em que o local de lançamento cruza o plano de uma órbita (para o
/// lançamento direto no plano do alvo), subindo para o norte ou descendo.
pub fn plane_crossings(site: &Site, normal: Vec3, t0: f64, span: f64, northbound: bool) -> Vec<f64> {
    let f = |t: f64| Body::Earth.from_geodetic(site.lat, site.lon, site.alt, t).unit().dot(normal);
    let mut out = Vec::new();
    let step = 60.0;
    let mut t = t0;
    let mut prev = f(t);
    while t < t0 + span {
        let t1 = t + step;
        let cur = f(t1);
        if prev.signum() != cur.signum() {
            // Bisseção.
            let (mut a, mut b) = (t, t1);
            for _ in 0..40 {
                let m = 0.5 * (a + b);
                if f(m).signum() == prev.signum() {
                    a = m;
                } else {
                    b = m;
                }
            }
            // Cruzando de baixo para cima do plano (produto escalar ficando positivo)
            // corresponde a lançar rumo ao nordeste para uma órbita prógrada.
            let rising = cur > prev;
            if rising == northbound {
                out.push(0.5 * (a + b));
            }
        }
        prev = cur;
        t = t1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    #[test]
    fn contagem_ate_a_liberacao() {
        let mut v = catalog::build("medio_sso").unwrap();
        v.id = 1;
        v.site = Some(catalog::site("Alcântara"));
        let mut lc = LaunchControl::default();
        lc.setup(&v, 700.0);
        let wx = Weather { evolve: false, ..Weather::default() };
        let mut rng = Rng::new(7);
        lc.state = CountState::Counting;
        let mut released = false;
        let mut ignited = false;
        for _ in 0..2_000 {
            let out = lc.tick(&mut v, &wx, &mut rng, 1.0, true);
            if out.ignite_liquids {
                ignited = true;
                for s in &mut v.stages {
                    for e in &mut s.engines {
                        e.state = EngineState::Running;
                        e.spool = 1.0;
                    }
                }
            }
            if out.release {
                released = true;
                break;
            }
            if lc.state == CountState::Hold {
                // Espera programada: vota e continua.
                assert!(lc.all_go(), "{:?}", lc.criteria.iter().filter(|c| !c.ok).collect::<Vec<_>>());
                lc.state = CountState::Counting;
            }
        }
        assert!(ignited && released);
    }

    #[test]
    fn vento_forte_segura_a_contagem() {
        let mut v = catalog::build("vsb30").unwrap();
        v.id = 1;
        let mut lc = LaunchControl::default();
        lc.setup(&v, 400.0);
        let wx = Weather { surface_wind: 25.0, evolve: false, ..Weather::default() };
        let mut rng = Rng::new(1);
        lc.state = CountState::Counting;
        for _ in 0..100 {
            lc.tick(&mut v, &wx, &mut rng, 1.0, true);
        }
        assert_eq!(lc.state, CountState::Hold);
        assert!(lc.hold_reason.contains("Vento"), "{}", lc.hold_reason);
    }
}
