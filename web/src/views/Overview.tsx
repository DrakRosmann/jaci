import { useMemo } from 'react'
import { MapView, kindColor, type Track } from '../components/MapView'
import { Panel, Tile, Tm } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { distance, duration, latlon, met, num, rate } from '../lib/format'
import { usePrediction } from '../lib/hooks'
import { store, useStore } from '../lib/store'
import type { Vehicle } from '../types'

/** Quanto prever para o traço: uma volta e meia, ou até o impacto. */
export function trackSpan(v: Vehicle | undefined): number {
  if (!v) return 6000
  const period = (v.tm?.['orb.period'] ?? 0) * 60
  if (v.central !== 'earth' && v.kind !== 'ksp') return 86_400 * 3
  if (period > 0 && isFinite(period) && (v.pe ?? -1) > 0) return Math.min(period * 1.5, 86_400 * 2)
  return 3000
}

export function Overview() {
  const v = useStore((s) => s.selected)
  const snap = useStore((s) => s.snap)!
  const span = trackSpan(v)
  // Nave do KSP: o mapa é o do corpo em que ela está.
  const body = v?.kind === 'ksp' ? v.central : 'earth'
  const pred = usePrediction(v && v.central === body && !['prelaunch', 'landed'].includes(v.phase) ? v.id : null, span, 5000)
  const tracks = useMemo<Track[]>(
    () => (v && pred.points.length ? [{ vehicle: v.id, points: pred.points.map((p) => [p[2], p[1]] as [number, number]), color: '#4cc3ff' }] : []),
    [pred, v?.id],
  )
  const fleet = snap.vehicles.filter((x) => x.kind !== 'debris')
  return (
    <div className="grid">
      {v?.phase === 'destroyed' && <div className="banner alarm">{t('Veículo perdido')}: {v.name}</div>}
      {v?.safe_mode && <div className="banner warn">{t('Modo de segurança ativo')}: {v.name}</div>}
      <div className="grid cols-3">
        <Panel title={t('Mapa')} className="span-2" flush>
          <MapView tracks={tracks} height={460} body={body} />
        </Panel>
        <Panel title={v ? `${v.name} · ${tx(v.kind_name)}` : t('Veículo')}>{v && <Summary v={v} />}</Panel>
      </div>
      <Panel title={t('Frota')} flush>
        <table>
          <thead>
            <tr>
              <th>{t('Veículo')}</th>
              <th>{t('Fase')}</th>
              <th>{t('Corpo')}</th>
              <th className="num">{t('Altitude')}</th>
              <th className="num">{t('Velocidade')}</th>
              <th className="num">{t('Periastro × apoastro')}</th>
              <th>{t('Enlace')}</th>
              <th className="num">{t('Tripulação')}</th>
              <th className="num">{t('Alarmes')}</th>
            </tr>
          </thead>
          <tbody>
            {fleet.map((x) => {
              const link = x.systems?.comms.link
              const al = snap.alarms.filter((a) => a.vehicle === x.id && a.active).length
              return (
                <tr key={x.id} className={x.id === v?.id ? 'sel' : ''} onClick={() => store.setUi({ selected: x.id })} style={{ cursor: 'pointer' }}>
                  <td>
                    <span className="dot" style={{ background: kindColor(x.kind) }} /> {x.name}
                  </td>
                  <td>{tx(x.phase_name)}</td>
                  <td>{tx(x.central_name ?? x.central)}</td>
                  <td className="num">{distance(x.alt)}</td>
                  <td className="num">{num(x.speed, 0)} m/s</td>
                  <td className="num">{x.pe != null && x.ap != null && x.phase !== 'prelaunch' ? `${num(x.pe, 0)} × ${num(x.ap, 0)}` : '—'}</td>
                  <td>{link ? `${link.station}` : x.systems ? <span className="muted">{t('sem enlace')}</span> : '—'}</td>
                  <td className="num">{x.crew || '—'}</td>
                  <td className={`num ${al ? 'bad' : ''}`}>{al || '—'}</td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </Panel>
    </div>
  )
}

function Summary({ v }: { v: Vehicle }) {
  const snap = useStore((s) => s.snap)!
  const link = v.systems?.comms.link
  const inAir = (v.central === 'earth' && v.alt < 150) || (v.kind === 'ksp' && v.phase === 'ascent')
  const orbiting = ['orbit', 'escape', 'docked'].includes(v.phase)
  const next = v.nodes?.find((n) => !n.done)
  return (
    <div className="grid">
      <div className="status-strip">
        <span>{tx(v.phase_name)}</span>
        {v.guidance && v.guidance.program !== 'none' && <span>{t('Programa')}: {tx(programName(v.guidance.program))}</span>}
        {v.systems && <span>{t('Atitude')}: {tx(v.systems.adcs.mode_name)}</span>}
        {v.crew > 0 && <span>{t('Tripulação')}: {v.crew}</span>}
        {v.attached_to != null && <span>{t('Acoplado a')} {store.vehicle(v.attached_to)?.name}</span>}
        {v.tle && (
          <span className={Math.abs(v.tle.age) > 7 ? 'warn' : ''}>
            NORAD {v.tle.norad} · TLE {num(v.tle.age, 1)} {t('dias')}
          </span>
        )}
      </div>
      <div className="tiles">
        <Tile label={t('Tempo de missão')} value={met(v.met ?? null)} />
        <Tile label={t('Posição')} value={v.central === 'earth' || v.kind === 'ksp' ? latlon(v.lat, v.lon) : `${latlon(v.lat, v.lon)} · ${tx(v.central_name ?? v.central)}`} title={tx(v.central_name)} />
        <Tile label={t('Altitude')} value={distance(v.alt)} />
        <Tile label={t('Velocidade')} value={num(v.speed, 0)} unit="m/s" />
        {v.tm && (inAir || v.phase === 'ascent' || v.phase === 'entry' || v.phase === 'descent') && (
          <>
            <Tm v={v} id="nav.vspeed" label={t('Vel. vertical')} digits={0} />
            <Tm v={v} id="nav.mach" label="Mach" digits={2} />
            <Tm v={v} id="nav.q" label={t('Pressão dinâmica')} digits={1} />
            <Tm v={v} id="nav.accel" label={t('Aceleração')} digits={2} />
          </>
        )}
        {v.tm && orbiting && (
          <>
            <Tm v={v} id="orb.pe" label={t('Periastro')} digits={0} />
            <Tm v={v} id="orb.ap" label={t('Apoastro')} digits={0} />
            <Tm v={v} id="orb.inc" label={t('Inclinação')} digits={2} />
            <Tm v={v} id="orb.period" label={t('Período')} digits={1} />
          </>
        )}
        {v.tm?.['nav.dv'] != null && <Tm v={v} id="nav.dv" label={t('Δv restante')} digits={0} />}
        {v.tm?.['eps.soc'] != null && <Tm v={v} id="eps.soc" label={t('Bateria')} digits={0} />}
        {v.systems && <Tile label={t('Enlace')} value={link ? rate(link.rate_bps) : t('sem enlace')} state={link ? 'nominal' : v.phase === 'prelaunch' ? 'none' : 'yellow'} title={link?.station} />}
        {link && link.owlt > 1 && <Tile label={t('Atraso da luz')} value={duration(link.owlt)} />}
        {v.tm?.['ecl.pressure'] != null && <Tm v={v} id="ecl.pressure" label={t('Cabine')} digits={1} />}
        {v.heat_shield && v.heat_shield.flux > 1 && <Tile label={t('Fluxo de calor')} value={num(v.heat_shield.flux, 0)} unit="kW/m²" />}
      </div>
      {next && (
        <div className="small">
          {t('Próxima queima')}: <b>{tx(next.label)}</b> — {num(next.dv_total, 1)} m/s {next.executing ? t('em execução') : `${t('em')} ${duration(next.t - snap.t)}`}
        </div>
      )}
      {v.iip && (
        <div className={`small ${v.iip.inside ? '' : 'bad'}`}>
          {t('Ponto de impacto instantâneo')}: {latlon(v.iip.lat, v.iip.lon)} ({num(v.iip.cross_km, 0)} km {t('do eixo do corredor')})
        </div>
      )}
      {v.guidance?.landing_target && <div className="small">{t('Alvo de pouso')}: {latlon(v.guidance.landing_target[0], v.guidance.landing_target[1])}</div>}
    </div>
  )
}

export function programName(p: string): string {
  return (
    {
      ascent: 'Subida',
      sounding: 'Sondagem',
      entry: 'Reentrada',
      docking: 'Acoplamento',
      rtls: 'Retorno ao local',
      powereddescent: 'Descida propulsada',
      none: 'Nenhum',
    } as Record<string, string>
  )[p] ?? p
}
