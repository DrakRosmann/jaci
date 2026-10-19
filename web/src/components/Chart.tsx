// Gráfico de linhas em canvas: séries no tempo, escala automática.

import { useEffect, useRef } from 'react'

export interface Series {
  label: string
  color: string
  t: number[]
  v: number[]
}

/** Canvas que se redesenha quando muda de tamanho ou quando `deps` mudam. */
export function useCanvas(draw: (ctx: CanvasRenderingContext2D, w: number, h: number) => void, deps: unknown[]) {
  const ref = useRef<HTMLCanvasElement>(null)
  const drawRef = useRef(draw)
  drawRef.current = draw
  useEffect(() => {
    const c = ref.current
    if (!c) return
    const render = () => {
      const r = c.getBoundingClientRect()
      const dpr = window.devicePixelRatio || 1
      const w = Math.max(1, Math.round(r.width * dpr))
      const h = Math.max(1, Math.round(r.height * dpr))
      if (c.width !== w || c.height !== h) {
        c.width = w
        c.height = h
      }
      const ctx = c.getContext('2d')
      if (!ctx) return
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      drawRef.current(ctx, r.width, r.height)
    }
    render()
    const ro = new ResizeObserver(render)
    ro.observe(c)
    return () => ro.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)
  return ref
}

function niceStep(range: number, ticks: number): number {
  const raw = range / Math.max(1, ticks)
  const mag = 10 ** Math.floor(Math.log10(raw))
  const n = raw / mag
  return (n < 1.5 ? 1 : n < 3 ? 2 : n < 7 ? 5 : 10) * mag
}

export function Chart({ series, height = 180, t0, unit, yMin, yMax }: { series: Series[]; height?: number; t0?: number; unit?: string; yMin?: number; yMax?: number }) {
  const ref = useCanvas(
    (ctx, w, h) => {
      ctx.clearRect(0, 0, w, h)
      const pad = { l: 52, r: 10, t: 8, b: 22 }
      let tmin = Infinity
      let tmax = -Infinity
      let vmin = Infinity
      let vmax = -Infinity
      for (const s of series) {
        for (let i = 0; i < s.t.length; i++) {
          const v = s.v[i]
          if (!isFinite(v)) continue
          tmin = Math.min(tmin, s.t[i])
          tmax = Math.max(tmax, s.t[i])
          vmin = Math.min(vmin, v)
          vmax = Math.max(vmax, v)
        }
      }
      ctx.font = '11px ui-monospace, monospace'
      if (!isFinite(tmin)) {
        ctx.fillStyle = '#8494b3'
        ctx.fillText('sem dados', w / 2 - 25, h / 2)
        return
      }
      if (yMin != null) vmin = Math.min(vmin, yMin)
      if (yMax != null) vmax = Math.max(vmax, yMax)
      if (vmax - vmin < 1e-9) {
        vmax += 1
        vmin -= 1
      }
      const margin = (vmax - vmin) * 0.06
      vmin -= margin
      vmax += margin
      if (tmax - tmin < 1e-6) tmax = tmin + 1
      const X = (t: number) => pad.l + ((t - tmin) / (tmax - tmin)) * (w - pad.l - pad.r)
      const Y = (v: number) => pad.t + (1 - (v - vmin) / (vmax - vmin)) * (h - pad.t - pad.b)
      // Grade.
      ctx.strokeStyle = '#1a2540'
      ctx.fillStyle = '#8494b3'
      ctx.lineWidth = 1
      const ys = niceStep(vmax - vmin, 5)
      for (let v = Math.ceil(vmin / ys) * ys; v <= vmax; v += ys) {
        const y = Math.round(Y(v)) + 0.5
        ctx.beginPath()
        ctx.moveTo(pad.l, y)
        ctx.lineTo(w - pad.r, y)
        ctx.stroke()
        const label = Math.abs(v) >= 1e4 ? v.toExponential(1) : Number(v.toPrecision(4)).toString()
        ctx.fillText(label, 4, y + 4)
      }
      const base = t0 ?? tmin
      const xs = niceStep(tmax - tmin, 6)
      for (let tt = Math.ceil((tmin - base) / xs) * xs + base; tt <= tmax; tt += xs) {
        const x = Math.round(X(tt)) + 0.5
        ctx.beginPath()
        ctx.moveTo(x, pad.t)
        ctx.lineTo(x, h - pad.b)
        ctx.stroke()
        // A unidade segue o intervalo todo, não cada marca: rótulos do mesmo tamanho.
        const rel = tt - base
        const span = tmax - tmin
        const lbl =
          span >= 4 * 3600
            ? `${(rel / 3600).toFixed(xs < 3600 ? 1 : 0)} h`
            : span >= 600
              ? `${(rel / 60).toFixed(xs < 60 ? 1 : 0)} min`
              : `${rel.toFixed(xs < 1 ? 1 : 0)} s`
        ctx.fillText(lbl, x - ctx.measureText(lbl).width / 2, h - 6)
      }
      if (unit) ctx.fillText(unit, 4, pad.t + 10)
      // Séries.
      for (const s of series) {
        ctx.strokeStyle = s.color
        ctx.lineWidth = 1.6
        ctx.beginPath()
        let pen = false
        for (let i = 0; i < s.t.length; i++) {
          const v = s.v[i]
          if (!isFinite(v)) {
            pen = false
            continue
          }
          const x = X(s.t[i])
          const y = Y(v)
          if (pen) ctx.lineTo(x, y)
          else ctx.moveTo(x, y)
          pen = true
        }
        ctx.stroke()
      }
      // Legenda.
      let lx = pad.l + 6
      for (const s of series) {
        ctx.fillStyle = s.color
        ctx.fillRect(lx, pad.t + 4, 10, 3)
        ctx.fillStyle = '#d9e2f2'
        ctx.fillText(s.label, lx + 14, pad.t + 9)
        lx += ctx.measureText(s.label).width + 30
      }
    },
    [series, t0, unit, yMin, yMax],
  )
  return (
    <div className="canvas-wrap" style={{ height }}>
      <canvas ref={ref} />
    </div>
  )
}

export const COLORS = ['#4cc3ff', '#ff8a3d', '#3ddc84', '#ffc23d', '#c084fc', '#ff4d5e', '#9fb4dd', '#2dd4bf']
