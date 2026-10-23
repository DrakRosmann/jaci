// "Meu console": blocos numa grade de 12 colunas, guardados por posição da
// sala (no navegador), com modelos prontos para cada posição.

import type { View } from './store'

export type WidgetType = 'map' | 'orbit3d' | 'chart' | 'tiles' | 'alarms' | 'events' | 'clocks' | 'fleet' | 'countdown' | 'timeline' | 'loops'

export interface Widget {
  i: string
  type: WidgetType
  x: number
  y: number
  w: number
  h: number
  /** Pontos de telemetria (gráfico e mostradores). */
  points?: string[]
}

export const COLS = 12
/** Altura de uma linha da grade (px). */
export const ROW = 44

/** Os blocos que existem, com o tamanho inicial. */
export const WIDGETS: { type: WidgetType; name: string; w: number; h: number; points?: string[] }[] = [
  { type: 'clocks', name: 'Relógios', w: 4, h: 3 },
  { type: 'map', name: 'Mapa', w: 8, h: 8 },
  { type: 'orbit3d', name: 'Órbita em 3D', w: 6, h: 8 },
  { type: 'chart', name: 'Gráfico', w: 6, h: 6, points: ['nav.alt'] },
  { type: 'tiles', name: 'Mostradores', w: 4, h: 5, points: ['nav.alt', 'nav.speed', 'orb.pe', 'orb.ap'] },
  { type: 'alarms', name: 'Alarmes', w: 4, h: 5 },
  { type: 'events', name: 'Eventos', w: 6, h: 5 },
  { type: 'fleet', name: 'Frota', w: 6, h: 5 },
  { type: 'countdown', name: 'Contagem', w: 4, h: 4 },
  { type: 'timeline', name: 'Linha do tempo', w: 12, h: 9 },
  { type: 'loops', name: 'Loops', w: 4, h: 6 },
]

let seq = 1
export function newId(): string {
  return `w${Date.now().toString(36)}${(seq++).toString(36)}`
}

function make(type: WidgetType, x: number, y: number, w: number, h: number, points?: string[]): Widget {
  return { i: newId(), type, x, y, w, h, points }
}

/** Modelo inicial de cada posição (sem posição: o de FLIGHT). */
export function defaultLayout(position: string | null): Widget[] {
  switch (position) {
    case 'EECOM':
      return [
        make('tiles', 0, 0, 4, 6, ['eps.soc', 'eps.battery', 'eps.bus', 'ecl.pressure', 'ecl.ppo2', 'ecl.ppco2']),
        make('chart', 4, 0, 8, 6, ['eps.soc']),
        make('alarms', 0, 6, 4, 5),
        make('events', 4, 6, 8, 5),
      ]
    case 'FIDO':
    case 'GNC':
      return [
        make('orbit3d', 0, 0, 7, 8),
        make('tiles', 7, 0, 5, 8, ['orb.pe', 'orb.ap', 'orb.inc', 'orb.period', 'nav.speed', 'nav.dv']),
        make('timeline', 0, 8, 12, 9),
      ]
    case 'LCC':
    case 'BOOSTER':
    case 'RSO':
    case 'METEO':
      return [
        make('countdown', 0, 0, 4, 4),
        make('chart', 4, 0, 8, 6, ['nav.alt', 'nav.q']),
        make('tiles', 0, 4, 4, 6, ['nav.alt', 'nav.speed', 'nav.q', 'nav.accel', 'nav.mach']),
        make('alarms', 4, 6, 8, 4),
      ]
    case 'INCO':
      return [make('map', 0, 0, 8, 8), make('tiles', 8, 0, 4, 8, ['com.margin', 'com.rate', 'com.stored']), make('timeline', 0, 8, 12, 9)]
    default:
      return [make('clocks', 0, 0, 4, 3), make('map', 4, 0, 8, 8), make('alarms', 0, 3, 4, 5), make('fleet', 0, 8, 6, 5), make('events', 6, 8, 6, 5)]
  }
}

const key = (position: string | null) => `jaci.layout.${position ?? 'geral'}`

export function loadLayout(position: string | null): Widget[] {
  try {
    const raw = localStorage.getItem(key(position))
    const list = raw ? (JSON.parse(raw) as Widget[]) : null
    if (Array.isArray(list) && list.every((w) => typeof w.i === 'string' && WIDGETS.some((d) => d.type === w.type))) return list
  } catch {
    /* sem armazenamento ou guardado estragado */
  }
  return defaultLayout(position)
}

export function saveLayout(position: string | null, list: Widget[]) {
  try {
    localStorage.setItem(key(position), JSON.stringify(list))
  } catch {
    /* sem armazenamento */
  }
}

const overlap = (a: Widget, b: Widget) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h

/**
 * Arruma a grade depois de mexer em `moved`: quem ficou por baixo desce, e
 * todos sobem até encostar (como numa parede de tijolos).
 */
export function settle(list: Widget[], moved?: string): Widget[] {
  const out = list.map((w) => ({ ...w, x: Math.max(0, Math.min(COLS - w.w, w.x)), y: Math.max(0, w.y), w: Math.max(2, Math.min(COLS, w.w)), h: Math.max(2, w.h) }))
  // O que foi mexido tem prioridade; o resto, de cima para baixo.
  out.sort((a, b) => (a.i === moved ? -1 : b.i === moved ? 1 : a.y - b.y || a.x - b.x))
  const placed: Widget[] = []
  for (const w of out) {
    if (w.i !== moved) {
      // Sobe até encostar.
      while (w.y > 0 && !placed.some((p) => overlap({ ...w, y: w.y - 1 }, p))) w.y--
    }
    while (placed.some((p) => overlap(w, p))) w.y++
    placed.push(w)
  }
  return placed
}

/** Console de cada posição da sala, aberto ao entrar nela. */
export const POSITION_VIEW: Record<string, View> = {
  FLIGHT: 'overview',
  LCC: 'launch',
  BOOSTER: 'trajectory',
  FIDO: 'orbit',
  GNC: 'systems',
  EECOM: 'systems',
  INCO: 'stations',
  PAYLOAD: 'systems',
  RSO: 'launch',
  METEO: 'launch',
  CAPCOM: 'overview',
  SURGEON: 'systems',
  SIM: 'instructor',
  OBS: 'wall',
}
