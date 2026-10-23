// MFD (tela multifunção) do controle manual: uma moldura com teclas nas
// bordas, como as dos painéis reais, e páginas — ADI (horizonte artificial),
// ORB (mapa orbital), RES (recursos), 3D (a nave) e MAPA (traço no solo).

import { Suspense, lazy, useRef, useState } from 'react'
import { useCanvas } from './Chart'
import { MapWidget } from './Widgets'
import { bodyAxes, cross, dot, elements, eulerLocal, markers, unit, type Q, type V3 } from '../lib/attitude'
import { t, tx } from '../lib/i18n'
import { duration, num } from '../lib/format'
import { useStore } from '../lib/store'
import type { Vehicle } from '../types'

const Vehicle3D = lazy(() => import('./Vehicle3D').then((m) => ({ default: m.Vehicle3D })))

export type MfdPage = 'adi' | 'orb' | 'res' | '3d' | 'map'

const PAGES: [MfdPage, string][] = [
  ['adi', 'ADI'],
  ['orb', 'ORB'],
  ['res', 'RES'],
  ['3d', '3D'],
  ['map', 'MAPA'],
]

const C = {
  sky: '#1f5fa8',
  ground: '#7a4a22',
  line: '#e8f0ff',
  green: '#4cff8a',
  amber: '#ffc23d',
  cyan: '#4cc3ff',
  magenta: '#e879f9',
  red: '#ff4d5e',
  dim: '#7f8fae',
}

export function Mfd({ v, initial = 'adi', height = 420 }: { v: Vehicle | undefined; initial?: MfdPage; height?: number }) {
  const [page, setPage] = useState<MfdPage>(initial)
  const [orbZoom, setOrbZoom] = useState(1)
  const bodyH = height - 64
  return (
    <div className="mfd" style={{ height }}>
      <div className="mfd-keys top">
        {PAGES.map(([p, label]) => (
          <button key={p} className={page === p ? 'on' : ''} onClick={() => setPage(p)} aria-pressed={page === p}>
            {label}
          </button>
        ))}
      </div>
      <div className="mfd-screen" style={{ height: bodyH }}>
        {!v ? (
          <div className="mfd-msg">{t('SEM VEÍCULO')}</div>
        ) : page === 'adi' ? (
          <Adi v={v} height={bodyH} />
        ) : page === 'orb' ? (
          <OrbitPage v={v} height={bodyH} zoom={orbZoom} />
        ) : page === 'res' ? (
          <Resources v={v} />
        ) : page === '3d' ? (
          <Suspense fallback={<div className="mfd-msg">…</div>}>
            <Vehicle3D v={v} height={bodyH} />
          </Suspense>
        ) : (
          <MapWidget height={bodyH} />
        )}
      </div>
      <div className="mfd-keys bottom">
        {page === 'orb' ? (
          <>
            <button onClick={() => setOrbZoom(orbZoom / 1.5)}>{t('ZOOM −')}</button>
            <button onClick={() => setOrbZoom(1)}>{t('AJUSTAR')}</button>
            <button onClick={() => setOrbZoom(orbZoom * 1.5)}>{t('ZOOM +')}</button>
          </>
        ) : (
          <span className="mfd-title">
            {v?.name ?? '—'} · {PAGES.find((p) => p[0] === page)?.[1]}
          </span>
        )}
      </div>
    </div>
  )
}

/** Ângulos (graus) de uma direção em relação à frente da nave: para a direita e para cima na tela. */
function offsets(d: V3, q: Q): { dx: number; dy: number; front: boolean } {
  const { fwd, right, down } = bodyAxes(q)
  const f = dot(d, fwd)
  return { dx: (Math.atan2(dot(d, right), f) * 180) / Math.PI, dy: (Math.atan2(-dot(d, down), f) * 180) / Math.PI, front: f > 0 }
}

/** Horizonte artificial: céu e chão no referencial local, escada de arfagem, arco de rolamento, fita de rumo e marcadores. */
function Adi({ v, height }: { v: Vehicle; height: number }) {
  const prev = useRef<{ t: number; e: { pitch: number; heading: number; roll: number } } | null>(null)
  const snapT = useStore((s) => s.snap?.t ?? 0)
  const ref = useCanvas(
    (ctx, w, h) => {
      ctx.fillStyle = '#000'
      ctx.fillRect(0, 0, w, h)
      if (!v.q || !v.r || !v.v) {
        ctx.fillStyle = C.dim
        ctx.font = '13px ui-monospace, monospace'
        ctx.fillText(t('SEM DADOS DE ATITUDE'), 20, 30)
        return
      }
      const q = v.q as Q
      const r = v.r as V3
      const e = eulerLocal(q, r)
      // Taxas (diferença entre instantâneos).
      let rates = { p: 0, y: 0, r: 0 }
      if (prev.current && snapT > prev.current.t) {
        const dt = snapT - prev.current.t
        const dh = (((e.heading - prev.current.e.heading + 540) % 360) - 180) / dt
        rates = { p: (e.pitch - prev.current.e.pitch) / dt, y: dh, r: (((e.roll - prev.current.e.roll + 540) % 360) - 180) / dt }
      }
      prev.current = { t: snapT, e }
      const cx = w / 2
      const cy = h / 2 - 10
      const R = Math.min(w, h) * 0.38
      const ppd = R / 35 // pixels por grau
      // Esfera: céu, chão e escada, girados pelo rolamento.
      ctx.save()
      ctx.beginPath()
      ctx.arc(cx, cy, R, 0, Math.PI * 2)
      ctx.clip()
      ctx.translate(cx, cy)
      ctx.rotate((-e.roll * Math.PI) / 180)
      const hy = e.pitch * ppd
      ctx.fillStyle = C.sky
      ctx.fillRect(-2 * R, -3 * R, 4 * R, 3 * R + hy)
      ctx.fillStyle = C.ground
      ctx.fillRect(-2 * R, hy, 4 * R, 3 * R)
      ctx.strokeStyle = C.line
      ctx.fillStyle = C.line
      ctx.lineWidth = 2
      ctx.beginPath()
      ctx.moveTo(-2 * R, hy)
      ctx.lineTo(2 * R, hy)
      ctx.stroke()
      ctx.lineWidth = 1.2
      ctx.font = '10px ui-monospace, monospace'
      for (let k = -90; k <= 90; k += 10) {
        if (k === 0) continue
        const y = hy - k * ppd
        if (Math.abs(y) > R * 1.2) continue
        const len = k % 30 === 0 ? R * 0.45 : R * 0.25
        ctx.beginPath()
        ctx.moveTo(-len / 2, y)
        ctx.lineTo(len / 2, y)
        ctx.stroke()
        if (k % 30 === 0) {
          ctx.fillText(String(k), len / 2 + 4, y + 3)
          ctx.fillText(String(k), -len / 2 - 22, y + 3)
        }
      }
      ctx.restore()
      // Marcadores de órbita (prógrado, normal, radial), onde estiverem.
      const mk = markers(r, v.v as V3)
      const sym: [string, string, string][] = [
        ['prograde', '⊕', C.green],
        ['retrograde', '⊗', C.green],
        ['normal', '▲', C.magenta],
        ['antinormal', '▼', C.magenta],
        ['radial_out', '◇', C.cyan],
        ['radial_in', '◆', C.cyan],
      ]
      ctx.font = 'bold 18px ui-monospace, monospace'
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      for (const [k, s, color] of sym) {
        const o = offsets(mk[k], q)
        if (!o.front || Math.hypot(o.dx, o.dy) > 34) continue
        ctx.fillStyle = color
        ctx.fillText(s, cx + o.dx * ppd, cy - o.dy * ppd)
      }
      ctx.textAlign = 'left'
      ctx.textBaseline = 'alphabetic'
      // Aeronave fixa no centro.
      ctx.strokeStyle = C.amber
      ctx.lineWidth = 3
      ctx.beginPath()
      ctx.moveTo(cx - R * 0.42, cy)
      ctx.lineTo(cx - R * 0.12, cy)
      ctx.lineTo(cx - R * 0.06, cy + 8)
      ctx.moveTo(cx + R * 0.42, cy)
      ctx.lineTo(cx + R * 0.12, cy)
      ctx.lineTo(cx + R * 0.06, cy + 8)
      ctx.stroke()
      ctx.fillStyle = C.amber
      ctx.fillRect(cx - 2, cy - 2, 4, 4)
      // Arco de rolamento.
      ctx.strokeStyle = C.line
      ctx.lineWidth = 1.5
      ctx.beginPath()
      ctx.arc(cx, cy, R + 8, (-150 * Math.PI) / 180, (-30 * Math.PI) / 180)
      ctx.stroke()
      for (const a of [-60, -45, -30, -20, -10, 0, 10, 20, 30, 45, 60]) {
        const rad = ((a - 90) * Math.PI) / 180
        const l = a % 30 === 0 ? 10 : 6
        ctx.beginPath()
        ctx.moveTo(cx + Math.cos(rad) * (R + 8), cy + Math.sin(rad) * (R + 8))
        ctx.lineTo(cx + Math.cos(rad) * (R + 8 + l), cy + Math.sin(rad) * (R + 8 + l))
        ctx.stroke()
      }
      const rr = ((-e.roll - 90) * Math.PI) / 180
      ctx.fillStyle = C.amber
      ctx.beginPath()
      ctx.moveTo(cx + Math.cos(rr) * (R + 6), cy + Math.sin(rr) * (R + 6))
      ctx.lineTo(cx + Math.cos(rr + 0.06) * (R - 6), cy + Math.sin(rr + 0.06) * (R - 6))
      ctx.lineTo(cx + Math.cos(rr - 0.06) * (R - 6), cy + Math.sin(rr - 0.06) * (R - 6))
      ctx.fill()
      // Fita de rumo embaixo.
      const ty = cy + R + 26
      ctx.strokeStyle = C.dim
      ctx.strokeRect(cx - R, ty - 12, 2 * R, 24)
      ctx.font = '11px ui-monospace, monospace'
      ctx.fillStyle = C.line
      ctx.save()
      ctx.beginPath()
      ctx.rect(cx - R, ty - 12, 2 * R, 24)
      ctx.clip()
      for (let hd = Math.floor(e.heading / 10) * 10 - 60; hd <= e.heading + 60; hd += 10) {
        const x = cx + (hd - e.heading) * (R / 50)
        ctx.beginPath()
        ctx.moveTo(x, ty - 12)
        ctx.lineTo(x, ty - (hd % 30 === 0 ? 2 : 6))
        ctx.stroke()
        if (hd % 30 === 0) ctx.fillText(String(((hd % 360) + 360) % 360).padStart(3, '0'), x - 10, ty + 9)
      }
      ctx.restore()
      ctx.fillStyle = C.amber
      ctx.fillRect(cx - 1, ty - 14, 2, 8)
      // Números nos cantos.
      ctx.font = '12px ui-monospace, monospace'
      const lines: [string, string, string][] = [
        ['ARF', `${num(e.pitch, 1)}°`, `${rates.p >= 0 ? '+' : ''}${num(rates.p, 2)}°/s`],
        ['RUM', `${num(e.heading, 1)}°`, `${rates.y >= 0 ? '+' : ''}${num(rates.y, 2)}°/s`],
        ['ROL', `${num(e.roll, 1)}°`, `${rates.r >= 0 ? '+' : ''}${num(rates.r, 2)}°/s`],
      ]
      lines.forEach(([k, a, b], i) => {
        ctx.fillStyle = C.dim
        ctx.fillText(k, 10, 18 + i * 16)
        ctx.fillStyle = C.green
        ctx.fillText(a, 44, 18 + i * 16)
        ctx.fillStyle = C.cyan
        ctx.fillText(b, 110, 18 + i * 16)
      })
      const mode = v.systems?.adcs.mode_name ?? (v.guidance?.program && v.guidance.program !== 'none' ? `GUIAMENTO ${v.guidance.program}` : '')
      ctx.fillStyle = C.amber
      ctx.textAlign = 'right'
      ctx.fillText(tx(mode).toUpperCase(), w - 10, 18)
      ctx.fillStyle = C.dim
      ctx.fillText(`ALT ${num(v.alt, 1)} km`, w - 10, 34)
      ctx.fillText(`VEL ${num(v.speed, 0)} m/s`, w - 10, 50)
      ctx.textAlign = 'left'
      ctx.fillStyle = C.dim
      ctx.font = '10px ui-monospace, monospace'
      ctx.fillText('⊕ PRO  ⊗ RET  ▲ NML  ◇ RAD', 10, h - 8)
    },
    [v, height],
  )
  return (
    <div className="canvas-wrap" style={{ height }}>
      <canvas ref={ref} role="img" aria-label={t('Horizonte artificial')} />
    </div>
  )
}

/** Mapa orbital: a órbita vista de cima do plano, o corpo, a nave, apsides e os outros veículos. */
function OrbitPage({ v, height, zoom }: { v: Vehicle; height: number; zoom: number }) {
  const bodies = useStore((s) => s.hello?.bodies)
  const others = useStore((s) => s.snap?.vehicles)
  const ref = useCanvas(
    (ctx, w, h) => {
      ctx.fillStyle = '#000'
      ctx.fillRect(0, 0, w, h)
      const body = bodies?.find((b) => b.id === v.central)
      const R = v.central_radius ?? body?.radius ?? 6.378e6
      const mu = MU[v.central] ?? 3.986004418e14
      if (!v.r || !v.v) return
      // Na plataforma ou no chão não há órbita para desenhar.
      if (['prelaunch', 'landed', 'splashdown', 'destroyed'].includes(v.phase)) {
        ctx.fillStyle = C.dim
        ctx.font = '13px ui-monospace, monospace'
        ctx.textAlign = 'center'
        ctx.fillText(`${tx(v.phase_name).toUpperCase()} — ${t('SEM ÓRBITA')}`, w / 2, h / 2)
        ctx.textAlign = 'left'
        return
      }
      const r = v.r as V3
      const el = elements(r, v.v as V3, mu)
      const xAxis = el.e > 1e-6 ? unit(el.evec) : unit(r)
      const yAxis = cross(unit(el.h), xAxis)
      const plane = (p: V3): [number, number] => [dot(p, xAxis), dot(p, yAxis)]
      const extent = (el.e < 1 ? Math.max(el.apoapsis, R * 1.2) : Math.max(norm2(r) * 2.5, R * 2)) / zoom
      const k = (Math.min(w, h) * 0.45) / extent
      // Periastro à direita; centro do corpo deslocado para a elipse caber.
      const center = el.e < 1 ? -el.a * el.e : 0
      const cx = w / 2 - center * k * 0.5
      const cy = h / 2
      const P = (x: number, y: number): [number, number] => [cx + x * k, cy - y * k]
      // Grade de anéis.
      ctx.strokeStyle = '#13213b'
      for (let rr = R; rr < extent * 2; rr *= 2) {
        ctx.beginPath()
        ctx.arc(cx, cy, rr * k, 0, Math.PI * 2)
        ctx.stroke()
      }
      // Corpo central.
      ctx.fillStyle = v.central === 'earth' ? '#1b4f8a' : v.central === 'moon' ? '#777' : v.central === 'mars' ? '#a0522d' : '#666'
      ctx.beginPath()
      ctx.arc(cx, cy, Math.max(2, R * k), 0, Math.PI * 2)
      ctx.fill()
      // Órbita.
      const p = dot(el.h, el.h) / mu
      ctx.strokeStyle = C.green
      ctx.lineWidth = 2
      ctx.beginPath()
      const lim = el.e < 1 ? Math.PI : Math.acos(-1 / el.e) - 0.02
      for (let i = 0; i <= 360; i++) {
        const nu = -lim + (2 * lim * i) / 360
        const rr = p / (1 + el.e * Math.cos(nu))
        if (rr < 0 || rr > extent * 4) continue
        const [x, y] = P(rr * Math.cos(nu), rr * Math.sin(nu))
        if (i === 0) ctx.moveTo(x, y)
        else ctx.lineTo(x, y)
      }
      ctx.stroke()
      // Apsides.
      ctx.font = '11px ui-monospace, monospace'
      const pe = P(el.periapsis, 0)
      ctx.fillStyle = C.cyan
      ctx.fillText(`Pe ${num((el.periapsis - R) / 1e3, 0)} km`, pe[0] + 6, pe[1] - 6)
      ctx.fillRect(pe[0] - 3, pe[1] - 3, 6, 6)
      if (el.e < 1) {
        const ap = P(-el.apoapsis, 0)
        ctx.fillStyle = C.magenta
        ctx.fillRect(ap[0] - 3, ap[1] - 3, 6, 6)
        ctx.fillText(`Ap ${num((el.apoapsis - R) / 1e3, 0)} km`, ap[0] + 6, ap[1] - 6)
      }
      // Outros veículos no mesmo corpo, projetados no plano.
      for (const o of others ?? []) {
        if (o.id === v.id || !o.r || o.central !== v.central || o.kind === 'debris') continue
        const [x, y] = P(...plane(o.r as V3))
        ctx.fillStyle = C.amber
        ctx.beginPath()
        ctx.arc(x, y, 3, 0, Math.PI * 2)
        ctx.fill()
        ctx.fillText(o.name, x + 5, y + 4)
      }
      // A nave e a direção do movimento.
      const [vx, vy] = P(...plane(r))
      const [tx2, ty2] = plane(v.v as V3)
      const vn = Math.hypot(tx2, ty2) || 1
      ctx.strokeStyle = '#fff'
      ctx.beginPath()
      ctx.moveTo(vx, vy)
      ctx.lineTo(vx + (tx2 / vn) * 24, vy - (ty2 / vn) * 24)
      ctx.stroke()
      ctx.fillStyle = '#fff'
      ctx.beginPath()
      ctx.arc(vx, vy, 4, 0, Math.PI * 2)
      ctx.fill()
      // Números.
      const period = el.e < 1 ? 2 * Math.PI * Math.sqrt(el.a ** 3 / mu) : NaN
      const toApsis = (target: number) => {
        if (!(el.e < 1)) return NaN
        const E = (nu: number) => 2 * Math.atan(Math.sqrt((1 - el.e) / (1 + el.e)) * Math.tan(nu / 2))
        const M = (nu: number) => E(nu) - el.e * Math.sin(E(nu))
        let dm = M(target) - M(el.nu)
        while (dm < 0) dm += 2 * Math.PI
        return (dm / (2 * Math.PI)) * period
      }
      const rows: [string, string][] = [
        ['e', num(el.e, 4)],
        ['P', isFinite(period) ? duration(period) : '—'],
        ['→Pe', el.e > 1e-3 ? duration(toApsis(0)) : '—'],
        ['→Ap', el.e > 1e-3 && el.e < 1 ? duration(toApsis(Math.PI - 1e-9)) : '—'],
        ['INC', `${num(v.inc ?? 0, 2)}°`],
      ]
      ctx.font = '12px ui-monospace, monospace'
      rows.forEach(([a, b], i) => {
        ctx.fillStyle = C.dim
        ctx.fillText(a, 10, 18 + i * 16)
        ctx.fillStyle = C.green
        ctx.fillText(b, 48, 18 + i * 16)
      })
      ctx.fillStyle = C.dim
      ctx.textAlign = 'right'
      ctx.fillText(tx(v.central_name ?? v.central).toUpperCase(), w - 10, 18)
      ctx.textAlign = 'left'
    },
    [v, zoom, bodies, others, height],
  )
  return (
    <div className="canvas-wrap" style={{ height }}>
      <canvas ref={ref} role="img" aria-label={t('Mapa orbital')} />
    </div>
  )
}

const norm2 = (a: V3) => Math.hypot(a[0], a[1], a[2])

/** μ dos corpos (m³/s²). */
const MU: Record<string, number> = {
  earth: 3.986004418e14,
  moon: 4.9048695e12,
  mars: 4.282837e13,
  venus: 3.24859e14,
  mercury: 2.2032e13,
  jupiter: 1.26686534e17,
  saturn: 3.7931187e16,
  uranus: 5.793939e15,
  neptune: 6.836529e15,
  sun: 1.32712440018e20,
}

/** Recursos da nave em barras: propelente por estágio, RCS, bateria, ar e água, gravador, escudo e Δv. */
function Resources({ v }: { v: Vehicle }) {
  const tm = (k: string) => v.tm?.[k] ?? null
  const rows: { label: string; frac: number | null; text: string; warn?: number }[] = []
  for (const s of v.stages ?? []) {
    if (!s.capacity || s.capacity <= 0) continue
    rows.push({ label: tx(s.name), frac: (s.prop ?? 0) / s.capacity, text: `${num(s.prop ?? 0, 0)} kg`, warn: 0.1 })
  }
  const sys = v.systems
  if (sys) {
    if (sys.adcs.rcs_prop > 0) rows.push({ label: 'RCS', frac: Math.min(1, sys.adcs.rcs_prop / Math.max(sys.adcs.rcs_prop, 50)), text: `${num(sys.adcs.rcs_prop, 1)} kg`, warn: 0.15 })
    if (tm('eps.soc') != null) rows.push({ label: t('Bateria'), frac: (tm('eps.soc') ?? 0) / 100, text: `${num(tm('eps.soc'), 0)} %`, warn: 0.3 })
    if (sys.comms.storage > 0) rows.push({ label: t('Gravador'), frac: sys.comms.stored / sys.comms.storage, text: `${num(sys.comms.stored / 1000, 1)} GB` })
  }
  if (tm('ecl.o2_tank') != null) rows.push({ label: 'O₂', frac: null, text: `${num(tm('ecl.o2_tank'), 1)} kg` })
  if (tm('ecl.n2_tank') != null) rows.push({ label: 'N₂', frac: null, text: `${num(tm('ecl.n2_tank'), 1)} kg` })
  if (tm('ecl.water') != null) rows.push({ label: t('Água'), frac: null, text: `${num(tm('ecl.water'), 0)} kg` })
  if (v.heat_shield) rows.push({ label: t('Escudo'), frac: v.heat_shield.ablator, text: `${num(v.heat_shield.ablator * 100, 0)} %`, warn: 0.3 })
  return (
    <div className="mfd-res">
      {rows.map((r, i) => {
        const low = r.frac != null && r.warn != null && r.frac < r.warn
        return (
          <div key={i} className="mfd-res-row">
            <span className="lbl">{r.label}</span>
            <span className="bar-track">{r.frac != null && <span className={`bar-fill ${low ? 'low' : ''}`} style={{ width: `${Math.max(0, Math.min(1, r.frac)) * 100}%` }} />}</span>
            <span className={`val ${low ? 'low' : ''}`}>{r.text}</span>
          </div>
        )
      })}
      <div className="mfd-res-row total">
        <span className="lbl">Δv</span>
        <span />
        <span className="val">{num(tm('nav.dv'), 0)} m/s</span>
      </div>
    </div>
  )
}
