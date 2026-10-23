// Condições dos procedimentos em arquivo, ex. "eps.soc > 80 and phase == 'orbit'".
// Um analisador pequeno (sem eval): números, textos entre aspas, nomes
// (telemetria pelo id ou campos do veículo e da contagem), comparações,
// and/or/not e parênteses.

import type { Snapshot, Vehicle } from '../types'

type Tok = { k: 'num'; v: number } | { k: 'str'; v: string } | { k: 'id'; v: string } | { k: 'op'; v: string }

export type Expr =
  | { k: 'lit'; v: number | string | boolean }
  | { k: 'name'; v: string }
  | { k: 'not'; e: Expr }
  | { k: 'bin'; op: string; a: Expr; b: Expr }

function lex(src: string): Tok[] {
  const out: Tok[] = []
  let i = 0
  while (i < src.length) {
    const c = src[i]
    if (/\s/.test(c)) {
      i++
    } else if (/[0-9]/.test(c) || (c === '-' && /[0-9.]/.test(src[i + 1] ?? '') && (out.length === 0 || out[out.length - 1].k === 'op'))) {
      const m = src.slice(i).match(/^-?[0-9]*\.?[0-9]+(e[-+]?[0-9]+)?/i)!
      out.push({ k: 'num', v: Number(m[0]) })
      i += m[0].length
    } else if (c === "'" || c === '"') {
      const end = src.indexOf(c, i + 1)
      if (end < 0) throw new Error(`texto sem fechar a aspa (posição ${i + 1})`)
      out.push({ k: 'str', v: src.slice(i + 1, end) })
      i = end + 1
    } else if (/[A-Za-z_]/.test(c)) {
      const m = src.slice(i).match(/^[A-Za-z_][A-Za-z0-9_.]*/)!
      const w = m[0]
      out.push(['and', 'or', 'not', 'true', 'false'].includes(w) ? { k: 'op', v: w } : { k: 'id', v: w })
      i += w.length
    } else {
      const two = src.slice(i, i + 2)
      if (['<=', '>=', '==', '!='].includes(two)) {
        out.push({ k: 'op', v: two })
        i += 2
      } else if ('<>()'.includes(c)) {
        out.push({ k: 'op', v: c })
        i++
      } else {
        throw new Error(`caractere inesperado "${c}" (posição ${i + 1})`)
      }
    }
  }
  return out
}

/** Lê a condição; erro com o motivo se não der. */
export function parse(src: string): Expr {
  const t = lex(src)
  let p = 0
  const peek = () => t[p]
  const isOp = (v: string) => peek()?.k === 'op' && peek()!.v === v
  const or = (): Expr => {
    let a = and()
    while (isOp('or')) {
      p++
      a = { k: 'bin', op: 'or', a, b: and() }
    }
    return a
  }
  const and = (): Expr => {
    let a = not()
    while (isOp('and')) {
      p++
      a = { k: 'bin', op: 'and', a, b: not() }
    }
    return a
  }
  const not = (): Expr => {
    if (isOp('not')) {
      p++
      return { k: 'not', e: not() }
    }
    return cmp()
  }
  const cmp = (): Expr => {
    const a = atom()
    const tk = peek()
    if (tk?.k === 'op' && ['<', '<=', '>', '>=', '==', '!='].includes(tk.v)) {
      p++
      return { k: 'bin', op: tk.v, a, b: atom() }
    }
    return a
  }
  const atom = (): Expr => {
    const tk = t[p++]
    if (!tk) throw new Error('condição incompleta')
    if (tk.k === 'num' || tk.k === 'str') return { k: 'lit', v: tk.v }
    if (tk.k === 'id') return { k: 'name', v: tk.v }
    if (tk.v === 'true' || tk.v === 'false') return { k: 'lit', v: tk.v === 'true' }
    if (tk.v === '(') {
      const e = or()
      if (!isOp(')')) throw new Error('falta fechar o parêntese')
      p++
      return e
    }
    throw new Error(`não esperava "${tk.v}"`)
  }
  if (t.length === 0) throw new Error('condição vazia')
  const e = or()
  if (p < t.length) throw new Error(`sobrou "${(t[p] as { v: unknown }).v}" no fim`)
  return e
}

/** O que a tela sabe além do instantâneo (console aberto, veículo selecionado). */
export interface UiCtx {
  view?: string
  selected?: string
}

/** Valor de um nome: campos conhecidos, depois telemetria pelo id. */
function lookup(name: string, v: Vehicle, s: Snapshot, ui?: UiCtx): number | string | boolean | null {
  switch (name) {
    case 'ui.view':
      return ui?.view ?? null
    case 'selected':
      return ui?.selected ?? null
    case 'warp':
      return s.replay ? s.replay.speed : s.warp
    case 'launch.polled':
      return s.launch.positions.length > 0 && s.launch.positions.every((p) => p.polled != null)
    case 'burns':
      return (v.nodes ?? []).filter((n) => !n.done).length
    case 'alarms':
      return s.alarms.filter((a) => a.vehicle === v.id && a.active).length
    case 'acked':
      return s.alarms.filter((a) => a.vehicle === v.id).every((a) => a.acked)
    case 'leak_isolated': {
      const mods = v.systems?.eclss?.modules ?? []
      return mods.some((m) => m.leak) && mods.every((m) => !m.leak || m.isolated)
    }
    case 'safe_mode':
      return !!v.safe_mode
    case 'att_mode':
      return v.systems?.adcs.mode ?? null
    case 'phase':
      return v.phase
    case 'kind':
      return v.kind
    case 'alt':
      return v.alt
    case 'speed':
      return v.speed
    case 'link':
      return !!v.systems?.comms.link
    case 'separated':
      return (v.stages ?? []).every((x) => x.spacecraft)
    case 'inserted':
      return !!v.guidance?.inserted
    case 'launch.state':
      return s.launch.state
    case 'launch.t_minus':
      return s.launch.t_minus
    case 'launch.all_go':
      return s.launch.all_go
  }
  const x = v.tm?.[name]
  return x == null ? null : x
}

export function evaluate(e: Expr, v: Vehicle, s: Snapshot, ui?: UiCtx): number | string | boolean | null {
  switch (e.k) {
    case 'lit':
      return e.v
    case 'name':
      return lookup(e.v, v, s, ui)
    case 'not':
      return !evaluate(e.e, v, s, ui)
    case 'bin': {
      if (e.op === 'and') return !!evaluate(e.a, v, s, ui) && !!evaluate(e.b, v, s, ui)
      if (e.op === 'or') return !!evaluate(e.a, v, s, ui) || !!evaluate(e.b, v, s, ui)
      const a = evaluate(e.a, v, s, ui)
      const b = evaluate(e.b, v, s, ui)
      // Sem o valor (ponto que este veículo não tem): a condição não vale.
      if (a == null || b == null) return false
      switch (e.op) {
        case '==':
          return a === b
        case '!=':
          return a !== b
        case '<':
          return a < b
        case '<=':
          return a <= b
        case '>':
          return a > b
        case '>=':
          return a >= b
      }
      return false
    }
  }
}

/** Compila uma condição numa função; a condição inválida vira erro na hora de carregar. */
export function compile(src: string): (v: Vehicle, s: Snapshot, ui?: UiCtx) => boolean {
  const e = parse(src)
  return (v, s, ui) => !!evaluate(e, v, s, ui)
}
