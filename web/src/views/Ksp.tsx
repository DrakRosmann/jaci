// Console do Kerbal Space Program: a ligação com o kOS e o controle das naves
// do jogo (voo, lançamento automático, atitude, manobras, sistemas e recursos).

import { useMemo, useState } from 'react'
import { Chart, COLORS } from '../components/Chart'
import { MapView, type Track } from '../components/MapView'
import { Bar, CmdButton, Empty, Panel, Tile, Tm, sendCommand, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { duration, kerbinTime, num } from '../lib/format'
import { usePrediction } from '../lib/hooks'
import { store, useStore } from '../lib/store'
import type { Vehicle } from '../types'

export function KspView() {
  const v = useStore((s) => s.selected)
  const ksp = useStore((s) => s.snap?.ksp ?? null)
  const vessels = useStore((s) => s.snap?.vehicles.filter((x) => x.kind === 'ksp') ?? [])
  const game = v?.kind === 'ksp' ? v : vessels[0]
  return (
    <div className="grid">
      <div className="grid cols-3">
        <Connection />
        <Panel title={t('Naves do KSP')} className="span-2" flush>
          {vessels.length === 0 ? (
            <Empty>{ksp ? t('Nenhuma nave com a ponte do kOS ainda. Veja os passos ao lado.') : t('Ligue o Jaci ao KSP para ver as naves.')}</Empty>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>{t('Nave')}</th>
                  <th>{t('Situação')}</th>
                  <th>{t('Corpo')}</th>
                  <th className="num">{t('Altitude')}</th>
                  <th className="num">{t('Periastro × apoastro')}</th>
                  <th>{t('Piloto automático')}</th>
                  <th className="num">{t('Sinal')}</th>
                </tr>
              </thead>
              <tbody>
                {vessels.map((x) => (
                  <tr key={x.id} className={x.id === game?.id ? 'sel' : ''} onClick={() => store.setUi({ selected: x.id })} style={{ cursor: 'pointer' }}>
                    <td>{x.name}</td>
                    <td>{tx(x.phase_name)}</td>
                    <td>{x.central_name}</td>
                    <td className="num">{num(x.alt, 1)} km</td>
                    <td className="num">{x.pe != null && x.ap != null ? `${num(x.pe, 1)} × ${num(x.ap, 1)}` : '—'}</td>
                    <td>{tx(modeName(x.ksp?.mode ?? ''))}</td>
                    <td className={`num ${(x.ksp?.age ?? 99) > 5 ? 'bad' : 'good'}`}>{(x.ksp?.age ?? 99) > 5 ? t('perdido') : t('ok')}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </Panel>
      </div>
      {game && <Vessel v={game} />}
    </div>
  )
}

function modeName(m: string): string {
  return ({ manual: 'Manual', ascent: 'Subida', coast: 'Saindo da atmosfera', node: 'Executando manobra' } as Record<string, string>)[m] ?? m
}

function Connection() {
  const ksp = useStore((s) => s.snap?.ksp ?? null)
  const [dir, setDir] = useState('')
  const connect = async () => {
    const r = await store.sim('ksp_connect', { value: dir })
    toast(r.ok ? tx(r.message) : tx(r.error), r.ok ? 'good' : 'bad')
  }
  const patch = async () => {
    const r = await store.sim('ksp_patch')
    toast(r.ok ? tx(r.message) : tx(r.error), r.ok ? 'good' : 'bad')
  }
  const ok = (b: boolean) => <span className={b ? 'good' : 'bad'}>{b ? '✓' : '✗'}</span>
  return (
    <Panel title={t('Ligação com o KSP (kOS)')}>
      {ksp ? (
        <div className="grid" style={{ overflowWrap: 'anywhere' }}>
          <div className="small">
            <div>
              {t('Pasta do KSP')}: <span className="mono">{ksp.dir}</span>
            </div>
            <div>
              {ok(ksp.kos)} kOS · {ok(ksp.module_manager)} Module Manager · {ok(ksp.patch)} {t('kOS em todas as peças de comando')}
            </div>
            <div>
              {t('Naves')}: {ksp.vessels} ({ksp.live} {t('com sinal')})
            </div>
          </div>
          {!ksp.kos && <div className="small bad">{t('Instale o kOS (pelo CKAN: ckan install kOS) e reinicie o KSP.')}</div>}
          {ksp.module_manager && !ksp.patch && (
            <button onClick={patch} title={t('Põe um computador kOS com o boot do Jaci em toda cápsula e sonda que ainda não tem')}>
              {t('Pôr o kOS em todas as peças de comando')}
            </button>
          )}
          <ol className="small" style={{ margin: 0, paddingLeft: 18 }}>
            <li>{t('No VAB, use uma nave com computador kOS (ou com o patch acima, qualquer cápsula ou sonda).')}</li>
            <li>
              {t('No menu da peça kOS, escolha o arquivo de boot')} <span className="mono">jaci.ks</span>.
            </li>
            <li>
              {t('Ou, no terminal do kOS, digite')} <span className="mono">RUNPATH("0:/jaci/jaci.ks").</span>
            </li>
            <li>{t('A nave aparece aqui em segundos. Sem conexão com o KSC (CommNet) não há telemetria nem comandos.')}</li>
          </ol>
        </div>
      ) : (
        <div className="grid">
          <div className="small muted">{t('O Jaci conversa com o kOS por arquivos na pasta Ships/Script do KSP. Deixe em branco para procurar a pasta do Steam.')}</div>
          <input value={dir} onChange={(e) => setDir(e.target.value)} placeholder={t('pasta do Kerbal Space Program')} />
          <button className="primary" onClick={connect}>
            {t('Ligar ao KSP')}
          </button>
        </div>
      )}
    </Panel>
  )
}

function Vessel({ v }: { v: Vehicle }) {
  const k = v.ksp!
  const lost = (k.age ?? 99) > 5
  const period = (v.tm?.['orb.period'] ?? NaN) * 60
  const span = isFinite(period) && period > 0 && (v.pe ?? -1) > 0 ? Math.min(period * 1.5, 86_400 * 5) : 1800
  const pred = usePrediction(v.phase !== 'prelaunch' && v.phase !== 'landed' ? v.id : null, span, 4000)
  const tracks = useMemo<Track[]>(() => (pred.points.length ? [{ vehicle: v.id, points: pred.points.map((p) => [p[2], p[1]] as [number, number]), color: '#4cc3ff' }] : []), [pred, v.id])
  return (
    <>
      {lost && <div className="banner alarm">{t('Sem sinal do kOS')}: {v.name} — {t('a nave saiu de foco, perdeu a conexão com o KSC ou o script parou')}</div>}
      {k.countdown && (
        <div className="banner warn" style={{ fontSize: 22, textAlign: 'center' }}>
          T−{Math.ceil(k.countdown.t_minus)} s · {t('lançamento automático para')} {num(k.countdown.apo, 0)} km, {t('rumo')} {num(k.countdown.heading, 0)}°
        </div>
      )}
      <div className="status-strip">
        <span>{tx(v.phase_name)}</span>
        <span>{v.central_name}</span>
        <span>{t('Piloto automático')}: {tx(modeName(k.mode))}</span>
        <span>{t('Atitude')}: {tx(steerName(k.steer))}</span>
        <span>{t('Estágio')} {k.stage ?? '—'}</span>
        {k.target && k.target !== '-' && <span>{t('Alvo')}: {k.target}</span>}
        <span className="mono">{kerbinTime(k.ut)}</span>
      </div>
      <div className="grid cols-3">
        <Panel title={`${t('Voo')} · ${v.name}`} className="span-2">
          <div className="tiles">
            <Tm v={v} id="nav.alt" digits={2} />
            <Tm v={v} id="nav.vspeed" digits={1} />
            <Tm v={v} id="nav.speed" digits={1} />
            <Tm v={v} id="nav.hspeed" digits={1} />
            <Tm v={v} id="orb.ap" digits={2} />
            <Tm v={v} id="orb.pe" digits={2} />
            <Tile label={t('Tempo até o apoastro')} value={duration(v.tm?.['orb.eta_ap'])} />
            <Tile label={t('Tempo até o periastro')} value={duration(v.tm?.['orb.eta_pe'])} />
            <Tm v={v} id="orb.inc" digits={2} />
            <Tm v={v} id="orb.period" digits={1} />
            <Tm v={v} id="nav.q" digits={2} />
            <Tm v={v} id="nav.accel" digits={2} />
            <Tm v={v} id="nav.twr" digits={2} />
            <Tm v={v} id="nav.mass" digits={2} />
            <Tm v={v} id="prop.thrust" digits={0} />
            <Tm v={v} id="prop.throttle" digits={0} />
            <Tm v={v} id="nav.pitch" digits={1} />
            <Tm v={v} id="nav.heading" digits={1} />
          </div>
        </Panel>
        <FlightControls v={v} />
      </div>
      <div className="grid cols-3">
        <Panel title={`${t('Mapa')} · ${v.central_name}`} className="span-2" flush>
          <MapView tracks={tracks} height={380} body={v.central} />
        </Panel>
        <AutoLaunch v={v} />
      </div>
      <div className="grid cols-3">
        <KspAttitude v={v} />
        <KspManeuvers v={v} />
        <Systems v={v} />
      </div>
      <div className="grid cols-3">
        <Resources v={v} />
        <Plot v={v} ids={[['nav.alt', 'Altitude'], ['orb.ap', 'Apoastro'], ['orb.pe', 'Periastro']]} unit="km" />
        <Plot v={v} ids={[['nav.speed', 'Velocidade orbital'], ['nav.hspeed', 'Velocidade em relação à superfície'], ['nav.vspeed', 'Velocidade vertical']]} unit="m/s" />
      </div>
    </>
  )
}

function Plot({ v, ids, unit }: { v: Vehicle; ids: [string, string][]; unit: string }) {
  const since = store.snap!.t - 1800
  const series = ids.map(([id, label], i) => {
    const [ts, vs] = store.history.series(v.id, id, since)
    return { label: t(label), color: COLORS[i], t: ts, v: vs }
  })
  return (
    <Panel title={t(ids[0][1])} flush>
      <Chart series={series} height={200} unit={unit} />
    </Panel>
  )
}

function FlightControls({ v }: { v: Vehicle }) {
  const [thr, setThr] = useState<number | null>(null)
  const current = v.tm?.['prop.throttle'] ?? 0
  const shown = thr ?? current
  const setThrottle = (x: number) => {
    setThr(null)
    sendCommand(v.id, 'ksp_throttle', { value: x })
  }
  return (
    <Panel title={t('Controles de voo')}>
      <div className="grid">
        <div className="row">
          <CmdButton vehicle={v.id} cmd="ksp_stage" className="primary">
            {t('Separar estágio')}
          </CmdButton>
          <CmdButton vehicle={v.id} cmd="ksp_abort">
            {t('ABORTO')}
          </CmdButton>
          <CmdButton vehicle={v.id} cmd="ksp_stop">
            {t('Parar o piloto automático')}
          </CmdButton>
        </div>
        <div>
          <div className="small muted">
            {t('Potência')}: {num(shown, 0)} %
          </div>
          <input
            type="range"
            min={0}
            max={100}
            step={1}
            value={shown}
            onChange={(e) => setThr(Number(e.target.value))}
            onPointerUp={() => thr != null && setThrottle(thr)}
            onKeyUp={() => thr != null && setThrottle(thr)}
            style={{ width: '100%' }}
          />
          <div className="row">
            {[0, 25, 50, 75, 100].map((x) => (
              <button key={x} className="small" onClick={() => setThrottle(x)}>
                {x}%
              </button>
            ))}
          </div>
        </div>
        <div>
          <div className="small muted">{t('Aceleração do tempo do KSP')}</div>
          <div className="row">
            {[0, 1, 2, 3, 4, 5, 6, 7].map((x) => (
              <CmdButton key={x} vehicle={v.id} cmd="ksp_warp" args={{ level: x }} className={`small ${v.ksp?.warp === x ? 'active' : ''}`}>
                {x === 0 ? '1×' : `${x}`}
              </CmdButton>
            ))}
          </div>
        </div>
      </div>
    </Panel>
  )
}

function AutoLaunch({ v }: { v: Vehicle }) {
  const [apo, setApo] = useState('80')
  const [hdg, setHdg] = useState('90')
  const [count, setCount] = useState('10')
  const n = (s: string) => Number(s.replace(',', '.'))
  const args = { apo: n(apo), heading: n(hdg) }
  const onGround = ['prelaunch', 'landed'].includes(v.phase)
  return (
    <Panel title={t('Lançamento automático')}>
      <div className="small muted" style={{ marginBottom: 8 }}>
        {t('O kOS acende o primeiro estágio, faz a curva de gravidade, separa os estágios vazios, corta no apoastro alvo e circulariza fora da atmosfera.')}
      </div>
      <div className="form">
        <label>{t('Apoastro alvo')} (km)</label>
        <input value={apo} onChange={(e) => setApo(e.target.value)} />
        <label>{t('Rumo')} (°)</label>
        <input value={hdg} onChange={(e) => setHdg(e.target.value)} title={t('90° = leste, órbita equatorial no sentido da rotação')} />
        <label>{t('Contagem')} (s)</label>
        <input value={count} onChange={(e) => setCount(e.target.value)} />
      </div>
      <div className="row" style={{ marginTop: 10 }}>
        {v.ksp?.countdown ? (
          <CmdButton vehicle={v.id} cmd="ksp_count_abort" className="danger">
            {t('Cancelar a contagem')}
          </CmdButton>
        ) : (
          <CmdButton vehicle={v.id} cmd="ksp_count" args={{ ...args, t_minus: n(count) }} disabled={!onGround}>
            {t('Iniciar contagem')}
          </CmdButton>
        )}
        <CmdButton vehicle={v.id} cmd="ksp_launch" args={args} disabled={!onGround && v.phase !== 'ascent'}>
          {t('Lançar agora')}
        </CmdButton>
      </div>
      {!onGround && <div className="small muted" style={{ marginTop: 6 }}>{t('A nave não está na plataforma.')}</div>}
    </Panel>
  )
}

const STEER: [string, string][] = [
  ['prograde', 'Prógrado'],
  ['retrograde', 'Retrógrado'],
  ['normal', 'Normal'],
  ['antinormal', 'Antinormal'],
  ['radial_out', 'Radial para fora'],
  ['radial_in', 'Radial para dentro'],
  ['up', 'Para cima'],
  ['hold', 'Manter'],
  ['sun', 'Apontado ao Sol'],
  ['target', 'Alvo'],
  ['free', 'Livre'],
]

function steerName(s: string): string {
  if (s === 'heading') return 'Rumo e arfagem'
  return STEER.find(([id]) => id === s)?.[1] ?? s
}

export function KspAttitude({ v }: { v: Vehicle }) {
  const k = v.ksp!
  const [hdg, setHdg] = useState('90')
  const [pitch, setPitch] = useState('45')
  return (
    <Panel title={t('Atitude')}>
      <div className="row">
        {STEER.map(([id, name]) => (
          <CmdButton key={id} vehicle={v.id} cmd="ksp_steer" args={{ mode: id }} className={`small ${k.steer === id ? 'active' : ''}`}>
            {t(name)}
          </CmdButton>
        ))}
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <span className="small muted">{t('Rumo')}</span>
        <input value={hdg} onChange={(e) => setHdg(e.target.value)} style={{ width: 60 }} />
        <span className="small muted">{t('Arfagem')}</span>
        <input value={pitch} onChange={(e) => setPitch(e.target.value)} style={{ width: 60 }} />
        <CmdButton vehicle={v.id} cmd="ksp_steer" args={{ mode: 'heading', heading: Number(hdg), pitch: Number(pitch) }} className={`small ${k.steer === 'heading' ? 'active' : ''}`}>
          {t('Apontar')}
        </CmdButton>
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <CmdButton vehicle={v.id} cmd="ksp_sas" args={{ on: k.sas ? 'off' : 'on' }} className={`small ${k.sas ? 'active' : ''}`}>
          SAS {k.sas ? t('ligado') : t('desligado')}
        </CmdButton>
        <CmdButton vehicle={v.id} cmd="ksp_rcs" args={{ on: k.rcs ? 'off' : 'on' }} className={`small ${k.rcs ? 'active' : ''}`}>
          RCS {k.rcs ? t('ligado') : t('desligado')}
        </CmdButton>
      </div>
    </Panel>
  )
}

export function KspManeuvers({ v }: { v: Vehicle }) {
  const [dt, setDt] = useState('120')
  const [pro, setPro] = useState('100')
  const [nor, setNor] = useState('0')
  const [rad, setRad] = useState('0')
  const n = (s: string) => Number(s.replace(',', '.'))
  const eta = v.tm?.['node.eta']
  const dv = v.tm?.['node.dv']
  const acc = (v.tm?.['prop.max_thrust'] ?? 0) / Math.max(1e-9, v.tm?.['nav.mass'] ?? 1)
  return (
    <Panel title={t('Manobras')}>
      <div className="tiles">
        <Tile label={t('Próxima manobra em')} value={eta != null ? duration(eta) : '—'} />
        <Tile label={t('Δv da manobra')} value={num(dv, 1)} unit="m/s" />
        <Tile label={t('Queima estimada')} value={dv != null && acc > 0 ? duration(dv / acc) : '—'} />
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <CmdButton vehicle={v.id} cmd="ksp_circ" args={{ at: 'ap' }} className="small">
          {t('Circularizar no apoastro')}
        </CmdButton>
        <CmdButton vehicle={v.id} cmd="ksp_circ" args={{ at: 'pe' }} className="small">
          {t('Circularizar no periastro')}
        </CmdButton>
      </div>
      <div className="form" style={{ marginTop: 8 }}>
        <label>{t('Daqui a')} (s)</label>
        <input value={dt} onChange={(e) => setDt(e.target.value)} />
        <label>{t('Prógrado')} (m/s)</label>
        <input value={pro} onChange={(e) => setPro(e.target.value)} />
        <label>{t('Normal')} (m/s)</label>
        <input value={nor} onChange={(e) => setNor(e.target.value)} />
        <label>{t('Radial')} (m/s)</label>
        <input value={rad} onChange={(e) => setRad(e.target.value)} />
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <CmdButton vehicle={v.id} cmd="ksp_node" args={{ t: n(dt), prograde: n(pro), normal: n(nor), radial: n(rad) }} className="small">
          {t('Criar manobra')}
        </CmdButton>
        <CmdButton vehicle={v.id} cmd="ksp_exec" className={`small primary ${v.ksp?.mode === 'node' ? 'active' : ''}`} disabled={dv == null}>
          {t('Executar a próxima manobra')}
        </CmdButton>
        <CmdButton vehicle={v.id} cmd="ksp_delnodes" className="small" disabled={dv == null}>
          {t('Apagar manobras')}
        </CmdButton>
      </div>
    </Panel>
  )
}

function Systems({ v }: { v: Vehicle }) {
  const k = v.ksp!
  const [target, setTarget] = useState('')
  const toggle = (cmd: string, on: boolean, label: string) => (
    <CmdButton vehicle={v.id} cmd={cmd} className={`small ${on ? 'active' : ''}`}>
      {t(label)} {on ? '●' : '○'}
    </CmdButton>
  )
  return (
    <Panel title={t('Sistemas')}>
      <div className="row">
        {toggle('ksp_gear', k.gear, 'Trem de pouso')}
        {toggle('ksp_lights', k.lights, 'Luzes')}
        {toggle('ksp_brakes', k.brakes, 'Freios')}
        <CmdButton vehicle={v.id} cmd="ksp_chutes" className="small">
          {t('Paraquedas')}
        </CmdButton>
        <CmdButton vehicle={v.id} cmd="ksp_panels" args={{ on: 'on' }} className="small">
          {t('Abrir painéis')}
        </CmdButton>
        <CmdButton vehicle={v.id} cmd="ksp_panels" args={{ on: 'off' }} className="small">
          {t('Fechar painéis')}
        </CmdButton>
      </div>
      <div className="small muted" style={{ marginTop: 8 }}>{t('Grupos de ação')}</div>
      <div className="row">
        {[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map((x) => (
          <CmdButton key={x} vehicle={v.id} cmd="ksp_ag" args={{ n: x }} className="small">
            {x}
          </CmdButton>
        ))}
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <input value={target} onChange={(e) => setTarget(e.target.value)} placeholder={t('nome do alvo (nave ou corpo)')} style={{ flex: 1 }} />
        <CmdButton vehicle={v.id} cmd="ksp_target" args={{ name: target }} className="small" disabled={!target.trim()}>
          {t('Escolher alvo')}
        </CmdButton>
      </div>
      {v.tm?.['tgt.dist'] != null && <div className="small" style={{ marginTop: 6 }}>{t('Distância ao alvo')}: {num(v.tm['tgt.dist'], 2)} km</div>}
    </Panel>
  )
}

const RES_NAME: Record<string, string> = {
  LiquidFuel: 'Combustível líquido',
  Oxidizer: 'Oxidante',
  MonoPropellant: 'Monopropelente',
  ElectricCharge: 'Carga elétrica',
  SolidFuel: 'Combustível sólido',
}

function Resources({ v }: { v: Vehicle }) {
  const list = (v.ksp?.resources ?? []).filter((r) => (r.capacity ?? 0) > 0)
  return (
    <Panel title={t('Recursos')}>
      {list.length === 0 && <Empty>{t('Sem recursos a bordo.')}</Empty>}
      {list.map((r) => (
        <div key={r.name} style={{ marginBottom: 8 }}>
          <div className="row small">
            <span>{t(RES_NAME[r.name] ?? r.name)}</span>
            <span className="muted" style={{ marginLeft: 'auto' }}>
              {num(r.amount, 1)} / {num(r.capacity, 1)}
            </span>
          </div>
          <Bar frac={(r.amount ?? 0) / (r.capacity ?? 1)} kind="fuel" />
        </div>
      ))}
    </Panel>
  )
}
