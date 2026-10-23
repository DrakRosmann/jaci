import { useEffect, useState } from 'react'
import { Panel, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { met } from '../lib/format'
import { store, useStore } from '../lib/store'
import type { RecordingEntry } from '../types'
import { FaultPlan, TrainingReport } from './Training'
import { Missions } from './Missions'

export function Instructor() {
  const hello = useStore((s) => s.hello)!
  const snap = useStore((s) => s.snap)!
  const v = useStore((s) => s.selected)
  const [scenario, setScenario] = useState(snap.scenario)
  const [start, setStart] = useState('')
  const [fault, setFault] = useState(hello.faults[0]?.id ?? '')
  const [arg, setArg] = useState('1')
  const [announce, setAnnounce] = useState(true)
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
              {s.file ? ` (${t('arquivo')})` : ''}
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
      <RoomPanel />
      <ScriptsPanel />
      <Recordings />
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
          <button className="danger" disabled={!v} onClick={() => store.sim('fault', { vehicle: v?.id, fault, arg, announce }).then(result)}>
            {t('Injetar')}
          </button>
          <label className="small" title={t('Sem aviso, a equipe tem de descobrir a falha pelos sintomas')}>
            <input type="checkbox" checked={announce} onChange={(e) => setAnnounce(e.target.checked)} /> {t('avisar a equipe')}
          </label>
          <button disabled={!v} onClick={() => store.sim('clear_faults', { vehicle: v?.id }).then(result)}>
            {t('Remover falhas')}
          </button>
        </div>
        {v?.faults && v.faults.length > 0 && <div className="small warn" style={{ marginTop: 6 }}>{t('Ativas')}: {v.faults.join(', ')}</div>}
      </Panel>
      <FaultPlan />
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
      <TrainingReport />
      <Missions />
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

/** Gravações da missão: gravar, listar e reproduzir para o debriefing. */
function Recordings() {
  const rec = useStore((s) => s.snap?.recording ?? null)
  const replay = useStore((s) => s.snap?.replay ?? null)
  const [name, setName] = useState('')
  const [list, setList] = useState<RecordingEntry[]>([])
  const [dir, setDir] = useState('')
  const load = () =>
    store
      .sim('recordings')
      .then((r: any) => {
        setList(r.list ?? [])
        setDir(r.dir ?? '')
      })
      .catch(() => {})
  useEffect(() => {
    load()
  }, [rec?.name])
  const result = (r: { ok?: boolean; error?: string; message?: string }) => toast(r.ok === false ? tx(r.error) : tx(r.message) || t('feito'), r.ok === false ? 'bad' : 'good')
  return (
    <Panel title={t('Gravações')} right={dir && <span className="small muted mono" title={dir}>{dir.split('/').slice(-2).join('/')}</span>}>
      {rec ? (
        <div className="row">
          <span className="rec">
            <span className="dot bad" /> {rec.name}
          </span>
          <span className="small muted">
            {rec.frames} {t('quadros')} · {met(rec.span).replace('T+', '')} · {bytes(rec.bytes)}
          </span>
          <button onClick={() => store.sim('record_stop').then(result).then(load)}>{t('Parar a gravação')}</button>
        </div>
      ) : (
        <div className="row">
          <input value={name} onChange={(e) => setName(e.target.value)} placeholder={t('nome (automático se vazio)')} style={{ width: 190 }} disabled={!!replay} />
          <button className="primary" disabled={!!replay} onClick={() => store.sim('record_start', { value: name || undefined }).then(result)}>
            ● {t('Gravar')}
          </button>
        </div>
      )}
      <p className="small muted">{t('Grava um quadro por segundo: telemetria, eventos, alarmes e comandos. Na reprodução a simulação espera e todas as telas mostram a gravação.')}</p>
      <div style={{ maxHeight: 220, overflowY: 'auto' }}>
        <table>
          <tbody>
            {list.map((g) => (
              <tr key={g.name} className={replay?.name === g.name ? 'sel' : ''}>
                <td>
                  {g.name}
                  <div className="small muted">
                    {tx(g.mission ?? '')} · {g.created ?? g.modified} · {bytes(g.bytes)}
                  </div>
                </td>
                <td>
                  <button className="small" disabled={!!rec || replay?.name === g.name} onClick={() => store.sim('replay_start', { value: g.name }).then(result)}>
                    ▶ {t('Reproduzir')}
                  </button>
                </td>
              </tr>
            ))}
            {list.length === 0 && (
              <tr>
                <td className="muted">{t('Nenhuma gravação ainda.')}</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </Panel>
  )
}

function bytes(n: number): string {
  return n >= 1e6 ? `${(n / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(n / 1e3))} kB`
}

/** Sala com posições: ligar e ver quem está em cada posição. */
function RoomPanel() {
  const room = useStore((s) => s.snap?.room)
  const roles = useStore((s) => s.hello?.roles ?? [])
  const result = (r: { ok?: boolean; error?: string; message?: string }) => toast(r.ok === false ? tx(r.error) : tx(r.message) || t('feito'), r.ok === false ? 'bad' : 'good')
  return (
    <Panel title={t('Sala com posições')}>
      <label>
        <input type="checkbox" checked={!!room?.on} onChange={(e) => store.sim('room_mode', { value: e.target.checked }).then(result)} /> {t('cada tela entra numa posição e só comanda o que é dela')}
      </label>
      <p className="small muted">{t('FLIGHT comanda tudo; cada posição comanda os grupos dela (RSO a terminação de voo, EECOM energia e suporte de vida…); as posições do lançamento ocupadas por pessoas votam elas mesmas; o instrutor só mexe no simulador; o observador só olha.')}</p>
      {room && room.members.length > 0 ? (
        <table>
          <tbody>
            {room.members.map((m, i) => (
              <tr key={i}>
                <td className="mono">{m.position}</td>
                <td>{m.name}</td>
                <td className="small muted">{tx(roles.find((r) => r.id === m.position)?.name ?? '')}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className="small muted">{t('Ninguém entrou numa posição ainda.')}</p>
      )}
    </Panel>
  )
}

/** Automação por script: ligar, desligar e ver se está rodando. */
function ScriptsPanel() {
  const list = useStore((s) => s.snap?.scripts ?? [])
  const result = (r: { ok?: boolean; error?: string; message?: string }) => toast(r.ok === false ? tx(r.error) : tx(r.message) || t('feito'), r.ok === false ? 'bad' : 'good')
  return (
    <Panel title={t('Scripts de automação')}>
      {list.length === 0 ? (
        <p className="small muted">{t('Nenhum script em missoes/scripts. Crie um no editor de missões, abaixo.')}</p>
      ) : (
        <table>
          <tbody>
            {list.map((s) => (
              <tr key={s.name}>
                <td>
                  <label>
                    <input type="checkbox" checked={s.enabled} onChange={(e) => store.sim('script_enable', { name: s.name, value: e.target.checked }).then(result)} /> <span className="mono">{s.name}</span>
                  </label>
                  {s.error && <div className="small bad">{s.error}</div>}
                </td>
                <td className="num small muted">{s.runs > 0 ? `${s.runs} ${t('execuções')}` : ''}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <p className="small muted">{t('Rodam a cada segundo de simulação: cmd() manda pelo enlace, onboard() executa a bordo na hora. Erro ou laço sem fim desliga o script.')}</p>
    </Panel>
  )
}
