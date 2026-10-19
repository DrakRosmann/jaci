import { useEffect, useRef, useState } from 'react'
import { t, tx } from '../lib/i18n'
import { utc } from '../lib/format'
import { store, useStore } from '../lib/store'

export function EventLog() {
  const open = useStore((s) => s.ui.eventsOpen)
  const events = useStore((s) => s.events)
  const vehicles = useStore((s) => s.snap?.vehicles)
  const selected = useStore((s) => s.ui.selected)
  const [onlySel, setOnlySel] = useState(false)
  const [level, setLevel] = useState<'all' | 'warn'>('all')
  const list = useRef<HTMLDivElement>(null)
  const shown = events.filter((e) => (!onlySel || e.vehicle === selected) && (level === 'all' || e.level === 'warn' || e.level === 'alarm')).slice(-400)
  useEffect(() => {
    const el = list.current
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight < 80) el.scrollTop = el.scrollHeight
  }, [shown.length])
  const name = (id: number | null) => (id == null ? '' : (vehicles?.find((v) => v.id === id)?.name ?? `#${id}`))
  return (
    <section className="events" style={{ maxHeight: open ? 210 : 26 }}>
      <header>
        <button className="small" onClick={() => store.setUi({ eventsOpen: !open })}>
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
        <div className="list" ref={list}>
          {shown.map((e, i) => (
            <div key={i} className={`ev ${e.level}`}>
              <span className="muted">{utc(e.t)}</span>
              <span className="who">{name(e.vehicle)}</span>
              <span className="txt">{tx(e.text)}</span>
            </div>
          ))}
        </div>
      )}
    </section>
  )
}
