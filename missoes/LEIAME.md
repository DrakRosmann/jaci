# Missões em arquivos

Veículos, cenários e procedimentos em TOML, sem recompilar o Jaci. O servidor lê esta pasta ao
iniciar (`missoes/` ao lado de `estados/`; instalado, `~/.local/share/jaci/missoes`) e de novo a
cada arquivo salvo pelo editor do console **Instrutor**, que também confere e mostra os erros.

* `veiculos/<id>.toml` — parte de um veículo do catálogo (`base = "medio_sso"`) com ajustes, ou
  monta uma pilha nova de estágios (`kind` e `[[stages]]`). Veja `vsb30-carga-pesada.toml` e
  `lancador-leve.toml`. Em `[ascent]`, `peg = false` troca o PEG da malha fechada pela lei
  antiga.
  `recovery_from = "medio_capsula"` copia de um veículo do catálogo a recuperação (paraquedas,
  escudo térmico, torre de escape, pernas ou pista, portas de acoplamento e a forma de cápsula), e
  `landing` troca só o modo de pouso (`splash`, `legs`, `runway` ou `none`). O Hangar da
  interface gera esses arquivos (com o projeto num comentário `# hangar:` para reabrir).
* `cenarios/<id>.toml` — veículos na plataforma (`place = "pad"`) ou em órbita
  (`place = "orbit"`; com `t_minus` abaixo de 600 s os tanques criogênicos já começam cheios),
  hora de início, clima, aceleração, satélites reais (`tle`) e falhas
  planejadas para o treino (`[[faults]]`). Pode partir de um cenário pronto (`base = "estacao"`).
* `procedimentos/<id>.toml` — passos `note`, `check`, `cmd` (comando com `args`) e `wait` (espera
  uma condição, ex. `until = "eps.soc > 80 and phase == 'orbit'"`). Nas condições: pontos de
  telemetria pelo id (`nav.alt`, `eps.soc`…), `phase`, `kind`, `alt`, `speed`, `link`,
  `separated`, `inserted`, `launch.state`, `launch.t_minus`, `launch.all_go`; números, textos
  entre aspas, `<`, `<=`, `>`, `>=`, `==`, `!=`, `and`, `or`, `not` e parênteses.

* `scripts/<nome>.rhai` — automação em [Rhai](https://rhai.rs): roda inteiro a cada segundo de
  simulação, lê o estado e pede ações. Leitura: `t()`, `vehicles()`, `vehicle("nome")` (id, ou
  -1), `tm(id, "eps.soc")` (NaN sem o ponto), `phase(id)`, `met(id)`, `link(id)`,
  `alarm(id, "ponto")` (`"red"`, `"yellow"` ou vazio), `alarms(id)`, `count_state()`, `t_minus()`.
  Ações: `cmd(id, "comando", #{ arg: valor })` (de solo: passa pelo enlace, com o atraso da
  luz), `cmd_armed(…)` (perigoso), `onboard(id, "comando", #{…})` (regra de bordo: executa na hora,
  nunca perigoso), `log("…")`, `warn("…")`. `mem` guarda o que quiser de uma vez para a outra;
  `once("chave")` é verdadeiro só na primeira vez e `every(segundos, "chave")` de tempos em tempos.
  Liga e desliga no console **Instrutor**, ou pelo cenário (`scripts = ["nome"]`). Erro ou laço
  sem fim desliga o script, com o motivo no registro de eventos.

* `escola/<id>.toml` — lições da escola de controle: `scenario`, `intro`, `done`, `par_s` (tempo
  de referência das três estrelas), `vehicle` e `[[objectives]]` com `text`, `hint`, `until` (a
  mesma linguagem das condições, mais `ui.view`, `selected`, `warp`, `launch.polled`, `burns`,
  `alarms`, `acked`, `leak_isolated`, `safe_mode`, `att_mode` e, para o console PILOT, `manual`
  (pilotagem manual ligada), `program` (o programa de guiamento: `ascent`, `powereddescent`…,
  ou `none`), `docked`, `target_range` e `target_rate` (m e m/s até o alvo) e `fd_error` (graus
  entre o nariz e o diretor de voo)), `console` e `vehicle`.
* `ccsds/<nome>.xml` — dicionários de telemetria CCSDS em XTCE: cada contêiner com o APID vira um
  pacote de layout fixo, com unidades, calibração e alarmes (veja `cubesat.xml` e
  `docs/protocolo.md`).

O id é o nome do arquivo e não pode repetir um do catálogo ou um cenário pronto.
