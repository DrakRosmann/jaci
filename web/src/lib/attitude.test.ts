import { describe, expect, it } from 'vitest'
import { elements, eulerLocal, look, rotate, scale, type V3 } from './attitude'

const r: V3 = [6_778_000, 0, 0] // no equador, longitude 0: para cima = +X, leste = +Y, norte = +Z

describe('atitude local', () => {
  it('nivelado para o leste', () => {
    const q = look([0, 1, 0], [-1, 0, 0])
    const e = eulerLocal(q, r)
    expect(e.pitch).toBeCloseTo(0, 6)
    expect(e.heading).toBeCloseTo(90, 6)
    expect(e.roll).toBeCloseTo(0, 6)
  })

  it('subindo 30° para o norte e rolado 20°', () => {
    const p = (30 * Math.PI) / 180
    const fwd: V3 = [Math.sin(p), 0, Math.cos(p)]
    const baseDown: V3 = [-Math.cos(p), 0, Math.sin(p)]
    // Rola 20° em torno da frente.
    const q0 = look(fwd, baseDown)
    const a = (20 * Math.PI) / 180
    const q: [number, number, number, number] = mul(q0, [Math.cos(a / 2), Math.sin(a / 2), 0, 0])
    const e = eulerLocal(q, r)
    expect(e.pitch).toBeCloseTo(30, 5)
    expect(e.heading).toBeCloseTo(0, 5)
    expect(Math.abs(e.roll)).toBeCloseTo(20, 4)
    expect(rotate(q, [1, 0, 0])[0]).toBeCloseTo(Math.sin(p), 6)
  })
})

describe('elementos', () => {
  it('órbita circular e elíptica', () => {
    const mu = 3.986004418e14
    const vc = Math.sqrt(mu / r[0])
    const c = elements(r, [0, vc, 0], mu)
    expect(c.e).toBeLessThan(1e-9)
    expect(c.a).toBeCloseTo(r[0], 0)
    const el = elements(r, scale([0, 1, 0], vc * 1.1), mu)
    expect(el.periapsis).toBeCloseTo(r[0], -1)
    expect(el.apoapsis).toBeGreaterThan(r[0] * 1.4)
    expect(el.nu).toBeCloseTo(0, 6)
  })
})

function mul(a: [number, number, number, number], b: [number, number, number, number]): [number, number, number, number] {
  return [
    a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
    a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
    a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
    a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
  ]
}
