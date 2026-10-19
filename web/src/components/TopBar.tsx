import { t, tx } from '../lib/i18n'
import { met } from '../lib/format'
import { store, useStore } from '../lib/store'

const WARPS = [1, 10, 100, 1_000, 10_000, 100_000]

export function TopBar() {
  const snap = useStore((s) => s.snap)
  const connected = useStore((s) => s.connected)
  const lang = useStore((s) => s.ui.lang)
  const sel = useStore((s) => s.selected)
  const selected = useStore((s) => s.ui.selected)
  const vehicles = snap?.vehicles.filter((v) => v.kind !== 'debris') ?? []
  const debris = snap?.vehicles.filter((v) => v.kind === 'debris') ?? []
  const lc = snap?.launch
  const counting = lc && lc.vehicle != null && lc.state !== 'launched' && lc.state !== 'idle'
  const alarms = snap?.alarms.filter((a) => a.active && !a.acked) ?? []
  const red = alarms.some((a) => a.level === 'red')
  return (
    <header className="topbar">
      <div className="brand">
        <img src="/jaci.svg" alt="" />
        <div>
          JACI
          <br />
          <small>{tx(snap?.mission) || t('Controle de missão')}</small>
        </div>
      </div>
      <div className="clock">
        <span>UTC</span>
        <b>{snap?.utc.replace(' UTC', '') ?? '—'}</b>
      </div>
      <div className="clock">
        <span>{t('Tempo de missão')}</span>
        <b>{met(sel?.met ?? null)}</b>
      </div>
      {counting && (
        <div className="clock countdown">
          <span>{lc.state === 'hold' ? t('Em espera') : lc.state === 'scrubbed' ? t('Cancelado') : t('Contagem')}</span>
          <b>{lc.clock}</b>
        </div>
      )}
      <div className="spacer" />
      <select value={selected ?? ''} onChange={(e) => store.setUi({ selected: Number(e.target.value) })} title={t('Veículo selecionado')}>
        {vehicles.map((v) => (
          <option key={v.id} value={v.id}>
            {tx(v.name)} · {tx(v.phase_name)}
          </option>
        ))}
        {debris.length > 0 && (
          <optgroup label={t('Detritos e estágios')}>
            {debris.map((v) => (
              <option key={v.id} value={v.id}>
                {v.name} · {tx(v.phase_name)}
              </option>
            ))}
          </optgroup>
        )}
      </select>
      <div className="warp">
        <button className={`small ${snap?.paused ? 'active' : ''}`} onClick={() => store.sim(snap?.paused ? 'resume' : 'pause')} title={t('Pausar ou continuar a simulação')}>
          {snap?.paused ? '▶' : '❚❚'}
        </button>
        {WARPS.map((w) => (
          <button key={w} className={`small ${snap?.warp === w ? 'active' : ''}`} onClick={() => store.sim('warp', { value: w })}>
            {w >= 1000 ? `${w / 1000}k` : w}×
          </button>
        ))}
        {snap && !WARPS.includes(snap.warp) && <span className="mono small">{snap.warp}×</span>}
      </div>
      <button className={`small ${alarms.length ? (red ? 'danger' : '') : ''}`} onClick={() => store.setUi({ view: 'telemetry' })} title={t('Alarmes não reconhecidos')}>
        <span className={`dot ${alarms.length ? (red ? 'bad' : 'warn') : 'good'}`} /> {alarms.length} {t('alarmes')}
      </button>
      <span title={connected ? t('Conectado') : t('Sem conexão')}>
        <span className={`dot ${connected ? 'good' : 'bad'}`} />
      </span>
      <button className="small" onClick={() => store.setUi({ lang: lang === 'pt' ? 'en' : 'pt' })} title="Português / English">
        {lang === 'pt' ? 'EN' : 'PT'}
      </button>
    </header>
  )
}
