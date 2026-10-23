// Páginas dos MFDs além do horizonte, da órbita e dos recursos: motores,
// manobras com o diretor de voo, descida e pouso, reentrada, alvo e
// aproximação, subsistemas, comunicações, telemetria escolhida e alarmes.

import type { ReactNode } from 'react'
import { useCanvas } from './Chart'
import { findDef, limitState } from './ui'
import { type Q, type V3 } from '../lib/attitude'
import { t, tx } from '../lib/i18n'
import { chuteState, distance, duration, latlon, num, rate, utc } from '../lib/format'
import { gravity, nodeDirection, offsets, palette, ratedThrust, relative, type Palette } from '../lib/mfd'
import { setPilotPrefs, usePilotPrefs } from '../lib/pilot'
import { store, useStore } from '../lib/store'
import type { Node, Vehicle } from '../types'

type Tone = 'ok' | 'warn' | 'bad' | 'dim' | 'hi' | undefined

const pct = (f: number) => `${Math.max(0, Math.min(1, f)) * 100}%`

function Sec({ title, children }: { title: ReactNode; children: ReactNode }) {
  return (
    <div className="mfd-sec">
      <div className="mfd-sec-title">{title}</div>
      {children}
    </div>
  )
}

/** Pares nome e valor em duas colunas. */
function KV({ items }: { items: [ReactNode, ReactNode, Tone?][] }) {
  return (
    <div className="mfd-kv">
      {items.map(([k, val, tone], i) => (
        <div key={i}>
          <span className="lbl">{k}</span>
          <span className={`val ${tone ?? ''}`}>{val}</span>
        </div>
      ))}
    </div>
  )
}

function Row({ label, frac, text, warn, tone }: { label: ReactNode; frac: number | null; text: ReactNode; warn?: number; tone?: Tone }) {
  const low = frac != null && warn != null && frac < warn
  return (
    <div className="mfd-res-row">
      <span className="lbl">{label}</span>
      <span className="bar-track">{frac != null && <span className={`bar-fill ${low || tone === 'warn' ? 'low' : ''} ${tone === 'bad' ? 'bad' : ''}`} style={{ width: pct(frac) }} />}</span>
      <span className={`val ${low ? 'low' : (tone ?? '')}`}>{text}</span>
    </div>
  )
}

/** Valor de um ponto de telemetria com a unidade do dicionário e a cor do limite. */
function tmv(v: Vehicle, id: string, digits?: number): [string, Tone] {
  const raw = v.tm?.[id]
  const def = findDef(v.id, id)
  const st = limitState(def, raw)
  return [raw == null ? '—' : `${num(raw, digits)}${def?.unit ? ` ${def.unit}` : ''}`, st === 'red' ? 'bad' : st === 'yellow' ? 'warn' : undefined]
}

const dist = (m: number) => (Math.abs(m) < 10_000 ? `${num(m, 0)} m` : distance(m / 1000))

function message(ctx: CanvasRenderingContext2D, C: Palette, w: number, h: number, lines: string[]) {
  ctx.fillStyle = C.dim
  ctx.font = '13px ui-monospace, monospace'
  ctx.textAlign = 'center'
  lines.forEach((l, i) => ctx.fillText(l, w / 2, h / 2 + (i - (lines.length - 1) / 2) * 20))
  ctx.textAlign = 'left'
}

/** Colunas de números no canto: nome apagado, valor aceso. */
function readout(ctx: CanvasRenderingContext2D, C: Palette, x: number, y: number, rows: [string, string, string?][], col = 64) {
  ctx.font = '12px ui-monospace, monospace'
  rows.forEach(([k, val, color], i) => {
    ctx.fillStyle = C.dim
    ctx.fillText(k, x, y + i * 17)
    ctx.fillStyle = color ?? C.green
    ctx.fillText(val, x + col, y + i * 17)
  })
}

// ── ENG ──

const ENG_STATE: Record<string, string> = { off: 'DESL', starting: 'PARTIDA', running: 'ACESO', stopping: 'CORTANDO', failed: 'FALHA' }

/** Motores por estágio: estado, empuxo, potência, pressão de câmara, ignições; TWR e tempo de queima. */
export function Engines({ v }: { v: Vehicle }) {
  const mass = v.tm?.['nav.mass'] ?? null
  const engines = (v.stages ?? []).flatMap((s) => s.engines ?? [])
  const thrust = engines.reduce((a, e) => a + e.thrust, 0)
  const rated = ratedThrust(v)
  const g = gravity(v)
  const aMax = mass ? rated / mass : null
  const dv = v.tm?.['nav.dv'] ?? null
  return (
    <div className="mfd-res">
      <KV
        items={[
          [t('Empuxo'), `${num(thrust, 0)} kN`, thrust > 0 ? 'hi' : undefined],
          [t('Máximo'), `${num(rated, 0)} kN`],
          ['TWR', num(mass ? thrust / (mass * g) : null, 2)],
          [t('TWR máx'), num(aMax != null ? aMax / g : null, 2), aMax != null && aMax < g ? 'warn' : undefined],
          [t('Aceleração'), `${num(v.tm?.['nav.accel'], 2)} m/s²`],
          [t('Massa'), `${num(mass, 2)} t`],
          ['Δv', `${num(dv, 0)} m/s`, 'hi'],
          [t('Queima (máx.)'), aMax && dv ? duration(dv / aMax) : '—'],
        ]}
      />
      {(v.stages ?? [])
        .filter((s) => (s.engines ?? []).length > 0)
        .map((s, i) => (
          <Sec key={i} title={tx(s.name)}>
            {(s.capacity ?? 0) > 0 && <Row label={t('Propelente')} frac={(s.prop ?? 0) / s.capacity!} text={`${num(s.prop ?? 0, 0)} kg`} warn={0.1} />}
            {s.engines!.map((e, k) => (
              <div key={k} className="mfd-eng">
                <span className={`st ${e.state}`}>{t(ENG_STATE[e.state] ?? e.state)}</span>
                <span className="lbl">{tx(e.name)}</span>
                <span className="bar-track">
                  <span className="bar-fill" style={{ width: pct(e.rated ? e.thrust / e.rated : 0) }} />
                </span>
                <span className="val">{num(e.thrust, 0)} kN</span>
                <span className="sub">
                  {e.solid ? t('sólido') : `${Math.round(e.throttle * 100)}%`} · Pc {num(e.pc, 1)} bar · {e.ignitions}× · {duration(e.burn)}
                </span>
              </div>
            ))}
          </Sec>
        ))}
      {engines.length === 0 && <div className="mfd-msg">{t('SEM MOTORES')}</div>}
    </div>
  )
}

// ── MNV ──

/** A queima escolhida: contagem, Δv em RTN, quanto falta e o diretor de voo (para onde apontar). */
export function Maneuver({ v, node, height }: { v: Vehicle; node: Node | undefined; height: number }) {
  const now = useStore((s) => s.snap?.t ?? 0)
  const pal = usePilotPrefs().palette
  const ref = useCanvas(
    (ctx, w, h) => {
      const C = palette()
      ctx.fillStyle = '#000'
      ctx.fillRect(0, 0, w, h)
      if (!node) {
        message(ctx, C, w, h, [t('SEM QUEIMAS PLANEJADAS'), t('Planeje no painel MANOBRAS ou no console FIDO.')])
        return
      }
      // Diretor de voo: a bola mostra o vetor da queima em relação ao nariz (centro).
      const R0 = Math.min(w * 0.3, h * 0.42)
      const cx = w - R0 - 18
      const cy = h / 2
      const ppd = R0 / 45
      ctx.strokeStyle = C.grid
      ctx.lineWidth = 1
      for (const a of [5, 15, 30, 45]) {
        ctx.beginPath()
        ctx.arc(cx, cy, a * ppd, 0, Math.PI * 2)
        ctx.stroke()
      }
      ctx.beginPath()
      ctx.moveTo(cx - R0, cy)
      ctx.lineTo(cx + R0, cy)
      ctx.moveTo(cx, cy - R0)
      ctx.lineTo(cx, cy + R0)
      ctx.stroke()
      ctx.strokeStyle = C.amber
      ctx.lineWidth = 2
      ctx.strokeRect(cx - 6, cy - 6, 12, 12)
      const dir = nodeDirection(v, node)
      let err: number | null = null
      if (dir && v.q) {
        const o = offsets(dir, v.q as Q)
        err = o.angle
        const inside = o.front && Math.hypot(o.dx, o.dy) <= 45
        ctx.font = 'bold 22px ui-monospace, monospace'
        ctx.textAlign = 'center'
        ctx.textBaseline = 'middle'
        if (inside) {
          ctx.fillStyle = err < 2 ? C.green : C.cyan
          ctx.fillText('✚', cx + o.dx * ppd, cy - o.dy * ppd)
        } else {
          // Fora da bola (ou atrás): uma seta na borda mostra para que lado girar.
          const a = Math.atan2(-o.dy, o.dx)
          ctx.fillStyle = C.red
          ctx.save()
          ctx.translate(cx + Math.cos(a) * (R0 + 2), cy + Math.sin(a) * (R0 + 2))
          ctx.rotate(a)
          ctx.beginPath()
          ctx.moveTo(8, 0)
          ctx.lineTo(-6, -7)
          ctx.lineTo(-6, 7)
          ctx.fill()
          ctx.restore()
        }
        ctx.textAlign = 'left'
        ctx.textBaseline = 'alphabetic'
      }
      // Números à esquerda.
      const tgo = node.t - now
      const burn = node.burn ?? null
      const status = node.executing ? t('QUEIMANDO') : tgo < 0 ? t('ATRASADA') : t('AGUARDANDO')
      ctx.font = 'bold 14px ui-monospace, monospace'
      ctx.fillStyle = C.line
      ctx.fillText(tx(node.label).toUpperCase(), 12, 22)
      ctx.font = 'bold 26px ui-monospace, monospace'
      ctx.fillStyle = node.executing ? C.amber : tgo < 60 ? C.amber : C.green
      ctx.fillText(node.executing ? status : `T− ${duration(Math.max(0, tgo))}`, 12, 56)
      readout(ctx, C, 12, 84, [
        ['Δv', `${num(node.dv_total, 1)} m/s`, C.cyan],
        ['R', `${num(node.dv[0], 1)} m/s`],
        ['T', `${num(node.dv[1], 1)} m/s`],
        ['N', `${num(node.dv[2], 1)} m/s`],
        [t('FALTA'), `${num(node.executing ? node.remaining : node.dv_total, 1)} m/s`, node.executing ? C.amber : C.green],
        [t('DURAÇÃO'), burn != null ? duration(burn) : '—'],
        [t('COM'), node.rcs ? 'RCS' : t('MOTOR')],
        [t('ERRO'), err != null ? `${num(err, 1)}°` : '—', err != null && err > 5 ? C.amber : C.green],
        [t('ESTADO'), status, node.executing ? C.amber : C.dim],
      ])
      // Barra do que já foi queimado.
      if (node.executing && node.dv_total > 0) {
        const f = Math.max(0, Math.min(1, 1 - node.remaining / node.dv_total))
        const bw = Math.max(80, cx - R0 - 40)
        ctx.strokeStyle = C.dim
        ctx.strokeRect(12, h - 26, bw, 10)
        ctx.fillStyle = C.cyan
        ctx.fillRect(13, h - 25, (bw - 2) * f, 8)
      }
      ctx.fillStyle = C.dim
      ctx.font = '10px ui-monospace, monospace'
      ctx.textAlign = 'center'
      ctx.fillText(t('DIRETOR: ✚ NO QUADRADO = APONTADO'), cx, h - 8)
      ctx.textAlign = 'left'
    },
    [v, node, height, now, pal],
  )
  return (
    <div className="canvas-wrap" style={{ height }}>
      <canvas ref={ref} role="img" aria-label={t('Manobras e diretor de voo')} />
    </div>
  )
}

// ── DSC ──

/** Descida: altitude, velocidade vertical e horizontal, tempo até o chão e a altura da queima de parada. */
export function Descent({ v, height }: { v: Vehicle; height: number }) {
  const pal = usePilotPrefs().palette
  const ref = useCanvas(
    (ctx, w, h) => {
      const C = palette()
      ctx.fillStyle = '#000'
      ctx.fillRect(0, 0, w, h)
      const alt = v.alt * 1000
      const vs = v.tm?.['nav.vspeed'] ?? 0
      const hs = v.tm?.['nav.hspeed'] ?? null
      const mass = v.tm?.['nav.mass'] ?? null
      const g = gravity(v)
      const aMax = mass ? ratedThrust(v) / mass : 0
      // Queima de parada: a altura em que o motor a pleno zera a descida.
      const stop = vs < 0 && aMax > g ? (vs * vs) / (2 * (aMax - g)) : null
      // Tempo até o chão só na queda (em órbita fechada acima do solo não há impacto).
      const falling = v.pe == null || v.pe < 0
      const tImpact = alt > 0 ? (vs + Math.sqrt(vs * vs + 2 * g * alt)) / g : 0
      // Fita de altitude em escala logarítmica (1 m a 1000 km).
      const top = 16
      const bot = h - 24
      const lg = (m: number) => bot - ((Math.log10(Math.max(1, m)) - 0) / 6) * (bot - top)
      const tx0 = 46
      ctx.strokeStyle = C.dim
      ctx.strokeRect(tx0, top, 18, bot - top)
      ctx.font = '10px ui-monospace, monospace'
      ctx.fillStyle = C.dim
      for (const [m, l] of [
        [1, '1m'],
        [10, '10m'],
        [100, '100m'],
        [1e3, '1km'],
        [1e4, '10km'],
        [1e5, '100km'],
        [1e6, '1000km'],
      ] as [number, string][]) {
        const y = lg(m)
        ctx.fillText(l, 4, y + 3)
        ctx.beginPath()
        ctx.moveTo(tx0, y)
        ctx.lineTo(tx0 + 5, y)
        ctx.stroke()
      }
      if (stop != null) {
        ctx.fillStyle = C.red
        ctx.fillRect(tx0, lg(stop), 18, bot - lg(stop))
      }
      ctx.fillStyle = C.green
      ctx.beginPath()
      const ya = lg(alt)
      ctx.moveTo(tx0 + 20, ya)
      ctx.lineTo(tx0 + 30, ya - 6)
      ctx.lineTo(tx0 + 30, ya + 6)
      ctx.fill()
      // Indicador de velocidade vertical (±).
      const vx = tx0 + 64
      const span = Math.max(20, Math.abs(vs) * 1.3)
      const mid = (top + bot) / 2
      ctx.strokeStyle = C.dim
      ctx.strokeRect(vx, top, 18, bot - top)
      ctx.beginPath()
      ctx.moveTo(vx - 4, mid)
      ctx.lineTo(vx + 22, mid)
      ctx.stroke()
      const yv = mid - (vs / span) * ((bot - top) / 2)
      ctx.fillStyle = vs < -10 && alt < 2000 ? C.red : vs < 0 ? C.amber : C.cyan
      ctx.fillRect(vx + 1, Math.min(mid, yv), 16, Math.abs(yv - mid))
      ctx.fillStyle = C.dim
      ctx.fillText(t('VV'), vx + 2, h - 8)
      ctx.fillText(t('ALT'), tx0, h - 8)
      // Números.
      const x0 = vx + 44
      ctx.font = 'bold 26px ui-monospace, monospace'
      ctx.fillStyle = C.green
      ctx.fillText(dist(alt), x0, 40)
      ctx.font = 'bold 20px ui-monospace, monospace'
      ctx.fillStyle = vs < 0 ? C.amber : C.cyan
      ctx.fillText(`${vs >= 0 ? '+' : ''}${num(vs, 1)} m/s`, x0, 68)
      const lt = v.guidance?.landing_target
      const R = v.central_radius ?? 6.371e6
      const toTarget = lt ? greatCircle(v.lat, v.lon, lt[0], lt[1]) * R : null
      const margin = stop != null ? alt - stop : null
      readout(
        ctx,
        C,
        x0,
        96,
        [
          [t('HORIZ'), hs != null ? `${num(hs, 1)} m/s` : '—'],
          ['g', `${num(g, 2)} m/s²`, C.dim],
          [t('ACEL MÁX'), aMax > 0 ? `${num(aMax, 2)} m/s²` : '—', aMax > 0 && aMax < g ? C.red : C.green],
          [t('IMPACTO'), falling && vs < 0 && alt > 0 ? duration(tImpact) : '—', falling && tImpact < 30 && vs < 0 ? C.amber : C.green],
          [t('PARADA'), stop != null ? dist(stop) : '—', C.red],
          [t('MARGEM'), margin != null ? dist(margin) : '—', margin != null && margin < alt * 0.15 ? C.red : C.green],
          [t('ALVO'), toTarget != null ? dist(toTarget) : '—', C.cyan],
          [t('PERNAS'), v.landing === 'legs' ? t('SIM') : v.gear_down ? t('TREM EMBAIXO') : '—', C.dim],
          [t('PROGRAMA'), v.guidance?.program && v.guidance.program !== 'none' ? v.guidance.program.toUpperCase() : '—', v.guidance?.program === 'powereddescent' ? C.amber : C.dim],
        ],
        86,
      )
      if (margin != null && margin < 0) {
        ctx.font = 'bold 16px ui-monospace, monospace'
        ctx.fillStyle = C.red
        ctx.fillText(t('QUEIMAR AGORA'), x0, h - 12)
      }
    },
    [v, height, pal],
  )
  return (
    <div className="canvas-wrap" style={{ height }}>
      <canvas ref={ref} role="img" aria-label={t('Descida e pouso')} />
    </div>
  )
}

/** Ângulo central entre dois pontos (rad). */
function greatCircle(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const r = Math.PI / 180
  const a = Math.sin(((lat2 - lat1) * r) / 2) ** 2 + Math.cos(lat1 * r) * Math.cos(lat2 * r) * Math.sin(((lon2 - lon1) * r) / 2) ** 2
  return 2 * Math.asin(Math.min(1, Math.sqrt(a)))
}

// ── ENT ──

/** Reentrada: aquecimento, escudo, carga g, pressão dinâmica, inclinação lateral e paraquedas. */
export function Entry({ v }: { v: Vehicle }) {
  const hs = v.heat_shield
  const g = v.guidance
  const accel = v.tm?.['nav.accel']
  return (
    <div className="mfd-res">
      <KV
        items={[
          [t('Carga'), accel != null ? `${num(accel / 9.80665, 2)} g` : '—', accel != null && accel > 4 * 9.8 ? 'warn' : undefined],
          ['q', ...tmv(v, 'nav.q', 1)],
          ['Mach', ...tmv(v, 'nav.mach', 2)],
          [t('Fluxo'), ...tmv(v, 'ent.flux', 0)],
          [t('Temp. escudo'), ...tmv(v, 'ent.temp', 0)],
          [t('Inclinação lateral'), g ? `${num(g.bank, 0)}°` : '—'],
          [t('Guiamento'), g ? (g.ballistic ? t('BALÍSTICA') : t('GUIADA')) : '—', 'hi'],
          [t('Apagão'), v.systems?.comms.blackout ? t('SIM') : t('não'), v.systems?.comms.blackout ? 'warn' : 'dim'],
        ]}
      />
      {hs && (
        <Sec title={t('Escudo térmico')}>
          <Row label={t('Ablador')} frac={hs.ablator} text={`${num(hs.ablator * 100, 0)} %`} warn={0.3} />
          <KV items={[[t('Pico do fluxo'), num(hs.peak, 0)]]} />
        </Sec>
      )}
      {g?.landing_target && (
        <Sec title={t('Local de pouso')}>
          <KV
            items={[
              [t('Alvo'), latlon(g.landing_target[0], g.landing_target[1])],
              [t('Distância'), distance((greatCircle(v.lat, v.lon, g.landing_target[0], g.landing_target[1]) * (v.central_radius ?? 6.371e6)) / 1000)],
            ]}
          />
        </Sec>
      )}
      {(v.chutes ?? []).length > 0 && (
        <Sec title={t('Paraquedas')}>
          {v.chutes!.map((c, i) => (
            <Row key={i} label={tx(c.name)} frac={c.inflation} text={`${t(chuteState(c.state))} · ${num(c.alt / 1000, 1)} km`} tone={c.state === 'failed' || c.state === 'cut' ? 'bad' : undefined} />
          ))}
        </Sec>
      )}
      {!hs && !(v.chutes ?? []).length && <div className="mfd-msg">{t('SEM ESCUDO NEM PARAQUEDAS')}</div>}
    </div>
  )
}

// ── TGT ──

/** Alvo: posição relativa no plano V-bar/R-bar do alvo, distância, taxa e a aproximação máxima. */
export function Target({ v, target, height }: { v: Vehicle; target: Vehicle | undefined; height: number }) {
  const pal = usePilotPrefs().palette
  const ref = useCanvas(
    (ctx, w, h) => {
      const C = palette()
      ctx.fillStyle = '#000'
      ctx.fillRect(0, 0, w, h)
      const rel = target ? relative(v, target) : null
      if (!target || !rel) {
        message(ctx, C, w, h, [t('SEM ALVO'), t('Nenhum veículo em volta de {b}.', { b: tx(v.central_name ?? v.central) })])
        return
      }
      // Plano do alvo: V-bar para a direita (à frente do alvo), R-bar para cima.
      const [x, , z] = rel.pos
      const raw = Math.max(Math.abs(x), Math.abs(z), 50) * 1.3
      const ext = niceUp(raw)
      const pw = Math.min(w * 0.58, h - 30)
      const cx = pw / 2 + 10
      const cy = h / 2
      const k = pw / 2 / ext
      ctx.strokeStyle = C.grid
      ctx.lineWidth = 1
      for (const f of [0.25, 0.5, 0.75, 1]) ctx.strokeRect(cx - f * ext * k, cy - f * ext * k, 2 * f * ext * k, 2 * f * ext * k)
      ctx.strokeStyle = C.dim
      ctx.beginPath()
      ctx.moveTo(cx - ext * k, cy)
      ctx.lineTo(cx + ext * k, cy)
      ctx.moveTo(cx, cy - ext * k)
      ctx.lineTo(cx, cy + ext * k)
      ctx.stroke()
      ctx.font = '10px ui-monospace, monospace'
      ctx.fillStyle = C.dim
      ctx.fillText('+V', cx + ext * k - 18, cy - 4)
      ctx.fillText('+R', cx + 4, cy - ext * k + 10)
      ctx.fillText(`±${dist(ext)}`, cx - ext * k + 2, cy + ext * k - 4)
      // O alvo no centro; a nave e para onde ela vai em linha reta nos próximos minutos.
      ctx.fillStyle = C.amber
      ctx.fillRect(cx - 4, cy - 4, 8, 8)
      const px = cx + x * k
      const py = cy - z * k
      const horizon = Math.min(600, Math.max(30, rel.tca > 0 ? rel.tca : 120))
      ctx.strokeStyle = C.cyan
      ctx.setLineDash([4, 4])
      ctx.beginPath()
      ctx.moveTo(px, py)
      ctx.lineTo(px + rel.vel[0] * horizon * k, py - rel.vel[2] * horizon * k)
      ctx.stroke()
      ctx.setLineDash([])
      ctx.fillStyle = C.green
      ctx.beginPath()
      ctx.arc(px, py, 4, 0, Math.PI * 2)
      ctx.fill()
      // Números.
      const closing = rel.rate < 0
      const nose = v.q ? offsets(rel.los, v.q as Q).angle : null
      const x0 = pw + 24
      ctx.font = 'bold 14px ui-monospace, monospace'
      ctx.fillStyle = C.amber
      ctx.fillText(target.name.toUpperCase(), x0, 22)
      ctx.font = 'bold 24px ui-monospace, monospace'
      ctx.fillStyle = C.green
      ctx.fillText(dist(rel.range), x0, 52)
      ctx.font = 'bold 16px ui-monospace, monospace'
      ctx.fillStyle = closing ? C.cyan : C.amber
      ctx.fillText(`${rel.rate >= 0 ? '+' : ''}${num(rel.rate, 2)} m/s`, x0, 74)
      readout(
        ctx,
        C,
        x0,
        100,
        [
          ['V', dist(rel.pos[0])],
          ['H', dist(rel.pos[1])],
          ['R', dist(rel.pos[2])],
          [t('VREL'), `${num(Math.hypot(...(rel.vel as V3)), 2)} m/s`],
          ['TCA', rel.tca > 0 ? duration(rel.tca) : '—'],
          [t('MÍN'), dist(rel.miss), rel.miss < 100 ? C.amber : C.green],
          [t('NARIZ'), nose != null ? `${num(nose, 1)}°` : '—'],
          [t('ESTADO'), v.attached_to === target.id ? t('ACOPLADO') : v.guidance?.dock_target === target.id ? t('APROXIMANDO') : closing ? t('FECHANDO') : t('ABRINDO'), C.dim],
        ],
        52,
      )
    },
    [v, target, height, pal],
  )
  return (
    <div className="canvas-wrap" style={{ height }}>
      <canvas ref={ref} role="img" aria-label={t('Alvo e aproximação')} />
    </div>
  )
}

/** Próximo valor "redondo" (1, 2 ou 5 vezes uma potência de 10). */
function niceUp(x: number): number {
  const p = 10 ** Math.floor(Math.log10(x))
  for (const m of [1, 2, 5, 10]) if (m * p >= x) return m * p
  return 10 * p
}

// ── SIS ──

/** Energia, atitude (rodas e RCS) e os nós térmicos. */
export function Systems({ v }: { v: Vehicle }) {
  const s = v.systems
  if (!s) return <div className="mfd-msg">{t('SEM SUBSISTEMAS')}</div>
  const on = s.loads.filter((l) => l.on)
  const a = s.adcs
  return (
    <div className="mfd-res">
      <Sec title={t('Energia')}>
        <KV
          items={[
            [t('Geração'), ...tmv(v, 'eps.gen', 0)],
            [t('Consumo'), ...tmv(v, 'eps.load', 0)],
            [t('Cargas'), `${on.length}/${s.loads.length} · ${num(on.reduce((x, l) => x + l.watts, 0), 0)} W`],
            [t('Painéis'), s.solar ? (s.solar.stuck ? t('PRESOS') : !s.solar.deployed ? t('FECHADOS') : s.solar.tracking ? t('NO SOL') : t('ABERTOS')) : '—', s.solar?.stuck ? 'bad' : undefined],
          ]}
        />
        {v.tm?.['eps.soc'] != null && <Row label={t('Bateria')} frac={(v.tm['eps.soc'] ?? 0) / 100} text={`${num(v.tm['eps.soc'], 0)} % · ${num(s.battery_wh, 0)} Wh`} warn={0.3} />}
        {s.fuel_cell && <Row label={t('Célula a combustível')} frac={s.fuel_cell.reactant / (s.fuel_cell.max || 1)} text={s.fuel_cell.on ? 'ON' : 'OFF'} warn={0.15} />}
        {s.brownout && <div className="mfd-alert">{t('SUBTENSÃO NO BARRAMENTO')}</div>}
      </Sec>
      <Sec title={`${t('Atitude')} · ${tx(a.mode_name).toUpperCase()}`}>
        <KV
          items={[
            [t('Taxa'), `${num(a.rate, 3)} °/s`],
            ['RCS', `${num(a.rcs_prop, 1)} kg`, a.rcs_prop > 0 && a.rcs_prop < 5 ? 'warn' : undefined],
            [t('Estrelas'), a.star_tracker ? t('ok') : t('cego'), a.star_tracker ? undefined : 'bad'],
            [t('Descarga'), a.desat ? t('em curso') : '—'],
          ]}
        />
        {a.wheels &&
          (['X', 'Y', 'Z'] as const).map((ax, i) => (
            <Row key={ax} label={`${t('Roda')} ${ax}`} frac={Math.abs(a.wheels!.h[i]) / (a.wheels!.max || 1)} text={a.wheels!.failed[i] ? t('FALHA') : `${num(a.wheels!.h[i], 2)} N·m·s`} tone={a.wheels!.failed[i] ? 'bad' : Math.abs(a.wheels!.h[i]) > 0.8 * a.wheels!.max ? 'warn' : undefined} />
          ))}
      </Sec>
      {s.thermal.length > 0 && (
        <Sec title={t('Térmico')}>
          {s.thermal.map((n) => {
            const out = n.temp < n.min || n.temp > n.max
            return <Row key={n.id} label={`${tx(n.name)}${n.heater ? ' ♨' : ''}`} frac={(n.temp - n.min) / (n.max - n.min || 1)} text={n.failed ? t('FALHA') : `${num(n.temp, 1)} °C`} tone={n.failed || out ? 'bad' : undefined} />
          })}
        </Sec>
      )}
    </div>
  )
}

// ── COM ──

/** Enlace com o solo, a estação, a margem, a taxa e o gravador de bordo. */
export function Comms({ v }: { v: Vehicle }) {
  const c = v.systems?.comms
  if (!c) return <div className="mfd-msg">{t('SEM SUBSISTEMAS')}</div>
  const l = c.link
  return (
    <div className="mfd-res">
      {l ? (
        <KV
          items={[
            [t('Estação'), l.station, 'hi'],
            [t('Via'), l.via_relay ? t('satélite relé') : l.via_hga ? t('alto ganho') : t('baixo ganho')],
            [t('Elevação'), `${num(l.elevation_deg, 1)}°`],
            [t('Azimute'), `${num(l.azimuth_deg, 0)}°`],
            [t('Distância'), distance(l.range_m / 1000)],
            [t('Margem'), `${num(l.margin_db, 1)} dB`, l.margin_db < 0 ? 'bad' : l.margin_db < 3 ? 'warn' : undefined],
            [t('Taxa'), rate(l.rate_bps)],
            [t('Atraso (ida)'), duration(l.owlt)],
          ]}
        />
      ) : (
        <div className="mfd-alert">{c.blackout ? t('APAGÃO DO PLASMA') : c.failed ? t('TRANSMISSOR EM FALHA') : t('SEM ENLACE')}</div>
      )}
      <Sec title={t('Bordo')}>
        <KV
          items={[
            [t('Transmissor'), c.tx_on ? 'ON' : 'OFF', c.tx_on ? undefined : 'warn'],
            [t('Taxa'), c.auto_rate ? t('automática') : rate(c.rate * 1000)],
            [t('Alto ganho'), c.hga ? (c.hga_deployed ? t('aberta') : t('fechada')) : '—'],
            [t('Geração de dados'), `${num(Math.max(0, c.gen), 1)} Mbit/s`],
          ]}
        />
        {c.storage > 0 && <Row label={t('Gravador')} frac={c.stored / c.storage} text={`${num(c.stored / 1000, 2)} / ${num(c.storage / 1000, 1)} GB${c.dumping ? ' ⇣' : ''}`} tone={c.stored / c.storage > 0.9 ? 'warn' : undefined} />}
      </Sec>
    </div>
  )
}

// ── TLM ──

/** Pontos de telemetria escolhidos, com o limite em cor e a curva dos últimos 15 minutos. */
export function TlmPage({ v, edit }: { v: Vehicle; edit: boolean }) {
  const { tlm } = usePilotPrefs()
  const now = useStore((s) => s.snap?.t ?? 0)
  const dict = useStore((s) => s.dictionary[String(v.id)] ?? [])
  return (
    <div className="mfd-res">
      {tlm.map((id) => {
        const def = dict.find((d) => d.id === id)
        const [text, tone] = tmv(v, id, 2)
        const [, vs] = store.history.series(v.id, id, now - 900)
        return (
          <div key={id} className="mfd-tlm">
            <span className="lbl" title={id}>
              {def ? tx(def.name) : id}
            </span>
            <Spark values={vs} />
            <span className={`val ${tone ?? ''}`}>{text}</span>
            {edit && (
              <button className="x" onClick={() => setPilotPrefs({ tlm: tlm.filter((x) => x !== id) })} aria-label={t('Tirar')}>
                ×
              </button>
            )}
          </div>
        )
      })}
      {edit && (
        <select value="" onChange={(e) => e.target.value && setPilotPrefs({ tlm: [...tlm, e.target.value] })} aria-label={t('Acrescentar ponto')}>
          <option value="">{t('+ acrescentar ponto…')}</option>
          {dict
            .filter((d) => !tlm.includes(d.id))
            .map((d) => (
              <option key={d.id} value={d.id}>
                {tx(d.group)} · {tx(d.name)}
              </option>
            ))}
        </select>
      )}
      {!tlm.length && !edit && <div className="mfd-msg">{t('Nenhum ponto: toque em EDITAR.')}</div>}
    </div>
  )
}

function Spark({ values }: { values: number[] }) {
  const pts = values.filter((x) => isFinite(x))
  if (pts.length < 2) return <svg className="spark" />
  const step = Math.max(1, Math.floor(pts.length / 80))
  const s = pts.filter((_, i) => i % step === 0)
  const lo = Math.min(...s)
  const hi = Math.max(...s)
  const d = s.map((y, i) => `${((i / (s.length - 1)) * 100).toFixed(1)},${(hi > lo ? 18 - ((y - lo) / (hi - lo)) * 16 : 10).toFixed(1)}`).join(' ')
  return (
    <svg className="spark" viewBox="0 0 100 20" preserveAspectRatio="none">
      <polyline points={d} />
    </svg>
  )
}

// ── ALM ──

/** Alarmes do veículo, os mais novos em cima, com o reconhecimento de cada um. */
export function Alarms({ v }: { v: Vehicle }) {
  const alarms = useStore((s) => (s.snap?.alarms ?? []).filter((a) => a.vehicle === v.id && a.active))
  if (!alarms.length) return <div className="mfd-msg">{t('NENHUM ALARME')}</div>
  return (
    <div className="mfd-res">
      {[...alarms]
        .sort((a, b) => b.t - a.t)
        .map((a) => (
          <div key={a.id} className={`mfd-alm ${a.level} ${a.acked ? 'acked' : ''}`}>
            <span className="tm">{utc(a.t, false)}</span>
            <span className="lbl">{tx(a.name)}</span>
            <span className="val">
              {num(a.value, 2)} {a.unit}
            </span>
            {a.acked ? (
              <span className="ack">ACK</span>
            ) : (
              <button className="x" onClick={() => store.sim('ack', { value: a.id })}>
                ACK
              </button>
            )}
          </div>
        ))}
    </div>
  )
}
