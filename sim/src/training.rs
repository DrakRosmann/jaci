//! Treino: falhas planejadas pelo instrutor (numa hora, depois do lançamento,
//! sorteadas ou não) e o relatório do treino (reação aos alarmes, detecção e
//! resposta às falhas, procedimentos e desfecho), para o debriefing.

use crate::{
    telemetry::LimitState,
    vehicle::{Phase, VehicleId},
    world::World,
};
use serde::{Deserialize, Serialize};

/// Quando uma falha planejada dispara.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum FaultWhen {
    /// Num instante (relógio da simulação).
    At(f64),
    /// Tantos segundos depois do lançamento do veículo (T+).
    Met(f64),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum PlanState {
    /// Espera; `offset` é o atraso sorteado dentro da janela.
    Pending { offset: f64 },
    Done { t: f64, message: String },
    /// O sorteio decidiu que não acontece.
    Skipped,
    Failed { t: f64, error: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlannedFault {
    pub id: u32,
    pub vehicle: VehicleId,
    /// Id da falha (veja `world::FAULTS`) ou "random" para sortear uma que sirva.
    pub fault: String,
    pub arg: Option<String>,
    pub when: FaultWhen,
    /// Janela (s): o instante é sorteado entre `when` e `when + window`.
    pub window: f64,
    /// Chance de acontecer (0–1), sorteada ao planejar.
    pub probability: f64,
    /// Avisa a equipe no registro de eventos (senão, ela tem de descobrir).
    pub announce: bool,
    pub state: PlanState,
}

/// Alarme no histórico do treino (a lista de alarmes da tela some com os reconhecidos).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlarmLog {
    pub id: u64,
    pub t: f64,
    pub vehicle: VehicleId,
    pub name: String,
    pub level: LimitState,
    pub acked_at: Option<f64>,
    pub cleared_at: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FaultLog {
    pub t: f64,
    pub vehicle: VehicleId,
    pub fault: String,
    pub message: String,
    pub planned: bool,
}

/// Passo de procedimento informado pelos consoles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcLog {
    pub t: f64,
    pub vehicle: VehicleId,
    pub procedure: String,
    /// "start", "step" ou "done".
    pub kind: String,
    pub step: Option<u32>,
    pub total: Option<u32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrainingLog {
    pub plan: Vec<PlannedFault>,
    pub next_plan_id: u32,
    pub alarms: Vec<AlarmLog>,
    pub faults: Vec<FaultLog>,
    pub procedures: Vec<ProcLog>,
    /// Início do treino (relógio da simulação).
    pub start: Option<f64>,
}

/// Falhas que o sorteio tenta, em ordem aleatória, quando o tipo é "random".
const RANDOM_FAULTS: &[&str] = &["engine", "leak", "battery_cell", "solar_stuck", "solar_degrade", "wheel", "gyro", "star_tracker", "comms", "heater", "cabin_leak", "scrubber", "fuel_cell"];

impl World {
    /// Planeja uma falha; o sorteio (acontece? quando, na janela?) é feito agora.
    pub fn plan_fault(&mut self, vehicle: VehicleId, fault: &str, arg: Option<String>, when: FaultWhen, window: f64, probability: f64, announce: bool) -> u32 {
        let p = probability.clamp(0.0, 1.0);
        let happens = self.rng.uniform() < p;
        let offset = self.rng.uniform() * window.max(0.0);
        let log = &mut self.training;
        log.next_plan_id += 1;
        let id = log.next_plan_id;
        log.plan.push(PlannedFault {
            id,
            vehicle,
            fault: fault.to_string(),
            arg,
            when,
            window: window.max(0.0),
            probability: p,
            announce,
            state: if happens { PlanState::Pending { offset } } else { PlanState::Skipped },
        });
        id
    }

    pub fn unplan_fault(&mut self, id: u32) -> bool {
        let n = self.training.plan.len();
        self.training.plan.retain(|f| f.id != id);
        n != self.training.plan.len()
    }

    /// Instante em que uma falha planejada dispara (se já dá para saber).
    pub fn plan_due(&self, f: &PlannedFault) -> Option<f64> {
        let PlanState::Pending { offset } = f.state else { return None };
        match f.when {
            FaultWhen::At(t) => Some(t + offset),
            FaultWhen::Met(met) => self.vehicle(f.vehicle).and_then(|v| v.launch_time).map(|l| l + met + offset),
        }
    }

    /// Dispara as falhas planejadas que chegaram na hora.
    pub(crate) fn run_fault_plan(&mut self) {
        let due: Vec<u32> = self.training.plan.iter().filter(|f| self.plan_due(f).is_some_and(|at| self.t >= at)).map(|f| f.id).collect();
        for id in due {
            let Some(f) = self.training.plan.iter().find(|f| f.id == id).cloned() else { continue };
            let result = if f.fault == "random" {
                let mut order: Vec<&str> = RANDOM_FAULTS.to_vec();
                for i in (1..order.len()).rev() {
                    let j = (self.rng.uniform() * (i + 1) as f64) as usize;
                    order.swap(i, j.min(i));
                }
                order.into_iter().find_map(|k| self.inject_as(f.vehicle, k, f.arg.as_deref(), f.announce, true).ok()).ok_or_else(|| "nenhuma falha serve a este veículo".to_string())
            } else {
                self.inject_as(f.vehicle, &f.fault, f.arg.as_deref(), f.announce, true)
            };
            let t = self.t;
            if let Some(p) = self.training.plan.iter_mut().find(|p| p.id == id) {
                p.state = match result {
                    Ok(message) => PlanState::Done { t, message },
                    Err(error) => PlanState::Failed { t, error },
                };
            }
        }
    }

    /// Injeta uma falha e registra no treino; `announce` põe o aviso no registro de eventos.
    pub fn inject_as(&mut self, vid: VehicleId, fault: &str, arg: Option<&str>, announce: bool, planned: bool) -> Result<String, String> {
        // Sem aviso, a equipe tem de descobrir a falha pelos sintomas.
        self.quiet_fault = !announce;
        let r = self.inject(vid, fault, arg);
        self.quiet_fault = false;
        let msg = r?;
        let t = self.t;
        self.training.start.get_or_insert(t);
        self.training.faults.push(FaultLog { t, vehicle: vid, fault: fault.to_string(), message: msg.clone(), planned });
        Ok(msg)
    }

    /// Reconhece um alarme (ou todos) e anota a hora.
    pub fn ack(&mut self, which: Option<u64>) {
        let t = self.t;
        for al in &mut self.alarms {
            if which.is_none_or(|n| n == al.id) && !al.acked {
                al.acked = true;
                if let Some(l) = self.training.alarms.iter_mut().find(|l| l.id == al.id) {
                    l.acked_at.get_or_insert(t);
                }
            }
        }
    }

    pub fn log_procedure(&mut self, vehicle: VehicleId, procedure: &str, kind: &str, step: Option<u32>, total: Option<u32>) {
        let t = self.t;
        self.training.procedures.push(ProcLog { t, vehicle, procedure: procedure.to_string(), kind: kind.to_string(), step, total });
        let name = self.vehicle(vehicle).map(|v| v.name.clone()).unwrap_or_default();
        match kind {
            "start" => self.event(Some(vehicle), crate::vehicle::Level::Info, format!("Procedimento iniciado: {procedure} ({name})")),
            "done" => self.event(Some(vehicle), crate::vehicle::Level::Good, format!("Procedimento concluído: {procedure} ({name})")),
            _ => {}
        }
    }

    /// Recomeça o relatório (as falhas planejadas pendentes continuam).
    pub fn reset_training(&mut self) {
        let plan = std::mem::take(&mut self.training.plan);
        let next = self.training.next_plan_id;
        self.training = TrainingLog { plan, next_plan_id: next, start: Some(self.t), ..Default::default() };
    }
}

// ── Relatório ──

#[derive(Clone, Debug, Serialize)]
pub struct AlarmReport {
    pub t: f64,
    pub vehicle: String,
    pub name: String,
    pub level: LimitState,
    /// Segundos até o reconhecimento (None: não reconhecido).
    pub reaction: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct FaultReport {
    pub t: f64,
    pub vehicle: String,
    pub fault: String,
    pub message: String,
    pub planned: bool,
    /// Segundos até o primeiro alarme do veículo depois da falha.
    pub detected: Option<f64>,
    /// Segundos até o primeiro comando ao veículo depois da falha.
    pub responded: Option<f64>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProcReport {
    pub procedure: String,
    pub vehicle: String,
    pub started: f64,
    pub finished: Option<f64>,
    pub steps: u32,
    pub total: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Score {
    /// 0–100, indicativa.
    pub total: f64,
    /// Média da reação aos alarmes (1 até 30 s, 0 a partir de 180 s ou sem reconhecer).
    pub reaction: Option<f64>,
    /// Resposta às falhas (1 com comando em até 2 min, 0,5 em até 10 min).
    pub response: Option<f64>,
    /// Multiplicador do desfecho (0,5 com veículo perdido).
    pub outcome: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub mission: String,
    pub from: f64,
    pub to: f64,
    pub alarms: Vec<AlarmReport>,
    pub faults: Vec<FaultReport>,
    pub procedures: Vec<ProcReport>,
    pub commands: usize,
    pub vehicles: Vec<(String, String)>,
    pub lost: Vec<String>,
    pub score: Score,
}

/// Nota de reação a um alarme: 1 até 30 s, cai até 0 em 180 s.
pub fn reaction_score(secs: Option<f64>) -> f64 {
    match secs {
        Some(s) if s <= 30.0 => 1.0,
        Some(s) if s < 180.0 => 1.0 - (s - 30.0) / 150.0,
        _ => 0.0,
    }
}

impl World {
    pub fn report(&self) -> Report {
        let log = &self.training;
        let from = log.start.unwrap_or(self.t);
        let name = |id: VehicleId| self.vehicle(id).map(|v| v.name.clone()).or_else(|| self.externals.iter().find(|e| e.id == id).map(|e| e.name.clone())).unwrap_or(format!("#{id}"));
        let alarms: Vec<AlarmReport> = log
            .alarms
            .iter()
            .map(|a| AlarmReport { t: a.t, vehicle: name(a.vehicle), name: a.name.clone(), level: a.level, reaction: a.acked_at.map(|x| x - a.t) })
            .collect();
        let faults: Vec<FaultReport> = log
            .faults
            .iter()
            .map(|f| {
                let detected = log.alarms.iter().filter(|a| a.vehicle == f.vehicle && a.t >= f.t).map(|a| a.t - f.t).reduce(f64::min);
                let responded = self.commands.iter().filter(|c| c.vehicle == f.vehicle && c.sent >= f.t).map(|c| c.sent - f.t).reduce(f64::min);
                FaultReport { t: f.t, vehicle: name(f.vehicle), fault: f.fault.clone(), message: f.message.clone(), planned: f.planned, detected, responded }
            })
            .collect();
        let mut procedures: Vec<ProcReport> = Vec::new();
        for p in &log.procedures {
            let who = name(p.vehicle);
            match p.kind.as_str() {
                "start" => procedures.push(ProcReport { procedure: p.procedure.clone(), vehicle: who, started: p.t, finished: None, steps: 0, total: p.total }),
                kind => {
                    if let Some(r) = procedures.iter_mut().rev().find(|r| r.procedure == p.procedure && r.vehicle == who && r.finished.is_none()) {
                        if kind == "step" {
                            r.steps += 1;
                        } else {
                            r.finished = Some(p.t);
                        }
                    }
                }
            }
        }
        let commands = self.commands.iter().filter(|c| c.sent >= from).count();
        let vehicles: Vec<(String, String)> = self.vehicles.iter().filter(|v| v.kind != crate::vehicle::Kind::Debris).map(|v| (v.name.clone(), v.phase.name().to_string())).collect();
        let lost: Vec<String> = self.vehicles.iter().filter(|v| v.kind != crate::vehicle::Kind::Debris && v.phase == Phase::Destroyed).map(|v| v.name.clone()).collect();
        let reaction = (!alarms.is_empty()).then(|| alarms.iter().map(|a| reaction_score(a.reaction)).sum::<f64>() / alarms.len() as f64);
        let response = (!faults.is_empty()).then(|| {
            faults
                .iter()
                .map(|f| match f.responded {
                    Some(s) if s <= 120.0 => 1.0,
                    Some(s) if s <= 600.0 => 0.5,
                    _ => 0.0,
                })
                .sum::<f64>()
                / faults.len() as f64
        });
        let outcome = if lost.is_empty() { 1.0 } else { 0.5 };
        let parts: Vec<f64> = [reaction, response].into_iter().flatten().collect();
        let base = if parts.is_empty() { 1.0 } else { parts.iter().sum::<f64>() / parts.len() as f64 };
        Report {
            mission: self.mission.clone(),
            from,
            to: self.t,
            alarms,
            faults,
            procedures,
            commands,
            vehicles,
            lost,
            score: Score { total: (100.0 * base * outcome).round(), reaction, response, outcome },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{commands::Command, scenarios, time::from_calendar};

    fn station() -> (World, VehicleId) {
        let w = scenarios::build("estacao", from_calendar(2026, 10, 19, 15, 0, 0.0), 7).unwrap();
        let id = w.vehicles.iter().find(|v| v.kind == crate::vehicle::Kind::Station).unwrap().id;
        (w, id)
    }

    fn run(w: &mut World, secs: f64) {
        let end = w.t + secs;
        while w.t < end {
            w.step(1.0);
        }
    }

    #[test]
    fn falha_planejada_silenciosa_dispara_na_hora() {
        let (mut w, id) = station();
        let t0 = w.t;
        w.plan_fault(id, "cabin_leak", Some("1".into()), FaultWhen::At(t0 + 30.0), 0.0, 1.0, false);
        run(&mut w, 20.0);
        assert!(w.training.faults.is_empty(), "disparou cedo");
        let seq = w.event_seq;
        run(&mut w, 20.0);
        assert_eq!(w.training.faults.len(), 1);
        assert!(matches!(w.training.plan[0].state, PlanState::Done { .. }));
        // Silenciosa: nenhum aviso do instrutor no registro.
        assert!(!w.events.iter().any(|e| e.text.contains("[Instrutor]")), "aviso vazou");
        assert!(w.event_seq >= seq);
    }

    #[test]
    fn sorteio_e_janela() {
        let (mut w, id) = station();
        let t0 = w.t;
        // Probabilidade zero: nunca acontece.
        w.plan_fault(id, "comms", None, FaultWhen::At(t0), 0.0, 0.0, true);
        assert_eq!(w.training.plan[0].state, PlanState::Skipped);
        // Janela de 100 s: o instante cai dentro dela.
        w.plan_fault(id, "random", None, FaultWhen::At(t0 + 10.0), 100.0, 1.0, true);
        let due = w.plan_due(&w.training.plan[1]).unwrap();
        assert!(due >= t0 + 10.0 && due <= t0 + 110.0, "{}", due - t0);
        run(&mut w, 115.0);
        assert!(matches!(w.training.plan[1].state, PlanState::Done { .. }), "{:?}", w.training.plan[1].state);
        // Avisada: aparece no registro.
        assert!(w.events.iter().any(|e| e.text.contains("[Instrutor]")));
    }

    #[test]
    fn falha_em_t_mais_espera_o_lancamento() {
        let mut w = scenarios::build("sondagem", from_calendar(2026, 10, 19, 15, 0, 0.0), 7).unwrap();
        let id = w.launch.vehicle.unwrap();
        w.plan_fault(id, "engine", None, FaultWhen::Met(5.0), 0.0, 1.0, false);
        assert_eq!(w.plan_due(&w.training.plan[0]), None, "sem lançamento ainda não tem hora");
        w.send(id, Command { id: "count_start".into(), args: [("t_minus".to_string(), serde_json::json!(3.0))].into_iter().collect() }, false, None).unwrap();
        run(&mut w, 15.0);
        let launched = w.vehicle(id).unwrap().launch_time.expect("não lançou");
        match &w.training.plan[0].state {
            PlanState::Done { t, .. } => assert!((t - launched - 5.0).abs() < 1.5, "T+{}", t - launched),
            s => panic!("{s:?}"),
        }
    }

    #[test]
    fn relatorio_mede_reacao_e_resposta() {
        let (mut w, id) = station();
        w.inject_as(id, "cabin_leak", Some("1"), false, false).unwrap();
        // Corre até o primeiro alarme do vazamento.
        let mut n = 0;
        while w.training.alarms.is_empty() && n < 20_000 {
            w.step(1.0);
            n += 1;
        }
        assert!(!w.training.alarms.is_empty(), "o vazamento não gerou alarme");
        run(&mut w, 20.0);
        w.ack(None);
        w.send(id, Command { id: "safe_mode".into(), args: [("on".to_string(), serde_json::json!("on"))].into_iter().collect() }, false, None).ok();
        let r = w.report();
        assert_eq!(r.faults.len(), 1);
        assert!(r.faults[0].detected.is_some());
        assert!(r.faults[0].responded.is_some());
        let first = &r.alarms[0];
        assert!(first.reaction.unwrap() >= 19.0, "{:?}", first.reaction);
        assert!(r.score.total > 0.0 && r.score.total <= 100.0);
        assert_eq!(reaction_score(Some(10.0)), 1.0);
        assert_eq!(reaction_score(Some(105.0)), 0.5);
        assert_eq!(reaction_score(None), 0.0);
    }
}
