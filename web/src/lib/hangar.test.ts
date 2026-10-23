import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { DEFAULT_DESIGN, DEFAULT_MISSION, computeStats, fileId, parseHangar, scenarioToml, vehicleToml, type Design } from './hangar'

const HEAVY: Design = {
  name: 'Pesado com boosters',
  payload: 'capsule',
  payloadMass: 9000,
  crew: 3,
  stages: [
    { boosters: true, propellant: 'solid', diameter: 1.8, length: 16, engine: 's1000', count: 4 },
    { boosters: false, propellant: 'kerolox', diameter: 3.7, length: 38, engine: 'k900', count: 9 },
    { boosters: false, propellant: 'kerolox', diameter: 3.7, length: 11, engine: 'k980v', count: 1 },
  ],
}

const PESADO = { ...DEFAULT_MISSION, site: 'Cabo Canaveral', inc: 28.5, pe: 250, ap: 250 }

describe('hangar', () => {
  it('o foguete padrão sobe e tem Δv para a órbita baixa', () => {
    const s = computeStats(DEFAULT_DESIGN)
    expect(s.liftoffTwr).toBeGreaterThan(1.2)
    expect(s.liftoffTwr).toBeLessThan(2)
    expect(s.dv).toBeGreaterThan(9_300)
    expect(s.warnings).toEqual([])
  })

  it('boosters queimam junto com o primeiro estágio', () => {
    const s = computeStats(HEAVY)
    expect(s.stages[0].name).toContain('Boosters')
    // O núcleo acaba antes dos boosters: boosters + 1º, depois o 2º e a cápsula.
    expect(s.stages.map((x) => x.name)).toEqual(['Boosters + 1º estágio', '2º estágio', 'Cápsula'])
    expect(s.stages.every((x) => x.dv > 0)).toBe(true)
    expect(s.liftoffTwr).toBeGreaterThan(1.15)
  })

  it('o arquivo guarda o projeto e o devolve', () => {
    const text = vehicleToml(HEAVY, DEFAULT_MISSION)
    expect(text).toContain('boosters = true')
    expect(text).toContain('recovery_from = "medio_capsula"')
    expect(text).toContain('crew = 3')
    expect(parseHangar(text)?.design.stages.length).toBe(3)
    expect(scenarioToml(HEAVY, DEFAULT_MISSION, 'x')).toContain('place = "pad"')
    expect(fileId('Pesado com boosters!')).toBe('pesado-com-boosters')
  })

  it('gera os mesmos arquivos que o núcleo testa em voo (sim/tests/hangar)', () => {
    const dir = fileURLToPath(new URL('../../../sim/tests/hangar/', import.meta.url))
    expect(vehicleToml(DEFAULT_DESIGN, DEFAULT_MISSION)).toBe(readFileSync(`${dir}hangar-leve.toml`, 'utf8'))
    expect(vehicleToml(HEAVY, PESADO)).toBe(readFileSync(`${dir}hangar-pesado.toml`, 'utf8'))
  })

  it('escreve os arquivos para o teste de voo do núcleo (HANGAR_OUT=pasta)', () => {
    const out = process.env.HANGAR_OUT
    if (!out) return
    writeFileSync(`${out}/hangar-leve.toml`, vehicleToml(DEFAULT_DESIGN, DEFAULT_MISSION))
    writeFileSync(`${out}/hangar-pesado.toml`, vehicleToml(HEAVY, PESADO))
  })
})
