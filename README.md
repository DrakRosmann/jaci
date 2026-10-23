# Jaci

[![CI](https://github.com/DrakRosmann/jaci/actions/workflows/ci.yml/badge.svg)](https://github.com/DrakRosmann/jaci/actions/workflows/ci.yml)

Centro de controle de missões espaciais: lançamento de foguetes, cápsulas tripuladas, ônibus
espacial, estações, satélites e sondas interplanetárias. Simula os veículos com física de verdade
(mecânica orbital, atmosfera, guiamento, subsistemas) e oferece os consoles de uma sala de
controle: contagem regressiva com votação GO/NO-GO, segurança de voo, dinâmica de voo, encontro e
acoplamento, energia, térmico, atitude, comunicações, suporte de vida, telecomando, alarmes,
estações de solo e procedimentos. Também troca telemetria e telecomando CCSDS por UDP com um
computador de bordo de verdade, e acompanha e comanda naves do Kerbal Space Program pelo kOS.

Jaci é a deusa tupi da Lua.

![Console de lançamento](docs/img/lancamento.png)

| Pasta | O que é |
|---|---|
| [`sim/`](sim/) | núcleo de simulação em Rust: física, veículos, guiamento, subsistemas, controle de lançamento, planejador de manobras |
| [`server/`](server/) | servidor `jaci`: roda a simulação, fala WebSocket com a interface e CCSDS por UDP; `computador-de-bordo`, um voo de demonstração com telemetria CCSDS |
| [`web/`](web/) | interface (TypeScript + React + three.js): os consoles |
| [`ksp/`](ksp/) | ponte com o Kerbal Space Program: o script do kOS (`jaci.ks`), o boot e o patch do Module Manager |
| [`docs/`](docs/) | protocolo da interface, do enlace CCSDS e da ponte com o kOS |
| [`missoes/`](missoes/) | exemplos de veículos, cenários e procedimentos em arquivos (TOML) |
| [`tests/`](tests/) | teste de ponta a ponta do servidor |

## Instalar

Com Rust e Node.js 22 instalados, `./install.sh` compila e instala em `~/.local` (ou
`sudo ./install.sh /usr` para todos): os programas `jaci` e `computador-de-bordo`, a interface e um
atalho **Jaci** no menu de aplicativos, que sobe o servidor e abre o navegador (clicado de novo,
só abre o navegador). A ação "Parar o Jaci" do atalho encerra o servidor. Os estados salvos ficam
em `~/.local/share/jaci/estados`.

## Começar pelo código

Com Rust e Node.js 22 instalados:

```bash
(cd web && npm ci && npm run build)
cargo run --release --bin jaci            # abra http://127.0.0.1:8080
```

`jaci --list` mostra os cenários; `jaci --scenario tripulada` começa num deles (padrão: `sondagem`);
`--open` abre o navegador.
`--start "2026-11-05 12:00:00"` escolhe a data (padrão: agora). Para outra máquina da rede,
`--addr 0.0.0.0:8080`: sem `--token` (ou `JACI_TOKEN`), o servidor gera uma senha e mostra no log
o endereço `http://<ip>:8080/?token=…`.

Para mexer na interface com recarga automática: `cargo run --release --bin jaci` num terminal e
`npm run dev` em `web/` no outro (o Vite repassa o WebSocket para a 8080).

O endereço aceita atalhos: `/#launch` abre o console de lançamento, `/#orbit&en` o de órbita em
inglês; voltar e avançar do navegador trocam de console. No teclado, `1`…`9`, `0`, `-`, `=` e `\`
(e `t`, `c`) abrem os consoles na ordem do menu, `w` o telão, espaço pausa, `[` e `]` mudam a aceleração do tempo, `e` mostra
os eventos, `l` troca a língua e `?` lista os atalhos (nenhum manda comando a veículo).

## Cenários

| Cenário | O que tem |
|---|---|
| `sondagem` | VSB-30 em Alcântara: contagem desde T-10 min, dois estágios sólidos, ~6 min de microgravidade a ~240 km, separação e recuperação da carga no mar com paraquedas |
| `vlm` | VLM-1 em Alcântara: três estágios sólidos; os dois primeiros põem o apogeu na altitude da órbita, o terceiro acende na horizontal no apogeu |
| `sso` | Lançador médio (tipo Falcon 9) com satélite de observação para 752 km, 98,4°: carregamento de propelente, inserção em 200 × 752 km, segunda ignição no apogeu, separação e ativação do satélite |
| `geo` | Satélite geoestacionário para órbita de transferência (GTO) e o motor de apogeu dele |
| `tripulada` | Cápsula para 4 em Cabo Canaveral, com a janela calculada para o plano da estação; encontro e acoplamento |
| `soyuz` | Lançador com quatro boosters laterais em Baikonur, torre de escape e nave com módulo de serviço |
| `sts` | Ônibus espacial: foguetes sólidos, tanque externo, abortos RTLS/TAL/ATO/AOA, reentrada guiada e pouso em pista |
| `estacao` | Estação de 420 t com 7 tripulantes e uma cápsula acoplada: energia, térmico, suporte de vida, reimpulso e emergências |
| `retorno` | Cápsula em órbita: saída de órbita mirando o local de pouso, reentrada guiada, paraquedas e amerissagem |
| `observacao` | Satélite de observação em operação: câmera, gravador de bordo, passagens nas estações, rodas de reação |
| `marte` | Sonda e estágio de escape em órbita de estacionamento, na janela de Marte de 2026: porkchop, injeção, cruzeiro, correção e captura |
| `lua` | Sonda lunar: injeção translunar e inserção em órbita da Lua |
| `ksp` | Nenhum veículo simulado: só as naves do Kerbal Space Program (veja abaixo) |

### Missões em arquivos

Veículos, cenários e procedimentos novos também podem ser escritos em TOML, sem recompilar, na
pasta `missoes/` (ao lado de `estados/`; instalado, `~/.local/share/jaci/missoes`, que começa com
os exemplos). Um veículo parte de um do catálogo com ajustes (carga, propelente, órbita alvo,
tripulação, alvo de pouso…) ou monta uma pilha nova de estágios e motores; um cenário põe veículos
na plataforma ou em órbita, com hora, clima, aceleração, satélites reais e falhas planejadas; um
procedimento tem passos de nota, verificação, comando e espera por uma condição
(`until = "eps.soc > 80 and phase == 'orbit'"`). Os cenários de arquivo aparecem na lista junto dos
prontos (e em `jaci --list`), e os procedimentos no console de procedimentos. Scripts em [Rhai](https://rhai.rs) (`missoes/scripts/*.rhai`) automatizam
sequências e regras de bordo: rodam a cada segundo de simulação, leem telemetria, fases, alarmes e
contagem, e mandam comandos de solo (`cmd`, pelo enlace e com o atraso da luz) ou de bordo
(`onboard`, na hora); `mem`, `once` e `every` montam sequências. Não têm acesso a arquivos nem à
rede, têm limite de operações por execução e não mandam comando perigoso a bordo; um erro desliga o
script e diz por quê. Ligam pelo Instrutor ou pelo cenário (`scripts = [...]`). O console
**Instrutor** tem um editor que confere o arquivo (montando o veículo ou o cenário) antes de salvar
e avisa as outras telas. O formato está em [`missoes/LEIAME.md`](missoes/LEIAME.md), com
exemplos: um VSB-30 com carga pesada, um lançador leve montado do zero (que chega a 500 km,
heliossíncrona, nos testes de voo), um treino com falha de motor sorteada na subida, um cenário
com a ISS real, um procedimento de bateria baixa, uma regra de bordo que protege a bateria e uma
sequência automática de ativação de satélite.

## Consoles

* **Visão geral (FLIGHT):** mapa-múndi com continentes, noite, estações, traço previsto do veículo
  e cobertura; o estado de cada veículo da frota.
* **Lançamento (LCC):** relógio da contagem com espera programada em T-10 min, sequência (fechamento
  da área, braço da tripulação, energia interna, terminação de voo, sequência automática, ignição,
  liberação), votação GO/NO-GO das posições (lançamento, voo, meteorologia, segurança de voo,
  veículo, propulsão, carga útil, comunicações, solo, tripulação, médico) com NO-GO manual,
  critérios de lançamento (vento, raios, teto, área, tanques, motores, bateria, cabine, braço de
  acesso, sinais vitais) que seguram a contagem sozinhos, clima, carregamento e evaporação de
  propelente criogênico, janela no plano de outro veículo, segurança de voo (terminação armada,
  ponto de impacto instantâneo, corredor e terminação autônoma) e os abortos possíveis no momento.
* **Subida e reentrada (BOOSTER):** gráficos de altitude, velocidade, pressão dinâmica e
  aceleração; cada motor com estado, empuxo, pressão de câmara, potência, ignições e tempo de queima;
  guiamento (malha aberta/fechada, órbita alvo, azimute, tempo para o corte); escudo térmico,
  fluxo de calor, ablação, rolamento, paraquedas e alvo de pouso.
* **Órbita e manobras (FIDO):** vista 3D com o corpo central girando, a órbita prevista e as queimas;
  elementos orbitais; planejador (circularizar, Hohmann, apoastro, periastro, inclinação, igualar
  plano, encontro por Lambert, saída de órbita mirando o local de pouso, injeção para outro corpo,
  correção de trajetória, captura); fila de queimas; queima manual em RTN; modos de atitude.
* **Encontro (RNDZ):** estado relativo no referencial LVLH do alvo, gráfico V-bar × R-bar,
  aproximação automática até a captura, portas de acoplamento e transferência da tripulação.
* **Subsistemas (EECOM):** energia (painéis, bateria, cargas com chaves, célula a combustível),
  térmico (nós, aquecedores com termostato, radiadores), atitude (modos, rodas de reação e
  saturação, RCS, rastreador de estrelas), comunicações (estação, margem, taxa, atraso da luz,
  gravador de bordo), suporte de vida (pressão, O₂, CO₂, reservas, módulos e escotilhas) e carga útil.
* **Comandos (CMD):** dicionário com formulário gerado pelos argumentos, execução agora ou com hora
  marcada, confirmação em dois passos dos perigosos e histórico com o estado de cada comando (a
  caminho, executado, falhou). Comandos de bordo exigem enlace e chegam com o atraso da luz.
* **Telemetria e alarmes (TLM):** alarmes amarelos e vermelhos com reconhecimento; todos os pontos
  com valor, unidade e limites; gráfico dos pontos escolhidos.
* **Estações de solo (INCO):** 17 estações (Alcântara, Natal, Cuiabá, Kourou, …, e a rede de espaço
  profundo), visada ao vivo para o veículo, enlace atual e as passagens das próximas 24 h; e os
  satélites reais acompanhados por TLE (veja abaixo).
* **Interplanetário (NAV):** porkchop (C3 de partida e v∞ de chegada) com clique para escolher a
  janela, vista do Sistema Solar com a sonda e a trajetória, distância e atraso da luz.
* **Procedimentos (PROC):** listas de verificação que mandam comandos e esperam condições de
  telemetria (contagem, ativação de satélite, reentrada, acoplamento, despressurização, bateria
  baixa, descarga de dados, injeção interplanetária, reimpulso).
* **Instrutor (SIM):** cenários, aceleração do tempo (até 100 000×, volta a 1× num alarme), estados
  salvos, gravações da missão para o debriefing, clima, falhas planejadas, relatório do treino e
  injeção de falhas: motor, vazamento de propelente, célula da bateria, painel solar
  travado ou degradado, roda de reação, girômetros, rastreador de estrelas, transmissor, aquecedor,
  vazamento na cabine, remoção de CO₂, paraquedas, célula a combustível, barco na área, falha de solo
  e tempestade.
* **Kerbal Space Program (KSP):** a ligação com o jogo e o controle das naves pelo kOS (veja abaixo).
* **Controle manual (PILOT):** dois MFDs (telas multifunção com teclas na moldura) com as páginas
  ADI — horizonte artificial com escada de arfagem, arco de rolamento, fita de rumo, taxas e os
  marcadores de prógrado, normal e radial —, ORB (a órbita vista de cima do plano, com apsides,
  tempos até eles e os outros veículos), RES (propelente por estágio, RCS, bateria, ar, água,
  gravador, escudo e Δv), 3D (a nave) e MAPA; o quadro de avisos (alarme mestre, falha de motor,
  combustível, bateria, pressão da cabine, enlace, modo seguro, terminação armada…); e o painel de
  chaves: ignição, corte e potência, separação de estágio, carga, coifa e torre (com tampa de
  proteção), aborto nos modos possíveis no momento, modo seguro, terminação de voo, modos de
  atitude, pilotagem manual (setas ou teclado: W/S arfagem, A/D guinada, Q/E rolamento) que gira a
  atitude alvo pelo enlace, pulsos de translação do RCS (I/K, J/L, H/N), paraquedas, trem,
  painéis e transmissor.
* **Nave em 3D (3D):** a nave montada dos estágios — diâmetros e comprimentos de verdade,
  boosters, coifa, cápsula com torre de escape, painéis solares, paraquedas abertos, plumas dos
  motores acesos —, na atitude dela em relação ao horizonte local, com os estágios e motores ao
  lado.
* **Escola de controle (EDU):** lições curtas para aprender a sala — primeiros passos, a primeira
  contagem (VSB-30), circularizar uma órbita, um vazamento na estação e uma asa solar danificada.
  Cada uma carrega um cenário e dá objetivos que o Jaci confere sozinho; um cartão acompanha em
  qualquer console, com dica e "me leve lá", e no fim dá até três estrelas (tempo e dicas). Novas
  lições se escrevem em `missoes/escola/*.toml`.
* **Meu console (MEU):** blocos dos outros consoles — mapa, órbita 3D, gráfico, mostradores,
  alarmes, eventos, relógios, frota, contagem, linha do tempo e loops — numa grade que se arrasta
  pelo título e se redimensiona pelo canto ("Editar"); guardado por posição da sala, com modelos
  prontos para cada uma e exportar/importar.
* **Telão (`/#telao`, tecla `w`, botão ⛶):** tela cheia para a parede da sala, em letra grande:
  relógios e contagem, mapa da frota, alarmes ativos (a borda fica vermelha com alarme vermelho),
  estado dos veículos e os últimos eventos. O botão ⧉ abre o console atual numa janela nova, para
  outro monitor.
* **Linha do tempo (FAO):** o que vem pela frente para o veículo, em faixas — eventos recentes,
  contagem e janela, queimas, comandos agendados, eclipses, periastros e apoastros, passagens em
  cada estação e (para o instrutor) as falhas planejadas —, com zoom de 1 h a 3 dias e a lista dos
  próximos acontecimentos.

A interface está em português e inglês (botão no canto); os textos do servidor que têm tradução
também mudam.

Inspirada nas salas de verdade: a faixa de relógios embaixo da barra de cima, como os painéis
numéricos do MOCR do Apollo — GMT com o dia do ano, GET (tempo desde o lançamento, ou a
contagem), o próximo evento com a contagem até ele e AOS/LOS (quanto falta para ganhar ou perder o
sinal) —; o "programa de trabalho" no telão, como o do TsUP de Moscou; e o menu 🖥 (Tela), com o
telão, a janela nova para outro monitor, a faixa de relógios e o visual: moderno, fósforo verde
(os consoles do Apollo) ou âmbar (os terminais dos anos 70 e 80), com linhas de varredura.

O botão 🔇 da barra de cima (ou a tecla `m`) liga o som de cada tela: o alarme mestre (dois tons
enquanto houver alarme vermelho não reconhecido; "Silenciar" cala só aquela tela até o próximo), um
aviso para os amarelos e a locução da contagem ("T menos 30 segundos", "10, 9, …", "Decolagem",
"Espera"), pela voz do próprio navegador, em português ou inglês; sem voz instalada, a contagem
vira bipes. Opcionalmente, também a locução dos alarmes e dos eventos importantes. A contagem só é
falada em tempo real (até 2×), na missão ou na reprodução.

| | |
|---|---|
| ![Visão geral](docs/img/visao-geral.png) | ![Subida](docs/img/subida.png) |
| Cápsula recém-inserida a 210 km, no plano da estação, com o traço previsto e o ponto de queda do 1º estágio | Subida da cápsula: pressão dinâmica achatada pela redução de potência, corte do 1º estágio e malha fechada |
| ![Órbita](docs/img/orbita.png) | ![Interplanetário](docs/img/interplanetario.png) |
| Órbita em 3D e as queimas do encontro com a estação | Porkchop da janela de Marte de 2026 |

## Satélites reais

No console **Estações de solo (INCO)**, o painel "Satélites reais" busca os elementos NORAD (TLE) no
[CelesTrak](https://celestrak.org/) — atalhos para a ISS, Tiangong, Hubble, Amazonia-1, CBERS-4 e
4A, SCD-1 e 2, SGDC-1 e outros, ou qualquer número NORAD ou nome — ou aceita TLEs colados (sem
internet). O servidor propaga cada um com o SGP4: aparecem no mapa, na vista 3D, na frota, nos
gráficos e nas gravações como os simulados, com as passagens nas estações e a visada ao vivo, mas
sem comandos. A idade do TLE vira um ponto de telemetria com alarme (o erro cresce alguns km por
dia). A lista fica em `satelites.tle`, ao lado dos estados salvos, e continua ao trocar de cenário.

## Sala de controle com várias pessoas

Com `jaci --room` (ou "Sala com posições" no console **Instrutor**), cada tela entra numa posição —
FLIGHT, LCC, BOOSTER, FIDO, GNC, EECOM, INCO, PAYLOAD, RSO, METEO, CAPCOM, SURGEON, SIM
(instrutor) ou OBS (observador) — pelo crachá na barra de cima (a tela vai para o console da
posição: EECOM para Subsistemas, FIDO para Órbita, SIM para o Instrutor, OBS para o telão…), e só
comanda o que é dela: o FLIGHT
comanda tudo; cada posição, os grupos de comando da sua área (o RSO a terminação de voo, a EECOM
energia, térmico e suporte de vida, o FIDO manobras e o planejador…); o instrutor só mexe no
simulador; o observador só acompanha. O servidor confere cada mensagem e a interface apaga o que
não é da posição. Na votação GO/NO-GO, as posições do lançamento ocupadas por pessoas esperam o voto
delas (a tela de cada uma pede o voto); as outras respondem sozinhas pelos critérios. Os loops 💬
são canais de texto — o geral e um por posição — que entram no registro de eventos, nas gravações e
na reprodução; chamada no loop da sua posição toca dois bipes (com o som ligado). Sem a sala
ligada, toda tela comanda tudo, como antes. As posições não têm senha: quem entra na sala é quem tem
o endereço (e o token, na rede).

## Treino

No console **Instrutor**, além de injetar falhas na hora (avisando a equipe ou não), dá para
planejá-las: daqui a N s, numa hora UTC ou em T+N s depois do lançamento de um veículo; com uma
janela aleatória (o instante é sorteado dentro dela), uma chance de acontecer (sorteada ao planejar)
e até o tipo aleatório (uma falha que sirva àquele veículo). Por padrão as planejadas são
silenciosas: a equipe tem de descobrir pelos sintomas. O plano não vai no instantâneo; só aparece
no console do instrutor.

O **relatório do treino** mostra, desde o começo do cenário (ou desde "Recomeçar"): cada falha com
o tempo até o primeiro alarme e até o primeiro comando ao veículo; cada alarme com o tempo até ser
reconhecido; os procedimentos seguidos (início, passos e duração, informados pelo console de
procedimentos); o desfecho dos veículos; e uma nota indicativa — a média da reação aos alarmes
(100% até 30 s, zero a partir de 3 min) e da resposta às falhas (100% com comando em até 2 min, 50%
em até 10 min), cortada pela metade se um veículo foi perdido. Sai em Markdown para arquivar.

## Gravação e debriefing

No console **Instrutor**, "Gravar" guarda a missão (um quadro por segundo: telemetria, eventos,
alarmes e comandos) em `gravacoes/` ao lado dos estados salvos (`--recordings-dir` muda a pasta);
`jaci --record` já começa gravando. "Reproduzir" para a simulação e mostra a gravação em todas as
telas conectadas, com todos os consoles: a barra de reprodução tem tocar e pausar (também pelo
espaço), velocidade, uma linha do tempo com as marcas dos alarmes, avisos e eventos (clique para
pular até lá) e o botão CSV com a telemetria do veículo selecionado. Na reprodução os comandos
ficam desligados. Ao vivo, o botão CSV do console de telemetria baixa os últimos 30 min.

## A física

* **Mecânica orbital:** corpo central com J2 mais a maré dos terceiros corpos — o Sol e a Lua dentro
  da esfera da Terra, o Sol e a Terra na da Lua, o Sol na dos planetas (um geoestacionário ganha
  0,93° de inclinação por ano, como os de verdade; desliga no Instrutor); elementos keplerianos,
  propagação em variáveis universais, Lambert, Hohmann, bielíptica, Clohessy-Wiltshire. Cônicas
  remendadas: o veículo troca de corpo central ao cruzar a esfera de influência (Terra, Lua, Sol,
  planetas).
* **Efemérides:** elementos médios de Standish (JPL, 1800–2050) para os planetas, elementos médios
  com regressão dos nodos para a Lua, calculados em Tempo Terrestre (TT = UTC + segundos
  intercalares + 32,184 s, pela tabela do IERS); tempo sideral de Greenwich; WGS-84.
* **Rotação dos corpos:** polos e meridianos principais da IAU (WGCCRE) para a Lua (com os termos
  periódicos), Marte, Mercúrio, Vênus, os gigantes e o Sol: latitude, longitude, locais na
  superfície e a inclinação no equador do corpo saem certas fora da Terra (vista da Lua, a Terra
  fica perto de 0°, 0°, como deve).
* **Atmosfera:** Padrão dos EUA de 1976 até 86 km e tabela exponencial até 1000 km; vento com
  corrente de jato; atmosferas exponenciais de Marte, Vênus e gigantes gasosos.
* **Veículos:** pilha de estágios com boosters em paralelo, motores líquidos (partida, potência,
  reignição) e sólidos, empuxo que muda com a pressão ambiente, coifa, torre de escape, separação
  da carga útil; estágios largados viram detritos que caem ou ficam em órbita.
* **Guiamento de subida:** programa de arfagem em malha aberta e, no estágio superior, malha
  fechada pelo PEG (Powered Explicit Guidance, o do ônibus espacial na forma 2D: direção
  linear-tangente com A, B e o tempo até o corte resolvidos a cada segundo para chegar ao raio de
  inserção com velocidade radial nula), com o plano alvo e o corte pela energia (circular) ou pelo
  apoastro (elíptica); num estágio que não chega sozinho, uma lei mais simples até a separação; órbitas altas inserem baixo e circularizam no
  apoastro; lançadores sólidos sobem em balística e acendem o motor de apogeu. Limite de g e redução
  de potência na pressão dinâmica máxima.
* **Reentrada:** arrasto e sustentação por número de Mach, aquecimento de Sutton-Graves, escudo
  ablativo, paraquedas em estágios, guiamento por preditor que acha o rolamento para cair no alvo
  e inverte o lado para corrigir o desvio lateral, apagão de comunicações por plasma; o ônibus
  espacial plana até a pista, baixa o trem e arredonda. A saída de órbita busca, nas voltas dos
  próximos dois dias (propagando com J2), a que cai mais perto do alvo.
* **Encontro:** queimas de Lambert até 1 km atrás do alvo e aproximação final pela V-bar com o RCS.
* **Subsistemas:** balanço de energia com eclipse e distância ao Sol, bateria, célula a combustível
  e energia de solo; nós térmicos com radiação, Sol, aquecimento pelo planeta e termostatos;
  dinâmica de corpo rígido com rodas de reação, saturação e descarga; orçamento de enlace (EIRP,
  perda no espaço livre, G/T, Eb/N0) com escolha automática da taxa, ocultação pela Lua e atraso da
  luz; gás da cabine por pressões parciais, metabolismo da tripulação, remoção de CO₂, eletrólise,
  vazamento por escoamento crítico e isolamento de módulos.
* **Proteção a bordo:** modo de segurança com bateria baixa, descarga das rodas perto da saturação,
  descarga do gravador cheio, alarmes de despressurização e CO₂.

Os números dos veículos são aproximações de dados públicos daqueles em que cada um se inspira:
servem para voar de forma plausível, não para projeto.

## Enlace CCSDS com hardware

O servidor recebe telemetria e manda telecomando em pacotes CCSDS (Space Packet Protocol) por UDP:

```bash
jaci --ccsds-in 0.0.0.0:10015                  # recebe a telemetria de computadores de bordo
computador-de-bordo --server 127.0.0.1:10015   # voo de demonstração (VSB-30) mandando telemetria
jaci --ccsds-out 127.0.0.1:10016               # manda a telemetria dos simulados para outra ferramenta
```

Cada APID novo aparece como um veículo externo, com mapa, telemetria e comandos (que voltam como
pacotes de telecomando para o endereço de onde a telemetria veio). Além do pacote de navegação
(APID 0x200), qualquer pacote pode ser descrito num dicionário XTCE em `missoes/ccsds/*.xml`
(tipos, codificação, calibração, unidades e faixas de alarme), que vira pontos de telemetria com
nome, unidade e alarmes amarelos e vermelhos; o editor do Instrutor confere o XML. O exemplo
`cubesat.xml` descreve um CubeSat no APID 0x210, e `computador-de-bordo --xtce
missoes/ccsds/cubesat.xml` o simula. O formato está em [`docs/protocolo.md`](docs/protocolo.md).

## Kerbal Space Program (kOS)

O Jaci acompanha e comanda naves do [Kerbal Space Program](https://www.kerbalspaceprogram.com/)
pelo [kOS](https://ksp-kos.github.io/KOS_DOC/), o mod de computadores de bordo programáveis. Um
script do kOS (`ksp/Script/jaci/jaci.ks`) roda na nave, escreve a telemetria e executa os comandos;
os dois conversam por arquivos na pasta `Ships/Script` do KSP, que é o volume `0:` (Archive) do kOS.
Não precisa de outro mod nem de rede.

```bash
ckan install kOS          # ou pelo CKAN gráfico; o Module Manager vem junto na maioria das instalações
jaci --ksp --open         # acha o KSP do Steam; ou --ksp "/caminho/do/Kerbal Space Program"
```

Também dá para ligar pela interface, no console **KSP**, ou pelo atalho "Jaci com o KSP" do menu.
Ao ligar, o Jaci copia `jaci/jaci.ks` e `boot/jaci.ks` para `Ships/Script`. Depois, no jogo:

1. Monte a nave com um computador kOS (peça *CX-4181* ou *KAL9000*). O botão "Pôr o kOS em todas
   as peças de comando" no console KSP instala um patch do Module Manager
   (`GameData/JaciKOS/jaci-kos.cfg`) que põe o computador, já com o boot do Jaci, em toda cápsula e
   sonda (vale depois de reiniciar o KSP; apague a pasta para desfazer).
2. No VAB, no menu da peça kOS, escolha o arquivo de boot `jaci.ks`; ou, já na plataforma, abra o
   terminal do kOS e digite `RUNPATH("0:/jaci/jaci.ks").`
3. A nave aparece no Jaci em segundos, como veículo do tipo "Nave do KSP".

O que dá para fazer com ela:

* **Telemetria** (5 por segundo): altitude, velocidades, apoastro e periastro com os tempos até
  eles, elementos orbitais, pressão dinâmica, aceleração, empuxo/peso, massa, empuxo, potência,
  arfagem e rumo, estágio, combustível, oxidante, monopropelente, carga elétrica (com alarme),
  combustível sólido, manobra e alvo. Entra nos gráficos, alarmes e histórico como a dos simulados.
* **Mapa e órbita** no corpo em que a nave está (Kerbin, Mun, Duna, …, inclusive os de pacotes de
  planetas), com o traço previsto e o KSC; relógio do KSP no calendário de Kerbin.
* **Lançamento automático com contagem:** a contagem corre no Jaci; no T-0 o kOS acende, faz a curva
  de gravidade, separa os estágios vazios, corta no apoastro alvo, repõe o que o arrasto tirou e
  circulariza fora da atmosfera com uma manobra que ele mesmo executa.
* **Voo:** separar estágio e aborto (com confirmação em dois passos), potência, aceleração do tempo
  do jogo, parar o piloto automático.
* **Atitude:** prógrado, retrógrado, normal, antinormal, radial, para cima, rumo e arfagem,
  manter, Sol, alvo, livre; SAS e RCS.
* **Manobras:** circularizar no apoastro ou no periastro, criar nós (prógrado, normal, radial),
  executar o próximo nó (aponta, espera, queima centrada no nó e corta no Δv) e apagar.
* **Sistemas:** trem de pouso, luzes, freios, paraquedas, painéis solares, os 10 grupos de ação e o
  alvo.

Sem conexão com o KSC (CommNet ligado e a nave sem antena ou atrás de um planeta) o kOS não chega
ao Archive: não há telemetria nem comandos, como numa perda de sinal de verdade. O kOS só roda em
naves carregadas (a ativa e as que estão a menos de ~2,5 km dela). O formato dos arquivos está em
[`docs/protocolo.md`](docs/protocolo.md); `ksp/verificar/verificar.sh` compila os scripts com o
compilador do próprio kOS, sem abrir o jogo.

![Console do KSP](docs/img/ksp.png)

## Testes

```bash
cargo test --release            # núcleo: órbitas, efemérides, atmosfera, subsistemas e voos completos
cd web && npm test              # interface: traduções
node tests/servidor.mjs         # servidor de ponta a ponta (depois de cargo build --release),
                                # inclusive a ponte do KSP com um kOS de mentira
```

Os voos completos (`sim/tests/missoes.rs`) contam, votam e lançam cada cenário e conferem o
resultado: apogeu e amerissagem do VSB-30, órbitas do VLM-1, da heliossíncrona, da GTO, da cápsula
(no plano da estação, a menos de 1°), da nave com boosters e do ônibus espacial, e a amerissagem da
cápsula a poucos km do ponto previsto. A sonda a Marte faz a viagem inteira: escolhe a partida em
que o v∞ cai no plano da órbita de estacionamento, simula a queima finita de injeção até o C3, sai
da Terra, corrige a trajetória duas vezes, chega com o periastro pedido e entra em órbita de Marte.

## Limitações conhecidas

* Cônicas remendadas: no cruzeiro heliocêntrico só o Sol puxa (o planejador conta com isso), e a
  Lua tem órbita média (erro de alguns graus). O traço previsto e o planejador usam só o corpo
  central com J2.
* Os elementos orbitais (`orb.inc`, `orb.raan`…) ficam no equador da Terra (J2000) em qualquer
  corpo, para o planejador; a inclinação no equador do corpo central vai em `orb.inc_eq`.
* A atitude dos foguetes e dos veículos em alta aceleração do tempo é cinemática (gira com taxa
  limitada até o alvo); a dinâmica de corpo rígido vale para naves em órbita.
* O PEG é a versão 2D de um estágio (não o UPFG completo de vários estágios); o RTLS do ônibus
  espacial é simplificado.
* UT1 é tratado como igual ao UTC (diferença abaixo de 0,9 s); a tabela de segundos intercalares
  (`sim/src/time.rs`) precisa de uma linha nova quando o IERS anunciar um.
* XTCE: o subconjunto de telemetria descrito em `docs/protocolo.md` (contêineres de layout fixo
  por APID); telecomando continua em JSON.

## Créditos

Contornos dos continentes: [Natural Earth](https://www.naturalearthdata.com/) (domínio público).
Efemérides: elementos médios de E. M. Standish (JPL). Licença do Jaci: MIT.
