// Modelos 3D das naves, montados dos estágios do instantâneo, e as texturas
// geradas em canvas (painéis com rebites, células solares, manta térmica,
// regolito, a superfície dos planetas). O eixo da frente é +X (como no
// núcleo), Y para a direita e Z para baixo.

import * as THREE from 'three'
import type { Vehicle } from '../types'

// ── Texturas ──

function canvasTexture(w: number, h: number, draw: (ctx: CanvasRenderingContext2D) => void, repeat: [number, number] = [1, 1]): THREE.CanvasTexture {
  const c = document.createElement('canvas')
  c.width = w
  c.height = h
  draw(c.getContext('2d')!)
  const tex = new THREE.CanvasTexture(c)
  tex.wrapS = tex.wrapT = THREE.RepeatWrapping
  tex.repeat.set(...repeat)
  tex.colorSpace = THREE.SRGBColorSpace
  tex.anisotropy = 4
  return tex
}

/** Gerador pseudoaleatório com semente (as texturas saem sempre iguais). */
function rng(seed: number) {
  let s = seed >>> 0
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0
    return s / 2 ** 32
  }
}

/** Ruído de valor em várias oitavas, de 0 a 1. */
function noise2(seed: number) {
  const r = rng(seed)
  const n = 64
  const grid = Array.from({ length: n * n }, () => r())
  const at = (x: number, y: number) => grid[(((y % n) + n) % n) * n + (((x % n) + n) % n)]
  const smooth = (t: number) => t * t * (3 - 2 * t)
  const val = (x: number, y: number) => {
    const xi = Math.floor(x)
    const yi = Math.floor(y)
    const fx = smooth(x - xi)
    const fy = smooth(y - yi)
    const a = at(xi, yi) * (1 - fx) + at(xi + 1, yi) * fx
    const b = at(xi, yi + 1) * (1 - fx) + at(xi + 1, yi + 1) * fx
    return a * (1 - fy) + b * fy
  }
  return (x: number, y: number) => {
    let sum = 0
    let amp = 0.5
    let f = 1
    for (let o = 0; o < 5; o++) {
      sum += val(x * f, y * f) * amp
      amp /= 2
      f *= 2
    }
    return sum / 0.97
  }
}

/** Painéis de um tanque: costuras, rebites e um pouco de sujeira. */
function panelTexture(base: string, line: string, seed: number) {
  return canvasTexture(256, 256, (ctx) => {
    ctx.fillStyle = base
    ctx.fillRect(0, 0, 256, 256)
    const r = rng(seed)
    for (let k = 0; k < 160; k++) {
      ctx.fillStyle = `rgba(0,0,0,${r() * 0.05})`
      ctx.fillRect(r() * 256, r() * 256, 8 + r() * 30, 2 + r() * 8)
    }
    ctx.strokeStyle = line
    ctx.lineWidth = 1.5
    for (let y = 0; y <= 256; y += 64) {
      ctx.beginPath()
      ctx.moveTo(0, y)
      ctx.lineTo(256, y)
      ctx.stroke()
    }
    for (let x = 0; x <= 256; x += 128) {
      ctx.beginPath()
      ctx.moveTo(x, 0)
      ctx.lineTo(x, 256)
      ctx.stroke()
    }
    ctx.fillStyle = line
    for (let y = 0; y <= 256; y += 64) for (let x = 4; x < 256; x += 8) ctx.fillRect(x, y + 3, 1.5, 1.5)
  })
}

const TEX = {
  get hull() {
    return (this._hull ??= panelTexture('#e4e8ee', 'rgba(70,80,95,0.35)', 1))
  },
  get dark() {
    return (this._dark ??= panelTexture('#3a4048', 'rgba(0,0,0,0.5)', 2))
  },
  get tank() {
    return (this._tank ??= canvasTexture(256, 256, (ctx) => {
      // Espuma do tanque externo: laranja com manchas.
      const n = noise2(7)
      const img = ctx.createImageData(256, 256)
      for (let y = 0; y < 256; y++)
        for (let x = 0; x < 256; x++) {
          const v = n(x / 18, y / 18)
          const i = (y * 256 + x) * 4
          img.data[i] = 190 + v * 40
          img.data[i + 1] = 100 + v * 30
          img.data[i + 2] = 40 + v * 15
          img.data[i + 3] = 255
        }
      ctx.putImageData(img, 0, 0)
    }))
  },
  get solar() {
    return (this._solar ??= canvasTexture(
      256,
      256,
      (ctx) => {
        ctx.fillStyle = '#c9ced8'
        ctx.fillRect(0, 0, 256, 256)
        const r = rng(3)
        for (let y = 0; y < 8; y++)
          for (let x = 0; x < 8; x++) {
            const g = 30 + r() * 20
            ctx.fillStyle = `rgb(${g * 0.4},${g * 0.7},${g * 2.6})`
            ctx.fillRect(x * 32 + 1.5, y * 32 + 1.5, 29, 29)
            ctx.fillStyle = 'rgba(255,255,255,0.08)'
            ctx.fillRect(x * 32 + 2, y * 32 + 2, 29, 4)
          }
      },
      [1, 4],
    ))
  },
  get mli() {
    return (this._mli ??= canvasTexture(256, 256, (ctx) => {
      // Manta térmica dourada, amassada.
      ctx.fillStyle = '#c8a03a'
      ctx.fillRect(0, 0, 256, 256)
      const r = rng(5)
      for (let k = 0; k < 220; k++) {
        const l = 0.3 + r() * 0.7
        ctx.fillStyle = `rgba(${255 * l},${210 * l},${110 * l},0.35)`
        ctx.beginPath()
        const x = r() * 256
        const y = r() * 256
        ctx.moveTo(x, y)
        for (let i = 0; i < 4; i++) ctx.lineTo(x + (r() - 0.5) * 60, y + (r() - 0.5) * 60)
        ctx.fill()
      }
    }))
  },
  get radiator() {
    return (this._radiator ??= canvasTexture(
      128,
      128,
      (ctx) => {
        ctx.fillStyle = '#eef1f5'
        ctx.fillRect(0, 0, 128, 128)
        ctx.strokeStyle = 'rgba(120,130,145,0.6)'
        for (let x = 0; x < 128; x += 8) {
          ctx.beginPath()
          ctx.moveTo(x, 0)
          ctx.lineTo(x, 128)
          ctx.stroke()
        }
      },
      [2, 2],
    ))
  },
  _hull: undefined as THREE.CanvasTexture | undefined,
  _dark: undefined as THREE.CanvasTexture | undefined,
  _tank: undefined as THREE.CanvasTexture | undefined,
  _solar: undefined as THREE.CanvasTexture | undefined,
  _mli: undefined as THREE.CanvasTexture | undefined,
  _radiator: undefined as THREE.CanvasTexture | undefined,
}

/** Solo perto do pouso: regolito com crateras (Lua), poeira (Marte), concreto e grama (base) ou mar. */
export function groundTexture(kind: 'moon' | 'mars' | 'pad' | 'sea'): THREE.CanvasTexture {
  const base: Record<string, [number, number, number]> = { moon: [120, 118, 112], mars: [168, 92, 52], pad: [92, 110, 70], sea: [18, 52, 92] }
  const [r0, g0, b0] = base[kind]
  return canvasTexture(
    512,
    512,
    (ctx) => {
      const n = noise2(kind.length * 11)
      const img = ctx.createImageData(512, 512)
      for (let y = 0; y < 512; y++)
        for (let x = 0; x < 512; x++) {
          const v = n(x / 40, y / 40) - 0.5
          const fine = (Math.sin(x * 12.9898 + y * 78.233) * 43758.5453) % 1
          const k = 1 + v * (kind === 'sea' ? 0.35 : 0.5) + fine * 0.08
          const i = (y * 512 + x) * 4
          img.data[i] = r0 * k
          img.data[i + 1] = g0 * k
          img.data[i + 2] = b0 * k
          img.data[i + 3] = 255
        }
      ctx.putImageData(img, 0, 0)
      const r = rng(kind.length)
      if (kind === 'moon' || kind === 'mars') {
        // Crateras: borda clara e fundo escuro.
        for (let k = 0; k < (kind === 'moon' ? 70 : 25); k++) {
          const x = r() * 512
          const y = r() * 512
          const rad = 3 + r() ** 3 * 40
          ctx.fillStyle = 'rgba(0,0,0,0.18)'
          ctx.beginPath()
          ctx.arc(x, y, rad, 0, Math.PI * 2)
          ctx.fill()
          ctx.strokeStyle = 'rgba(255,255,255,0.15)'
          ctx.lineWidth = rad * 0.15
          ctx.beginPath()
          ctx.arc(x - rad * 0.1, y - rad * 0.1, rad, Math.PI * 0.9, Math.PI * 1.9)
          ctx.stroke()
        }
      }
      if (kind === 'pad') {
        ctx.fillStyle = '#9a9a96'
        ctx.beginPath()
        ctx.arc(256, 256, 90, 0, Math.PI * 2)
        ctx.fill()
        ctx.fillStyle = '#5c5c58'
        ctx.fillRect(244, 170, 24, 172)
      }
    },
    kind === 'pad' ? [1, 1] : [6, 6],
  )
}

/** Superfície de um corpo inteiro (equirretangular): mares e continentes, mares lunares, a poeira de Marte. */
export function planetTexture(body: string): THREE.CanvasTexture {
  return canvasTexture(1024, 512, (ctx) => {
    const n = noise2(body.length * 31 + 3)
    const c = noise2(body.length * 17 + 9)
    const img = ctx.createImageData(1024, 512)
    for (let y = 0; y < 512; y++) {
      const lat = Math.abs(y / 512 - 0.5) * 2
      for (let x = 0; x < 1024; x++) {
        const v = n(x / 90, y / 90)
        const i = (y * 1024 + x) * 4
        let rgb: [number, number, number]
        if (body === 'earth') {
          const cloud = c(x / 50, y / 35)
          rgb = v > 0.55 ? (lat > 0.8 ? [235, 240, 245] : v > 0.68 ? [120, 105, 70] : [60, 110, 50]) : [16, 48, 105]
          if (lat > 0.88) rgb = [235, 240, 245]
          const cl = Math.max(0, (cloud - 0.55) * 2.6)
          rgb = rgb.map((ch) => ch * (1 - cl) + 250 * cl) as [number, number, number]
        } else if (body === 'mars') {
          rgb = [170 + v * 50, 80 + v * 35, 45 + v * 20]
          if (lat > 0.9) rgb = [230, 225, 220]
        } else if (body === 'moon') {
          const k = 90 + v * 110
          rgb = [k, k * 0.98, k * 0.93]
        } else {
          const k = 120 + v * 90
          rgb = [k, k * 0.9, k * 0.75]
        }
        img.data[i] = rgb[0]
        img.data[i + 1] = rgb[1]
        img.data[i + 2] = rgb[2]
        img.data[i + 3] = 255
      }
    }
    ctx.putImageData(img, 0, 0)
  })
}

// ── Materiais ──

const MAT = {
  get hull() {
    return (this._hull ??= new THREE.MeshStandardMaterial({ color: 0xffffff, map: TEX.hull, metalness: 0.25, roughness: 0.55 }))
  },
  get darkHull() {
    return (this._darkHull ??= new THREE.MeshStandardMaterial({ color: 0xffffff, map: TEX.dark, metalness: 0.45, roughness: 0.6 }))
  },
  get tank() {
    return (this._tank ??= new THREE.MeshStandardMaterial({ color: 0xffffff, map: TEX.tank, metalness: 0.1, roughness: 0.85 }))
  },
  get solar() {
    return (this._solar ??= new THREE.MeshStandardMaterial({ color: 0xffffff, map: TEX.solar, metalness: 0.55, roughness: 0.3, side: THREE.DoubleSide }))
  },
  get mli() {
    return (this._mli ??= new THREE.MeshStandardMaterial({ color: 0xffffff, map: TEX.mli, metalness: 0.85, roughness: 0.35 }))
  },
  get radiator() {
    return (this._radiator ??= new THREE.MeshStandardMaterial({ color: 0xffffff, map: TEX.radiator, metalness: 0.2, roughness: 0.4, side: THREE.DoubleSide }))
  },
  band: new THREE.MeshStandardMaterial({ color: 0x1c2b44, metalness: 0.4, roughness: 0.5 }),
  black: new THREE.MeshStandardMaterial({ color: 0x15181d, metalness: 0.3, roughness: 0.8 }),
  shield: new THREE.MeshStandardMaterial({ color: 0x5a4632, metalness: 0.1, roughness: 0.95 }),
  window: new THREE.MeshStandardMaterial({ color: 0x0a1424, metalness: 0.9, roughness: 0.1, emissive: 0x0a2040, emissiveIntensity: 0.4 }),
  nozzle: new THREE.MeshStandardMaterial({ color: 0x6a6d72, metalness: 0.9, roughness: 0.3, side: THREE.DoubleSide }),
  nozzleHot: new THREE.MeshStandardMaterial({ color: 0x6a6d72, metalness: 0.9, roughness: 0.3, side: THREE.DoubleSide, emissive: 0xff5a1a, emissiveIntensity: 0 }),
  chute: new THREE.MeshStandardMaterial({ color: 0xff8a3d, roughness: 0.9, side: THREE.DoubleSide }),
  chuteWhite: new THREE.MeshStandardMaterial({ color: 0xf2f2ee, roughness: 0.9, side: THREE.DoubleSide }),
  truss: new THREE.MeshStandardMaterial({ color: 0x9aa3ad, metalness: 0.6, roughness: 0.5, wireframe: false }),
  orbiterBelly: new THREE.MeshStandardMaterial({ color: 0x202226, roughness: 0.85 }),
  _hull: undefined as THREE.MeshStandardMaterial | undefined,
  _darkHull: undefined as THREE.MeshStandardMaterial | undefined,
  _tank: undefined as THREE.MeshStandardMaterial | undefined,
  _solar: undefined as THREE.MeshStandardMaterial | undefined,
  _mli: undefined as THREE.MeshStandardMaterial | undefined,
  _radiator: undefined as THREE.MeshStandardMaterial | undefined,
}

// ── Plumas (shader aditivo) ──

const PLUME_VERT = `
varying vec2 vUv;
varying vec3 vN;
varying vec3 vV;
void main() {
  vUv = uv;
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  vN = normalize(normalMatrix * normal);
  vV = normalize(-mv.xyz);
  gl_Position = projectionMatrix * mv;
}`
const PLUME_FRAG = `
uniform float uTime;
uniform float uPower;
uniform vec3 uCore;
uniform vec3 uTail;
varying vec2 vUv;
varying vec3 vN;
varying vec3 vV;
void main() {
  float along = vUv.y;                       // 1 no bocal, 0 na ponta
  float edge = pow(abs(dot(vN, vV)), 1.6);   // mais forte no meio do cone
  float flick = 0.85 + 0.15 * sin(uTime * 40.0 + vUv.y * 30.0) * sin(uTime * 23.0 + vUv.x * 12.0);
  float a = pow(along, 1.4) * edge * flick * uPower;
  vec3 col = mix(uTail, uCore, pow(along, 2.0));
  gl_FragColor = vec4(col, a);
}`

export function plumeMaterial(core = 0xfff1c8, tail = 0xff6a1a): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    vertexShader: PLUME_VERT,
    fragmentShader: PLUME_FRAG,
    uniforms: { uTime: { value: 0 }, uPower: { value: 1 }, uCore: { value: new THREE.Color(core) }, uTail: { value: new THREE.Color(tail) } },
    transparent: true,
    depthWrite: false,
    blending: THREE.AdditiveBlending,
    side: THREE.DoubleSide,
  })
}

export interface Plume {
  /** O cone externo e o núcleo, ancorados na saída do bocal (o grupo cresce para −X). */
  group: THREE.Group
  outer: THREE.Mesh
  core: THREE.Mesh
  stage: number
  engine: number
  vehicle: number
  radius: number
  bell: THREE.Mesh
}

export interface Built {
  group: THREE.Group
  plumes: Plume[]
  /** Extensão ao longo de X (m), já centrada: de −length/2 a +length/2. */
  length: number
  /** Raio maior, para enquadrar. */
  radius: number
}

// ── Montagem ──

export function buildVehicle(v: Vehicle): Built {
  const g = new THREE.Group()
  const plumes: Plume[] = []
  const stages = v.stages ?? []
  let x = 0
  let maxR = 0.5
  // Estágio a que as peças criadas pertencem (para destacar no hangar).
  let cur = 0
  const tag = <T extends THREE.Object3D>(o: T): T => {
    o.userData.stage = cur
    return o
  }
  /** Cilindro (ou tronco de cone) ao longo do X, de `at` a `at + len`. */
  const cyl = (r1: number, r2: number, len: number, mat: THREE.Material, at: number, y = 0, z = 0, seg = 32) => {
    const m = tag(new THREE.Mesh(new THREE.CylinderGeometry(r2, r1, len, seg, 1, false), mat))
    m.rotation.z = -Math.PI / 2
    m.position.set(at + len / 2, y, z)
    g.add(m)
    maxR = Math.max(maxR, Math.abs(y) + r1, Math.abs(z) + r1)
    return m
  }
  const box = (sx: number, sy: number, sz: number, mat: THREE.Material, px: number, py = 0, pz = 0) => {
    const m = tag(new THREE.Mesh(new THREE.BoxGeometry(sx, sy, sz), mat))
    m.position.set(px, py, pz)
    g.add(m)
    return m
  }
  /** Sino de motor (perfil de Laval) com a saída em `exitX`, apontado para −X, e a pluma. */
  const bell = (si: number, ei: number, r: number, exitX: number, y = 0, z = 0) => {
    const len = r * 2.4
    const pts: THREE.Vector2[] = []
    for (let k = 0; k <= 12; k++) {
      const s = k / 12
      // Garganta estreita perto da câmara, abrindo em parábola até a saída.
      pts.push(new THREE.Vector2(r * (0.28 + 0.72 * Math.pow(s, 0.6)), len * (1 - s)))
    }
    const geo = new THREE.LatheGeometry(pts, 28)
    const m = tag(new THREE.Mesh(geo, MAT.nozzleHot.clone()))
    m.rotation.z = Math.PI / 2 // Y do torno → −X
    m.position.set(exitX + len, y, z)
    g.add(m)
    // Câmara de combustão em cima do sino.
    const ch = new THREE.Mesh(new THREE.CylinderGeometry(r * 0.32, r * 0.32, r * 0.6, 16), MAT.nozzle)
    ch.rotation.z = -Math.PI / 2
    ch.position.set(exitX + len + r * 0.3, y, z)
    g.add(ch)
    const pg = new THREE.Group()
    pg.position.set(exitX, y, z)
    const outer = new THREE.Mesh(new THREE.CylinderGeometry(r * 0.95, r * 1.6, r * 10, 28, 1, true), plumeMaterial())
    outer.rotation.z = -Math.PI / 2
    outer.position.x = -r * 5
    const core = new THREE.Mesh(new THREE.CylinderGeometry(r * 0.55, r * 0.25, r * 4.5, 20, 1, true), plumeMaterial(0xffffff, 0xffd27a))
    core.rotation.z = -Math.PI / 2
    core.position.x = -r * 2.25
    pg.add(outer, core)
    pg.visible = false
    pg.userData.fx = true
    g.add(pg)
    plumes.push({ group: pg, outer, core, stage: si, engine: ei, vehicle: v.id, radius: r, bell: m })
    return len
  }
  /** Painel solar: asa de células com o braço. */
  const wing = (px: number, py: number, pz: number, w: number, l: number, axis: 'y' | 'z', deployed = true) => {
    const span = deployed ? l : l * 0.25
    const geo = axis === 'z' ? new THREE.BoxGeometry(w, 0.05, span) : new THREE.BoxGeometry(w, span, 0.05)
    const m = new THREE.Mesh(geo, MAT.solar)
    const off = span / 2 + 0.3
    const sgn = Math.sign(axis === 'z' ? pz : py) || 1
    m.position.set(px, axis === 'y' ? py + sgn * off : py, axis === 'z' ? pz + sgn * off : pz)
    g.add(m)
    const boom = new THREE.Mesh(new THREE.CylinderGeometry(0.04, 0.04, 0.6, 6), MAT.truss)
    if (axis === 'z') boom.rotation.x = Math.PI / 2
    boom.position.set(px, axis === 'y' ? py + sgn * 0.3 : py, axis === 'z' ? pz + sgn * 0.3 : pz)
    g.add(boom)
    maxR = Math.max(maxR, span + 0.5)
  }

  // Lançador: estágios de baixo para cima.
  stages.forEach((s, i) => {
    if (s.spacecraft) return
    cur = i
    const d = s.diameter ?? 1
    const len = s.length ?? d * 3
    const r = d / 2
    const engines = s.engines ?? []
    if (s.boosters) {
      const next = stages.slice(i + 1).find((x) => !x.spacecraft) ?? stages[i + 1]
      const cr = (next?.diameter ?? d) / 2
      const n = Math.max(2, Math.min(engines.length || 2, 6))
      const br = Math.min(r, cr * 0.7)
      for (let k = 0; k < n; k++) {
        const a = (k / n) * Math.PI * 2 + Math.PI / n
        const y = Math.cos(a) * (cr + br * 1.05)
        const z = Math.sin(a) * (cr + br * 1.05)
        cyl(br, br, len * 0.88, MAT.hull, x, y, z)
        cyl(br * 1.02, br * 1.02, len * 0.04, MAT.band, x + len * 0.3, y, z)
        const cone = new THREE.Mesh(new THREE.ConeGeometry(br, br * 2.8, 28), MAT.hull)
        cone.rotation.z = -Math.PI / 2
        cone.position.set(x + len * 0.88 + br * 1.4, y, z)
        g.add(cone)
        bell(i, Math.min(k, engines.length - 1), br * 0.7, x - br * 1.4, y, z)
      }
      return
    }
    // Saia dos motores e os sinos.
    if (engines.length > 0) {
      const n = Math.min(engines.length, 9)
      const er = n > 1 ? Math.min(r * 0.3, (r * 2.2) / n) : r * 0.42
      const exit = x - er * 2.4
      engines.slice(0, n).forEach((_, k) => {
        const a = (k / n) * Math.PI * 2
        const ring = n === 1 ? 0 : n >= 7 && k === 0 ? 0 : r * 0.6
        const kk = n >= 7 ? k - 1 : k
        const nn = n >= 7 ? n - 1 : n
        const aa = n >= 7 ? (kk / nn) * Math.PI * 2 : a
        bell(i, k, er, exit, ring ? Math.cos(aa) * ring : 0, ring ? Math.sin(aa) * ring : 0)
      })
    }
    const isET = v.kind === 'shuttle' && s.cryogenic && !engines.length
    cyl(r, r, len, isET ? MAT.tank : MAT.hull, x)
    if (isET) {
      // Ogiva do tanque externo.
      const nose = new THREE.Mesh(new THREE.ConeGeometry(r, r * 2.2, 32), MAT.tank)
      nose.rotation.z = -Math.PI / 2
      nose.position.set(x + len + r * 1.1, 0, 0)
      g.add(nose)
    } else {
      // Anel entre estágios e a faixa escura de cima (interestágio).
      cyl(r * 1.015, r * 1.015, Math.min(0.35, len * 0.05), MAT.band, x)
      cyl(r * 1.005, r * 1.005, Math.min(1.2, len * 0.12), MAT.darkHull, x + len - Math.min(1.2, len * 0.12))
    }
    x += len
  })

  const craft = stages.filter((s) => s.spacecraft)
  const fairing = (v.items ?? []).some((i) => i.id === 'fairing')
  // Daqui em diante, a nave (o primeiro estágio "spacecraft").
  cur = stages.findIndex((s) => s.spacecraft)
  if (cur < 0) cur = stages.length
  const coreR = Math.max(0.4, ...stages.filter((s) => !s.spacecraft && !s.boosters).map((s) => (s.diameter ?? 1) / 2))

  if (v.kind === 'debris') {
    // Estágio largado: só ele (sem a carga em cima); sem estágio, meia coifa caindo.
    if (!stages.length) {
      const half = new THREE.Mesh(new THREE.CylinderGeometry(1, 1, 3, 24, 1, true, 0, Math.PI), MAT.hull)
      half.material = MAT.hull
      half.rotation.z = -Math.PI / 2
      half.position.set(1.5, 0, 0)
      g.add(half)
      x = 3
    }
  } else if (v.kind === 'shuttle' || v.shape === 'winged') {
    // Orbitador: ao lado do tanque externo na subida; sozinho em órbita.
    const withTank = stages.some((s) => !s.spacecraft && s.cryogenic)
    const L = 37
    const base = withTank ? 2 : x
    const side = withTank ? coreR + 2.6 : 0
    const fus = new THREE.Mesh(new THREE.CylinderGeometry(2.5, 2.6, L * 0.72, 28), MAT.hull)
    fus.rotation.z = -Math.PI / 2
    fus.position.set(base + L * 0.36, 0, side)
    g.add(fus)
    const nose = new THREE.Mesh(new THREE.SphereGeometry(2.5, 24, 16, 0, Math.PI * 2, 0, Math.PI / 2), MAT.hull)
    nose.rotation.z = -Math.PI / 2
    nose.scale.set(1, 2.2, 1)
    nose.position.set(base + L * 0.72, 0, side)
    g.add(nose)
    // Asas em delta (barriga preta) e o leme.
    const wingShape = new THREE.Shape()
    wingShape.moveTo(0, 0)
    wingShape.lineTo(L * 0.42, 0)
    wingShape.lineTo(L * 0.05, 12)
    wingShape.lineTo(0, 12)
    for (const s of [-1, 1]) {
      const wgeo = new THREE.ExtrudeGeometry(wingShape, { depth: 0.6, bevelEnabled: false })
      const w = new THREE.Mesh(wgeo, MAT.orbiterBelly)
      w.rotation.x = s > 0 ? Math.PI / 2 : -Math.PI / 2
      w.position.set(base, s * 2.2, side + (s > 0 ? 1.8 : 1.2))
      g.add(w)
    }
    const fin = new THREE.Shape()
    fin.moveTo(0, 0)
    fin.lineTo(8, 0)
    fin.lineTo(1.5, 8)
    fin.lineTo(0, 8)
    const tail = new THREE.Mesh(new THREE.ExtrudeGeometry(fin, { depth: 0.4, bevelEnabled: false }), MAT.hull)
    tail.position.set(base - 1, 0, side - 2.4)
    tail.rotation.x = Math.PI
    g.add(tail)
    // Janelas da cabine e os motores principais.
    for (const k of [-1, 0, 1]) box(0.5, 0.5, 0.15, MAT.window, base + L * 0.72 - 1, k * 0.8, side - 2.3)
    craft[0]?.engines?.forEach((_, k) => bell(stages.indexOf(craft[0]), k, 0.9, base - 2, (k - 0.5) * 1.6, side - 0.6))
    if (!withTank) x = base + L
    maxR = Math.max(maxR, 12, side + 3)
  } else if (v.landing === 'legs' && !(v.heat_shield && v.kind !== 'capsule')) {
    // Módulo de pouso: estágio de descida octogonal dourado e pernas; em cima, o estágio
    // de subida (tripulado, como o LM) ou o convés com instrumentos (sonda).
    const descent = stages.find((s) => !s.spacecraft) ?? stages[0]
    const W = 4.2
    const dh = 1.7
    const oct = new THREE.Mesh(new THREE.CylinderGeometry(W / 2, W / 2, dh, 8), MAT.mli)
    oct.rotation.z = -Math.PI / 2
    oct.position.set(x + dh / 2, 0, 0)
    g.add(oct)
    descent?.engines?.forEach((_, k) => bell(stages.indexOf(descent), k, 0.75, x - 0.9))
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * Math.PI * 2 + Math.PI / 4
      const fy = Math.cos(a) * 4.3
      const fz = Math.sin(a) * 4.3
      const top = new THREE.Vector3(x + dh * 0.8, Math.cos(a) * 2, Math.sin(a) * 2)
      const foot = new THREE.Vector3(x - 1.6, fy, fz)
      const leg = new THREE.Mesh(new THREE.CylinderGeometry(0.07, 0.07, top.distanceTo(foot), 8), MAT.truss)
      leg.position.copy(top.clone().add(foot).multiplyScalar(0.5))
      leg.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), foot.clone().sub(top).normalize())
      g.add(leg)
      const pad = new THREE.Mesh(new THREE.CylinderGeometry(0.45, 0.45, 0.12, 16), MAT.mli)
      pad.rotation.z = -Math.PI / 2
      pad.position.copy(foot)
      g.add(pad)
    }
    x += dh
    const ascent = v.kind === 'capsule' ? craft[0] : undefined
    if (!ascent) {
      cyl(W / 2 - 0.2, W / 2 - 0.2, 0.25, MAT.radiator, x)
      cyl(0.08, 0.08, 1.6, MAT.truss, x + 0.25, 0.9, 0.9)
      const dish = new THREE.Mesh(new THREE.SphereGeometry(0.55, 20, 8, 0, Math.PI * 2, 0, Math.PI / 3.5), MAT.radiator)
      dish.rotation.z = Math.PI / 2
      dish.position.set(x + 1.9, 0.9, 0.9)
      g.add(dish)
      box(0.6, 0.7, 0.5, MAT.mli, x + 0.55, -0.9, 0.4)
      cyl(0.18, 0.18, 0.9, MAT.black, x + 0.25, -0.8, -1.0)
      x += 1.2
      maxR = Math.max(maxR, 4.8)
    }
    if (ascent) {
      const ab = new THREE.Mesh(new THREE.CylinderGeometry(1.7, 2.0, 2.4, 6), MAT.hull)
      ab.rotation.z = -Math.PI / 2
      ab.position.set(x + 1.2, 0, 0)
      g.add(ab)
      box(0.5, 0.7, 0.1, MAT.window, x + 1.6, 0.6, -1.75)
      box(0.5, 0.7, 0.1, MAT.window, x + 1.6, -0.6, -1.75)
      cyl(0.5, 0.5, 0.5, MAT.darkHull, x + 2.4)
      // A antena parabólica da subida.
      const dish = new THREE.Mesh(new THREE.SphereGeometry(0.5, 16, 8, 0, Math.PI * 2, 0, Math.PI / 4), MAT.radiator)
      dish.position.set(x + 2.0, 1.9, 0.8)
      g.add(dish)
      ascent.engines?.forEach((_, k) => bell(stages.indexOf(ascent), k, 0.35, x - 0.2))
      x += 2.9
      maxR = Math.max(maxR, 4.8)
    }
  } else if (v.kind === 'capsule' || v.shape === 'capsule') {
    const trunk = craft.find((s) => (s.engines?.length ?? 0) === 0 && craft.length > 1) ?? craft.find((s) => s.name.toLowerCase().includes('serviço'))
    const d = Math.max(2, craft[craft.length - 1]?.diameter ?? coreR * 2)
    const service = craft.length > 1 ? craft[0] : undefined
    if (service) {
      // Módulo de serviço: motor, radiadores e, quando há, painéis.
      const sl = service.length ?? d * 0.9
      service.engines?.slice(0, 1).forEach((_, k) => bell(stages.indexOf(service), k, d * 0.18, x - d * 0.43))
      cyl(d / 2, d / 2, sl, MAT.hull, x)
      for (let k = 0; k < 4; k++) {
        const a = (k / 4) * Math.PI * 2
        const rad = new THREE.Mesh(new THREE.CylinderGeometry(d / 2 + 0.01, d / 2 + 0.01, sl * 0.6, 24, 1, true, a, 0.6), MAT.radiator)
        rad.rotation.z = -Math.PI / 2
        rad.position.set(x + sl / 2, 0, 0)
        g.add(rad)
      }
      if (v.systems?.solar) for (const s of [-1, 1]) wing(x + sl * 0.5, 0, s * (d / 2), d * 0.8, d * 2.4, 'z', v.systems.solar.deployed)
      x += sl
    } else if (trunk) {
      cyl(d / 2, d / 2, trunk.length ?? d, MAT.darkHull, x)
      x += trunk.length ?? d
    }
    // Escudo, cápsula cônica com janelas e RCS, e a ponta com a porta (ou a torre).
    cyl(d / 2, d / 2, d * 0.07, MAT.shield, x)
    x += d * 0.07
    const capLen = d * 0.72
    const crewed = v.kind === 'capsule'
    cyl(d / 2, d * 0.24, capLen, MAT.hull, x, 0, 0, 40)
    if (crewed) for (const a of [0.6, -0.6]) {
      const w = box(d * 0.12, d * 0.08, 0.04, MAT.window, x + capLen * 0.45, Math.sin(a) * d * 0.36, -Math.cos(a) * d * 0.36)
      w.lookAt(new THREE.Vector3(x + capLen * 0.45, Math.sin(a) * d, -Math.cos(a) * d))
    }
    for (let k = 0; k < 4; k++) {
      const a = (k / 4) * Math.PI * 2 + Math.PI / 4
      box(d * 0.05, d * 0.05, d * 0.05, MAT.black, x + capLen * 0.2, Math.cos(a) * d * 0.44, Math.sin(a) * d * 0.44)
    }
    x += capLen
    if (!crewed) {
      cyl(d * 0.24, d * 0.2, d * 0.06, MAT.darkHull, x)
      x += d * 0.06
    } else if (v.les?.tower) {
      cyl(d * 0.06, d * 0.06, d * 1.4, MAT.darkHull, x)
      cyl(d * 0.14, d * 0.03, d * 0.5, MAT.hull, x + d * 1.4)
      bell(-1, -1, d * 0.06, x + d * 0.6)
    } else {
      // Anel e sonda de acoplamento.
      cyl(d * 0.24, d * 0.22, d * 0.1, MAT.darkHull, x)
      cyl(d * 0.03, d * 0.01, d * 0.25, MAT.truss, x + d * 0.1)
      x += d * 0.12
    }
    // Paraquedas abertos, com as cordas.
    for (const [k, c] of (v.chutes ?? []).entries()) {
      if (c.state !== 'deployed' && c.state !== 'reefed') continue
      const cr = d * (0.6 + Math.max(0, c.inflation) * 1.8)
      const dome = new THREE.Mesh(new THREE.SphereGeometry(cr, 32, 12, 0, Math.PI * 2, 0, Math.PI / 2.3), k % 2 ? MAT.chute : MAT.chuteWhite)
      dome.rotation.z = -Math.PI / 2
      const off = (k - ((v.chutes?.length ?? 1) - 1) / 2) * cr * 1.1
      const top = x + d * 2.5 + cr * 1.6
      dome.position.set(top, off, 0)
      g.add(dome)
      for (let l = 0; l < 6; l++) {
        const a = (l / 6) * Math.PI * 2
        const p1 = new THREE.Vector3(top, off + Math.cos(a) * cr * 0.95, Math.sin(a) * cr * 0.95)
        const p2 = new THREE.Vector3(x, 0, 0)
        const line = new THREE.Mesh(new THREE.CylinderGeometry(0.015, 0.015, p1.distanceTo(p2), 4), MAT.chuteWhite)
        line.position.copy(p1.clone().add(p2).multiplyScalar(0.5))
        line.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), p2.clone().sub(p1).normalize())
        g.add(line)
      }
      maxR = Math.max(maxR, cr + Math.abs(off))
    }
  } else if (v.kind === 'station') {
    // Módulos em fila, nós, a treliça com as asas e os radiadores.
    const mods = 5
    const ml = 6.5
    for (let k = 0; k < mods; k++) {
      cyl(2.1, 2.1, ml, MAT.hull, x + k * (ml + 0.6))
      cyl(1.2, 1.2, 0.6, MAT.darkHull, x + k * (ml + 0.6) + ml)
    }
    const len = mods * (ml + 0.6)
    const tx = x + len * 0.55
    // Treliça ao longo de Y, por cima dos módulos (−Z é "para cima" no corpo), com diagonais.
    const tl = 90
    const tz = -4.2
    box(1.4, tl, 1.4, MAT.truss, tx, 0, tz)
    for (let k = -tl / 2 + 1.5; k < tl / 2; k += 3) {
      const dgl = box(0.12, 4.2, 0.12, MAT.truss, tx, k, tz - 0.75)
      dgl.rotation.x = Math.PI / 4
    }
    for (const ys of [-1, 1]) {
      // Asas solares nas pontas e radiadores perto do meio.
      for (const xs of [-1, 1]) box(11, 30, 0.06, MAT.solar, tx + xs * 6.5, ys * 36, tz).material = MAT.solar
      const rad = box(0.08, 11, 9, MAT.radiator, tx, ys * 14, tz + 5.2)
      rad.material = MAT.radiator
    }
    x += len
    maxR = Math.max(maxR, 52)
  } else {
    // Satélite ou sonda (ou a coifa que ainda os cobre).
    const sat = craft[craft.length - 1]
    const d = Math.max(1, sat?.diameter ?? coreR * 2)
    const len = Math.max(1.2, (sat?.length ?? d) * 0.8)
    if (fairing) {
      const fl = len * 1.3
      cyl(coreR, coreR, fl, MAT.hull, x)
      const ogive = new THREE.Mesh(new THREE.SphereGeometry(coreR, 32, 16, 0, Math.PI * 2, 0, Math.PI / 2), MAT.hull)
      ogive.rotation.z = -Math.PI / 2
      ogive.scale.set(1, 2.2, 1)
      ogive.position.set(x + fl, 0, 0)
      g.add(ogive)
      x += fl + coreR * 2.2
    } else {
      if (sat?.engines?.length) sat.engines.slice(0, 1).forEach((_, k) => bell(stages.indexOf(sat), k, d * 0.12, x - d * 0.29))
      const bus = box(len, d * 0.85, d * 0.85, MAT.mli, x + len / 2)
      bus.castShadow = true
      // Antena de alto ganho e instrumentos na face de baixo.
      const dish = new THREE.Mesh(new THREE.SphereGeometry(d * 0.45, 24, 10, 0, Math.PI * 2, 0, Math.PI / 3.2), MAT.radiator)
      dish.rotation.x = Math.PI
      dish.position.set(x + len * 0.6, 0, d * 0.43 + d * 0.32)
      g.add(dish)
      const feed = new THREE.Mesh(new THREE.CylinderGeometry(0.03, 0.03, d * 0.35, 6), MAT.truss)
      feed.rotation.x = Math.PI / 2
      feed.position.set(x + len * 0.6, 0, d * 0.43 + d * 0.15)
      g.add(feed)
      cyl(d * 0.12, d * 0.12, len * 0.3, MAT.black, x + len, 0, d * 0.2)
      if (v.systems?.solar) for (const s of [-1, 1]) wing(x + len / 2, s * d * 0.43, 0, len * 0.85, d * 2.6, 'y', v.systems.solar.deployed)
      x += len
    }
  }
  // Centraliza ao longo do X e liga as sombras (menos nas plumas).
  const mid = x / 2
  g.children.forEach((c) => (c.position.x -= mid))
  g.traverse((o) => {
    if ((o as THREE.Mesh).isMesh && !o.parent?.userData.fx) o.castShadow = o.receiveShadow = true
  })
  return { group: g, plumes, length: Math.max(x, 2), radius: maxR }
}

/** Caixa das partes sólidas (sem as plumas), no referencial do mundo. */
export function solidBounds(root: THREE.Object3D): THREE.Box3 {
  root.updateMatrixWorld(true)
  const box = new THREE.Box3()
  const visit = (o: THREE.Object3D) => {
    if (o.userData.fx) return
    const m = o as THREE.Mesh
    if (m.isMesh && m.geometry) {
      m.geometry.computeBoundingBox()
      box.union(m.geometry.boundingBox!.clone().applyMatrix4(m.matrixWorld))
    }
    o.children.forEach(visit)
  }
  visit(root)
  return box
}

/** Liberar as geometrias e materiais clonados de um grupo. */
export function disposeGroup(o: THREE.Object3D) {
  o.traverse((c) => {
    const m = c as THREE.Mesh
    m.geometry?.dispose()
    const mat = m.material as THREE.Material | THREE.Material[] | undefined
    if (mat && !Array.isArray(mat) && (mat as THREE.ShaderMaterial).isShaderMaterial) mat.dispose()
  })
}
