# Protocolo

## Interface ↔ servidor (WebSocket)

A interface abre `ws://<servidor>/ws` (com `?token=…` quando o servidor exige). Todas as mensagens
são JSON com um campo `type`. As que pedem resposta levam `req` (um número); a resposta volta com o
mesmo `req` só para quem pediu.

### Do servidor

| `type` | Quando | Conteúdo |
|---|---|---|
| `hello` | ao conectar | versão, cenários, catálogo, dicionário de comandos, falhas, estações, locais de lançamento, corpos, últimos eventos e o dicionário de telemetria de cada veículo |
| `snapshot` | 5 vezes por segundo | instante (`t`, segundos desde J2000, e `utc`), aceleração, veículos (estado, telemetria `tm`, estágios, motores, queimas, guiamento, subsistemas), contagem, clima, alarmes, comandos recentes, eventos novos, corredor de segurança, posições dos corpos |
| `dictionary` | quando um veículo muda de forma (separa estágio, troca tripulação) | `dictionary`: id do veículo → lista de pontos (`id`, `name`, `unit`, `group`, `yellow`, `red`) |
| `command_result`, `plan`, `plan_applied`, `predict`, `passes`, `porkchop`, `saves`, `recordings`, `sim_result` | respostas | veja abaixo |
| `reload` | depois de carregar cenário ou estado, e ao entrar e sair da reprodução (para todas as interfaces) | a interface reconecta e recebe um `hello` novo |
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
| `porkchop` | `from`, `to`, `start` (UTC), `span_days`, `tof_min`, `tof_max`, `n` | `porkchop`: datas, durações, C3 e v∞ de chegada, melhor ponto |
| `sim` | `action`: `warp` (`value`), `pause`, `resume`, `warp_stop`, `scenario` (`value`, `start`), `save` (`value`), `saves`, `load` (`value`), `weather` (`surface_wind`, `wind_from`, `jet_wind`, `lightning_km`, `ceiling_m`, `rain`, `evolve`), `fault` (`vehicle`, `fault`, `arg`), `clear_faults`, `ack` (`value`: id do alarme, ou todos), `station` (`id`, `enabled`), `remove_debris`, `ksp_connect` (`value`: pasta do KSP, ou vazio para procurar), `ksp_patch`, `record_start` (`value`: nome, ou automático), `record_stop`, `recordings`, `replay_start` (`value`: nome), `replay_control` (`playing`, `speed`, `seek`: número do quadro), `replay_stop` | `sim_result`, `saves`, `recordings` ou `reload` |
| `dictionary` | — | `dictionary` |

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

### Telecomando (tipo 1)

Sem cabeçalho secundário. Os dados são o comando em JSON (UTF-8), igual ao da interface:

```json
{"id": "chute", "args": {"index": 1, "action": "deploy"}}
```

Um telecomando que chega no `--ccsds-in` com APID 0x100 + id executa no veículo simulado (já armado):
outro centro de controle pode comandar a simulação.
