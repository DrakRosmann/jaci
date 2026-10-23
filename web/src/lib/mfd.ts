// Contas e cores compartilhadas pelas páginas dos MFDs: a paleta escolhida,
// os μ dos corpos, direções em relação ao nariz da nave, o vetor de uma
// manobra e o movimento relativo a um alvo.

import { add, bodyAxes, cross, dot, norm, scale, unit, type Q, type V3 } from './attitude'
import { pilotPrefs } from './pilot'
import type { Node, Vehicle } from '../types'

export interface Palette {
  sky: string
  ground: string
  line: string
  green: string
  amber: string
  cyan: string
  magenta: string
  red: string
  dim: string
  grid: string
}

const PALETTES: Record<string, Palette> = {
  cores: { sky: '#1f5fa8', ground: '#7a4a22', line: '#e8f0ff', green: '#4cff8a', amber: '#ffc23d', cyan: '#4cc3ff', magenta: '#e879f9', red: '#ff4d5e', dim: '#7f8fae', grid: '#13213b' },
  verde: { sky: '#0b3a1c', ground: '#031208', line: '#9dffbd', green: '#4cff8a', amber: '#d6ff8a', cyan: '#5cffc0', magenta: '#b8ffcf', red: '#ff6a6a', dim: '#2f8a52', grid: '#0b2a17' },
  ambar: { sky: '#3d2700', ground: '#120900', line: '#ffd88a', green: '#ffbf47', amber: '#ffe08a', cyan: '#ffcf66', magenta: '#ffd27a', red: '#ff6a4a', dim: '#9a6c22', grid: '#2a1a04' },
}

/** As cores das telas, conforme a preferência do console. */
export function palette(): Palette {
  return PALETTES[pilotPrefs().palette] ?? PALETTES.cores
}

/** μ dos corpos (m³/s²). */
export const MU: Record<string, number> = {
  earth: 3.986004418e14,
  moon: 4.9048695e12,
  mars: 4.282837e13,
  venus: 3.24859e14,
  mercury: 2.2032e13,
  jupiter: 1.26686534e17,
  saturn: 3.7931187e16,
  uranus: 5.793939e15,
  neptune: 6.836529e15,
  sun: 1.32712440018e20,
}

/** Ângulos (graus) de uma direção em relação à frente da nave: para a direita e para cima na tela. */
export function offsets(d: V3, q: Q): { dx: number; dy: number; front: boolean; angle: number } {
  const { fwd, right, down } = bodyAxes(q)
  const f = dot(d, fwd)
  return {
    dx: (Math.atan2(dot(d, right), f) * 180) / Math.PI,
    dy: (Math.atan2(-dot(d, down), f) * 180) / Math.PI,
    front: f > 0,
    angle: (Math.acos(Math.max(-1, Math.min(1, f / (norm(d) || 1)))) * 180) / Math.PI,
  }
}

/** Eixos RTN (radial, transversal, normal) de uma órbita, como os do núcleo. */
export function rtnAxes(r: V3, v: V3): [V3, V3, V3] {
  const x = unit(r)
  const z = unit(cross(r, v))
  return [x, cross(z, x), z]
}

/** A próxima queima ainda não feita. */
export function nextNode(v: Vehicle): Node | undefined {
  return (v.nodes ?? []).filter((n) => !n.done).sort((a, b) => a.t - b.t)[0]
}

/** Direção (inercial, unitária) do Δv de uma queima, com a órbita de agora. */
export function nodeDirection(v: Vehicle, n: Node): V3 | null {
  if (!v.r || !v.v || n.dv_total <= 0) return null
  const [R, T, N] = rtnAxes(v.r as V3, v.v as V3)
  return unit(add(add(scale(R, n.dv[0]), scale(T, n.dv[1])), scale(N, n.dv[2])))
}

/** Veículos que podem ser alvo: em volta do mesmo corpo, voando, sem detritos. */
export function targetCandidates(v: Vehicle, all: Vehicle[]): Vehicle[] {
  return all.filter((o) => o.id !== v.id && o.r && o.v && o.central === v.central && o.kind !== 'debris' && !['prelaunch', 'destroyed'].includes(o.phase))
}

/** O alvo: o da aproximação automática, o escolhido no console ou o mais perto. */
export function pickTarget(v: Vehicle, all: Vehicle[]): Vehicle | undefined {
  const c = targetCandidates(v, all)
  const want = v.guidance?.dock_target ?? pilotPrefs().target
  const chosen = c.find((o) => o.id === want)
  if (chosen || !v.r) return chosen
  return c.sort((a, b) => norm(sub(a.r as V3, v.r as V3)) - norm(sub(b.r as V3, v.r as V3)))[0]
}

const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]]

export interface Relative {
  /** Posição e velocidade da nave em relação ao alvo, no LVLH do alvo: V-bar (x), H-bar (y), R-bar (z, para cima). */
  pos: V3
  vel: V3
  range: number
  rate: number
  /** Direção inercial da nave até o alvo. */
  los: V3
  /** Aproximação máxima em linha reta: tempo e distância. */
  tca: number
  miss: number
}

export function relative(v: Vehicle, o: Vehicle): Relative | null {
  if (!v.r || !v.v || !o.r || !o.v) return null
  const dr = sub(v.r as V3, o.r as V3)
  const dv = sub(v.v as V3, o.v as V3)
  const [R, T, N] = rtnAxes(o.r as V3, o.v as V3)
  const range = norm(dr)
  const vv = dot(dv, dv)
  const tca = vv > 1e-12 ? -dot(dr, dv) / vv : 0
  const miss = tca > 0 ? norm(add(dr, scale(dv, tca))) : range
  return {
    pos: [dot(dr, T), dot(dr, N), dot(dr, R)],
    vel: [dot(dv, T), dot(dv, N), dot(dv, R)],
    range,
    rate: range > 0 ? dot(dr, dv) / range : 0,
    los: unit(scale(dr, -1)),
    tca,
    miss,
  }
}

/** Empuxo máximo (kN) do estágio que está queimando ou que queima a seguir. */
export function ratedThrust(v: Vehicle): number {
  const st = (v.stages ?? []).find((s) => (s.engines ?? []).length > 0 && ((s.prop ?? 0) > 0 || (s.engines ?? []).some((e) => e.state === 'running')))
  return (st?.engines ?? []).filter((e) => e.state !== 'failed').reduce((sum, e) => sum + e.rated, 0)
}

/** Gravidade na posição da nave (m/s²). */
export function gravity(v: Vehicle): number {
  const mu = MU[v.central] ?? MU.earth
  return v.r ? mu / dot(v.r as V3, v.r as V3) : 9.81
}
