import { describe, expect, it } from 'vitest'
import { AU, MU_SUN, hohmannWindow, lambert, longitude, planetPosition } from './ephem'
import { norm, type V3 } from './attitude'

const DAY = 86_400

describe('efemérides e transferências', () => {
  it('a Terra em J2000 está a ~100° de longitude, a 0,983 UA', () => {
    const p = planetPosition('earth', 0)!
    expect((longitude(p) * 180) / Math.PI).toBeCloseTo(100.4, 0)
    expect(norm(p) / AU).toBeCloseTo(0.983, 2)
  })

  it('Hohmann Terra → Marte: ~259 dias, fase de ~44°, janela no fim de 2026', () => {
    // 2026-10-23 em segundos desde J2000.
    const t = (Date.UTC(2026, 9, 23, 12) / 1000) - 946_728_000
    const w = hohmannWindow('earth', 'mars', t)!
    expect(w.tof / DAY).toBeGreaterThan(240)
    expect(w.tof / DAY).toBeLessThan(290)
    expect((w.phaseReq * 180) / Math.PI).toBeGreaterThan(38)
    expect((w.phaseReq * 180) / Math.PI).toBeLessThan(50)
    expect(w.synodic / DAY).toBeCloseTo(780, -1)
    const next = new Date((w.next + 946_728_000) * 1000)
    expect(next.getUTCFullYear()).toBe(2026)
    expect(next.getUTCMonth()).toBeGreaterThanOrEqual(9)
  })

  it('Lambert reproduz a velocidade de uma órbita circular', () => {
    const r = AU
    const v = Math.sqrt(MU_SUN / r)
    const th = Math.PI / 3
    const r2: V3 = [r * Math.cos(th), r * Math.sin(th), 0]
    const [v1, v2] = lambert([r, 0, 0], r2, th / (v / r))!
    expect(v1[0]).toBeCloseTo(0, -1)
    expect(v1[1] / v).toBeCloseTo(1, 6)
    expect(norm(v2) / v).toBeCloseTo(1, 6)
  })
})
