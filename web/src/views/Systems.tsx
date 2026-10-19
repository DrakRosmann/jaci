import { useState } from 'react'
import { Chart, COLORS } from '../components/Chart'
import { Bar, CmdButton, Empty, Panel, Tile, Tm, sendCommand } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { distance, duration, num, rate } from '../lib/format'
import { store, useStore } from '../lib/store'
import type { Vehicle } from '../types'

type Tab = 'eps' | 'tcs' | 'adcs' | 'com' | 'ecl' | 'pl'

export function SystemsView() {
  const v = useStore((s) => s.selected)
  useStore((s) => s.version)
  const [tab, setTab] = useState<Tab>('eps')
  if (!v || !v.systems) return <Empty>{t('Este veículo não tem subsistemas monitorados (detrito ou externo).')}</Empty>
  const tabs: [Tab, string, boolean][] = [
    ['eps', 'Energia (EPS)', true],
    ['tcs', 'Térmico (TCS)', true],
    ['adcs', 'Atitude (ADCS)', true],
    ['com', 'Comunicações', true],
    ['ecl', 'Suporte de vida (ECLSS)', v.systems.eclss != null],
    ['pl', 'Carga útil', v.systems.instruments.length > 0],
  ]
  return (
    <div className="grid">
      {v.safe_mode && (
        <div className="banner warn row">
          {t('Modo de segurança ativo: cargas não essenciais cortadas.')}
          <CmdButton vehicle={v.id} cmd="safe_mode" args={{ on: 'off' }} className="small">
            {t('Sair do modo de segurança')}
          </CmdButton>
        </div>
      )}
      <div className="row">
        {tabs
          .filter((x) => x[2])
          .map(([id, name]) => (
            <button key={id} className={tab === id ? 'active' : ''} onClick={() => setTab(id)}>
              {t(name)}
            </button>
          ))}
        <span className="spacer" />
        {!v.safe_mode && (
          <CmdButton vehicle={v.id} cmd="safe_mode" args={{ on: 'on' }}>
            {t('Modo de segurança')}
          </CmdButton>
        )}
      </div>
      {tab === 'eps' && <Eps v={v} />}
      {tab === 'tcs' && <Tcs v={v} />}
      {tab === 'adcs' && <Adcs v={v} />}
      {tab === 'com' && <Com v={v} />}
      {tab === 'ecl' && <Ecl v={v} />}
      {tab === 'pl' && <Payload v={v} />}
    </div>
  )
}

function plot(v: Vehicle, ids: [string, string][], hours = 3) {
  const since = store.snap!.t - hours * 3600
  return ids.map(([id, label], i) => {
    const [ts, vs] = store.history.series(v.id, id, since)
    return { label: t(label), color: COLORS[i], t: ts, v: vs }
  })
}

function Eps({ v }: { v: Vehicle }) {
  const s = v.systems!
  return (
    <div className="grid cols-2">
      <Panel title={t('Energia')}>
        <div className="tiles">
          <Tm v={v} id="eps.soc" digits={1} />
          <Tm v={v} id="eps.gen" digits={0} />
          <Tm v={v} id="eps.load" digits={0} />
          <Tm v={v} id="eps.bus" digits={2} />
          <Tm v={v} id="eps.battery" digits={0} />
          <Tm v={v} id="eps.sun_angle" digits={0} />
          <Tile label={t('Iluminado')} value={(v.tm?.['eps.sunlit'] ?? 0) > 0.5 ? t('sim') : t('eclipse')} />
          <Tile label={t('Capacidade')} value={num(s.battery_wh, 0)} unit="Wh" />
          {s.fuel_cell && <Tm v={v} id="eps.reactant" digits={0} />}
        </div>
        {s.brownout && <div className="banner alarm" style={{ marginTop: 8 }}>{t('SEM ENERGIA: bateria esgotada')}</div>}
        <div className="row" style={{ marginTop: 8 }}>
          {s.solar && (
            <>
              <span className="small">
                {t('Painéis')}: {s.solar.deployed ? t('abertos') : t('recolhidos')}
                {s.solar.tracking ? ` · ${t('acompanham o Sol')}` : ''}
                {s.solar.stuck ? ` · ${t('TRAVADOS')}` : ''}
                {s.solar.degraded > 0 ? ` · ${t('perda')} ${num(s.solar.degraded * 100, 0)} %` : ''}
              </span>
              {!s.solar.deployed && <CmdButton vehicle={v.id} cmd="solar_deploy">{t('Abrir painéis')}</CmdButton>}
            </>
          )}
          {s.fuel_cell && (
            <CmdButton vehicle={v.id} cmd="fuel_cell" args={{ on: s.fuel_cell.on ? 'off' : 'on' }}>
              {s.fuel_cell.on ? t('Desligar célula a combustível') : t('Ligar célula a combustível')}
            </CmdButton>
          )}
        </div>
      </Panel>
      <Panel title={t('Cargas elétricas')} flush>
        <table>
          <thead>
            <tr>
              <th>{t('Carga')}</th>
              <th className="num">W</th>
              <th>{t('Essencial')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {s.loads.map((l) => (
              <tr key={l.id}>
                <td>{tx(l.name)}</td>
                <td className="num">{num(l.watts, 0)}</td>
                <td>{l.essential ? '●' : ''}</td>
                <td>
                  <label>
                    <input type="checkbox" checked={l.on} onChange={(e) => sendCommand(v.id, 'load_switch', { load: l.id, on: e.target.checked ? 'on' : 'off' })} /> {l.on ? t('ligada') : t('desligada')}
                  </label>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Panel>
      <Panel title={t('Carga da bateria (%)')} flush>
        <Chart series={plot(v, [['eps.soc', 'Bateria']])} height={180} yMin={0} yMax={100} />
      </Panel>
      <Panel title={t('Geração × consumo (W)')} flush>
        <Chart series={plot(v, [['eps.gen', 'Geração'], ['eps.load', 'Consumo']])} height={180} />
      </Panel>
    </div>
  )
}

function Tcs({ v }: { v: Vehicle }) {
  const s = v.systems!
  return (
    <div className="grid cols-2">
      <Panel title={t('Nós térmicos')} flush>
        <table>
          <thead>
            <tr>
              <th>{t('Nó')}</th>
              <th className="num">°C</th>
              <th className="num">{t('Faixa')}</th>
              <th>{t('Aquecedor')}</th>
            </tr>
          </thead>
          <tbody>
            {s.thermal.map((n) => {
              const bad = n.temp < n.min || n.temp > n.max
              const warn = n.temp < n.min + 5 || n.temp > n.max - 5
              return (
                <tr key={n.id}>
                  <td>{tx(n.name)}</td>
                  <td className={`num ${bad ? 'bad' : warn ? 'warn' : ''}`}>{num(n.temp, 1)}</td>
                  <td className="num muted">
                    {num(n.min, 0)} … {num(n.max, 0)}
                  </td>
                  <td>
                    {n.heater_w > 0 ? (
                      <span className="row">
                        <select value={n.auto ? 'auto' : n.heater ? 'on' : 'off'} onChange={(e) => sendCommand(v.id, 'heater', { node: n.id, mode: e.target.value })}>
                          <option value="auto">{t('automático')}</option>
                          <option value="on">{t('ligado')}</option>
                          <option value="off">{t('desligado')}</option>
                        </select>
                        <span className={n.failed ? 'bad small' : n.heater ? 'warn small' : 'muted small'}>{n.failed ? t('FALHA') : n.heater ? `${num(n.heater_w, 0)} W` : ''}</span>
                      </span>
                    ) : (
                      <span className="muted">—</span>
                    )}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
        {s.has_radiator && (
          <div className="row" style={{ padding: 10 }}>
            <span>{t('Radiadores')}: {s.radiator ? t('ligados') : t('desligados')}</span>
            <CmdButton vehicle={v.id} cmd="radiator" args={{ on: s.radiator ? 'off' : 'on' }}>
              {s.radiator ? t('Desligar') : t('Ligar')}
            </CmdButton>
          </div>
        )}
      </Panel>
      <Panel title={t('Temperaturas (°C)')} flush>
        <Chart series={plot(v, s.thermal.map((n) => [`tcs.${n.id}`, n.name] as [string, string]))} height={260} />
      </Panel>
    </div>
  )
}

function Adcs({ v }: { v: Vehicle }) {
  const a = v.systems!.adcs
  const snap = useStore((s) => s.snap)!
  const [target, setTarget] = useState<number | ''>('')
  const others = snap.vehicles.filter((x) => x.id !== v.id && x.kind !== 'debris' && x.central === v.central)
  return (
    <div className="grid cols-2">
      <Panel title={t('Atitude')}>
        <div className="tiles">
          <Tile label={t('Modo')} value={tx(a.mode_name)} />
          <Tm v={v} id="adcs.error" digits={2} />
          <Tm v={v} id="adcs.rate" digits={3} />
          {a.wheels && <Tm v={v} id="adcs.wheels" digits={0} />}
          <Tile label={t('Propelente do RCS')} value={num(a.rcs_prop, 1)} unit="kg" />
          <Tile label={t('Rastreador de estrelas')} value={a.star_tracker ? t('ok') : t('cego')} state={a.star_tracker ? 'nominal' : 'red'} />
        </div>
        <div className="row" style={{ marginTop: 8 }}>
          <select value={a.mode} onChange={(e) => sendCommand(v.id, 'att_mode', { mode: e.target.value, target: target || undefined })}>
            {(store.hello?.commands.find((c) => c.id === 'att_mode')?.args[0].options ?? []).map(([id, name]) => (
              <option key={id} value={id}>
                {tx(name)}
              </option>
            ))}
            {a.mode === 'guidance' && <option value="guidance">{t('Guiamento')}</option>}
            {a.mode === 'inertial' && <option value="inertial">{t('Inercial')}</option>}
          </select>
          <select value={target} onChange={(e) => setTarget(e.target.value ? Number(e.target.value) : '')}>
            <option value="">{t('alvo (modo Alvo)')}</option>
            {others.map((x) => (
              <option key={x.id} value={x.id}>
                {x.name}
              </option>
            ))}
          </select>
          {a.wheels && (
            <CmdButton vehicle={v.id} cmd="desat" disabled={a.desat}>
              {a.desat ? t('Descarregando…') : t('Descarregar rodas')}
            </CmdButton>
          )}
        </div>
      </Panel>
      {a.wheels && (
        <Panel title={t('Rodas de reação: momento por eixo')}>
          {['X', 'Y', 'Z'].map((ax, i) => (
            <div key={ax} style={{ marginBottom: 6 }}>
              <div className="row small">
                {t('Eixo')} {ax}
                {a.wheels!.failed[i] && <span className="bad">{t('FALHA')}</span>}
                <span className="spacer" />
                <span className="mono">
                  {num(a.wheels!.h[i], 2)} / {num(a.wheels!.max, 0)} N·m·s
                </span>
              </div>
              <Bar frac={Math.abs(a.wheels!.h[i]) / a.wheels!.max} />
            </div>
          ))}
        </Panel>
      )}
      <Panel title={t('Erro de apontamento (°)')} flush>
        <Chart series={plot(v, [['adcs.error', 'Erro']], 1)} height={180} yMin={0} />
      </Panel>
      {a.wheels && (
        <Panel title={t('Saturação das rodas (%)')} flush>
          <Chart series={plot(v, [['adcs.wheels', 'Saturação']], 6)} height={180} yMin={0} yMax={100} />
        </Panel>
      )}
    </div>
  )
}

function Com({ v }: { v: Vehicle }) {
  const c = v.systems!.comms
  const l = c.link
  const [kbps, setKbps] = useState('')
  return (
    <div className="grid cols-2">
      <Panel title={t('Enlace')}>
        {c.blackout && <div className="banner warn">{t('Apagão de comunicações: plasma da reentrada')}</div>}
        {c.failed && <div className="banner alarm">{t('Transmissor em pane')}</div>}
        <div className="tiles">
          <Tile label={t('Estação')} value={l ? tx(l.station) : t('sem enlace')} state={l ? 'nominal' : 'yellow'} />
          <Tile label={t('Taxa')} value={rate(l?.rate_bps)} />
          <Tile label={t('Margem')} value={num(l?.margin_db, 1)} unit="dB" state={l ? (l.margin_db < 3 ? 'yellow' : 'nominal') : 'none'} />
          <Tile label={t('Elevação')} value={num(l?.elevation_deg, 1)} unit="°" />
          <Tile label={t('Distância')} value={distance(l ? l.range_m / 1000 : null)} />
          <Tile label={t('Atraso da luz')} value={l ? duration(l.owlt) : '—'} />
          <Tile label={t('Antena')} value={l ? (l.via_relay ? t('relé') : l.via_hga ? t('alto ganho') : t('baixo ganho')) : '—'} />
        </div>
        <div className="row" style={{ marginTop: 8 }}>
          <CmdButton vehicle={v.id} cmd="tx" args={{ on: c.tx_on ? 'off' : 'on' }}>
            {c.tx_on ? t('Desligar transmissor') : t('Ligar transmissor')}
          </CmdButton>
          {c.hga && !c.hga_deployed && <CmdButton vehicle={v.id} cmd="hga_deploy">{t('Abrir antena de alto ganho')}</CmdButton>}
          <select value={kbps} onChange={(e) => setKbps(e.target.value)}>
            <option value="">{t('taxa automática')}</option>
            {c.rates.map((r) => (
              <option key={r} value={r / 1000}>
                {rate(r)}
              </option>
            ))}
          </select>
          <button onClick={() => sendCommand(v.id, 'rate', kbps ? { kbps: Number(kbps) } : {})}>{t('Aplicar taxa')}</button>
        </div>
        <div className="small muted" style={{ marginTop: 6 }}>
          {c.auto_rate ? t('Taxa automática: a maior com 3 dB de margem.') : `${t('Taxa fixa')}: ${rate(c.rate)}`}
          {c.relay ? ` ${t('Com cobertura de satélites de retransmissão.')}` : ''}
        </div>
      </Panel>
      <Panel title={t('Gravador de bordo')}>
        <div className="row small">
          <span>
            {num(c.stored, 0)} / {num(c.storage, 0)} MB
          </span>
          <span className="spacer" />
          <span>{t('gerando')} {num(c.gen, 2)} Mbit/s</span>
        </div>
        <Bar frac={c.stored / (c.storage || 1)} />
        <div className="row" style={{ marginTop: 8 }}>
          <CmdButton vehicle={v.id} cmd="dump" disabled={c.dumping || !l}>
            {c.dumping ? t('Descarregando…') : t('Descarregar para o solo')}
          </CmdButton>
          {c.dumping && l && <span className="small">{t('tempo restante')} ≈ {duration((c.stored * 8e6) / (l.rate_bps * 0.9))}</span>}
        </div>
      </Panel>
      <Panel title={t('Margem do enlace (dB)')} flush>
        <Chart series={plot(v, [['com.margin', 'Margem']], 6)} height={180} />
      </Panel>
      <Panel title={t('Gravador ocupado (%)')} flush>
        <Chart series={plot(v, [['com.stored', 'Gravador']], 12)} height={180} yMin={0} yMax={100} />
      </Panel>
    </div>
  )
}

function Ecl({ v }: { v: Vehicle }) {
  const e = v.systems!.eclss!
  return (
    <div className="grid cols-2">
      <Panel title={t('Cabine')}>
        <div className="tiles">
          <Tm v={v} id="ecl.pressure" digits={1} />
          <Tm v={v} id="ecl.ppo2" digits={2} />
          <Tm v={v} id="ecl.ppco2" digits={3} />
          <Tm v={v} id="ecl.temp" digits={1} />
          <Tm v={v} id="ecl.o2_tank" digits={1} />
          <Tm v={v} id="ecl.n2_tank" digits={1} />
          <Tm v={v} id="ecl.water" digits={0} />
          <Tm v={v} id="ecl.crew" digits={0} />
          <Tm v={v} id="ecl.hr" digits={0} />
          {e.lioh && <Tm v={v} id="ecl.scrubber" digits={2} />}
        </div>
        <div className="row" style={{ marginTop: 8 }}>
          {e.has_o2_gen && (
            <label>
              <input type="checkbox" checked={e.o2_gen} onChange={(x) => sendCommand(v.id, 'o2_gen', { on: x.target.checked ? 'on' : 'off' })} /> {t('gerador de O₂')}
            </label>
          )}
          <label>
            <input type="checkbox" checked={e.scrubber} onChange={(x) => sendCommand(v.id, 'scrubber', { on: x.target.checked ? 'on' : 'off' })} /> {t('remoção de CO₂')}
          </label>
          <label>
            <input type="checkbox" checked={e.auto_press} onChange={(x) => sendCommand(v.id, 'auto_press', { on: x.target.checked ? 'on' : 'off' })} /> {t('repressurização automática')}
          </label>
        </div>
      </Panel>
      <Panel title={t('Módulos e escotilhas')} flush>
        <table>
          <thead>
            <tr>
              <th>{t('Módulo')}</th>
              <th className="num">m³</th>
              <th>{t('Estado')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {e.modules.map((m) => (
              <tr key={m.name}>
                <td>{tx(m.name)}</td>
                <td className="num">{num(m.volume, 0)}</td>
                <td>
                  {m.leak && <span className="bad">{t('VAZAMENTO')} </span>}
                  {m.isolated ? <span className="warn">{t('isolado')} ({num(m.pressure, 1)} kPa)</span> : <span className="muted">{t('aberto')}</span>}
                </td>
                <td>
                  <button className="small" onClick={() => sendCommand(v.id, 'hatch', { module: m.name, closed: m.isolated ? 'off' : 'on' })}>
                    {m.isolated ? t('Abrir escotilha') : t('Fechar escotilha')}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </Panel>
      <Panel title={t('Pressões (kPa)')} flush>
        <Chart series={plot(v, [['ecl.pressure', 'Total'], ['ecl.ppo2', 'O₂']], 6)} height={200} />
      </Panel>
      <Panel title={t('CO₂ (kPa)')} flush>
        <Chart series={plot(v, [['ecl.ppco2', 'CO₂']], 6)} height={200} yMin={0} />
      </Panel>
    </div>
  )
}

function Payload({ v }: { v: Vehicle }) {
  const s = v.systems!
  return (
    <Panel title={t('Instrumentos')} flush>
      <table>
        <thead>
          <tr>
            <th>{t('Instrumento')}</th>
            <th className="num">W</th>
            <th className="num">Mbit/s</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {s.instruments.map((i) => (
            <tr key={i.id}>
              <td>{tx(i.name)}</td>
              <td className="num">{num(i.watts, 0)}</td>
              <td className="num">{num(i.mbps, 2)}</td>
              <td>
                <label>
                  <input type="checkbox" checked={i.on} onChange={(e) => sendCommand(v.id, 'instrument', { id: i.id, on: e.target.checked ? 'on' : 'off' })} /> {i.on ? t('ligado') : t('desligado')}
                </label>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </Panel>
  )
}
