// Contas de atitude e órbita para as telas do controle manual (MFD): eixos
// da nave a partir do quatérnio, o horizonte local, arfagem, rumo e
// rolamento, e os elementos de uma órbita a partir de r e v.
//
// Convenção do núcleo: o quatérnio leva do corpo ao inercial; o eixo X do
// corpo é o do empuxo (frente) e o Z aponta para "baixo" (nadir em atitude
// nadir); Y = Z × X.

export type V3 = [number, number, number]
export type Q = [number, number, number, number] // w, x, y, z

export const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
export const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
export const norm = (a: V3) => Math.hypot(a[0], a[1], a[2])
export const scale = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k]
export const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
export const unit = (a: V3): V3 => {
  const n = norm(a) || 1
  return [a[0] / n, a[1] / n, a[2] / n]
}

/** Gira `v` pelo quatérnio (corpo → inercial). */
export function rotate(q: Q, v: V3): V3 {
  const [w, x, y, z] = q
  const u: V3 = [x, y, z]
  const t = scale(cross(u, v), 2)
  return add(add(v, scale(t, w)), cross(u, t))
}

/** Eixos da nave no inercial: frente (X), direita (Y) e baixo (Z). */
export function bodyAxes(q: Q): { fwd: V3; right: V3; down: V3 } {
  return { fwd: rotate(q, [1, 0, 0]), right: rotate(q, [0, 1, 0]), down: rotate(q, [0, 0, 1]) }
}

/** Referencial local: para cima, leste e norte (polo do corpo no Z inercial). */
export function localFrame(r: V3): { up: V3; east: V3; north: V3 } {
  const up = unit(r)
  let east = cross([0, 0, 1], up)
  if (norm(east) < 1e-9) east = [0, 1, 0]
  east = unit(east)
  return { up, east, north: cross(up, east) }
}

/** Arfagem (acima do horizonte), rumo (do norte, para leste) e rolamento, em graus. */
export function eulerLocal(q: Q, r: V3): { pitch: number; heading: number; roll: number } {
  const { fwd, down } = bodyAxes(q)
  const { up, east, north } = localFrame(r)
  const pitch = (Math.asin(Math.max(-1, Math.min(1, dot(fwd, up)))) * 180) / Math.PI
  const heading = (((Math.atan2(dot(fwd, east), dot(fwd, north)) * 180) / Math.PI) + 360) % 360
  // Rolamento: o "baixo" da nave em relação ao baixo local, em torno da frente.
  const localDown = unit(add(scale(up, -1), scale(fwd, dot(fwd, up))))
  const bodyDown = unit(add(down, scale(fwd, -dot(down, fwd))))
  const roll = (Math.atan2(dot(cross(localDown, bodyDown), fwd), dot(localDown, bodyDown)) * 180) / Math.PI
  return { pitch, heading, roll: Math.abs(pitch) > 89.5 ? 0 : roll }
}

/** Direções de referência do navball, no inercial. */
export function markers(r: V3, v: V3): Record<string, V3> {
  const pro = unit(v)
  const n = unit(cross(r, v))
  const radial = unit(cross(n, pro)) // no plano, para fora
  return { prograde: pro, retrograde: scale(pro, -1), normal: n, antinormal: scale(n, -1), radial_out: radial, radial_in: scale(radial, -1) }
}

export interface Elements {
  a: number
  e: number
  /** Argumento do periastro medido no plano (rad) e anomalia verdadeira (rad). */
  argp: number
  nu: number
  periapsis: number
  apoapsis: number
  /** Vetor excentricidade (inercial) e normal do plano. */
  evec: V3
  h: V3
}

/** Elementos de uma órbita a partir de r (m) e v (m/s). */
export function elements(r: V3, v: V3, mu: number): Elements {
  const rn = norm(r)
  const h = cross(r, v)
  const evec = add(scale(cross(v, h), 1 / mu), scale(r, -1 / rn))
  const e = norm(evec)
  const energy = dot(v, v) / 2 - mu / rn
  const a = Math.abs(energy) > 1e-12 ? -mu / (2 * energy) : Infinity
  // Ângulos no plano: X do plano no periastro (ou na posição, se circular).
  const xAxis = e > 1e-6 ? unit(evec) : unit(r)
  const yAxis = cross(unit(h), xAxis)
  const nu = Math.atan2(dot(r, yAxis), dot(r, xAxis))
  const p = dot(h, h) / mu
  return { a, e, argp: 0, nu, periapsis: p / (1 + e), apoapsis: e < 1 ? p / (1 - e) : Infinity, evec, h }
}

/** Quatérnio da rotação cujas colunas são os eixos X, Y e Z do corpo (no inercial). */
export function quatFromAxes(x: V3, y: V3, z: V3): Q {
  const [m00, m10, m20] = x
  const [m01, m11, m21] = y
  const [m02, m12, m22] = z
  const tr = m00 + m11 + m22
  if (tr > 0) {
    const s = Math.sqrt(tr + 1) * 2
    return [s / 4, (m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s]
  }
  if (m00 > m11 && m00 > m22) {
    const s = Math.sqrt(1 + m00 - m11 - m22) * 2
    return [(m21 - m12) / s, s / 4, (m01 + m10) / s, (m02 + m20) / s]
  }
  if (m11 > m22) {
    const s = Math.sqrt(1 + m11 - m00 - m22) * 2
    return [(m02 - m20) / s, (m01 + m10) / s, s / 4, (m12 + m21) / s]
  }
  const s = Math.sqrt(1 + m22 - m00 - m11) * 2
  return [(m10 - m01) / s, (m02 + m20) / s, (m12 + m21) / s, s / 4]
}

/** Como o Quat::look do núcleo: frente e baixo. */
export function look(fwd: V3, down: V3): Q {
  const x = unit(fwd)
  const z = unit(add(down, scale(x, -dot(down, x))))
  return quatFromAxes(x, cross(z, x), z)
}
