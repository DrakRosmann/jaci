// Conexão com o servidor e o estado da interface, num só lugar. Os componentes
// leem com useStore(); a cada instantâneo a versão muda e eles redesenham.

import { useSyncExternalStore } from 'react'
import type { EventMsg, Hello, Snapshot, TmDef, Vehicle } from '../types'
import { History } from './history'

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

export interface UiState {
  view: View
  selected: number | null
  lang: 'pt' | 'en'
  eventsOpen: boolean
}

const VIEWS: View[] = ['overview', 'launch', 'trajectory', 'orbit', 'rendezvous', 'systems', 'commands', 'telemetry', 'stations', 'interplanetary', 'procedures', 'instructor', 'ksp']

function loadUi(): UiState {
  const base: UiState = { view: 'overview', selected: null, lang: 'pt', eventsOpen: true }
  let ui = base
  try {
    const saved = JSON.parse(localStorage.getItem('jaci.ui') ?? '{}')
    ui = { ...base, ...saved, selected: null }
  } catch {
    /* sem armazenamento */
  }
  // Atalho pelo endereço: /#launch abre o console de lançamento; /#en em inglês.
  const hash = typeof location === 'undefined' ? '' : location.hash
  for (const part of hash.slice(1).split('&')) {
    if ((VIEWS as string[]).includes(part)) ui.view = part as View
    if (part === 'en' || part === 'pt') ui.lang = part
  }
  return ui
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
    this.ui = { ...this.ui, ...patch }
    if (patch.view) history.replaceState(null, '', `#${patch.view}`)
    try {
      const { view, lang, eventsOpen } = this.ui
      localStorage.setItem('jaci.ui', JSON.stringify({ view, lang, eventsOpen }))
    } catch {
      /* sem armazenamento */
    }
    this.emit()
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
        break
      case 'dictionary':
        this.dictionary = d.dictionary
        break
      case 'history':
        // Telemetria de antes de conectar, para os gráficos.
        for (const [vid, h] of Object.entries(d.vehicles as Record<string, { t: number[]; points: Record<string, (number | null)[]> }>)) {
          h.t.forEach((t, i) => {
            const tm: Record<string, number | null> = {}
            for (const [k, col] of Object.entries(h.points)) tm[k] = col[i]
            this.history.push(Number(vid), t, tm)
          })
        }
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

/** O veículo mais interessante: o da plataforma, senão o primeiro que não é detrito. */
export function defaultVehicle(vs: Vehicle[]): number | null {
  const pad = vs.find((v) => v.phase === 'prelaunch')
  const live = vs.find((v) => v.kind !== 'debris' && v.kind !== 'station' && !['destroyed'].includes(v.phase))
  return (pad ?? live ?? vs.find((v) => v.kind !== 'debris') ?? vs[0])?.id ?? null
}

export const store = new Store()

export function useStore<T>(select: (s: Store) => T): T {
  useSyncExternalStore(store.subscribe, () => store.version)
  return select(store)
}
