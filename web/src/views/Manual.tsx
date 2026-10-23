// Controle manual: de um a quatro MFDs (horizonte artificial, mapa orbital,
// manobras, motores, recursos, descida, reentrada, alvo, subsistemas,
// comunicações, telemetria, alarmes, a nave em 3D e o mapa), o painel de
// chaves e botões de comando e o quadro de avisos, como nos consoles reais.
// Pilotagem pelo teclado, com as teclas que a pessoa escolher (padrão: W/S
// arfagem, A/D guinada, Q/E rolamento; I/K prógrado/retrógrado, J/L radial,
// H/N normal; R/F potência, Z/X máxima e zero, B ignição, O corte, G manter).
// Tudo o que se personaliza fica no navegador (lib/pilot.ts).

import { useEffect, useState, type ReactNode } from 'react'
import { Mfd } from '../components/Mfd'
import { CmdButton, Empty, sendCommand, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { duration, num } from '../lib/format'
import { nextNode, pickTarget, relative, targetCandidates } from '../lib/mfd'
import { ACTIONS, GROUPS, actionFor, pilot, resetPilotPrefs, setPilotPrefs, usePilotPrefs, type PilotAction, type PilotPrefs } from '../lib/pilot'
import { canCommand } from '../lib/room'
import { store, useStore } from '../lib/store'
import type { Plan, Vehicle } from '../types'

const HEIGHTS = { s: 340, m: 460, l: 600 }

export function Manual() {
  const v = useStore((s) => s.selected)
  const snap = useStore((s) => s.snap)!
  const prefs = usePilotPrefs()
  const [custom, setCustom] = useState(false)
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  return (
    <div className="manual">
      <div className="manual-bar">
        {prefs.annunciators ? <Annunciators v={v} /> : <span />}
        <button className={`small ${custom ? 'active' : ''}`} onClick={() => setCustom(!custom)} aria-expanded={custom}>
          ⚙ {t('Personalizar')}
        </button>
      </div>
      {custom && <Customize onClose={() => setCustom(false)} />}
      <div className={`manual-grid panel-${prefs.panel}`}>
        <div className={`mfd-bank n${prefs.count}`}>
          {Array.from({ length: prefs.count }, (_, i) => (
            <Mfd key={i} v={v} index={i} height={HEIGHTS[prefs.size]} />
          ))}
        </div>
        <ControlPanel v={v} replay={snap.replay != null} />
      </div>
    </div>
  )
}

/** Quadro de avisos: luzes que acendem com as condições, como nos painéis de cautela e alerta. Tocar no alarme mestre reconhece todos. */
function Annunciators({ v }: { v: Vehicle }) {
  const alarms = useStore((s) => (s.snap?.alarms ?? []).filter((a) => a.vehicle === v.id && a.active))
  const now = useStore((s) => s.snap?.t ?? 0)
  const tm = (k: string) => v.tm?.[k]
  const engines = (v.stages ?? []).flatMap((s) => s.engines ?? [])
  const prop = (v.stages ?? []).filter((s) => !s.spacecraft && (s.capacity ?? 0) > 0)
  const sys = v.systems
  const nn = nextNode(v)
  const wheels = sys?.adcs.wheels
  const lights: [string, 'red' | 'amber' | 'green' | 'off'][] = [
    [t('ALARME MESTRE'), alarms.some((a) => a.level === 'red' && !a.acked) ? 'red' : alarms.some((a) => !a.acked) ? 'amber' : 'off'],
    [t('FALHA MOTOR'), engines.some((e) => e.state === 'failed') ? 'red' : 'off'],
    [t('MOTOR ACESO'), engines.some((e) => e.state === 'running') ? 'green' : 'off'],
    [t('COMB BAIXO'), prop.some((s) => (s.prop ?? 0) / (s.capacity || 1) < 0.1 && (s.prop ?? 0) > 0) ? 'amber' : 'off'],
    [t('BATERIA'), (tm('eps.soc') ?? 100) < 30 ? 'amber' : 'off'],
    [t('SUBTENSÃO'), sys?.brownout ? 'red' : 'off'],
    [t('PRESS CABINE'), tm('ecl.pressure') != null && (tm('ecl.pressure') ?? 101) < 95 ? 'red' : 'off'],
    [t('SEM ENLACE'), sys && !sys.comms.link ? 'amber' : 'off'],
    [t('APAGÃO'), sys?.comms.blackout ? 'amber' : 'off'],
    [t('MODO SEGURO'), v.safe_mode ? 'amber' : 'off'],
    [t('TERM ARMADA'), v.fts?.armed ? 'red' : 'off'],
    [t('GUIAMENTO'), v.guidance?.program && v.guidance.program !== 'none' ? 'green' : 'off'],
    [t('QUEIMA'), nn?.executing ? 'green' : nn && nn.t - now < 120 ? 'amber' : 'off'],
    // Só para quem tem RCS (satélites com rodas de reação não têm propelente de RCS).
    [t('RCS BAIXO'), sys && sys.adcs.rcs_prop > 0 && sys.adcs.rcs_prop < 5 ? 'amber' : 'off'],
    [t('RODAS SAT'), wheels && wheels.h.some((h) => Math.abs(h) > 0.9 * wheels.max) ? 'amber' : 'off'],
    [t('TEMPERATURA'), sys?.thermal.some((n) => n.failed || n.temp < n.min || n.temp > n.max) ? 'amber' : 'off'],
    [t('CALOR'), v.heat_shield && v.heat_shield.flux > 100 ? 'amber' : 'off'],
    [t('ACOPLADO'), v.attached_to != null ? 'green' : 'off'],
  ]
  return (
    <div className="annunciators" role="status" aria-label={t('Quadro de avisos')}>
      {lights.map(([label, st], i) =>
        i === 0 ? (
          <button key={label} className={`ann ${st}`} onClick={() => store.sim('ack')} title={t('Reconhecer todos os alarmes')}>
            {label}
          </button>
        ) : (
          <span key={label} className={`ann ${st}`}>
            {label}
          </span>
        ),
      )}
    </div>
  )
}

function Group({ title, children, extra }: { title: string; children: ReactNode; extra?: ReactNode }) {
  const [open, setOpen] = useState(true)
  return (
    <fieldset className={`ctl-group ${open ? '' : 'closed'}`}>
      <legend>
        <button className="fold" onClick={() => setOpen(!open)} aria-expanded={open}>
          {open ? '▾' : '▸'} {title}
        </button>
        {open && extra}
      </legend>
      {open && children}
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

/** Planeja uma manobra pronta e põe as queimas no plano de voo. */
async function quickPlan(v: Vehicle, kind: string) {
  try {
    const r = await store.request<{ ok: boolean; plan?: Plan; error?: string }>({ type: 'plan', vehicle: v.id, kind, params: {} }, 60000)
    if (!r.ok || !r.plan) {
      toast(tx(r.error) || t('Sem solução'), 'bad')
      return
    }
    await store.request({ type: 'plan_apply', vehicle: v.id, nodes: r.plan.nodes, landing: r.plan.landing })
    toast(`${tx(r.plan.summary)} — Δv ${num(r.plan.total_dv, 1)} m/s`, 'good')
  } catch (e) {
    toast(String(e), 'bad')
  }
}

function ControlPanel({ v, replay }: { v: Vehicle; replay: boolean }) {
  const prefs = usePilotPrefs()
  const vehicles = useStore((s) => s.snap?.vehicles ?? [])
  const now = useStore((s) => s.snap?.t ?? 0)
  const [keys, setKeys] = useState(false)
  const [burn, setBurn] = useState({ t: '600', r: '0', tr: '10', n: '0' })
  const { step, dv } = prefs
  const id = v.id
  const hasAdcs = !!v.systems
  const show = (g: string) => !prefs.hidden.includes(g)
  // Estágios do lançador que ainda estão presos (os da nave não contam).
  const launcher = (v.stages ?? []).filter((s) => !s.spacecraft).length
  const throttle = Math.round(((v.stages ?? []).flatMap((s) => s.engines ?? []).find((e) => !e.solid)?.throttle ?? 1) * 100)
  const canThrottle = !replay && canCommand(store.hello?.commands.find((c) => c.id === 'throttle'))
  const canPlan = !replay && canCommand(store.hello?.commands.find((c) => c.id === 'burn'))
  const nudge = (pitch: number, yaw: number, roll: number) => sendCommand(id, 'att_nudge', { pitch, yaw, roll })
  const pulse = (dir: string) => sendCommand(id, 'rcs_pulse', { dir, dv })
  const setThrottle = (p: number) => sendCommand(id, 'throttle', { value: Math.max(0, Math.min(100, p)) })
  const tgt = pickTarget(v, vehicles)
  const candidates = targetCandidates(v, vehicles)
  const rel = tgt ? relative(v, tgt) : null
  const nn = nextNode(v)
  // Pilotagem pelo teclado.
  useEffect(() => {
    pilot.active = keys
    if (!keys) return
    const down = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return
      const el = e.target as HTMLElement | null
      if (el && ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName)) return
      const a = actionFor(e.key)
      if (!a) return
      e.preventDefault()
      // Só a potência anda segurando a tecla; o resto é um toque por vez.
      if (e.repeat && a !== 'throttle_up' && a !== 'throttle_down') return
      const act: Record<PilotAction, () => void> = {
        pitch_minus: () => nudge(-step, 0, 0),
        pitch_plus: () => nudge(step, 0, 0),
        yaw_minus: () => nudge(0, -step, 0),
        yaw_plus: () => nudge(0, step, 0),
        roll_minus: () => nudge(0, 0, -step),
        roll_plus: () => nudge(0, 0, step),
        rcs_prograde: () => pulse('prograde'),
        rcs_retrograde: () => pulse('retrograde'),
        rcs_radial_in: () => pulse('radial_in'),
        rcs_radial_out: () => pulse('radial_out'),
        rcs_normal: () => pulse('normal'),
        rcs_antinormal: () => pulse('antinormal'),
        throttle_up: () => setThrottle(throttle + prefs.throttleStep),
        throttle_down: () => setThrottle(throttle - prefs.throttleStep),
        throttle_full: () => setThrottle(100),
        throttle_zero: () => setThrottle(0),
        ignite: () => sendCommand(id, 'ignite'),
        cutoff: () => sendCommand(id, 'shutdown'),
        hold: () => sendCommand(id, 'att_mode', { mode: 'hold' }),
        detumble: () => sendCommand(id, 'att_mode', { mode: 'detumble' }),
      }
      act[a]()
    }
    window.addEventListener('keydown', down)
    return () => {
      window.removeEventListener('keydown', down)
      pilot.active = false
    }
  })
  const att = (mode: string, label: string, args: Record<string, unknown> = {}) => (
    <CmdButton vehicle={id} cmd="att_mode" args={{ mode, ...args }} className={`small ${v.systems?.adcs.mode === mode ? 'active' : ''}`} disabled={!hasAdcs || replay}>
      {label}
    </CmdButton>
  )
  const key = (a: PilotAction) => (prefs.keys[a] ? ` (${prefs.keys[a].toUpperCase()})` : '')
  return (
    <div className="ctl-panel">
      {show('prop') && (
        <Group title={t('PROPULSÃO')}>
          <div className="row">
            <CmdButton vehicle={id} cmd="ignite" className="primary">
              {t('IGNIÇÃO')}
              {keys && key('ignite')}
            </CmdButton>
            <CmdButton vehicle={id} cmd="shutdown">
              {t('CORTE')}
              {keys && key('cutoff')}
            </CmdButton>
          </div>
          <label className="throttle">
            {t('Potência')} <b className="mono">{throttle}%</b>
            <input type="range" min={0} max={100} step={5} defaultValue={throttle} disabled={!canThrottle} onChange={(e) => setThrottle(Number(e.target.value))} />
          </label>
          <div className="row">
            {[0, 40, 70, 100].map((p) => (
              <CmdButton key={p} vehicle={id} cmd="throttle" args={{ value: p }} className={`small ${throttle === p ? 'active' : ''}`}>
                {p}%
              </CmdButton>
            ))}
          </div>
        </Group>
      )}
      {show('stages') && (
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
      )}
      {show('emerg') && (
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
      )}
      {show('att') && (
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
            {v.central !== 'earth' && att('earth', t('TERRA'))}
            {tgt && att('target', t('ALVO'), { target: tgt.id })}
            {att('hold', t('MANTER') + (keys ? key('hold') : ''))}
            {att('detumble', t('AMORT'))}
            {att('free', t('LIVRE'))}
            {v.systems?.adcs.wheels && (
              <CmdButton vehicle={id} cmd="desat" className={`small ${v.systems.adcs.desat ? 'active' : ''}`}>
                {t('DESCARREGAR RODAS')}
              </CmdButton>
            )}
          </div>
          <div className="pilot">
            <div className="pad">
              <span />
              <button onClick={() => nudge(-step, 0, 0)} disabled={!hasAdcs || replay} title={prefs.keys.pitch_minus.toUpperCase()}>
                ▲
              </button>
              <span />
              <button onClick={() => nudge(0, -step, 0)} disabled={!hasAdcs || replay} title={prefs.keys.yaw_minus.toUpperCase()}>
                ◀
              </button>
              <span className="small muted">{t('ARF/GUI')}</span>
              <button onClick={() => nudge(0, step, 0)} disabled={!hasAdcs || replay} title={prefs.keys.yaw_plus.toUpperCase()}>
                ▶
              </button>
              <button onClick={() => nudge(0, 0, -step)} disabled={!hasAdcs || replay} title={prefs.keys.roll_minus.toUpperCase()}>
                ↺
              </button>
              <button onClick={() => nudge(step, 0, 0)} disabled={!hasAdcs || replay} title={prefs.keys.pitch_plus.toUpperCase()}>
                ▼
              </button>
              <button onClick={() => nudge(0, 0, step)} disabled={!hasAdcs || replay} title={prefs.keys.roll_plus.toUpperCase()}>
                ↻
              </button>
            </div>
            <div className="pilot-opts">
              <label className="small">
                {t('Passo')}
                <select value={step} onChange={(e) => setPilotPrefs({ step: Number(e.target.value) })}>
                  {[0.5, 1, 2, 5, 10, 15, 30, 45, 90].map((s) => (
                    <option key={s} value={s}>
                      {s}°
                    </option>
                  ))}
                </select>
              </label>
              <label className="small">
                <input type="checkbox" checked={keys} onChange={(e) => setKeys(e.target.checked)} disabled={replay} /> {t('Pilotar pelo teclado')}
              </label>
              {keys && <span className="small muted">{t('As teclas mudam em ⚙ Personalizar.')}</span>}
            </div>
          </div>
        </Group>
      )}
      {show('rcs') && (
        <Group title={t('TRANSLAÇÃO (RCS)')}>
          <div className="row">
            {(
              [
                ['prograde', 'PRO', 'rcs_prograde'],
                ['retrograde', 'RET', 'rcs_retrograde'],
                ['radial_out', 'RAD+', 'rcs_radial_out'],
                ['radial_in', 'RAD−', 'rcs_radial_in'],
                ['normal', 'NML', 'rcs_normal'],
                ['antinormal', 'ANML', 'rcs_antinormal'],
              ] as [string, string, PilotAction][]
            ).map(([d, label, a]) => (
              <CmdButton key={d} vehicle={id} cmd="rcs_pulse" args={{ dir: d, dv }} className="small">
                {label}
                {key(a)}
              </CmdButton>
            ))}
            <select value={dv} onChange={(e) => setPilotPrefs({ dv: Number(e.target.value) })} aria-label={t('Δv do pulso')}>
              {[0.01, 0.02, 0.05, 0.1, 0.5, 1, 5, 10].map((x) => (
                <option key={x} value={x}>
                  {x} m/s
                </option>
              ))}
            </select>
          </div>
        </Group>
      )}
      {show('mnv') && (
        <Group title={t('MANOBRAS')}>
          <div className="small">
            {nn ? (
              <>
                {t('Próxima')}: <b>{tx(nn.label)}</b> · {nn.executing ? <span className="warn">{t('queimando')}</span> : `T− ${duration(nn.t - now)}`} · Δv {num(nn.dv_total, 1)} m/s
              </>
            ) : (
              <span className="muted">{t('Nenhuma queima no plano de voo.')}</span>
            )}
          </div>
          <div className="row">
            {(
              [
                ['circularize_ap', t('CIRC NO AP')],
                ['circularize_pe', t('CIRC NO PE')],
              ] as [string, string][]
            ).map(([k, label]) => (
              <button key={k} className="small" onClick={() => quickPlan(v, k)} disabled={!canPlan || !v.r} title={t('Planeja e põe no plano de voo')}>
                {label}
              </button>
            ))}
            {nn && (
              <CmdButton vehicle={id} cmd="burn_delete" args={{ node: nn.id }} className="small">
                {t('APAGAR PRÓXIMA')}
              </CmdButton>
            )}
            {(v.nodes ?? []).some((n) => !n.done) && (
              <CmdButton vehicle={id} cmd="burn_clear" className="small">
                {t('APAGAR TODAS')}
              </CmdButton>
            )}
          </div>
          <div className="quick-burn">
            <label>
              {t('em (s)')}
              <input value={burn.t} onChange={(e) => setBurn({ ...burn, t: e.target.value })} inputMode="decimal" />
            </label>
            <label>
              ΔR
              <input value={burn.r} onChange={(e) => setBurn({ ...burn, r: e.target.value })} inputMode="decimal" />
            </label>
            <label>
              ΔT
              <input value={burn.tr} onChange={(e) => setBurn({ ...burn, tr: e.target.value })} inputMode="decimal" />
            </label>
            <label>
              ΔN
              <input value={burn.n} onChange={(e) => setBurn({ ...burn, n: e.target.value })} inputMode="decimal" />
            </label>
            <CmdButton
              vehicle={id}
              cmd="burn"
              className="small"
              args={{ t: numOr(burn.t), dv_r: numOr(burn.r), dv_t: numOr(burn.tr), dv_n: numOr(burn.n), label: t('Queima manual') }}
              title={t('Queima em RTN (m/s): radial, transversal e normal')}
            >
              {t('PROGRAMAR')}
            </CmdButton>
          </div>
        </Group>
      )}
      {show('rdv') && (
        <Group title={t('ENCONTRO')}>
          {candidates.length ? (
            <>
              <div className="row">
                <select value={tgt?.id ?? ''} onChange={(e) => setPilotPrefs({ target: Number(e.target.value) })} aria-label={t('Alvo')}>
                  {candidates.map((o) => (
                    <option key={o.id} value={o.id}>
                      {o.name}
                    </option>
                  ))}
                </select>
                {rel && (
                  <span className="small mono">
                    {rel.range < 10_000 ? `${num(rel.range, 0)} m` : `${num(rel.range / 1000, 1)} km`} · {rel.rate >= 0 ? '+' : ''}
                    {num(rel.rate, 2)} m/s
                  </span>
                )}
              </div>
              <div className="row">
                {tgt && hasAdcs && (
                  <CmdButton vehicle={id} cmd="att_mode" args={{ mode: 'target', target: tgt.id }} className={`small ${v.systems?.adcs.mode === 'target' ? 'active' : ''}`}>
                    {t('APONTAR AO ALVO')}
                  </CmdButton>
                )}
                {tgt && (
                  <CmdButton vehicle={id} cmd="dock" args={{ target: tgt.id }} className={`small ${v.guidance?.dock_target === tgt.id ? 'active' : ''}`}>
                    {t('APROXIMAR E ACOPLAR')}
                  </CmdButton>
                )}
                {v.attached_to != null && (
                  <CmdButton vehicle={id} cmd="undock" className="small">
                    {t('DESACOPLAR')}
                  </CmdButton>
                )}
              </div>
            </>
          ) : (
            <span className="small muted">{t('Nenhum veículo em volta de {b}.', { b: tx(v.central_name ?? v.central) })}</span>
          )}
        </Group>
      )}
      {show('rec') && (
        <Group title={t('RECUPERAÇÃO E SISTEMAS')}>
          <div className="row">
            {(v.chutes ?? []).map((c, i) => (
              <Guarded key={i} label={tx(c.name).toUpperCase()}>
                {/* O núcleo conta os paraquedas a partir de 1. */}
                <CmdButton vehicle={id} cmd="chute" args={{ index: i + 1, action: 'deploy' }}>
                  {t('ABRIR')} {tx(c.name)}
                </CmdButton>
              </Guarded>
            ))}
            {(v.chutes ?? []).length > 0 && (
              <CmdButton vehicle={id} cmd="auto_chutes" args={{ on: v.guidance?.auto_chutes ? 'off' : 'on' }} className={`small ${v.guidance?.auto_chutes ? 'active' : ''}`}>
                {t('REENTRADA AUTO')}
              </CmdButton>
            )}
            {['capsule', 'shuttle'].includes(v.kind) && (
              <CmdButton vehicle={id} cmd="entry_mode" args={{ mode: v.guidance?.ballistic ? 'guided' : 'ballistic' }} className="small">
                {v.guidance?.ballistic ? t('BALÍSTICA') : t('GUIADA')}
              </CmdButton>
            )}
            {v.landing === 'legs' && !['landed', 'destroyed'].includes(v.phase) && (
              <Guarded label={t('DESCIDA PROPULSADA')}>
                <CmdButton vehicle={id} cmd="pdi" className={v.guidance?.program === 'powereddescent' ? 'active' : ''}>
                  {t('PDI · DESCER E POUSAR')}
                </CmdButton>
              </Guarded>
            )}
            {v.kind === 'shuttle' && (
              <CmdButton vehicle={id} cmd="gear" className={v.gear_down ? 'active' : ''}>
                {t('TREM')}
              </CmdButton>
            )}
          </div>
          {v.systems && (
            <div className="row">
              {v.systems.solar && !v.systems.solar.deployed && (
                <CmdButton vehicle={id} cmd="solar_deploy" className="small">
                  {t('ABRIR PAINÉIS')}
                </CmdButton>
              )}
              {v.systems.has_radiator && (
                <CmdButton vehicle={id} cmd="radiator" args={{ on: v.systems.radiator ? 'off' : 'on' }} className={`small ${v.systems.radiator ? 'active' : ''}`}>
                  {t('RADIADORES')}
                </CmdButton>
              )}
              <CmdButton vehicle={id} cmd="tx" args={{ on: v.systems.comms.tx_on ? 'off' : 'on' }} className={`small ${v.systems.comms.tx_on ? 'active' : ''}`}>
                TX {v.systems.comms.tx_on ? 'ON' : 'OFF'}
              </CmdButton>
              {v.systems.comms.hga && !v.systems.comms.hga_deployed && (
                <CmdButton vehicle={id} cmd="hga_deploy" className="small">
                  {t('ABRIR ANTENA')}
                </CmdButton>
              )}
              {v.systems.comms.storage > 0 && (
                <CmdButton vehicle={id} cmd="dump" className={`small ${v.systems.comms.dumping ? 'active' : ''}`}>
                  {t('DESCARREGAR GRAVADOR')}
                </CmdButton>
              )}
            </div>
          )}
          <div className="small muted">
            {t('Δv restante')}: {num(v.tm?.['nav.dv'], 0)} m/s · {t('massa')}: {num(v.tm?.['nav.mass'], 2)} t
          </div>
        </Group>
      )}
    </div>
  )
}

const numOr = (s: string) => Number(s.replace(',', '.')) || 0

/** Personalização do console: telas, horizonte, grupos do painel e teclas. */
function Customize({ onClose }: { onClose: () => void }) {
  const p = usePilotPrefs()
  const [listen, setListen] = useState<PilotAction | null>(null)
  useEffect(() => {
    if (!listen) return
    // Na fase de captura, antes dos atalhos da interface.
    const down = (e: KeyboardEvent) => {
      e.preventDefault()
      e.stopPropagation()
      if (e.key === 'Escape') return setListen(null)
      const k = e.key === 'Backspace' || e.key === 'Delete' ? '' : e.key.toLowerCase()
      if (k.length > 1 || k === ' ') return
      const keys = { ...p.keys }
      for (const a of Object.keys(keys) as PilotAction[]) if (k && keys[a] === k) keys[a] = ''
      keys[listen] = k
      setPilotPrefs({ keys })
      setListen(null)
    }
    window.addEventListener('keydown', down, true)
    return () => window.removeEventListener('keydown', down, true)
  }, [listen, p.keys])
  const radio = <K extends keyof PilotPrefs>(field: K, options: [PilotPrefs[K], string][]) => (
    <div className="seg">
      {options.map(([val, label]) => (
        <button key={String(val)} className={`small ${p[field] === val ? 'active' : ''}`} onClick={() => setPilotPrefs({ [field]: val } as Partial<PilotPrefs>)} aria-pressed={p[field] === val}>
          {label}
        </button>
      ))}
    </div>
  )
  const check = (field: 'annunciators' | 'adiMarkers' | 'adiNode' | 'adiTarget', label: string) => (
    <label>
      <input type="checkbox" checked={p[field]} onChange={(e) => setPilotPrefs({ [field]: e.target.checked })} /> {label}
    </label>
  )
  return (
    <div className="customize" role="dialog" aria-label={t('Personalizar o controle manual')}>
      <section>
        <h4>{t('Telas')}</h4>
        <div className="field">
          <span>{t('MFDs')}</span>
          {radio('count', [
            [1, '1'],
            [2, '2'],
            [3, '3'],
            [4, '4'],
          ])}
        </div>
        <div className="field">
          <span>{t('Tamanho')}</span>
          {radio('size', [
            ['s', t('P')],
            ['m', t('M')],
            ['l', t('G')],
          ])}
        </div>
        <div className="field">
          <span>{t('Cores')}</span>
          {radio('palette', [
            ['cores', t('Coloridas')],
            ['verde', t('Fósforo verde')],
            ['ambar', t('Âmbar')],
          ])}
        </div>
        <div className="field">
          <span>{t('Painel')}</span>
          {radio('panel', [
            ['right', t('à direita')],
            ['below', t('embaixo')],
          ])}
        </div>
        {check('annunciators', t('Quadro de avisos'))}
        <p className="small muted">{t('As páginas de cada MFD se escolhem no MENU da própria tela (★ põe na moldura).')}</p>
      </section>
      <section>
        <h4>{t('Horizonte artificial')}</h4>
        {check('adiMarkers', t('Marcadores da órbita (prógrado, normal, radial)'))}
        {check('adiNode', t('Vetor da próxima queima (✚)'))}
        {check('adiTarget', t('Direção do alvo (◎)'))}
        <h4>{t('Grupos do painel')}</h4>
        <div className="checks">
          {GROUPS.map(([g, name]) => (
            <label key={g}>
              <input type="checkbox" checked={!p.hidden.includes(g)} onChange={(e) => setPilotPrefs({ hidden: e.target.checked ? p.hidden.filter((x) => x !== g) : [...p.hidden, g] })} /> {t(name)}
            </label>
          ))}
        </div>
        <div className="field">
          <span>{t('Passo da potência')}</span>
          {radio('throttleStep', [
            [1, '1%'],
            [5, '5%'],
            [10, '10%'],
            [25, '25%'],
          ])}
        </div>
      </section>
      <section className="keymap">
        <h4>{t('Teclas de voo')}</h4>
        <div className="keys">
          {ACTIONS.map(([a, name]) => (
            <div key={a}>
              <span>{t(name)}</span>
              <button className={`small mono ${listen === a ? 'armed' : ''}`} onClick={() => setListen(listen === a ? null : a)}>
                {listen === a ? t('tecle…') : p.keys[a] ? p.keys[a].toUpperCase() : '—'}
              </button>
            </div>
          ))}
        </div>
        <p className="small muted">{t('Clique e tecle a nova tecla; Delete deixa sem tecla, Esc cancela. Os comandos perigosos não vão para o teclado.')}</p>
      </section>
      <div className="row customize-foot">
        <button className="small" onClick={resetPilotPrefs}>
          {t('Restaurar o padrão')}
        </button>
        <button className="small primary" onClick={onClose}>
          {t('Fechar')}
        </button>
      </div>
    </div>
  )
}
