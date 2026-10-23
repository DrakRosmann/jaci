// MFD (tela multifunção) do controle manual: uma moldura com teclas nas
// bordas, como as dos painéis reais, e páginas — ADI (horizonte artificial),
// ORB (mapa orbital), MNV (manobras), ENG (motores), RES (recursos), DSC
// (descida), ENT (reentrada), TGT (alvo), SIS, COM, TLM, ALM, 3D e MAPA. As
// teclas de cima são as páginas preferidas de cada tela; o MENU tem todas e
// escolhe quais ficam na moldura.

import { Suspense, lazy, useEffect, useRef, useState, type ReactNode } from 'react'
import { useCanvas } from './Chart'
import { MapWidget } from './Widgets'
import { CmdButton } from './ui'
import { Alarms, Comms, Descent, Engines, Entry, Maneuver, Systems, Target, TlmPage } from './MfdPages'
import { cross, dot, elements, eulerLocal, markers, unit, type Q, type V3 } from '../lib/attitude'
import { t, tx } from '../lib/i18n'
import { duration, num } from '../lib/format'
import { MU, nextNode, nodeDirection, offsets, palette, pickTarget, relative, targetCandidates } from '../lib/mfd'
import { MAX_KEYS, PAGES, setMfdPrefs, setPilotPrefs, usePilotPrefs, type MfdPage } from '../lib/pilot'
import { store, useStore } from '../lib/store'
import type { Vehicle } from '../types'

const Vehicle3D = lazy(() => import('./Vehicle3D').then((m) => ({ default: m.Vehicle3D })))

const label = (p: MfdPage) => t(PAGES.find((x) => x[0] === p)?.[1] ?? p)

export function Mfd({ v, index, height = 420 }: { v: Vehicle | undefined; index: number; height?: number }) {
  const prefs = usePilotPrefs()
  const mine = prefs.mfds[index]
  const page = mine.page
  const [menu, setMenu] = useState(false)
  const [orbZoom, setOrbZoom] = useState(1)
  const [node, setNode] = useState(0)
  const [editTlm, setEditTlm] = useState(false)
  const vehicles = useStore((s) => s.snap?.vehicles ?? [])
  // A altura da tela é a que sobra entre as teclas (medida: as teclas podem quebrar linha).
  const screen = useRef<HTMLDivElement>(null)
  const [inner, setInner] = useState(height - 90)
  useEffect(() => {
    const el = screen.current
    if (!el) return
    const ro = new ResizeObserver(() => setInner(Math.max(60, el.clientHeight)))
    ro.observe(el)
    return () => ro.disconnect()
  }, [])
  const go = (p: MfdPage) => {
    setMfdPrefs(index, { page: p })
    setMenu(false)
  }
  const pin = (p: MfdPage) => {
    const has = mine.keys.includes(p)
    if (!has && mine.keys.length >= MAX_KEYS) return
    setMfdPrefs(index, { keys: has ? mine.keys.filter((k) => k !== p) : PAGES.map(([id]) => id).filter((id) => id === p || mine.keys.includes(id)) })
  }
  const nodes = (v?.nodes ?? []).filter((n) => !n.done).sort((a, b) => a.t - b.t)
  const selNode = nodes[Math.min(node, nodes.length - 1)]
  const tgt = v ? pickTarget(v, vehicles) : undefined
  const cycleTarget = (dir: number) => {
    if (!v) return
    const c = targetCandidates(v, vehicles)
    if (!c.length) return
    const i = c.findIndex((o) => o.id === tgt?.id)
    setPilotPrefs({ target: c[(i + dir + c.length) % c.length].id })
  }
  let bottom: ReactNode = (
    <span className="mfd-title">
      {v?.name ?? '—'} · {menu ? 'MENU' : label(page)}
    </span>
  )
  if (v && !menu) {
    if (page === 'orb')
      bottom = (
        <>
          <button onClick={() => setOrbZoom(orbZoom / 1.5)}>{t('ZOOM −')}</button>
          <button onClick={() => setOrbZoom(1)}>{t('AJUSTAR')}</button>
          <button onClick={() => setOrbZoom(orbZoom * 1.5)}>{t('ZOOM +')}</button>
        </>
      )
    else if (page === 'mnv' && nodes.length > 0)
      bottom = (
        <>
          <button onClick={() => setNode(Math.max(0, node - 1))} disabled={node === 0}>
            ◀
          </button>
          <span className="mfd-title">
            {Math.min(node, nodes.length - 1) + 1}/{nodes.length}
          </span>
          <button onClick={() => setNode(Math.min(nodes.length - 1, node + 1))} disabled={node >= nodes.length - 1}>
            ▶
          </button>
          {selNode && (
            <CmdButton vehicle={v.id} cmd="burn_delete" args={{ node: selNode.id }}>
              {t('APAGAR')}
            </CmdButton>
          )}
        </>
      )
    else if (page === 'tgt')
      bottom = (
        <>
          <button onClick={() => cycleTarget(-1)}>◀</button>
          <span className="mfd-title">{t('ALVO')}</span>
          <button onClick={() => cycleTarget(1)}>▶</button>
          {tgt && v.systems && (
            <CmdButton vehicle={v.id} cmd="att_mode" args={{ mode: 'target', target: tgt.id }} className={v.systems.adcs.mode === 'target' ? 'on' : ''}>
              {t('APONTAR')}
            </CmdButton>
          )}
        </>
      )
    else if (page === 'com' && v.systems)
      bottom = (
        <>
          <CmdButton vehicle={v.id} cmd="tx" args={{ on: v.systems.comms.tx_on ? 'off' : 'on' }} className={v.systems.comms.tx_on ? 'on' : ''}>
            TX
          </CmdButton>
          <CmdButton vehicle={v.id} cmd="dump" className={v.systems.comms.dumping ? 'on' : ''}>
            {t('DESCARREGAR')}
          </CmdButton>
          <CmdButton vehicle={v.id} cmd="rate" args={{}} className={v.systems.comms.auto_rate ? 'on' : ''} title={t('Taxa de dados automática')}>
            {t('TAXA AUTO')}
          </CmdButton>
        </>
      )
    else if (page === 'alm')
      bottom = <button onClick={() => store.sim('ack')}>{t('RECONHECER TODOS')}</button>
    else if (page === 'tlm')
      bottom = (
        <button className={editTlm ? 'on' : ''} onClick={() => setEditTlm(!editTlm)}>
          {editTlm ? t('PRONTO') : t('EDITAR')}
        </button>
      )
  }
  return (
    <div className={`mfd pal-${prefs.palette}`} style={{ height }}>
      <div className="mfd-keys top">
        {mine.keys.map((p) => (
          <button key={p} className={!menu && page === p ? 'on' : ''} onClick={() => go(p)} aria-pressed={!menu && page === p} title={t(PAGES.find((x) => x[0] === p)?.[2] ?? '')}>
            {label(p)}
          </button>
        ))}
        <button className={menu ? 'on' : ''} onClick={() => setMenu(!menu)} aria-pressed={menu} title={t('Todas as páginas')}>
          MENU
        </button>
      </div>
      <div className="mfd-screen" ref={screen}>
        {!v ? (
          <div className="mfd-msg">{t('SEM VEÍCULO')}</div>
        ) : menu ? (
          <div className="mfd-menu">
            {PAGES.map(([p, key, desc]) => (
              <div key={p} className={`mfd-menu-item ${page === p ? 'on' : ''}`}>
                <button className="go" onClick={() => go(p)}>
                  <b>{t(key)}</b> {t(desc)}
                </button>
                <button
                  className={`pin ${mine.keys.includes(p) ? 'on' : ''}`}
                  onClick={() => pin(p)}
                  disabled={!mine.keys.includes(p) && mine.keys.length >= MAX_KEYS}
                  title={mine.keys.includes(p) ? t('Tirar da moldura') : t('Pôr na moldura')}
                  aria-pressed={mine.keys.includes(p)}
                >
                  ★
                </button>
              </div>
            ))}
            <div className="mfd-menu-hint">{t('★ põe a página nas teclas de cima (até {n}).', { n: MAX_KEYS })}</div>
          </div>
        ) : page === 'adi' ? (
          <Adi v={v} height={inner} />
        ) : page === 'orb' ? (
          <OrbitPage v={v} height={inner} zoom={orbZoom} />
        ) : page === 'res' ? (
          <Resources v={v} />
        ) : page === 'eng' ? (
          <Engines v={v} />
        ) : page === 'mnv' ? (
          <Maneuver v={v} node={selNode} height={inner} />
        ) : page === 'dsc' ? (
          <Descent v={v} height={inner} />
        ) : page === 'ent' ? (
          <Entry v={v} />
        ) : page === 'tgt' ? (
          <Target v={v} target={tgt} height={inner} />
        ) : page === 'sys' ? (
          <Systems v={v} />
        ) : page === 'com' ? (
          <Comms v={v} />
        ) : page === 'tlm' ? (
          <TlmPage v={v} edit={editTlm} />
        ) : page === 'alm' ? (
          <Alarms v={v} />
        ) : page === '3d' ? (
          <Suspense fallback={<div className="mfd-msg">…</div>}>
            <Vehicle3D v={v} height={inner} />
          </Suspense>
        ) : (
          <MapWidget height={inner} />
        )}
      </div>
      <div className="mfd-keys bottom">{bottom}</div>
    </div>
  )
}

/** Horizonte artificial: céu e chão no referencial local, escada de arfagem, arco de rolamento, fita de rumo e marcadores. */
function Adi({ v, height }: { v: Vehicle; height: number }) {
  const prev = useRef<{ t: number; e: { pitch: number; heading: number; roll: number } } | null>(null)
  const snapT = useStore((s) => s.snap?.t ?? 0)
  const vehicles = useStore((s) => s.snap?.vehicles)
  const prefs = usePilotPrefs()
  const ref = useCanvas(
    (ctx, w, h) => {
      const C = palette()
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
      const cy = h / 2 - 16
      const R = Math.min(w, h - 40) * 0.38
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
      // Marcadores de órbita (prógrado, normal, radial), da próxima queima e do alvo, onde estiverem.
      const mk = markers(r, v.v as V3)
      const sym: [V3, string, string][] = []
      if (prefs.adiMarkers)
        sym.push(
          [mk.prograde, '⊕', C.green],
          [mk.retrograde, '⊗', C.green],
          [mk.normal, '▲', C.magenta],
          [mk.antinormal, '▼', C.magenta],
          [mk.radial_out, '◇', C.cyan],
          [mk.radial_in, '◆', C.cyan],
        )
      const nn = nextNode(v)
      const nd = prefs.adiNode && nn ? nodeDirection(v, nn) : null
      if (nd) sym.push([nd, '✚', C.cyan])
      const tg = prefs.adiTarget && vehicles ? pickTarget(v, vehicles) : undefined
      const rel = tg ? relative(v, tg) : null
      if (rel) sym.push([rel.los, '◎', C.amber])
      // Retrógrado em relação ao solo, perto do chão: para onde apontar o motor no pouso.
      const vs = v.v_surf as V3 | undefined
      const surf = prefs.adiMarkers && vs && v.alt < 200 && Math.hypot(...vs) > 1 ? (vs.map((x) => -x) as V3) : null
      if (surf) sym.push([surf, '⊘', C.line])
      ctx.font = 'bold 18px ui-monospace, monospace'
      ctx.textAlign = 'center'
      ctx.textBaseline = 'middle'
      for (const [d, s, color] of sym) {
        const o = offsets(d, q)
        if (!o.front || Math.hypot(o.dx, o.dy) > 34) continue
        ctx.fillStyle = color
        ctx.fillText(s, cx + o.dx * ppd, cy - o.dy * ppd)
      }
      ctx.textAlign = 'left'
      ctx.textBaseline = 'alphabetic'
      // Diretor de voo: as barras cruzadas mostram para onde o guiamento quer o nariz.
      const fd = prefs.adiDirector && v.guidance?.director ? offsets(v.guidance.director as V3, q) : null
      if (fd) {
        const lim = 30
        const dx = fd.front ? Math.max(-lim, Math.min(lim, fd.dx)) : Math.sign(fd.dx || 1) * lim
        const dy = fd.front ? Math.max(-lim, Math.min(lim, fd.dy)) : Math.sign(fd.dy || 1) * lim
        ctx.strokeStyle = C.magenta
        ctx.lineWidth = 3
        ctx.beginPath()
        ctx.moveTo(cx - R * 0.5, cy - dy * ppd)
        ctx.lineTo(cx + R * 0.5, cy - dy * ppd)
        ctx.moveTo(cx + dx * ppd, cy - R * 0.5)
        ctx.lineTo(cx + dx * ppd, cy + R * 0.5)
        ctx.stroke()
      }
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
      ctx.fillText(`${prefs.adiMarkers ? '⊕ PRO  ⊗ RET  ▲ NML  ◇ RAD  ' : ''}${surf ? `⊘ ${t('SOLO')}  ` : ''}${nd ? `✚ ${t('MNV')} ${num(offsets(nd, q).angle, 1)}°  ` : ''}${rel ? `◎ ${tg!.name}` : ''}`, 10, h - 8)
      if (fd) {
        ctx.fillStyle = fd.angle > 5 ? C.amber : C.magenta
        ctx.textAlign = 'right'
        ctx.font = 'bold 12px ui-monospace, monospace'
        ctx.fillText(`FD ${num(fd.angle, 1)}°${v.guidance?.manual ? ` · ${t('MANUAL')}` : ''}`, w - 10, 66)
        ctx.textAlign = 'left'
      }
    },
    [v, height, prefs, vehicles],
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
  const pal = usePilotPrefs().palette
  const ref = useCanvas(
    (ctx, w, h) => {
      const C = palette()
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
      ctx.strokeStyle = C.grid
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
    [v, zoom, bodies, others, height, pal],
  )
  return (
    <div className="canvas-wrap" style={{ height }}>
      <canvas ref={ref} role="img" aria-label={t('Mapa orbital')} />
    </div>
  )
}

const norm2 = (a: V3) => Math.hypot(a[0], a[1], a[2])

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
