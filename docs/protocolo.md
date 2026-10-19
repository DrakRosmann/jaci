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
| `command_result`, `plan`, `plan_applied`, `predict`, `passes`, `porkchop`, `saves`, `sim_result` | respostas | veja abaixo |
| `reload` | depois de carregar cenário ou estado | a interface reconecta e recebe um `hello` novo |

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
| `sim` | `action`: `warp` (`value`), `pause`, `resume`, `warp_stop`, `scenario` (`value`, `start`), `save` (`value`), `saves`, `load` (`value`), `weather` (`surface_wind`, `wind_from`, `jet_wind`, `lightning_km`, `ceiling_m`, `rain`, `evolve`), `fault` (`vehicle`, `fault`, `arg`), `clear_faults`, `ack` (`value`: id do alarme, ou todos), `station` (`id`, `enabled`), `remove_debris` | `sim_result`, `saves` ou `reload` |
| `dictionary` | — | `dictionary` |

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
