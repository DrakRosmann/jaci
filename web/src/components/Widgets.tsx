// Os blocos do "Meu console" e do telão: pedaços dos consoles que cabem em
// qualquer tamanho.

import { Suspense, lazy, useMemo, useState } from 'react'
import { Chart, COLORS } from './Chart'
import { MapView, kindColor, type Track } from './MapView'
import { Empty, Tm, findDef } from './ui'
import { t, tx } from '../lib/i18n'
import { distance, met, num, utc } from '../lib/format'
import { usePrediction } from '../lib/hooks'
import { shallow, store, useStore } from '../lib/store'
import type { WidgetType } from '../lib/layout'

const Orbit3D = lazy(() => import('./Orbit3D').then((m) => ({ default: m.Orbit3D })))
const Timeline = lazy(() => import('../views/Timeline').then((m) => ({ default: m.Timeline })))

export function WidgetBody({ type, points, height }: { type: WidgetType; points?: string[]; height: number }) {
  switch (type) {
    case 'map':
      return <MapWidget height={height} />
    case 'orbit3d':
      return <OrbitWidget height={height} />
    case 'chart':
      return <ChartWidget points={points ?? []} height={height} />
    case 'tiles':
      return <TilesWidget points={points ?? []} />
    case 'alarms':
      return <AlarmsWidget />
    case 'events':
      return <EventsWidget />
    case 'clocks':
      return <ClocksWidget />
    case 'fleet':
      return <FleetWidget />
    case 'countdown':
      return <CountdownWidget />
    case 'timeline':
      return (
        <Suspense fallback={null}>
          <Timeline />
        </Suspense>
      )
    case 'loops':
      return <LoopsWidget />
  }
}

/** Quanto prever para o traço: uma volta e meia, ou até o impacto. */
function trackSpan(period: number | undefined, pe: number | undefined): number {
  const p = (period ?? 0) * 60
  return p > 0 && isFinite(p) && (pe ?? -1) > 0 ? Math.min(p * 1.5, 86_400 * 2) : 3000
}

export function MapWidget({ height }: { height: number }) {
  const v = useStore((s) => s.selected)
  // O mapa é o do corpo em volta do qual a nave está (no Sol não há mapa).
  const body = v && v.central !== 'sun' ? v.central : 'earth'
  const pred = usePrediction(v && v.central === body && !['prelaunch', 'landed'].includes(v.phase) ? v.id : null, trackSpan(v?.tm?.['orb.period'] ?? undefined, v?.pe ?? undefined), 5000)
  const vid = v?.id
  const tracks = useMemo<Track[]>(() => (vid != null && pred.points.length ? [{ vehicle: vid, points: pred.points.map((p) => [p[2], p[1]] as [number, number]), color: '#4cc3ff' }] : []), [pred, vid])
  return <MapView tracks={tracks} height={height} body={body} />
}

function OrbitWidget({ height }: { height: number }) {
  const v = useStore((s) => s.selected)
  const period = (v?.tm?.['orb.period'] ?? NaN) * 60
  const span = isFinite(period) && period > 0 && (v?.ecc ?? 1) < 1 ? Math.min(period * 1.05, 3e8) : 86_400
  const pred = usePrediction(v && v.phase !== 'prelaunch' && v.kind !== 'external' ? v.id : null, span, 3000, span / 360)
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  return (
    <Suspense fallback={null}>
      <Orbit3D vehicle={v.id} prediction={pred.points} height={height} />
    </Suspense>
  )
}

function ChartWidget({ points, height }: { points: string[]; height: number }) {
  const v = useStore((s) => s.selected)
  const now = useStore((s) => s.snap?.t ?? 0)
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  return (
    <Chart
      height={height}
      series={points.map((id, i) => {
        const [ts, vs] = store.history.series(v.id, id, now - 3_600)
        const d = findDef(v.id, id)
        return { label: `${tx(d?.name ?? id)}${d?.unit ? ` (${d.unit})` : ''}`, color: COLORS[i % COLORS.length], t: ts, v: vs }
      })}
    />
  )
}

function TilesWidget({ points }: { points: string[] }) {
  const v = useStore((s) => s.selected)
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  return (
    <div className="tiles">
      {points.map((id) => (
        <Tm key={id} v={v} id={id} digits={2} />
      ))}
    </div>
  )
}

export function AlarmsWidget({ big }: { big?: boolean }) {
  const alarms = useStore((s) => (s.snap?.alarms ?? []).filter((a) => a.active), shallow)
  const names = useStore((s) => Object.fromEntries((s.snap?.vehicles ?? []).map((v) => [v.id, v.name])) as Record<number, string>, shallow)
  if (alarms.length === 0) return <div className={`muted ${big ? 'wall-ok' : 'small'}`}>{t('Nenhum alarme ativo.')}</div>
  return (
    <div className={`alarm-list ${big ? 'big' : ''}`}>
      {alarms
        .slice()
        .sort((a, b) => (a.level === b.level ? b.t - a.t : a.level === 'red' ? -1 : 1))
        .map((a) => (
          <div key={a.id} className={`alarm-row ${a.level} ${a.acked ? 'acked' : ''}`}>
            <span className="mono">{utc(a.t, false)}</span>
            <b>{names[a.vehicle]}</b>
            <span>
              {tx(a.name)} {num(a.value, 2)} {a.unit}
            </span>
            {!a.acked && !big && (
              <button className="small" onClick={() => store.sim('ack', { value: a.id })}>
                {t('Reconhecer')}
              </button>
            )}
          </div>
        ))}
    </div>
  )
}

export function EventsWidget({ count = 60 }: { count?: number }) {
  const events = useStore((s) => s.events)
  const names = useStore((s) => Object.fromEntries((s.snap?.vehicles ?? []).map((v) => [v.id, v.name])) as Record<number, string>, shallow)
  const list = events.filter((e) => e.level !== 'chat').slice(-count).reverse()
  return (
    <div className="mini-events">
      {list.map((e, i) => (
        <div key={i} className={`ev ${e.level}`}>
          <span className="muted">{utc(e.t, false)}</span>
          <span className="who">{e.vehicle != null ? names[e.vehicle] : ''}</span>
          <span className="txt">{tx(e.text)}</span>
        </div>
      ))}
    </div>
  )
}

export function ClocksWidget() {
  const snap = useStore((s) => s.snap)
  const v = useStore((s) => s.selected)
  const lc = snap?.launch
  const counting = lc && lc.vehicle != null && lc.state !== 'launched' && lc.state !== 'idle'
  return (
    <div className="clocks-widget">
      <div>
        <span>UTC</span>
        <b>{snap?.utc.replace(' UTC', '').slice(11) ?? '—'}</b>
      </div>
      <div>
        <span>{t('Tempo de missão')}</span>
        <b>{met(v?.met ?? null)}</b>
      </div>
      {counting && (
        <div className={lc.state === 'hold' ? 'hold' : 'count'}>
          <span>{lc.state === 'hold' ? t('Em espera') : t('Contagem')}</span>
          <b>{lc.clock}</b>
        </div>
      )}
    </div>
  )
}

export function FleetWidget({ big }: { big?: boolean }) {
  const snap = useStore((s) => s.snap)
  const fleet = (snap?.vehicles ?? []).filter((x) => x.kind !== 'debris')
  return (
    <table className={big ? 'big' : ''}>
      <tbody>
        {fleet.map((x) => {
          const al = snap!.alarms.filter((a) => a.vehicle === x.id && a.active).length
          return (
            <tr key={x.id} onClick={() => store.setUi({ selected: x.id })} style={{ cursor: 'pointer' }}>
              <td>
                <span className="dot" style={{ background: kindColor(x.kind) }} /> {x.name}
              </td>
              <td>{tx(x.phase_name)}</td>
              <td className="num">{distance(x.alt)}</td>
              <td className={`num ${al ? 'bad' : 'muted'}`}>{al ? `${al} ⚠` : '✓'}</td>
            </tr>
          )
        })}
      </tbody>
    </table>
  )
}

export function CountdownWidget({ big }: { big?: boolean }) {
  const lc = useStore((s) => s.snap?.launch)
  const name = useStore((s) => (s.snap?.launch.vehicle != null ? s.vehicle(s.snap.launch.vehicle)?.name : undefined))
  if (!lc || lc.vehicle == null) return <Empty>{t('Nenhum veículo na plataforma neste cenário.')}</Empty>
  return (
    <div className={`countdown-widget ${big ? 'big' : ''}`}>
      <div className="muted small">{name}</div>
      <div className={`big-clock ${lc.state === 'hold' ? 'hold' : ''}`}>{lc.state === 'launched' ? '—' : lc.clock}</div>
      <span className={`pill ${lc.all_go ? 'go' : 'nogo'}`}>{lc.all_go ? 'GO' : 'NO-GO'}</span>
      {lc.state === 'hold' && lc.hold_reason && <div className="warn small">{tx(lc.hold_reason)}</div>}
    </div>
  )
}

function LoopsWidget() {
  const events = useStore((s) => s.events)
  const msgs = events.filter((e) => e.level === 'chat').slice(-50)
  const [text, setText] = useState('')
  const send = async () => {
    if (text.trim() && (await store.chat(text.trim(), 'geral')).ok) setText('')
  }
  return (
    <div className="loops-widget">
      <div className="msgs">
        {msgs.map((e, i) => (
          <div key={i}>
            <span className="muted mono small">{utc(e.t, false)}</span> <b>{e.from}</b>
            {e.channel !== 'geral' && <span className="muted small"> → {e.channel}</span>}: {e.text}
          </div>
        ))}
      </div>
      <input value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && send()} placeholder={t('mensagem no loop geral…')} />
    </div>
  )
}
