import { useEffect, useState } from 'react'
import { MapView } from '../components/MapView'
import { Empty, Panel, Tile, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { distance, duration, num, rate, utc } from '../lib/format'
import { store, useStore } from '../lib/store'
import type { Pass, Station, Vehicle } from '../types'
import { RealSatellites } from './RealSatellites'

const A = 6378137
const F = 1 / 298.257223563

/** Posição da estação no inercial (m), girando com o tempo sideral. */
function stationEci(st: Station, gmst: number): [number, number, number] {
  const e2 = F * (2 - F)
  const la = (st.lat * Math.PI) / 180
  const lo = (st.lon * Math.PI) / 180 + gmst
  const n = A / Math.sqrt(1 - e2 * Math.sin(la) ** 2)
  return [(n + st.alt) * Math.cos(la) * Math.cos(lo), (n + st.alt) * Math.cos(la) * Math.sin(lo), (n * (1 - e2) + st.alt) * Math.sin(la)]
}

export function lookAngles(st: Station, r: [number, number, number], gmst: number) {
  const s = stationEci(st, gmst)
  const d = [r[0] - s[0], r[1] - s[1], r[2] - s[2]]
  const sn = Math.hypot(...s)
  const up = s.map((x) => x / sn)
  const east = [-up[1], up[0], 0]
  const en = Math.hypot(east[0], east[1]) || 1
  east[0] /= en
  east[1] /= en
  const north = [up[1] * east[2] - up[2] * east[1], up[2] * east[0] - up[0] * east[2], up[0] * east[1] - up[1] * east[0]]
  const range = Math.hypot(d[0], d[1], d[2])
  const el = (Math.asin((d[0] * up[0] + d[1] * up[1] + d[2] * up[2]) / range) * 180) / Math.PI
  const az = ((Math.atan2(d[0] * east[0] + d[1] * east[1], d[0] * north[0] + d[1] * north[1] + d[2] * north[2]) * 180) / Math.PI + 360) % 360
  return { az, el, range }
}

export function Stations() {
  const v = useStore((s) => s.selected)
  const hello = useStore((s) => s.hello)
  const snap = useStore((s) => s.snap)!
  const [passes, setPasses] = useState<Pass[] | null>(null)
  useEffect(() => {
    setPasses(null)
    if (!v || v.central !== 'earth' || v.phase === 'prelaunch' || v.kind === 'external') return
    store
      .request<{ passes: Pass[] }>({ type: 'passes', vehicle: v.id, span: 86_400 })
      .then((d) => setPasses(d.passes))
      .catch((e) => toast(String(e), 'bad'))
  }, [v?.id, v?.phase === 'prelaunch'])
  if (!hello) return null
  const link = v?.systems?.comms.link
  return (
    <div className="grid">
      <div className="grid cols-3">
        <Panel title={t('Mapa das estações')} className="span-2" flush>
          <MapView height={380} />
        </Panel>
        <Panel title={t('Enlace atual')}>
          {v?.kind === 'tracked' ? (
            <Visibility v={v} stations={hello.stations} snap={snap} />
          ) : !v?.systems ? (
            <span className="muted">{t('Sem rádio.')}</span>
          ) : link ? (
            <div className="tiles">
              <Tile label={t('Estação')} value={tx(link.station)} state="nominal" />
              <Tile label={t('Elevação')} value={num(link.elevation_deg, 1)} unit="°" />
              <Tile label={t('Azimute')} value={num(link.azimuth_deg, 1)} unit="°" />
              <Tile label={t('Distância')} value={distance(link.range_m / 1000)} />
              <Tile label={t('Margem')} value={num(link.margin_db, 1)} unit="dB" />
              <Tile label={t('Taxa')} value={rate(link.rate_bps)} />
              <Tile label={t('Atraso da luz')} value={duration(link.owlt)} />
            </div>
          ) : (
            <span className="warn">{t('Sem enlace agora.')}</span>
          )}
        </Panel>
      </div>
      <div className="grid cols-2">
        <Panel title={t('Estações')} flush>
          <table>
            <thead>
              <tr>
                <th>{t('Estação')}</th>
                <th className="num">{t('Antena')}</th>
                <th className="num">{t('Elevação')}</th>
                <th className="num">{t('Azimute')}</th>
                <th className="num">{t('Distância')}</th>
                <th>{t('Ativa')}</th>
              </tr>
            </thead>
            <tbody>
              {hello.stations.map((st) => {
                const live = snap.stations_live.find((x) => x.id === st.id)
                const la = v ? look(st, v, snap) : null
                const visible = la && la.el >= st.min_el
                return (
                  <tr key={st.id}>
                    <td>
                      {tx(st.name)}
                      <div className="small muted">
                        {num(st.lat, 2)}°, {num(st.lon, 2)}° {st.deep_space && `· ${t('espaço profundo')}`}
                      </div>
                    </td>
                    <td className="num">{num(st.dish_m, 1)} m</td>
                    <td className={`num ${visible ? 'good' : 'muted'}`}>{la ? num(la.el, 1) : '—'}°</td>
                    <td className="num">{la ? num(la.az, 0) : '—'}°</td>
                    <td className="num">{la ? distance(la.range / 1000) : '—'}</td>
                    <td>
                      <input type="checkbox" checked={live?.enabled ?? true} onChange={(e) => store.sim('station', { id: st.id, enabled: e.target.checked })} />
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </Panel>
        <Panel title={`${t('Passagens nas próximas 24 h')}${v ? ` · ${v.name}` : ''}`} flush>
          {!v || v.central !== 'earth' ? (
            <Empty>{t('Passagens só para veículos em órbita da Terra.')}</Empty>
          ) : passes == null ? (
            <Empty>{t('Calculando…')}</Empty>
          ) : passes.length === 0 ? (
            <Empty>{t('Nenhuma passagem.')}</Empty>
          ) : (
            <div style={{ maxHeight: 520, overflowY: 'auto' }}>
              <table>
                <thead>
                  <tr>
                    <th>{t('Estação')}</th>
                    <th>{t('Aquisição (AOS)')}</th>
                    <th className="num">{t('Duração')}</th>
                    <th className="num">{t('Elevação máx.')}</th>
                    <th className="num">{t('Em')}</th>
                  </tr>
                </thead>
                <tbody>
                  {passes.map((p, i) => (
                    <tr key={i} style={{ opacity: p.los < snap.t ? 0.4 : 1 }}>
                      <td>{tx(p.station)}</td>
                      <td className="mono small">{utc(p.aos)}</td>
                      <td className="num">{duration(p.los - p.aos)}</td>
                      <td className="num">{num(p.max_el, 0)}°</td>
                      <td className="num">{p.aos > snap.t ? duration(p.aos - snap.t) : p.los > snap.t ? <span className="good">{t('agora')}</span> : '—'}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Panel>
      </div>
      <RealSatellites />
    </div>
  )
}

/** Satélite real (sem rádio simulado): de quais estações ele está à vista agora. */
function Visibility({ v, stations, snap }: { v: Vehicle; stations: Station[]; snap: { gmst: number; bodies: { id: string; r: [number, number, number] }[] } }) {
  const seen = stations
    .filter((st) => !st.deep_space)
    .map((st) => ({ st, la: look(st, v, snap) }))
    .filter((x) => x.la && x.la.el >= x.st.min_el)
    .sort((a, b) => b.la!.el - a.la!.el)
  if (seen.length === 0) return <span className="muted">{t('Fora da vista de todas as estações agora.')}</span>
  return (
    <div className="tiles">
      {seen.map(({ st, la }) => (
        <Tile key={st.id} label={tx(st.name)} value={`${num(la!.el, 1)}°`} unit={`az ${num(la!.az, 0)}°`} state="nominal" title={distance(la!.range / 1000)} />
      ))}
    </div>
  )
}

function look(st: Station, v: Vehicle, snap: { gmst: number; bodies: { id: string; r: [number, number, number] }[] }) {
  if (!v.r) return null
  let r = v.r
  if (v.central !== 'earth') {
    // Relativo à Terra: soma as posições heliocêntricas.
    const c = snap.bodies.find((b) => b.id === v.central)
    const e = snap.bodies.find((b) => b.id === 'earth')
    if (!c || !e) return null
    r = [c.r[0] - e.r[0] + v.r[0], c.r[1] - e.r[1] + v.r[1], c.r[2] - e.r[2] + v.r[2]]
  }
  return lookAngles(st, r, snap.gmst)
}
