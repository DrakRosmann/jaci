# Protocolo

## Interface ↔ servidor (WebSocket)

A interface abre `ws://<servidor>/ws` (com `?token=…` quando o servidor exige). Todas as mensagens
são JSON com um campo `type`. As que pedem resposta levam `req` (um número); a resposta volta com o
mesmo `req` só para quem pediu.

### Do servidor

| `type` | Quando | Conteúdo |
|---|---|---|
| `hello` | ao conectar | versão, cenários e catálogo (os de arquivo com `"file": true`), procedimentos dos arquivos, dicionário de comandos, falhas, estações, locais de lançamento, corpos, papéis da sala, últimos eventos e o dicionário de telemetria de cada veículo |
| `snapshot` | 5 vezes por segundo | instante (`t`, segundos desde J2000, e `utc`), aceleração, veículos (estado, telemetria `tm`, estágios, motores, queimas, guiamento, subsistemas), contagem, clima, alarmes, comandos recentes, eventos novos, corredor de segurança, posições dos corpos |
| `dictionary` | quando um veículo muda de forma (separa estágio, troca tripulação) | `dictionary`: id do veículo → lista de pontos (`id`, `name`, `unit`, `group`, `yellow`, `red`) |
| `command_result`, `plan`, `plan_applied`, `predict`, `passes`, `porkchop`, `saves`, `recordings`, `sim_result` | respostas | veja abaixo |
| `reload` | depois de carregar cenário ou estado, e ao entrar e sair da reprodução (para todas as interfaces) | a interface reconecta e recebe um `hello` novo |
| `library` | quando um arquivo de missão é salvo | `scenarios`, `catalog` e `procedures` novos (os mesmos campos da saudação) |
| `rewind` | na reprodução, ao pular para outro ponto | `events` e `dictionary` até ali e `history` (a telemetria, no formato de `history`): a interface recomeça eventos e gráficos |

### Da interface

```json
{"type": "command", "req": 1, "vehicle": 2, "cmd": {"id": "att_mode", "args": {"mode": "sun"}}, "armed": false}
```

Comandos perigosos (marcados `hazardous` no dicionário) exigem `"armed": true`. `"in": 60` executa
daqui a 60 s; `"at": "2026-10-20 03:00:00"` numa hora (UTC). Comandos de bordo precisam de enlace e
executam depois do atraso da luz; os de solo (`ground`) executam na hora. Resposta:
`{"type": "command_result", "ok": true, "id": 17}` ou `{"ok": false, "error": "sem enlace com o veículo"}`.

| `type` | Campos | Resposta |
|---|---|---|
| `plan` | `vehicle`, `kind` (`circularize_ap`, `circularize_pe`, `hohmann`, `apoapsis`, `periapsis`, `inclination`, `match_plane`, `rendezvous`, `deorbit`, `injection`, `tcm`, `capture`), `params` (`alt`, `inc`, `target`, `pe`, `body`, `depart_in_h`, `tof_days`, `arrive_in_days`, `apo`, `lat`, `lon`) | `plan`: queimas (instante relativo e Δv em RTN), Δv total, resumo e pouso previsto |
| `plan_apply` | `vehicle`, `nodes`, `landing` | `plan_applied` com os ids das queimas |
| `predict` | `vehicle`, `span` (s), `step` (s) | `predict`: pontos `[t, lat, lon, alt_km, x, y, z]` |
| `passes` | `vehicle`, `span` (s) | `passes`: estação, AOS, LOS, elevação máxima |
| `timeline` | `vehicle`, `span` (s, até 3 dias) | `timeline`: `eclipses` (`[início, fim]`), `apsides` (`t`, `kind` `pe`/`ap`, `alt`), `passes`, `burns` (`t`, `label`, `dv`, `duration`), `commands` agendados, `countdown` (itens da sequência em hora absoluta), `window` e, para o instrutor, `faults` |
| `porkchop` | `from`, `to`, `start` (UTC), `span_days`, `tof_min`, `tof_max`, `n` | `porkchop`: datas, durações, C3 e v∞ de chegada, melhor ponto |
| `sim` | `action`: `warp` (`value`), `pause`, `resume`, `warp_stop`, `third_body` (`value`), `scenario` (`value`, `start`), `save` (`value`), `saves`, `load` (`value`), `weather` (`surface_wind`, `wind_from`, `jet_wind`, `lightning_km`, `ceiling_m`, `rain`, `evolve`), `fault` (`vehicle`, `fault`, `arg`), `clear_faults`, `ack` (`value`: id do alarme, ou todos), `station` (`id`, `enabled`), `remove_debris`, `ksp_connect` (`value`: pasta do KSP, ou vazio para procurar), `ksp_patch`, `record_start` (`value`: nome, ou automático), `record_stop`, `recordings`, `replay_start` (`value`: nome), `replay_control` (`playing`, `speed`, `seek`: número do quadro), `replay_stop`, `room_mode` (`value`), `files`, `file_get` (`sub`: `veiculos`/`cenarios`/`procedimentos`/`scripts`, `id`), `file_validate` e `file_save` (`sub`, `id`, `text`), `files_reload`, `script_enable` (`name`, `value`), `track` (`value`: texto com TLEs, duas ou três linhas por satélite), `untrack` (`vehicle`), `plan_fault` (`vehicle`, `fault` ou `random`, `arg`, `when`: `in`/`met`/`at`, `value` em s ou `at` em UTC, `window` em s, `probability` de 0 a 1, `announce`), `unplan_fault` (`id`), `fault_plan`, `report`, `report_reset`, `proc_log` (`vehicle`, `procedure`, `kind`: `start`/`step`/`done`, `step`, `total`); `fault` aceita também `announce` (padrão: sim) | `sim_result`, `saves`, `recordings`, `fault_plan`, `report` ou `reload` |
| `dictionary` | — | `dictionary` |
| `join` | `name`, `position` (id de um dos `roles` da saudação) | `joined` |
| `leave` | — | `joined` (`position`: null) |
| `chat` | `text`, `channel` (`geral` ou o id de uma posição) | `chat_result`; a mensagem chega a todos como evento `"level": "chat"` com `from` e `channel` |

### Sala com posições

A saudação traz `roles`: cada posição com `id`, `name`, `description`, os grupos de comando que
comanda (`groups`, `*` = todos), as posições da votação do lançamento que ocupa (`launch`) e as ações
do simulador que pede (`sim`). O instantâneo traz `room` (`on`, `members`: `name` e `position`).
Com a sala ligada, mensagens fora da posição voltam com `ok: false` e o motivo; sem posição, a tela
é observadora. Na contagem, `launch.positions[].awaiting` marca as posições com gente que ainda não
votaram; o comando `vote` (`position`, `go`: `go`/`nogo`) é o voto delas.

O instantâneo traz `scripts`: cada script com `name`, `enabled`, `error` e `runs`. O que eles
escrevem chega como eventos `[script <nome>] …`.

### Satélites reais

Satélites acompanhados por TLE vêm no instantâneo como veículos `"kind": "tracked"`, com posição,
velocidade e elementos do SGP4, telemetria (`nav.*`, `orb.*` e `tle.age`, em dias) e um objeto
`tle` (`norad`, `line1`, `line2`, `epoch`, `age`). `predict` e `passes` funcionam com eles;
comandos e planos são recusados.

### Gravação e reprodução

Gravando, o instantâneo leva `recording` (`name`, `frames`, `span` em segundos de simulação,
`bytes`). A gravação é um arquivo `<nome>.jaci.gz`: JSON Lines em gzip, com o cabeçalho
(`{"type": "header", "format": 1, "name", "created", "hello"}`), os `dictionary` quando mudam e um
`snapshot` por segundo de relógio, cada um com os eventos desde o anterior.

Reproduzindo, a simulação fica parada e o servidor envia os quadros gravados como instantâneos,
com `replay` (`name`, `frames`, `index`, `playing`, `speed`, `t0`, `t1`, `t`); o `hello` leva
também `replay.markers` (`[quadro, nível, texto]` de cada evento) e `replay.times` (`[quadro, t]` de
alguns quadros). `pause`, `resume` e `warp` controlam a reprodução; comandos e planos são
recusados; `predict` parte do estado gravado; `passes` volta vazio.

`GET /api/csv?vehicle=<id>` (com `&token=…` se o servidor exigir) devolve a telemetria do veículo em
CSV: hora UTC, segundos desde J2000 e uma coluna por ponto (`id [unidade]`). Na reprodução, a
gravação inteira; ao vivo, os últimos 30 min.

Naves do KSP vêm no instantâneo como veículos `"kind": "ksp"`, com `central` (o nome do corpo em
minúsculas), `central_radius` e um objeto `ksp` (situação, piloto automático, atitude, SAS, RCS,
estágio, recursos, contagem, idade do último pacote). O instantâneo leva também `ksp` com o estado
da ligação (pasta, kOS, Module Manager, patch, naves). Os comandos delas são os `ksp_*` do
dicionário.

## Ponte com o kOS (arquivos)

O script `jaci.ks` roda no kOS e conversa com o servidor por dois arquivos de texto por nave em
`<KSP>/Ships/Script/jaci/` (o volume `0:` do kOS), com `<id>` sendo o UID da peça do computador:

`tm-<id>.txt`, escrito pelo kOS (o servidor lê as linhas novas; o kOS apaga e recomeça o arquivo a
cada 3000 linhas):

| Linha | Conteúdo |
|---|---|
| `H <id> <nome da nave>` | cabeçalho, no começo e em cada arquivo novo |
| `B <corpo> <raio m> <μ m³/s²> <topo da atmosfera m> <período de rotação s>` | corpo central, também ao trocar de esfera de influência (espaços no nome viram `_`) |
| `T <campos>` | telemetria, 5 por segundo |
| `R <número> <True\|False> <mensagem>` | resposta a um comando |
| `E <texto>` | evento (separação, ignição, corte…) |

Campos da linha `T`, nessa ordem: número, UT (s), tempo de missão (s), situação (`PRELAUNCH`,
`FLYING`, `SUB_ORBITAL`, `ORBITING`, `ESCAPING`, `LANDED`, `SPLASHED`, `DOCKED`), altitude (m),
latitude, longitude, velocidade orbital, velocidade de superfície, velocidade vertical (m/s),
apoastro, periastro (m), inclinação, excentricidade, período (s, -1 em hipérbole), nodo ascendente,
argumento do periastro, anomalia verdadeira (°), semieixo maior (m), massa (t), empuxo, empuxo
máximo (kN), potência (0 a 1), pressão dinâmica (kPa), estágio, e quantidade e capacidade de
LiquidFuel, Oxidizer, MonoPropellant, ElectricCharge e SolidFuel, arfagem e rumo (°), SAS, RCS,
trem, luzes, freios (`True`/`False`), tempo até o apoastro e até o periastro (s, -1 se não há),
tempo até a manobra (-1 sem manobra) e Δv dela, alvo (`-` sem alvo) e distância (m), aceleração do
tempo, tripulantes, modo do piloto automático (`manual`, `ascent`, `coast`, `node`) e modo de
atitude.

`cmd-<id>.txt`, reescrito inteiro pelo servidor (troca atômica) a cada comando: a primeira linha é
`S <sessão>` e as outras `<número> <comando> <argumentos>`, com os números crescendo. O kOS executa
os de número maior que o último que viu; sessão nova (o servidor reiniciou) zera a contagem, e ao
iniciar o script marca como vistos os que já estavam lá.

| Comando | Argumentos |
|---|---|
| `stage`, `abort`, `stop`, `gear`, `lights`, `brakes`, `chutes`, `exec`, `delnodes` | — |
| `throttle` | potência de 0 a 1 |
| `sas`, `rcs`, `panels` | `on` ou `off` |
| `steer` | `prograde`, `retrograde`, `normal`, `antinormal`, `radial_out`, `radial_in`, `up`, `heading` (rumo e arfagem a seguir), `hold`, `sun`, `target` ou `free` |
| `ag` | grupo de ação, 1 a 10 |
| `warp` | nível, 0 a 7 |
| `node` | daqui a (s), radial, normal, prógrado (m/s) |
| `circ` | `ap` ou `pe` |
| `launch` | apoastro alvo (km), rumo (°) |
| `target` | nome (espaços como `_`) |

## Ponte com o kRPC (rede)

Com `--krpc [endereço]` (ou `krpc_connect` pela interface), uma thread do servidor conecta ao
servidor RPC do kRPC (TCP, padrão `127.0.0.1:50000`): pedido de conexão RPC (`ConnectionRequest`,
tipo 0, nome `Jaci`), depois requisições `Request` com várias `ProcedureCall` cada, todas as
mensagens com o tamanho em varint na frente. Valores no formato do kRPC: `double`/`float` em
little-endian, inteiros em zigzag, booleanos e objetos (ids) em varint, texto com o tamanho na
frente, listas e tuplas como mensagem com os itens no campo 1.

A cada 0,1 s ela lê a nave ativa (`SpaceCenter.get_ActiveVessel`, órbita, controle, voo nos
referenciais da superfície e do corpo, recursos, nós e alvo) e, a cada 0,2 s, produz uma linha `T`
igual à do kOS. As linhas `H`, `B`, `R` e `E` também seguem a tabela acima; o identificador da nave
é `krpc-<id do objeto>`. Os comandos são os mesmos da tabela do kOS: a thread os traduz em chamadas
(`Control_ActivateNextStage`, `Control_set_Throttle`, `Control_set_SASMode`, `Control_AddNode`,
`AutoPilot_TargetPitchAndHeading`…) e responde com `R`. Subida, circularização e execução de manobra
rodam na thread, com o piloto automático do kRPC. Quando a conexão cai, ela tenta de novo a cada 2 s.

## Enlace CCSDS (UDP)

Pacotes do Space Packet Protocol (CCSDS 133.0-B): cabeçalho primário de 6 bytes, big-endian.

```
 bits  3      1     1          11        2         14         16
     versão  tipo  sec.hdr    APID    sequência  contador  tamanho-1
       0     0=TM
             1=TC
```

### Telemetria (tipo 0)

Com cabeçalho secundário: tempo CUC de 6 bytes (4 de segundos desde J2000 e 2 de fração em 1/65536 s).
Os dados são pares de 6 bytes: índice do ponto (u16) e valor (f32), big-endian.

**Computador de bordo → Jaci (`--ccsds-in`), APID 0x200 (navegação):**

| Índice | Ponto | Unidade |
|---|---|---|
| 0 | tempo de missão | s |
| 1 | latitude | ° |
| 2 | longitude | ° |
| 3 | altitude | km |
| 4 | velocidade | m/s |
| 5 | velocidade vertical | m/s |
| 6 | aceleração | g |
| 7 | pressão dinâmica | kPa |
| 8 | Mach | — |
| 9 | tensão do barramento | V |
| 10 | arfagem | ° |
| 11 | fase (0 solo, 1 subida, 2 balístico, 3 descida, 4 pousado) | — |

Pode mandar só parte dos pontos em cada pacote. Cada APID vira um veículo externo; os
telecomandos para ele voltam para o endereço e a porta de onde veio a telemetria.

**Jaci → outra ferramenta (`--ccsds-out`):** um pacote por segundo para cada veículo simulado, APID
0x100 + id do veículo, com todos os pontos na ordem do dicionário de telemetria dele (o mesmo do
`hello`).

### Dicionários XTCE

Outros pacotes de telemetria são descritos em XTCE (`missoes/ccsds/*.xml`, veja
[`cubesat.xml`](../missoes/ccsds/cubesat.xml)). Cada `SequenceContainer` com
`RestrictionCriteria/Comparison parameterRef="CCSDS_APID" value="…"` vira um pacote de layout fixo:
os parâmetros do `EntryList`, na ordem, big-endian, logo depois do tempo CUC, sem preenchimento.
Tipos: `FloatParameterType` (32 ou 64 bits) e `IntegerParameterType` (8, 16, 24, 32 ou 64 bits,
`encoding="unsigned"` ou com sinal), com `Unit`, `PolynomialCalibrator` e `StaticAlarmRanges`
(`WarningRange` → limite amarelo, `CriticalRange` → vermelho, que geram alarmes como nos veículos
simulados). O nome do parâmetro é o `shortDescription`; o id no Jaci é o `Alias nameSpace="jaci"`
(use `nav.lat`, `nav.lon` e `nav.alt` para o veículo aparecer no mapa) ou o nome. O veículo externo
se chama `<SpaceSystem> (<contêiner>)`. APIDs sem dicionário seguem o formato de pares acima.
`computador-de-bordo --xtce missoes/ccsds/cubesat.xml` manda os pacotes de um dicionário.

### Telecomando (tipo 1)

Sem cabeçalho secundário. Os dados são o comando em JSON (UTF-8), igual ao da interface:

```json
{"id": "chute", "args": {"index": 1, "action": "deploy"}}
```

Um telecomando que chega no `--ccsds-in` com APID 0x100 + id executa no veículo simulado (já armado):
outro centro de controle pode comandar a simulação.
