import { useEffect, useState } from 'react'
import { Panel, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { store, useStore } from '../lib/store'

export function Instructor() {
  const hello = useStore((s) => s.hello)!
  const snap = useStore((s) => s.snap)!
  const v = useStore((s) => s.selected)
  const [scenario, setScenario] = useState(snap.scenario)
  const [start, setStart] = useState('')
  const [fault, setFault] = useState(hello.faults[0]?.id ?? '')
  const [arg, setArg] = useState('1')
  const [saveName, setSaveName] = useState('estado')
  const [saves, setSaves] = useState<string[]>([])
  const [wx, setWx] = useState({ surface_wind: '', wind_from: '', jet_wind: '', lightning_km: '', ceiling_m: '' })
  const [warp, setWarp] = useState(String(snap.warp))
  const loadSaves = () => store.sim('saves').then((r: any) => setSaves(r.list ?? []))
  useEffect(() => {
    loadSaves()
  }, [])
  const result = (r: { ok?: boolean; error?: string; message?: string }) => toast(r.ok === false ? tx(r.error) : tx(r.message) || t('feito'), r.ok === false ? 'bad' : 'good')
  const faultDef = hello.faults.find((f) => f.id === fault)
  return (
    <div className="grid cols-3">
      <Panel title={t('Cenário')}>
        <select value={scenario} onChange={(e) => setScenario(e.target.value)} style={{ width: '100%' }}>
          {hello.scenarios.map((s) => (
            <option key={s.id} value={s.id}>
              {tx(s.name)}
            </option>
          ))}
        </select>
        <p className="small muted">{tx(hello.scenarios.find((s) => s.id === scenario)?.description)}</p>
        <div className="form">
          <label>{t('Início (UTC)')}</label>
          <input value={start} onChange={(e) => setStart(e.target.value)} placeholder={t('agora')} />
        </div>
        <div className="row" style={{ marginTop: 8 }}>
          <button className="primary" onClick={() => store.sim('scenario', { value: scenario, start: start || undefined })}>
            {t('Carregar cenário')}
          </button>
        </div>
      </Panel>
      <Panel title={t('Tempo')}>
        <div className="row">
          <button onClick={() => store.sim(snap.paused ? 'resume' : 'pause')}>{snap.paused ? t('Continuar') : t('Pausar')}</button>
          <input value={warp} onChange={(e) => setWarp(e.target.value)} style={{ width: 90 }} />
          <button onClick={() => store.sim('warp', { value: Number(warp) })}>{t('Aceleração')}</button>
        </div>
        <label className="small" style={{ display: 'block', marginTop: 8 }}>
          <input type="checkbox" checked={snap.warp_stop} onChange={(e) => store.sim('warp_stop', { value: e.target.checked })} /> {t('voltar para 1× num alarme')}
        </label>
        <p className="small muted">{t('Em aceleração alta a atitude passa a ser cinemática e os subsistemas são avaliados em passos maiores.')}</p>
        <button onClick={() => store.sim('remove_debris').then(result)}>{t('Remover detritos caídos')}</button>
      </Panel>
      <Panel title={t('Estados salvos')}>
        <div className="row">
          <input value={saveName} onChange={(e) => setSaveName(e.target.value)} style={{ width: 140 }} />
          <button
            onClick={() =>
              store.sim('save', { value: saveName }).then((r) => {
                result(r)
                loadSaves()
              })
            }
          >
            {t('Salvar')}
          </button>
        </div>
        <table style={{ marginTop: 8 }}>
          <tbody>
            {saves.map((s) => (
              <tr key={s}>
                <td>{s}</td>
                <td>
                  <button className="small" onClick={() => store.sim('load', { value: s })}>
                    {t('Carregar')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Panel>
      <Panel title={t('Injeção de falhas')}>
        <div className="form">
          <label>{t('Veículo')}</label>
          <span>{v?.name ?? '—'}</span>
          <label>{t('Falha')}</label>
          <select value={fault} onChange={(e) => setFault(e.target.value)}>
            {hello.faults.map((f) => (
              <option key={f.id} value={f.id}>
                {tx(f.name)}
              </option>
            ))}
          </select>
          {faultDef?.arg && (
            <>
              <label>{tx(faultDef.arg)}</label>
              <input value={arg} onChange={(e) => setArg(e.target.value)} />
            </>
          )}
        </div>
        <div className="row" style={{ marginTop: 8 }}>
          <button className="danger" disabled={!v} onClick={() => store.sim('fault', { vehicle: v?.id, fault, arg }).then(result)}>
            {t('Injetar')}
          </button>
          <button disabled={!v} onClick={() => store.sim('clear_faults', { vehicle: v?.id }).then(result)}>
            {t('Remover falhas')}
          </button>
        </div>
        {v?.faults && v.faults.length > 0 && <div className="small warn" style={{ marginTop: 6 }}>{t('Ativas')}: {v.faults.join(', ')}</div>}
      </Panel>
      <Panel title={t('Clima')}>
        <div className="form">
          {(
            [
              ['surface_wind', 'Vento de superfície (m/s)', snap.weather.surface_wind],
              ['wind_from', 'Direção do vento (°)', snap.weather.wind_from],
              ['jet_wind', 'Corrente de jato (m/s)', snap.weather.jet_wind],
              ['lightning_km', 'Raio mais próximo (km)', snap.weather.lightning_km],
              ['ceiling_m', 'Teto (m)', snap.weather.ceiling_m],
            ] as [keyof typeof wx, string, number][]
          ).map(([k, label, cur]) => (
            <Row key={k} label={t(label)}>
              <input value={wx[k]} placeholder={cur.toFixed(1)} onChange={(e) => setWx({ ...wx, [k]: e.target.value })} />
            </Row>
          ))}
        </div>
        <div className="row" style={{ marginTop: 8 }}>
          <button
            onClick={() => {
              const p: Record<string, number> = {}
              for (const [k, val] of Object.entries(wx)) if (val !== '') p[k] = Number(val)
              store.sim('weather', p).then(result)
            }}
          >
            {t('Aplicar')}
          </button>
          <label className="small">
            <input type="checkbox" checked={snap.weather.evolve} onChange={(e) => store.sim('weather', { evolve: e.target.checked })} /> {t('o clima muda sozinho')}
          </label>
          <label className="small">
            <input type="checkbox" checked={snap.weather.rain} onChange={(e) => store.sim('weather', { rain: e.target.checked })} /> {t('chuva')}
          </label>
        </div>
      </Panel>
      <Panel title={t('Catálogo de veículos')} flush>
        <div style={{ maxHeight: 320, overflowY: 'auto' }}>
          {hello.catalog.map((c) => (
            <div key={c.id} style={{ padding: '6px 10px', borderBottom: '1px solid #1a2540' }}>
              <b>{tx(c.name)}</b>
              <div className="small muted">{tx(c.description)}</div>
            </div>
          ))}
        </div>
      </Panel>
    </div>
  )
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <label>{label}</label>
      {children}
    </>
  )
}
