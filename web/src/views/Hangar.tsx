// Hangar (VAB): montar o foguete com peças — estágios com tanque e motores,
// boosters laterais e a carga — vendo a nave em 3D, o Δv e o empuxo de cada
// estágio, e montar a missão (de onde lança e para que órbita, ou já em
// órbita). Salva os arquivos de veículo e de cenário e lança.

import { Suspense, lazy, useEffect, useMemo, useState } from 'react'
import { Chart } from '../components/Chart'
import { Panel, Tile, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { duration, num } from '../lib/format'
import {
  DEFAULT_DESIGN,
  DEFAULT_MISSION,
  ENGINES,
  PAYLOADS,
  PROPELLANTS,
  computeStats,
  engine,
  engineSheet,
  launcherCost,
  fileId,
  parseHangar,
  payload,
  previewVehicle,
  scenarioToml,
  stageMasses,
  vehicleToml,
  type Design,
  type Mission,
  type PropellantId,
  type StageDesign,
} from '../lib/hangar'
import { canSim } from '../lib/room'
import { store, useStore } from '../lib/store'

const Vehicle3D = lazy(() => import('../components/Vehicle3D').then((m) => ({ default: m.Vehicle3D })))

/** Modelos prontos para começar. */
const PRESETS: [string, Design][] = [
  ['Lançador leve', DEFAULT_DESIGN],
  [
    'Lançador médio',
    {
      name: 'Lançador médio',
      payload: 'satellite',
      payloadMass: 4000,
      crew: 0,
      stages: [
        { boosters: false, propellant: 'kerolox', diameter: 3.66, length: 41, engine: 'k900', count: 9 },
        { boosters: false, propellant: 'kerolox', diameter: 3.66, length: 13.8, engine: 'k980v', count: 1 },
      ],
    },
  ],
  [
    'Pesado com boosters e cápsula',
    {
      name: 'Pesado tripulado',
      payload: 'capsule',
      payloadMass: 9000,
      crew: 3,
      stages: [
        { boosters: true, propellant: 'solid', diameter: 1.8, length: 16, engine: 's1000', count: 4 },
        { boosters: false, propellant: 'kerolox', diameter: 3.7, length: 38, engine: 'k900', count: 9 },
        { boosters: false, propellant: 'kerolox', diameter: 3.7, length: 11, engine: 'k980v', count: 1 },
      ],
    },
  ],
  [
    'Hidrogênio com sonda',
    {
      name: 'Lançador a hidrogênio',
      payload: 'probe',
      payloadMass: 1000,
      crew: 0,
      stages: [
        { boosters: true, propellant: 'solid', diameter: 3, length: 25, engine: 's5000', count: 2 },
        { boosters: false, propellant: 'hydrolox', diameter: 5.4, length: 30, engine: 'h1860', count: 1 },
        { boosters: false, propellant: 'hydrolox', diameter: 4, length: 9, engine: 'h110v', count: 1 },
      ],
    },
  ],
]

/** O que o teste de voo do servidor devolve (sim/src/flight_test.rs). */
interface FlightReport {
  ok: boolean
  result: string
  pe_km: number
  ap_km: number
  inc_deg: number
  max_q_kpa: number
  max_q_t: number
  max_g: number
  time_to_orbit: number | null
  dv_left: number
  events: [number, string, string][]
  track: { t: number; alt_km: number; downrange_km: number; speed: number; q_kpa: number; g: number }[]
}

/** Δv aproximado da plataforma até cada destino (m/s), para comparar. */
const DESTINATIONS: [string, number][] = [
  ['Órbita baixa', 9_400],
  ['Órbita de transferência geoestacionária', 11_900],
  ['Injeção para a Lua', 12_500],
  ['Injeção para Marte', 13_000],
]

function load(): { design: Design; mission: Mission } {
  try {
    const o = JSON.parse(localStorage.getItem('jaci.hangar') ?? 'null')
    if (o?.design?.stages) return { design: { ...DEFAULT_DESIGN, ...o.design }, mission: { ...DEFAULT_MISSION, ...o.mission } }
  } catch {
    /* sem armazenamento */
  }
  return { design: DEFAULT_DESIGN, mission: DEFAULT_MISSION }
}

export function Hangar() {
  // O projeto e o histórico para desfazer e refazer (as mudanças de um arrasto contam como uma).
  type S = ReturnType<typeof load>
  const [h, setH] = useState(() => ({ cur: load(), past: [] as S[], future: [] as S[], at: 0 }))
  const state = h.cur
  const { design: d, mission: m } = state
  const setState = (next: S | ((s: S) => S)) =>
    setH((p) => {
      const now = Date.now()
      const cur = typeof next === 'function' ? next(p.cur) : next
      const merge = now - p.at < 600
      return { cur, past: merge ? p.past : [...p.past.slice(-80), p.cur], future: merge ? p.future : [], at: now }
    })
  const undo = () => setH((p) => (p.past.length ? { cur: p.past[p.past.length - 1], past: p.past.slice(0, -1), future: [...p.future, p.cur], at: 0 } : p))
  const redo = () => setH((p) => (p.future.length ? { cur: p.future[p.future.length - 1], past: [...p.past, p.cur], future: p.future.slice(0, -1), at: 0 } : p))
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null
      if (!(e.ctrlKey || e.metaKey) || e.key.toLowerCase() !== 'z' || (el && ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName))) return
      e.preventDefault()
      if (e.shiftKey) redo()
      else undo()
    }
    window.addEventListener('keydown', key)
    return () => window.removeEventListener('keydown', key)
  })
  const [hover, setHover] = useState<number | null>(null)
  const [test, setTest] = useState<FlightReport | null>(null)
  const [testing, setTesting] = useState(false)
  const runTest = async () => {
    setTesting(true)
    setTest(null)
    try {
      const r = await store.request<{ ok: boolean; data?: FlightReport; error?: string }>({ type: 'hangar_test', text: vehicleToml(d, m), site: m.site }, 180000)
      if (!r.ok || !r.data) throw new Error(tx(r.error))
      setTest(r.data)
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'bad')
    }
    setTesting(false)
  }
  const sites = useStore((s) => s.hello?.sites ?? [])
  const [files, setFiles] = useState<{ sub: string; id: string; name: string | null }[]>([])
  const [busy, setBusy] = useState(false)
  const stats = useMemo(() => computeStats(d), [d])
  const preview = useMemo(() => previewVehicle(d), [d])
  const setD = (patch: Partial<Design>) => setState((s) => ({ ...s, design: { ...s.design, ...patch } }))
  const setM = (patch: Partial<Mission>) => setState((s) => ({ ...s, mission: { ...s.mission, ...patch } }))
  useEffect(() => {
    try {
      localStorage.setItem('jaci.hangar', JSON.stringify(state))
    } catch {
      /* sem armazenamento */
    }
  }, [state])
  const refresh = () =>
    store
      .request<{ list?: { sub: string; id: string; name: string | null }[] }>({ type: 'sim', action: 'files' })
      .then((r) => setFiles((r.list ?? []).filter((f) => f.sub === 'veiculos' && f.name)))
      .catch(() => {})
  useEffect(() => {
    refresh()
  }, [])
  const setStage = (i: number, patch: Partial<StageDesign>) => setD({ stages: d.stages.map((s, k) => (k === i ? { ...s, ...patch } : s)) })
  const duplicate = (i: number) => {
    if (d.stages[i].boosters) return
    const st = [...d.stages]
    st.splice(i + 1, 0, { ...d.stages[i] })
    setD({ stages: st })
  }
  const move = (i: number, dir: number) => {
    const j = i + dir
    if (j < 0 || j >= d.stages.length) return
    const st = [...d.stages]
    ;[st[i], st[j]] = [st[j], st[i]]
    setD({ stages: st })
  }
  const addStage = () => {
    const top = d.stages[d.stages.length - 1]
    setD({ stages: [...d.stages, { boosters: false, propellant: top?.propellant === 'solid' ? 'kerolox' : (top?.propellant ?? 'kerolox'), diameter: top?.diameter ?? 1.2, length: Math.max(2, (top?.length ?? 6) / 3), engine: top?.propellant === 'hydrolox' ? 'h110v' : 'k25v', count: 1 }] })
  }
  const addBoosters = () => {
    if (d.stages[0]?.boosters) return
    const core = d.stages[0]
    setD({ stages: [{ boosters: true, propellant: 'solid', diameter: Math.max(0.6, (core?.diameter ?? 2) * 0.5), length: (core?.length ?? 15) * 0.6, engine: 's1000', count: 2 }, ...d.stages] })
  }
  const id = fileId(d.name)
  const allowed = canSim('file_save')
  const save = async (launch: boolean) => {
    setBusy(true)
    try {
      const v = (await store.sim('file_save', { sub: 'veiculos', id, text: vehicleToml(d, m) })) as { ok?: boolean; error?: string }
      if (!v.ok) throw new Error(tx(v.error))
      const sc = (await store.sim('file_save', { sub: 'cenarios', id: `${id}-missao`, text: scenarioToml(d, m, id) })) as { ok?: boolean; error?: string }
      if (!sc.ok) throw new Error(tx(sc.error))
      refresh()
      if (launch) {
        // Carregar um cenário recomeça a conexão (a resposta é um "reload"): só um erro volta como resposta.
        store
          .sim('scenario', { value: `${id}-missao` })
          .then((r) => r.ok === false && toast(tx(r.error), 'bad'))
          .catch(() => {})
        toast(m.mode === 'pad' ? t('Na plataforma: comece a contagem no LCC.') : t('Em órbita!'), 'good')
        store.setUi({ view: m.mode === 'pad' ? 'launch' : 'manual' })
      } else toast(t('Salvo: veiculos/{v}.toml e cenarios/{v}-missao.toml', { v: id }), 'good')
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'bad')
    }
    setBusy(false)
  }
  const open = async (fid: string) => {
    const r = await store.request<{ ok?: boolean; text?: string }>({ type: 'sim', action: 'file_get', sub: 'veiculos', id: fid })
    const h = r.ok && r.text ? parseHangar(r.text) : null
    if (h) setState(h)
    else toast(t('Esse veículo não foi montado no hangar (edite no console do Instrutor).'), 'info')
  }
  const pl = payload(d.payload)
  const best = DESTINATIONS.filter(([, need]) => stats.dv >= need).pop()
  return (
    <div className="hangar">
      <Panel
        title={t('Hangar: montagem do foguete')}
        right={
          <div className="row">
            <button className="small" onClick={undo} disabled={!h.past.length} title={`${t('Desfazer')} (Ctrl+Z)`}>
              ↶
            </button>
            <button className="small" onClick={redo} disabled={!h.future.length} title={`${t('Refazer')} (Ctrl+Shift+Z)`}>
              ↷
            </button>
            <select value="" onChange={(e) => e.target.value && setState({ design: PRESETS[Number(e.target.value)][1], mission: m })} aria-label={t('Modelos')}>
              <option value="">{t('Modelos…')}</option>
              {PRESETS.map(([n], i) => (
                <option key={n} value={i}>
                  {t(n)}
                </option>
              ))}
            </select>
            <select value="" onChange={(e) => e.target.value && open(e.target.value)} aria-label={t('Abrir')}>
              <option value="">{t('Abrir…')}</option>
              {files.map((f) => (
                <option key={f.id} value={f.id}>
                  {f.name} ({f.id})
                </option>
              ))}
            </select>
          </div>
        }
      >
        <div className="hangar-grid">
          <div className="hangar-parts">
            <label className="field">
              <span>{t('Nome')}</span>
              <input value={d.name} onChange={(e) => setD({ name: e.target.value })} />
            </label>
            {/* A carga no alto, depois os estágios de cima para baixo (como o foguete em pé). */}
            <div className="part payload" onMouseEnter={() => setHover(d.stages.length)} onMouseLeave={() => setHover(null)}>
              <div className="part-head">
                <b>{t('Carga')}</b>
                <select value={d.payload} onChange={(e) => setD({ payload: e.target.value as Design['payload'], payloadMass: payload(e.target.value).mass, crew: payload(e.target.value).crew ? 3 : 0 })}>
                  {PAYLOADS.map((p) => (
                    <option key={p.id} value={p.id}>
                      {t(p.name)}
                    </option>
                  ))}
                </select>
              </div>
              <div className="small muted">{t(pl.description)}</div>
              <div className="row">
                {pl.adjustable && (
                  <label className="small">
                    {t('Massa')}{' '}
                    <input type="number" min={10} step={10} value={d.payloadMass} onChange={(e) => setD({ payloadMass: Math.max(10, Number(e.target.value)) })} style={{ width: 80 }} /> kg
                  </label>
                )}
                {pl.crew && (
                  <label className="small">
                    {t('Tripulação')}{' '}
                    <select value={d.crew} onChange={(e) => setD({ crew: Number(e.target.value) })}>
                      {[1, 2, 3, 4].map((n) => (
                        <option key={n}>{n}</option>
                      ))}
                    </select>
                  </label>
                )}
              </div>
            </div>
            {d.stages
              .map((s, i) => ({ s, i }))
              .reverse()
              .map(({ s, i }) => (
                <StageCard key={i} s={s} i={i} n={d.stages.length} onHover={(on) => setHover(on ? i : null)} onDuplicate={() => duplicate(i)} onChange={(p) => setStage(i, p)} onMove={(dir) => move(i, dir)} onRemove={() => setD({ stages: d.stages.filter((_, k) => k !== i) })} label={s.boosters ? t('Boosters') : `${i + 1 - (d.stages[0]?.boosters ? 1 : 0)}º ${t('estágio')}`} />
              ))}
            <div className="row">
              <button className="small" onClick={addStage}>
                + {t('Estágio em cima')}
              </button>
              <button className="small" onClick={addBoosters} disabled={!!d.stages[0]?.boosters || !d.stages.length}>
                + {t('Boosters')}
              </button>
            </div>
          </div>
          <div className="hangar-view">
            <Suspense fallback={<div className="loading muted">{t('Carregando a vista 3D…')}</div>}>
              <Vehicle3D v={preview} height={560} highlight={hover} />
            </Suspense>
          </div>
          <div className="hangar-stats">
            <div className="tiles">
              <Tile label="Δv" value={num(stats.dv, 0)} unit="m/s" state={stats.dv >= 9_400 ? 'nominal' : 'yellow'} />
              <Tile label={t('TWR na decolagem')} value={num(stats.liftoffTwr, 2)} state={stats.liftoffTwr < 1.15 ? 'red' : 'nominal'} />
              <Tile label={t('Massa na decolagem')} value={num(stats.liftoff / 1000, 1)} unit="t" />
              <Tile label={t('Carga e coifa')} value={num(stats.payload / 1000, 2)} unit="t" />
              <Tile label={t('Custo do lançador')} value={`≈ ${num(launcherCost(d), 0)}`} unit={t('mi US$')} title={t('Motores, estrutura e propelente; ordem de grandeza')} />
            </div>
            <table className="dv-table">
              <thead>
                <tr>
                  <th>{t('Fase')}</th>
                  <th className="num">Δv</th>
                  <th className="num">TWR</th>
                  <th className="num">{t('Queima')}</th>
                </tr>
              </thead>
              <tbody>
                {stats.stages.map((s, k) => (
                  <tr key={k}>
                    <td>{tx(s.name)}</td>
                    <td className="num">{num(s.dv, 0)}</td>
                    <td className={`num ${s.index >= 0 && s.twr < (k === 0 ? 1.15 : 0.3) ? 'bad' : ''}`}>{num(s.twr, 2)}</td>
                    <td className="num">{num(s.burn, 0)} s</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div className="dv-bar" title={t('Δv do foguete comparado com o que cada destino pede')}>
              <div className="fill" style={{ width: `${Math.min(100, (stats.dv / 14_000) * 100)}%` }} />
              {DESTINATIONS.map(([n, need]) => (
                <span key={n} className="mark" style={{ left: `${(need / 14_000) * 100}%` }} title={`${t(n)}: ${num(need, 0)} m/s`} />
              ))}
            </div>
            <div className="small">{best ? t('Chega a: {d}', { d: t(best[0]) }) : t('Ainda não chega à órbita (≈ 9.400 m/s da plataforma).')}</div>
            {stats.warnings.map((w) => (
              <div key={w} className="banner warn small">
                {t(w)}
              </div>
            ))}
          </div>
        </div>
      </Panel>
      <Panel title={t('Missão')}>
        <div className="hangar-mission">
          <div className="seg">
            <button className={`small ${m.mode === 'pad' ? 'active' : ''}`} onClick={() => setM({ mode: 'pad' })}>
              {t('Lançar da plataforma')}
            </button>
            <button className={`small ${m.mode === 'orbit' ? 'active' : ''}`} onClick={() => setM({ mode: 'orbit' })}>
              {t('Começar em órbita')}
            </button>
          </div>
          {m.mode === 'pad' ? (
            <>
              <label className="small">
                {t('Local')}{' '}
                <select value={m.site} onChange={(e) => setM({ site: e.target.value })}>
                  {sites.map((s) => (
                    <option key={s.name} value={s.name}>
                      {s.name} ({num(s.lat, 1)}°)
                    </option>
                  ))}
                </select>
              </label>
              <label className="small">
                T− <input type="number" min={30} max={3600} value={m.tMinus} onChange={(e) => setM({ tMinus: Number(e.target.value) })} style={{ width: 70 }} /> s
              </label>
              <label className="small">
                <input type="checkbox" checked={m.calm} onChange={(e) => setM({ calm: e.target.checked })} /> {t('Tempo bom')}
              </label>
            </>
          ) : (
            <label className="small">
              {t('Corpo')}{' '}
              <select value={m.body} onChange={(e) => setM({ body: e.target.value })}>
                {['earth', 'moon', 'mars'].map((b) => (
                  <option key={b} value={b}>
                    {tx(store.hello?.bodies.find((x) => x.id === b)?.name ?? b)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label className="small">
            {m.mode === 'pad' ? t('Órbita alvo') : t('Órbita')}: Pe <input type="number" min={80} value={m.pe} onChange={(e) => setM({ pe: Number(e.target.value) })} style={{ width: 70 }} /> × Ap{' '}
            <input type="number" min={80} value={m.ap} onChange={(e) => setM({ ap: Number(e.target.value) })} style={{ width: 80 }} /> km
          </label>
          <label className="small">
            {t('Inclinação')} <input type="number" min={0} max={180} step={0.1} value={m.inc} onChange={(e) => setM({ inc: Number(e.target.value) })} style={{ width: 70 }} />°
          </label>
          {m.mode === 'pad' && (
            <label className="small">
              <input type="checkbox" checked={m.north} onChange={(e) => setM({ north: e.target.checked })} /> {t('Rumo norte (nas órbitas inclinadas)')}
            </label>
          )}
          <div className="row" style={{ marginLeft: 'auto' }}>
            <span className="small muted mono">
              veiculos/{id}.toml · cenarios/{id}-missao.toml
            </span>
            {m.mode === 'pad' && (
              <button onClick={runTest} disabled={testing || !d.stages.length} title={t('Voa o foguete no simulador, num mundo à parte, sem salvar nada')}>
                {testing ? t('Voando…') : `🧪 ${t('Testar voo')}`}
              </button>
            )}
            <button onClick={() => save(false)} disabled={busy || !allowed || !d.stages.length}>
              {t('Salvar')}
            </button>
            <button className="primary" onClick={() => save(true)} disabled={busy || !allowed || !d.stages.length} title={allowed ? '' : t('Na sala com posições, só o instrutor (SIM) carrega missões')}>
              🚀 {t('Salvar e lançar')}
            </button>
          </div>
        </div>
      </Panel>
      {test && <TestReport r={test} onClose={() => setTest(null)} />}
    </div>
  )
}

/** Resultado do teste de voo: o veredito, os números, a trajetória e os eventos. */
function TestReport({ r, onClose }: { r: FlightReport; onClose: () => void }) {
  const tr = r.track
  const series = (label: string, color: string, f: (p: FlightReport['track'][number]) => number) => [{ label, color, t: tr.map((p) => p.t), v: tr.map(f) }]
  return (
    <Panel
      title={t('Teste de voo')}
      right={
        <button className="small" onClick={onClose}>
          ×
        </button>
      }
    >
      <div className={`banner ${r.ok ? 'good' : 'alarm'}`}>{tx(r.result)}</div>
      <div className="tiles" style={{ marginTop: 8 }}>
        <Tile label={t('Órbita')} value={r.pe_km > -6000 ? `${num(r.pe_km, 0)} × ${num(r.ap_km, 0)}` : '—'} unit="km" state={r.ok ? 'nominal' : 'red'} />
        <Tile label={t('Inclinação')} value={num(r.inc_deg, 1)} unit="°" />
        <Tile label={t('Tempo até a órbita')} value={r.time_to_orbit != null ? duration(r.time_to_orbit) : '—'} />
        <Tile label={t('Pressão dinâmica máxima')} value={num(r.max_q_kpa, 1)} unit="kPa" title={`T+${num(r.max_q_t, 0)} s`} state={r.max_q_kpa > 45 ? 'yellow' : 'nominal'} />
        <Tile label={t('Carga g máxima')} value={num(r.max_g, 1)} unit="g" state={r.max_g > 6 ? 'yellow' : 'nominal'} />
        <Tile label={t('Δv que sobrou')} value={num(r.dv_left, 0)} unit="m/s" />
      </div>
      {tr.length > 2 && (
        <div className="grid cols-3" style={{ marginTop: 8 }}>
          <Chart series={series(t('Altitude (km)'), '#4cc3ff', (p) => p.alt_km)} height={170} t0={0} />
          <Chart series={series(t('Velocidade (m/s)'), '#3ddc84', (p) => p.speed)} height={170} t0={0} />
          <Chart series={[...series('q (kPa)', '#ffc23d', (p) => p.q_kpa), ...series(t('Carga (g)'), '#ff8a3d', (p) => p.g)]} height={170} t0={0} />
        </div>
      )}
      <div className="test-events">
        {r.events
          .filter(([tt, lvl]) => lvl !== 'Chat' && tt >= -10)
          .map(([tt, lvl, text], i) => (
            <div key={i} className={`tev ${String(lvl).toLowerCase()}`}>
              <span className="mono muted">{tt < 0 ? `T${num(tt, 0)}` : `T+${num(tt, 0)}`} s</span>
              <span>{tx(text)}</span>
            </div>
          ))}
      </div>
    </Panel>
  )
}

function StageCard({
  s,
  i,
  n,
  label,
  onChange,
  onMove,
  onRemove,
  onHover,
  onDuplicate,
}: {
  s: StageDesign
  i: number
  n: number
  label: string
  onChange: (p: Partial<StageDesign>) => void
  onMove: (dir: number) => void
  onRemove: () => void
  onHover: (on: boolean) => void
  onDuplicate: () => void
}) {
  const ms = stageMasses(s)
  const engines = ENGINES.filter((e) => e.propellant === s.propellant)
  const setProp = (p: PropellantId) => {
    const first = ENGINES.find((e) => e.propellant === p && (e.vacuum === (i > 0) || p === 'solid')) ?? ENGINES.find((e) => e.propellant === p)!
    onChange({ propellant: p, engine: first.id })
  }
  return (
    <div className={`part ${s.boosters ? 'boosters' : ''}`} onMouseEnter={() => onHover(true)} onMouseLeave={() => onHover(false)}>
      <div className="part-head">
        <b>{label}</b>
        <span className="row">
          <button className="small" onClick={() => onMove(1)} disabled={i >= n - 1 || s.boosters} title={t('Subir')}>
            ↑
          </button>
          <button className="small" onClick={() => onMove(-1)} disabled={i === 0 || s.boosters} title={t('Descer')}>
            ↓
          </button>
          {!s.boosters && (
            <button className="small" onClick={onDuplicate} title={t('Duplicar')}>
              ⧉
            </button>
          )}
          <button className="small" onClick={onRemove} title={t('Tirar')}>
            ×
          </button>
        </span>
      </div>
      <div className="part-grid">
        <label>
          {t('Propelente')}
          <select value={s.propellant} onChange={(e) => setProp(e.target.value as PropellantId)}>
            {(Object.keys(PROPELLANTS) as PropellantId[]).map((p) => (
              <option key={p} value={p}>
                {t(PROPELLANTS[p].name)}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t('Motor')}
          <select value={s.engine} onChange={(e) => onChange({ engine: e.target.value })}>
            {engines.map((e) => (
              <option key={e.id} value={e.id}>
                {t(e.name)} · Isp {e.ispVac} s
              </option>
            ))}
          </select>
        </label>
        <label>
          {s.boosters ? t('Boosters') : t('Motores')}
          <input type="number" min={s.boosters ? 2 : 1} max={s.boosters ? 8 : 33} value={s.count} onChange={(e) => onChange({ count: Math.max(s.boosters ? 2 : 1, Math.round(Number(e.target.value))) })} />
        </label>
        <label>
          {t('Diâmetro')} <b className="mono">{num(s.diameter, 2)} m</b>
          <input type="range" min={0.5} max={10} step={0.05} value={s.diameter} onChange={(e) => onChange({ diameter: Number(e.target.value) })} />
        </label>
        <label>
          {t('Comprimento')} <b className="mono">{num(s.length, 1)} m</b>
          <input type="range" min={1} max={70} step={0.1} value={s.length} onChange={(e) => onChange({ length: Number(e.target.value) })} />
        </label>
      </div>
      <div className="small engine-sheet">{engineSheet(s.engine)}</div>
      <div className="small muted">
        {num(ms.prop / 1000, 1)} t {t('de propelente')} · {num(ms.dry / 1000, 2)} t {t('seco')} · {num(ms.thrust, 0)} kN ({engine(s.engine).vacuum ? t('vácuo') : t('nível do mar')})
      </div>
    </div>
  )
}
