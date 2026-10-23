// Gravidade assistida: procura trajetórias com um sobrevoo (origem → planeta
// do meio → destino) no núcleo, compara com o voo direto e mostra a escolhida
// no mapa do Sistema Solar e num diagrama da hipérbole do sobrevoo.

import { useMemo, useState } from 'react'
import { useCanvas } from '../components/Chart'
import { Panel, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { distance, num, utc } from '../lib/format'
import { PLANETS, hohmannWindow } from '../lib/ephem'
import { MU } from '../lib/mfd'
import { store, useStore } from '../lib/store'

export interface Flyby {
  depart: number
  flyby: number
  arrive: number
  c3: number
  vinf_in: number
  vinf_out: number
  turn_deg: number
  turn_max_deg: number
  rp_alt_km: number
  dv_flyby: number
  vinf_arr: number
  gain: number
  cost: number
}

interface FlybySearch {
  from: string
  via: string
  to: string
  results: Flyby[]
  direct: [number, number, number, number] | null
  evaluated: number
}

/** A escolhida, com os corpos, para o mapa. */
export interface FlybyPick extends Flyby {
  from: string
  via: string
  to: string
}

const DAY = 86_400
const day = (t: number) => utc(t).slice(0, 10)
const signed = (x: number) => `${x >= 0 ? '+' : ''}${num(x, 2)}`
const name = (id: string) => tx(store.hello?.bodies.find((b) => b.id === id)?.name ?? id)

export function FlybyPanel({ from, to, onPick, picked }: { from: string; to: string; onPick: (f: FlybyPick | null) => void; picked: FlybyPick | null }) {
  const now = useStore((s) => s.snap?.t ?? 0)
  const [via, setVia] = useState(from === 'earth' && to !== 'venus' ? 'venus' : 'mars')
  const [form, setForm] = useState({ start: day(now), span: '1200', t1: ['', ''], t2: ['', ''] })
  const [busy, setBusy] = useState(false)
  const [data, setData] = useState<FlybySearch | null>(null)
  // Viagens sugeridas: meia e uma vez e meia a Hohmann de cada trecho.
  const suggest = useMemo(() => {
    const leg = (a: string, b: string) => hohmannWindow(a, b, now)?.tof ?? 200 * DAY
    const r = (x: number, k: number) => String(Math.round((x * k) / DAY))
    const a = leg(from, via)
    const b = leg(via, to)
    return { t1: [r(a, 0.4), r(a, 1.5)], t2: [r(b, 0.4), r(b, 1.5)] }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [from, via, to])
  const t1 = [form.t1[0] || suggest.t1[0], form.t1[1] || suggest.t1[1]]
  const t2 = [form.t2[0] || suggest.t2[0], form.t2[1] || suggest.t2[1]]
  const bad = via === from || via === to || from === to
  const search = async () => {
    setBusy(true)
    onPick(null)
    try {
      const r = await store.request<{ ok: boolean; data?: FlybySearch; error?: string }>(
        { type: 'flyby', from, via, to, start: `${form.start} 00:00:00`, span_days: Number(form.span), tof1_min: Number(t1[0]), tof1_max: Number(t1[1]), tof2_min: Number(t2[0]), tof2_max: Number(t2[1]), n: 24 },
        180000,
      )
      if (!r.ok || !r.data) throw new Error(tx(r.error) || t('Sem solução'))
      setData(r.data)
      if (r.data.results[0]) onPick({ ...r.data.results[0], from, via, to })
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'bad')
    }
    setBusy(false)
  }
  const direct = data?.direct ? Math.sqrt(data.direct[2]) + data.direct[3] : null
  const field = (label: string, value: string, set: (s: string) => void, w = 58) => (
    <label className="small">
      <span className="muted">{label}</span> <input value={value} onChange={(e) => set(e.target.value)} style={{ width: w }} />
    </label>
  )
  return (
    <Panel title={t('Gravidade assistida (um sobrevoo)')} className="span-2">
      <div className="row">
        <span>{name(from)}</span>
        <span>→</span>
        <select value={via} onChange={(e) => setVia(e.target.value)} aria-label={t('Planeta do sobrevoo')}>
          {PLANETS.filter((p) => p !== from && p !== to).map((p) => (
            <option key={p} value={p}>
              {name(p)}
            </option>
          ))}
        </select>
        <span>→</span>
        <span>{name(to)}</span>
        {field(t('partidas a partir de'), form.start, (s) => setForm({ ...form, start: s }), 100)}
        {field(t('por (dias)'), form.span, (s) => setForm({ ...form, span: s }))}
      </div>
      <div className="row" style={{ marginTop: 6 }}>
        {field(`${t('1º trecho (dias)')}`, t1[0], (s) => setForm({ ...form, t1: [s, form.t1[1] || t1[1]] }))}
        {field('–', t1[1], (s) => setForm({ ...form, t1: [form.t1[0] || t1[0], s] }))}
        {field(`${t('2º trecho (dias)')}`, t2[0], (s) => setForm({ ...form, t2: [s, form.t2[1] || t2[1]] }))}
        {field('–', t2[1], (s) => setForm({ ...form, t2: [form.t2[0] || t2[0], s] }))}
        <button className="primary" onClick={search} disabled={busy || bad}>
          {busy ? t('Procurando…') : t('Procurar')}
        </button>
      </div>
      <p className="small muted">{t('Origem e destino são os do porkchop. A busca resolve os dois trechos por Lambert e procura onde a gravidade do planeta do meio gira a velocidade de graça; o que não couber vira queima no periastro.')}</p>
      {data && (
        <>
          <div className="small" style={{ margin: '6px 0' }}>
            {direct != null && data.direct ? (
              <>
                {t('Direto')}: {day(data.direct[0])}, {num(data.direct[1], 0)} {t('dias')}, √C3 + v∞ = <b>{num(direct, 2)} km/s</b>
                {data.results[0] && (
                  <span className={data.results[0].cost < direct ? 'good' : 'muted'}>
                    {' '}
                    · {t('com sobrevoo')}: {num(data.results[0].cost, 2)} km/s ({data.results[0].cost < direct ? t('economia de {x} km/s', { x: num(direct - data.results[0].cost, 2) }) : t('mais caro que o direto')})
                  </span>
                )}
              </>
            ) : (
              t('Sem voo direto na mesma janela.')
            )}
            <span className="muted"> · {num(data.evaluated, 0)} {t('trajetórias avaliadas')}</span>
          </div>
          <div className="table-scroll">
            <table className="windows">
              <thead>
                <tr>
                  <th>{t('Partida')}</th>
                  <th>{t('Sobrevoo')}</th>
                  <th>{t('Chegada')}</th>
                  <th className="num">C3</th>
                  <th className="num">v∞ {t('no sobrevoo')}</th>
                  <th className="num">{t('Deflexão')}</th>
                  <th className="num">{t('Periastro')}</th>
                  <th className="num">Δv {t('no sobrevoo')}</th>
                  <th className="num">{t('Ganho')}</th>
                  <th className="num">v∞ {t('chegada')}</th>
                  <th className="num">{t('Custo')}</th>
                </tr>
              </thead>
              <tbody>
                {data.results.map((f) => (
                  <tr key={f.depart} className={picked?.depart === f.depart ? 'sel' : ''} onClick={() => onPick({ ...f, from: data.from, via: data.via, to: data.to })} style={{ cursor: 'pointer' }}>
                    <td>{day(f.depart)}</td>
                    <td>{day(f.flyby)}</td>
                    <td>{day(f.arrive)}</td>
                    <td className="num">{num(f.c3, 1)}</td>
                    <td className="num">
                      {num(f.vinf_in, 2)} → {num(f.vinf_out, 2)}
                    </td>
                    <td className="num">
                      {num(f.turn_deg, 0)}° / {num(f.turn_max_deg, 0)}°
                    </td>
                    <td className="num">{distance(f.rp_alt_km)}</td>
                    <td className={`num ${f.dv_flyby < 0.05 ? 'good' : ''}`}>{f.dv_flyby < 0.005 ? t('balístico') : `${num(f.dv_flyby, 2)} km/s`}</td>
                    <td className="num">{signed(f.gain)} km/s</td>
                    <td className="num">{num(f.vinf_arr, 2)}</td>
                    <td className="num">
                      <b>{num(f.cost, 2)}</b>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {!data.results.length && <p className="muted">{t('Nenhuma trajetória com esse sobrevoo nessas datas.')}</p>}
        </>
      )}
    </Panel>
  )
}

/** A hipérbole do sobrevoo no referencial do planeta: chegada, periastro, saída e a deflexão. */
export function FlybyDiagram({ f }: { f: FlybyPick }) {
  const ref = useCanvas(
    (ctx, w, h) => {
      ctx.fillStyle = '#050914'
      ctx.fillRect(0, 0, w, h)
      const radius = store.hello?.bodies.find((b) => b.id === f.via)?.radius ?? 6e6
      const mu = MU[f.via] ?? 3.986e14
      const rp = radius + f.rp_alt_km * 1e3
      const vinf = ((f.vinf_in + f.vinf_out) / 2) * 1e3
      const e = 1 + (rp * vinf * vinf) / mu
      const p = rp * (1 + e)
      const nuInf = Math.acos(-1 / e)
      // Escala: o planeta e uns 6 periastros cabem.
      const ext = Math.max(rp * 6, radius * 3)
      const k = Math.min(w, h) / 2 / ext
      // Periastro à esquerda do planeta; o eixo da hipérbole na horizontal.
      const cx = w * 0.58
      const cy = h / 2
      const P = (x: number, y: number) => [cx - x * k, cy - y * k] as const
      const colors: Record<string, string> = { mercury: '#a8a8a8', venus: '#e8c87a', earth: '#4cc3ff', mars: '#e0643a', jupiter: '#d8b48a', saturn: '#e6d39a', uranus: '#8fe3e8', neptune: '#5c84ff' }
      ctx.fillStyle = colors[f.via] ?? '#999'
      ctx.beginPath()
      ctx.arc(cx, cy, Math.max(3, radius * k), 0, Math.PI * 2)
      ctx.fill()
      // A trajetória.
      ctx.strokeStyle = '#ff8a3d'
      ctx.lineWidth = 2
      ctx.beginPath()
      let first = true
      for (let i = 0; i <= 200; i++) {
        const nu = -nuInf * 0.985 + ((2 * nuInf * 0.985) * i) / 200
        const r = p / (1 + e * Math.cos(nu))
        if (r > ext * 2) continue
        const [x, y] = P(r * Math.cos(nu), r * Math.sin(nu))
        if (first) ctx.moveTo(x, y)
        else ctx.lineTo(x, y)
        first = false
      }
      ctx.stroke()
      // Assíntotas (direções do v∞ na chegada e na saída).
      ctx.setLineDash([4, 4])
      ctx.strokeStyle = '#8494b3'
      ctx.lineWidth = 1
      const a = -mu / (vinf * vinf)
      const centerX = rp - a // centro da hipérbole (do lado de fora), no eixo
      for (const s of [-1, 1]) {
        const ang = s * nuInf
        const [x0, y0] = P(centerX, 0)
        const [x1, y1] = P(centerX + Math.cos(ang) * ext * 3, Math.sin(ang) * ext * 3)
        ctx.beginPath()
        ctx.moveTo(x0, y0)
        ctx.lineTo(x1, y1)
        ctx.stroke()
      }
      ctx.setLineDash([])
      // Periastro.
      const [px, py] = P(rp, 0)
      ctx.fillStyle = '#fff'
      ctx.beginPath()
      ctx.arc(px, py, 3, 0, Math.PI * 2)
      ctx.fill()
      ctx.font = '11px ui-monospace, monospace'
      ctx.fillStyle = '#d9e2f2'
      ctx.fillText(`${t('periastro')} ${distance(f.rp_alt_km)}`, px - 4 - ctx.measureText(`${t('periastro')} ${distance(f.rp_alt_km)}`).width, py - 8)
      const lines = [
        `${name(f.via)} · ${day(f.flyby)}`,
        `v∞ ${num(f.vinf_in, 2)} → ${num(f.vinf_out, 2)} km/s`,
        `${t('deflexão')} ${num(f.turn_deg, 1)}° (${t('máx.')} ${num(f.turn_max_deg, 1)}°)`,
        `${t('ganho heliocêntrico')} ${signed(f.gain)} km/s`,
        f.dv_flyby < 0.005 ? t('balístico: sem queima') : `${t('queima no periastro')} ${num(f.dv_flyby, 2)} km/s`,
      ]
      lines.forEach((l, i) => {
        ctx.fillStyle = i === 0 ? '#ffb27a' : '#9fb0d0'
        ctx.fillText(l, 10, 18 + i * 15)
      })
    },
    [f],
  )
  return (
    <Panel title={t('O sobrevoo')} flush>
      <div className="canvas-wrap" style={{ height: 300 }}>
        <canvas ref={ref} role="img" aria-label={t('O sobrevoo')} />
      </div>
    </Panel>
  )
}
