import { useEffect, useRef, useState } from 'react'
import { useCanvas } from '../components/Chart'
import { CmdButton, Empty, Panel, Tile, sendCommand } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { distance, num } from '../lib/format'
import { store, useStore } from '../lib/store'
import type { Vehicle } from '../types'
import { Planner } from './OrbitView'

type V3 = [number, number, number]
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
const norm = (a: V3) => Math.hypot(a[0], a[1], a[2])
const unit = (a: V3): V3 => {
  const n = norm(a) || 1
  return [a[0] / n, a[1] / n, a[2] / n]
}

/** Posição e velocidade do perseguidor no LVLH do alvo (x radial, y ao longo, z normal). */
export function relative(chaser: Vehicle, target: Vehicle) {
  const rt = target.r!
  const vt = target.v!
  const x = unit(rt)
  const z = unit(cross(rt, vt))
  const y = cross(z, x)
  const dr = sub(chaser.r!, rt)
  const n = norm(cross(rt, vt)) / dot(rt, rt)
  const omega: V3 = [z[0] * n, z[1] * n, z[2] * n]
  const dv = sub(sub(chaser.v!, vt), cross(omega, dr))
  return { pos: [dot(dr, x), dot(dr, y), dot(dr, z)] as V3, vel: [dot(dv, x), dot(dv, y), dot(dv, z)] as V3 }
}

export function Rendezvous() {
  const v = useStore((s) => s.selected)
  const snap = useStore((s) => s.snap)!
  const candidates = snap.vehicles.filter((x) => v && x.id !== v.id && x.kind !== 'debris' && x.central === v.central && x.r && x.phase !== 'prelaunch')
  const [targetId, setTargetId] = useState<number | null>(null)
  const target = candidates.find((x) => x.id === targetId) ?? candidates.find((x) => x.kind === 'station') ?? candidates[0]
  const trail = useRef<{ key: string; pts: V3[] }>({ key: '', pts: [] })
  const rel = v && target && v.r && target.r && !v.attached_to ? relative(v, target) : null
  const key = `${v?.id}-${target?.id}`
  useEffect(() => {
    if (!rel) return
    if (trail.current.key !== key) trail.current = { key, pts: [] }
    const pts = trail.current.pts
    const last = pts[pts.length - 1]
    if (!last || norm(sub(rel.pos, last)) > Math.max(0.5, norm(rel.pos) * 0.005)) pts.push(rel.pos)
    if (pts.length > 2000) pts.splice(0, pts.length - 2000)
  })
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  const range = rel ? norm(rel.pos) : NaN
  const rate = rel ? dot(rel.pos, rel.vel) / (range || 1) : NaN
  return (
    <div className="grid">
      <div className="row">
        <span className="muted">{t('Alvo')}:</span>
        <select value={target?.id ?? ''} onChange={(e) => setTargetId(Number(e.target.value))}>
          {candidates.map((x) => (
            <option key={x.id} value={x.id}>
              {x.name}
            </option>
          ))}
        </select>
        {v.attached_to != null && <span className="pill go">{t('Acoplado a')} {store.vehicle(v.attached_to)?.name}</span>}
      </div>
      {!target ? (
        <Empty>{t('Não há outro veículo em torno do mesmo corpo.')}</Empty>
      ) : (
        <>
          <div className="grid cols-3">
            <Panel title={t('Estado relativo (LVLH do alvo)')}>
              <div className="tiles">
                <Tile label={t('Distância')} value={distance(range / 1000)} />
                <Tile label={t('Velocidade de aproximação')} value={num(-rate, 2)} unit="m/s" />
                <Tile label={t('R-bar (radial)')} value={num(rel?.pos[0], 1)} unit="m" />
                <Tile label={t('V-bar (ao longo)')} value={num(rel?.pos[1], 1)} unit="m" />
                <Tile label={t('H-bar (normal)')} value={num(rel?.pos[2], 1)} unit="m" />
                <Tile label={t('Vel. relativa')} value={num(rel ? norm(rel.vel) : NaN, 2)} unit="m/s" />
              </div>
              <div className="row" style={{ marginTop: 8 }}>
                <CmdButton vehicle={v.id} cmd="dock" args={{ target: target.id }} className="primary" disabled={range > 8000 || v.attached_to != null}>
                  {t('Aproximar e acoplar')}
                </CmdButton>
                <CmdButton vehicle={v.id} cmd="undock" disabled={v.attached_to == null}>
                  {t('Desacoplar')}
                </CmdButton>
                <CmdButton vehicle={v.id} cmd="att_mode" args={{ mode: 'target', target: target.id }}>
                  {t('Apontar para o alvo')}
                </CmdButton>
              </div>
              {v.guidance?.program === 'docking' && <div className="small warn" style={{ marginTop: 6 }}>{t('Aproximação automática em andamento')}</div>}
            </Panel>
            <Panel title={t('Aproximação: V-bar × R-bar')} flush>
              <RelPlot pts={trail.current.pts} cur={rel?.pos} />
            </Panel>
            <Panel title={`${t('Portas de acoplamento')} · ${target.name}`}>
              {(target.ports ?? []).length === 0 ? (
                <span className="muted">{t('O alvo não tem portas.')}</span>
              ) : (
                <table>
                  <tbody>
                    {target.ports!.map((p, i) => (
                      <tr key={i}>
                        <td>{tx(p.name)}</td>
                        <td>{p.occupant != null ? store.vehicle(p.occupant)?.name : <span className="muted">{t('livre')}</span>}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              {v.attached_to != null && v.crew > 0 && (
                <div className="row" style={{ marginTop: 8 }}>
                  <button onClick={() => sendCommand(v.id, 'crew_transfer', { count: v.crew })}>{t('Transferir a tripulação')}</button>
                </div>
              )}
              {v.attached_to == null && target.crew > 0 && v.crew === 0 && <div className="small muted" style={{ marginTop: 6 }}>{t('A tripulação passa de um para o outro com os dois acoplados.')}</div>}
            </Panel>
          </div>
          <div className="grid cols-2">
            <Planner v={v} initial="rendezvous" />
            <Panel title={t('Como fazer')}>
              <ol className="small" style={{ margin: 0, paddingLeft: 18 }}>
                <li>{t('Planeje "Igualar plano a um alvo" se a diferença de inclinação passar de 0,1°.')}</li>
                <li>{t('Planeje "Encontro com um alvo": duas queimas levam a 1 km atrás dele.')}</li>
                <li>{t('Acelere o tempo: as queimas executam sozinhas.')}</li>
                <li>{t('A menos de 8 km, "Aproximar e acoplar" faz a aproximação pela V-bar até a captura.')}</li>
              </ol>
            </Panel>
          </div>
        </>
      )}
    </div>
  )
}

function RelPlot({ pts, cur }: { pts: V3[]; cur?: V3 }) {
  const ref = useCanvas(
    (ctx, w, h) => {
      ctx.fillStyle = '#081426'
      ctx.fillRect(0, 0, w, h)
      let span = 50
      for (const p of pts.slice(-600)) span = Math.max(span, Math.abs(p[0]) * 2.2, Math.abs(p[1]) * 1.1)
      if (cur) span = Math.max(span, Math.abs(cur[0]) * 2.2, Math.abs(cur[1]) * 1.1)
      const s = Math.min(w, h * 1.6) / (2 * span)
      // V-bar na horizontal (positivo à esquerda: o alvo voa para a esquerda), R-bar para cima.
      const X = (y: number) => w / 2 - y * s
      const Y = (x: number) => h / 2 - x * s
      ctx.strokeStyle = '#22304d'
      ctx.beginPath()
      ctx.moveTo(0, h / 2)
      ctx.lineTo(w, h / 2)
      ctx.moveTo(w / 2, 0)
      ctx.lineTo(w / 2, h)
      ctx.stroke()
      ctx.fillStyle = '#8494b3'
      ctx.font = '11px ui-monospace, monospace'
      ctx.fillText('V-bar →', 6, h / 2 - 4)
      ctx.fillText('R-bar ↑', w / 2 + 4, 12)
      ctx.fillText(`±${span >= 1000 ? (span / 1000).toFixed(1) + ' km' : span.toFixed(0) + ' m'}`, w - 80, h - 6)
      ctx.strokeStyle = '#4cc3ff'
      ctx.beginPath()
      pts.forEach((p, i) => (i ? ctx.lineTo(X(p[1]), Y(p[0])) : ctx.moveTo(X(p[1]), Y(p[0]))))
      ctx.stroke()
      ctx.fillStyle = '#c084fc'
      ctx.fillRect(w / 2 - 4, h / 2 - 4, 8, 8)
      if (cur) {
        ctx.fillStyle = '#ffc23d'
        ctx.beginPath()
        ctx.arc(X(cur[1]), Y(cur[0]), 4, 0, Math.PI * 2)
        ctx.fill()
      }
    },
    [pts.length, cur?.[0], cur?.[1]],
  )
  return (
    <div className="canvas-wrap" style={{ height: 260 }}>
      <canvas ref={ref} />
    </div>
  )
}
