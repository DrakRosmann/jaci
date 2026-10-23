// "Meu console": blocos dos outros consoles numa grade de 12 colunas. Em
// edição, arrasta pelo título e redimensiona pelo canto; o layout fica
// guardado por posição da sala, e dá para exportar e importar.

import { useEffect, useRef, useState } from 'react'
import { WidgetBody } from '../components/Widgets'
import { toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { COLS, ROW, WIDGETS, defaultLayout, loadLayout, newId, saveLayout, settle, type Widget, type WidgetType } from '../lib/layout'
import { useStore } from '../lib/store'

const GAP = 8

export function Custom() {
  const position = useStore((s) => (s.snap?.room?.on ? (s.me?.position ?? 'OBS') : (s.me?.position ?? null)))
  const [list, setList] = useState<Widget[]>(() => loadLayout(position))
  const [editing, setEditing] = useState(false)
  const [width, setWidth] = useState(1000)
  const [drag, setDrag] = useState<{ i: string; mode: 'move' | 'size'; x0: number; y0: number; orig: Widget } | null>(null)
  const [preview, setPreview] = useState<Widget[] | null>(null)
  const [config, setConfig] = useState<string | null>(null)
  const box = useRef<HTMLDivElement>(null)
  // Outra posição: o layout dela.
  const [loadedFor, setLoadedFor] = useState(position)
  if (loadedFor !== position) {
    setLoadedFor(position)
    setList(loadLayout(position))
  }
  useEffect(() => {
    const el = box.current
    if (!el) return
    const ro = new ResizeObserver(() => setWidth(el.clientWidth))
    ro.observe(el)
    setWidth(el.clientWidth)
    return () => ro.disconnect()
  }, [])
  const colW = (width - GAP * (COLS - 1)) / COLS
  const commit = (next: Widget[], moved?: string) => {
    const s = settle(next, moved)
    setList(s)
    saveLayout(position, s)
  }
  // Arrastar e redimensionar com o ponteiro, na grade.
  useEffect(() => {
    if (!drag) return
    const at = (e: PointerEvent) => {
      const dx = Math.round((e.clientX - drag.x0) / (colW + GAP))
      const dy = Math.round((e.clientY - drag.y0) / (ROW + GAP))
      const o = drag.orig
      const w = drag.mode === 'move' ? { ...o, x: o.x + dx, y: o.y + dy } : { ...o, w: o.w + dx, h: o.h + dy }
      return settle(
        list.map((x) => (x.i === o.i ? w : x)),
        o.i,
      )
    }
    const move = (e: PointerEvent) => setPreview(at(e))
    const up = (e: PointerEvent) => {
      commit(at(e), drag.orig.i)
      setPreview(null)
      setDrag(null)
    }
    window.addEventListener('pointermove', move)
    window.addEventListener('pointerup', up)
    return () => {
      window.removeEventListener('pointermove', move)
      window.removeEventListener('pointerup', up)
    }
  })
  const shown = preview ?? list
  const rows = Math.max(...shown.map((w) => w.y + w.h), 4)
  const add = (type: WidgetType) => {
    const d = WIDGETS.find((x) => x.type === type)!
    commit([...list, { i: newId(), type, x: 0, y: rows, w: d.w, h: d.h, points: d.points }])
  }
  const exportLayout = () => {
    const blob = new Blob([JSON.stringify(list, null, 2)], { type: 'application/json' })
    const a = document.createElement('a')
    a.href = URL.createObjectURL(blob)
    a.download = `jaci-console-${position ?? 'geral'}.json`
    a.click()
    setTimeout(() => URL.revokeObjectURL(a.href), 1000)
  }
  const importLayout = (file: File) =>
    file.text().then((text) => {
      try {
        const l = JSON.parse(text) as Widget[]
        if (!Array.isArray(l) || !l.every((w) => WIDGETS.some((d) => d.type === w.type))) throw new Error('formato')
        commit(l.map((w) => ({ ...w, i: newId() })))
        toast(t('Layout importado'), 'good')
      } catch {
        toast(t('Arquivo de layout inválido'), 'bad')
      }
    })
  return (
    <div className="custom">
      <div className="row custom-bar">
        <b>{t('Meu console')}</b>
        <span className="small muted">{position ? `${t('posição')} ${position}` : t('sem posição')}</span>
        <span className="spacer" style={{ flex: 1 }} />
        {editing && (
          <>
            <select value="" onChange={(e) => e.target.value && add(e.target.value as WidgetType)} aria-label={t('Acrescentar bloco')}>
              <option value="">+ {t('Acrescentar bloco')}</option>
              {WIDGETS.map((w) => (
                <option key={w.type} value={w.type}>
                  {t(w.name)}
                </option>
              ))}
            </select>
            <button className="small" onClick={() => commit(defaultLayout(position))}>
              {t('Modelo da posição')}
            </button>
            <button className="small" onClick={exportLayout}>
              {t('Exportar')}
            </button>
            <label className="button small">
              {t('Importar')}
              <input type="file" accept="application/json" hidden onChange={(e) => e.target.files?.[0] && importLayout(e.target.files[0])} />
            </label>
          </>
        )}
        <button className={`small ${editing ? 'active' : ''}`} onClick={() => setEditing(!editing)}>
          {editing ? t('Pronto') : t('Editar')}
        </button>
      </div>
      <div ref={box} className={`grid-board ${editing ? 'editing' : ''}`} style={{ height: rows * (ROW + GAP) }}>
        {shown.map((w) => {
          const def = WIDGETS.find((d) => d.type === w.type)!
          const style = { left: w.x * (colW + GAP), top: w.y * (ROW + GAP), width: w.w * colW + (w.w - 1) * GAP, height: w.h * ROW + (w.h - 1) * GAP }
          const bodyH = style.height - 30
          return (
            <section key={w.i} className={`panel board-item ${drag?.i === w.i ? 'dragging' : ''}`} style={style}>
              <h3
                onPointerDown={(e) => {
                  if (!editing || (e.target as HTMLElement).closest('button')) return
                  e.preventDefault()
                  setDrag({ i: w.i, mode: 'move', x0: e.clientX, y0: e.clientY, orig: w })
                }}
                style={{ cursor: editing ? 'move' : undefined }}
              >
                {t(def.name)}
                {editing && (
                  <span className="right row">
                    {(w.type === 'chart' || w.type === 'tiles') && (
                      <button className="small" onClick={() => setConfig(config === w.i ? null : w.i)} aria-label={t('Pontos')} title={t('Pontos')}>
                        ⚙
                      </button>
                    )}
                    <button className="small" onClick={() => commit(list.filter((x) => x.i !== w.i))} aria-label={t('Remover')} title={t('Remover')}>
                      ✕
                    </button>
                  </span>
                )}
              </h3>
              <div className="content board-body" style={{ height: bodyH }}>
                {config === w.i ? <PointsEditor points={w.points ?? []} onChange={(p) => commit(list.map((x) => (x.i === w.i ? { ...x, points: p } : x)))} /> : <WidgetBody type={w.type} points={w.points} height={bodyH - 20} />}
              </div>
              {editing && (
                <span
                  className="resize"
                  onPointerDown={(e) => {
                    e.preventDefault()
                    setDrag({ i: w.i, mode: 'size', x0: e.clientX, y0: e.clientY, orig: w })
                  }}
                  aria-hidden
                />
              )}
            </section>
          )
        })}
      </div>
    </div>
  )
}

/** Escolhe os pontos de telemetria de um bloco (do dicionário do veículo selecionado). */
function PointsEditor({ points, onChange }: { points: string[]; onChange: (p: string[]) => void }) {
  const v = useStore((s) => s.selected)
  const dict = useStore((s) => (v ? s.dictionary[String(v.id)] : undefined))
  const [filter, setFilter] = useState('')
  return (
    <div className="points-editor">
      <input placeholder={t('procurar…')} value={filter} onChange={(e) => setFilter(e.target.value)} />
      <div className="list">
        {(dict ?? [])
          .filter((d) => !filter || `${d.name} ${d.id}`.toLowerCase().includes(filter.toLowerCase()))
          .map((d) => (
            <label key={d.id}>
              <input type="checkbox" checked={points.includes(d.id)} onChange={(e) => onChange(e.target.checked ? [...points, d.id].slice(-8) : points.filter((x) => x !== d.id))} /> {tx(d.name)} <span className="muted mono small">{d.id}</span>
            </label>
          ))}
      </div>
    </div>
  )
}
