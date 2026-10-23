// Vista 3D da órbita: o corpo central, a trajetória prevista, os outros
// veículos e as queimas. Unidade: o raio do corpo central.

import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { kindColor } from './MapView'
import type { PredPoint } from '../lib/hooks'
import { t } from '../lib/i18n'
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

// Peças repetidas a cada instantâneo: uma geometria e um material por cor,
// reaproveitados, para não criar (e vazar) memória de vídeo cinco vezes por segundo.
const MARKER = new THREE.SphereGeometry(1, 12, 8)
const NODE = new THREE.OctahedronGeometry(0.03)
const basic = new Map<string, THREE.MeshBasicMaterial>()
function basicMaterial(color: number | string): THREE.MeshBasicMaterial {
  const key = String(color)
  let m = basic.get(key)
  if (!m) {
    m = new THREE.MeshBasicMaterial({ color })
    basic.set(key, m)
  }
  return m
}

/** Textura equirretangular do corpo, desenhada na hora (sem baixar imagens). */
const textures = new Map<string, THREE.CanvasTexture>()
function bodyTexture(central: string): THREE.CanvasTexture | null {
  const key = `${central}:${land ? 1 : 0}`
  const hit = textures.get(key)
  if (hit) return hit
  const W = 2048
  const H = 1024
  const c = document.createElement('canvas')
  c.width = W
  c.height = H
  const g = c.getContext('2d')
  if (!g) return null
  const X = (lon: number) => ((lon + 180) / 360) * W
  const Y = (lat: number) => ((90 - lat) / 180) * H
  let seed = 7
  const rnd = () => ((seed = (seed * 16807) % 2147483647) / 2147483647)
  if (central === 'earth') {
    const sea = g.createLinearGradient(0, 0, 0, H)
    sea.addColorStop(0, '#0b2f5c')
    sea.addColorStop(0.5, '#0e4a8a')
    sea.addColorStop(1, '#0b2f5c')
    g.fillStyle = sea
    g.fillRect(0, 0, W, H)
    if (!land) return null
    for (const ring of land) {
      const lat0 = ring.reduce((m, p) => m + p[1], 0) / ring.length
      g.fillStyle = lat0 < -60 ? '#dfe8f0' : lat0 > 60 ? '#5d7a52' : Math.abs(lat0) < 30 ? '#4a6b2e' : '#56703a'
      g.strokeStyle = '#9fb88a'
      g.lineWidth = 1
      g.beginPath()
      ring.forEach(([lo, la], i) => (i ? g.lineTo(X(lo), Y(la)) : g.moveTo(X(lo), Y(la))))
      g.closePath()
      g.fill()
      g.stroke()
    }
    // Calotas de gelo e um pouco de deserto nos subtrópicos.
    g.fillStyle = '#e8eef5'
    g.fillRect(0, 0, W, Y(80))
    g.fillRect(0, Y(-72), W, H - Y(-72))
  } else {
    const base: Record<string, [string, string]> = { moon: ['#8a8a8a', '#5c5c5c'], mars: ['#b5532d', '#7a3218'], mercury: ['#7d756d', '#55504a'], venus: ['#d9b77a', '#b08d4f'] }
    const [a, b] = base[central] ?? ['#667788', '#445566']
    g.fillStyle = a
    g.fillRect(0, 0, W, H)
    // Manchas e crateras procedurais.
    for (let i = 0; i < (central === 'moon' || central === 'mercury' ? 900 : 260); i++) {
      const r = (central === 'mars' ? 40 : 6) + rnd() ** 3 * (central === 'mars' ? 220 : 60)
      g.fillStyle = b
      g.globalAlpha = 0.15 + rnd() * 0.35
      g.beginPath()
      g.ellipse(rnd() * W, rnd() * H, r, r * (0.6 + rnd() * 0.4), 0, 0, Math.PI * 2)
      g.fill()
    }
    g.globalAlpha = 1
    if (central === 'mars') {
      g.fillStyle = '#f2ece4'
      g.fillRect(0, 0, W, Y(83))
      g.fillRect(0, Y(-80), W, H - Y(-80))
    }
  }
  const tex = new THREE.CanvasTexture(c)
  tex.colorSpace = THREE.SRGBColorSpace
  tex.anisotropy = 4
  textures.set(key, tex)
  return tex
}

/** Etiqueta com o nome (sprite que não muda de tamanho com a distância). */
const labels = new Map<string, THREE.SpriteMaterial>()
function label(text: string, color: string): THREE.Sprite {
  const key = `${text}|${color}`
  let mat = labels.get(key)
  if (!mat) {
    const c = document.createElement('canvas')
    const g = c.getContext('2d')!
    g.font = '28px system-ui, sans-serif'
    c.width = Math.ceil(g.measureText(text).width) + 16
    c.height = 40
    g.font = '28px system-ui, sans-serif'
    g.fillStyle = color
    g.fillText(text, 8, 29)
    const tex = new THREE.CanvasTexture(c)
    mat = new THREE.SpriteMaterial({ map: tex, sizeAttenuation: false, depthTest: false, transparent: true })
    labels.set(key, mat)
  }
  const s = new THREE.Sprite(mat)
  const img = mat.map!.image as HTMLCanvasElement
  s.scale.set((0.045 * img.width) / img.height, 0.045, 1)
  s.center.set(-0.08, 0.5)
  s.renderOrder = 10
  return s
}

/** Esvazia um grupo liberando a memória de vídeo do que não é compartilhado. */
function clearGroup(g: THREE.Group) {
  for (const o of g.children) {
    const m = o as THREE.Mesh
    if (m.geometry && m.geometry !== MARKER && m.geometry !== NODE) m.geometry.dispose()
    const mats = Array.isArray(m.material) ? m.material : m.material ? [m.material] : []
    for (const mat of mats) if (![...basic.values()].includes(mat as THREE.MeshBasicMaterial) && !(mat instanceof THREE.SpriteMaterial) && !((mat as THREE.MeshPhongMaterial).map && [...textures.values()].includes((mat as THREE.MeshPhongMaterial).map as THREE.CanvasTexture))) mat.dispose()
    if (o.children.length) clearGroup(o as THREE.Group)
  }
  g.clear()
}

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
    paths: THREE.Group
    dyn: THREE.Group
    radius: number
    central: string
  } | null>(null)
  const snap = useStore((s) => s.snap)
  const hello = useStore((s) => s.hello)
  const [landReady, setLandReady] = useState(land != null)
  const [noGl, setNoGl] = useState(false)
  const [follow, setFollow] = useState(false)

  useEffect(() => {
    const el = mount.current!
    let renderer: THREE.WebGLRenderer
    try {
      renderer = new THREE.WebGLRenderer({ antialias: true })
    } catch {
      // Sem WebGL (driver, máquina remota): o resto do console continua.
      setNoGl(true)
      return
    }
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
    const paths = new THREE.Group()
    const dyn = new THREE.Group()
    scene.add(body, paths, dyn)
    // Estrelas ao fundo.
    const stars = new THREE.BufferGeometry()
    const sp: number[] = []
    for (let i = 0; i < 1500; i++) {
      const v = new THREE.Vector3().randomDirection().multiplyScalar(5e5)
      sp.push(v.x, v.y, v.z)
    }
    stars.setAttribute('position', new THREE.Float32BufferAttribute(sp, 3))
    scene.add(new THREE.Points(stars, new THREE.PointsMaterial({ color: 0x8899bb, size: 1.2, sizeAttenuation: false })))
    three.current = { renderer, scene, camera, controls, body, paths, dyn, radius: 1, central: '' }
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
    // Só desenha enquanto a vista está na tela.
    let visible = true
    const io = new IntersectionObserver(([e]) => {
      visible = e.isIntersecting
    })
    io.observe(el)
    let raf = 0
    const loop = () => {
      if (visible) {
        controls.update()
        renderer.render(scene, camera)
      }
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
      io.disconnect()
      controls.dispose()
      clearGroup(body)
      clearGroup(paths)
      clearGroup(dyn)
      stars.dispose()
      renderer.dispose()
      el.removeChild(renderer.domElement)
    }
  }, [])

  // Corpo central.
  const v = snap?.vehicles.find((x) => x.id === vehicle)
  const central = v?.central ?? 'earth'
  const nodes = v?.nodes?.filter((n) => !n.done) ?? []
  const nodesKey = nodes.map((n) => `${n.id}:${n.t}`).join(',')
  useEffect(() => {
    const th = three.current
    if (!th || !hello) return
    // Naves do KSP trazem o raio do corpo (o "sun" do jogo não é o Sol de verdade).
    const radius = v?.central_radius ?? hello.bodies.find((b) => b.id === central)?.radius ?? 6.378e6
    if (th.central === central && th.radius === radius && th.body.children.length > 1) return
    clearGroup(th.body)
    th.central = central
    th.radius = radius
    const tex = bodyTexture(central)
    const sphere = new THREE.Mesh(
      new THREE.SphereGeometry(1, 96, 64),
      tex ? new THREE.MeshPhongMaterial({ map: tex, shininess: central === 'earth' ? 18 : 2, specular: central === 'earth' ? 0x223344 : 0x000000 }) : new THREE.MeshPhongMaterial({ color: BODY_COLOR[central] ?? 0x445566, shininess: 8 }),
    )
    th.body.add(sphere)
    // Atmosfera: um brilho fino em volta (Terra e Marte).
    if (central === 'earth' || central === 'mars' || central === 'kerbin') {
      const atm = new THREE.Mesh(
        new THREE.SphereGeometry(1.025, 64, 48),
        new THREE.MeshBasicMaterial({ color: central === 'mars' ? 0xd98a5a : 0x5aa8ff, transparent: true, opacity: 0.13, side: THREE.BackSide, blending: THREE.AdditiveBlending, depthWrite: false }),
      )
      th.body.add(atm)
    }
    // Traço no solo: gira junto com o corpo.
    const ground = new THREE.Group()
    ground.name = 'groundtrack'
    th.body.add(ground)
    if (central === 'earth' && land && !tex) {
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

  // Trajetórias e queimas: só quando a previsão muda.
  useEffect(() => {
    const th = three.current
    if (!th) return
    const R = th.radius
    clearGroup(th.paths)
    const line = (pts: PredPoint[], color: number | string, opacity = 1) => {
      if (pts.length < 2) return
      const g = new THREE.BufferGeometry().setFromPoints(pts.map((p) => toThree(p[4], p[5], p[6], R)))
      th.paths.add(new THREE.Line(g, new THREE.LineBasicMaterial({ color, transparent: opacity < 1, opacity })))
    }
    line(prediction, 0x4cc3ff)
    for (const o of others) line(o.points, o.color, 0.6)
    // Traço no solo (latitude e longitude da previsão, no corpo que gira).
    const ground = th.body.getObjectByName('groundtrack') as THREE.Group | undefined
    if (ground) {
      clearGroup(ground)
      if (central === 'earth' && prediction.length > 1) {
        const pts = prediction.map((p) => {
          const a = (p[1] * Math.PI) / 180
          const b = (p[2] * Math.PI) / 180
          return new THREE.Vector3(Math.cos(a) * Math.cos(b), Math.sin(a), -Math.cos(a) * Math.sin(b)).multiplyScalar(1.004)
        })
        // Quebra onde a longitude dá a volta.
        let seg: THREE.Vector3[] = []
        for (let i = 0; i < pts.length; i++) {
          if (i > 0 && Math.abs(prediction[i][2] - prediction[i - 1][2]) > 180) {
            if (seg.length > 1) ground.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(seg), new THREE.LineBasicMaterial({ color: 0xffc23d, transparent: true, opacity: 0.8 })))
            seg = []
          }
          seg.push(pts[i])
        }
        if (seg.length > 1) ground.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(seg), new THREE.LineBasicMaterial({ color: 0xffc23d, transparent: true, opacity: 0.8 })))
      }
    }
    // Queimas planejadas: um marcador no ponto previsto mais próximo.
    for (const n of nodes) {
      const p = prediction.reduce<PredPoint | null>((best, q) => (best == null || Math.abs(q[0] - n.t) < Math.abs(best[0] - n.t) ? q : best), null)
      if (!p) continue
      const m = new THREE.Mesh(NODE, basicMaterial(0xff8a3d))
      m.position.copy(toThree(p[4], p[5], p[6], R))
      th.paths.add(m)
    }
  }, [prediction, others, nodesKey, central]) // eslint-disable-line react-hooks/exhaustive-deps

  // Rotação do corpo, Sol e veículos: a cada instantâneo.
  useEffect(() => {
    const th = three.current
    if (!th || !snap) return
    const R = th.radius
    if (central === 'earth') th.body.rotation.y = snap.gmst
    const sun = th.scene.getObjectByName('sun') as THREE.DirectionalLight | undefined
    if (sun) sun.position.copy(toThree(snap.sun[0], snap.sun[1], snap.sun[2], 1).multiplyScalar(10))
    clearGroup(th.dyn)
    for (const x of snap.vehicles) {
      if (!x.r || x.central !== central || x.attached_to) continue
      if (x.kind === 'debris' && x.id !== vehicle) continue
      const m = new THREE.Mesh(MARKER, basicMaterial(x.id === vehicle ? 0xffffff : kindColor(x.kind)))
      m.scale.setScalar(x.id === vehicle ? 0.035 : 0.022)
      m.position.copy(toThree(x.r[0], x.r[1], x.r[2], R))
      th.dyn.add(m)
      const tag = label(x.name, x.id === vehicle ? '#ffffff' : kindColor(x.kind))
      tag.position.copy(m.position)
      th.dyn.add(tag)
      // Câmera seguindo a nave selecionada: anda junto, mantendo o ângulo.
      if (x.id === vehicle && follow) {
        const delta = m.position.clone().sub(th.controls.target)
        th.controls.target.add(delta)
        th.camera.position.add(delta)
      }
    }
  }, [snap, vehicle, central, follow])

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

  return (
    <div ref={mount} style={{ width: '100%', height, position: 'relative' }} role="img" aria-label={t('Vista 3D da órbita')}>
      {!noGl && (
        <label className="o3d-follow small">
          <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} /> {t('seguir a nave')}
        </label>
      )}
      {noGl && <div className="loading muted" style={{ height }}>{t('Sem WebGL neste navegador: a vista 3D não está disponível.')}</div>}
    </div>
  )
}
