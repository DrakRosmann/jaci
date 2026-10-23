// Conexão com o servidor e o estado da interface, num só lugar. Os componentes
// leem com useStore(seletor): a cada mudança o seletor roda de novo, e o
// componente só redesenha quando o valor escolhido muda.

import { useRef, useSyncExternalStore } from 'react'
import type { EventMsg, Hello, Snapshot, TmDef, Vehicle } from '../types'
import { History } from './history'
import { POSITION_VIEW } from './layout'

type Listener = () => void

export type View =
  | 'overview'
  | 'launch'
  | 'trajectory'
  | 'orbit'
  | 'rendezvous'
  | 'systems'
  | 'commands'
  | 'telemetry'
  | 'stations'
  | 'interplanetary'
  | 'procedures'
  | 'instructor'
  | 'ksp'
  | 'timeline'
  | 'custom'
  | 'wall'

export interface UiState {
  view: View
  selected: number | null
  lang: 'pt' | 'en'
  eventsOpen: boolean
  navCollapsed: boolean
}

export const VIEWS: View[] = ['overview', 'launch', 'trajectory', 'orbit', 'rendezvous', 'systems', 'commands', 'telemetry', 'stations', 'interplanetary', 'procedures', 'instructor', 'ksp', 'timeline', 'custom', 'wall']

/** Console e língua pedidos no endereço: /#launch abre o lançamento; /#orbit&en, a órbita em inglês. */
export function parseHash(hash: string): Partial<Pick<UiState, 'view' | 'lang'>> {
  const out: Partial<Pick<UiState, 'view' | 'lang'>> = {}
  for (const part of hash.replace(/^#/, '').split('&')) {
    if ((VIEWS as string[]).includes(part)) out.view = part as View
    if (part === 'telao') out.view = 'wall'
    if (part === 'en' || part === 'pt') out.lang = part
  }
  return out
}

function loadUi(): UiState {
  const base: UiState = { view: 'overview', selected: null, lang: 'pt', eventsOpen: true, navCollapsed: false }
  let ui = base
  try {
    const saved = JSON.parse(localStorage.getItem('jaci.ui') ?? '{}')
    ui = { ...base, ...saved, selected: null }
  } catch {
    /* sem armazenamento */
  }
  return { ...ui, ...parseHash(typeof location === 'undefined' ? '' : location.hash) }
}

class Store {
  hello: Hello | null = null
  snap: Snapshot | null = null
  dictionary: Record<string, TmDef[]> = {}
  events: EventMsg[] = []
  connected = false
  ui: UiState = loadUi()
  history = new History(4000)
  version = 0
  /** Quando chegou o último instantâneo (relógio do navegador, ms). */
  lastSnapAt = 0
  /** Quem está nesta tela, na sala com posições (guardado no navegador). */
  me: { name: string; position: string } | null = loadMe()
  private listeners = new Set<Listener>()
  private ws: WebSocket | null = null
  private reqId = 1
  private pending = new Map<number, (v: any) => void>()
  private retry = 0

  subscribe = (l: Listener) => {
    this.listeners.add(l)
    return () => this.listeners.delete(l)
  }

  private emit() {
    this.version++
    for (const l of this.listeners) l()
  }

  setUi(patch: Partial<UiState>) {
    const before = this.ui
    this.ui = { ...this.ui, ...patch }
    // O console vai para o histórico do navegador (voltar e avançar trocam de console).
    if (typeof location !== 'undefined' && (this.ui.view !== before.view || this.ui.lang !== before.lang)) {
      const hash = `#${this.ui.view === 'wall' ? 'telao' : this.ui.view}${this.ui.lang === 'en' ? '&en' : ''}`
      if (location.hash !== hash) {
        if (this.ui.view !== before.view) history.pushState(null, '', hash)
        else history.replaceState(null, '', hash)
      }
    }
    try {
      const { view, lang, eventsOpen, navCollapsed } = this.ui
      localStorage.setItem('jaci.ui', JSON.stringify({ view, lang, eventsOpen, navCollapsed }))
    } catch {
      /* sem armazenamento */
    }
    this.emit()
  }

  /** O endereço mudou (voltar, avançar ou digitado): segue o que ele pede. */
  followHash = () => {
    const want = parseHash(location.hash)
    if ((want.view && want.view !== this.ui.view) || (want.lang && want.lang !== this.ui.lang)) {
      this.ui = { ...this.ui, ...want }
      this.emit()
    }
  }

  connect() {
    if (this.ws && this.ws.readyState <= WebSocket.OPEN) return
    const token = new URLSearchParams(location.search).get('token')
    const proto = location.protocol === 'https:' ? 'wss' : 'ws'
    const url = `${proto}://${location.host}/ws${token ? `?token=${encodeURIComponent(token)}` : ''}`
    const ws = new WebSocket(url)
    this.ws = ws
    ws.onopen = () => {
      this.connected = true
      this.retry = 0
      this.emit()
    }
    ws.onclose = () => {
      this.connected = false
      this.emit()
      this.retry = Math.min(this.retry + 1, 5)
      setTimeout(() => this.connect(), 500 * this.retry)
    }
    ws.onmessage = (m) => this.receive(JSON.parse(m.data))
  }

  private receive(d: any) {
    switch (d.type) {
      case 'hello':
        this.hello = d
        this.dictionary = d.dictionary
        this.events = d.events
        this.history.clear()
        // Reconectou: volta para a mesma posição.
        if (this.me) this.send({ type: 'join', name: this.me.name, position: this.me.position })
        break
      case 'dictionary':
        this.dictionary = d.dictionary
        break
      case 'library':
        // Um arquivo de missão foi salvo: cenários, catálogo e procedimentos novos.
        if (this.hello) this.hello = { ...this.hello, scenarios: d.scenarios, catalog: d.catalog, procedures: d.procedures }
        break
      case 'history':
        // Telemetria de antes de conectar, para os gráficos.
        this.loadHistory(d.vehicles)
        break
      case 'rewind':
        // Reprodução: pulou para outro ponto. Eventos e gráficos recomeçam dali.
        this.events = d.events
        this.dictionary = d.dictionary
        this.history.clear()
        this.loadHistory(d.history ?? {})
        break
      case 'reload':
        // Cenário ou estado novo: recomeça do zero.
        this.snap = null
        this.events = []
        this.history.clear()
        this.ui = { ...this.ui, selected: null }
        this.ws?.close()
        break
      case 'snapshot': {
        const s = d as Snapshot
        this.snap = s
        this.lastSnapAt = Date.now()
        if (s.events.length) {
          this.events = this.events.concat(s.events)
          if (this.events.length > 3000) this.events = this.events.slice(-2500)
        }
        for (const v of s.vehicles) if (v.tm) this.history.push(v.id, s.t, v.tm)
        if (this.ui.selected == null || !s.vehicles.some((v) => v.id === this.ui.selected)) {
          this.ui = { ...this.ui, selected: defaultVehicle(s.vehicles) }
        }
        break
      }
    }
    if (d.req != null && this.pending.has(d.req)) {
      this.pending.get(d.req)!(d)
      this.pending.delete(d.req)
    }
    this.emit()
  }

  private loadHistory(vehicles: Record<string, { t: number[]; points: Record<string, (number | null)[]> }>) {
    for (const [vid, h] of Object.entries(vehicles)) {
      h.t.forEach((t, i) => {
        const tm: Record<string, number | null> = {}
        for (const [k, col] of Object.entries(h.points)) tm[k] = col[i]
        this.history.push(Number(vid), t, tm)
      })
    }
  }

  /** Entra na sala numa posição (ou sai, com null). */
  async join(me: { name: string; position: string } | null) {
    const r = await this.request<{ ok: boolean; error?: string }>(me ? { type: 'join', ...me } : { type: 'leave' })
    if (r.ok) {
      // Posição nova: abre no console dela.
      const changed = me && me.position !== this.me?.position
      this.me = me
      if (changed && POSITION_VIEW[me.position]) this.setUi({ view: POSITION_VIEW[me.position] })
      try {
        if (me) localStorage.setItem('jaci.me', JSON.stringify(me))
        else localStorage.removeItem('jaci.me')
      } catch {
        /* sem armazenamento */
      }
      this.emit()
    }
    return r
  }

  chat(text: string, channel: string) {
    return this.request<{ ok: boolean; error?: string }>({ type: 'chat', text, channel })
  }

  /** Endereço do CSV da telemetria de um veículo (com a senha, se houver). */
  csvUrl(vehicle: number): string {
    const token = new URLSearchParams(location.search).get('token')
    return `/api/csv?vehicle=${vehicle}${token ? `&token=${encodeURIComponent(token)}` : ''}`
  }

  send(msg: object) {
    if (this.ws?.readyState === WebSocket.OPEN) this.ws.send(JSON.stringify(msg))
  }

  /** Envia e espera a resposta (pelo campo req). */
  request<T = any>(msg: object, timeoutMs = 30000): Promise<T> {
    const req = this.reqId++
    return new Promise((resolve, reject) => {
      this.pending.set(req, resolve)
      this.send({ ...msg, req })
      setTimeout(() => {
        if (this.pending.has(req)) {
          this.pending.delete(req)
          reject(new Error('sem resposta do servidor'))
        }
      }, timeoutMs)
    })
  }

  command(vehicle: number, id: string, args: Record<string, unknown> = {}, opts: { armed?: boolean; at?: string; in?: number } = {}) {
    return this.request<{ ok: boolean; error?: string; id?: number }>({ type: 'command', vehicle, cmd: { id, args }, armed: !!opts.armed, at: opts.at, in: opts.in })
  }

  sim(action: string, extra: Record<string, unknown> = {}) {
    return this.request<{ ok?: boolean; error?: string; message?: string }>({ type: 'sim', action, ...extra })
  }

  vehicle(id: number | null | undefined): Vehicle | undefined {
    return this.snap?.vehicles.find((v) => v.id === id)
  }

  get selected(): Vehicle | undefined {
    return this.vehicle(this.ui.selected)
  }
}

function loadMe(): { name: string; position: string } | null {
  try {
    const m = JSON.parse(localStorage.getItem('jaci.me') ?? 'null')
    return m && typeof m.name === 'string' && typeof m.position === 'string' ? m : null
  } catch {
    return null
  }
}

/** O veículo mais interessante: o da plataforma, senão o primeiro que não é detrito. */
export function defaultVehicle(vs: Vehicle[]): number | null {
  const pad = vs.find((v) => v.phase === 'prelaunch')
  const live = vs.find((v) => v.kind !== 'debris' && v.kind !== 'station' && !['destroyed'].includes(v.phase))
  return (pad ?? live ?? vs.find((v) => v.kind !== 'debris') ?? vs[0])?.id ?? null
}

export const store = new Store()

/** Igualdade rasa: mesmos itens (listas) ou mesmos campos (objetos). */
export function shallow<T>(a: T, b: T): boolean {
  if (Object.is(a, b)) return true
  if (typeof a !== 'object' || typeof b !== 'object' || a == null || b == null) return false
  if (Array.isArray(a) !== Array.isArray(b)) return false
  const ka = Object.keys(a)
  if (ka.length !== Object.keys(b).length) return false
  return ka.every((k) => Object.is((a as Record<string, unknown>)[k], (b as Record<string, unknown>)[k]))
}

/**
 * Lê o estado e redesenha o componente só quando o valor escolhido muda
 * (por `Object.is`, ou pela igualdade dada; `shallow` para listas e objetos
 * montados no seletor).
 */
export function useStore<T>(select: (s: Store) => T, equal: (a: T, b: T) => boolean = Object.is): T {
  const last = useRef<{ version: number; select: (s: Store) => T; value: T } | null>(null)
  const get = () => {
    const prev = last.current
    if (prev && prev.version === store.version && prev.select === select) return prev.value
    const value = select(store)
    if (prev && equal(prev.value, value)) {
      last.current = { version: store.version, select, value: prev.value }
      return prev.value
    }
    last.current = { version: store.version, select, value }
    return value
  }
  return useSyncExternalStore(store.subscribe, get, get)
}
