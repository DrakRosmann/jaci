// A nave em 3D (modelos em models3d.ts) no referencial local: para cima no
// alto da tela, o planeta embaixo, com a luz do Sol de verdade e as estrelas.
// Efeitos: plumas que se abrem com a altitude, o plasma, o rastro e as
// faíscas da reentrada (pela velocidade em relação ao ar e o aquecimento), o
// solo perto do pouso com a poeira (ou a fumaça na plataforma) levantada pelo
// motor, e as naves acopladas presas nas portas da estação.

import { useEffect, useRef, useState } from 'react'
import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { RoomEnvironment } from 'three/examples/jsm/environments/RoomEnvironment.js'
import { EffectComposer } from 'three/examples/jsm/postprocessing/EffectComposer.js'
import { OutputPass } from 'three/examples/jsm/postprocessing/OutputPass.js'
import { RenderPass } from 'three/examples/jsm/postprocessing/RenderPass.js'
import { UnrealBloomPass } from 'three/examples/jsm/postprocessing/UnrealBloomPass.js'
import { cross, localFrame, unit, type Q, type V3 } from '../lib/attitude'
import { t } from '../lib/i18n'
import { useStore } from '../lib/store'
import type { Vehicle } from '../types'
import { buildVehicle, disposeGroup, groundTexture, planetTexture, plumeMaterial, solidBounds, type Plume } from './models3d'

/** Uma assinatura do que muda a forma (para não remontar a cada instantâneo). */
function shapeKey(v: Vehicle): string {
  const st = (v.stages ?? []).map((s) => `${s.name}:${s.diameter}:${s.length}:${s.boosters}:${s.spacecraft}:${s.engines?.length ?? 0}`).join('|')
  return `${v.id}|${v.kind}|${v.shape}|${v.landing}|${!!v.heat_shield}|${st}|${(v.items ?? []).map((i) => i.id).join(',')}|${v.les?.tower}|${v.systems?.solar?.deployed}|${(v.chutes ?? []).map((c) => `${c.state}${Math.round(c.inflation * 4)}`).join(',')}|${v.gear_down}`
}

const ATMOSPHERE: Record<string, number> = { earth: 0x5aa8ff, mars: 0xd9905a, venus: 0xf0d090 }
const SURFACE = new Set(['earth', 'moon', 'mars'])

/** Halo da atmosfera: brilho na borda (lado de dentro da esfera). */
function atmosphereMaterial(color: number) {
  return new THREE.ShaderMaterial({
    uniforms: { uColor: { value: new THREE.Color(color) } },
    vertexShader: `varying vec3 vN; varying vec3 vV;
      void main(){ vec4 mv = modelViewMatrix*vec4(position,1.0); vN = normalize(normalMatrix*normal); vV = normalize(-mv.xyz); gl_Position = projectionMatrix*mv; }`,
    fragmentShader: `uniform vec3 uColor; varying vec3 vN; varying vec3 vV;
      void main(){ float f = pow(1.0 - abs(dot(vN, vV)), 3.0); gl_FragColor = vec4(uColor*1.4, f); }`,
    side: THREE.BackSide,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
  })
}

/** Choque da reentrada: casca brilhante na frente, mais quente no meio. */
function shockMaterial() {
  return new THREE.ShaderMaterial({
    uniforms: { uTime: { value: 0 }, uPower: { value: 0 } },
    vertexShader: `varying vec3 vN; varying vec3 vV; varying vec3 vP;
      void main(){ vP = position; vec4 mv = modelViewMatrix*vec4(position,1.0); vN = normalize(normalMatrix*normal); vV = normalize(-mv.xyz); gl_Position = projectionMatrix*mv; }`,
    fragmentShader: `uniform float uTime; uniform float uPower; varying vec3 vN; varying vec3 vV; varying vec3 vP;
      void main(){
        float rim = pow(1.0 - abs(dot(vN, vV)), 1.5);
        float nose = clamp(vP.y, 0.0, 1.0);
        float flick = 0.8 + 0.2*sin(uTime*37.0 + vP.x*9.0)*sin(uTime*21.0 + vP.z*7.0);
        float a = (0.06 + rim*rim*0.9) * (0.4 + nose) * flick * uPower;
        vec3 col = mix(vec3(1.0,0.35,0.1), vec3(1.0,0.8,0.95), nose*0.7);
        gl_FragColor = vec4(col*1.4, min(a, 0.9));
      }`,
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
  })
}

/** Ponto redondo e suave para as partículas. */
let softDot: THREE.CanvasTexture | null = null
function dotTexture() {
  if (softDot) return softDot
  const c = document.createElement('canvas')
  c.width = c.height = 64
  const ctx = c.getContext('2d')!
  const g = ctx.createRadialGradient(32, 32, 0, 32, 32, 32)
  g.addColorStop(0, 'rgba(255,255,255,1)')
  g.addColorStop(0.35, 'rgba(255,255,255,0.6)')
  g.addColorStop(1, 'rgba(255,255,255,0)')
  ctx.fillStyle = g
  ctx.fillRect(0, 0, 64, 64)
  softDot = new THREE.CanvasTexture(c)
  return softDot
}

/** Partículas (faíscas, poeira, fumaça). */
function particles(n: number, color: number, size: number, additive: boolean) {
  const geo = new THREE.BufferGeometry()
  geo.setAttribute('position', new THREE.BufferAttribute(new Float32Array(n * 3), 3))
  const mat = new THREE.PointsMaterial({ color, size, map: dotTexture(), alphaTest: 0.01, transparent: true, opacity: 0, depthWrite: false, blending: additive ? THREE.AdditiveBlending : THREE.NormalBlending, sizeAttenuation: true })
  const pts = new THREE.Points(geo, mat)
  pts.frustumCulled = false
  const seeds = Array.from({ length: n }, () => [Math.random(), Math.random(), Math.random(), Math.random()])
  return { pts, seeds }
}

/** Torre de lançamento: treliça ao lado da plataforma. */
function launchTower(height: number): THREE.Group {
  const g = new THREE.Group()
  const mat = new THREE.MeshStandardMaterial({ color: 0xb04a2a, metalness: 0.5, roughness: 0.6 })
  const w = 3
  for (const [x, z] of [
    [-w / 2, -w / 2],
    [w / 2, -w / 2],
    [-w / 2, w / 2],
    [w / 2, w / 2],
  ]) {
    const c = new THREE.Mesh(new THREE.BoxGeometry(0.25, height, 0.25), mat)
    c.position.set(x, height / 2, z)
    g.add(c)
  }
  for (let y = 2; y < height; y += 3) {
    for (const r of [0, Math.PI / 2]) {
      const b = new THREE.Mesh(new THREE.BoxGeometry(w, 0.15, 0.15), mat)
      b.position.set(0, y, 0)
      b.rotation.y = r
      const b2 = b.clone()
      b.position.z = r ? 0 : w / 2
      b.position.x = r ? w / 2 : 0
      b2.position.z = r ? 0 : -w / 2
      b2.position.x = r ? -w / 2 : 0
      g.add(b, b2)
    }
  }
  return g
}

/** Preferências da vista: câmera, brilho e sombras (no navegador). */
type CameraMode = 'free' | 'chase' | 'ground'
interface View3D {
  cam: CameraMode
  bloom: boolean
  shadows: boolean
}
function loadView(): View3D {
  try {
    return { cam: 'free', bloom: true, shadows: true, ...JSON.parse(localStorage.getItem('jaci.3d') ?? '{}') }
  } catch {
    return { cam: 'free', bloom: true, shadows: true }
  }
}

/** Matriz da atitude (quatérnio corpo → inercial) levada para a cena. */
function attitudeMatrix(q: Q, toScene: (d: V3) => V3): THREE.Matrix4 {
  const rot = (b: V3): V3 => {
    const [w, x, y, z] = q
    const u: V3 = [x, y, z]
    const tt = cross(u, b).map((c) => c * 2) as V3
    const c2 = cross(u, tt)
    return [b[0] + tt[0] * w + c2[0], b[1] + tt[1] * w + c2[1], b[2] + tt[2] * w + c2[2]]
  }
  return new THREE.Matrix4().makeBasis(new THREE.Vector3(...unit(toScene(rot([1, 0, 0])))), new THREE.Vector3(...unit(toScene(rot([0, 1, 0])))), new THREE.Vector3(...unit(toScene(rot([0, 0, 1])))))
}

interface Scene3D {
  renderer: THREE.WebGLRenderer
  scene: THREE.Scene
  camera: THREE.PerspectiveCamera
  controls: OrbitControls
  holder: THREE.Group
  sun: THREE.DirectionalLight
  hemi: THREE.HemisphereLight
  planet: THREE.Mesh
  atmo: THREE.Mesh
  ground: THREE.Mesh
  groundKind: string
  planetBody: string
  tower: THREE.Group | null
  shock: THREE.Mesh
  wake: THREE.Mesh
  glow: THREE.PointLight
  sparks: ReturnType<typeof particles>
  dust: ReturnType<typeof particles>
  key: string
  plumes: Plume[]
  composer: EffectComposer
  view: View3D
  /** Outras naves por perto (estágios largados, coifa, o alvo), por id. */
  others: Map<number, { group: THREE.Group; key: string; plumes: Plume[]; label: HTMLDivElement }>
  /** Onde ficam os rótulos das outras naves (por cima do canvas). */
  labels: HTMLDivElement
  highlight: THREE.Box3Helper
  size: number
  /** Raio da nave em volta do eixo (para a casca do choque). */
  radius: number
  /** Nave acoplada selecionada: a câmera mira nela (uma vez, depois de montar). */
  focus: THREE.Group | null
  // Estado dos efeitos, atualizado a cada instantâneo e animado a cada quadro.
  fx: { heat: number; vel: THREE.Vector3; dust: number; dustColor: number; groundY: number; below: THREE.Vector3; trackY: number | null }
}

export function Vehicle3D({ v, height, highlight, tools = true }: { v: Vehicle; height: number; highlight?: number | null; tools?: boolean }) {
  const mount = useRef<HTMLDivElement>(null)
  const three = useRef<Scene3D | null>(null)
  const [view, setView] = useState(loadView)
  const setV = (patch: Partial<View3D>) => {
    const nv = { ...view, ...patch }
    setView(nv)
    if (three.current) three.current.view = nv
    try {
      localStorage.setItem('jaci.3d', JSON.stringify(nv))
    } catch {
      /* sem armazenamento */
    }
  }
  const vehicles = useStore((s) => s.snap?.vehicles)
  const sunDir = useStore((s) => s.snap?.sun)
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
      renderer = new THREE.WebGLRenderer({ antialias: true, logarithmicDepthBuffer: true })
    } catch {
      return
    }
    renderer.setPixelRatio(window.devicePixelRatio)
    renderer.toneMapping = THREE.ACESFilmicToneMapping
    renderer.shadowMap.enabled = true
    renderer.shadowMap.type = THREE.PCFSoftShadowMap
    el.appendChild(renderer.domElement)
    const labels = document.createElement('div')
    labels.className = 'v3d-labels'
    el.appendChild(labels)
    const scene = new THREE.Scene()
    scene.background = new THREE.Color(0x010207)
    const camera = new THREE.PerspectiveCamera(40, 1, 0.05, 2e6)
    const controls = new OrbitControls(camera, renderer.domElement)
    controls.enableDamping = true
    const hemi = new THREE.HemisphereLight(0x8aa0c8, 0x1a1410, 0.55)
    scene.add(hemi)
    const sun = new THREE.DirectionalLight(0xfff4e6, 2.6)
    sun.position.set(30, 50, 20)
    sun.castShadow = true
    sun.shadow.mapSize.set(2048, 2048)
    sun.shadow.bias = -0.0004
    sun.shadow.normalBias = 0.02
    scene.add(sun, sun.target)
    // Reflexos suaves no metal (um ambiente de estúdio, fraco para não clarear o espaço).
    const pmrem = new THREE.PMREMGenerator(renderer)
    scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture
    scene.environmentIntensity = 0.3
    pmrem.dispose()
    // Estrelas.
    const starGeo = new THREE.BufferGeometry()
    const sp = new Float32Array(4000 * 3)
    for (let i = 0; i < 4000; i++) {
      const u = Math.random() * 2 - 1
      const a = Math.random() * Math.PI * 2
      const s = Math.sqrt(1 - u * u)
      sp.set([Math.cos(a) * s * 9e5, u * 9e5, Math.sin(a) * s * 9e5], i * 3)
    }
    starGeo.setAttribute('position', new THREE.BufferAttribute(sp, 3))
    scene.add(new THREE.Points(starGeo, new THREE.PointsMaterial({ color: 0xffffff, size: 1.3, sizeAttenuation: false })))
    // O planeta embaixo e a atmosfera.
    const planet = new THREE.Mesh(new THREE.SphereGeometry(1, 96, 48), new THREE.MeshStandardMaterial({ color: 0xffffff, roughness: 0.95 }))
    const atmo = new THREE.Mesh(new THREE.SphereGeometry(1.025, 64, 32), atmosphereMaterial(0x5aa8ff))
    planet.add(atmo)
    scene.add(planet)
    // O solo de perto (pouso, plataforma, mar).
    const ground = new THREE.Mesh(new THREE.CircleGeometry(12000, 64), new THREE.MeshStandardMaterial({ color: 0xffffff, roughness: 1 }))
    ground.rotation.x = -Math.PI / 2
    ground.visible = false
    ground.receiveShadow = true
    scene.add(ground)
    // Caixa de destaque (o estágio sob o mouse no hangar).
    const highlightBox = new THREE.Box3Helper(new THREE.Box3(), new THREE.Color(0xffc23d))
    highlightBox.visible = false
    scene.add(highlightBox)
    // Brilho das plumas e do plasma.
    const composer = new EffectComposer(renderer)
    composer.addPass(new RenderPass(scene, camera))
    const bloom = new UnrealBloomPass(new THREE.Vector2(256, 256), 0.75, 0.45, 0.8)
    composer.addPass(bloom)
    composer.addPass(new OutputPass())
    const holder = new THREE.Group()
    scene.add(holder)
    // Reentrada: choque na frente, rastro atrás, faíscas e a luz laranja.
    const shock = new THREE.Mesh(new THREE.SphereGeometry(1, 40, 20, 0, Math.PI * 2, 0, Math.PI * 0.55), shockMaterial())
    const wake = new THREE.Mesh(new THREE.CylinderGeometry(1, 0.15, 1, 32, 1, true), plumeMaterial(0xffd9f0, 0xff3d1a))
    const glow = new THREE.PointLight(0xff7a3a, 0, 0, 1.2)
    const sparks = particles(320, 0xffa050, 0.3, true)
    const dust = particles(500, 0xb8a890, 1.2, false)
    shock.visible = wake.visible = false
    scene.add(shock, wake, glow, sparks.pts, dust.pts)
    three.current = {
      renderer,
      scene,
      camera,
      controls,
      holder,
      sun,
      hemi,
      planet,
      atmo,
      ground,
      groundKind: '',
      planetBody: '',
      tower: null,
      shock,
      wake,
      glow,
      sparks,
      dust,
      key: '',
      plumes: [],
      composer,
      view: loadView(),
      others: new Map(),
      labels,
      highlight: highlightBox,
      size: 10,
      radius: 2,
      focus: null,
      fx: { heat: 0, vel: new THREE.Vector3(0, -1, 0), dust: 0, dustColor: 0xb8a890, groundY: -1e5, below: new THREE.Vector3(), trackY: null },
    }
    const resize = () => {
      renderer.setSize(el.clientWidth, el.clientHeight)
      composer.setSize(el.clientWidth, el.clientHeight)
      camera.aspect = el.clientWidth / Math.max(1, el.clientHeight)
      camera.updateProjectionMatrix()
    }
    resize()
    const ro = new ResizeObserver(resize)
    ro.observe(el)
    let raf = 0
    const clock = new THREE.Clock()
    const loop = () => {
      const th = three.current!
      const time = clock.getElapsedTime()
      moveCamera(th)
      for (const p of [...th.plumes, ...[...th.others.values()].flatMap((o) => o.plumes)]) {
        if (!p.group.visible) continue
        ;(p.outer.material as THREE.ShaderMaterial).uniforms.uTime.value = time
        ;(p.core.material as THREE.ShaderMaterial).uniforms.uTime.value = time + 1.7
      }
      animateFx(th, time)
      placeLabels(th)
      sun.castShadow = th.view.shadows
      if (th.view.bloom) composer.render()
      else renderer.render(scene, camera)
      raf = requestAnimationFrame(loop)
    }
    loop()
    return () => {
      cancelAnimationFrame(raf)
      ro.disconnect()
      controls.dispose()
      disposeGroup(scene)
      renderer.dispose()
      el.removeChild(renderer.domElement)
      el.removeChild(labels)
    }
  }, [noGl])

  // Remonta só quando a forma muda; a cada instantâneo, atitude, motores, solo e efeitos.
  useEffect(() => {
    const th = three.current
    if (!th) return
    // A nave e as que estão acopladas nela (ou a estação, se a selecionada está acoplada).
    const host = (v.attached_to != null && vehicles?.find((x) => x.id === v.attached_to)) || v
    const docked = (vehicles ?? []).filter((x) => x.attached_to === host.id)
    const key = [host, ...docked].map(shapeKey).join('#')
    if (key !== th.key) {
      th.holder.children.forEach(disposeGroup)
      th.holder.clear()
      const main = buildVehicle(host)
      // A caixa no referencial do modelo (antes de entrar no holder, que está girado).
      const box = solidBounds(main.group)
      th.holder.add(main.group)
      const plumes = [...main.plumes]
      // Portas: a primeira atrás (−X), a segunda na frente, as outras em cima e embaixo.
      const slots: [THREE.Vector3, THREE.Euler][] = [
        [new THREE.Vector3(-1, 0, 0), new THREE.Euler(0, 0, 0)],
        [new THREE.Vector3(1, 0, 0), new THREE.Euler(0, 0, Math.PI)],
        [new THREE.Vector3(0, 0, -1), new THREE.Euler(0, Math.PI / 2, 0)],
        [new THREE.Vector3(0, 0, 1), new THREE.Euler(0, -Math.PI / 2, 0)],
      ]
      docked.forEach((d, i) => {
        const port = Math.max(0, host.ports?.findIndex((p) => p.occupant === d.id) ?? i)
        const [dir, rot] = slots[port % slots.length]
        const b = buildVehicle(d)
        // O nariz da nave visitante (+X dela) encosta na porta.
        const half = b.length / 2
        const at = dir.x < 0 ? box.min.x - half : dir.x > 0 ? box.max.x + half : 0
        b.group.rotation.copy(rot)
        b.group.position.set(dir.x ? at : main.length * 0.15, 0, dir.z ? dir.z * (main.radius > 20 ? 4.5 : main.radius) + dir.z * half : 0)
        th.holder.add(b.group)
        plumes.push(...b.plumes)
        if (d.id === v.id) th.focus = b.group
      })
      th.plumes = plumes
      th.key = key
      th.radius = Math.max(1, Math.min(main.radius, 6))
      const size = solidBounds(th.holder).getSize(new THREE.Vector3()).length()
      if (Math.abs(th.size - size) > 0.5 || th.camera.position.length() < 1) {
        th.size = size
        th.camera.position.set(size * 1.1, size * 0.45, size * 1.35)
        th.controls.target.set(0, 0, 0)
        // A sombra cobre a nave (e o chão logo embaixo).
        const sc = th.sun.shadow.camera
        sc.left = sc.bottom = -size * 0.75
        sc.right = sc.top = size * 0.75
        sc.near = 0.1
        sc.far = size * 8
        sc.updateProjectionMatrix()
      }
    }
    for (const p of th.plumes) {
      const veh = p.vehicle === v.id ? v : ([host, ...docked].find((x) => x.id === p.vehicle) ?? v)
      updatePlume(p, veh)
    }
    if (!v.r) return
    // Base local → cena: leste → +X, cima → +Y, norte → −Z.
    const { up, east, north } = localFrame(v.r as V3)
    const toScene = (d: V3): V3 => [d[0] * east[0] + d[1] * east[1] + d[2] * east[2], d[0] * up[0] + d[1] * up[1] + d[2] * up[2], -(d[0] * north[0] + d[1] * north[1] + d[2] * north[2])]
    const q = (host.q ?? v.q) as Q | undefined
    if (q) th.holder.quaternion.setFromRotationMatrix(attitudeMatrix(q, toScene))
    th.holder.updateMatrixWorld(true)
    updateOthers(th, host, docked, vehicles ?? [], toScene)
    updateHighlight(th, highlight)
    if (th.focus) {
      // A visitante acoplada em primeiro plano, com a porta e a estação atrás.
      const fb = solidBounds(th.focus)
      const c = fb.getCenter(new THREE.Vector3())
      const r = fb.getSize(new THREE.Vector3()).length()
      th.controls.target.copy(c)
      th.camera.position.copy(c).add(new THREE.Vector3(r * 1.3, r * 0.6, r * 1.6))
      th.focus = null
    }
    // Luz do Sol de verdade (escura na sombra do planeta).
    if (sunDir) th.sun.position.set(...(toScene(sunDir as V3).map((x) => x * th.size * 3) as V3))
    // Na sombra do planeta fica a luz refletida dele (fraca, azulada).
    th.sun.intensity = v.tm?.['eps.sunlit'] === 0 ? 0.25 : 2.6
    th.hemi.intensity = v.tm?.['eps.sunlit'] === 0 ? 0.9 : 0.55
    updateSurface(th, v)
    updateReentry(th, v, toScene)
  }, [v, vehicles, sunDir, highlight])

  return (
    <div className="v3d" style={{ height }}>
      <div ref={mount} style={{ width: '100%', height }} role="img" aria-label={t('A nave em 3D')}>
        {noGl && <div className="loading muted" style={{ height }}>{t('Sem WebGL neste navegador: a vista 3D não está disponível.')}</div>}
      </div>
      {tools && !noGl && (
        <div className="v3d-tools">
          {(
            [
              ['free', t('Livre'), t('Arraste para girar, role para aproximar')],
              ['chase', t('Perseguir'), t('Atrás da nave, no sentido do movimento')],
              ['ground', t('Do chão'), t('Câmera fixa no solo acompanhando (lançamento e pouso)')],
            ] as [CameraMode, string, string][]
          ).map(([m, label, tip]) => (
            <button key={m} className={`small ${view.cam === m ? 'active' : ''}`} onClick={() => setV({ cam: m })} title={tip} aria-pressed={view.cam === m}>
              {label}
            </button>
          ))}
          <button className={`small ${view.bloom ? 'active' : ''}`} onClick={() => setV({ bloom: !view.bloom })} title={t('Brilho das plumas e do plasma')} aria-pressed={view.bloom}>
            ✨
          </button>
          <button className={`small ${view.shadows ? 'active' : ''}`} onClick={() => setV({ shadows: !view.shadows })} title={t('Sombras')} aria-pressed={view.shadows}>
            ◐
          </button>
        </div>
      )}
    </div>
  )
}

/** Câmera: livre (a do mouse), perseguindo a nave ou parada no chão olhando para ela. */
function moveCamera(th: Scene3D) {
  const { camera, controls } = th
  const mode = th.view.cam
  controls.enabled = mode === 'free'
  if (mode === 'free') {
    if (camera.fov !== 40) {
      camera.fov = 40
      camera.updateProjectionMatrix()
    }
    controls.update()
    return
  }
  const size = th.size
  if (mode === 'ground' && th.fx.trackY != null) {
    // Uma câmera de rastreio no solo, a uma distância fixa da plataforma; a teleobjetiva fecha com a distância.
    const d = Math.max(60, size * 2.5)
    camera.position.set(d * 0.8, th.fx.trackY + 2, d * 0.6)
    camera.lookAt(0, 0, 0)
    const dist = camera.position.length()
    const fov = THREE.MathUtils.clamp((2 * Math.atan((size * 0.9) / dist) * 180) / Math.PI, 0.5, 50)
    if (Math.abs(camera.fov - fov) > 0.01) {
      camera.fov = fov
      camera.updateProjectionMatrix()
    }
    return
  }
  // Perseguição: atrás, no sentido contrário ao do movimento, um pouco acima.
  const dir = th.fx.vel.lengthSq() > 0 ? th.fx.vel.clone().normalize() : new THREE.Vector3(0, 1, 0)
  // De três quartos: atrás e um pouco de lado, para ver as plumas e o corpo.
  const side = new THREE.Vector3(0, 1, 0).cross(dir)
  if (side.lengthSq() < 1e-6) side.set(1, 0, 0)
  const want = dir
    .clone()
    .multiplyScalar(-size * 1.2)
    .addScaledVector(side.normalize(), size * 0.8)
    .add(new THREE.Vector3(0, size * 0.3, 0))
  // Vindo de longe (da câmera do chão), pula direto; perto, desliza.
  if (camera.position.distanceTo(want) > size * 4) camera.position.copy(want)
  else camera.position.lerp(want, 0.08)
  camera.lookAt(0, 0, 0)
  if (camera.fov !== 40) {
    camera.fov = 40
    camera.updateProjectionMatrix()
  }
}

/** Naves por perto (até 5 km, as 6 mais próximas): estágios largados, a coifa, o alvo do encontro. */
function updateOthers(th: Scene3D, host: Vehicle, docked: Vehicle[], all: Vehicle[], toScene: (d: V3) => V3) {
  if (!host.r) return
  const skip = new Set([host.id, ...docked.map((d) => d.id)])
  const near = all
    .filter((o) => !skip.has(o.id) && o.attached_to == null && o.r && o.central === host.central && o.id >= 0)
    .map((o) => ({ o, d: Math.hypot(o.r![0] - host.r![0], o.r![1] - host.r![1], o.r![2] - host.r![2]) }))
    .filter((x) => x.d < 5_000)
    .sort((a, b) => a.d - b.d)
    .slice(0, 6)
  const keep = new Set(near.map((x) => x.o.id))
  for (const [id, o] of th.others) {
    if (!keep.has(id)) {
      disposeGroup(o.group)
      th.scene.remove(o.group)
      o.label.remove()
      th.others.delete(id)
    }
  }
  for (const { o, d } of near) {
    const key = shapeKey(o)
    let ent = th.others.get(o.id)
    if (!ent || ent.key !== key) {
      if (ent) {
        disposeGroup(ent.group)
        th.scene.remove(ent.group)
        ent.label.remove()
      }
      const b = buildVehicle(o)
      const g = new THREE.Group()
      g.add(b.group)
      th.scene.add(g)
      const label = document.createElement('div')
      label.className = 'v3d-label'
      th.labels.appendChild(label)
      ent = { group: g, key, plumes: b.plumes, label }
      th.others.set(o.id, ent)
    }
    ent.label.textContent = `${o.name} · ${d < 1000 ? `${Math.round(d)} m` : `${(d / 1000).toFixed(1)} km`}`
    const rel = toScene([o.r![0] - host.r![0], o.r![1] - host.r![1], o.r![2] - host.r![2]])
    ent.group.position.set(...rel)
    if (o.q) ent.group.quaternion.setFromRotationMatrix(attitudeMatrix(o.q as Q, toScene))
    for (const p of ent.plumes) updatePlume(p, o)
  }
}

/** Rótulos das outras naves na posição delas na tela (escondidos atrás da câmera). */
function placeLabels(th: Scene3D) {
  const w = th.renderer.domElement.clientWidth
  const h = th.renderer.domElement.clientHeight
  for (const o of th.others.values()) {
    const p = o.group.position.clone().project(th.camera)
    const show = p.z < 1 && Math.abs(p.x) < 1.1 && Math.abs(p.y) < 1.1
    o.label.style.display = show ? 'block' : 'none'
    if (show) o.label.style.transform = `translate(${((p.x + 1) / 2) * w}px, ${((1 - p.y) / 2) * h}px)`
  }
}

/** Contorno do estágio destacado (índice na pilha), ou nada. */
function updateHighlight(th: Scene3D, stage: number | null | undefined) {
  if (stage == null) {
    th.highlight.visible = false
    return
  }
  const box = new THREE.Box3()
  th.holder.traverse((o) => {
    const m = o as THREE.Mesh
    if (m.isMesh && o.userData.stage === stage && !o.parent?.userData.fx) {
      m.geometry.computeBoundingBox()
      box.union(m.geometry.boundingBox!.clone().applyMatrix4(m.matrixWorld))
    }
  })
  th.highlight.visible = !box.isEmpty()
  if (th.highlight.visible) th.highlight.box.copy(box.expandByScalar(0.15))
}

/** Plumas dos motores acesos: força pelo empuxo, abertura pela altitude (menos ar, pluma mais larga). */
function updatePlume(p: Plume, v: Vehicle) {
  const st = v.stages?.[p.stage]
  const e = p.stage < 0 ? undefined : (st?.engines?.[p.engine] ?? st?.engines?.[0])
  const on = !!e && (e.state === 'running' || e.state === 'starting') && e.thrust > 0
  p.group.visible = on
  const hot = p.bell.material as THREE.MeshStandardMaterial
  hot.emissiveIntensity = on ? 0.9 : Math.max(0, hot.emissiveIntensity * 0.7)
  if (!on || !e) return
  const power = Math.min(1, e.thrust / Math.max(1, e.rated))
  const vac = Math.min(1, Math.max(0, (v.alt - 2) / 50))
  const wide = 1 + 2.4 * vac
  const long = (0.7 + 0.5 * power) * (1 + 1.2 * vac)
  p.outer.scale.set(wide, long, wide)
  p.outer.position.x = -p.radius * 5 * long
  ;(p.outer.material as THREE.ShaderMaterial).uniforms.uPower.value = 0.6 + 0.5 * power
  p.core.scale.set(1, 0.7 + 0.5 * power, 1)
  p.core.position.x = -p.radius * 2.25 * p.core.scale.y
  ;(p.core.material as THREE.ShaderMaterial).uniforms.uPower.value = 0.8 * (1 - vac * 0.5)
}

/** O planeta embaixo (girado para a posição da nave) ou, perto do chão, o solo. */
function updateSurface(th: Scene3D, v: Vehicle) {
  const altM = Math.max(0, v.alt * 1000)
  const body = v.central
  const bounds = solidBounds(th.holder)
  const near = SURFACE.has(body) && (altM < 4000 || ['prelaunch', 'landed', 'splashdown'].includes(v.phase))
  // A câmera do chão acompanha até 40 km de altura (o solo pode já estar escondido).
  th.fx.trackY = SURFACE.has(body) && altM < 40_000 ? bounds.min.y - (['landed', 'prelaunch', 'splashdown'].includes(v.phase) ? 0 : altM) : null
  // Planeta: uma esfera com a curvatura de verdade "vista de cima", cada vez mais longe.
  th.planet.visible = !near && body !== 'sun'
  if (th.planet.visible) {
    if (th.planetBody !== body) {
      const mat = th.planet.material as THREE.MeshStandardMaterial
      mat.map?.dispose()
      mat.map = planetTexture(body)
      mat.needsUpdate = true
      th.atmo.visible = body in ATMOSPHERE
      if (th.atmo.visible) (th.atmo.material as THREE.ShaderMaterial).uniforms.uColor.value.set(ATMOSPHERE[body])
      th.planetBody = body
    }
    const R = 6000
    const hv = 60 + 320 * Math.log10(1 + v.alt)
    th.planet.scale.setScalar(R)
    th.planet.position.set(0, -(R + hv), 0)
    // O ponto embaixo da nave (lat, lon) no alto da esfera, com o leste para +X.
    const phi = ((v.lon + 180) / 360) * Math.PI * 2
    const lat = (v.lat * Math.PI) / 180
    const d = new THREE.Vector3(-Math.cos(phi) * Math.cos(lat), Math.sin(lat), Math.sin(phi) * Math.cos(lat))
    const q1 = new THREE.Quaternion().setFromUnitVectors(d, new THREE.Vector3(0, 1, 0))
    const e = new THREE.Vector3(Math.sin(phi), 0, Math.cos(phi)).applyQuaternion(q1)
    const q2 = new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(e.x, 0, e.z).normalize(), new THREE.Vector3(1, 0, 0))
    th.planet.quaternion.copy(q2.multiply(q1))
  }
  // Solo: regolito, poeira de Marte, a plataforma ou o mar.
  th.ground.visible = near
  if (near) {
    const kind = body === 'moon' ? 'moon' : body === 'mars' ? 'mars' : v.phase === 'prelaunch' || (v.phase === 'ascent' && altM < 3000) || v.landing === 'runway' ? 'pad' : 'sea'
    if (th.groundKind !== kind) {
      const mat = th.ground.material as THREE.MeshStandardMaterial
      mat.map?.dispose()
      mat.map = groundTexture(kind)
      mat.roughness = kind === 'sea' ? 0.35 : 1
      mat.metalness = kind === 'sea' ? 0.2 : 0
      mat.needsUpdate = true
      th.groundKind = kind
    }
    const gy = bounds.min.y - (['landed', 'prelaunch', 'splashdown'].includes(v.phase) ? 0 : altM)
    th.ground.position.y = gy
    th.fx.groundY = gy
    // Torre na plataforma, que fica no chão quando o foguete sobe.
    if (kind === 'pad' && v.kind !== 'shuttle' && (v.phase === 'prelaunch' || v.phase === 'ascent')) {
      const h = Math.max(10, (bounds.max.y - bounds.min.y) * 1.05)
      if (!th.tower || Math.abs(th.tower.userData.h - h) > 1) {
        if (th.tower) {
          disposeGroup(th.tower)
          th.scene.remove(th.tower)
        }
        th.tower = launchTower(h)
        th.tower.userData.h = h
        th.scene.add(th.tower)
      }
      th.tower.position.set(Math.max(bounds.max.x, -bounds.min.x) + 4, gy, 0)
      th.tower.visible = true
    } else if (th.tower) th.tower.visible = false
  } else if (th.tower) th.tower.visible = false
  // Poeira (Lua, Marte) ou fumaça (plataforma) quando o motor sopra o chão.
  const thrust = (v.stages ?? []).flatMap((s) => s.engines ?? []).reduce((a, e) => a + (e.state === 'running' ? e.thrust : 0), 0)
  const reach = 70
  th.fx.dust = near && thrust > 0 && altM < reach && th.groundKind !== 'sea' ? Math.min(1, (1 - altM / reach) * 1.3) : 0
  th.fx.dustColor = th.groundKind === 'mars' ? 0xc07a4a : th.groundKind === 'moon' ? 0xa8a49a : 0xe8e8e8
  ;(th.dust.pts.material as THREE.PointsMaterial).color.set(th.fx.dustColor)
  ;(th.dust.pts.material as THREE.PointsMaterial).size = th.groundKind === 'pad' ? 4 : 1.4
}

/** Reentrada: intensidade pelo aquecimento e direção pela velocidade em relação ao ar. */
function updateReentry(th: Scene3D, v: Vehicle, toScene: (d: V3) => V3) {
  const vs = v.v_surf as V3 | undefined
  const speed = vs ? Math.hypot(...vs) : 0
  const flux = v.tm?.['ent.flux'] ?? v.heat_shield?.flux ?? 0
  const q = v.tm?.['nav.q'] ?? 0
  const heat = ATMOSPHERE[v.central] !== undefined && speed > 1200 && v.alt < 140 ? Math.min(1, Math.max(flux / 150, (q * 1000 * speed) / 1.5e7)) : 0
  th.fx.heat = heat
  if (vs && speed > 1) th.fx.vel.set(...toScene(unit(vs)))
}

/** Animação dos efeitos a cada quadro. */
function animateFx(th: Scene3D, time: number) {
  const { heat, vel } = th.fx
  const size = th.size / 2 || 3
  const on = heat > 0.02
  th.shock.visible = th.wake.visible = on
  th.glow.intensity = on ? heat * 40 * size : 0
  const sparkMat = th.sparks.pts.material as THREE.PointsMaterial
  sparkMat.opacity = on ? Math.min(1, heat * 1.5) : 0
  if (on) {
    const dir = vel.clone().normalize()
    const qd = new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir)
    // A casca do choque à frente; o rastro atrás.
    th.shock.quaternion.copy(qd)
    // Casca justa na frente (o raio da nave, não o comprimento).
    const r = Math.min(size, th.radius * 1.35)
    th.shock.scale.set(r, r * 0.8, r)
    th.shock.position.copy(dir.clone().multiplyScalar(r * 0.1))
    const sm = th.shock.material as THREE.ShaderMaterial
    sm.uniforms.uTime.value = time
    sm.uniforms.uPower.value = heat
    const wl = size * (4 + 10 * heat)
    th.wake.quaternion.copy(new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 1, 0), dir.clone().negate()))
    th.wake.scale.set(size * 0.95, wl, size * 0.95)
    th.wake.position.copy(dir.clone().multiplyScalar(-wl / 2))
    const wm = th.wake.material as THREE.ShaderMaterial
    wm.uniforms.uTime.value = time
    wm.uniforms.uPower.value = heat * 0.9
    th.glow.position.copy(dir.clone().multiplyScalar(size))
    // Faíscas: saem da borda do choque e correm para trás.
    const pos = th.sparks.pts.geometry.getAttribute('position') as THREE.BufferAttribute
    const side = new THREE.Vector3(1, 0, 0).cross(dir)
    if (side.lengthSq() < 1e-6) side.set(0, 0, 1)
    side.normalize()
    const side2 = dir.clone().cross(side)
    th.sparks.seeds.forEach(([a, b, c, d], i) => {
      const life = (time * (0.6 + d) + a) % 1
      const ang = b * Math.PI * 2
      const rr = size * (0.8 + c * 0.5) * (1 + life * 0.8)
      const p = dir
        .clone()
        .multiplyScalar(-life * size * (6 + 8 * heat))
        .addScaledVector(side, Math.cos(ang) * rr)
        .addScaledVector(side2, Math.sin(ang) * rr)
      pos.setXYZ(i, p.x, p.y, p.z)
    })
    pos.needsUpdate = true
  }
  // Poeira ou fumaça: anel que corre pelo chão a partir do ponto embaixo do motor.
  const dustMat = th.dust.pts.material as THREE.PointsMaterial
  dustMat.opacity = th.fx.dust * 0.75
  if (th.fx.dust > 0) {
    const pos = th.dust.pts.geometry.getAttribute('position') as THREE.BufferAttribute
    const smoke = th.groundKind === 'pad'
    th.dust.seeds.forEach(([a, b, c], i) => {
      const life = (time * (0.35 + c * 0.4) + a) % 1
      const ang = b * Math.PI * 2
      const r = (smoke ? 4 : 1) + life * (smoke ? 60 : 40) * (0.6 + c)
      const hgt = smoke ? life * 18 * c : life * 3 * c
      pos.setXYZ(i, Math.cos(ang) * r, th.fx.groundY + 0.3 + hgt, Math.sin(ang) * r)
    })
    pos.needsUpdate = true
  }
}
