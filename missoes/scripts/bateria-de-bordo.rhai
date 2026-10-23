// Regra de bordo: protege a bateria de todos os satélites e sondas.
// Abaixo de 25 % entra em modo de segurança sozinho (a bordo, sem esperar o
// enlace); acima de 70 % avisa o solo que dá para sair.
//
// Roda a cada segundo de simulação. `mem` guarda o estado entre uma vez e outra.

for nome in vehicles() {
    let v = vehicle(nome);
    let soc = tm(v, "eps.soc");
    if soc != soc { continue; }            // sem bateria (NaN): não é com ela
    let chave = `seguro ${v}`;
    if soc < 25.0 && !mem.contains(chave) {
        onboard(v, "safe_mode", #{ on: "on" });
        mem[chave] = true;
        warn(`${nome}: bateria em ${soc.round().to_int()} %, modo de segurança a bordo`);
    }
    if soc > 70.0 && mem.contains(chave) {
        mem.remove(chave);
        log(`${nome}: bateria recuperada (${soc.round().to_int()} %); o solo pode sair do modo de segurança`);
    }
}
