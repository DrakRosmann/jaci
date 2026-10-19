import { useState } from 'react'
import { useCanvas } from '../components/Chart'
import { Panel, Tile, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { distance, duration, num, utc } from '../lib/format'
import { usePrediction } from '../lib/hooks'
import { store, useStore } from '../lib/store'
import { Planner } from './OrbitView'

interface Porkchop {
  from: string
  to: string
  departures: number[]
  tofs_days: number[]
  c3: (number | null)[][]
  vinf_arr: (number | null)[][]
  best: [number, number, number, number] | null
}

const AU = 149_597_870_700
const EPS = (23.439281 * Math.PI) / 180
/** Equatorial J2000 → plano da eclíptica (vista de cima). */
const ecl = (x: number, y: number, z: number): [number, number] => [x, y * Math.cos(EPS) + z * Math.sin(EPS)]

export function Interplanetary() {
  const v = useStore((s) => s.selected)
  const snap = useStore((s) => s.snap)!
  const hello = useStore((s) => s.hello)!
  const [pc, setPc] = useState<Porkchop | null>(null)
  const [busy, setBusy] = useState(false)
  const [form, setForm] = useState({ to: 'mars', start: utc(snap.t).slice(0, 10), span: '400', tmin: '120', tmax: '400' })
  const [pick, setPick] = useState<{ dep: number; tof: number; c3: number } | null>(null)
  const heliospan = v?.central === 'sun' ? 86_400 * 400 : 86_400 * 30
  const pred = usePrediction(v && v.kind === 'probe' ? v.id : null, heliospan, 8000, heliospan / 300)
  const run = async () => {
    setBusy(true)
    try {
      const r = await store.request<{ data: Porkchop }>(
        { type: 'porkchop', from: 'earth', to: form.to, start: `${form.start} 00:00:00`, span_days: Number(form.span), tof_min: Number(form.tmin), tof_max: Number(form.tmax), n: 45 },
        120000,
      )
      setPc(r.data)
    } catch (e) {
      toast(String(e), 'bad')
    }
    setBusy(false)
  }
  const earth = snap.bodies.find((b) => b.id === 'earth')!
  let helio: [number, number, number] | null = null
  if (v?.r) {
    if (v.central === 'sun') helio = v.r
    else {
      const c = snap.bodies.find((b) => b.id === v.central)
      if (c) helio = [c.r[0] + v.r[0], c.r[1] + v.r[1], c.r[2] + v.r[2]]
    }
  }
  const fromEarth = helio ? Math.hypot(helio[0] - earth.r[0], helio[1] - earth.r[1], helio[2] - earth.r[2]) : NaN
  // Trajetória prevista em heliocêntrico: soma a posição do corpo central (aproximação: a de agora).
  const central = snap.bodies.find((b) => b.id === (pred.central ?? 'sun'))
  const off = pred.central === 'sun' || !central ? [0, 0, 0] : central.r
  const path = pred.points.map((p) => ecl(p[4] + off[0], p[5] + off[1], p[6] + off[2]))
  return (
    <div className="grid">
      <div className="grid cols-3">
        <Panel title={t('Sistema Solar (vista de cima)')} className="span-2" flush>
          <SolarSystem path={path} probe={helio} />
        </Panel>
        <Panel title={t('Sonda')}>
          {v ? (
            <div className="tiles">
              <Tile label={t('Veículo')} value={tx(v.name)} />
              <Tile label={t('Corpo central')} value={tx(hello.bodies.find((b) => b.id === v.central)?.name ?? v.central)} />
              <Tile label={t('Distância da Terra')} value={distance(fromEarth / 1000)} />
              <Tile label={t('Atraso da luz (ida)')} value={duration(fromEarth / 299_792_458)} />
              <Tile label={t('Distância do Sol')} value={helio ? distance(Math.hypot(...helio) / 1000) : '—'} />
              <Tile label={t('Velocidade')} value={num(v.speed, 0)} unit="m/s" />
            </div>
          ) : (
            <span className="muted">{t('Selecione a sonda.')}</span>
          )}
        </Panel>
      </div>
      <div className="grid cols-3">
        <Panel title={t('Porkchop: janelas de lançamento')} className="span-2">
          <div className="row">
            <span>{t('Terra →')}</span>
            <select value={form.to} onChange={(e) => setForm({ ...form, to: e.target.value })}>
              {['mars', 'venus', 'mercury', 'jupiter', 'saturn'].map((b) => (
                <option key={b} value={b}>
                  {tx(hello.bodies.find((x) => x.id === b)?.name ?? b)}
                </option>
              ))}
            </select>
            <span className="muted">{t('a partir de')}</span>
            <input value={form.start} onChange={(e) => setForm({ ...form, start: e.target.value })} style={{ width: 110 }} />
            <span className="muted">{t('por (dias)')}</span>
            <input value={form.span} onChange={(e) => setForm({ ...form, span: e.target.value })} style={{ width: 60 }} />
            <span className="muted">{t('viagem (dias)')}</span>
            <input value={form.tmin} onChange={(e) => setForm({ ...form, tmin: e.target.value })} style={{ width: 55 }} />
            <input value={form.tmax} onChange={(e) => setForm({ ...form, tmax: e.target.value })} style={{ width: 55 }} />
            <button className="primary" onClick={run} disabled={busy}>
              {busy ? t('Calculando…') : t('Calcular')}
            </button>
          </div>
          {pc && <PorkchopPlot pc={pc} onPick={setPick} />}
          {pc?.best && (
            <div className="small" style={{ marginTop: 6 }}>
              {t('Melhor')}: {t('partida')} {utc(pc.best[0]).slice(0, 10)}, {num(pc.best[1], 0)} {t('dias')}, C3 {num(pc.best[2], 1)} km²/s², v∞ {t('chegada')} {num(pc.best[3], 2)} km/s
            </div>
          )}
          {pick && (
            <div className="small" style={{ marginTop: 4 }}>
              {t('Escolhido')}: {t('partida')} {utc(pick.dep).slice(0, 10)} ({t('em')} {duration(pick.dep - snap.t)}), {num(pick.tof, 0)} {t('dias')}, C3 {num(pick.c3, 1)} —{' '}
              {t('no planejador: partida em {h} h, viagem de {d} dias', { h: Math.round((pick.dep - snap.t) / 3600), d: Math.round(pick.tof) })}
            </div>
          )}
        </Panel>
        {v && <Planner v={v} initial={v.central === 'earth' ? 'injection' : v.central === 'sun' ? 'tcm' : 'capture'} />}
      </div>
    </div>
  )
}

function PorkchopPlot({ pc, onPick }: { pc: Porkchop; onPick: (p: { dep: number; tof: number; c3: number }) => void }) {
  const ref = useCanvas(
    (ctx, w, h) => {
      const pad = { l: 50, b: 28, t: 8, r: 70 }
      const nx = pc.departures.length
      const ny = pc.tofs_days.length
      const cw = (w - pad.l - pad.r) / nx
      const ch = (h - pad.t - pad.b) / ny
      ctx.fillStyle = '#081426'
      ctx.fillRect(0, 0, w, h)
      const vals = pc.c3.flat().filter((x): x is number => x != null && isFinite(x))
      const lo = Math.min(...vals)
      const hi = Math.min(Math.max(...vals), lo * 4 + 10)
      for (let j = 0; j < ny; j++) {
        for (let i = 0; i < nx; i++) {
          const c = pc.c3[j][i]
          if (c == null) continue
          const f = Math.min(1, (c - lo) / (hi - lo))
          ctx.fillStyle = `hsl(${(1 - f) * 220}, 80%, ${18 + (1 - f) * 40}%)`
          ctx.fillRect(pad.l + i * cw, h - pad.b - (j + 1) * ch, cw + 0.5, ch + 0.5)
        }
      }
      ctx.fillStyle = '#8494b3'
      ctx.font = '11px ui-monospace, monospace'
      for (let i = 0; i < nx; i += Math.ceil(nx / 6)) ctx.fillText(utc(pc.departures[i]).slice(2, 10), pad.l + i * cw, h - 8)
      for (let j = 0; j < ny; j += Math.ceil(ny / 6)) ctx.fillText(`${pc.tofs_days[j].toFixed(0)} d`, 6, h - pad.b - j * ch)
      if (pc.best) {
        const i = pc.departures.findIndex((d) => d >= pc.best![0])
        const j = pc.tofs_days.findIndex((d) => d >= pc.best![1])
        ctx.strokeStyle = '#fff'
        ctx.strokeRect(pad.l + i * cw - 3, h - pad.b - (j + 1) * ch - 3, cw + 6, ch + 6)
      }
      // Escala.
      for (let k = 0; k < 100; k++) {
        const f = k / 99
        ctx.fillStyle = `hsl(${(1 - f) * 220}, 80%, ${18 + (1 - f) * 40}%)`
        ctx.fillRect(w - pad.r + 12, pad.t + (1 - f) * (h - pad.t - pad.b), 12, (h - pad.t - pad.b) / 99 + 1)
      }
      ctx.fillStyle = '#d9e2f2'
      ctx.fillText(`${hi.toFixed(0)}`, w - pad.r + 28, pad.t + 10)
      ctx.fillText(`${lo.toFixed(1)}`, w - pad.r + 28, h - pad.b)
      ctx.fillText('C3', w - pad.r + 28, (h - pad.b) / 2)
    },
    [pc],
  )
  const click = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    const pad = { l: 50, b: 28, t: 8, r: 70 }
    const x = e.clientX - r.left
    const y = e.clientY - r.top
    const i = Math.floor(((x - pad.l) / (r.width - pad.l - pad.r)) * pc.departures.length)
    const j = Math.floor(((r.height - pad.b - y) / (r.height - pad.t - pad.b)) * pc.tofs_days.length)
    const c = pc.c3[j]?.[i]
    if (c != null) onPick({ dep: pc.departures[i], tof: pc.tofs_days[j], c3: c })
  }
  return (
    <div className="canvas-wrap" style={{ height: 320, marginTop: 8 }}>
      <canvas ref={ref} onClick={click} style={{ cursor: 'crosshair' }} />
    </div>
  )
}

function SolarSystem({ path, probe }: { path: [number, number][]; probe: [number, number, number] | null }) {
  const snap = useStore((s) => s.snap)!
  const [zoom, setZoom] = useState(2.2)
  const ref = useCanvas(
    (ctx, w, h) => {
      ctx.fillStyle = '#050914'
      ctx.fillRect(0, 0, w, h)
      const s = Math.min(w, h) / 2 / (zoom * AU)
      const X = (x: number) => w / 2 + x * s
      const Y = (y: number) => h / 2 - y * s
      ctx.fillStyle = '#ffcc33'
      ctx.beginPath()
      ctx.arc(w / 2, h / 2, 6, 0, Math.PI * 2)
      ctx.fill()
      ctx.font = '11px system-ui'
      const colors: Record<string, string> = { mercury: '#999', venus: '#e8c87a', earth: '#4cc3ff', mars: '#e0643a', jupiter: '#d8b48a', saturn: '#e6d39a', uranus: '#8fe3e8', neptune: '#5c84ff' }
      for (const b of snap.bodies) {
        if (b.id === 'sun' || b.id === 'moon') continue
        const [bx, by] = ecl(b.r[0], b.r[1], b.r[2])
        const r = Math.hypot(bx, by)
        ctx.strokeStyle = '#1a2540'
        ctx.beginPath()
        ctx.arc(w / 2, h / 2, r * s, 0, Math.PI * 2)
        ctx.stroke()
        ctx.fillStyle = colors[b.id] ?? '#ccc'
        ctx.beginPath()
        ctx.arc(X(bx), Y(by), 4, 0, Math.PI * 2)
        ctx.fill()
        ctx.fillText(tx(store.hello?.bodies.find((x) => x.id === b.id)?.name ?? b.id), X(bx) + 6, Y(by) - 4)
      }
      if (path.length > 1) {
        ctx.strokeStyle = '#2dd4bf'
        ctx.beginPath()
        path.forEach(([x, y], i) => (i ? ctx.lineTo(X(x), Y(y)) : ctx.moveTo(X(x), Y(y))))
        ctx.stroke()
      }
      if (probe) {
        const [px, py] = ecl(probe[0], probe[1], probe[2])
        ctx.fillStyle = '#fff'
        ctx.fillRect(X(px) - 3, Y(py) - 3, 6, 6)
      }
      ctx.fillStyle = '#8494b3'
      ctx.fillText(`${zoom} ${t('UA')}`, 8, h - 8)
    },
    [snap, path, probe, zoom],
  )
  return (
    <div style={{ position: 'relative' }}>
      <div className="canvas-wrap" style={{ height: 420 }}>
        <canvas ref={ref} />
      </div>
      <div className="row" style={{ position: 'absolute', top: 8, right: 8 }}>
        {[0.01, 1.2, 2.2, 6, 32].map((z) => (
          <button key={z} className={`small ${zoom === z ? 'active' : ''}`} onClick={() => setZoom(z)}>
            {z} {t('UA')}
          </button>
        ))}
      </div>
    </div>
  )
}
