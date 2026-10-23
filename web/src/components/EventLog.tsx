import { useEffect, useMemo, useRef, useState } from 'react'
import { t, tx } from '../lib/i18n'
import { utc } from '../lib/format'
import { shallow, store, useStore } from '../lib/store'
import type { EventMsg } from '../types'

// Chave estável de cada evento (a lista é cortada no começo quando cresce).
const keys = new WeakMap<EventMsg, number>()
let nextKey = 1
function keyOf(e: EventMsg): number {
  let k = keys.get(e)
  if (k == null) {
    k = nextKey++
    keys.set(e, k)
  }
  return k
}

export function EventLog() {
  const open = useStore((s) => s.ui.eventsOpen)
  const events = useStore((s) => s.events)
  // Só os nomes: o registro não precisa redesenhar a cada instantâneo.
  const names = useStore((s) => Object.fromEntries((s.snap?.vehicles ?? []).map((v) => [v.id, v.name])) as Record<number, string>, shallow)
  const selected = useStore((s) => s.ui.selected)
  const [onlySel, setOnlySel] = useState(false)
  const [level, setLevel] = useState<'all' | 'warn'>('all')
  const list = useRef<HTMLDivElement>(null)
  const shown = useMemo(
    () => events.filter((e) => (!onlySel || e.vehicle === selected) && (level === 'all' || e.level === 'warn' || e.level === 'alarm')).slice(-400),
    [events, onlySel, selected, level],
  )
  useEffect(() => {
    const el = list.current
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight < 80) el.scrollTop = el.scrollHeight
  }, [shown])
  const name = (id: number | null) => (id == null ? '' : (names[id] ?? `#${id}`))
  return (
    <section className="events" style={{ maxHeight: open ? 210 : 26 }}>
      <header>
        <button className="small" onClick={() => store.setUi({ eventsOpen: !open })} aria-expanded={open} aria-label={open ? t('Esconder os eventos') : t('Mostrar os eventos')} title="e">
          {open ? '▾' : '▸'}
        </button>
        {t('Eventos')} ({events.length})
        {open && (
          <>
            <label>
              <input type="checkbox" checked={onlySel} onChange={(e) => setOnlySel(e.target.checked)} /> {t('só o veículo selecionado')}
            </label>
            <label>
              <input type="checkbox" checked={level === 'warn'} onChange={(e) => setLevel(e.target.checked ? 'warn' : 'all')} /> {t('só avisos e alarmes')}
            </label>
          </>
        )}
      </header>
      {open && (
        <div className="list" ref={list} role="log" aria-label={t('Eventos')}>
          {shown.map((e) => (
            <div key={keyOf(e)} className={`ev ${e.level}`}>
              <span className="muted">{utc(e.t)}</span>
              <span className="who">{e.level === 'chat' ? e.from : name(e.vehicle)}</span>
              <span className="txt">{e.level === 'chat' ? `${e.channel !== 'geral' ? `→ ${e.channel}: ` : ''}${e.text}` : tx(e.text)}</span>
            </div>
          ))}
        </div>
      )}
    </section>
  )
}
