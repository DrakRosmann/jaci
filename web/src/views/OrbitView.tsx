import { useState } from 'react'
import { Orbit3D } from '../components/Orbit3D'
import { CmdButton, Empty, Panel, Tm, sendCommand, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { duration, latlon, num } from '../lib/format'
import { usePrediction } from '../lib/hooks'
import { store, useStore } from '../lib/store'
import type { Plan, Vehicle } from '../types'

export function OrbitView() {
  const v = useStore((s) => s.selected)
  const span = orbitSpan(v)
  const pred = usePrediction(v && v.phase !== 'prelaunch' && v.kind !== 'external' ? v.id : null, span, 3000, span / 360)
  if (!v || !v.tm) return <Empty>{t('Selecione um veículo com telemetria.')}</Empty>
  return (
    <div className="grid">
      <div className="grid cols-3">
        <Panel title={`${t('Órbita')} · ${v.name} · ${v.central}`} className="span-2" flush>
          <Orbit3D vehicle={v.id} prediction={pred.points} height={440} />
        </Panel>
        <Panel title={t('Elementos orbitais')}>
          <div className="tiles">
            <Tm v={v} id="orb.pe" digits={1} />
            <Tm v={v} id="orb.ap" digits={1} />
            <Tm v={v} id="orb.sma" digits={1} />
            <Tm v={v} id="orb.ecc" digits={5} />
            <Tm v={v} id="orb.inc" digits={3} />
            <Tm v={v} id="orb.raan" digits={2} />
            <Tm v={v} id="orb.argp" digits={2} />
            <Tm v={v} id="orb.ta" digits={2} />
            <Tm v={v} id="orb.period" digits={2} />
            <Tm v={v} id="nav.speed" digits={1} />
            <Tm v={v} id="nav.dv" digits={0} />
            <Tm v={v} id="nav.mass" digits={3} />
          </div>
          <AttitudeButtons v={v} />
        </Panel>
      </div>
      <div className="grid cols-3">
        <Planner v={v} />
        <NodeQueue v={v} />
        <ManualBurn v={v} />
      </div>
    </div>
  )
}

export function orbitSpan(v: Vehicle | undefined): number {
  if (!v) return 6000
  const period = (v.tm?.['orb.period'] ?? NaN) * 60
  if (isFinite(period) && period > 0 && (v.ecc ?? 1) < 1) return Math.min(period * 1.05, 3e8)
  return v.central === 'sun' ? 86_400 * 400 : 86_400 * 5
}

function AttitudeButtons({ v }: { v: Vehicle }) {
  const modes: [string, string][] = [
    ['prograde', 'Prógrado'],
    ['retrograde', 'Retrógrado'],
    ['normal', 'Normal'],
    ['antinormal', 'Antinormal'],
    ['radial_out', 'Radial +'],
    ['radial_in', 'Radial −'],
    ['sun', 'Sol'],
    ['nadir', 'Nadir'],
    ['earth', 'Terra'],
    ['hold', 'Manter'],
  ]
  if (!v.systems) return null
  return (
    <div style={{ marginTop: 10 }}>
      <div className="small muted">{t('Atitude')}: {tx(v.systems.adcs.mode_name)}</div>
      <div className="row" style={{ marginTop: 4 }}>
        {modes.map(([m, name]) => (
          <button key={m} className={`small ${v.systems!.adcs.mode === m ? 'active' : ''}`} onClick={() => sendCommand(v.id, 'att_mode', { mode: m })}>
            {t(name)}
          </button>
        ))}
      </div>
    </div>
  )
}

const KINDS: { id: string; name: string; params: [string, string, number][]; target?: boolean; body?: boolean }[] = [
  { id: 'circularize_ap', name: 'Circularizar no apoastro', params: [] },
  { id: 'circularize_pe', name: 'Circularizar no periastro', params: [] },
  { id: 'hohmann', name: 'Hohmann para altitude', params: [['alt', 'Altitude (km)', 500]] },
  { id: 'apoapsis', name: 'Mudar apoastro', params: [['alt', 'Novo apoastro (km)', 1000]] },
  { id: 'periapsis', name: 'Mudar periastro', params: [['alt', 'Novo periastro (km)', 300]] },
  { id: 'inclination', name: 'Mudar inclinação', params: [['inc', 'Inclinação (°)', 51.6]] },
  { id: 'match_plane', name: 'Igualar plano a um alvo', params: [], target: true },
  { id: 'rendezvous', name: 'Encontro com um alvo (Lambert)', params: [], target: true },
  { id: 'deorbit', name: 'Saída de órbita (mira o alvo de pouso)', params: [['pe', 'Periastro (km)', 40]] },
  { id: 'injection', name: 'Injeção para outro corpo', params: [['depart_in_h', 'Partida em (h)', 1], ['tof_days', 'Viagem (dias)', 200]], body: true },
  { id: 'tcm', name: 'Correção de trajetória (TCM)', params: [['arrive_in_days', 'Chegada em (dias)', 150], ['pe', 'Periastro na chegada (km)', 300]], body: true },
  { id: 'capture', name: 'Captura (inserção em órbita)', params: [['apo', 'Apoastro final (km)', 10000]] },
]

export function Planner({ v, initial }: { v: Vehicle; initial?: string }) {
  const snap = useStore((s) => s.snap)!
  const [kind, setKind] = useState(initial ?? 'hohmann')
  const [params, setParams] = useState<Record<string, string>>({})
  const [target, setTarget] = useState<number | ''>('')
  const [body, setBody] = useState('mars')
  const [plan, setPlan] = useState<Plan | null>(null)
  const [busy, setBusy] = useState(false)
  const def = KINDS.find((k) => k.id === kind)!
  const targets = snap.vehicles.filter((x) => x.id !== v.id && x.kind !== 'debris' && x.central === v.central && x.phase !== 'prelaunch')
  const run = async () => {
    setBusy(true)
    const p: Record<string, unknown> = {}
    for (const [k, , d] of def.params) p[k] = Number((params[k] ?? String(d)).replace(',', '.'))
    if (def.target) p.target = target
    if (def.body) p.body = body
    try {
      const r = await store.request<{ ok: boolean; plan?: Plan; error?: string }>({ type: 'plan', vehicle: v.id, kind, params: p }, 60000)
      if (r.ok && r.plan) setPlan(r.plan)
      else {
        setPlan(null)
        toast(tx(r.error) || t('Sem solução'), 'bad')
      }
    } catch (e) {
      toast(String(e), 'bad')
    }
    setBusy(false)
  }
  const apply = async () => {
    if (!plan) return
    await store.request({ type: 'plan_apply', vehicle: v.id, nodes: plan.nodes, landing: plan.landing })
    toast(t('Queimas acrescentadas ao plano de voo'), 'good')
    setPlan(null)
  }
  return (
    <Panel title={t('Planejador de manobras')}>
      <div className="form">
        <label>{t('Manobra')}</label>
        <select value={kind} onChange={(e) => (setKind(e.target.value), setPlan(null))}>
          {KINDS.map((k) => (
            <option key={k.id} value={k.id}>
              {t(k.name)}
            </option>
          ))}
        </select>
        {def.params.map(([k, label, d]) => (
          <Field key={k} label={t(label)}>
            <input value={params[k] ?? String(d)} onChange={(e) => setParams({ ...params, [k]: e.target.value })} />
          </Field>
        ))}
        {def.target && (
          <Field label={t('Alvo')}>
            <select value={target} onChange={(e) => setTarget(e.target.value ? Number(e.target.value) : '')}>
              <option value="">—</option>
              {targets.map((x) => (
                <option key={x.id} value={x.id}>
                  {x.name}
                </option>
              ))}
            </select>
          </Field>
        )}
        {def.body && (
          <Field label={t('Destino')}>
            <select value={body} onChange={(e) => setBody(e.target.value)}>
              {['moon', 'mars', 'venus', 'mercury', 'jupiter', 'saturn'].map((b) => (
                <option key={b} value={b}>
                  {tx(store.hello?.bodies.find((x) => x.id === b)?.name ?? b)}
                </option>
              ))}
            </select>
          </Field>
        )}
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <button className="primary" onClick={run} disabled={busy || (def.target && target === '')}>
          {busy ? t('Calculando…') : t('Planejar')}
        </button>
        {plan && (
          <button className="primary" onClick={apply}>
            {t('Acrescentar ao plano de voo')}
          </button>
        )}
      </div>
      {plan && (
        <div style={{ marginTop: 8 }}>
          <div className="small">{tx(plan.summary)}</div>
          <table style={{ marginTop: 6 }}>
            <thead>
              <tr>
                <th>{t('Queima')}</th>
                <th className="num">{t('Em')}</th>
                <th className="num">Δv (m/s)</th>
              </tr>
            </thead>
            <tbody>
              {plan.nodes.map((n, i) => (
                <tr key={i}>
                  <td>{tx(n.label)}</td>
                  <td className="num">{duration(n.t)}</td>
                  <td className="num">{num(Math.hypot(n.dv_rtn.x, n.dv_rtn.y, n.dv_rtn.z), 1)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="small">Δv {t('total')}: <b>{num(plan.total_dv, 1)} m/s</b></div>
          {plan.landing && <div className="small">{t('Pouso previsto')}: {latlon(plan.landing[0], plan.landing[1])}</div>}
        </div>
      )}
    </Panel>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <label>{label}</label>
      {children}
    </>
  )
}

function NodeQueue({ v }: { v: Vehicle }) {
  const snap = useStore((s) => s.snap)!
  const nodes = v.nodes ?? []
  return (
    <Panel title={t('Plano de voo (queimas)')} right={nodes.length > 0 && <CmdButton vehicle={v.id} cmd="burn_clear" className="small">{t('Apagar todas')}</CmdButton>} flush>
      {nodes.length === 0 ? (
        <div className="muted" style={{ padding: 10 }}>{t('Nenhuma queima programada.')}</div>
      ) : (
        <table>
          <thead>
            <tr>
              <th>#</th>
              <th>{t('Queima')}</th>
              <th className="num">{t('Em')}</th>
              <th className="num">Δv</th>
              <th className="num">{t('Duração')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {nodes.map((n) => (
              <tr key={n.id} style={{ opacity: n.done ? 0.5 : 1 }}>
                <td className="mono">{n.id}</td>
                <td>
                  {tx(n.label)}
                  {n.rcs && <span className="muted small"> RCS</span>}
                  <div className="small muted mono">
                    R {num(n.dv[0], 1)} · T {num(n.dv[1], 1)} · N {num(n.dv[2], 1)}
                  </div>
                </td>
                <td className="num">{n.done ? t('feita') : n.executing ? <span className="warn">{t('queimando')}</span> : duration(n.t - snap.t)}</td>
                <td className="num">{num(n.executing ? n.remaining : n.dv_total, 1)}</td>
                <td className="num">{duration(n.burn)}</td>
                <td>
                  {!n.done && !n.executing && (
                    <CmdButton vehicle={v.id} cmd="burn_delete" args={{ node: n.id }} className="small">
                      ✕
                    </CmdButton>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  )
}

function ManualBurn({ v }: { v: Vehicle }) {
  const [f, setF] = useState({ t: '120', r: '0', tt: '10', n: '0' })
  const n = (s: string) => Number(s.replace(',', '.')) || 0
  return (
    <Panel title={t('Queima manual (RTN)')}>
      <div className="form">
        <label>{t('Daqui a (s)')}</label>
        <input value={f.t} onChange={(e) => setF({ ...f, t: e.target.value })} />
        <label>{t('Radial (m/s)')}</label>
        <input value={f.r} onChange={(e) => setF({ ...f, r: e.target.value })} />
        <label>{t('Transversal (m/s)')}</label>
        <input value={f.tt} onChange={(e) => setF({ ...f, tt: e.target.value })} />
        <label>{t('Normal (m/s)')}</label>
        <input value={f.n} onChange={(e) => setF({ ...f, n: e.target.value })} />
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <button className="primary" onClick={() => sendCommand(v.id, 'burn', { t: n(f.t), dv_r: n(f.r), dv_t: n(f.tt), dv_n: n(f.n), label: t('Queima manual') })}>
          {t('Programar')}
        </button>
        <span className="small muted">Δv {num(Math.hypot(n(f.r), n(f.tt), n(f.n)), 1)} m/s</span>
      </div>
      <div className="small muted" style={{ marginTop: 8 }}>
        {t('R: para fora; T: no sentido do movimento; N: normal ao plano. Até 2 m/s usa o RCS.')}
      </div>
    </Panel>
  )
}
