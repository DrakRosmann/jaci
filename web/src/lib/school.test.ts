import { describe, expect, it } from 'vitest'
import { compile } from './expr'
import { starsFor } from './school'
import type { Snapshot, Vehicle } from '../types'

describe('escola de controle', () => {
  it('estrelas pelo tempo e pelas dicas', () => {
    expect(starsFor(100, 0, 180)).toBe(3)
    expect(starsFor(200, 0, 180)).toBe(2)
    expect(starsFor(100, 2, 180)).toBe(2)
    expect(starsFor(100, 3, 180)).toBe(1)
    expect(starsFor(500, 0, 180)).toBe(1)
    expect(starsFor(9999, 0, null)).toBe(3)
  })

  it('condições com a tela e com a estação', () => {
    const v = {
      id: 1,
      name: 'Estação espacial',
      phase: 'orbit',
      safe_mode: true,
      nodes: [{ done: false }, { done: true }],
      systems: { adcs: { mode: 'sun' }, eclss: { modules: [{ leak: true, isolated: true }, { leak: false, isolated: false }] } },
      tm: {},
    } as unknown as Vehicle
    const s = { warp: 100, alarms: [{ vehicle: 1, active: true, acked: true }], launch: { positions: [{ polled: true }], state: 'idle' } } as unknown as Snapshot
    const ui = { view: 'systems', selected: 'Estação espacial' }
    for (const c of ["ui.view == 'systems'", "selected == 'Estação espacial'", 'warp >= 100', 'launch.polled', 'burns == 1', 'alarms > 0', 'acked', 'leak_isolated', 'safe_mode', "att_mode == 'sun'"]) {
      expect(compile(c)(v, s, ui), c).toBe(true)
    }
  })
})

describe('lições do repositório', () => {
  it('toda condição until é válida', async () => {
    const { readdirSync, readFileSync } = await import('node:fs')
    const { fileURLToPath } = await import('node:url')
    const dir = fileURLToPath(new URL('../../../missoes/escola', import.meta.url))
    const { parse } = await import('./expr')
    let n = 0
    for (const f of readdirSync(dir).filter((x: string) => x.endsWith('.toml'))) {
      for (const m of readFileSync(`${dir}/${f}`, 'utf8').matchAll(/^until = "(.*)"$/gm)) {
        expect(() => parse(m[1]), `${f}: ${m[1]}`).not.toThrow()
        n++
      }
    }
    expect(n).toBeGreaterThan(20)
  })
})
