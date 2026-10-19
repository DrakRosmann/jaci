import { useState } from 'react'
import { Chart, COLORS } from '../components/Chart'
import { Bar, CmdButton, Empty, Panel, Tile, Tm, sendCommand } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { duration, latlon, num } from '../lib/format'
import { store, useStore } from '../lib/store'
import type { Vehicle } from '../types'
import { AbortPanel } from './Launch'
import { programName } from './Overview'

export function Trajectory() {
  const v = useStore((s) => s.selected)
  useStore((s) => s.version)
  if (!v || !v.tm) return <Empty>{t('Selecione um veículo com telemetria.')}</Empty>
  return (
    <div className="grid">
      <div className="grid cols-4">
        <Plot v={v} ids={[['nav.alt', 'Altitude (km)']]} />
        <Plot v={v} ids={[['nav.speed', 'Velocidade (m/s)'], ['nav.hspeed', 'Horizontal']]} />
        <Plot v={v} ids={[['nav.q', 'Pressão dinâmica (kPa)']]} />
        <Plot v={v} ids={[['nav.accel', 'Aceleração (g)']]} />
      </div>
      <div className="grid cols-3">
        <Panel title={t('Navegação')}>
          <div className="tiles">
            <Tm v={v} id="nav.alt" digits={2} />
            <Tm v={v} id="nav.speed" digits={0} />
            <Tm v={v} id="nav.vspeed" digits={0} />
            <Tm v={v} id="nav.mach" digits={2} />
            <Tm v={v} id="nav.q" digits={2} />
            <Tm v={v} id="nav.accel" digits={2} />
            <Tm v={v} id="nav.pitch" digits={1} />
            <Tm v={v} id="nav.heading" digits={1} />
            <Tm v={v} id="nav.aoa" digits={1} />
            <Tm v={v} id="nav.downrange" digits={0} />
            <Tm v={v} id="nav.twr" digits={2} />
            <Tm v={v} id="nav.mass" digits={2} />
            <Tm v={v} id="orb.pe" digits={0} />
            <Tm v={v} id="orb.ap" digits={0} />
          </div>
        </Panel>
        <GuidancePanel v={v} />
        <AbortPanel v={v} />
      </div>
      <EnginesPanel v={v} />
      {(v.heat_shield || (v.chutes?.length ?? 0) > 0 || v.landing === 'runway') && <EntryPanel v={v} />}
    </div>
  )
}

function Plot({ v, ids }: { v: Vehicle; ids: [string, string][] }) {
  const t0 = v.met != null ? store.snap!.t - v.met : undefined
  const since = t0 ?? -Infinity
  const series = ids.map(([id, label], i) => {
    const [ts, vs] = store.history.series(v.id, id, since)
    return { label: t(label), color: COLORS[i], t: ts, v: vs }
  })
  return (
    <Panel title={t(ids[0][1])} flush>
      <Chart series={series} height={170} t0={t0} />
    </Panel>
  )
}

function GuidancePanel({ v }: { v: Vehicle }) {
  const g = v.guidance
  if (!g) return null
  return (
    <Panel title={t('Guiamento')}>
      <div className="tiles">
        <Tile label={t('Programa')} value={tx(programName(g.program))} />
        <Tile label={t('Malha')} value={g.closed_loop ? t('fechada') : t('aberta')} />
        {g.t_go > 0 && g.program !== 'none' && <Tile label={t('Tempo para o corte')} value={duration(g.t_go)} />}
        {g.ascent && <Tile label={t('Órbita alvo')} value={`${num(g.ascent.pe, 0)} × ${num(g.ascent.ap, 0)}`} unit="km" />}
        {g.ascent && <Tile label={t('Inclinação alvo')} value={num(g.ascent.inc, 2)} unit="°" />}
        {g.azimuth != null && <Tile label={t('Azimute de lançamento')} value={num(((g.azimuth % 360) + 360) % 360, 1)} unit="°" />}
        <Tile label={t('Inserção')} value={g.inserted ? t('feita') : '—'} state={g.inserted ? 'nominal' : 'none'} />
        {g.kick_wait && <Tile label={t('Motor de apogeu')} value={t('esperando o apogeu')} />}
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <label className="small">
          <input type="checkbox" checked={g.auto_stage} onChange={(e) => sendCommand(v.id, 'auto_stage', { on: e.target.checked ? 'on' : 'off' })} /> {t('estágios automáticos')}
        </label>
        <CmdButton vehicle={v.id} cmd="stage">{t('Separar estágio')}</CmdButton>
        {(v.items ?? []).map((it) => (
          <CmdButton key={it.id} vehicle={v.id} cmd="jettison" args={{ item: it.id }}>
            {t('Descartar')} {tx(it.name).toLowerCase()}
          </CmdButton>
        ))}
        {v.les?.available && v.les.tower && (
          <CmdButton vehicle={v.id} cmd="jettison" args={{ item: 'les' }}>
            {t('Descartar torre de escape')}
          </CmdButton>
        )}
        {(v.stages ?? []).some((s) => !s.spacecraft) && v.phase !== 'prelaunch' && (
          <CmdButton vehicle={v.id} cmd="payload_sep">{t('Separar carga útil')}</CmdButton>
        )}
      </div>
    </Panel>
  )
}

function EnginesPanel({ v }: { v: Vehicle }) {
  const [throttle, setThrottle] = useState(100)
  const stages = v.stages ?? []
  return (
    <Panel
      title={t('Propulsão')}
      right={
        <span className="row">
          <CmdButton vehicle={v.id} cmd="ignite" className="small">{t('Ignição')}</CmdButton>
          <CmdButton vehicle={v.id} cmd="shutdown" className="small">{t('Corte')}</CmdButton>
          <input type="number" min={0} max={100} value={throttle} onChange={(e) => setThrottle(Number(e.target.value))} style={{ width: 60 }} />
          <button className="small" onClick={() => sendCommand(v.id, 'throttle', { value: throttle })}>
            {t('Potência %')}
          </button>
        </span>
      }
      flush
    >
      <table>
        <thead>
          <tr>
            <th>{t('Estágio')}</th>
            <th>{t('Propelente')}</th>
            <th>{t('Motor')}</th>
            <th>{t('Estado')}</th>
            <th className="num">{t('Empuxo')} (kN)</th>
            <th className="num">{t('Câmara')} (bar)</th>
            <th className="num">{t('Potência')}</th>
            <th className="num">{t('Ignições')}</th>
            <th className="num">{t('Queima')}</th>
          </tr>
        </thead>
        <tbody>
          {stages.flatMap((s, i) => {
            const engines = s.engines?.length ? s.engines : [null]
            return engines.map((e, j) => (
              <tr key={`${i}-${j}`}>
                {j === 0 && (
                  <>
                    <td rowSpan={engines.length}>
                      {tx(s.name)}
                      {s.boosters && <span className="muted small"> ({t('boosters')})</span>}
                      {s.spacecraft && <span className="muted small"> ({t('nave')})</span>}
                    </td>
                    <td rowSpan={engines.length} style={{ minWidth: 110 }}>
                      {(s.capacity ?? 0) > 0 ? (
                        <>
                          <Bar frac={(s.prop ?? 0) / (s.capacity || 1)} kind="fuel" />
                          <span className="small mono">{num(s.prop ?? 0, 0)} kg</span>
                        </>
                      ) : (
                        '—'
                      )}
                    </td>
                  </>
                )}
                {e ? (
                  <>
                    <td>{tx(e.name)}</td>
                    <td className={engineClass(e.state)}>{t(engineState(e.state))}</td>
                    <td className="num">{num(e.thrust, 1)}</td>
                    <td className="num">{num(e.pc, 1)}</td>
                    <td className="num">{e.solid ? '—' : `${num(e.throttle * 100, 0)} %`}</td>
                    <td className="num">{e.ignitions}</td>
                    <td className="num">{duration(e.burn)}</td>
                  </>
                ) : (
                  <td colSpan={7} className="muted">
                    {t('sem motores')}
                  </td>
                )}
              </tr>
            ))
          })}
        </tbody>
      </table>
    </Panel>
  )
}

function engineState(s: string): string {
  return ({ off: 'desligado', starting: 'partida', running: 'queimando', stopping: 'desligando', failed: 'FALHA' } as Record<string, string>)[s] ?? s
}

function engineClass(s: string): string {
  return s === 'running' ? 'good' : s === 'failed' ? 'bad' : s === 'off' ? 'muted' : 'warn'
}

function EntryPanel({ v }: { v: Vehicle }) {
  const g = v.guidance
  const [lat, setLat] = useState(String(g?.landing_target?.[0] ?? ''))
  const [lon, setLon] = useState(String(g?.landing_target?.[1] ?? ''))
  const hs = v.heat_shield
  return (
    <div className="grid cols-3">
      <Panel title={t('Escudo térmico e cargas')}>
        <div className="tiles">
          {hs && <Tile label={t('Temperatura do escudo')} value={num(hs.temp, 0)} unit="K" state={hs.temp > 2900 ? 'red' : hs.temp > 2300 ? 'yellow' : 'none'} />}
          {hs && <Tile label={t('Fluxo de calor')} value={num(hs.flux, 0)} unit="kW/m²" />}
          {hs && <Tile label={t('Pico de fluxo')} value={num(hs.peak, 0)} unit="kW/m²" />}
          {hs && <Tile label={t('Ablativo')} value={num(hs.ablator * 100, 1)} unit="%" state={hs.ablator < 0.1 ? 'red' : hs.ablator < 0.3 ? 'yellow' : 'nominal'} />}
          <Tm v={v} id="nav.accel" digits={2} />
          {g && <Tile label={t('Rolamento')} value={num(g.bank, 1)} unit="°" />}
          {v.systems?.comms.blackout && <Tile label={t('Comunicações')} value={t('apagão')} state="yellow" />}
        </div>
      </Panel>
      <Panel title={t('Paraquedas e pouso')}>
        <table>
          <tbody>
            {(v.chutes ?? []).map((c, i) => (
              <tr key={i}>
                <td>{tx(c.name)}</td>
                <td className="small muted">{num(c.alt / 1000, 1)} km</td>
                <td className={c.state === 'deployed' ? 'good' : c.state === 'failed' ? 'bad' : 'muted'}>{t(chuteState(c.state))}</td>
                <td>
                  <CmdButton vehicle={v.id} cmd="chute" args={{ index: i + 1, action: 'deploy' }} className="small" disabled={c.state !== 'stowed'}>
                    {t('Abrir')}
                  </CmdButton>{' '}
                  <CmdButton vehicle={v.id} cmd="chute" args={{ index: i + 1, action: 'cut' }} className="small" disabled={c.state !== 'deployed'}>
                    {t('Cortar')}
                  </CmdButton>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {v.landing === 'runway' && (
          <div className="row" style={{ marginTop: 8 }}>
            <Tile label={t('Trem de pouso')} value={v.gear_down ? t('baixado') : t('recolhido')} />
            <CmdButton vehicle={v.id} cmd="gear" disabled={v.gear_down}>
              {t('Baixar trem')}
            </CmdButton>
          </div>
        )}
        <label className="small" style={{ display: 'block', marginTop: 8 }}>
          <input type="checkbox" checked={g?.auto_chutes ?? false} onChange={(e) => sendCommand(v.id, 'auto_chutes', { on: e.target.checked ? 'on' : 'off' })} /> {t('sequência automática de reentrada')}
        </label>
      </Panel>
      <Panel title={t('Alvo de pouso')}>
        {g?.landing_target ? <div className="mono">{latlon(g.landing_target[0], g.landing_target[1])}</div> : <div className="muted">{t('sem alvo')}</div>}
        <div className="row" style={{ marginTop: 8 }}>
          <input value={lat} onChange={(e) => setLat(e.target.value)} placeholder={t('lat')} style={{ width: 80 }} />
          <input value={lon} onChange={(e) => setLon(e.target.value)} placeholder={t('lon')} style={{ width: 80 }} />
          <button onClick={() => sendCommand(v.id, 'landing_target', { lat: Number(lat.replace(',', '.')), lon: Number(lon.replace(',', '.')) })}>{t('Definir')}</button>
        </div>
        <div className="row" style={{ marginTop: 8 }}>
          <button className={g?.ballistic ? '' : 'active'} onClick={() => sendCommand(v.id, 'entry_mode', { mode: 'guided' })}>
            {t('Guiada')}
          </button>
          <button className={g?.ballistic ? 'active' : ''} onClick={() => sendCommand(v.id, 'entry_mode', { mode: 'ballistic' })}>
            {t('Balística')}
          </button>
        </div>
        <div className="small muted" style={{ marginTop: 8 }}>
          {t('O guiamento prevê o ponto de chegada e ajusta o rolamento para cair no alvo; a sustentação alterna de lado para corrigir o desvio lateral.')}
        </div>
      </Panel>
    </div>
  )
}

function chuteState(s: string): string {
  return ({ stowed: 'guardado', deployed: 'aberto', cut: 'cortado', failed: 'FALHA' } as Record<string, string>)[s] ?? s
}
