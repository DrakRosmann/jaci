// Interplanetário: o Sistema Solar visto de cima (as órbitas de verdade, zoom
// pela roda e arrasto, a trajetória da sonda e a transferência escolhida no
// porkchop), as janelas de Hohmann de cada planeta, o porkchop com a métrica
// escolhida e o planejador, que recebe a janela escolhida.

import { useMemo, useRef, useState } from 'react'
import { useCanvas } from '../components/Chart'
import { Panel, Tile, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { distance, duration, num, utc } from '../lib/format'
import { usePrediction } from '../lib/hooks'
import { add, norm, type V3 } from '../lib/attitude'
import { AU, PLANETS, flybyArcs, hohmannWindow, lambert, orbitPath, planetPosition, planetVelocity, transferArc, type Window } from '../lib/ephem'
import { MU, nextNode } from '../lib/mfd'
import { elements } from '../lib/attitude'
import { store, useStore } from '../lib/store'
import { Planner, type PlannerPreset } from './OrbitView'
import { FlybyDiagram, FlybyPanel, type FlybyPick } from './Flyby'
import type { Vehicle } from '../types'

interface Porkchop {
  from: string
  to: string
  departures: number[]
  tofs_days: number[]
  c3: (number | null)[][]
  vinf_arr: (number | null)[][]
  best: [number, number, number, number] | null
}

type Metric = 'c3' | 'vinf' | 'total' | 'dv'

interface Pick {
  dep: number
  tof: number
  c3: number
  vinf: number
}

const DAY = 86_400
const EPS = (23.439281 * Math.PI) / 180
/** Equatorial J2000 → plano da eclíptica (vista de cima). */
const ecl = (x: number, y: number, z: number): [number, number] => [x, y * Math.cos(EPS) + z * Math.sin(EPS)]
/** Eclíptica → equatorial J2000. */
const equatorial = (p: V3): V3 => [p[0], p[1] * Math.cos(EPS) - p[2] * Math.sin(EPS), p[1] * Math.sin(EPS) + p[2] * Math.cos(EPS)]

const COLORS: Record<string, string> = { mercury: '#a8a8a8', venus: '#e8c87a', earth: '#4cc3ff', mars: '#e0643a', jupiter: '#d8b48a', saturn: '#e6d39a', uranus: '#8fe3e8', neptune: '#5c84ff', moon: '#bbb' }
const RADIUS: Record<string, number> = { mercury: 3, venus: 4, earth: 4.5, mars: 4, jupiter: 7, saturn: 6.5, uranus: 5, neptune: 5, moon: 2.5 }

const bodyName = (id: string) => tx(store.hello?.bodies.find((b) => b.id === id)?.name ?? id)
const day = (t: number) => utc(t).slice(0, 10)
const deg = (r: number) => `${num((r * 180) / Math.PI, 0)}°`

export function Interplanetary() {
  const v = useStore((s) => s.selected)
  const snap = useStore((s) => s.snap)!
  const [pc, setPc] = useState<Porkchop | null>(null)
  const [busy, setBusy] = useState(false)
  const [form, setForm] = useState({ from: 'earth', to: 'mars', start: day(snap.t), span: '400', tmin: '120', tmax: '400' })
  const [metric, setMetric] = useState<Metric>('c3')
  const [pick, setPick] = useState<Pick | null>(null)
  const [preset, setPreset] = useState<PlannerPreset>()
  // Partir da órbita de verdade da sonda (quando ela está no planeta de origem).
  const [fromProbe, setFromProbe] = useState(true)
  const [flyby, setFlyby] = useState<FlybyPick | null>(null)
  const heliospan = v?.central === 'sun' ? 86_400 * 400 : 86_400 * 30
  const pred = usePrediction(v && v.kind === 'probe' ? v.id : null, heliospan, 8000, heliospan / 300)
  const run = async (f = form) => {
    setBusy(true)
    try {
      const r = await store.request<{ data: Porkchop }>(
        { type: 'porkchop', from: f.from, to: f.to, start: `${f.start} 00:00:00`, span_days: Number(f.span), tof_min: Number(f.tmin), tof_max: Number(f.tmax), n: 45 },
        120000,
      )
      setPc(r.data)
      const b = r.data.best
      setPick(b ? { dep: b[0], tof: b[1], c3: b[2], vinf: b[3] } : null)
    } catch (e) {
      toast(String(e), 'bad')
    }
    setBusy(false)
  }
  // As janelas mudam devagar: recalcula a cada hora simulada.
  const hour = Math.floor(snap.t / 3600)
  const windows = useMemo(
    () => PLANETS.filter((p) => p !== form.from).map((p) => hohmannWindow(form.from, p, hour * 3600)).filter((w): w is Window => w != null),
    [form.from, hour],
  )
  /** Porkchop em volta da próxima janela de Hohmann. */
  const aroundWindow = (w: Window) => {
    const span = Math.min(240, Math.max(60, (0.35 * w.synodic) / DAY))
    const start = Math.max(snap.t, w.next - (span / 2) * DAY)
    const tof = w.tof / DAY
    const f = { ...form, to: w.to, start: day(start), span: String(Math.round(span)), tmin: String(Math.round(tof * 0.6)), tmax: String(Math.round(tof * 1.4)) }
    setForm(f)
    run(f)
  }
  // Onde a sonda está, no heliocêntrico.
  const earth = snap.bodies.find((b) => b.id === 'earth')!
  let helio: V3 | null = null
  let helioV: V3 | null = null
  if (v?.r) {
    if (v.central === 'sun') {
      helio = v.r
      helioV = v.v ?? null
    } else {
      const c = snap.bodies.find((b) => b.id === v.central)
      if (c) helio = [c.r[0] + v.r[0], c.r[1] + v.r[1], c.r[2] + v.r[2]]
      const cv = planetVelocity(v.central === 'moon' ? 'earth' : v.central, snap.t)
      if (v.v && cv) helioV = add(v.v as V3, equatorial(cv))
    }
  }
  const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
  const fromEarth = helio ? norm(sub(helio, earth.r)) : NaN
  // Elongação da sonda vista da Terra (ângulo Sol–Terra–sonda): perto de 0°, o Sol atrapalha o enlace.
  const sep = helio && v?.central !== 'earth' ? Math.acos(Math.max(-1, Math.min(1, -(earth.r[0] * (helio[0] - earth.r[0]) + earth.r[1] * (helio[1] - earth.r[1]) + earth.r[2] * (helio[2] - earth.r[2])) / (norm(earth.r) * fromEarth)))) : null
  const dest = snap.bodies.find((b) => b.id === form.to)
  const nn = v ? nextNode(v) : undefined
  // Trajetória prevista em heliocêntrico: soma a posição do corpo central (aproximação: a de agora).
  const central = snap.bodies.find((b) => b.id === (pred.central ?? 'sun'))
  const off = pred.central === 'sun' || !central ? [0, 0, 0] : central.r
  const path = pred.points.map((p) => ecl(p[4] + off[0], p[5] + off[1], p[6] + off[2]))
  const usable = !!(v && pick && v.central === form.from && pick.dep > snap.t)
  const toPlanner = () => {
    if (!pick) return
    setPreset({ kind: 'injection', params: { depart_in_h: String(Math.round((pick.dep - snap.t) / 3600)), tof_days: String(Math.round(pick.tof)) }, body: form.to, n: Date.now() })
    toast(t('Janela levada ao planejador: confira e planeje.'), 'good')
  }
  const depart = departure(v, form.from, fromProbe)
  const injection = pick && depart ? injectionDv(depart, pick.c3) : null
  const spare = injection != null && depart?.budget != null ? depart.budget - injection : null
  return (
    <div className="grid">
      <div className="grid cols-3">
        <Panel title={t('Sistema Solar (vista de cima)')} className="span-2" flush>
          <SolarSystem path={path} probe={helio} from={form.from} to={form.to} pick={pick} flyby={flyby} />
        </Panel>
        <Panel title={v ? tx(v.name) : t('Sonda')}>
          {v ? (
            <div className="tiles">
              <Tile label={t('Corpo central')} value={bodyName(v.central)} />
              <Tile label={t('Velocidade')} value={num(v.speed, 0)} unit="m/s" />
              <Tile label={t('Velocidade heliocêntrica')} value={helioV ? num(norm(helioV) / 1000, 2) : '—'} unit="km/s" />
              <Tile label={t('Distância do Sol')} value={helio ? `${num(norm(helio) / AU, 3)} ${t('UA')}` : '—'} />
              <Tile label={t('Distância da Terra')} value={distance(fromEarth / 1000)} />
              <Tile label={t('Atraso da luz (ida)')} value={duration(fromEarth / 299_792_458)} />
              <Tile label={t('Elongação (Sol–Terra–sonda)')} value={sep != null ? deg(sep) : '—'} state={sep != null && sep < (5 * Math.PI) / 180 ? 'red' : sep != null && sep < (15 * Math.PI) / 180 ? 'yellow' : 'none'} title={t('Perto do Sol no céu da Terra, o enlace sofre com o ruído solar (conjunção)')} />
              <Tile label={`${t('Distância a')} ${bodyName(form.to)}`} value={helio && dest ? distance(norm(sub(helio, dest.r)) / 1000) : '—'} />
              <Tile label={t('Próxima queima')} value={nn ? `${tx(nn.label)} · ${nn.executing ? t('queimando') : duration(nn.t - snap.t)}` : '—'} />
            </div>
          ) : (
            <span className="muted">{t('Selecione a sonda.')}</span>
          )}
        </Panel>
      </div>
      <Panel
        title={t('Janelas de transferência (Hohmann)')}
        right={
          <label className="small">
            {t('Origem')}{' '}
            <select value={form.from} onChange={(e) => setForm({ ...form, from: e.target.value, to: e.target.value === form.to ? (e.target.value === 'earth' ? 'mars' : 'earth') : form.to })}>
              {PLANETS.map((b) => (
                <option key={b} value={b}>
                  {bodyName(b)}
                </option>
              ))}
            </select>
          </label>
        }
      >
        <div className="table-scroll">
          <table className="windows">
            <thead>
              <tr>
                <th>{t('Destino')}</th>
                <th className="num">{t('Viagem')}</th>
                <th className="num">C3 (km²/s²)</th>
                <th className="num">v∞ {t('chegada')} (km/s)</th>
                <th className="num">{t('Fase agora')}</th>
                <th className="num">{t('Fase da transferência')}</th>
                <th>{t('Próxima janela')}</th>
                <th className="num">{t('Sinódico')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {windows.map((w) => {
                const soon = w.next - snap.t < 60 * DAY
                return (
                  <tr key={w.to} className={w.to === form.to ? 'sel' : ''}>
                    <td>
                      <span className="dot" style={{ background: COLORS[w.to] }} /> {bodyName(w.to)}
                    </td>
                    <td className="num">{fmtDays(w.tof / DAY)}</td>
                    <td className="num">{num(w.c3, 1)}</td>
                    <td className="num">{num(w.vinfArr, 2)}</td>
                    <td className="num">{deg(w.phase)}</td>
                    <td className="num">{deg(w.phaseReq)}</td>
                    <td className={soon ? 'good' : ''}>
                      {day(w.next)} <span className="muted">({t('em')} {fmtDays((w.next - snap.t) / DAY)})</span>
                    </td>
                    <td className="num">{fmtDays(w.synodic / DAY)}</td>
                    <td>
                      <button className="small" onClick={() => aroundWindow(w)} disabled={busy} title={t('Calcula o porkchop em volta desta janela')}>
                        {t('Porkchop')} ▸
                      </button>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
        <p className="small muted">{t('Órbitas circulares e no mesmo plano: uma primeira ideia. O porkchop resolve o problema de Lambert com as órbitas de verdade.')}</p>
      </Panel>
      <div className="grid cols-3">
        <Panel
          title={t('Porkchop: janelas de lançamento')}
          className="span-2"
          right={
            <div className="seg">
              {(
                [
                  ['c3', 'C3'],
                  ['vinf', t('v∞ chegada')],
                  ['total', t('Total')],
                  ['dv', t('Δv de injeção')],
                ] as [Metric, string][]
              ).map(([m, label]) => (
                <button key={m} className={`small ${metric === m ? 'active' : ''}`} onClick={() => setMetric(m)} aria-pressed={metric === m}>
                  {label}
                </button>
              ))}
            </div>
          }
        >
          <div className="row">
            <select value={form.from} onChange={(e) => setForm({ ...form, from: e.target.value })} aria-label={t('Origem')}>
              {PLANETS.filter((b) => b !== form.to).map((b) => (
                <option key={b} value={b}>
                  {bodyName(b)}
                </option>
              ))}
            </select>
            <span>→</span>
            <select value={form.to} onChange={(e) => setForm({ ...form, to: e.target.value })} aria-label={t('Destino')}>
              {PLANETS.filter((b) => b !== form.from).map((b) => (
                <option key={b} value={b}>
                  {bodyName(b)}
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
            <button className="primary" onClick={() => run()} disabled={busy}>
              {busy ? t('Calculando…') : t('Calcular')}
            </button>
          </div>
          {v && v.central === form.from && v.r && (
            <label className="small porkchop-probe">
              <input type="checkbox" checked={fromProbe} onChange={(e) => setFromProbe(e.target.checked)} /> {t('Partir da órbita de {n}', { n: tx(v.name) })}
              {depart?.probe && (
                <span className="muted">
                  {' '}
                  · {t('periastro')} {num((depart.rp - (store.hello?.bodies.find((b) => b.id === form.from)?.radius ?? 0)) / 1000, 0)} km · Δv {t('disponível')} {num(depart.budget, 0)} m/s
                </span>
              )}
            </label>
          )}
          {pc ? <PorkchopPlot pc={pc} metric={metric} pick={pick} onPick={setPick} depart={depart} /> : <div className="porkchop-empty muted">{t('Escolha origem, destino e datas e calcule — ou toque em “Porkchop ▸” numa janela acima.')}</div>}
          {pick && (
            <div className="pick">
              <div className="tiles">
                <Tile label={t('Partida')} value={day(pick.dep)} title={`${t('em')} ${duration(pick.dep - snap.t)}`} />
                <Tile label={t('Chegada')} value={day(pick.dep + pick.tof * DAY)} />
                <Tile label={t('Viagem')} value={num(pick.tof, 0)} unit={t('dias')} />
                <Tile label="C3" value={num(pick.c3, 1)} unit="km²/s²" />
                <Tile label={t('v∞ partida')} value={num(Math.sqrt(pick.c3), 2)} unit="km/s" />
                <Tile label={t('v∞ chegada')} value={num(pick.vinf, 2)} unit="km/s" />
                {injection != null && <Tile label={depart?.probe ? t('Δv de injeção da sonda') : t('Δv da órbita de 200 km')} value={num(injection, 0)} unit="m/s" />}
                {spare != null && <Tile label={t('Sobra depois da injeção')} value={num(spare, 0)} unit="m/s" state={spare < 0 ? 'red' : spare < 200 ? 'yellow' : 'nominal'} />}
              </div>
              <div className="row">
                {pc?.best && pick.dep === pc.best[0] && pick.tof === pc.best[1] && <span className="small good">★ {t('Melhor do gráfico (menor √C3 + v∞)')}</span>}
                <button className="primary small" onClick={toPlanner} disabled={!usable} title={usable ? t('Preenche a injeção no planejador') : t('A sonda selecionada precisa estar na origem, antes da partida')}>
                  {t('Usar no planejador')}
                </button>
              </div>
            </div>
          )}
        </Panel>
        {v && <Planner v={v} initial={v.central === 'earth' ? 'injection' : v.central === 'sun' ? 'tcm' : 'capture'} preset={preset} />}
      </div>
      <div className="grid cols-3">
        <FlybyPanel from={form.from} to={form.to} onPick={setFlyby} picked={flyby} />
        {flyby ? <FlybyDiagram f={flyby} /> : <div />}
      </div>
    </div>
  )
}

const fmtDays = (d: number) => (d >= 730 ? `${num(d / 365.25, 1)} ${t('anos')}` : `${num(d, 0)} ${t('dias')}`)

/** De onde a injeção parte: o periastro da órbita da sonda, ou uma órbita circular de 200 km. */
interface Departure {
  mu: number
  rp: number
  /** Velocidade no periastro (m/s). */
  vp: number
  /** Δv da sonda (m/s), quando a partida é da órbita dela. */
  budget: number | null
  probe: boolean
}

function departure(v: Vehicle | undefined, from: string, fromProbe: boolean): Departure | null {
  const mu = MU[from]
  if (!mu) return null
  if (fromProbe && v && v.central === from && v.r && v.v) {
    const el = elements(v.r as V3, v.v as V3, mu)
    if (el.e < 1) return { mu, rp: el.periapsis, vp: Math.sqrt(mu * (2 / el.periapsis - 1 / el.a)), budget: v.tm?.['nav.dv'] ?? null, probe: true }
  }
  const radius = store.hello?.bodies.find((b) => b.id === from)?.radius
  if (!radius) return null
  const rp = radius + 200_000
  return { mu, rp, vp: Math.sqrt(mu / rp), budget: null, probe: false }
}

/** Δv da queima no periastro que deixa a órbita com o C3 dado (m/s). */
function injectionDv(d: Departure, c3: number): number {
  return Math.sqrt(c3 * 1e6 + (2 * d.mu) / d.rp) - d.vp
}

/** Paleta sequencial (do escuro ao claro), boa para ler "melhor" e "pior". */
const STOPS = ['#fde725', '#7ad151', '#22a884', '#2a788e', '#414487', '#2b1145']
function ramp(f: number): string {
  const x = Math.max(0, Math.min(1, f)) * (STOPS.length - 1)
  const i = Math.min(STOPS.length - 2, Math.floor(x))
  const k = x - i
  const c = (h: string, o: number) => parseInt(h.slice(o, o + 2), 16)
  const mix = (o: number) => Math.round(c(STOPS[i], o) * (1 - k) + c(STOPS[i + 1], o) * k)
  return `rgb(${mix(1)},${mix(3)},${mix(5)})`
}

const PAD = { l: 56, b: 34, t: 10, r: 78 }

function PorkchopPlot({ pc, metric, pick, onPick, depart }: { pc: Porkchop; metric: Metric; pick: Pick | null; onPick: (p: Pick) => void; depart: Departure | null }) {
  const [hover, setHover] = useState<{ i: number; j: number; x: number; y: number } | null>(null)
  const value = (i: number, j: number): number | null => {
    const c = pc.c3[j]?.[i]
    const a = pc.vinf_arr[j]?.[i]
    if (c == null || a == null) return null
    if (metric === 'dv') return depart ? injectionDv(depart, c) / 1000 : null
    return metric === 'c3' ? c : metric === 'vinf' ? a : Math.sqrt(c) + a
  }
  // Células além do Δv da sonda: a injeção não cabe no tanque.
  const over = (i: number, j: number) => {
    const c = pc.c3[j]?.[i]
    return c != null && depart?.budget != null && injectionDv(depart, c) > depart.budget
  }
  const unit = metric === 'c3' ? 'km²/s²' : 'km/s'
  const ref = useCanvas(
    (ctx, w, h) => {
      const nx = pc.departures.length
      const ny = pc.tofs_days.length
      const cw = (w - PAD.l - PAD.r) / nx
      const ch = (h - PAD.t - PAD.b) / ny
      ctx.fillStyle = '#060b16'
      ctx.fillRect(0, 0, w, h)
      const vals: number[] = []
      for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) {
        const x = value(i, j)
        if (x != null && isFinite(x)) vals.push(x)
      }
      if (!vals.length) return
      vals.sort((a, b) => a - b)
      const lo = vals[0]
      // A escala se concentra perto do mínimo (onde estão as janelas): o que passa disso é "caro" e fica escuro.
      const hi = Math.max(lo + 1e-6, Math.min(vals[Math.floor(vals.length * 0.6)], metric === 'c3' ? lo * 3 + 10 : lo * 2 + 2))
      for (let j = 0; j < ny; j++) {
        for (let i = 0; i < nx; i++) {
          const c = value(i, j)
          if (c == null) continue
          ctx.fillStyle = ramp((c - lo) / (hi - lo))
          ctx.fillRect(PAD.l + i * cw, h - PAD.b - (j + 1) * ch, cw + 0.5, ch + 0.5)
        }
      }
      // Além do Δv da sonda: hachurado.
      if (depart?.budget != null) {
        ctx.save()
        ctx.beginPath()
        for (let j = 0; j < ny; j++) for (let i = 0; i < nx; i++) if (over(i, j)) ctx.rect(PAD.l + i * cw, h - PAD.b - (j + 1) * ch, cw + 0.5, ch + 0.5)
        ctx.clip()
        ctx.fillStyle = '#000000a0'
        ctx.fillRect(PAD.l, PAD.t, w - PAD.l - PAD.r, h - PAD.t - PAD.b)
        ctx.strokeStyle = '#ff4d5e90'
        ctx.lineWidth = 1
        ctx.beginPath()
        for (let x = PAD.l - h; x < w; x += 7) {
          ctx.moveTo(x, h)
          ctx.lineTo(x + h, 0)
        }
        ctx.stroke()
        ctx.restore()
      }
      ctx.fillStyle = '#8494b3'
      ctx.font = '11px ui-monospace, monospace'
      ctx.textAlign = 'center'
      for (let i = 0; i < nx; i += Math.ceil(nx / 6)) ctx.fillText(utc(pc.departures[i]).slice(2, 10), PAD.l + (i + 0.5) * cw, h - PAD.b + 14)
      ctx.fillText(t('partida'), PAD.l + (w - PAD.l - PAD.r) / 2, h - 4)
      ctx.textAlign = 'right'
      for (let j = 0; j < ny; j += Math.ceil(ny / 6)) ctx.fillText(`${pc.tofs_days[j].toFixed(0)} d`, PAD.l - 6, h - PAD.b - (j + 0.5) * ch + 4)
      ctx.textAlign = 'left'
      // Melhor (★) e escolhido (○).
      const cell = (dep: number, tof: number) => [pc.departures.findIndex((d) => d >= dep - 1), pc.tofs_days.findIndex((d) => d >= tof - 1e-6)]
      if (pc.best) {
        const [i, j] = cell(pc.best[0], pc.best[1])
        ctx.strokeStyle = '#fff'
        ctx.lineWidth = 1.5
        ctx.strokeRect(PAD.l + i * cw - 3, h - PAD.b - (j + 1) * ch - 3, cw + 6, ch + 6)
      }
      if (pick) {
        const [i, j] = cell(pick.dep, pick.tof)
        ctx.strokeStyle = '#ff8a3d'
        ctx.lineWidth = 2
        ctx.beginPath()
        ctx.arc(PAD.l + (i + 0.5) * cw, h - PAD.b - (j + 0.5) * ch, Math.max(cw, ch) + 3, 0, Math.PI * 2)
        ctx.stroke()
      }
      if (hover) {
        ctx.strokeStyle = '#ffffff80'
        ctx.lineWidth = 1
        ctx.beginPath()
        ctx.moveTo(PAD.l + (hover.i + 0.5) * cw, PAD.t)
        ctx.lineTo(PAD.l + (hover.i + 0.5) * cw, h - PAD.b)
        ctx.moveTo(PAD.l, h - PAD.b - (hover.j + 0.5) * ch)
        ctx.lineTo(w - PAD.r, h - PAD.b - (hover.j + 0.5) * ch)
        ctx.stroke()
      }
      // Escala.
      const sx = w - PAD.r + 14
      for (let k = 0; k < 100; k++) {
        ctx.fillStyle = ramp(k / 99)
        ctx.fillRect(sx, h - PAD.b - ((k + 1) / 100) * (h - PAD.t - PAD.b), 12, (h - PAD.t - PAD.b) / 100 + 1)
      }
      ctx.fillStyle = '#d9e2f2'
      ctx.fillText(`${hi.toFixed(metric === 'c3' ? 0 : 1)}+`, sx + 16, PAD.t + 10)
      ctx.fillText(lo.toFixed(metric === 'c3' ? 1 : 2), sx + 16, h - PAD.b)
      ctx.fillText(metric === 'c3' ? 'C3' : metric === 'vinf' ? 'v∞' : metric === 'dv' ? 'Δv' : 'Σv∞', sx + 16, (h - PAD.b) / 2)
      ctx.fillStyle = '#8494b3'
      ctx.fillText(unit, sx + 16, (h - PAD.b) / 2 + 14)
    },
    [pc, metric, pick, hover, depart],
  )
  const at = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const r = e.currentTarget.getBoundingClientRect()
    const x = e.clientX - r.left
    const y = e.clientY - r.top
    const i = Math.floor(((x - PAD.l) / (r.width - PAD.l - PAD.r)) * pc.departures.length)
    const j = Math.floor(((r.height - PAD.b - y) / (r.height - PAD.t - PAD.b)) * pc.tofs_days.length)
    return i >= 0 && j >= 0 && i < pc.departures.length && j < pc.tofs_days.length ? { i, j, x, y } : null
  }
  const hv = hover ? value(hover.i, hover.j) : null
  return (
    <div className="canvas-wrap porkchop" style={{ height: 340, marginTop: 8 }}>
      <canvas
        ref={ref}
        onMouseMove={(e) => setHover(at(e))}
        onMouseLeave={() => setHover(null)}
        onClick={(e) => {
          const p = at(e)
          const c = p && pc.c3[p.j]?.[p.i]
          const a = p && pc.vinf_arr[p.j]?.[p.i]
          if (p && c != null && a != null) onPick({ dep: pc.departures[p.i], tof: pc.tofs_days[p.j], c3: c, vinf: a })
        }}
        style={{ cursor: 'crosshair' }}
        role="img"
        aria-label={t('Porkchop: janelas de lançamento')}
      />
      {hover && hv != null && (
        <div className="chart-tip" style={{ left: Math.min(hover.x + 14, 9999), top: hover.y + 10 }}>
          <b>{day(pc.departures[hover.i])}</b> · {num(pc.tofs_days[hover.j], 0)} {t('dias')}
          <br />
          C3 {num(pc.c3[hover.j][hover.i], 1)} · v∞ {num(pc.vinf_arr[hover.j][hover.i], 2)} km/s
          {depart && (
            <>
              <br />
              Δv {t('injeção')} {num(injectionDv(depart, pc.c3[hover.j][hover.i]!), 0)} m/s{over(hover.i, hover.j) ? ` · ${t('não cabe')}` : ''}
            </>
          )}
        </div>
      )}
      {depart?.budget != null && <div className="porkchop-legend small">▨ {t('além do Δv da sonda ({dv} m/s)', { dv: num(depart.budget, 0) })}</div>}
    </div>
  )
}

/** O Sistema Solar de cima: órbitas, planetas, a sonda e a transferência escolhida. Roda aproxima, arrastar move, duplo clique volta ao Sol. */
function SolarSystem({ path, probe, from, to, pick, flyby }: { path: [number, number][]; probe: V3 | null; from: string; to: string; pick: Pick | null; flyby: FlybyPick | null }) {
  const fly = useMemo(() => (flyby ? flybyArcs(flyby) : null), [flyby])
  const snap = useStore((s) => s.snap)!
  const [zoom, setZoom] = useState(2.2)
  const [center, setCenter] = useState<'sun' | 'probe' | string>('sun')
  const [pan, setPan] = useState<[number, number]>([0, 0])
  const drag = useRef<{ x: number; y: number; pan: [number, number] } | null>(null)
  // A transferência do porkchop: Lambert entre as posições na partida e na chegada.
  const transfer = useMemo(() => {
    if (!pick) return null
    const t2 = pick.dep + pick.tof * DAY
    const r1 = planetPosition(from, pick.dep)
    const r2 = planetPosition(to, t2)
    if (!r1 || !r2) return null
    const sol = lambert(r1, r2, pick.tof * DAY)
    return { r1, r2, arc: sol ? transferArc(r1, sol[0], r2) : [], t2 }
  }, [pick, from, to])
  const ref = useCanvas(
    (ctx, w, h) => {
      ctx.fillStyle = '#050914'
      ctx.fillRect(0, 0, w, h)
      // Centro da vista: o Sol, a sonda ou um planeta, mais o arrasto.
      let c: [number, number] = [0, 0]
      if (center === 'probe' && probe) c = ecl(probe[0], probe[1], probe[2])
      else if (center !== 'sun') {
        const b = snap.bodies.find((x) => x.id === center)
        if (b) c = ecl(b.r[0], b.r[1], b.r[2])
      }
      const s = Math.min(w, h) / 2 / (zoom * AU)
      const X = (x: number) => w / 2 + (x - c[0]) * s + pan[0]
      const Y = (y: number) => h / 2 - (y - c[1]) * s + pan[1]
      // Órbitas (elementos médios), a de origem e a de destino em destaque.
      for (const id of PLANETS) {
        const pts = orbitPath(id, snap.t)
        const hl = id === from || id === to
        ctx.strokeStyle = hl ? `${COLORS[id]}aa` : '#1d2945'
        ctx.lineWidth = hl ? 1.5 : 1
        ctx.beginPath()
        pts.forEach(([x, y], i) => (i ? ctx.lineTo(X(x), Y(y)) : ctx.moveTo(X(x), Y(y))))
        ctx.stroke()
      }
      // Sol com brilho.
      const g = ctx.createRadialGradient(X(0), Y(0), 0, X(0), Y(0), 22)
      g.addColorStop(0, '#fff6c0')
      g.addColorStop(0.25, '#ffcc33')
      g.addColorStop(1, '#ffcc3300')
      ctx.fillStyle = g
      ctx.beginPath()
      ctx.arc(X(0), Y(0), 22, 0, Math.PI * 2)
      ctx.fill()
      // Transferência escolhida: o arco, a partida e onde o destino estará na chegada.
      if (transfer && transfer.arc.length > 1) {
        ctx.strokeStyle = '#ff8a3d'
        ctx.lineWidth = 2
        ctx.setLineDash([6, 4])
        ctx.beginPath()
        transfer.arc.forEach(([x, y], i) => (i ? ctx.lineTo(X(x), Y(y)) : ctx.moveTo(X(x), Y(y))))
        ctx.stroke()
        ctx.setLineDash([])
        ctx.font = '11px ui-monospace, monospace'
        for (const [p, label, color] of [
          [transfer.r1, `${t('partida')} ${day(pick!.dep)}`, COLORS[from]],
          [transfer.r2, `${t('chegada')} ${day(transfer.t2)}`, COLORS[to]],
        ] as [V3, string, string][]) {
          ctx.strokeStyle = color
          ctx.lineWidth = 1.5
          ctx.beginPath()
          ctx.arc(X(p[0]), Y(p[1]), 7, 0, Math.PI * 2)
          ctx.stroke()
          ctx.fillStyle = '#ffb27a'
          ctx.fillText(label, X(p[0]) + 10, Y(p[1]) + 14)
        }
      }
      // Trajetória com sobrevoo: os dois trechos e as três passagens.
      if (fly && flyby) {
        ctx.lineWidth = 2
        for (const [arc, color] of [
          [fly.leg1, '#c084fc'],
          [fly.leg2, '#f472b6'],
        ] as [[number, number][], string][]) {
          ctx.strokeStyle = color
          ctx.beginPath()
          arc.forEach(([x, y], i) => (i ? ctx.lineTo(X(x), Y(y)) : ctx.moveTo(X(x), Y(y))))
          ctx.stroke()
        }
        ctx.font = '11px ui-monospace, monospace'
        for (const [p, label, color] of [
          [fly.r0, `${t('partida')} ${day(flyby.depart)}`, COLORS[flyby.from]],
          [fly.rf, `${t('sobrevoo')} ${day(flyby.flyby)}`, COLORS[flyby.via]],
          [fly.ra, `${t('chegada')} ${day(flyby.arrive)}`, COLORS[flyby.to]],
        ] as [V3, string, string][]) {
          ctx.strokeStyle = color
          ctx.lineWidth = 1.5
          ctx.beginPath()
          ctx.arc(X(p[0]), Y(p[1]), 7, 0, Math.PI * 2)
          ctx.stroke()
          ctx.fillStyle = '#f0b8ff'
          ctx.fillText(label, X(p[0]) + 10, Y(p[1]) - 10)
        }
      }
      // Planetas (e a Lua, de perto).
      ctx.font = '11px system-ui'
      for (const b of snap.bodies) {
        if (b.id === 'sun' || (b.id === 'moon' && zoom > 0.05)) continue
        const [bx, by] = ecl(b.r[0], b.r[1], b.r[2])
        ctx.fillStyle = COLORS[b.id] ?? '#ccc'
        ctx.beginPath()
        ctx.arc(X(bx), Y(by), RADIUS[b.id] ?? 4, 0, Math.PI * 2)
        ctx.fill()
        if (b.id === 'saturn') {
          ctx.strokeStyle = '#e6d39a99'
          ctx.beginPath()
          ctx.ellipse(X(bx), Y(by), 11, 4, -0.4, 0, Math.PI * 2)
          ctx.stroke()
        }
        ctx.fillStyle = b.id === from || b.id === to ? '#e8eefc' : '#9fb0d0'
        ctx.fillText(bodyName(b.id), X(bx) + 8, Y(by) - 6)
      }
      if (path.length > 1) {
        ctx.strokeStyle = '#2dd4bf'
        ctx.lineWidth = 1.5
        ctx.beginPath()
        path.forEach(([x, y], i) => (i ? ctx.lineTo(X(x), Y(y)) : ctx.moveTo(X(x), Y(y))))
        ctx.stroke()
      }
      if (probe) {
        const [px, py] = ecl(probe[0], probe[1], probe[2])
        ctx.fillStyle = '#fff'
        ctx.fillRect(X(px) - 3, Y(py) - 3, 6, 6)
      }
      // Régua e data.
      const target = (w * 0.18) / s
      const p10 = 10 ** Math.floor(Math.log10(target / AU))
      const bar = [1, 2, 5].map((m) => m * p10).filter((m) => m * AU <= target).pop() ?? p10
      ctx.strokeStyle = '#8494b3'
      ctx.fillStyle = '#8494b3'
      ctx.lineWidth = 1
      ctx.beginPath()
      ctx.moveTo(12, h - 14)
      ctx.lineTo(12 + bar * AU * s, h - 14)
      ctx.moveTo(12, h - 18)
      ctx.lineTo(12, h - 10)
      ctx.moveTo(12 + bar * AU * s, h - 18)
      ctx.lineTo(12 + bar * AU * s, h - 10)
      ctx.stroke()
      ctx.font = '11px ui-monospace, monospace'
      ctx.fillText(`${bar < 0.01 ? num(bar * AU / 1e3, 0) + ' km' : `${num(bar, bar < 1 ? 2 : 0)} ${t('UA')}`}`, 12, h - 22)
      ctx.textAlign = 'right'
      ctx.fillText(day(snap.t), w - 10, h - 10)
      ctx.textAlign = 'left'
    },
    [snap, path, probe, zoom, center, pan, from, to, transfer, fly, flyby],
  )
  const reset = () => {
    setPan([0, 0])
    setCenter('sun')
  }
  return (
    <div style={{ position: 'relative' }}>
      <div className="canvas-wrap" style={{ height: 440 }}>
        <canvas
          ref={ref}
          role="img"
          aria-label={t('Sistema Solar (vista de cima)')}
          className="pannable"
          onWheel={(e) => setZoom((z) => Math.min(60, Math.max(0.002, z * (e.deltaY > 0 ? 1.15 : 1 / 1.15))))}
          onPointerDown={(e) => {
            drag.current = { x: e.clientX, y: e.clientY, pan }
            e.currentTarget.setPointerCapture(e.pointerId)
          }}
          onPointerMove={(e) => {
            const d = drag.current
            if (d) setPan([d.pan[0] + e.clientX - d.x, d.pan[1] + e.clientY - d.y])
          }}
          onPointerUp={() => (drag.current = null)}
          onDoubleClick={reset}
        />
      </div>
      <div className="row solar-tools">
        {[0.01, 1.2, 2.2, 6, 32].map((z) => (
          <button key={z} className={`small ${zoom === z ? 'active' : ''}`} onClick={() => setZoom(z)}>
            {z} {t('UA')}
          </button>
        ))}
        <select
          value={center}
          onChange={(e) => {
            setCenter(e.target.value)
            setPan([0, 0])
          }}
          aria-label={t('Centro da vista')}
        >
          <option value="sun">{t('centro')}: {bodyName('sun')}</option>
          {probe && <option value="probe">{t('centro')}: {t('sonda')}</option>}
          {PLANETS.map((p) => (
            <option key={p} value={p}>
              {t('centro')}: {bodyName(p)}
            </option>
          ))}
        </select>
      </div>
      <div className="solar-hint small muted">{t('roda: zoom · arrastar: mover · duplo clique: voltar')}</div>
    </div>
  )
}
