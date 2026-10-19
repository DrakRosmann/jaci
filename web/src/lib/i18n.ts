// Tradução da interface. O texto em português é a própria chave; o dicionário
// inglês fica em i18n-en.ts. O teste i18n.test.ts confere que toda chave usada
// com t('…') no código tem tradução. Textos que vêm do servidor (eventos,
// nomes de telemetria e comandos) passam por tx(): traduzidos quando há,
// inclusive os compostos ("2º estágio Motor de vácuo: empuxo").

import { EN } from './i18n-en'
import { store } from './store'

export function t(s: string, vars?: Record<string, string | number>): string {
  let out = store.ui.lang === 'en' ? (EN[s] ?? s) : s
  if (vars) out = out.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m))
  return out
}

const cache = new Map<string, string>()
let prefixes: string[] | null = null

/** Traduz um texto composto por partes conhecidas. */
export function translate(s: string): string {
  const hit = EN[s]
  if (hit) return hit
  const c = cache.get(s)
  if (c != null) return c
  let out = s
  const i = s.indexOf(': ')
  if (i > 0) {
    out = `${translate(s.slice(0, i))}: ${translate(s.slice(i + 2))}`
  } else {
    const m = s.match(/^(Motor|Booster|SSME) (\d+)$/)
    if (m) out = `${m[1] === 'Motor' ? 'Engine' : m[1]} ${m[2]}`
    else {
      prefixes ??= Object.keys(EN).sort((a, b) => b.length - a.length)
      const p = prefixes.find((k) => s.startsWith(`${k} `))
      if (p) out = `${EN[p]} ${translate(s.slice(p.length + 1))}`
    }
  }
  cache.set(s, out)
  return out
}

/** Texto do servidor: traduz se estiver no dicionário. */
export function tx(s: string | null | undefined): string {
  if (!s) return ''
  return store.ui.lang === 'en' ? translate(s) : s
}
