import { describe, expect, it } from 'vitest'
import { parseHash, shallow } from './store'

describe('endereço', () => {
  it('lê o console e a língua', () => {
    expect(parseHash('#orbit&en')).toEqual({ view: 'orbit', lang: 'en' })
    expect(parseHash('#launch')).toEqual({ view: 'launch' })
    expect(parseHash('#nada')).toEqual({})
    expect(parseHash('')).toEqual({})
  })
})

describe('igualdade rasa', () => {
  it('compara listas e objetos pelos itens', () => {
    const a = { id: 1 }
    expect(shallow([a, 2], [a, 2])).toBe(true)
    expect(shallow([a, 2], [{ id: 1 }, 2])).toBe(false)
    expect(shallow({ 1: 'x', 2: 'y' }, { 1: 'x', 2: 'y' })).toBe(true)
    expect(shallow({ 1: 'x' }, { 1: 'x', 2: 'y' })).toBe(false)
    expect(shallow([1], { 0: 1 })).toBe(false)
    expect(shallow(null, null)).toBe(true)
    expect(shallow<unknown>(null, {})).toBe(false)
  })
})
