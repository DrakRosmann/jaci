# Missões em arquivos

Veículos, cenários e procedimentos em TOML, sem recompilar o Jaci. O servidor lê esta pasta ao
iniciar (`missoes/` ao lado de `estados/`; instalado, `~/.local/share/jaci/missoes`) e de novo a
cada arquivo salvo pelo editor do console **Instrutor**, que também confere e mostra os erros.

* `veiculos/<id>.toml` — parte de um veículo do catálogo (`base = "medio_sso"`) com ajustes, ou
  monta uma pilha nova de estágios (`kind` e `[[stages]]`). Veja `vsb30-carga-pesada.toml` e
  `lancador-leve.toml`.
* `cenarios/<id>.toml` — veículos na plataforma (`place = "pad"`) ou em órbita
  (`place = "orbit"`), hora de início, clima, aceleração, satélites reais (`tle`) e falhas
  planejadas para o treino (`[[faults]]`). Pode partir de um cenário pronto (`base = "estacao"`).
* `procedimentos/<id>.toml` — passos `note`, `check`, `cmd` (comando com `args`) e `wait` (espera
  uma condição, ex. `until = "eps.soc > 80 and phase == 'orbit'"`). Nas condições: pontos de
  telemetria pelo id (`nav.alt`, `eps.soc`…), `phase`, `kind`, `alt`, `speed`, `link`,
  `separated`, `inserted`, `launch.state`, `launch.t_minus`, `launch.all_go`; números, textos
  entre aspas, `<`, `<=`, `>`, `>=`, `==`, `!=`, `and`, `or`, `not` e parênteses.

O id é o nome do arquivo e não pode repetir um do catálogo ou um cenário pronto.
