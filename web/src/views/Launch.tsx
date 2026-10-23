import { useState } from 'react'
import { Bar, CmdButton, Empty, Panel, Tile, sendCommand } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { latlon, num, utc } from '../lib/format'
import { canVote, myLaunchPositions } from '../lib/room'
import { useStore } from '../lib/store'
import type { Vehicle } from '../types'

export const ABORT_NAMES: Record<string, string> = {
  les: 'Sistema de escape',
  separate: 'Separar e reentrar',
  ato: 'Aborto para órbita (ATO)',
  aoa: 'Uma volta e reentrada (AOA)',
  tal: 'Pouso transoceânico (TAL)',
  rtls: 'Retorno ao local (RTLS)',
}

export function LaunchView() {
  const snap = useStore((s) => s.snap)!
  const lc = snap.launch
  const v = useStore((s) => (lc.vehicle != null ? s.vehicle(lc.vehicle) : s.selected))
  const room = useStore((s) => !!s.snap?.room?.on)
  const members = useStore((s) => s.snap?.room?.members)
  const roles = useStore((s) => s.hello?.roles)
  useStore((s) => s.me)
  if (!v) return <Empty>{t('Nenhum veículo na plataforma neste cenário.')}</Empty>
  const id = v.id
  const onPad = v.phase === 'prelaunch'
  // Sala com posições: quem ocupa cada posição da votação e quais são desta tela.
  const mine = room ? myLaunchPositions() : []
  const staff = (lp: string) =>
    (members ?? [])
      .filter((m) => roles?.find((r) => r.id === m.position)?.launch.includes(lp))
      .map((m) => `${m.name} (${m.position})`)
      .join(', ')
  const waitingMine = lc.positions.filter((p) => p.awaiting && mine.includes(p.id))
  return (
    <div className="grid">
      {lc.state === 'hold' && lc.hold_reason && <div className="banner warn">{t('Em espera')}: {tx(lc.hold_reason)}</div>}
      {lc.state === 'scrubbed' && <div className="banner alarm">{tx(lc.hold_reason) || t('Lançamento cancelado')}</div>}
      {waitingMine.length > 0 && (
        <div className="banner warn row vote-banner" role="alert">
          <b>{t('Votação GO/NO-GO')}:</b> {waitingMine.map((p) => tx(p.name)).join(', ')} — {t('qual é o seu voto?')}
          {waitingMine.map((p) => (
            <span key={p.id} className="row">
              <button className="primary" onClick={() => sendCommand(id, 'vote', { position: p.id, go: 'go' })}>
                {tx(p.name)}: GO
              </button>
              <button className="danger" onClick={() => sendCommand(id, 'vote', { position: p.id, go: 'nogo' })}>
                NO-GO
              </button>
            </span>
          ))}
        </div>
      )}
      <div className="grid cols-3">
        <Panel title={`${t('Contagem')} · ${v.name}`}>
          <div className={`big-clock ${lc.state === 'hold' ? 'hold' : ''}`}>{onPad || lc.state !== 'launched' ? lc.clock : '—'}</div>
          <div className="row" style={{ justifyContent: 'center', margin: '8px 0' }}>
            <span className={`pill ${lc.all_go ? 'go' : 'nogo'}`}>{lc.all_go ? 'GO' : 'NO-GO'}</span>
            <span className="muted">{stateName(lc.state)}</span>
            {lc.auto_sequence && <span className="pill pending">{t('sequência automática')}</span>}
          </div>
          <div className="row" style={{ justifyContent: 'center' }}>
            <CmdButton vehicle={id} cmd="count_start" className="primary" disabled={!onPad}>
              {lc.state === 'idle' ? t('Iniciar contagem') : t('Retomar')}
            </CmdButton>
            <CmdButton vehicle={id} cmd="count_hold" disabled={!onPad || lc.state !== 'counting'}>
              {t('Espera')}
            </CmdButton>
            <CmdButton vehicle={id} cmd="count_recycle" disabled={!onPad}>
              {t('Recomeçar em T-10')}
            </CmdButton>
            <CmdButton vehicle={id} cmd="poll" disabled={!onPad}>
              {t('Votação GO/NO-GO')}
            </CmdButton>
            <CmdButton vehicle={id} cmd="count_scrub" disabled={!onPad}>
              {t('Cancelar')}
            </CmdButton>
          </div>
          {lc.window && (
            <div className="small" style={{ marginTop: 10, textAlign: 'center' }}>
              {t('Janela')}: {utc(lc.window[0])} → {utc(lc.window[1], false)}
            </div>
          )}
          <WindowForm v={v} />
        </Panel>
        <Panel title={t('Posições do console')} flush>
          <table>
            <tbody>
              {lc.positions.map((p) => (
                <tr key={p.id} className={mine.includes(p.id) ? 'sel' : ''}>
                  <td>
                    {tx(p.name)}
                    {staff(p.id) && <div className="small muted">{staff(p.id)}</div>}
                  </td>
                  <td>
                    {p.awaiting ? <span className="pill pending">{t('aguardando voto')}</span> : <span className={`pill ${p.go ? 'go' : 'nogo'}`}>{p.go ? 'GO' : 'NO-GO'}</span>}
                    {p.polled != null && <span className="muted small"> · {t('votou')} {p.polled ? 'GO' : 'NO-GO'}</span>}
                  </td>
                  <td>
                    {room && mine.includes(p.id) ? (
                      <span className="row">
                        <button className="small" disabled={!onPad} onClick={() => sendCommand(id, 'vote', { position: p.id, go: 'go' })}>
                          GO
                        </button>
                        <button className="small danger" disabled={!onPad} onClick={() => sendCommand(id, 'vote', { position: p.id, go: 'nogo' })}>
                          NO-GO
                        </button>
                      </span>
                    ) : (
                    <select
                      value={p.manual == null ? 'auto' : p.manual ? 'go' : 'nogo'}
                      onChange={(e) => sendCommand(id, 'position', { position: p.id, go: e.target.value })}
                      disabled={!onPad || !canVote(p.id)}
                    >
                      <option value="auto">{t('automático')}</option>
                      <option value="go">GO</option>
                      <option value="nogo">NO-GO</option>
                    </select>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {lc.positions.filter((p) => p.reason && !p.go).map((p) => (
            <div key={p.id} className="small bad" style={{ padding: '2px 10px' }}>
              {tx(p.name)}: {tx(p.reason)}
            </div>
          ))}
        </Panel>
        <Panel title={t('Sequência')} flush>
          <table>
            <tbody>
              {lc.sequence.map((s, i) => (
                <tr key={i} style={{ opacity: s.done ? 0.55 : 1 }}>
                  <td className="mono">T-{fmtT(s.at)}</td>
                  <td>{tx(s.name)}</td>
                  <td>{s.done ? <span className="good">✓</span> : s.at >= lc.t_minus - 0.5 && lc.state === 'counting' ? '…' : ''}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Panel>
      </div>
      <div className="grid cols-3">
        <Panel title={t('Critérios de lançamento (LCC)')} flush>
          <table>
            <thead>
              <tr>
                <th>{t('Critério')}</th>
                <th>{t('Valor')}</th>
                <th>{t('Limite')}</th>
              </tr>
            </thead>
            <tbody>
              {lc.criteria.map((c) => (
                <tr key={c.id}>
                  <td className={c.ok ? '' : 'bad'}>{c.ok ? '✓' : '✗'} {tx(c.name)}</td>
                  <td className="mono">{tx(c.value)}</td>
                  <td className="muted small">{tx(c.limit)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </Panel>
        <Panel title={t('Clima')}>
          <div className="tiles">
            <Tile label={t('Vento de superfície')} value={num(snap.weather.surface_wind, 1)} unit="m/s" />
            <Tile label={t('Direção do vento')} value={num(snap.weather.wind_from, 0)} unit="°" />
            <Tile label={t('Corrente de jato')} value={num(snap.weather.jet_wind, 0)} unit="m/s" />
            <Tile label={t('Raio mais próximo')} value={num(snap.weather.lightning_km, 0)} unit="km" state={snap.weather.lightning_km < 18 ? 'red' : 'nominal'} />
            <Tile label={t('Teto')} value={num(snap.weather.ceiling_m, 0)} unit="m" />
            <Tile label={t('Chuva')} value={snap.weather.rain ? t('sim') : t('não')} />
            <Tile label={t('Temperatura')} value={num(snap.weather.temperature_c, 0)} unit="°C" />
          </div>
          {v.site && <div className="small muted" style={{ marginTop: 8 }}>{v.site.name} · {latlon(v.site.lat, v.site.lon)}</div>}
        </Panel>
        <Panel title={t('Propelentes')}>
          {(v.stages ?? []).filter((s) => (s.capacity ?? 0) > 0).map((s, i) => (
            <div key={i} style={{ marginBottom: 6 }}>
              <div className="row small">
                <span>{tx(s.name)}</span>
                {s.cryogenic && <span className="muted">{t('criogênico')}</span>}
                {s.leak && <span className="bad">{t('vazamento')}</span>}
                <span className="spacer" />
                <span className="mono">{num(((s.prop ?? 0) / (s.capacity || 1)) * 100, 1)} %</span>
              </div>
              <Bar frac={(s.prop ?? 0) / (s.capacity || 1)} kind="fuel" />
            </div>
          ))}
          {(v.stages ?? []).some((s) => s.cryogenic) && <div className="row" style={{ marginTop: 8 }}>
            <CmdButton vehicle={id} cmd="load" args={{ on: 'on' }} disabled={!onPad || lc.loading}>
              {t('Carregar')}
            </CmdButton>
            <CmdButton vehicle={id} cmd="load" args={{ on: 'off' }} disabled={!onPad || !lc.loading}>
              {t('Parar carregamento')}
            </CmdButton>
            {lc.loading && <span className="warn small">{t('carregando…')}</span>}
          </div>}
        </Panel>
      </div>
      <div className="grid cols-2">
        <SafetyPanel v={v} />
        <AbortPanel v={v} />
      </div>
    </div>
  )
}

function WindowForm({ v }: { v: Vehicle }) {
  const snap = useStore((s) => s.snap)!
  const targets = snap.vehicles.filter((x) => x.id !== v.id && x.kind !== 'debris' && ['orbit', 'docked'].includes(x.phase))
  const [target, setTarget] = useState<number | ''>('')
  const [time, setTime] = useState('')
  if (v.phase !== 'prelaunch') return null
  return (
    <div className="row small" style={{ marginTop: 10, justifyContent: 'center' }}>
      <select value={target} onChange={(e) => setTarget(e.target.value ? Number(e.target.value) : '')}>
        <option value="">{t('Janela no plano de…')}</option>
        {targets.map((x) => (
          <option key={x.id} value={x.id}>
            {x.name}
          </option>
        ))}
      </select>
      <button disabled={target === ''} onClick={() => sendCommand(v.id, 'window', { target })}>
        {t('Calcular')}
      </button>
      <input placeholder="AAAA-MM-DD HH:MM:SS" value={time} onChange={(e) => setTime(e.target.value)} style={{ width: 160 }} />
      <button disabled={!time} onClick={() => sendCommand(v.id, 'window', { utc: time })}>
        {t('T-0 nesta hora')}
      </button>
    </div>
  )
}

export function SafetyPanel({ v }: { v: Vehicle }) {
  const snap = useStore((s) => s.snap)!
  if (!v.fts?.installed) return <Panel title={t('Segurança de voo')}><span className="muted">{t('Sem sistema de terminação de voo.')}</span></Panel>
  return (
    <Panel title={t('Segurança de voo')}>
      <div className="tiles">
        <Tile label={t('Terminação')} value={v.fts.armed ? t('ARMADA') : t('em segurança')} state={v.fts.armed ? 'yellow' : 'nominal'} />
        <Tile label={t('Autônoma')} value={v.fts.autonomous ? t('ligada') : t('desligada')} />
        {v.iip && <Tile label={t('Distância ao eixo')} value={num(v.iip.cross_km, 0)} unit="km" state={v.iip.inside ? 'nominal' : 'red'} />}
        {v.iip && <Tile label={t('Impacto previsto')} value={latlon(v.iip.lat, v.iip.lon)} />}
        {snap.corridor && <Tile label={t('Meia largura do corredor')} value={num(snap.corridor.half_width_km, 0)} unit="km" />}
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <CmdButton vehicle={v.id} cmd="fts_arm" disabled={v.fts.armed}>
          {t('Armar')}
        </CmdButton>
        <CmdButton vehicle={v.id} cmd="fts_safe" disabled={!v.fts.armed}>
          {t('Desarmar')}
        </CmdButton>
        <CmdButton vehicle={v.id} cmd="fts_auto" args={{ on: v.fts.autonomous ? 'off' : 'on' }}>
          {v.fts.autonomous ? t('Desligar autônoma') : t('Ligar autônoma')}
        </CmdButton>
        <span className="spacer" />
        <CmdButton vehicle={v.id} cmd="fts_terminate" disabled={!v.fts.armed || v.phase === 'destroyed'}>
          {t('TERMINAR O VOO')}
        </CmdButton>
      </div>
    </Panel>
  )
}

export function AbortPanel({ v }: { v: Vehicle }) {
  const modes = v.aborts ?? []
  return (
    <Panel title={t('Abortos')}>
      {modes.length === 0 ? (
        <span className="muted">{t('Sem modos de aborto para este veículo.')}</span>
      ) : (
        <div className="row">
          {modes.map(([m, ok]) => (
            <CmdButton key={m} vehicle={v.id} cmd="abort" args={{ mode: m }} disabled={!ok}>
              {t(ABORT_NAMES[m] ?? m)}
            </CmdButton>
          ))}
        </div>
      )}
      <div className="small muted" style={{ marginTop: 8 }}>{t('Os modos acendem conforme a fase do voo; cada um pede confirmação.')}</div>
    </Panel>
  )
}

function stateName(s: string): string {
  return t(({ idle: 'Parada', counting: 'Contando', hold: 'Em espera', scrubbed: 'Cancelado', launched: 'Lançado' } as Record<string, string>)[s] ?? s)
}

function fmtT(s: number): string {
  const a = Math.max(0, Math.round(s))
  return `${String(Math.floor(a / 3600)).padStart(2, '0')}:${String(Math.floor((a % 3600) / 60)).padStart(2, '0')}:${String(a % 60).padStart(2, '0')}`
}
