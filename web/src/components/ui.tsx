// Peças pequenas: painel, mostrador, barra, avisos e botão de comando.

import { useEffect, useState, type ReactNode } from 'react'
import { t, tx } from '../lib/i18n'
import { store, useStore } from '../lib/store'
import { num } from '../lib/format'
import type { TmDef, Vehicle } from '../types'

export function Panel({ title, right, children, className, flush }: { title: ReactNode; right?: ReactNode; children: ReactNode; className?: string; flush?: boolean }) {
  return (
    <section className={`panel ${className ?? ''}`}>
      <h3>
        {title}
        {right && <span className="right">{right}</span>}
      </h3>
      <div className={`content ${flush ? 'flush' : ''}`}>{children}</div>
    </section>
  )
}

export type LimitState = 'nominal' | 'yellow' | 'red' | 'none'

export function limitState(def: TmDef | undefined, v: number | null | undefined): LimitState {
  if (!def || v == null || !isFinite(v)) return 'none'
  if (def.red && (v < def.red[0] || v > def.red[1])) return 'red'
  if (def.yellow && (v < def.yellow[0] || v > def.yellow[1])) return 'yellow'
  return def.red || def.yellow ? 'nominal' : 'none'
}

export function findDef(vehicle: number, id: string): TmDef | undefined {
  return store.dictionary[String(vehicle)]?.find((d) => d.id === id)
}

/** Mostrador de um ponto de telemetria, colorido pelos limites. */
export function Tm({ v, id, label, digits, unit, scale = 1 }: { v: Vehicle; id: string; label?: string; digits?: number; unit?: string; scale?: number }) {
  const def = findDef(v.id, id)
  const raw = v.tm?.[id]
  const val = raw == null ? null : raw * scale
  const st = limitState(def, raw)
  return <Tile label={label ?? tx(def?.name) ?? id} value={num(val, digits)} unit={unit ?? def?.unit} state={st} />
}

export function Tile({ label, value, unit, state, title }: { label: ReactNode; value: ReactNode; unit?: string; state?: LimitState; title?: string }) {
  return (
    <div className={`tile ${state && state !== 'none' ? state : ''}`} title={title}>
      <div className="label">{label}</div>
      <div className="value">
        {value}
        {unit && <span className="unit">{unit}</span>}
      </div>
    </div>
  )
}

export function Bar({ frac, kind }: { frac: number; kind?: 'fuel' }) {
  const f = Math.max(0, Math.min(1, isFinite(frac) ? frac : 0))
  return (
    <div className={`bar ${kind ?? ''}`}>
      <div style={{ width: `${f * 100}%` }} />
    </div>
  )
}

// ── Avisos ──

interface Toast {
  id: number
  text: string
  kind: 'good' | 'bad' | 'info'
}
let toasts: Toast[] = []
let toastId = 1
const toastListeners = new Set<() => void>()

export function toast(text: string, kind: Toast['kind'] = 'info') {
  const id = toastId++
  toasts = [...toasts, { id, text, kind }].slice(-5)
  toastListeners.forEach((l) => l())
  setTimeout(() => {
    toasts = toasts.filter((x) => x.id !== id)
    toastListeners.forEach((l) => l())
  }, kind === 'bad' ? 7000 : 4000)
}

export function Toasts() {
  const [, set] = useState(0)
  useEffect(() => {
    const l = () => set((n) => n + 1)
    toastListeners.add(l)
    return () => {
      toastListeners.delete(l)
    }
  }, [])
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((x) => (
        <div key={x.id} className={`toast ${x.kind}`}>
          {x.text}
        </div>
      ))}
    </div>
  )
}

/** Envia um comando e mostra o resultado. */
export async function sendCommand(vehicle: number, id: string, args: Record<string, unknown> = {}, opts: { armed?: boolean; at?: string; in?: number } = {}) {
  try {
    const r = await store.command(vehicle, id, args, opts)
    if (!r.ok) toast(`${t('Comando recusado')}: ${tx(r.error)}`, 'bad')
    return r.ok
  } catch (e) {
    toast(String(e), 'bad')
    return false
  }
}

/** Botão de comando. Perigosos pedem dois cliques: armar e executar (5 s). */
export function CmdButton({
  vehicle,
  cmd,
  args,
  children,
  hazardous,
  className,
  disabled,
  title,
}: {
  vehicle: number | undefined
  cmd: string
  args?: Record<string, unknown>
  children: ReactNode
  hazardous?: boolean
  className?: string
  disabled?: boolean
  title?: string
}) {
  const def = useStore((s) => s.hello?.commands.find((c) => c.id === cmd))
  // Na reprodução de uma gravação não há o que comandar.
  const replay = useStore((s) => s.snap?.replay != null)
  const danger = hazardous ?? def?.hazardous ?? false
  const [armed, setArmed] = useState(false)
  useEffect(() => {
    if (!armed) return
    const h = setTimeout(() => setArmed(false), 5000)
    return () => clearTimeout(h)
  }, [armed])
  const click = async () => {
    if (vehicle == null) return
    if (danger && !armed) {
      setArmed(true)
      return
    }
    setArmed(false)
    await sendCommand(vehicle, cmd, args, { armed: danger })
  }
  return (
    <button className={`${className ?? ''} ${armed ? 'armed' : ''} ${danger && !armed ? 'danger' : ''}`} disabled={disabled || replay || vehicle == null} onClick={click} title={replay ? t('Reprodução: comandos desligados') : (title ?? tx(def?.description))}>
      {armed ? t('CONFIRMAR') : children}
    </button>
  )
}

export function Empty({ children }: { children: ReactNode }) {
  return <div className="muted" style={{ padding: 20, textAlign: 'center' }}>{children}</div>
}
