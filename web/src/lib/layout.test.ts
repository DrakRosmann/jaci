import { describe, expect, it } from 'vitest'
import { COLS, defaultLayout, settle, type Widget } from './layout'

const w = (i: string, x: number, y: number, ww: number, h: number): Widget => ({ i, type: 'alarms', x, y, w: ww, h })
const overlap = (a: Widget, b: Widget) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h

describe('grade do meu console', () => {
  it('quem foi mexido fica; quem estava embaixo desce', () => {
    const out = settle([w('a', 0, 0, 6, 4), w('b', 0, 4, 6, 4), w('c', 0, 1, 6, 2)], 'c')
    const get = (i: string) => out.find((x) => x.i === i)!
    expect(get('c').y).toBe(1)
    for (const a of out) for (const b of out) if (a !== b) expect(overlap(a, b)).toBe(false)
  })

  it('sobe até encostar e não sai da grade', () => {
    const out = settle([w('a', 10, 20, 6, 3), w('b', -3, 9, 2, 2)])
    expect(out.find((x) => x.i === 'a')!.y).toBe(0)
    expect(out.find((x) => x.i === 'a')!.x).toBe(COLS - 6)
    expect(out.find((x) => x.i === 'b')!.x).toBe(0)
  })

  it('os modelos das posições não se sobrepõem', () => {
    for (const p of [null, 'EECOM', 'FIDO', 'LCC', 'INCO']) {
      const l = settle(defaultLayout(p))
      for (const a of l) for (const b of l) if (a !== b) expect(overlap(a, b)).toBe(false)
    }
  })
})
