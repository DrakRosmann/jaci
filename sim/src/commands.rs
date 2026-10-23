//! Dicionário de telecomandos: o que existe, para que tipo de veículo, com que
//! argumentos e quais exigem confirmação em dois passos (perigosos).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Command {
    pub id: String,
    #[serde(default)]
    pub args: serde_json::Map<String, serde_json::Value>,
}

impl Command {
    pub fn new(id: &str) -> Self {
        Command { id: id.into(), args: Default::default() }
    }
    pub fn with(mut self, k: &str, v: impl Into<serde_json::Value>) -> Self {
        self.args.insert(k.into(), v.into());
        self
    }
    pub fn num(&self, k: &str) -> Option<f64> {
        self.args.get(k).and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.replace(',', ".").parse().ok())))
    }
    pub fn str(&self, k: &str) -> Option<&str> {
        self.args.get(k).and_then(|v| v.as_str())
    }
    pub fn flag(&self, k: &str) -> Option<bool> {
        self.args.get(k).and_then(|v| v.as_bool().or_else(|| v.as_str().map(|s| matches!(s, "on" | "true" | "1" | "sim"))))
    }
    /// Texto curto para o histórico.
    pub fn describe(&self) -> String {
        if self.args.is_empty() {
            self.id.clone()
        } else {
            let a: Vec<String> = self.args.iter().map(|(k, v)| format!("{k}={}", v.to_string().trim_matches('"'))).collect();
            format!("{} {}", self.id, a.join(" "))
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ArgDef {
    pub name: &'static str,
    pub label: &'static str,
    /// number, bool, enum, text, vehicle, utc
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<(&'static str, &'static str)>,
    pub optional: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct CmdDef {
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    pub description: &'static str,
    pub args: Vec<ArgDef>,
    /// Exige armar antes de executar.
    pub hazardous: bool,
    /// Comando de solo: não passa pelo enlace com o veículo.
    pub ground: bool,
    /// Tipos de veículo a que se aplica (vazio = todos).
    pub kinds: Vec<&'static str>,
}

fn a(name: &'static str, label: &'static str, kind: &'static str) -> ArgDef {
    ArgDef { name, label, kind, unit: None, options: vec![], optional: false }
}
fn n(name: &'static str, label: &'static str, unit: &'static str) -> ArgDef {
    ArgDef { unit: Some(unit), ..a(name, label, "number") }
}
fn e(name: &'static str, label: &'static str, options: Vec<(&'static str, &'static str)>) -> ArgDef {
    ArgDef { options, ..a(name, label, "enum") }
}
fn opt(mut d: ArgDef) -> ArgDef {
    d.optional = true;
    d
}

const ON_OFF: [(&str, &str); 2] = [("on", "Ligar"), ("off", "Desligar")];

pub fn att_modes() -> Vec<(&'static str, &'static str)> {
    vec![
        ("hold", "Manter"),
        ("prograde", "Prógrado"),
        ("retrograde", "Retrógrado"),
        ("normal", "Normal"),
        ("antinormal", "Antinormal"),
        ("radial_out", "Radial para fora"),
        ("radial_in", "Radial para dentro"),
        ("sun", "Apontado ao Sol"),
        ("nadir", "Nadir"),
        ("earth", "Apontado à Terra"),
        ("target", "Alvo"),
        ("detumble", "Amortecimento"),
        ("free", "Livre"),
    ]
}

/// O dicionário completo.
pub fn dictionary() -> Vec<CmdDef> {
    let launch = vec!["sounding", "satellite", "capsule", "shuttle", "probe"];
    let crewed = vec!["capsule", "shuttle", "station"];
    let all: Vec<&'static str> = vec![];
    let def = |id, name, group, description, args: Vec<ArgDef>, hazardous, ground, kinds: &Vec<&'static str>| CmdDef {
        id,
        name,
        group,
        description,
        args,
        hazardous,
        ground,
        kinds: kinds.clone(),
    };
    vec![
        // Lançamento (solo)
        def("count_start", "Iniciar/retomar contagem", "Lançamento", "Começa a contagem regressiva (ou retoma depois de uma espera)", vec![opt(n("t_minus", "Começar em T-", "s"))], false, true, &launch),
        def("count_hold", "Espera", "Lançamento", "Segura a contagem", vec![], false, true, &launch),
        def("count_recycle", "Recomeçar em T-10 min", "Lançamento", "Volta a contagem para T-10 min", vec![], false, true, &launch),
        def("count_scrub", "Cancelar o lançamento", "Lançamento", "Cancela a tentativa de hoje e esvazia os tanques", vec![], true, true, &launch),
        def("poll", "Votação GO/NO-GO", "Lançamento", "Pergunta a cada posição se está GO", vec![], false, true, &launch),
        def("vote", "Voto GO/NO-GO", "Lançamento", "Resposta de uma posição ocupada por uma pessoa na votação", vec![a("position", "Posição", "text"), e("go", "Voto", vec![("go", "GO"), ("nogo", "NO-GO")])], false, true, &launch),
        def("position", "Posição GO/NO-GO", "Lançamento", "Força o estado de uma posição do console", vec![a("position", "Posição", "text"), e("go", "Estado", vec![("go", "GO"), ("nogo", "NO-GO"), ("auto", "Automático")])], false, true, &launch),
        def("load", "Carregamento de propelente", "Lançamento", "Liga ou para o carregamento dos tanques criogênicos", vec![e("on", "Carregamento", ON_OFF.to_vec())], false, true, &launch),
        def("window", "Janela de lançamento", "Lançamento", "Ajusta o T-0 para o plano de outro veículo (encontro) ou para uma hora", vec![opt(a("target", "Veículo alvo", "vehicle")), opt(a("utc", "T-0 (UTC)", "utc"))], false, true, &launch),
        def("fts_arm", "Armar terminação de voo", "Segurança de voo", "Arma o sistema de terminação de voo", vec![], false, true, &launch),
        def("fts_safe", "Desarmar terminação de voo", "Segurança de voo", "Coloca o sistema de terminação em segurança", vec![], false, true, &launch),
        def("fts_auto", "Terminação autônoma", "Segurança de voo", "Termina sozinho se o ponto de impacto sair do corredor", vec![e("on", "Autônoma", ON_OFF.to_vec())], false, true, &launch),
        def("fts_terminate", "TERMINAR O VOO", "Segurança de voo", "Destrói o veículo em voo (com tripulação, o escape dispara antes)", vec![], true, true, &launch),
        def("abort", "Aborto", "Lançamento", "Aborta a subida no modo escolhido", vec![e("mode", "Modo", vec![("les", "Sistema de escape"), ("separate", "Separar e reentrar"), ("ato", "Aborto para órbita (ATO)"), ("aoa", "Uma volta e reentrada (AOA)"), ("tal", "Pouso transoceânico (TAL)"), ("rtls", "Retorno ao local (RTLS)")])], true, true, &launch),
        // Voo
        def("stage", "Separar estágio", "Voo", "Larga o estágio de baixo (ou os boosters)", vec![], true, false, &all),
        def("ignite", "Ignição", "Voo", "Acende os motores do estágio ativo", vec![], false, false, &all),
        def("shutdown", "Corte dos motores", "Voo", "Desliga os motores líquidos acesos", vec![], false, false, &all),
        def("throttle", "Potência dos motores", "Voo", "Potência dos motores líquidos (%)", vec![n("value", "Potência", "%")], false, false, &all),
        def("engine_off", "Desligar um motor", "Voo", "Desliga um motor do estágio ativo", vec![n("engine", "Motor nº", "")], true, false, &all),
        def("jettison", "Descartar peça", "Voo", "Larga a coifa ou a torre de escape", vec![e("item", "Peça", vec![("fairing", "Coifa"), ("les", "Torre de escape")])], true, false, &all),
        def("payload_sep", "Separar a carga útil", "Voo", "Separa a nave dos estágios do lançador", vec![], true, false, &all),
        def("auto_stage", "Sequenciador automático", "Voo", "Separação e ignição automáticas dos estágios", vec![e("on", "Automático", ON_OFF.to_vec())], false, false, &all),
        // Atitude
        def("att_mode", "Modo de atitude", "Atitude", "Escolhe para onde a nave aponta", vec![e("mode", "Modo", att_modes()), opt(a("target", "Alvo", "vehicle"))], false, false, &all),
        def("desat", "Descarregar rodas", "Atitude", "Descarrega o momento das rodas de reação (bobinas ou RCS)", vec![], false, false, &all),
        def("att_nudge", "Atitude manual", "Atitude", "Pilotagem manual: gira a atitude alvo em graus, no referencial da nave", vec![opt(n("pitch", "Arfagem", "°")), opt(n("yaw", "Guinada", "°")), opt(n("roll", "Rolamento", "°"))], false, false, &all),
        def("pdi", "Descida propulsada", "Reentrada", "Liga o programa de pouso com motor: freia, desce e pousa nas pernas (Lua, Marte)", vec![], false, false, &all),
        def("rcs_pulse", "Pulso de RCS", "Atitude", "Translação curta com os propulsores de controle", vec![e("dir", "Direção", vec![("prograde", "Prógrado"), ("retrograde", "Retrógrado"), ("radial_out", "Radial +"), ("radial_in", "Radial −"), ("normal", "Normal"), ("antinormal", "Antinormal")]), n("dv", "Δv", "m/s")], false, false, &all),
        // Manobras
        def("burn", "Queima programada", "Manobras", "Acrescenta uma queima com Δv em RTN num instante", vec![n("t", "Daqui a", "s"), n("dv_r", "Δv radial", "m/s"), n("dv_t", "Δv transversal", "m/s"), n("dv_n", "Δv normal", "m/s"), opt(a("label", "Nome", "text"))], false, false, &all),
        def("burn_delete", "Apagar queima", "Manobras", "Remove uma queima da fila", vec![n("node", "Queima nº", "")], false, false, &all),
        def("burn_clear", "Apagar todas as queimas", "Manobras", "Esvazia a fila de queimas", vec![], false, false, &all),
        def("dock", "Aproximar e acoplar", "Encontro", "Aproximação automática pela V-bar até a porta do alvo (a menos de 8 km)", vec![a("target", "Alvo", "vehicle")], false, false, &vec!["capsule", "shuttle", "satellite"]),
        def("undock", "Desacoplar", "Encontro", "Solta da porta e se afasta a 0,1 m/s", vec![], true, false, &all),
        def("crew_transfer", "Transferir tripulação", "Encontro", "Passa tripulantes para o veículo acoplado", vec![n("count", "Tripulantes", "")], false, false, &crewed),
        // Energia, térmico, comunicações
        def("load_switch", "Chave de carga", "Energia", "Liga ou desliga uma carga elétrica", vec![a("load", "Carga", "text"), e("on", "Estado", ON_OFF.to_vec())], false, false, &all),
        def("solar_deploy", "Abrir painéis solares", "Energia", "Abre os painéis solares", vec![], false, false, &all),
        def("fuel_cell", "Célula a combustível", "Energia", "Liga ou desliga a célula a combustível", vec![e("on", "Estado", ON_OFF.to_vec())], false, false, &vec!["shuttle"]),
        def("heater", "Aquecedor", "Térmico", "Controle de um aquecedor", vec![a("node", "Nó", "text"), e("mode", "Modo", vec![("auto", "Automático"), ("on", "Ligado"), ("off", "Desligado")])], false, false, &all),
        def("radiator", "Radiadores", "Térmico", "Liga ou desliga os radiadores (abrir as portas do compartimento no ônibus espacial)", vec![e("on", "Estado", ON_OFF.to_vec())], false, false, &all),
        def("tx", "Transmissor", "Comunicações", "Liga ou desliga o transmissor", vec![e("on", "Estado", ON_OFF.to_vec())], false, false, &all),
        def("rate", "Taxa de dados", "Comunicações", "Fixa a taxa de dados ou deixa automática", vec![opt(n("kbps", "Taxa", "kbit/s"))], false, false, &all),
        def("dump", "Descarregar gravador", "Comunicações", "Transmite o gravador de bordo para o solo", vec![], false, false, &all),
        def("hga_deploy", "Abrir antena de alto ganho", "Comunicações", "Abre a antena de alto ganho", vec![], false, false, &all),
        def("instrument", "Instrumento", "Carga útil", "Liga ou desliga um instrumento", vec![a("id", "Instrumento", "text"), e("on", "Estado", ON_OFF.to_vec())], false, false, &all),
        // Suporte de vida
        def("o2_gen", "Gerador de O₂", "Suporte de vida", "Eletrólise de água", vec![e("on", "Estado", ON_OFF.to_vec())], false, false, &crewed),
        def("scrubber", "Remoção de CO₂", "Suporte de vida", "Liga ou desliga a remoção de CO₂", vec![e("on", "Estado", ON_OFF.to_vec())], false, false, &crewed),
        def("auto_press", "Repressurização automática", "Suporte de vida", "Repõe O₂ e N₂ dos tanques", vec![e("on", "Estado", ON_OFF.to_vec())], false, false, &crewed),
        def("hatch", "Escotilha", "Suporte de vida", "Fecha ou abre a escotilha de um módulo (isola um vazamento)", vec![a("module", "Módulo", "text"), e("closed", "Escotilha", vec![("on", "Fechar"), ("off", "Abrir")])], false, false, &crewed),
        // Reentrada
        def("chute", "Paraquedas", "Reentrada", "Abre ou corta um paraquedas", vec![n("index", "Paraquedas nº", ""), e("action", "Ação", vec![("deploy", "Abrir"), ("cut", "Cortar")])], true, false, &all),
        def("auto_chutes", "Sequência automática de reentrada", "Reentrada", "Programa de reentrada, separação do módulo de serviço e paraquedas automáticos", vec![e("on", "Automática", ON_OFF.to_vec())], false, false, &all),
        def("landing_target", "Local de pouso", "Reentrada", "Alvo do guiamento de reentrada", vec![n("lat", "Latitude", "°"), n("lon", "Longitude", "°")], false, false, &vec!["capsule", "shuttle"]),
        def("entry_mode", "Guiamento de reentrada", "Reentrada", "Guiada pelo preditor ou balística", vec![e("mode", "Modo", vec![("guided", "Guiada"), ("ballistic", "Balística")])], false, false, &vec!["capsule", "shuttle"]),
        def("gear", "Trem de pouso", "Reentrada", "Baixa o trem de pouso", vec![], false, false, &vec!["shuttle"]),
        // FDIR
        def("safe_mode", "Modo de segurança", "Proteção", "Corta cargas não essenciais e aponta ao Sol", vec![e("on", "Estado", ON_OFF.to_vec())], false, false, &all),
    ]
}

pub fn find(id: &str) -> Option<CmdDef> {
    dictionary().into_iter().find(|d| d.id == id)
}
