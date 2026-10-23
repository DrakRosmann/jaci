import { describe, expect, it } from 'vitest'
import type { Snapshot, Vehicle } from '../types'
import { compile, parse } from './expr'

const v = { phase: 'orbit', kind: 'satellite', alt: 500, speed: 7600, tm: { 'eps.soc': 85, 'nav.vspeed': -3 }, stages: [{ name: 's', spacecraft: true }], systems: { comms: { link: { station: 'X' } } } } as unknown as Vehicle
const s = { launch: { state: 'hold', t_minus: 590, all_go: true } } as unknown as Snapshot

describe('condições dos procedimentos', () => {
  it('compara telemetria, campos e textos', () => {
    expect(compile('eps.soc > 80')(v, s)).toBe(true)
    expect(compile("eps.soc > 80 and phase == 'orbit'")(v, s)).toBe(true)
    expect(compile('nav.vspeed < -1 or alt > 1000')(v, s)).toBe(true)
    expect(compile('not (link and separated)')(v, s)).toBe(false)
    expect(compile("launch.state == 'hold' and launch.t_minus < 600 and launch.all_go")(v, s)).toBe(true)
    expect(compile('kind != "probe"')(v, s)).toBe(true)
  })

  it('ponto que não existe não vale', () => {
    expect(compile('ecl.pressure > 0')(v, s)).toBe(false)
  })

  it('erros com motivo', () => {
    expect(() => parse('eps.soc >')).toThrow(/incompleta/)
    expect(() => parse('(eps.soc > 1')).toThrow(/parêntese/)
    expect(() => parse("phase == 'orbit")).toThrow(/aspa/)
    expect(() => parse('eps.soc > 1 2')).toThrow(/sobrou/)
    expect(() => parse('a $ b')).toThrow(/inesperado/)
    expect(() => parse('')).toThrow(/vazia/)
  })
})
