// Pilotagem pelo teclado no console de controle manual (enquanto ligada, as
// teclas de voo deixam de ser atalhos da interface) e as preferências do
// console: quantos MFDs, as páginas de cada um, as cores das telas, os grupos
// do painel e as teclas. Ficam no navegador.

import { useSyncExternalStore } from 'react'

export type PilotAction =
  | 'pitch_minus'
  | 'pitch_plus'
  | 'yaw_minus'
  | 'yaw_plus'
  | 'roll_minus'
  | 'roll_plus'
  | 'rcs_prograde'
  | 'rcs_retrograde'
  | 'rcs_radial_in'
  | 'rcs_radial_out'
  | 'rcs_normal'
  | 'rcs_antinormal'
  | 'throttle_up'
  | 'throttle_down'
  | 'throttle_full'
  | 'throttle_zero'
  | 'ignite'
  | 'cutoff'
  | 'hold'
  | 'detumble'

/** Ações do teclado, na ordem da tela de personalização, com a tecla padrão. */
export const ACTIONS: [PilotAction, string, string][] = [
  ['pitch_minus', 'Arfagem −', 'w'],
  ['pitch_plus', 'Arfagem +', 's'],
  ['yaw_minus', 'Guinada −', 'a'],
  ['yaw_plus', 'Guinada +', 'd'],
  ['roll_minus', 'Rolamento −', 'q'],
  ['roll_plus', 'Rolamento +', 'e'],
  ['rcs_prograde', 'RCS prógrado', 'i'],
  ['rcs_retrograde', 'RCS retrógrado', 'k'],
  ['rcs_radial_in', 'RCS radial −', 'j'],
  ['rcs_radial_out', 'RCS radial +', 'l'],
  ['rcs_normal', 'RCS normal', 'h'],
  ['rcs_antinormal', 'RCS antinormal', 'n'],
  ['throttle_up', 'Mais potência', 'r'],
  ['throttle_down', 'Menos potência', 'f'],
  ['throttle_full', 'Potência máxima', 'z'],
  ['throttle_zero', 'Potência zero', 'x'],
  ['ignite', 'Ignição', 'b'],
  ['cutoff', 'Corte dos motores', 'o'],
  ['hold', 'Manter atitude', 'g'],
  ['detumble', 'Amortecer rotação', ''],
]

export type MfdPage = 'adi' | 'orb' | 'res' | '3d' | 'map' | 'eng' | 'mnv' | 'dsc' | 'ent' | 'tgt' | 'sys' | 'com' | 'tlm' | 'alm'

/** Páginas dos MFDs: tecla e descrição. */
export const PAGES: [MfdPage, string, string][] = [
  ['adi', 'ADI', 'Horizonte artificial'],
  ['orb', 'ORB', 'Mapa orbital'],
  ['mnv', 'MNV', 'Manobras e diretor de voo'],
  ['eng', 'ENG', 'Motores e empuxo'],
  ['res', 'RES', 'Recursos'],
  ['dsc', 'DSC', 'Descida e pouso'],
  ['ent', 'ENT', 'Reentrada'],
  ['tgt', 'TGT', 'Alvo e aproximação'],
  ['sys', 'SIS', 'Energia, atitude e térmico'],
  ['com', 'COM', 'Comunicações'],
  ['tlm', 'TLM', 'Telemetria escolhida'],
  ['alm', 'ALM', 'Alarmes'],
  ['3d', '3D', 'A nave em 3D'],
  ['map', 'MAPA', 'Traço no solo'],
]

/** Teclas de página na moldura de cima (as outras ficam no MENU). */
export const MAX_KEYS = 6

export const GROUPS: [string, string][] = [
  ['prop', 'PROPULSÃO'],
  ['stages', 'ESTÁGIOS'],
  ['emerg', 'EMERGÊNCIA'],
  ['att', 'ATITUDE'],
  ['rcs', 'TRANSLAÇÃO (RCS)'],
  ['mnv', 'MANOBRAS'],
  ['rdv', 'ENCONTRO'],
  ['rec', 'RECUPERAÇÃO E SISTEMAS'],
]

export interface MfdPrefs {
  page: MfdPage
  keys: MfdPage[]
}

export interface PilotPrefs {
  /** Quantos MFDs (1 a 4). */
  count: number
  size: 's' | 'm' | 'l'
  /** Cores das telas: a colorida ou monocromáticas de fósforo. */
  palette: 'cores' | 'verde' | 'ambar'
  /** O painel de chaves à direita dos MFDs ou embaixo. */
  panel: 'right' | 'below'
  mfds: MfdPrefs[]
  hidden: string[]
  annunciators: boolean
  /** O que o horizonte artificial mostra além da atitude. */
  adiMarkers: boolean
  adiNode: boolean
  adiTarget: boolean
  step: number
  dv: number
  throttleStep: number
  keys: Record<PilotAction, string>
  /** Pontos da página TLM. */
  tlm: string[]
  /** Alvo escolhido para TGT e o encontro. */
  target: number | null
}

export const DEFAULT_PREFS: PilotPrefs = {
  count: 2,
  size: 'm',
  palette: 'cores',
  panel: 'right',
  mfds: [
    { page: 'adi', keys: ['adi', 'mnv', 'eng', 'dsc', 'tgt', '3d'] },
    { page: 'orb', keys: ['orb', 'res', 'sys', 'com', 'alm', 'map'] },
    { page: 'tlm', keys: ['tlm', 'ent', 'eng', 'res', 'alm', 'map'] },
    { page: 'tgt', keys: ['tgt', 'mnv', 'orb', 'sys', 'com', '3d'] },
  ],
  hidden: [],
  annunciators: true,
  adiMarkers: true,
  adiNode: true,
  adiTarget: true,
  step: 5,
  dv: 0.1,
  throttleStep: 10,
  keys: Object.fromEntries(ACTIONS.map(([a, , k]) => [a, k])) as Record<PilotAction, string>,
  tlm: ['nav.alt', 'nav.speed', 'nav.vspeed', 'nav.accel', 'orb.pe', 'orb.ap', 'eps.soc', 'com.margin'],
  target: null,
}

const KEY = 'jaci.pilot'

function load(): PilotPrefs {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? '{}') as Partial<PilotPrefs>
    const valid = (p: unknown): p is MfdPage => PAGES.some(([id]) => id === p)
    const mfds = DEFAULT_PREFS.mfds.map((d, i) => {
      const m = saved.mfds?.[i]
      return m ? { page: valid(m.page) ? m.page : d.page, keys: (m.keys ?? d.keys).filter(valid).slice(0, MAX_KEYS) } : d
    })
    return { ...DEFAULT_PREFS, ...saved, mfds, keys: { ...DEFAULT_PREFS.keys, ...saved.keys } }
  } catch {
    return DEFAULT_PREFS
  }
}

let prefs = load()
const listeners = new Set<() => void>()

export function pilotPrefs(): PilotPrefs {
  return prefs
}

export function setPilotPrefs(patch: Partial<PilotPrefs>) {
  prefs = { ...prefs, ...patch }
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs))
  } catch {
    /* sem armazenamento */
  }
  listeners.forEach((l) => l())
}

export function setMfdPrefs(index: number, patch: Partial<MfdPrefs>) {
  setPilotPrefs({ mfds: prefs.mfds.map((m, i) => (i === index ? { ...m, ...patch } : m)) })
}

export function resetPilotPrefs() {
  setPilotPrefs(DEFAULT_PREFS)
}

export function usePilotPrefs(): PilotPrefs {
  return useSyncExternalStore(
    (l) => (listeners.add(l), () => listeners.delete(l)),
    () => prefs,
  )
}

/** Ação ligada a uma tecla, se houver. */
export function actionFor(key: string): PilotAction | undefined {
  const k = key.toLowerCase()
  return ACTIONS.find(([a]) => prefs.keys[a] === k)?.[0]
}

export const pilot = {
  active: false,
  /** As teclas que estão ligadas a uma ação de voo. */
  keys(): string[] {
    return Object.values(prefs.keys).filter(Boolean)
  },
}
