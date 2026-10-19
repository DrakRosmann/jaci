import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'
import { EN } from './i18n-en'
import { translate } from './i18n'

function files(dir: string): string[] {
  return readdirSync(dir).flatMap((f) => {
    const p = join(dir, f)
    return statSync(p).isDirectory() ? files(p) : /\.tsx?$/.test(f) && !f.endsWith('.test.ts') && !f.endsWith('i18n.ts') ? [p] : []
  })
}

describe('tradução', () => {
  it("toda chave usada com t('…') tem inglês", () => {
    const missing: string[] = []
    for (const f of files(fileURLToPath(new URL('..', import.meta.url)))) {
      const src = readFileSync(f, 'utf8')
      for (const m of src.matchAll(/\bt\(\s*'((?:[^'\\]|\\.)*)'/g)) if (!(m[1] in EN)) missing.push(`${f}: ${m[1]}`)
    }
    expect(missing).toEqual([])
  })

  it('traduz nomes compostos da telemetria', () => {
    expect(translate('2º estágio Motor de vácuo: empuxo')).toBe('2nd stage Vacuum engine: thrust')
    expect(translate('1º estágio Motor 3: pressão de câmara')).toBe('1st stage Engine 3: chamber pressure')
    expect(translate('Temperatura: Bateria')).toBe('Temperature: Battery')
  })
})
