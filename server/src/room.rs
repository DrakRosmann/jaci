//! Sala de controle com várias pessoas: cada uma entra numa posição (FLIGHT,
//! FIDO, EECOM, …) e só comanda o que é dela. Com a sala desligada (o padrão)
//! qualquer tela faz tudo, como um console só.
//!
//! A tabela abaixo é a única fonte das permissões: o servidor confere cada
//! mensagem por ela e a manda na saudação, para a interface apagar o que a
//! posição não usa.

use serde_json::{Value, json};

pub struct Role {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Grupos do dicionário de comandos que a posição comanda.
    pub groups: &'static [&'static str],
    /// Posições da votação GO/NO-GO do lançamento que ela ocupa.
    pub launch: &'static [&'static str],
    /// Ações do simulador ("sim") que ela pode pedir.
    pub sim: &'static [&'static str],
}

/// Ações do simulador que qualquer posição (menos o observador) pode pedir.
const ANYONE: &[&str] = &["ack", "proc_log", "saves", "recordings"];

pub const ROLES: &[Role] = &[
    Role {
        id: "FLIGHT",
        name: "Diretor de voo",
        description: "Comanda tudo e decide; o único que aprova planos de manobra além do FIDO",
        groups: &["*"],
        launch: &["fd"],
        sim: &["pause", "resume", "warp", "warp_stop", "record_start", "record_stop", "room_mode", "script_enable"],
    },
    Role {
        id: "LCC",
        name: "Diretor de lançamento",
        description: "Contagem, votação, carregamento, janela e sistemas de solo",
        groups: &["Lançamento", "KSP: lançamento"],
        launch: &["ld", "gs"],
        sim: &[],
    },
    Role {
        id: "BOOSTER",
        name: "Propulsão e veículo lançador",
        description: "Motores, estágios e potência",
        groups: &["Voo", "KSP: voo"],
        launch: &["lv", "prop"],
        sim: &[],
    },
    Role {
        id: "FIDO",
        name: "Dinâmica de voo",
        description: "Manobras, encontro, reentrada e o planejador",
        groups: &["Manobras", "Encontro", "Reentrada", "KSP: manobras"],
        launch: &[],
        sim: &[],
    },
    Role { id: "GNC", name: "Guiamento e atitude", description: "Modos de atitude, rodas e RCS", groups: &["Atitude", "KSP: atitude"], launch: &[], sim: &[] },
    Role {
        id: "EECOM",
        name: "Energia, térmico e suporte de vida",
        description: "Cargas, painéis, aquecedores, cabine e modo de segurança",
        groups: &["Energia", "Térmico", "Suporte de vida", "Proteção", "KSP: sistemas"],
        launch: &[],
        sim: &[],
    },
    Role { id: "INCO", name: "Comunicações", description: "Rádio, taxas, gravador e estações; satélites reais", groups: &["Comunicações"], launch: &["com"], sim: &["station", "track", "untrack"] },
    Role { id: "PAYLOAD", name: "Carga útil", description: "Instrumentos e separação da carga", groups: &["Carga útil"], launch: &["pl"], sim: &[] },
    Role { id: "RSO", name: "Segurança de voo", description: "Terminação de voo e corredor", groups: &["Segurança de voo"], launch: &["rso"], sim: &[] },
    Role { id: "METEO", name: "Meteorologia", description: "Vota o tempo na contagem", groups: &[], launch: &["wx"], sim: &[] },
    Role { id: "CAPCOM", name: "Comunicador com a tripulação", description: "Fala com a tripulação e vota por ela", groups: &[], launch: &["crew"], sim: &[] },
    Role { id: "SURGEON", name: "Médico de voo", description: "Saúde da tripulação; vota na contagem", groups: &[], launch: &["surg"], sim: &[] },
    Role { id: "SIM", name: "Instrutor", description: "Cenários, tempo, clima, falhas, gravações e relatório; não comanda os veículos", groups: &[], launch: &[], sim: &["*"] },
    Role { id: "OBS", name: "Observador", description: "Só acompanha", groups: &[], launch: &[], sim: &[] },
];

pub fn role(id: &str) -> Option<&'static Role> {
    ROLES.iter().find(|r| r.id == id)
}

/// Pode mandar este comando (do grupo `group`)?
pub fn can_command(position: &str, group: &str) -> bool {
    role(position).is_some_and(|r| r.groups.iter().any(|g| *g == "*" || *g == group))
}

/// Pode pedir esta ação do simulador?
pub fn can_sim(position: &str, action: &str) -> bool {
    let Some(r) = role(position) else { return false };
    if r.id == "OBS" {
        return false;
    }
    r.sim.iter().any(|a| *a == "*" || *a == action) || ANYONE.contains(&action)
}

/// Pode votar (ou forçar) esta posição do lançamento? FLIGHT e LCC forçam qualquer uma.
pub fn can_vote(position: &str, launch_position: &str) -> bool {
    matches!(position, "FLIGHT" | "LCC") || role(position).is_some_and(|r| r.launch.contains(&launch_position))
}

pub fn roles_json() -> Value {
    json!(ROLES.iter().map(|r| json!({"id": r.id, "name": r.name, "description": r.description, "groups": r.groups, "launch": r.launch, "sim": r.sim})).collect::<Vec<_>>())
}

/// Uma pessoa na sala (por conexão).
#[derive(Clone, Debug)]
pub struct Member {
    pub name: String,
    pub position: String,
}

impl Member {
    pub fn label(&self) -> String {
        format!("{} ({})", self.name, self.position)
    }
}

/// Nome limpo e curto.
pub fn clean(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).take(max).collect::<String>().trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissoes() {
        assert!(can_command("FLIGHT", "Segurança de voo"));
        assert!(can_command("RSO", "Segurança de voo"));
        assert!(!can_command("EECOM", "Segurança de voo"));
        assert!(can_command("EECOM", "Energia"));
        assert!(!can_command("SIM", "Energia"), "o instrutor não comanda veículo");
        assert!(!can_command("OBS", "Energia"));
        assert!(!can_command("NINGUEM", "Energia"));
        assert!(can_sim("SIM", "scenario"));
        assert!(can_sim("FLIGHT", "warp"));
        assert!(!can_sim("FLIGHT", "scenario"));
        assert!(can_sim("EECOM", "ack"));
        assert!(!can_sim("OBS", "ack"));
        assert!(can_sim("INCO", "track"));
        assert!(can_vote("METEO", "wx"));
        assert!(!can_vote("METEO", "rso"));
        assert!(can_vote("LCC", "rso"));
        // Toda posição do lançamento tem alguém que vota por ela.
        for (id, _) in jaci_sim::launch::POSITIONS {
            assert!(ROLES.iter().any(|r| r.launch.contains(&id)), "{id} sem dono");
        }
    }
}
