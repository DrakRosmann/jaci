import { describe, expect, it } from 'vitest'
import { nodeDirection, relative } from './mfd'
import type { Node, Vehicle } from '../types'

const R = 6_778_000
const V = Math.sqrt(3.986004418e14 / R)

function veh(id: number, r: [number, number, number], v: [number, number, number]): Vehicle {
  return { id, name: `v${id}`, kind: 'capsule', kind_name: '', phase: 'orbit', phase_name: '', central: 'earth', crew: 0, lat: 0, lon: 0, alt: 400, speed: V, r, v }
}

describe('contas dos MFDs', () => {
  it('posição relativa no LVLH do alvo', () => {
    const alvo = veh(1, [R, 0, 0], [0, V, 0])
    // A nave 100 m à frente (V-bar), 50 m acima (R-bar) e se aproximando a 0,5 m/s.
    const nave = veh(2, [R + 50, 100, 0], [0, V - 0.5, 0])
    const rel = relative(nave, alvo)!
    expect(rel.pos[0]).toBeCloseTo(100, 6)
    expect(rel.pos[2]).toBeCloseTo(50, 6)
    expect(rel.range).toBeCloseTo(Math.hypot(100, 50), 6)
    expect(rel.rate).toBeLessThan(0)
    expect(rel.tca).toBeCloseTo(200, 6)
    expect(rel.miss).toBeCloseTo(50, 6)
  })

  it('queima transversal aponta para o prógrado numa órbita circular', () => {
    const v = veh(1, [R, 0, 0], [0, V, 0])
    const n: Node = { id: 1, label: 'x', t: 0, dv: [0, 10, 0], dv_total: 10, executing: false, done: false, remaining: 10, rcs: false, burn: null }
    const d = nodeDirection(v, n)!
    expect(d[0]).toBeCloseTo(0, 9)
    expect(d[1]).toBeCloseTo(1, 9)
    expect(d[2]).toBeCloseTo(0, 9)
  })
})
