// Barra da reprodução de uma gravação: tocar, velocidade, linha do tempo com
// as marcas dos alarmes e avisos, CSV e sair.

import { useEffect, useRef, useState } from 'react'
import { t, tx } from '../lib/i18n'
import { utc } from '../lib/format'
import { store, useStore } from '../lib/store'

const SPEEDS = [1, 2, 5, 10, 30, 60, 300]

export function ReplayBar() {
  const rp = useStore((s) => s.snap?.replay ?? null)
  const meta = useStore((s) => s.hello?.replay)
  const selected = useStore((s) => s.ui.selected)
  // Posição arrastada (antes de soltar), para não pular a cada pixel.
  const [drag, setDrag] = useState<number | null>(null)
  const pending = useRef<number | null>(null)
  useEffect(() => {
    const h = setInterval(() => {
      if (pending.current != null) {
        store.sim('replay_control', { seek: pending.current })
        pending.current = null
      }
    }, 150)
    return () => clearInterval(h)
  }, [])
  if (!rp || !meta) return null
  const last = Math.max(1, rp.frames - 1)
  const index = drag ?? rp.index
  const seek = (i: number) => {
    pending.current = Math.max(0, Math.min(last, Math.round(i)))
  }
  // Hora aproximada de um quadro, pelos instantes amostrados.
  const timeOf = (i: number) => {
    const ts = meta.times
    let best = ts[0]
    for (const p of ts) if (p[0] <= i) best = p
    return best?.[1] ?? rp.t0
  }
  return (
    <div className="replay-bar" role="region" aria-label={t('Reprodução')}>
      <span className="pill replay-tag">{t('REPRODUÇÃO')}</span>
      <b className="small" title={rp.name}>
        {rp.name}
      </b>
      <button className="small" onClick={() => store.sim('replay_control', { playing: !rp.playing })} aria-label={rp.playing ? t('Pausar') : t('Tocar')} title={`${rp.playing ? t('Pausar') : t('Tocar')} (${t('espaço')})`}>
        {rp.playing ? '❚❚' : '▶'}
      </button>
      <select className="small" value={rp.speed} onChange={(e) => store.sim('replay_control', { speed: Number(e.target.value) })} aria-label={t('Velocidade')}>
        {SPEEDS.map((s) => (
          <option key={s} value={s}>
            {s}×
          </option>
        ))}
      </select>
      <div className="timeline">
        <div className="marks" aria-hidden>
          {meta.markers.map(([i, level, text], k) => (
            <button key={k} className={`mark ${level}`} style={{ left: `${(i / last) * 100}%` }} title={`${utc(timeOf(i))} · ${tx(text)}`} onClick={() => seek(i)} tabIndex={-1} />
          ))}
        </div>
        <input
          type="range"
          min={0}
          max={last}
          value={index}
          aria-label={t('Posição na gravação')}
          aria-valuetext={utc(timeOf(index))}
          onChange={(e) => {
            setDrag(Number(e.target.value))
            seek(Number(e.target.value))
          }}
          onPointerUp={() => setDrag(null)}
          onKeyUp={() => setDrag(null)}
          onBlur={() => setDrag(null)}
        />
      </div>
      <span className="mono small">
        {utc(drag != null ? timeOf(drag) : rp.t)} · {index + 1}/{rp.frames}
      </span>
      {selected != null && (
        <a className="button small" href={store.csvUrl(selected)} download title={t('Telemetria do veículo selecionado, a gravação inteira')}>
          CSV
        </a>
      )}
      <button className="small" onClick={() => store.sim('replay_stop')}>
        {t('Sair da reprodução')}
      </button>
    </div>
  )
}
