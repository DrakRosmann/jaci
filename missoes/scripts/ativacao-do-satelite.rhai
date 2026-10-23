// Sequência automática de ativação depois da separação, comandada do solo
// (cada comando passa pelo enlace). Um passo por vez, guardado em mem.passo:
// amortecer a rotação, abrir os painéis, apontar ao Sol e, com a bateria
// carregando, apontar ao nadir e ligar a câmera.

let sat = vehicle("Satélite de observação");
if sat < 0 { return; }
if !mem.contains("passo") { mem.passo = 0; }
let separado = phase(sat) == "orbit" && tm(sat, "nav.dv") == tm(sat, "nav.dv");

switch mem.passo {
    0 => if separado && link(sat) {
        cmd(sat, "att_mode", #{ mode: "detumble" });
        log("passo 1: amortecer a rotação");
        mem.passo = 1;
    },
    1 => if tm(sat, "adcs.rate") < 0.5 {
        cmd(sat, "solar_deploy");
        log("passo 2: abrir os painéis");
        mem.passo = 2;
    },
    2 => if every(30.0, "espera painel") && link(sat) {
        cmd(sat, "att_mode", #{ mode: "sun" });
        log("passo 3: painéis ao Sol");
        mem.passo = 3;
    },
    3 => if tm(sat, "eps.soc") > 90.0 && link(sat) {
        cmd(sat, "att_mode", #{ mode: "nadir" });
        cmd(sat, "instrument", #{ id: "wfi", on: "on" });
        log("passo 4: nadir e câmera ligada; ativação concluída");
        mem.passo = 4;
    },
}
