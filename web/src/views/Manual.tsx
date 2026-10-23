// Controle manual: dois MFDs (horizonte artificial, mapa orbital, recursos,
// a nave em 3D, mapa), o painel de chaves e botões de comando e o quadro de
// avisos, como nos consoles reais. Pilotagem pelo teclado: W/S arfagem, A/D
// guinada, Q/E rolamento; I/K prógrado/retrógrado, J/L radial, H/N normal.

import { useEffect, useState, type ReactNode } from 'react'
import { Mfd } from '../components/Mfd'
import { CmdButton, Empty, sendCommand } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { num } from '../lib/format'
import { PILOT_KEYS, pilot } from '../lib/pilot'
import { canCommand } from '../lib/room'
import { store, useStore } from '../lib/store'
import type { Vehicle } from '../types'

export function Manual() {
  const v = useStore((s) => s.selected)
  const snap = useStore((s) => s.snap)!
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  return (
    <div className="manual">
      <Annunciators v={v} />
      <div className="manual-grid">
        <Mfd v={v} initial="adi" height={460} />
        <Mfd v={v} initial="orb" height={460} />
        <ControlPanel v={v} replay={snap.replay != null} />
      </div>
    </div>
  )
}

/** Quadro de avisos: luzes que acendem com as condições, como nos painéis de cautela e alerta. */
function Annunciators({ v }: { v: Vehicle }) {
  const alarms = useStore((s) => (s.snap?.alarms ?? []).filter((a) => a.vehicle === v.id && a.active))
  const tm = (k: string) => v.tm?.[k]
  const engines = (v.stages ?? []).flatMap((s) => s.engines ?? [])
  const prop = (v.stages ?? []).filter((s) => !s.spacecraft && (s.capacity ?? 0) > 0)
  const lights: [string, 'red' | 'amber' | 'green' | 'off'][] = [
    [t('ALARME MESTRE'), alarms.some((a) => a.level === 'red' && !a.acked) ? 'red' : alarms.some((a) => !a.acked) ? 'amber' : 'off'],
    [t('FALHA MOTOR'), engines.some((e) => e.state === 'failed') ? 'red' : 'off'],
    [t('MOTOR ACESO'), engines.some((e) => e.state === 'running') ? 'green' : 'off'],
    [t('COMB BAIXO'), prop.some((s) => (s.prop ?? 0) / (s.capacity || 1) < 0.1 && (s.prop ?? 0) > 0) ? 'amber' : 'off'],
    [t('BATERIA'), (tm('eps.soc') ?? 100) < 30 ? 'amber' : 'off'],
    [t('PRESS CABINE'), tm('ecl.pressure') != null && (tm('ecl.pressure') ?? 101) < 95 ? 'red' : 'off'],
    [t('SEM ENLACE'), v.systems && !v.systems.comms.link ? 'amber' : 'off'],
    [t('MODO SEGURO'), v.safe_mode ? 'amber' : 'off'],
    [t('TERM ARMADA'), v.fts?.armed ? 'red' : 'off'],
    [t('GUIAMENTO'), v.guidance?.program && v.guidance.program !== 'none' ? 'green' : 'off'],
    // Só para quem tem RCS (satélites com rodas de reação não têm propelente de RCS).
    [t('RCS BAIXO'), v.systems && v.systems.adcs.rcs_prop > 0 && v.systems.adcs.rcs_prop < 5 ? 'amber' : 'off'],
    [t('CALOR'), v.heat_shield && v.heat_shield.flux > 100 ? 'amber' : 'off'],
  ]
  return (
    <div className="annunciators" role="status" aria-label={t('Quadro de avisos')}>
      {lights.map(([label, st]) => (
        <span key={label} className={`ann ${st}`}>
          {label}
        </span>
      ))}
    </div>
  )
}

function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <fieldset className="ctl-group">
      <legend>{title}</legend>
      {children}
    </fieldset>
  )
}

/** Chave com tampa: levanta a tampa e só então o botão (perigoso) fica à mão. */
function Guarded({ children, label }: { children: ReactNode; label: string }) {
  const [open, setOpen] = useState(false)
  useEffect(() => {
    if (!open) return
    const h = setTimeout(() => setOpen(false), 8000)
    return () => clearTimeout(h)
  }, [open])
  return open ? (
    <span className="guarded open">{children}</span>
  ) : (
    <button className="guard-cover" onClick={() => setOpen(true)} title={t('Levantar a tampa')}>
      {label}
    </button>
  )
}

function ControlPanel({ v, replay }: { v: Vehicle; replay: boolean }) {
  const [step, setStep] = useState(5)
  const [dv, setDv] = useState(0.1)
  const [keys, setKeys] = useState(false)
  const id = v.id
  const hasAdcs = !!v.systems
  // Estágios do lançador que ainda estão presos (os da nave não contam).
  const launcher = (v.stages ?? []).filter((s) => !s.spacecraft).length
  const throttle = Math.round(((v.stages ?? []).flatMap((s) => s.engines ?? []).find((e) => !e.solid)?.throttle ?? 1) * 100)
  const nudge = (pitch: number, yaw: number, roll: number) => sendCommand(id, 'att_nudge', { pitch, yaw, roll })
  const pulse = (dir: string) => sendCommand(id, 'rcs_pulse', { dir, dv })
  // Pilotagem pelo teclado.
  useEffect(() => {
    pilot.active = keys
    if (!keys) return
    const down = (e: KeyboardEvent) => {
      const el = e.target as HTMLElement | null
      if (el && ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName)) return
      const k = e.key.toLowerCase()
      if (!PILOT_KEYS.includes(k) || e.repeat) return
      e.preventDefault()
      const act: Record<string, () => void> = {
        w: () => nudge(-step, 0, 0),
        s: () => nudge(step, 0, 0),
        a: () => nudge(0, -step, 0),
        d: () => nudge(0, step, 0),
        q: () => nudge(0, 0, -step),
        e: () => nudge(0, 0, step),
        i: () => pulse('prograde'),
        k: () => pulse('retrograde'),
        j: () => pulse('radial_in'),
        l: () => pulse('radial_out'),
        h: () => pulse('normal'),
        n: () => pulse('antinormal'),
      }
      act[k]?.()
    }
    window.addEventListener('keydown', down)
    return () => {
      window.removeEventListener('keydown', down)
      pilot.active = false
    }
  })
  const att = (mode: string, label: string) => (
    <CmdButton vehicle={id} cmd="att_mode" args={{ mode }} className={`small ${v.systems?.adcs.mode === mode ? 'active' : ''}`} disabled={!hasAdcs || replay}>
      {label}
    </CmdButton>
  )
  return (
    <div className="ctl-panel">
      <Group title={t('PROPULSÃO')}>
        <div className="row">
          <CmdButton vehicle={id} cmd="ignite" className="primary">
            {t('IGNIÇÃO')}
          </CmdButton>
          <CmdButton vehicle={id} cmd="shutdown">
            {t('CORTE')}
          </CmdButton>
        </div>
        <label className="throttle">
          {t('Potência')} <b className="mono">{throttle}%</b>
          <input type="range" min={0} max={100} step={5} defaultValue={throttle} disabled={replay || !canCommand(store.hello?.commands.find((c) => c.id === 'throttle'))} onChange={(e) => sendCommand(id, 'throttle', { value: Number(e.target.value) })} />
        </label>
        <div className="row">
          {[0, 40, 70, 100].map((p) => (
            <CmdButton key={p} vehicle={id} cmd="throttle" args={{ value: p }} className="small">
              {p}%
            </CmdButton>
          ))}
        </div>
      </Group>
      <Group title={t('ESTÁGIOS')}>
        <div className="row">
          {launcher > 0 && (
            <Guarded label={t('SEPARAR ESTÁGIO')}>
              <CmdButton vehicle={id} cmd="stage">
                {t('SEPARAR ESTÁGIO')}
              </CmdButton>
            </Guarded>
          )}
          {launcher > 0 && (
            <Guarded label={t('SEPARAR CARGA')}>
              <CmdButton vehicle={id} cmd="payload_sep">
                {t('SEPARAR CARGA')}
              </CmdButton>
            </Guarded>
          )}
          {launcher === 0 && <span className="small muted">{t('só a nave: nenhum estágio para separar')}</span>}
        </div>
        <div className="row">
          {(v.items ?? []).some((i) => i.id === 'fairing') && (
            <Guarded label={t('COIFA')}>
              <CmdButton vehicle={id} cmd="jettison" args={{ item: 'fairing' }}>
                {t('LARGAR COIFA')}
              </CmdButton>
            </Guarded>
          )}
          {v.les?.tower && (
            <Guarded label={t('TORRE')}>
              <CmdButton vehicle={id} cmd="jettison" args={{ item: 'les' }}>
                {t('LARGAR TORRE')}
              </CmdButton>
            </Guarded>
          )}
          <CmdButton vehicle={id} cmd="auto_stage" args={{ on: v.guidance?.auto_stage ? 'off' : 'on' }} className={`small ${v.guidance?.auto_stage ? 'active' : ''}`}>
            {t('SEQ AUTO')}
          </CmdButton>
        </div>
      </Group>
      <Group title={t('EMERGÊNCIA')}>
        <div className="row">
          {(v.aborts ?? []).some(([, ok]) => ok) ? (
            (v.aborts ?? [])
              .filter(([, ok]) => ok)
              .map(([m]) => (
              <Guarded key={m} label={`${t('ABORTO')} ${m.toUpperCase()}`}>
                <CmdButton vehicle={id} cmd="abort" args={{ mode: m }}>
                  {t('ABORTAR')} {m.toUpperCase()}
                </CmdButton>
              </Guarded>
              ))
          ) : (
            <span className="small muted">{t('sem aborto disponível agora')}</span>
          )}
        </div>
        <div className="row">
          <CmdButton vehicle={id} cmd="safe_mode" args={{ on: v.safe_mode ? 'off' : 'on' }} className={v.safe_mode ? 'active' : ''} disabled={!hasAdcs}>
            {v.safe_mode ? t('SAIR DO MODO SEGURO') : t('MODO SEGURO')}
          </CmdButton>
          {v.fts?.installed && (
            <Guarded label={t('TERMINAÇÃO')}>
              <CmdButton vehicle={id} cmd="fts_terminate">
                {t('TERMINAR O VOO')}
              </CmdButton>
            </Guarded>
          )}
        </div>
      </Group>
      <Group title={t('ATITUDE')}>
        <div className="row">
          {att('prograde', 'PRO')}
          {att('retrograde', 'RET')}
          {att('normal', 'NML')}
          {att('antinormal', 'ANML')}
          {att('radial_out', 'RAD+')}
          {att('radial_in', 'RAD−')}
          {att('sun', t('SOL'))}
          {att('nadir', 'NADIR')}
          {att('hold', t('MANTER'))}
          {att('detumble', t('AMORT'))}
        </div>
        <div className="pilot">
          <div className="pad">
            <span />
            <button onClick={() => nudge(-step, 0, 0)} disabled={!hasAdcs || replay} title="W">
              ▲
            </button>
            <span />
            <button onClick={() => nudge(0, -step, 0)} disabled={!hasAdcs || replay} title="A">
              ◀
            </button>
            <span className="small muted">{t('ARF/GUI')}</span>
            <button onClick={() => nudge(0, step, 0)} disabled={!hasAdcs || replay} title="D">
              ▶
            </button>
            <button onClick={() => nudge(0, 0, -step)} disabled={!hasAdcs || replay} title="Q">
              ↺
            </button>
            <button onClick={() => nudge(step, 0, 0)} disabled={!hasAdcs || replay} title="S">
              ▼
            </button>
            <button onClick={() => nudge(0, 0, step)} disabled={!hasAdcs || replay} title="E">
              ↻
            </button>
          </div>
          <div className="pilot-opts">
            <label className="small">
              {t('Passo')}
              <select value={step} onChange={(e) => setStep(Number(e.target.value))}>
                {[1, 2, 5, 10, 15, 30].map((s) => (
                  <option key={s} value={s}>
                    {s}°
                  </option>
                ))}
              </select>
            </label>
            <label className="small">
              <input type="checkbox" checked={keys} onChange={(e) => setKeys(e.target.checked)} disabled={replay} /> {t('Teclado (WASD QE)')}
            </label>
          </div>
        </div>
      </Group>
      <Group title={t('TRANSLAÇÃO (RCS)')}>
        <div className="row">
          {(
            [
              ['prograde', 'PRO (I)'],
              ['retrograde', 'RET (K)'],
              ['radial_out', 'RAD+ (L)'],
              ['radial_in', 'RAD− (J)'],
              ['normal', 'NML (H)'],
              ['antinormal', 'ANML (N)'],
            ] as [string, string][]
          ).map(([d, label]) => (
            <CmdButton key={d} vehicle={id} cmd="rcs_pulse" args={{ dir: d, dv }} className="small">
              {label}
            </CmdButton>
          ))}
          <select value={dv} onChange={(e) => setDv(Number(e.target.value))} aria-label={t('Δv do pulso')}>
            {[0.02, 0.05, 0.1, 0.5, 1, 5].map((x) => (
              <option key={x} value={x}>
                {x} m/s
              </option>
            ))}
          </select>
        </div>
      </Group>
      <Group title={t('RECUPERAÇÃO E SISTEMAS')}>
        <div className="row">
          {(v.chutes ?? []).map((c, i) => (
            <Guarded key={i} label={tx(c.name).toUpperCase()}>
              <CmdButton vehicle={id} cmd="chute" args={{ index: i, action: 'deploy' }}>
                {t('ABRIR')} {tx(c.name)}
              </CmdButton>
            </Guarded>
          ))}
          {v.kind === 'shuttle' && (
            <CmdButton vehicle={id} cmd="gear" className={v.gear_down ? 'active' : ''}>
              {t('TREM')}
            </CmdButton>
          )}
          {v.systems?.solar && !v.systems.solar.deployed && (
            <CmdButton vehicle={id} cmd="solar_deploy">
              {t('ABRIR PAINÉIS')}
            </CmdButton>
          )}
          {v.systems && (
            <CmdButton vehicle={id} cmd="tx" args={{ on: v.systems.comms.tx_on ? 'off' : 'on' }} className={v.systems.comms.tx_on ? 'active' : ''}>
              TX {v.systems.comms.tx_on ? 'ON' : 'OFF'}
            </CmdButton>
          )}
        </div>
        <div className="small muted">
          {t('Δv restante')}: {num(v.tm?.['nav.dv'], 0)} m/s · {t('massa')}: {num(v.tm?.['nav.mass'], 2)} t
        </div>
      </Group>
    </div>
  )
}
