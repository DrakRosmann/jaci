import { describe, expect, it } from 'vitest'
import { countdownCallout } from './audio'

describe('locução da contagem', () => {
  it('fala ao cruzar cada marca', () => {
    expect(countdownCallout(61, 59.8, 'pt')).toBe('T menos 1 minuto')
    expect(countdownCallout(601, 599, 'pt')).toBe('T menos 10 minutos')
    expect(countdownCallout(30.1, 29.9, 'en')).toBe('T minus 30 seconds')
    expect(countdownCallout(10.2, 9.95, 'pt')).toBe('10')
    expect(countdownCallout(1.1, 0.9, 'pt')).toBe('1')
    expect(countdownCallout(0.2, -0.1, 'pt')).toBe('Decolagem')
  })

  it('fica quieta entre as marcas, parada ou voltando', () => {
    expect(countdownCallout(45, 44.8, 'pt')).toBeNull()
    expect(countdownCallout(9.5, 9.5, 'pt')).toBeNull()
    expect(countdownCallout(600, 610, 'pt')).toBeNull()
  })

  it('pulando várias marcas, fala só a última', () => {
    expect(countdownCallout(12, 5.5, 'pt')).toBe('6')
  })
})
