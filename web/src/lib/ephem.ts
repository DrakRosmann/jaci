// Efemérides dos planetas para as vistas interplanetárias: os elementos
// médios de Standish (JPL), os mesmos do núcleo (sim/src/bodies.rs), no plano
// da eclíptica J2000; a equação de Kepler; transferências de Hohmann e de
// Lambert para desenhar e comparar janelas de lançamento.

import { add, cross, dot, norm, scale, unit, type V3 } from './attitude'

export const AU = 149_597_870_700
export const MU_SUN = 1.32712440018e20
const DEG = Math.PI / 180
const CENTURY = 36_525 * 86_400

/** a (UA), e, I, L, ϖ, Ω e as taxas por século. */
const STANDISH: Record<string, [number[], number[]]> = {
  mercury: [
    [0.38709927, 0.20563593, 7.00497902, 252.2503235, 77.45779628, 48.33076593],
    [0.00000037, 0.00001906, -0.00594749, 149472.67411175, 0.16047689, -0.12534081],
  ],
  venus: [
    [0.72333566, 0.00677672, 3.39467605, 181.9790995, 131.60246718, 76.67984255],
    [0.0000039, -0.00004107, -0.0007889, 58517.81538729, 0.00268329, -0.27769418],
  ],
  earth: [
    [1.00000261, 0.01671123, -0.00001531, 100.46457166, 102.93768193, 0],
    [0.00000562, -0.00004392, -0.01294668, 35999.37244981, 0.32327364, 0],
  ],
  mars: [
    [1.52371034, 0.0933941, 1.84969142, -4.55343205, -23.94362959, 49.55953891],
    [0.00001847, 0.00007882, -0.00813131, 19140.30268499, 0.44441088, -0.29257343],
  ],
  jupiter: [
    [5.202887, 0.04838624, 1.30439695, 34.39644051, 14.72847983, 100.47390909],
    [-0.00011607, -0.00013253, -0.00183714, 3034.74612775, 0.21252668, 0.20469106],
  ],
  saturn: [
    [9.53667594, 0.05386179, 2.48599187, 49.95424423, 92.59887831, 113.66242448],
    [-0.0012506, -0.00050991, 0.00193609, 1222.49362201, -0.41897216, -0.28867794],
  ],
  uranus: [
    [19.18916464, 0.04725744, 0.77263783, 313.23810451, 170.9542763, 74.01692503],
    [-0.00196176, -0.00004397, -0.00242939, 428.48202785, 0.40805281, 0.04240589],
  ],
  neptune: [
    [30.06992276, 0.00859048, 1.77004347, -55.12002969, 44.96476227, 131.78422574],
    [0.00026291, 0.00005105, 0.00035372, 218.45945325, -0.32241464, -0.00508664],
  ],
}

export const PLANETS = Object.keys(STANDISH)

interface Kepler {
  a: number
  e: number
  i: number
  node: number
  argp: number
  m: number
}

function kepler(id: string, t: number): Kepler | null {
  const k = STANDISH[id]
  if (!k) return null
  const tc = t / CENTURY
  const [a, e, i, l, varpi, node] = k[0].map((x, j) => x + k[1][j] * tc)
  return { a: a * AU, e, i: i * DEG, node: node * DEG, argp: (varpi - node) * DEG, m: (l - varpi) * DEG }
}

/** Do plano da órbita (x para o periélio) para a eclíptica. */
function toEcliptic(el: Kepler, x: number, y: number): V3 {
  const [cO, sO, cw, sw, ci, si] = [Math.cos(el.node), Math.sin(el.node), Math.cos(el.argp), Math.sin(el.argp), Math.cos(el.i), Math.sin(el.i)]
  return [(cO * cw - sO * sw * ci) * x + (-cO * sw - sO * cw * ci) * y, (sO * cw + cO * sw * ci) * x + (-sO * sw + cO * cw * ci) * y, sw * si * x + cw * si * y]
}

function eccentricAnomaly(m: number, e: number): number {
  let E = e < 0.8 ? m : Math.PI
  for (let k = 0; k < 30; k++) {
    const d = (E - e * Math.sin(E) - m) / (1 - e * Math.cos(E))
    E -= d
    if (Math.abs(d) < 1e-12) break
  }
  return E
}

/** Posição heliocêntrica do planeta (m, eclíptica J2000) no instante t (s desde J2000). */
export function planetPosition(id: string, t: number): V3 | null {
  const el = kepler(id, t)
  if (!el) return null
  const E = eccentricAnomaly(el.m, el.e)
  return toEcliptic(el, el.a * (Math.cos(E) - el.e), el.a * Math.sqrt(1 - el.e * el.e) * Math.sin(E))
}

/** Velocidade heliocêntrica (m/s, eclíptica), por diferença central. */
export function planetVelocity(id: string, t: number): V3 | null {
  const a = planetPosition(id, t - 60)
  const b = planetPosition(id, t + 60)
  return a && b ? scale(add(b, scale(a, -1)), 1 / 120) : null
}

/** A órbita inteira do planeta, vista de cima (x, y na eclíptica). */
export function orbitPath(id: string, t: number, n = 180): [number, number][] {
  const el = kepler(id, t)
  if (!el) return []
  const b = el.a * Math.sqrt(1 - el.e * el.e)
  return Array.from({ length: n + 1 }, (_, k) => {
    const E = (2 * Math.PI * k) / n
    const p = toEcliptic(el, el.a * (Math.cos(E) - el.e), b * Math.sin(E))
    return [p[0], p[1]] as [number, number]
  })
}

/** Período orbital médio (s). */
export function period(id: string, t = 0): number {
  const el = kepler(id, t)
  return el ? 2 * Math.PI * Math.sqrt(el.a ** 3 / MU_SUN) : NaN
}

/** Longitude eclíptica (rad). */
export const longitude = (p: V3) => Math.atan2(p[1], p[0])

const wrap = (a: number) => ((a % (2 * Math.PI)) + 2 * Math.PI) % (2 * Math.PI)

export interface Window {
  to: string
  /** Hohmann entre as distâncias de agora (origem) e média (destino). */
  tof: number
  c3: number
  vinfArr: number
  /** Ângulo de fase (destino − origem, rad, 0 a 2π) agora e o da transferência. */
  phase: number
  phaseReq: number
  /** Próxima janela (s desde J2000) e o período sinódico (s). */
  next: number
  synodic: number
}

/** A janela de Hohmann da origem para o destino, a partir do instante t. */
export function hohmannWindow(from: string, to: string, t: number): Window | null {
  const p1 = planetPosition(from, t)
  const p2 = planetPosition(to, t)
  const k2 = kepler(to, t)
  if (!p1 || !p2 || !k2) return null
  const r1 = norm(p1)
  const r2 = k2.a
  const at = (r1 + r2) / 2
  const tof = Math.PI * Math.sqrt(at ** 3 / MU_SUN)
  const v1 = Math.sqrt(MU_SUN / r1) * (Math.sqrt((2 * r2) / (r1 + r2)) - 1)
  const v2 = Math.sqrt(MU_SUN / r2) * (1 - Math.sqrt((2 * r1) / (r1 + r2)))
  const n1 = (2 * Math.PI) / period(from, t)
  const n2 = (2 * Math.PI) / period(to, t)
  const phase = wrap(longitude(p2) - longitude(p1))
  const phaseReq = wrap(Math.PI - n2 * tof)
  const rel = n2 - n1
  const synodic = (2 * Math.PI) / Math.abs(rel)
  // Quando a fase (que anda a n2 − n1) chega à da transferência.
  const dt = wrap((phaseReq - phase) * Math.sign(rel)) / Math.abs(rel)
  return { to, tof, c3: (v1 / 1e3) ** 2, vinfArr: Math.abs(v2) / 1e3, phase, phaseReq, next: t + dt, synodic }
}

// ── Lambert (variáveis universais, uma volta, sentido prógrado) ──

function stumpffC(z: number) {
  return z > 1e-8 ? (1 - Math.cos(Math.sqrt(z))) / z : z < -1e-8 ? (Math.cosh(Math.sqrt(-z)) - 1) / -z : 0.5 - z / 24
}
function stumpffS(z: number) {
  if (z > 1e-8) {
    const s = Math.sqrt(z)
    return (s - Math.sin(s)) / s ** 3
  }
  if (z < -1e-8) {
    const s = Math.sqrt(-z)
    return (Math.sinh(s) - s) / s ** 3
  }
  return 1 / 6 - z / 120
}

/** Velocidades de partida e chegada da transferência de r1 a r2 em `tof` segundos. */
export function lambert(r1: V3, r2: V3, tof: number, mu = MU_SUN): [V3, V3] | null {
  const n1 = norm(r1)
  const n2 = norm(r2)
  let dth = Math.acos(Math.max(-1, Math.min(1, dot(r1, r2) / (n1 * n2))))
  if (cross(r1, r2)[2] < 0) dth = 2 * Math.PI - dth
  const A = Math.sin(dth) * Math.sqrt((n1 * n2) / (1 - Math.cos(dth)))
  if (!isFinite(A) || Math.abs(A) < 1e-9) return null
  const y = (z: number) => n1 + n2 + (A * (z * stumpffS(z) - 1)) / Math.sqrt(stumpffC(z))
  const F = (z: number) => {
    const yz = y(z)
    return (yz / stumpffC(z)) ** 1.5 * stumpffS(z) + A * Math.sqrt(yz) - Math.sqrt(mu) * tof
  }
  let lo = -4 * Math.PI * Math.PI
  while (y(lo) < 0 && lo < 4 * Math.PI * Math.PI) lo += 0.1
  let hi = 4 * Math.PI * Math.PI - 1e-6
  if (!(F(lo) < 0 && F(hi) > 0)) return null
  for (let k = 0; k < 200; k++) {
    const mid = (lo + hi) / 2
    if (F(mid) > 0) hi = mid
    else lo = mid
  }
  const z = (lo + hi) / 2
  const yz = y(z)
  const f = 1 - yz / n1
  const g = A * Math.sqrt(yz / mu)
  const gd = 1 - yz / n2
  return [scale(add(r2, scale(r1, -f)), 1 / g), scale(add(scale(r2, gd), scale(r1, -1)), 1 / g)]
}

/** Pontos (x, y na eclíptica) do arco da transferência de r1 a r2 com a velocidade de partida v1. */
export function transferArc(r1: V3, v1: V3, r2: V3, mu = MU_SUN, n = 120): [number, number][] {
  const h = cross(r1, v1)
  const evec = add(scale(cross(v1, h), 1 / mu), scale(r1, -1 / norm(r1)))
  const e = norm(evec)
  const x = e > 1e-9 ? unit(evec) : unit(r1)
  const y = cross(unit(h), x)
  const p = dot(h, h) / mu
  const nu1 = Math.atan2(dot(r1, y), dot(r1, x))
  let nu2 = Math.atan2(dot(r2, y), dot(r2, x))
  while (nu2 < nu1) nu2 += 2 * Math.PI
  return Array.from({ length: n + 1 }, (_, k) => {
    const nu = nu1 + ((nu2 - nu1) * k) / n
    const r = p / (1 + e * Math.cos(nu))
    const q = add(scale(x, r * Math.cos(nu)), scale(y, r * Math.sin(nu)))
    return [q[0], q[1]] as [number, number]
  })
}
