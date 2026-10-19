// Procedimentos de operação: cada passo é uma nota, uma verificação manual, um
// comando ou uma espera por uma condição. A interface executa e acompanha.

import type { Snapshot, Vehicle } from '../types'

export type Step =
  | { kind: 'note'; text: string }
  | { kind: 'check'; text: string }
  | { kind: 'cmd'; text: string; cmd: string; args?: Record<string, unknown> | ((v: Vehicle, s: Snapshot) => Record<string, unknown>); hazardous?: boolean }
  | { kind: 'wait'; text: string; until: (v: Vehicle, s: Snapshot) => boolean }

export interface Procedure {
  id: string
  name: string
  description: string
  kinds: string[]
  steps: Step[]
}

const tm = (v: Vehicle, id: string) => v.tm?.[id] ?? NaN
const station = (s: Snapshot) => s.vehicles.find((x) => x.kind === 'station')

export const PROCEDURES: Procedure[] = [
  {
    id: 'countdown',
    name: 'Contagem e lançamento',
    description: 'Da contagem regressiva à inserção (ou ao apogeu, no foguete de sondagem).',
    kinds: ['sounding', 'satellite', 'capsule', 'shuttle', 'probe'],
    steps: [
      { kind: 'check', text: 'Clima e área de lançamento verificados no console de lançamento' },
      { kind: 'cmd', text: 'Iniciar a contagem', cmd: 'count_start' },
      { kind: 'wait', text: 'Propelentes carregados (≥ 98,5 %)', until: (v) => (v.stages ?? []).filter((s) => s.cryogenic).every((s) => (s.prop ?? 0) / (s.capacity || 1) >= 0.985) },
      { kind: 'wait', text: 'Espera programada em T-10 min', until: (_v, s) => s.launch.state === 'hold' || s.launch.t_minus < 600 },
      { kind: 'cmd', text: 'Votação GO/NO-GO', cmd: 'poll' },
      { kind: 'wait', text: 'Todas as posições GO', until: (_v, s) => s.launch.all_go },
      { kind: 'cmd', text: 'Retomar a contagem', cmd: 'count_start' },
      { kind: 'wait', text: 'Decolagem', until: (v) => v.phase !== 'prelaunch' },
      { kind: 'wait', text: 'Max-Q superado', until: (v) => tm(v, 'nav.alt') > 20 },
      { kind: 'wait', text: 'Inserção orbital (ou apogeu)', until: (v) => !!v.guidance?.inserted || tm(v, 'nav.vspeed') < 0 || v.phase === 'orbit' },
      { kind: 'note', text: 'Confirme a órbita no console de órbita e anote periastro, apoastro e inclinação' },
    ],
  },
  {
    id: 'activation',
    name: 'Ativação do satélite',
    description: 'Depois da separação: painéis, apontamento, carga útil.',
    kinds: ['satellite', 'probe'],
    steps: [
      { kind: 'wait', text: 'Separação da carga útil', until: (v) => (v.stages ?? []).every((s) => s.spacecraft) },
      { kind: 'cmd', text: 'Amortecer a rotação', cmd: 'att_mode', args: { mode: 'detumble' } },
      { kind: 'wait', text: 'Velocidade angular abaixo de 0,5°/s', until: (v) => tm(v, 'adcs.rate') < 0.5 },
      { kind: 'cmd', text: 'Abrir os painéis solares', cmd: 'solar_deploy' },
      { kind: 'cmd', text: 'Apontar os painéis ao Sol', cmd: 'att_mode', args: { mode: 'sun' } },
      { kind: 'wait', text: 'Bateria carregando', until: (v) => tm(v, 'eps.battery') > 0 || tm(v, 'eps.soc') > 95 },
      { kind: 'wait', text: 'Enlace com uma estação', until: (v) => !!v.systems?.comms.link },
      { kind: 'cmd', text: 'Apontamento nadir para operar a carga útil', cmd: 'att_mode', args: { mode: 'nadir' } },
      { kind: 'note', text: 'Ligue os instrumentos no console de subsistemas e acompanhe o gravador' },
    ],
  },
  {
    id: 'deorbit',
    name: 'Saída de órbita, reentrada e amerissagem',
    description: 'Cápsula: queima de saída de órbita, separação do tronco, reentrada guiada, paraquedas.',
    kinds: ['capsule', 'shuttle'],
    steps: [
      { kind: 'check', text: 'Alvo de pouso definido (console de subida e reentrada)' },
      { kind: 'note', text: 'No console de órbita, planeje "Saída de órbita" e acrescente ao plano de voo' },
      { kind: 'wait', text: 'Queima de saída de órbita programada', until: (v) => (v.nodes ?? []).some((n) => !n.done) },
      { kind: 'cmd', text: 'Sequência automática de reentrada ligada', cmd: 'auto_chutes', args: { on: 'on' } },
      { kind: 'wait', text: 'Queima concluída', until: (v) => (v.nodes ?? []).length > 0 && (v.nodes ?? []).every((n) => n.done) },
      { kind: 'wait', text: 'Interface de reentrada (120 km)', until: (v) => tm(v, 'nav.alt') < 120 },
      { kind: 'wait', text: 'Fim do apagão de comunicações', until: (v) => tm(v, 'nav.alt') < 35 },
      { kind: 'wait', text: 'Paraquedas abertos', until: (v) => (v.chutes ?? []).some((c) => c.state === 'deployed') || v.landing === 'runway' },
      { kind: 'wait', text: 'Pouso ou amerissagem', until: (v) => ['splashdown', 'landed'].includes(v.phase) },
      { kind: 'note', text: 'Acione as equipes de resgate para o ponto de pouso' },
    ],
  },
  {
    id: 'rendezvous',
    name: 'Encontro e acoplamento com a estação',
    description: 'Do plano às queimas de encontro e à aproximação final pela V-bar.',
    kinds: ['capsule', 'shuttle'],
    steps: [
      { kind: 'note', text: 'No console de encontro, planeje "Igualar plano" se a diferença passar de 0,1°' },
      { kind: 'note', text: 'Planeje "Encontro com um alvo" e acrescente as duas queimas' },
      { kind: 'wait', text: 'Queimas de encontro concluídas', until: (v) => (v.nodes ?? []).length > 0 && (v.nodes ?? []).every((n) => n.done) },
      {
        kind: 'wait',
        text: 'A menos de 8 km da estação',
        until: (v, s) => {
          const st = station(s)
          return !!st?.r && !!v.r && Math.hypot(st.r[0] - v.r[0], st.r[1] - v.r[1], st.r[2] - v.r[2]) < 8000
        },
      },
      { kind: 'cmd', text: 'Aproximação automática', cmd: 'dock', args: (_v, s) => ({ target: station(s)?.id }) },
      { kind: 'wait', text: 'Captura e acoplamento', until: (v) => v.attached_to != null },
      { kind: 'cmd', text: 'Transferir a tripulação', cmd: 'crew_transfer', args: (v) => ({ count: v.crew }) },
    ],
  },
  {
    id: 'depress',
    name: 'Emergência: despressurização',
    description: 'Vazamento na cabine: repressurizar, achar o módulo e isolar.',
    kinds: ['station', 'capsule', 'shuttle'],
    steps: [
      { kind: 'cmd', text: 'Repressurização automática ligada', cmd: 'auto_press', args: { on: 'on' } },
      { kind: 'check', text: 'Tripulação reunida no veículo de retorno' },
      { kind: 'note', text: 'Em Subsistemas › Suporte de vida, ache o módulo marcado com VAZAMENTO e feche a escotilha dele' },
      { kind: 'wait', text: 'Módulo com vazamento isolado', until: (v) => (v.systems?.eclss?.modules ?? []).every((m) => !m.leak || m.isolated) },
      { kind: 'wait', text: 'Pressão acima de 95 kPa', until: (v) => tm(v, 'ecl.pressure') > 95 },
      { kind: 'note', text: 'Registre o consumo de O₂ e N₂ de reserva' },
    ],
  },
  {
    id: 'low_battery',
    name: 'Emergência: bateria baixa',
    description: 'Corta cargas, aponta ao Sol e espera recarregar.',
    kinds: [],
    steps: [
      { kind: 'cmd', text: 'Modo de segurança', cmd: 'safe_mode', args: { on: 'on' } },
      { kind: 'cmd', text: 'Painéis ao Sol', cmd: 'att_mode', args: { mode: 'sun' } },
      { kind: 'wait', text: 'Gerando mais do que consome', until: (v) => tm(v, 'eps.battery') > 0 },
      { kind: 'wait', text: 'Bateria acima de 60 %', until: (v) => tm(v, 'eps.soc') > 60 },
      { kind: 'cmd', text: 'Sair do modo de segurança', cmd: 'safe_mode', args: { on: 'off' } },
    ],
  },
  {
    id: 'downlink',
    name: 'Descarga do gravador',
    description: 'Espera uma passagem e transmite os dados gravados.',
    kinds: ['satellite', 'probe', 'station'],
    steps: [
      { kind: 'wait', text: 'Enlace com uma estação', until: (v) => !!v.systems?.comms.link },
      { kind: 'cmd', text: 'Descarregar o gravador', cmd: 'dump' },
      { kind: 'wait', text: 'Gravador abaixo de 5 %', until: (v) => tm(v, 'com.stored') < 5 },
    ],
  },
  {
    id: 'interplanetary',
    name: 'Injeção interplanetária',
    description: 'Da órbita de estacionamento ao cruzeiro: injeção, separação, ativação, correção.',
    kinds: ['probe'],
    steps: [
      { kind: 'note', text: 'No console interplanetário, calcule o porkchop e escolha a janela' },
      { kind: 'note', text: 'Planeje "Injeção para outro corpo" com a partida e a duração da viagem e acrescente ao plano' },
      { kind: 'wait', text: 'Injeção concluída', until: (v) => (v.nodes ?? []).length > 0 && (v.nodes ?? []).every((n) => n.done) },
      { kind: 'cmd', text: 'Separar o estágio de escape', cmd: 'payload_sep', hazardous: true },
      { kind: 'cmd', text: 'Abrir os painéis', cmd: 'solar_deploy' },
      { kind: 'cmd', text: 'Abrir a antena de alto ganho', cmd: 'hga_deploy' },
      { kind: 'cmd', text: 'Antena para a Terra', cmd: 'att_mode', args: { mode: 'earth' } },
      { kind: 'wait', text: 'Fora da esfera de influência da Terra', until: (v) => v.central !== 'earth' },
      { kind: 'note', text: 'Planeje uma correção de trajetória (TCM) para o periastro desejado na chegada' },
      { kind: 'wait', text: 'Chegada à esfera de influência do destino', until: (v) => v.central !== 'sun' && v.central !== 'earth' },
      { kind: 'note', text: 'Planeje "Captura" e acrescente: a queima é no periastro' },
    ],
  },
  {
    id: 'reboost',
    name: 'Reimpulso da estação',
    description: 'Compensa o arrasto subindo a órbita.',
    kinds: ['station'],
    steps: [
      { kind: 'check', text: 'Sem veículos em aproximação' },
      { kind: 'note', text: 'No console de órbita, planeje "Hohmann para altitude" alguns km acima e acrescente' },
      { kind: 'wait', text: 'Queimas concluídas', until: (v) => (v.nodes ?? []).length > 0 && (v.nodes ?? []).every((n) => n.done) },
      { kind: 'cmd', text: 'Voltar ao apontamento nadir', cmd: 'att_mode', args: { mode: 'nadir' } },
    ],
  },
]
