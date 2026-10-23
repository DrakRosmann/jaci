// Mapa-múndi (projeção equiretangular) em canvas: continentes, noite, estações,
// traços previstos, veículos, ponto de impacto instantâneo e corredor. Em
// outro corpo (os do KSP), só a grade, os traços e os veículos dele.

import { useEffect, useRef, useState } from 'react'
import { useCanvas } from './Chart'
import { circle, footprintDeg, splitDateline, subsolar, terminator, wrapLon } from '../lib/geo'
import { t } from '../lib/i18n'
import { store, useStore } from '../lib/store'
import type { Vehicle } from '../types'

let land: [number, number][][] | null = null
let landPromise: Promise<void> | null = null
function loadLand(): Promise<void> {
  if (!landPromise) {
    landPromise = fetch('/land.json')
      .then((r) => r.json())
      .then((d) => {
        land = d
      })
      .catch(() => {
        land = []
      })
  }
  return landPromise
}

export interface Track {
  vehicle: number
  points: [number, number][]
  color: string
}

const KIND_COLOR: Record<string, string> = {
  station: '#c084fc',
  capsule: '#ffc23d',
  shuttle: '#ff8a3d',
  satellite: '#4cc3ff',
  probe: '#2dd4bf',
  sounding: '#ff8a3d',
  debris: '#5b6a88',
  external: '#3ddc84',
  ksp: '#a3e635',
  tracked: '#f472b6',
}

/** Lugares conhecidos nos corpos do KSP. */
const KSP_SITES: Record<string, [string, number, number][]> = {
  kerbin: [
    ['KSC', -0.0972, -74.5577],
    ['Deserto', -6.5604, -143.95],
    ['Woomerang', 45.29, 136.11],
  ],
}

export function kindColor(kind: string): string {
  return KIND_COLOR[kind] ?? '#d9e2f2'
}

export function MapView({
  tracks = [],
  height = 360,
  showStations = true,
  showDebris = false,
  center = 0,
  body = 'earth',
}: {
  tracks?: Track[]
  height?: number
  showStations?: boolean
  showDebris?: boolean
  center?: number
  body?: string
}) {
  const earth = body === 'earth'
  const [ready, setReady] = useState(land != null)
  useEffect(() => {
    loadLand().then(() => setReady(true))
  }, [])
  const snap = useStore((s) => s.snap)
  const hello = useStore((s) => s.hello)
  const selected = useStore((s) => s.ui.selected)
  // Fundo, grade e continentes não mudam a cada instantâneo: ficam numa camada
  // guardada, refeita só quando muda o tamanho, o centro ou o corpo.
  const base = useRef<{ key: string; canvas: HTMLCanvasElement } | null>(null)
  const ref = useCanvas(
    (ctx, w, h) => {
      const X = (lon: number) => ((wrapLon(lon - center) + 180) / 360) * w
      const Y = (lat: number) => ((90 - lat) / 180) * h
      const dpr = window.devicePixelRatio || 1
      const theme = document.documentElement.dataset.theme ?? ''
      const key = `${w}x${h}@${dpr}:${center}:${earth}:${land != null}:${theme}`
      if (base.current?.key !== key) {
        const c = base.current?.canvas ?? document.createElement('canvas')
        c.width = Math.max(1, Math.round(w * dpr))
        c.height = Math.max(1, Math.round(h * dpr))
        const b = c.getContext('2d')
        if (b) {
          b.setTransform(dpr, 0, 0, dpr, 0, 0)
          drawBase(b, w, h, X, Y, earth, center)
        }
        base.current = { key, canvas: c }
      }
      ctx.drawImage(base.current.canvas, 0, 0, w, h)
      if (!snap) return
      if (!earth) {
        ctx.font = '10px ui-monospace, monospace'
        for (const [name, lat, lon] of KSP_SITES[body] ?? []) {
          ctx.fillStyle = '#9fb4dd'
          ctx.fillRect(X(lon) - 2.5, Y(lat) - 2.5, 5, 5)
          ctx.fillText(name, X(lon) + 6, Y(lat) + 3)
        }
      }
      // Noite.
      if (earth) {
        const [sLat, sLon] = subsolar(snap.sun, snap.gmst)
        ctx.fillStyle = 'rgba(0,0,0,0.38)'
        ctx.beginPath()
        const night = sLat > 0 ? 90 : -90
        ctx.moveTo(0, Y(-night))
        for (let px = 0; px <= w; px += 4) {
          const lon = (px / w) * 360 - 180 + center
          ctx.lineTo(px, Y(terminator(sLat, sLon, lon)))
        }
        ctx.lineTo(w, Y(-night))
        ctx.closePath()
        ctx.fill()
        ctx.fillStyle = '#ffd24d'
        ctx.beginPath()
        ctx.arc(X(sLon), Y(sLat), 4, 0, Math.PI * 2)
        ctx.fill()
      }
      // Estações.
      if (earth && showStations && hello) {
        ctx.font = '10px ui-monospace, monospace'
        for (const st of hello.stations) {
          const live = snap.stations_live.find((s) => s.id === st.id)
          if (live && !live.enabled) continue
          ctx.fillStyle = st.deep_space ? '#2dd4bf' : '#9fb4dd'
          ctx.fillRect(X(st.lon) - 2.5, Y(st.lat) - 2.5, 5, 5)
        }
      }
      // Corredor de lançamento.
      if (earth && snap.corridor) {
        const c = snap.corridor
        ctx.strokeStyle = 'rgba(255,138,61,0.5)'
        ctx.setLineDash([4, 4])
        for (const side of [-1, 1]) {
          const pts: [number, number][] = []
          for (let d = 0; d <= 3000; d += 50) {
            const [la, lo] = destination(c.lat, c.lon, c.azimuth, d)
            const [la2, lo2] = destination(la, lo, c.azimuth + 90 * side, c.half_width_km)
            pts.push([lo2, la2])
          }
          drawLine(ctx, pts, X, Y)
        }
        ctx.setLineDash([])
      }
      const sel = snap.vehicles.find((v) => v.id === selected)
      // Pegada de visibilidade do selecionado.
      if (earth && sel && sel.central === 'earth' && sel.alt > 50 && sel.kind !== 'debris') {
        ctx.strokeStyle = 'rgba(76,195,255,0.45)'
        drawLine(ctx, circle(sel.lat, sel.lon, footprintDeg(sel.alt, 5)), X, Y)
      }
      // Traços previstos.
      for (const tr of tracks) {
        ctx.strokeStyle = tr.color
        ctx.lineWidth = 1.4
        drawLine(ctx, tr.points, X, Y)
      }
      ctx.lineWidth = 1
      // Alvos de pouso e pontos de impacto.
      for (const v of snap.vehicles) {
        const lt = v.guidance?.landing_target
        if (lt && v.id === selected) {
          cross(ctx, X(lt[1]), Y(lt[0]), '#3ddc84')
        }
        if (v.iip) {
          ctx.strokeStyle = v.iip.inside ? '#ffc23d' : '#ff4d5e'
          ctx.beginPath()
          ctx.arc(X(v.iip.lon), Y(v.iip.lat), 6, 0, Math.PI * 2)
          ctx.stroke()
          cross(ctx, X(v.iip.lon), Y(v.iip.lat), ctx.strokeStyle as string)
        }
      }
      // Veículos.
      ctx.font = '11px system-ui, sans-serif'
      for (const v of snap.vehicles) {
        if (v.central !== body) continue
        if (v.kind === 'debris' && !showDebris) continue
        if (v.attached_to) continue
        drawVehicle(ctx, v, X(v.lon), Y(v.lat), v.id === selected)
      }
    },
    [snap, ready, tracks, selected, showStations, showDebris, center, body],
  )
  const click = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (!snap) return
    const r = e.currentTarget.getBoundingClientRect()
    const lon = ((e.clientX - r.left) / r.width) * 360 - 180 + center
    const lat = 90 - ((e.clientY - r.top) / r.height) * 180
    let best: Vehicle | null = null
    let bd = 6
    for (const v of snap.vehicles) {
      if (v.central !== body || (v.kind === 'debris' && !showDebris)) continue
      const d = Math.hypot(wrapLon(v.lon - lon), v.lat - lat)
      if (d < bd) {
        bd = d
        best = v
      }
    }
    if (best) store.setUi({ selected: best.id })
  }
  return (
    <div className="canvas-wrap" style={{ height, aspectRatio: '2 / 1', maxHeight: height, width: '100%' }}>
      <canvas ref={ref} onClick={click} style={{ cursor: 'crosshair' }} role="img" aria-label={t('Mapa com os veículos, as estações e os traços previstos')} />
    </div>
  )
}

/** A camada fixa: fundo, grade e continentes. */
/** Cor do tema (variável CSS), ou a padrão. */
function cssVar(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return v || fallback
}

function drawBase(ctx: CanvasRenderingContext2D, w: number, h: number, X: (l: number) => number, Y: (l: number) => number, earth: boolean, center: number) {
  ctx.fillStyle = cssVar('--map-sea', '#081426')
  ctx.fillRect(0, 0, w, h)
  ctx.strokeStyle = cssVar('--map-grid', '#15233f')
  ctx.lineWidth = 1
  for (let lon = -180; lon <= 180; lon += 30) {
    const x = Math.round(X(lon)) + 0.5
    ctx.beginPath()
    ctx.moveTo(x, 0)
    ctx.lineTo(x, h)
    ctx.stroke()
  }
  for (let lat = -60; lat <= 60; lat += 30) {
    const y = Math.round(Y(lat)) + 0.5
    ctx.beginPath()
    ctx.moveTo(0, y)
    ctx.lineTo(w, y)
    ctx.stroke()
  }
  if (land && earth) {
    ctx.fillStyle = cssVar('--map-land', '#16294a')
    ctx.strokeStyle = cssVar('--map-coast', '#2d4a7a')
    for (const ring of land) {
      for (const part of splitDateline(ring.map(([lo, la]) => [wrapLon(lo - center) + center, la] as [number, number]))) {
        ctx.beginPath()
        part.forEach(([lo, la], i) => (i ? ctx.lineTo(X(lo), Y(la)) : ctx.moveTo(X(lo), Y(la))))
        ctx.closePath()
        ctx.fill()
        ctx.stroke()
      }
    }
  }
}

function drawLine(ctx: CanvasRenderingContext2D, pts: [number, number][], X: (l: number) => number, Y: (l: number) => number) {
  for (const part of splitDateline(pts.map(([lo, la]) => [wrapLon(lo), la] as [number, number]))) {
    ctx.beginPath()
    part.forEach(([lo, la], i) => (i ? ctx.lineTo(X(lo), Y(la)) : ctx.moveTo(X(lo), Y(la))))
    ctx.stroke()
  }
}

function cross(ctx: CanvasRenderingContext2D, x: number, y: number, color: string) {
  ctx.strokeStyle = color
  ctx.beginPath()
  ctx.moveTo(x - 5, y - 5)
  ctx.lineTo(x + 5, y + 5)
  ctx.moveTo(x + 5, y - 5)
  ctx.lineTo(x - 5, y + 5)
  ctx.stroke()
}

function drawVehicle(ctx: CanvasRenderingContext2D, v: Vehicle, x: number, y: number, sel: boolean) {
  const color = v.phase === 'destroyed' ? '#ff4d5e' : kindColor(v.kind)
  ctx.fillStyle = color
  ctx.strokeStyle = '#000'
  ctx.beginPath()
  if (v.kind === 'debris') {
    ctx.arc(x, y, 2.5, 0, Math.PI * 2)
    ctx.fill()
    return
  }
  ctx.moveTo(x, y - 6)
  ctx.lineTo(x + 5, y + 4)
  ctx.lineTo(x - 5, y + 4)
  ctx.closePath()
  ctx.fill()
  ctx.stroke()
  if (sel) {
    ctx.strokeStyle = '#fff'
    ctx.beginPath()
    ctx.arc(x, y, 10, 0, Math.PI * 2)
    ctx.stroke()
  }
  ctx.fillStyle = '#e6edf8'
  ctx.fillText(v.name, x + 9, y + 4)
}

/** Ponto a `d` km de (lat, lon) no rumo `brg`. */
export function destination(lat: number, lon: number, brg: number, d: number): [number, number] {
  const R = 6371
  const la = (lat * Math.PI) / 180
  const lo = (lon * Math.PI) / 180
  const b = (brg * Math.PI) / 180
  const a = d / R
  const la2 = Math.asin(Math.sin(la) * Math.cos(a) + Math.cos(la) * Math.sin(a) * Math.cos(b))
  const lo2 = lo + Math.atan2(Math.sin(b) * Math.sin(a) * Math.cos(la), Math.cos(a) - Math.sin(la) * Math.sin(la2))
  return [(la2 * 180) / Math.PI, wrapLon((lo2 * 180) / Math.PI)]
}
