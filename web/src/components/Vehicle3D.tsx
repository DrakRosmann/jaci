// A nave em 3D, montada dos estágios do instantâneo: cilindros com o
// diâmetro e o comprimento de cada estágio, boosters ao lado, coifa, cápsula
// com torre de escape, painéis solares, motores acesos com pluma. A atitude é
// mostrada no referencial local (para cima no alto da tela, a Terra embaixo).

import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { cross, localFrame, unit, type Q, type V3 } from '../lib/attitude'
import { t } from '../lib/i18n'
import type { Vehicle } from '../types'

/** Uma assinatura do que muda a forma (para não remontar a cada instantâneo). */
function shapeKey(v: Vehicle): string {
  const st = (v.stages ?? []).map((s) => `${s.name}:${s.diameter}:${s.length}:${s.boosters}:${s.spacecraft}:${s.engines?.length ?? 0}`).join('|')
  return `${v.id}|${v.kind}|${v.shape}|${st}|${(v.items ?? []).map((i) => i.id).join(',')}|${v.les?.tower}|${v.systems?.solar?.deployed}|${(v.chutes ?? []).map((c) => c.state).join(',')}|${v.gear_down}`
}

const MAT = {
  body: new THREE.MeshStandardMaterial({ color: 0xdfe5ee, metalness: 0.3, roughness: 0.55 }),
  dark: new THREE.MeshStandardMaterial({ color: 0x2b313b, metalness: 0.5, roughness: 0.6 }),
  band: new THREE.MeshStandardMaterial({ color: 0x1c2b44, metalness: 0.4, roughness: 0.5 }),
  tank: new THREE.MeshStandardMaterial({ color: 0xc8742f, metalness: 0.2, roughness: 0.7 }),
  gold: new THREE.MeshStandardMaterial({ color: 0xc9a24a, metalness: 0.8, roughness: 0.35 }),
  solar: new THREE.MeshStandardMaterial({ color: 0x1d3f8f, metalness: 0.6, roughness: 0.3, side: THREE.DoubleSide }),
  nozzle: new THREE.MeshStandardMaterial({ color: 0x55585e, metalness: 0.9, roughness: 0.35, side: THREE.DoubleSide }),
  chute: new THREE.MeshStandardMaterial({ color: 0xff8a3d, roughness: 0.9, side: THREE.DoubleSide }),
  plume: new THREE.MeshBasicMaterial({ color: 0xffb35c, transparent: true, opacity: 0.55, depthWrite: false }),
}

/** Monta a nave com o eixo da frente em +X (como no núcleo); devolve o grupo e as plumas. */
function build(v: Vehicle): { group: THREE.Group; plumes: { mesh: THREE.Mesh; stage: number; engine: number }[]; length: number } {
  const g = new THREE.Group()
  const plumes: { mesh: THREE.Mesh; stage: number; engine: number }[] = []
  const stages = v.stages ?? []
  let x = 0
  // Cilindro ao longo do X.
  const cyl = (r1: number, r2: number, len: number, mat: THREE.Material, at: number, y = 0, z = 0) => {
    const m = new THREE.Mesh(new THREE.CylinderGeometry(r2, r1, len, 28), mat)
    m.rotation.z = -Math.PI / 2
    m.position.set(at + len / 2, y, z)
    g.add(m)
    return m
  }
  const nozzles = (si: number, n: number, r: number, at: number, ring: number, y0 = 0, z0 = 0) => {
    for (let k = 0; k < n; k++) {
      const a = (k / n) * Math.PI * 2
      const rr = n > 1 ? ring : 0
      const y = y0 + Math.cos(a) * rr
      const z = z0 + Math.sin(a) * rr
      const nz = cyl(r, r * 0.35, r * 2.2, MAT.nozzle, at - r * 2.2, y, z)
      nz.scale.set(1, 1, 1)
      const pl = new THREE.Mesh(new THREE.ConeGeometry(r * 0.9, r * 9, 16, 1, true), MAT.plume.clone())
      pl.rotation.z = Math.PI / 2
      pl.position.set(at - r * 2.2 - r * 4.5, y, z)
      pl.visible = false
      g.add(pl)
      plumes.push({ mesh: pl, stage: si, engine: k })
    }
  }
  let coreR = 0.5
  stages.forEach((s, i) => {
    const d = s.diameter ?? 1
    const len = s.length ?? d * 3
    const r = d / 2
    const engines = s.engines?.length ?? 0
    if (s.boosters) {
      // Boosters em volta do estágio seguinte, embaixo.
      const next = stages[i + 1]
      const cr = (next?.diameter ?? d) / 2
      const n = Math.max(2, Math.min(engines || 2, 6))
      const br = Math.min(r, cr * 0.75)
      for (let k = 0; k < n; k++) {
        const a = (k / n) * Math.PI * 2
        const y = Math.cos(a) * (cr + br)
        const z = Math.sin(a) * (cr + br)
        cyl(br, br, len * 0.9, i === 0 ? MAT.body : MAT.body, x, y, z)
        const cone = new THREE.Mesh(new THREE.ConeGeometry(br, br * 2.5, 20), MAT.body)
        cone.rotation.z = -Math.PI / 2
        cone.position.set(x + len * 0.9 + br * 1.25, y, z)
        g.add(cone)
        nozzles(i, 1, br * 0.6, x, 0, y, z)
      }
      return
    }
    if (s.spacecraft) return
    const tankMat = s.cryogenic && i === 0 && v.kind === 'shuttle' ? MAT.tank : MAT.body
    cyl(r, r, len, tankMat, x)
    cyl(r * 1.01, r * 1.01, Math.min(0.25, len * 0.05), MAT.band, x + len - Math.min(0.25, len * 0.05))
    if (engines > 0) {
      const n = Math.min(engines, 9)
      const er = n > 1 ? Math.min(r * 0.32, (r * 2.4) / n) : r * 0.45
      nozzles(i, n, er, x, n > 1 ? r * 0.62 : 0)
    }
    coreR = r
    x += len
  })
  // A nave (estágios "spacecraft") e a ponta.
  const craft = stages.filter((s) => s.spacecraft)
  const fairing = (v.items ?? []).some((i) => i.id === 'fairing')
  if (v.kind === 'capsule' || v.shape === 'capsule') {
    const trunk = craft.find((s) => (s.engines?.length ?? 0) === 0)
    const d = craft[craft.length - 1]?.diameter ?? coreR * 2
    if (trunk) {
      cyl(d / 2, d / 2, trunk.length ?? d, MAT.dark, x)
      x += trunk.length ?? d
    }
    // Escudo e cápsula cônica.
    cyl(d / 2, d / 2, d * 0.08, MAT.gold, x)
    x += d * 0.08
    cyl(d / 2, d * 0.22, d * 0.75, MAT.body, x)
    x += d * 0.75
    if (v.les?.tower) {
      cyl(d * 0.06, d * 0.06, d * 1.4, MAT.dark, x)
      cyl(d * 0.12, d * 0.02, d * 0.5, MAT.body, x + d * 1.4)
    }
    // Paraquedas abertos.
    for (const [k, c] of (v.chutes ?? []).entries()) {
      if (c.state !== 'deployed' && c.state !== 'reefed') continue
      const cr = d * (c.inflation > 0 ? 0.6 + c.inflation * 1.6 : 0.6)
      const dome = new THREE.Mesh(new THREE.SphereGeometry(cr, 24, 12, 0, Math.PI * 2, 0, Math.PI / 2.4), MAT.chute)
      dome.rotation.z = -Math.PI / 2
      dome.position.set(x + d * 3 + k * 0.4, (k - 1) * cr * 0.9, 0)
      g.add(dome)
    }
  } else if (v.kind === 'station') {
    // Treliça, módulos e painéis grandes.
    const len = 30
    const truss = new THREE.Mesh(new THREE.BoxGeometry(1.2, 1.2, 100), MAT.dark)
    truss.position.set(x + len / 2, 3, 0)
    g.add(truss)
    for (let k = 0; k < 5; k++) cyl(2.1, 2.1, 6, MAT.body, x + k * 6.2, 0, 0)
    for (const zs of [-1, 1]) {
      for (const xs of [-1, 1]) {
        const p = new THREE.Mesh(new THREE.BoxGeometry(12, 0.1, 30), MAT.solar)
        p.position.set(x + len / 2 + xs * 7, 3, zs * 35)
        g.add(p)
      }
    }
    x += len
  } else {
    // Satélite, sonda: caixa dourada e painéis.
    const sat = craft[craft.length - 1]
    const d = sat?.diameter ?? coreR * 2
    const len = Math.max(1, (sat?.length ?? d) * 0.8)
    if (fairing) {
      cyl(coreR, coreR, len * 1.2, MAT.body, x)
      const nose = new THREE.Mesh(new THREE.ConeGeometry(coreR, coreR * 2.4, 28), MAT.body)
      nose.rotation.z = -Math.PI / 2
      nose.position.set(x + len * 1.2 + coreR * 1.2, 0, 0)
      g.add(nose)
      x += len * 1.2 + coreR * 2.4
    } else {
      const box = new THREE.Mesh(new THREE.BoxGeometry(len, d * 0.8, d * 0.8), MAT.gold)
      box.position.set(x + len / 2, 0, 0)
      g.add(box)
      if (v.systems?.solar) {
        const deployed = v.systems.solar.deployed
        for (const side of [-1, 1]) {
          const w = deployed ? d * 2.2 : d * 0.75
          const p = new THREE.Mesh(new THREE.BoxGeometry(len * 0.9, 0.04, w), MAT.solar)
          p.position.set(x + len / 2, 0, side * (d * 0.4 + w / 2 + 0.05))
          if (!deployed) p.rotation.x = Math.PI / 2
          g.add(p)
        }
      }
      x += len
    }
  }
  // Centraliza no meio da pilha.
  g.children.forEach((c) => (c.position.x -= x / 2))
  return { group: g, plumes, length: Math.max(x, 2) }
}

/** Plumas dos motores acesos, do tamanho do empuxo. */
function updatePlumes(plumes: { mesh: THREE.Mesh; stage: number; engine: number }[], v: Vehicle) {
  for (const p of plumes) {
    const e = v.stages?.[p.stage]?.engines?.[p.engine] ?? v.stages?.[p.stage]?.engines?.[0]
    const on = !!e && (e.state === 'running' || e.state === 'starting') && e.thrust > 0
    p.mesh.visible = on
    if (on && e) p.mesh.scale.x = p.mesh.scale.z = 0.6 + 0.4 * Math.min(1, e.thrust / Math.max(1, e.rated))
  }
}

export function Vehicle3D({ v, height }: { v: Vehicle; height: number }) {
  const mount = useRef<HTMLDivElement>(null)
  const three = useRef<{ renderer: THREE.WebGLRenderer; scene: THREE.Scene; camera: THREE.PerspectiveCamera; controls: OrbitControls; holder: THREE.Group; ground: THREE.Mesh; key: string; plumes: { mesh: THREE.Mesh; stage: number; engine: number }[]; length: number } | null>(null)
  // Sem WebGL (driver, máquina remota): só o aviso.
  const [noGl] = useState(() => {
    try {
      return !document.createElement('canvas').getContext('webgl2') && !document.createElement('canvas').getContext('webgl')
    } catch {
      return true
    }
  })

  useEffect(() => {
    const el = mount.current!
    if (noGl) return
    let renderer: THREE.WebGLRenderer
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true })
    } catch {
      return
    }
    renderer.setPixelRatio(window.devicePixelRatio)
    el.appendChild(renderer.domElement)
    const scene = new THREE.Scene()
    scene.background = new THREE.Color(0x03060d)
    const camera = new THREE.PerspectiveCamera(40, 1, 0.1, 5000)
    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = true
    scene.add(new THREE.AmbientLight(0x8090b0, 0.7))
    const sun = new THREE.DirectionalLight(0xffffff, 2.2)
    sun.position.set(30, 50, 20)
    scene.add(sun)
    // A Terra embaixo, como referência do "para baixo".
    const ground = new THREE.Mesh(new THREE.SphereGeometry(400, 64, 32), new THREE.MeshStandardMaterial({ color: 0x1b4f8a, roughness: 0.9 }))
    ground.position.set(0, -430, 0)
    scene.add(ground)
    const holder = new THREE.Group()
    scene.add(holder)
    three.current = { renderer, scene, camera, controls, holder, ground, key: '', plumes: [], length: 10 }
    const resize = () => {
      renderer.setSize(el.clientWidth, el.clientHeight)
      camera.aspect = el.clientWidth / Math.max(1, el.clientHeight)
      camera.updateProjectionMatrix()
    }
    resize()
    const ro = new ResizeObserver(resize)
    ro.observe(el)
    let raf = 0
    const loop = () => {
      controls.update()
      // Pluma tremulando.
      for (const p of three.current?.plumes ?? []) if (p.mesh.visible) p.mesh.scale.y = 0.85 + Math.random() * 0.3
      renderer.render(scene, camera)
      raf = requestAnimationFrame(loop)
    }
    loop()
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      controls.dispose()
      scene.traverse((o) => {
        const m = o as THREE.Mesh
        m.geometry?.dispose()
      })
      renderer.dispose()
      el.removeChild(renderer.domElement)
    }
  }, [noGl])

  // Remonta só quando a forma muda; a cada instantâneo, atitude e motores.
  useEffect(() => {
    const th = three.current
    if (!th) return
    const key = shapeKey(v)
    if (key !== th.key) {
      th.holder.children.forEach((c) => c.traverse((o) => (o as THREE.Mesh).geometry?.dispose()))
      th.holder.clear()
      const { group, plumes, length } = build(v)
      th.holder.add(group)
      th.plumes = plumes
      th.key = key
      if (th.length !== length || th.camera.position.length() < 1) {
        th.length = length
        th.camera.position.set(length * 1.1, length * 0.5, length * 1.3)
        th.controls.target.set(0, 0, 0)
      }
    }
    updatePlumes(th.plumes, v)
    // Atitude no referencial local: cena Y = para cima, X = leste, Z = sul (Terra embaixo).
    if (v.q && v.r) {
      const { up, east, north } = localFrame(v.r as V3)
      // Base local → cena: leste → +X, cima → +Y, norte → −Z.
      const sceneFromInertial = (d: V3): V3 => [d[0] * east[0] + d[1] * east[1] + d[2] * east[2], d[0] * up[0] + d[1] * up[1] + d[2] * up[2], -(d[0] * north[0] + d[1] * north[1] + d[2] * north[2])]
      const q = v.q as Q
      const rot = (b: V3): V3 => {
        const [w, x, y, z] = q
        const u: V3 = [x, y, z]
        const tt = cross(u, b).map((c) => c * 2) as V3
        const c2 = cross(u, tt)
        return [b[0] + tt[0] * w + c2[0], b[1] + tt[1] * w + c2[1], b[2] + tt[2] * w + c2[2]]
      }
      const ax = unit(sceneFromInertial(rot([1, 0, 0])))
      const ay = unit(sceneFromInertial(rot([0, 1, 0])))
      const az = unit(sceneFromInertial(rot([0, 0, 1])))
      const m = new THREE.Matrix4().makeBasis(new THREE.Vector3(...ax), new THREE.Vector3(...ay), new THREE.Vector3(...az))
      th.holder.quaternion.setFromRotationMatrix(m)
    }
    th.ground.visible = v.central === 'earth' || v.central === 'moon' || v.central === 'mars'
  }, [v])

  return (
    <div ref={mount} style={{ width: '100%', height }} role="img" aria-label={t('A nave em 3D')}>
      {noGl && <div className="loading muted" style={{ height }}>{t('Sem WebGL neste navegador: a vista 3D não está disponível.')}</div>}
    </div>
  )
}
