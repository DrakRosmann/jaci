// Linha do tempo da missão (FAO): o que vem pela frente para o veículo
// selecionado, em faixas — contagem, janela, queimas, comandos agendados,
// passagens, eclipses, apsides e, para o instrutor, as falhas planejadas.

import { useEffect, useState, type ReactNode } from 'react'
import { Empty, Panel } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { duration, num, utc } from '../lib/format'
import { store, useStore } from '../lib/store'
import type { Pass } from '../types'

interface TimelineData {
  t: number
  span: number
  eclipses: [number, number][]
  apsides: { t: number; kind: 'pe' | 'ap'; alt: number }[]
  passes: Pass[]
  burns: { t: number; label: string; dv: number; duration: number; executing: boolean }[]
  commands: { t: number; text: string }[]
  countdown: { t: number; name: string; at: number }[]
  window: [number, number] | null
  hold?: boolean
  faults: { t: number; fault: string; announce: boolean }[]
  replay?: boolean
}

const SPANS: [number, string][] = [
  [3_600, '1 h'],
  [6 * 3_600, '6 h'],
  [24 * 3_600, '24 h'],
  [3 * 86_400, '3 d'],
]

export function Timeline() {
  const v = useStore((s) => s.selected)
  const now = useStore((s) => s.snap?.t ?? 0)
  const events = useStore((s) => s.events)
  const [span, setSpan] = useState(6 * 3_600)
  // Quanto do intervalo fica antes do "agora" (para ver o passado recente).
  const [back, setBack] = useState(0.1)
  const [data, setData] = useState<TimelineData | null>(null)
  const vid = v?.id
  useEffect(() => {
    if (vid == null) return
    let alive = true
    const load = () =>
      store
        .request<TimelineData>({ type: 'timeline', vehicle: vid, span })
        .then((d) => alive && setData(d))
        .catch(() => {})
    load()
    const h = setInterval(load, 10_000)
    return () => {
      alive = false
      clearInterval(h)
    }
  }, [vid, span])
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  const t0 = now - span * back
  const t1 = t0 + span
  const x = (tt: number) => `${(((tt - t0) / (t1 - t0)) * 100).toFixed(3)}%`
  const w = (a: number, b: number) => `${((Math.max(0, Math.min(b, t1) - Math.max(a, t0)) / (t1 - t0)) * 100).toFixed(3)}%`
  const inside = (tt: number) => tt >= t0 && tt <= t1
  const d = data
  // Passagens por estação (uma faixa para cada estação com passagem).
  const stations = [...new Set((d?.passes ?? []).filter((p) => p.los >= t0 && p.aos <= t1).map((p) => p.station))]
  const past = events.filter((e) => e.vehicle === v.id && e.t >= t0 && e.t <= now && e.level !== 'chat').slice(-150)
  const next = [
    ...(d?.countdown ?? []).map((c) => ({ t: c.t, text: tx(c.name) })),
    ...(d?.burns ?? []).map((b) => ({ t: b.t, text: `${tx(b.label)} · ${num(b.dv, 1)} m/s` })),
    ...(d?.commands ?? []).map((c) => ({ t: c.t, text: tx(c.text) })),
    ...(d?.passes ?? []).map((p) => ({ t: p.aos, text: `${t('AOS')} ${tx(p.station)} · ${num(p.max_el, 0)}°` })),
    ...(d?.eclipses ?? []).map((e) => ({ t: e[0], text: `${t('Entrada no eclipse')} (${duration(e[1] - e[0])})` })),
    ...(d?.faults ?? []).map((f) => ({ t: f.t, text: `${t('Falha planejada')}: ${f.fault}` })),
  ]
    .filter((e) => e.t > now)
    .sort((a, b) => a.t - b.t)
    .slice(0, 12)
  return (
    <div className="grid">
      <Panel
        title={`${t('Linha do tempo')} · ${v.name}`}
        right={
          <span className="row">
            <button className="small" onClick={() => setBack(Math.min(0.9, back + 0.25))} aria-label={t('Para trás')} title={t('Para trás')}>
              ◀
            </button>
            <button className="small" onClick={() => setBack(0.1)} title={t('Voltar ao agora')}>
              {t('agora')}
            </button>
            <button className="small" onClick={() => setBack(Math.max(-2, back - 0.25))} aria-label={t('Para a frente')} title={t('Para a frente')}>
              ▶
            </button>
            {SPANS.map(([s, label]) => (
              <button key={s} className={`small ${span === s ? 'active' : ''}`} onClick={() => setSpan(s)}>
                {label}
              </button>
            ))}
          </span>
        }
      >
        {d?.replay && <p className="small muted">{t('Na reprodução, a linha do tempo mostra só os eventos gravados.')}</p>}
        <div className="timeline-chart">
          <Axis t0={t0} t1={t1} now={now} />
          <Lane label={t('Eventos')}>
            {past.map((e, i) => (
              <span key={i} className={`tl-tick ${e.level}`} style={{ left: x(e.t) }} title={`${utc(e.t)} · ${tx(e.text)}`} />
            ))}
          </Lane>
          {((d?.countdown.length ?? 0) > 0 || d?.window) && (
            <Lane label={t('Contagem')}>
              {d?.window && <span className="tl-bar window" style={{ left: x(Math.max(d.window[0], t0)), width: w(d.window[0], d.window[1]) }} title={`${t('Janela')}: ${utc(d.window[0])} → ${utc(d.window[1])}`} />}
              {d?.countdown.filter((c) => inside(c.t)).map((c, i) => (
                <span key={i} className={`tl-diamond ${d.hold ? 'hold' : ''}`} style={{ left: x(c.t) }} title={`T-${duration(c.at)} · ${tx(c.name)}${d.hold ? ` (${t('em espera: hora se retomar agora')})` : ''}`} />
              ))}
            </Lane>
          )}
          <Lane label={t('Queimas')}>
            {d?.burns.filter((b) => b.t + b.duration >= t0 && b.t <= t1).map((b, i) => (
              <span key={i} className={`tl-bar burn ${b.executing ? 'now' : ''}`} style={{ left: x(Math.max(b.t - b.duration / 2, t0)), width: w(b.t - b.duration / 2, b.t + b.duration / 2), minWidth: 4 }} title={`${utc(b.t)} · ${tx(b.label)} · ${num(b.dv, 1)} m/s · ${duration(b.duration)}`} />
            ))}
          </Lane>
          {(d?.commands.length ?? 0) > 0 && (
            <Lane label={t('Comandos agendados')}>
              {d?.commands.filter((c) => inside(c.t)).map((c, i) => (
                <span key={i} className="tl-tick cmd" style={{ left: x(c.t) }} title={`${utc(c.t)} · ${tx(c.text)}`} />
              ))}
            </Lane>
          )}
          <Lane label={t('Eclipse')}>
            {d?.eclipses.filter((e) => e[1] >= t0 && e[0] <= t1).map((e, i) => (
              <span key={i} className="tl-bar eclipse" style={{ left: x(Math.max(e[0], t0)), width: w(e[0], e[1]) }} title={`${utc(e[0])} → ${utc(e[1], false)} (${duration(e[1] - e[0])})`} />
            ))}
          </Lane>
          <Lane label={t('Apsides')}>
            {d?.apsides.filter((a) => inside(a.t)).map((a, i) => (
              <span key={i} className={`tl-apsis ${a.kind}`} style={{ left: x(a.t) }} title={`${a.kind === 'pe' ? t('Periastro') : t('Apoastro')} ${num(a.alt, 0)} km · ${utc(a.t)}`}>
                {a.kind === 'pe' ? '▼' : '▲'}
              </span>
            ))}
          </Lane>
          {stations.map((st) => (
            <Lane key={st} label={tx(st)} small>
              {d?.passes.filter((p) => p.station === st && p.los >= t0 && p.aos <= t1).map((p, i) => (
                <span key={i} className="tl-bar pass" style={{ left: x(Math.max(p.aos, t0)), width: w(p.aos, p.los), minWidth: 2, opacity: 0.45 + Math.min(p.max_el, 90) / 160 }} title={`${tx(st)} · ${utc(p.aos)} → ${utc(p.los, false)} · ${num(p.max_el, 0)}°`} />
              ))}
            </Lane>
          ))}
          {(d?.faults.length ?? 0) > 0 && (
            <Lane label={t('Falhas planejadas')}>
              {d?.faults.filter((f) => inside(f.t)).map((f, i) => (
                <span key={i} className="tl-tick fault" style={{ left: x(f.t) }} title={`${utc(f.t)} · ${f.fault}${f.announce ? '' : ` (${t('silenciosa')})`}`} />
              ))}
            </Lane>
          )}
          <span className="tl-now" style={{ left: `calc(var(--tl-label) + (100% - var(--tl-label)) * ${(now - t0) / (t1 - t0)})` }} aria-hidden />
        </div>
      </Panel>
      <Panel title={t('Próximos acontecimentos')} flush>
        {next.length === 0 ? (
          <Empty>{t('Nada previsto nesta janela.')}</Empty>
        ) : (
          <table>
            <tbody>
              {next.map((e, i) => (
                <tr key={i}>
                  <td className="mono small">{utc(e.t)}</td>
                  <td className="num">{t('em')} {duration(e.t - now)}</td>
                  <td>{e.text}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Panel>
    </div>
  )
}

function Lane({ label, children, small }: { label: string; children: ReactNode; small?: boolean }) {
  return (
    <div className={`tl-lane ${small ? 'small' : ''}`}>
      <span className="tl-label" title={label}>
        {label}
      </span>
      <div className="tl-track">{children}</div>
    </div>
  )
}

/** Régua de horas no topo. */
function Axis({ t0, t1, now }: { t0: number; t1: number; now: number }) {
  const span = t1 - t0
  const step = [300, 900, 1_800, 3_600, 3 * 3_600, 6 * 3_600, 12 * 3_600].find((s) => span / s <= 10) ?? 86_400
  const ticks: number[] = []
  for (let tt = Math.ceil(t0 / step) * step; tt <= t1; tt += step) ticks.push(tt)
  return (
    <div className="tl-lane axis">
      <span className="tl-label" />
      <div className="tl-track">
        {ticks.map((tt) => (
          <span key={tt} className="tl-hour" style={{ left: `${((tt - t0) / span) * 100}%` }}>
            {utc(tt, false).slice(0, 5)}
            <small>{tt >= now ? ` +${duration(tt - now)}` : ''}</small>
          </span>
        ))}
      </div>
    </div>
  )
}
