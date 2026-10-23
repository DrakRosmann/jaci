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
| `pouso-lua` | Módulo lunar em 15 × 110 km chegando ao perilúnio: comando `pdi` (ou o botão no console PILOT) e o guiamento freia, inclina para a vertical a ~2,5 km e pousa nas pernas a 1,5 m/s, como o P63/P64/P66 do Apollo |
| `pouso-marte` | Sonda de pouso entrando em Marte: escudo térmico, paraquedas supersônico a Mach 2, e a 1,2 km larga o paraquedas e desce no motor até o solo (tipo Phoenix) |
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
* **Órbita e manobras (FIDO):** vista 3D com o corpo central girando — a Terra com oceanos,
  continentes, gelo e atmosfera, a Lua e Marte com relevo, desenhados na hora —, a órbita
  prevista, o traço no solo sobre o globo, as queimas, o nome de cada veículo e a câmera que
  pode seguir a nave;
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
* **Interplanetário (NAV):** o Sistema Solar visto de cima, com as órbitas de verdade dos
  planetas (os elementos médios de Standish, os mesmos do núcleo), zoom pela roda, arrasto, centro
  no Sol, na sonda ou num planeta, a trajetória prevista da sonda e o arco da transferência
  escolhida no porkchop (resolvida por Lambert), com onde o planeta de origem está na partida e o
  de destino na chegada; a tabela de janelas de Hohmann de qualquer planeta para os outros (viagem,
  C3, v∞ de chegada, ângulo de fase de agora e o da transferência, a próxima janela e o período
  sinódico), com um botão que calcula o porkchop em volta da janela; o porkchop de qualquer planeta
  para qualquer outro, colorido por C3, por v∞ de chegada ou pela soma, com os valores sob o mouse,
  o melhor ponto marcado e, no ponto escolhido, as datas, os v∞, o Δv a partir de uma órbita de
  200 km e o botão que leva a janela ao planejador de manobras. Com a sonda no planeta de origem,
  o porkchop parte da órbita de verdade dela (queima no periastro): mostra o Δv de injeção em cada
  célula, quanto sobra do Δv da sonda e hachura o que não cabe no tanque. **Gravidade assistida:**
  procura trajetórias com um sobrevoo (por exemplo Terra–Vênus–Marte ou Terra–Júpiter–Saturno)
  resolvendo os dois trechos por Lambert numa grade de datas, refinada em volta das melhores. Num
  sobrevoo, a gravidade gira o v∞ até o máximo que o periastro mínimo permite (acima da atmosfera,
  e de 3 raios em Júpiter, pela radiação); o que não couber vira queima no periastro. A tabela
  mostra as datas, C3, v∞ chegando e saindo, deflexão, periastro, Δv do sobrevoo, ganho
  heliocêntrico e o custo comparado com o voo direto. A escolhida aparece no mapa (os dois
  trechos) e num diagrama da hipérbole do sobrevoo. E a sonda: velocidade
  heliocêntrica, distâncias do Sol, da Terra e do destino, atraso da luz, elongação (o ângulo
  Sol–Terra–sonda, que avisa da conjunção solar) e a próxima queima.
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
* **Controle manual (PILOT):** de um a quatro MFDs (telas multifunção com teclas na moldura). As
  teclas de cima são as páginas preferidas de cada tela, e o MENU mostra todas e escolhe quais ficam
  na moldura. As páginas: ADI (horizonte artificial com escada de arfagem, arco de rolamento, fita
  de rumo, taxas, os marcadores de prógrado, normal e radial, o vetor da próxima queima ✚ e a
  direção do alvo ◎); ORB (a órbita vista de cima do plano, com apsides, tempos até eles e os outros
  veículos); MNV (a queima escolhida: contagem, Δv em RTN, quanto falta, duração, e o diretor de voo,
  que mostra para onde girar); ENG (cada motor com estado, empuxo, potência, pressão de câmara e
  ignições, TWR atual e máximo, e o tempo de queima do Δv restante); RES (propelente por estágio,
  RCS, bateria, ar, água, gravador, escudo e Δv); DSC (descida: fita de altitude, velocidade vertical
  e horizontal, tempo até o chão e a altura da queima de parada com o motor a pleno); ENT (carga g,
  q, Mach, fluxo, escudo, inclinação lateral, local de pouso e paraquedas); TGT (o alvo no plano
  V-bar/R-bar dele, distância, taxa, aproximação máxima em linha reta, e as teclas para trocar de
  alvo e apontar para ele); SIS (energia, rodas de reação, RCS e nós térmicos); COM (estação,
  margem, taxa, atraso, gravador, com as teclas do transmissor e da descarga); TLM (os pontos de
  telemetria que a pessoa escolher, com a curva dos últimos 15 minutos); ALM (alarmes com o
  reconhecimento); 3D (a nave) e MAPA. O quadro de avisos acende com alarme mestre (que reconhece
  todos ao toque), falha de motor, combustível, bateria, subtensão, pressão da cabine, enlace,
  apagão, modo seguro, terminação armada, guiamento, queima próxima, RCS, rodas saturadas,
  temperatura, aquecimento e acoplamento. O painel de chaves tem ignição, corte e potência,
  separação de estágio, carga, coifa e torre (com tampa de proteção), aborto nos modos possíveis no
  momento, modo seguro, terminação de voo, modos de atitude (incluindo alvo, Terra e livre) e a
  descarga das rodas, pilotagem manual que gira a atitude alvo pelo enlace, pulsos de translação do
  RCS, manobras (circularizar no apoastro ou no periastro, uma queima em RTN programada à mão,
  apagar queimas), encontro (escolher o alvo, apontar, aproximar e acoplar, desacoplar),
  paraquedas, reentrada automática e guiada ou balística, trem, painéis, radiadores, transmissor,
  antena, gravador e, nas naves com pernas, a descida propulsada (PDI).
  Em ⚙ Personalizar escolhem-se quantos MFDs e de que tamanho, as cores das telas (coloridas,
  fósforo verde ou âmbar), o painel à direita ou embaixo, o quadro de avisos, o que o horizonte
  mostra, quais grupos do painel aparecem, o passo da potência e as teclas de voo (padrão: W/S
  arfagem, A/D guinada, Q/E rolamento, I/K, J/L e H/N para o RCS, R/F mais e menos potência, Z/X
  máxima e zero, B ignição, O corte, G manter atitude). Os comandos perigosos nunca vão para o
  teclado. Tudo fica guardado no navegador.
  **Pilotagem manual com diretor de voo (CSS):** na subida, na descida propulsada, na reentrada e
  nas queimas planejadas, o guiamento continua calculando a atitude, a potência e os cortes, mas
  quem aponta a nave é o piloto. As barras magenta do diretor (FD) no horizonte artificial e a
  página MNV mostram para onde o guiamento quer o nariz e o erro em graus. Sem ninguém nas teclas,
  a nave não segue sozinha: um foguete fica na vertical. Na descida propulsada, ASSUMIR (P66)
  desliga o programa com o motor aceso e o piloto controla atitude e potência até o toque, como na
  Apollo 11. O ADI mostra também o retrógrado em relação ao solo (⊘), para matar a velocidade
  horizontal no pouso. Os pulsos de RCS transladam sem girar a nave, e a nave pilotada até a porta
  livre de outra (a menos de 1,5 m, se aproximando a menos de 0,4 m/s) é capturada. Ao desacoplar,
  ela sai da porta a 0,1 m/s e a dinâmica orbital faz o resto.
* **Pouso na Lua e em Marte:** naves com pernas tocam o solo se chegarem a menos de 3,5 m/s na
  vertical e 2,5 m/s de lado. O programa de descida propulsada segue uma curva de frenagem a
  desaceleração constante até 15 m e depois desce a 1,5 m/s; o empuxo atende primeiro a vertical e
  o resto mata a velocidade horizontal, e a inclinação máxima cai de 75° a 2,5 km para 20° perto do
  chão. Em Marte, a sonda pende do paraquedas com o motor para baixo e começa a descida sozinha a
  1,2 km.
* **Nave em 3D (3D):** a nave montada dos estágios, com diâmetros e comprimentos de verdade e
  texturas geradas no próprio Jaci:
  - foguete: tanques de painéis com rebites, interestágios e sinos de motor com perfil de Laval;
  - boosters, coifa ogival e tanque externo de espuma laranja com o orbitador alado ao lado;
  - cápsula com escudo, janelas, propulsores de RCS e sonda de acoplamento (ou a torre de escape);
  - módulo de serviço com radiadores e painéis;
  - módulo lunar com pernas e estágio de subida, e sonda de pouso com convés de instrumentos;
  - satélite com manta térmica dourada, asas de células e antena parabólica;
  - estação com módulos, treliça, asas e radiadores;
  - paraquedas abertos com as cordas.

  A nave aparece na atitude dela em relação ao horizonte local, com a luz do Sol de verdade (fraca
  na sombra), as estrelas e o planeta embaixo, com mares, continentes, nuvens e o halo da
  atmosfera, girado para o ponto sob a nave. Os efeitos:
  - plumas com núcleo claro, que se abrem com a altitude e esquentam o sino;
  - na reentrada, a casca de plasma na frente, o rastro e as faíscas, orientados pela velocidade
    em relação ao ar e com a intensidade do aquecimento;
  - perto do pouso, o solo de verdade embaixo (regolito com crateras na Lua, a poeira de Marte, o
    mar ou a plataforma com a torre na Terra) e a poeira, ou a fumaça na plataforma, levantada
    pelo motor;
  - as naves acopladas presas nas portas da estação: selecionando a visitante, a câmera mira nela
    e na porta;
  - as outras naves por perto (até 5 km): estágios largados caindo, a coifa, o alvo de um
    encontro, cada uma com um rótulo de nome e distância.

  Sombras do Sol de verdade, reflexos no metal e brilho nas plumas e no plasma (os dois se
  desligam). Câmeras: livre, perseguição (de três quartos, no sentido do movimento) e do chão
  (rastreio a partir da plataforma até 40 km, com a teleobjetiva fechando).
* **Hangar (VAB):** montar foguetes e missões, como no Kerbal Space Program.
  - **Estágios:** cada um com propelente (querosene, metano ou hidrogênio com oxigênio,
    hipergólicos ou sólido), motor (de nível do mar ou de vácuo, com empuxo, Isp, massa,
    regulagem e reignições parecidos com os de verdade), quantidade, diâmetro e comprimento. O
    tamanho dá o propelente pela densidade e a estrutura pela fração de cada tipo.
  - **Boosters laterais** e a **carga**: satélite, sonda com motor próprio, cápsula tripulada
    com escudo, paraquedas, torre de escape e porta, ou só massa.
  - **Ao lado:** o foguete em 3D na plataforma, o Δv e o TWR de cada fase (boosters queimando
    junto com o núcleo), o tempo de queima, a massa na decolagem, avisos e uma régua com o Δv que
    cada destino pede.
  - **Missão:** da plataforma (local, T-, órbita alvo, rumo, tempo bom) ou já em órbita da Terra,
    da Lua ou de Marte.
  - **Teste de voo:** o servidor voa o foguete no próprio simulador, num mundo à parte e sem
    salvar nada, e mostra o veredito (em órbita, suborbital, destruído), a órbita, o tempo até
    ela, a pressão dinâmica e a carga g máximas, o Δv que sobrou, gráficos da altitude, da
    velocidade, de q e de g, e os eventos do voo.
  - **Mais:** a ficha de cada motor (empuxo ao nível do mar e no vácuo, Isp, massa, regulagem,
    reignições), desfazer e refazer (Ctrl+Z), duplicar estágio, o custo estimado do lançador e o
    estágio sob o mouse destacado no 3D.
  - **Salvar e lançar:** grava `veiculos/<nome>.toml` e `cenarios/<nome>-missao.toml` (o projeto
    vai num comentário para reabrir no hangar) e carrega a missão.
* **Escola de controle (EDU):** lições curtas para aprender a sala:
  - primeiros passos, a primeira contagem (VSB-30), circularizar uma órbita, um vazamento na
    estação e uma asa solar danificada;
  - no console PILOT: pilotar a nave (modos de atitude, teclado e circularizar só com pulsos de
    RCS), a subida pilotada do VLM-1 pelo diretor de voo, acoplar na estação só com pulsos e o
    pouso na Lua pilotado, com o P66 no fim.
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
telão, a janela nova para outro monitor, a faixa de relógios e o visual, escolhido por cartões
com a amostra das cores: moderno; claro, para a sala iluminada; automático, que segue o claro ou
escuro do sistema; fósforo verde (os consoles do Apollo) ou âmbar (os terminais dos anos 70 e 80),
com linhas de varredura; noite, só em vermelho e com pouca luz (as telas e gráficos também), para
não ofuscar quem está no escuro; e alto contraste, preto e branco com bordas grossas.

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
  0,93° de inclinação por ano, como os de verdade; desliga no Instrutor), e no cruzeiro
  heliocêntrico Vênus, Terra, Lua, Marte, Júpiter e Saturno (n corpos); elementos keplerianos,
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

## Kerbal Space Program (kOS ou kRPC)

O Jaci acompanha e comanda naves do [Kerbal Space Program](https://www.kerbalspaceprogram.com/)
por um de dois mods: o [kOS](https://ksp-kos.github.io/KOS_DOC/) (por arquivos, veja abaixo) ou o
[kRPC](https://krpc.github.io/krpc/) (pela rede, em [Pelo kRPC](#pelo-krpc)). As duas pontes dão a
mesma telemetria e os mesmos comandos.

### Pelo kOS

O kOS é o mod de computadores de bordo programáveis. Um
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

### Pelo kRPC

O kRPC abre um servidor RPC dentro do jogo (TCP, porta 50000), e o Jaci conversa com ele pela rede:
não há scripts nas naves nem pasta do jogo para achar, e o KSP pode estar em outro computador.

```bash
ckan install kRPC         # no jogo: janela do kRPC → Start server (e aceite a conexão do Jaci)
jaci --krpc --open        # 127.0.0.1:50000; ou --krpc 192.168.0.10:50000
```

Também dá para ligar pelo console **KSP** ("Ligar pelo kRPC"); o kOS e o kRPC podem estar ligados
ao mesmo tempo. Diferenças em relação ao kOS:

* O Jaci acompanha a **nave ativa** (trocar de nave no jogo traz a outra).
* A subida automática, a circularização e a execução de manobras rodam **no Jaci**, a 10 Hz, com a
  mesma lógica do script do kOS, comandando o piloto automático e a potência do kRPC.
* Os modos de atitude do navball (prógrado, radial, alvo, manter…) usam o **SAS** da nave (precisa
  de piloto ou núcleo de sonda com esse nível); rumo e arfagem usam o piloto automático do kRPC. O
  modo "Sol" não existe pelo kRPC.
* O kRPC não depende do CommNet: com a nave sem antena, a telemetria continua.

A ponte (`server/src/krpc.rs`) fala o protocolo do kRPC direto (protobuf com o tamanho em varint) e
é testada contra um servidor kRPC de mentira: telemetria, comandos e a execução de uma manobra.

![Console do KSP](docs/img/ksp.png)

## Testes

```bash
cargo test --release            # núcleo: órbitas, efemérides, atmosfera, subsistemas e voos completos
cd web && npm test              # interface: traduções
node tests/servidor.mjs         # servidor de ponta a ponta (depois de cargo build --release),
                                # inclusive a ponte do KSP com um kOS de mentira
                                # (a do kRPC tem um servidor de mentira no cargo test)
```

Os voos completos (`sim/tests/missoes.rs`) contam, votam e lançam cada cenário e conferem o
resultado: apogeu e amerissagem do VSB-30, órbitas do VLM-1, da heliossíncrona, da GTO, da cápsula
(no plano da estação, a menos de 1°), da nave com boosters e do ônibus espacial, e a amerissagem da
cápsula a poucos km do ponto previsto. A sonda a Marte faz a viagem inteira: escolhe a partida em
que o v∞ cai no plano da órbita de estacionamento, simula a queima finita de injeção até o C3, sai
da Terra, corrige a trajetória duas vezes, chega com o periastro pedido e entra em órbita de Marte.
* **Cruzeiro com n corpos:** o traço previsto e o planejador usam o mesmo modelo da simulação
  (RK4 com os planetas a cada subpasso; na simulação, a maré dos planetas entra no meio do passo,
  e o passo encurta perto de um planeta). A correção de trajetória (TCM) parte do Lambert, mira a
  posição de chegada sem o planeta de destino (Newton 3×3) e acerta o plano B na entrada da esfera
  de influência (B·T e B·R, correção de menor norma). Como a Terra continua puxando depois da
  esfera, a primeira TCM corrige também o que as cônicas remendadas da injeção não veem; a segunda
  fica só com o erro de execução da primeira (uns 9 m/s).

## Limitações conhecidas

* Cônicas remendadas na troca de corpo central: dentro de uma esfera de influência, o corpo central
  com J2 e a maré do Sol (e da Lua ou da Terra); fora, os planetas. A injeção ainda é planejada com
  cônicas (a primeira TCM corrige). A Lua tem órbita média (erro de alguns graus). Fora do cruzeiro,
  o traço previsto e o planejador usam só o corpo central com J2.
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
