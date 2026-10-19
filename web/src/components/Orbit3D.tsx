// Vista 3D da órbita: o corpo central, a trajetória prevista, os outros
// veículos e as queimas. Unidade: o raio do corpo central.

import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { kindColor } from './MapView'
import type { PredPoint } from '../lib/hooks'
import { useStore } from '../lib/store'

interface Props {
  vehicle: number | null
  prediction: PredPoint[]
  others?: { id: number; points: PredPoint[]; color: string }[]
  height?: number
}

const BODY_COLOR: Record<string, number> = {
  earth: 0x1b4f8a,
  moon: 0x8a8a8a,
  mars: 0xa0522d,
  venus: 0xc9a86b,
  mercury: 0x777777,
  jupiter: 0xc8a27a,
  saturn: 0xd9c38a,
  uranus: 0x7fd4d9,
  neptune: 0x3b6fd9,
  sun: 0xffcc33,
  // KSP
  kerbin: 0x2f6db5,
  mun: 0x8c8c8c,
  minmus: 0x9fd9c8,
  eve: 0x7b3fa0,
  gilly: 0x8a6a55,
  duna: 0xb0502d,
  ike: 0x6e6e6e,
  dres: 0x7a7468,
  jool: 0x5f9e3a,
  laythe: 0x2a5a8a,
  vall: 0x9fb8c8,
  tylo: 0xb8b0a0,
  bop: 0x6a5a48,
  pol: 0xc8b878,
  eeloo: 0xd8d8d0,
  moho: 0x6a4a3a,
}

let land: [number, number][][] | null = null

// Inercial (x, y, z com z ao norte) → three.js (y para cima).
function toThree(x: number, y: number, z: number, scale: number): THREE.Vector3 {
  return new THREE.Vector3(x / scale, z / scale, -y / scale)
}

export function Orbit3D({ vehicle, prediction, others = [], height = 420 }: Props) {
  const mount = useRef<HTMLDivElement>(null)
  const three = useRef<{
    renderer: THREE.WebGLRenderer
    scene: THREE.Scene
    camera: THREE.PerspectiveCamera
    controls: OrbitControls
    body: THREE.Group
    dyn: THREE.Group
    radius: number
    central: string
  } | null>(null)
  const snap = useStore((s) => s.snap)
  const hello = useStore((s) => s.hello)
  const [landReady, setLandReady] = useState(land != null)

  useEffect(() => {
    const el = mount.current!
    const renderer = new THREE.WebGLRenderer({ antialias: true })
    renderer.setPixelRatio(window.devicePixelRatio)
    el.appendChild(renderer.domElement)
    const scene = new THREE.Scene()
    scene.background = new THREE.Color(0x050914)
    const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1e7)
    camera.position.set(2.2, 1.4, 3.2)
    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = true
    scene.add(new THREE.AmbientLight(0x6070a0, 0.9))
    const sun = new THREE.DirectionalLight(0xffffff, 1.6)
    sun.name = 'sun'
    scene.add(sun)
    const body = new THREE.Group()
    const dyn = new THREE.Group()
    scene.add(body, dyn)
    // Estrelas ao fundo.
    const stars = new THREE.BufferGeometry()
    const sp: number[] = []
    for (let i = 0; i < 1500; i++) {
      const v = new THREE.Vector3().randomDirection().multiplyScalar(5e5)
      sp.push(v.x, v.y, v.z)
    }
    stars.setAttribute('position', new THREE.Float32BufferAttribute(sp, 3))
    scene.add(new THREE.Points(stars, new THREE.PointsMaterial({ color: 0x8899bb, size: 1.2, sizeAttenuation: false })))
    three.current = { renderer, scene, camera, controls, body, dyn, radius: 1, central: '' }
    const resize = () => {
      const w = el.clientWidth
      const h = el.clientHeight
      renderer.setSize(w, h)
      camera.aspect = w / h
      camera.updateProjectionMatrix()
    }
    resize()
    const ro = new ResizeObserver(resize)
    ro.observe(el)
    let raf = 0
    const loop = () => {
      controls.update()
      renderer.render(scene, camera)
      raf = requestAnimationFrame(loop)
    }
    loop()
    if (!land) {
      fetch('/land.json')
        .then((r) => r.json())
        .then((d) => {
          land = d
          if (three.current) three.current.central = ''
          setLandReady(true)
        })
        .catch(() => {})
    }
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      controls.dispose()
      renderer.dispose()
      el.removeChild(renderer.domElement)
    }
  }, [])

  // Corpo central.
  const v = snap?.vehicles.find((x) => x.id === vehicle)
  const central = v?.central ?? 'earth'
  useEffect(() => {
    const th = three.current
    if (!th || !hello) return
    // Naves do KSP trazem o raio do corpo (o "sun" do jogo não é o Sol de verdade).
    const radius = v?.central_radius ?? hello.bodies.find((b) => b.id === central)?.radius ?? 6.378e6
    if (th.central === central && th.radius === radius && th.body.children.length > 1) return
    th.body.clear()
    th.central = central
    th.radius = radius
    const sphere = new THREE.Mesh(new THREE.SphereGeometry(1, 64, 48), new THREE.MeshPhongMaterial({ color: BODY_COLOR[central] ?? 0x445566, shininess: 8 }))
    th.body.add(sphere)
    if (central === 'earth' && land) {
      const mat = new THREE.LineBasicMaterial({ color: 0x7fb0ff })
      for (const ring of land) {
        const pts = ring.map(([lo, la]) => {
          const a = (la * Math.PI) / 180
          const b = (lo * Math.PI) / 180
          return new THREE.Vector3(Math.cos(a) * Math.cos(b), Math.sin(a), -Math.cos(a) * Math.sin(b)).multiplyScalar(1.002)
        })
        th.body.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(pts), mat))
      }
    }
    // Equador e eixo.
    const eq: THREE.Vector3[] = []
    for (let i = 0; i <= 128; i++) {
      const a = (i / 128) * Math.PI * 2
      eq.push(new THREE.Vector3(Math.cos(a) * 1.003, 0, Math.sin(a) * 1.003))
    }
    th.body.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(eq), new THREE.LineBasicMaterial({ color: 0x35507a })))
    th.body.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(0, -1.4, 0), new THREE.Vector3(0, 1.4, 0)]), new THREE.LineBasicMaterial({ color: 0x35507a })))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [central, hello, landReady, v?.central_radius])

  // Rotação do corpo, Sol, trajetórias e veículos.
  useEffect(() => {
    const th = three.current
    if (!th || !snap) return
    const R = th.radius
    if (central === 'earth') th.body.rotation.y = snap.gmst
    const sun = th.scene.getObjectByName('sun') as THREE.DirectionalLight | undefined
    if (sun) sun.position.copy(toThree(snap.sun[0], snap.sun[1], snap.sun[2], 1).multiplyScalar(10))
    th.dyn.clear()
    const line = (pts: PredPoint[], color: number | string, opacity = 1) => {
      if (pts.length < 2) return
      const g = new THREE.BufferGeometry().setFromPoints(pts.map((p) => toThree(p[4], p[5], p[6], R)))
      th.dyn.add(new THREE.Line(g, new THREE.LineBasicMaterial({ color, transparent: opacity < 1, opacity })))
    }
    line(prediction, 0x4cc3ff)
    for (const o of others) line(o.points, o.color, 0.6)
    for (const x of snap.vehicles) {
      if (!x.r || x.central !== central || x.attached_to) continue
      if (x.kind === 'debris' && x.id !== vehicle) continue
      const p = toThree(x.r[0], x.r[1], x.r[2], R)
      const size = x.id === vehicle ? 0.035 : 0.022
      const m = new THREE.Mesh(new THREE.SphereGeometry(size, 12, 8), new THREE.MeshBasicMaterial({ color: x.id === vehicle ? 0xffffff : kindColor(x.kind) }))
      m.position.copy(p)
      th.dyn.add(m)
    }
    // Queimas planejadas: seta no ponto previsto.
    if (v?.nodes) {
      for (const n of v.nodes.filter((n) => !n.done)) {
        const p = prediction.reduce<PredPoint | null>((best, q) => (best == null || Math.abs(q[0] - n.t) < Math.abs(best[0] - n.t) ? q : best), null)
        if (!p) continue
        const m = new THREE.Mesh(new THREE.OctahedronGeometry(0.03), new THREE.MeshBasicMaterial({ color: 0xff8a3d }))
        m.position.copy(toThree(p[4], p[5], p[6], R))
        th.dyn.add(m)
      }
    }
  }, [snap, prediction, others, vehicle, central, v?.nodes])

  // Enquadra a órbita quando muda de veículo ou de corpo: olha de cima do plano,
  // um pouco inclinado, e afasta até caber.
  useEffect(() => {
    const th = three.current
    if (!th || prediction.length < 3) return
    let max = 1.5
    for (const p of prediction) max = Math.max(max, Math.hypot(p[4], p[5], p[6]) / th.radius)
    const d = Math.min(max * 2.8, 2e5)
    const a = toThree(prediction[0][4], prediction[0][5], prediction[0][6], 1)
    const b = toThree(prediction[Math.floor(prediction.length / 4)][4], prediction[Math.floor(prediction.length / 4)][5], prediction[Math.floor(prediction.length / 4)][6], 1)
    const normal = a.clone().cross(b).normalize()
    const side = a.clone().normalize()
    const dir = normal.multiplyScalar(0.75).add(side.multiplyScalar(0.66)).normalize()
    th.camera.position.copy(dir.multiplyScalar(d))
    th.controls.target.set(0, 0, 0)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [vehicle, central, prediction.length > 2])

  return <div ref={mount} style={{ width: '100%', height }} />
}
