// Hangar: montar foguetes com peças (tanques, motores, boosters, a carga) e
// missões (de onde e para que órbita), como no Kerbal Space Program. O
// projeto vira os arquivos TOML de sempre (missoes/veiculos e
// missoes/cenarios, veja missoes/LEIAME.md) e vai junto, num comentário, para
// poder ser reaberto no hangar.

import type { Vehicle } from '../types'

export const G0 = 9.80665

export type PropellantId = 'kerolox' | 'methalox' | 'hydrolox' | 'hypergolic' | 'solid'

/** Propelentes: densidade média da mistura (kg/m³) e a fração de estrutura do estágio. */
export const PROPELLANTS: Record<PropellantId, { name: string; density: number; structure: number; cryogenic: boolean }> = {
  kerolox: { name: 'Querosene e oxigênio', density: 1030, structure: 0.065, cryogenic: true },
  methalox: { name: 'Metano e oxigênio', density: 830, structure: 0.07, cryogenic: true },
  hydrolox: { name: 'Hidrogênio e oxigênio', density: 360, structure: 0.11, cryogenic: true },
  hypergolic: { name: 'Hipergólicos', density: 1190, structure: 0.08, cryogenic: false },
  solid: { name: 'Sólido', density: 1750, structure: 0.12, cryogenic: false },
}

export interface EnginePart {
  id: string
  name: string
  propellant: PropellantId
  /** Empuxo no vácuo (kN), impulsos específicos (s), massa (kg). */
  thrust: number
  ispVac: number
  ispSl: number
  mass: number
  minThrottle: number
  ignitions: number
  pc: number
  /** Para o estágio de baixo (bocal para o nível do mar) ou de vácuo. */
  vacuum: boolean
}

/** Motores do hangar: tipos genéricos, com números parecidos com os de motores de verdade. */
export const ENGINES: EnginePart[] = [
  { id: 'k25', name: 'Querosene 25 kN', propellant: 'kerolox', thrust: 25.8, ispVac: 311, ispSl: 303, mass: 35, minThrottle: 0.8, ignitions: 1, pc: 120, vacuum: false },
  { id: 'k25v', name: 'Querosene 25 kN de vácuo', propellant: 'kerolox', thrust: 25.8, ispVac: 343, ispSl: 250, mass: 40, minThrottle: 0.5, ignitions: 3, pc: 120, vacuum: true },
  { id: 'k900', name: 'Querosene 900 kN', propellant: 'kerolox', thrust: 914, ispVac: 311, ispSl: 282, mass: 470, minThrottle: 0.4, ignitions: 3, pc: 97, vacuum: false },
  { id: 'k980v', name: 'Querosene 980 kN de vácuo', propellant: 'kerolox', thrust: 981, ispVac: 348, ispSl: 200, mass: 550, minThrottle: 0.39, ignitions: 4, pc: 97, vacuum: true },
  { id: 'm2300', name: 'Metano 2.300 kN', propellant: 'methalox', thrust: 2450, ispVac: 350, ispSl: 327, mass: 1600, minThrottle: 0.4, ignitions: 3, pc: 300, vacuum: false },
  { id: 'm2500v', name: 'Metano 2.500 kN de vácuo', propellant: 'methalox', thrust: 2530, ispVac: 380, ispSl: 200, mass: 1800, minThrottle: 0.4, ignitions: 5, pc: 300, vacuum: true },
  { id: 'h1860', name: 'Hidrogênio 1.860 kN', propellant: 'hydrolox', thrust: 2279, ispVac: 452, ispSl: 366, mass: 3500, minThrottle: 0.67, ignitions: 1, pc: 206, vacuum: false },
  { id: 'h110v', name: 'Hidrogênio 110 kN de vácuo', propellant: 'hydrolox', thrust: 110, ispVac: 465, ispSl: 200, mass: 300, minThrottle: 0.2, ignitions: 15, pc: 44, vacuum: true },
  { id: 'n27v', name: 'Hipergólico 27 kN de vácuo', propellant: 'hypergolic', thrust: 27, ispVac: 316, ispSl: 250, mass: 120, minThrottle: 1, ignitions: 50, pc: 9, vacuum: true },
  { id: 'n45', name: 'Hipergólico 45 kN regulável (pouso)', propellant: 'hypergolic', thrust: 45, ispVac: 311, ispSl: 250, mass: 180, minThrottle: 0.1, ignitions: 20, pc: 7, vacuum: true },
  { id: 's100', name: 'Sólido 100 kN', propellant: 'solid', thrust: 100, ispVac: 280, ispSl: 250, mass: 0, minThrottle: 1, ignitions: 1, pc: 60, vacuum: false },
  { id: 's1000', name: 'Sólido 1.000 kN', propellant: 'solid', thrust: 1000, ispVac: 280, ispSl: 255, mass: 0, minThrottle: 1, ignitions: 1, pc: 70, vacuum: false },
  { id: 's5000', name: 'Sólido 5.000 kN', propellant: 'solid', thrust: 5000, ispVac: 285, ispSl: 260, mass: 0, minThrottle: 1, ignitions: 1, pc: 70, vacuum: false },
  { id: 's13000', name: 'Sólido 13.000 kN', propellant: 'solid', thrust: 13000, ispVac: 268, ispSl: 242, mass: 0, minThrottle: 1, ignitions: 1, pc: 63, vacuum: false },
]

export const engine = (id: string) => ENGINES.find((e) => e.id === id) ?? ENGINES[0]

export type PayloadId = 'satellite' | 'probe' | 'capsule' | 'mass'

export interface PayloadPart {
  id: PayloadId
  name: string
  description: string
  kind: 'satellite' | 'probe' | 'capsule'
  systems: string
  recovery?: string
  /** Massa padrão (kg) e se o usuário escolhe. */
  mass: number
  adjustable: boolean
  fairing: boolean
  crew?: boolean
}

export const PAYLOADS: PayloadPart[] = [
  { id: 'satellite', name: 'Satélite', description: 'Painéis, rádio, gravador e atitude; vai sob a coifa', kind: 'satellite', systems: 'amazonia', mass: 600, adjustable: true, fairing: true },
  { id: 'probe', name: 'Sonda interplanetária', description: 'Com motor próprio para correções e captura', kind: 'probe', systems: 'sonda_marte', mass: 1000, adjustable: true, fairing: true },
  { id: 'capsule', name: 'Cápsula tripulada', description: 'Escudo térmico, paraquedas, torre de escape e porta de acoplamento', kind: 'capsule', systems: 'medio_capsula', recovery: 'medio_capsula', mass: 9000, adjustable: false, fairing: false, crew: true },
  { id: 'mass', name: 'Carga de teste', description: 'Só massa, para experimentar o foguete', kind: 'satellite', systems: 'amazonia', mass: 2000, adjustable: true, fairing: true },
]

export const payload = (id: string) => PAYLOADS.find((p) => p.id === id) ?? PAYLOADS[0]

export interface StageDesign {
  /** Boosters laterais (só o primeiro da pilha): `count` boosters iguais. */
  boosters: boolean
  propellant: PropellantId
  diameter: number
  length: number
  engine: string
  /** Motores no estágio, ou quantos boosters. */
  count: number
}

export interface Design {
  name: string
  payload: PayloadId
  payloadMass: number
  crew: number
  /** De baixo para cima; boosters, se houver, ficam no índice 0. */
  stages: StageDesign[]
}

export interface Mission {
  mode: 'pad' | 'orbit'
  site: string
  tMinus: number
  pe: number
  ap: number
  inc: number
  north: boolean
  body: string
  calm: boolean
}

export const DEFAULT_DESIGN: Design = {
  name: 'Meu foguete',
  payload: 'satellite',
  payloadMass: 150,
  crew: 0,
  stages: [
    { boosters: false, propellant: 'kerolox', diameter: 1.2, length: 12, engine: 'k25', count: 9 },
    { boosters: false, propellant: 'kerolox', diameter: 1.2, length: 2.4, engine: 'k25v', count: 1 },
  ],
}

export const DEFAULT_MISSION: Mission = { mode: 'pad', site: 'Alcântara', tMinus: 120, pe: 300, ap: 300, inc: 5, north: true, body: 'earth', calm: true }

// ── Massas e desempenho ──

/** Propelente (kg) e massa seca (kg) de um estágio (ou de todos os boosters juntos). */
export function stageMasses(s: StageDesign): { prop: number; dry: number; thrust: number; ispVac: number; ispSl: number; engines: number } {
  const p = PROPELLANTS[s.propellant]
  const e = engine(s.engine)
  const tanks = s.boosters ? s.count : 1
  const volume = Math.PI * (s.diameter / 2) ** 2 * s.length * 0.92
  const prop = volume * p.density * tanks
  const engines = s.boosters ? s.count : s.count
  const dry = prop * p.structure + e.mass * engines
  return { prop, dry, thrust: e.thrust * engines, ispVac: e.ispVac, ispSl: e.ispSl, engines }
}

export interface StageStats {
  index: number
  name: string
  wet: number
  dry: number
  prop: number
  /** Massa no começo e no fim da queima deste estágio (com tudo o que vai em cima). */
  m0: number
  m1: number
  dv: number
  twr: number
  burn: number
}

export interface Stats {
  stages: StageStats[]
  payload: number
  liftoff: number
  dv: number
  liftoffTwr: number
  warnings: string[]
}

/** Δv (Tsiolkovsky), TWR e tempo de queima de cada estágio, com o que vai em cima. */
export function computeStats(d: Design): Stats {
  const pl = payloadStages(d)
  const plMass = pl.reduce((a, s) => a + s.dry + s.prop, 0) + (payload(d.payload).fairing ? fairingMass(d) : 0)
  const st = d.stages.map((s, i) => ({ s, i, ...stageMasses(s) }))
  const above = (i: number) => st.slice(i + 1).reduce((a, x) => a + x.dry + x.prop, 0) + plMass
  const out: StageStats[] = []
  const warnings: string[] = []
  const booster = st[0]?.s.boosters ? st[0] : null
  for (const x of st) {
    if (x.s.boosters) continue
    const isp = x.i === (booster ? 1 : 0) ? (x.ispSl + x.ispVac) / 2 : x.ispVac
    let m0 = x.dry + x.prop + above(x.i)
    let dv = 0
    let prop = x.prop
    let burn = 0
    // Boosters queimam junto com o primeiro estágio central até o primeiro dos dois acabar;
    // o que sobrar de um deles queima sozinho em seguida.
    if (booster && x.i === 1) {
      m0 += booster.dry + booster.prop
      const ispB = (booster.ispSl + booster.ispVac) / 2
      const mdotB = (booster.thrust * 1e3) / (ispB * G0)
      const mdotC = (x.thrust * 1e3) / (isp * G0)
      const t1 = Math.min(booster.prop / mdotB, prop / mdotC)
      const ispMix = ((booster.thrust + x.thrust) * 1e3) / ((mdotB + mdotC) * G0)
      let m1 = m0 - (mdotB + mdotC) * t1
      dv = ispMix * G0 * Math.log(m0 / m1)
      let burnB = t1
      // Os boosters continuam depois que o núcleo acabou.
      const leftB = booster.prop - mdotB * t1
      if (leftB > 1) {
        const m2 = m1 - leftB
        dv += ispB * G0 * Math.log(m1 / m2)
        burnB += leftB / mdotB
        m1 = m2
      }
      out.push({ index: 0, name: 'Boosters + 1º estágio', wet: booster.dry + booster.prop, dry: booster.dry, prop: booster.prop, m0, m1, dv, twr: ((booster.thrust + x.thrust) * 1e3) / (m0 * G0), burn: burnB })
      prop -= mdotC * t1
      m0 = m1 - booster.dry
      dv = 0
      burn = 0
      if (prop < 1) continue
    }
    const m1 = m0 - prop
    dv = isp * G0 * Math.log(m0 / m1)
    burn = (prop * isp * G0) / Math.max(1, x.thrust * 1e3)
    out.push({ index: x.i, name: `${x.i - (booster ? 1 : 0) + 1}º estágio`, wet: x.dry + x.prop, dry: x.dry, prop: x.prop, m0, m1, dv, twr: (x.thrust * 1e3) / (m0 * G0), burn })
  }
  // A nave por último (o motor próprio da carga).
  for (const s of pl) {
    if (!s.thrust || !s.prop) continue
    const m0 = s.dry + s.prop
    out.push({ index: -1, name: s.name, wet: m0, dry: s.dry, prop: s.prop, m0, m1: s.dry, dv: s.isp * G0 * Math.log(m0 / s.dry), twr: (s.thrust * 1e3) / (m0 * G0), burn: (s.prop * s.isp * G0) / (s.thrust * 1e3) })
  }
  const liftoff = st.reduce((a, x) => a + x.dry + x.prop, 0) + plMass
  // Na decolagem acendem o primeiro estágio (e os boosters), com o empuxo ao nível do mar.
  const lit = booster ? st.slice(0, 2) : st.slice(0, 1)
  const liftoffTwr = (lit.reduce((a, x) => a + x.thrust * (x.ispSl / x.ispVac), 0) * 1e3) / (liftoff * G0)
  if (!d.stages.length) warnings.push('Sem estágios: a carga não sai do chão.')
  if (liftoffTwr < 1.15 && d.stages.length) warnings.push('Empuxo na decolagem baixo demais (TWR < 1,15): o foguete não sobe ou desperdiça propelente.')
  if (liftoffTwr > 3.5) warnings.push('TWR alto na decolagem: muita carga g e pressão dinâmica.')
  out.forEach((s, k) => {
    if (k > 0 && s.index >= 0 && s.twr < 0.3) warnings.push(`${s.name}: TWR ${s.twr.toFixed(2)}, queima longa demais para a subida.`)
  })
  d.stages.forEach((s, i) => {
    if (s.boosters && i !== 0) warnings.push('Boosters só no primeiro estágio da pilha.')
    if (engine(s.engine).propellant !== s.propellant) warnings.push(`Estágio ${i + 1}: o motor não queima ${PROPELLANTS[s.propellant].name.toLowerCase()}.`)
    if (i > 0 && !s.boosters && s.diameter > d.stages[i - 1].diameter * 1.6 && !d.stages[i - 1].boosters) warnings.push(`Estágio ${i + 1} bem mais largo que o de baixo.`)
  })
  const dv = out.reduce((a, s) => a + s.dv, 0)
  return { stages: out, payload: plMass, liftoff, dv, liftoffTwr, warnings }
}

/** Coifa: proporcional à carga (kg). */
export function fairingMass(d: Design): number {
  return Math.round(Math.max(50, Math.min(2500, d.payloadMass * 0.12)))
}

interface PlStage {
  name: string
  dry: number
  prop: number
  thrust: number
  isp: number
  engine?: { name: string; thrust: number; isp: number; ign: number; pc: number }
  diameter: number
  length: number
}

/** Os estágios da nave (no alto da pilha). */
export function payloadStages(d: Design): PlStage[] {
  const p = payload(d.payload)
  const top = d.stages[d.stages.length - 1]
  const dia = Math.min(top?.diameter ?? 2, 4)
  switch (p.id) {
    case 'capsule':
      return [
        { name: 'Tronco', dry: 1300, prop: 0, thrust: 0, isp: 0, diameter: 3.7, length: 3.7 },
        { name: 'Cápsula', dry: 9000, prop: 2400, thrust: 3.2, isp: 300, engine: { name: 'Propulsores de manobra 8×400 N', thrust: 3.2, isp: 300, ign: 100, pc: 10 }, diameter: 3.7, length: 3 },
      ]
    case 'probe': {
      const dry = d.payloadMass
      const prop = Math.round(d.payloadMass * 1.1)
      return [{ name: 'Sonda', dry, prop, thrust: 1.02, isp: 318, engine: { name: 'Motores principais 6×170 N', thrust: 1.02, isp: 318, ign: 40, pc: 9 }, diameter: Math.min(dia, 3), length: 3 }]
    }
    default:
      return [{ name: p.id === 'mass' ? 'Carga de teste' : 'Satélite', dry: d.payloadMass, prop: 60, thrust: 0.004, isp: 220, engine: { name: 'Propulsores de hidrazina 4×1 N', thrust: 0.004, isp: 220, ign: 500, pc: 15 }, diameter: Math.min(dia, 2.5), length: 2.5 }]
  }
}

// ── Arquivos ──

const num = (x: number, d = 1) => String(Math.round(x * 10 ** d) / 10 ** d)
const q = (s: string) => JSON.stringify(s)

/** Id de arquivo a partir do nome (letras, números, - e _). */
export function fileId(name: string): string {
  return (
    name
      .normalize('NFD')
      .replace(/[̀-ͯ]/g, '')
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-|-$/g, '')
      .slice(0, 40) || 'foguete'
  )
}

/** Índice (na pilha do arquivo) do estágio em que o guiamento fecha a malha: o último do lançador. */
function closedLoopStage(d: Design): number {
  return Math.max(0, d.stages.length - 1)
}

/** O veículo em TOML (missoes/veiculos), com o projeto num comentário para reabrir. */
export function vehicleToml(d: Design, m: Mission): string {
  const p = payload(d.payload)
  const lines: string[] = []
  lines.push(`# Montado no hangar do Jaci.`)
  lines.push(`# hangar: ${JSON.stringify({ design: d, mission: m })}`)
  lines.push(`name = ${q(d.name)}`)
  lines.push(`description = ${q(`${d.stages.length} estágio(s) e ${p.name.toLowerCase()}, montado no hangar`)}`)
  lines.push(`kind = ${q(p.kind)}`)
  lines.push(`systems_from = ${q(p.systems)}`)
  if (p.recovery) lines.push(`recovery_from = ${q(p.recovery)}`)
  if (p.crew && d.crew > 0) lines.push(`crew = ${d.crew}`)
  lines.push(`fairing_kg = ${p.fairing ? fairingMass(d) : 0}`)
  lines.push('')
  lines.push('[ascent]')
  lines.push(`target_pe_km = ${num(m.pe, 0)}`)
  lines.push(`target_ap_km = ${num(Math.max(m.ap, m.pe), 0)}`)
  lines.push(`inclination_deg = ${num(m.inc, 2)}`)
  lines.push(`north = ${m.north}`)
  lines.push(`closed_loop_stage = ${closedLoopStage(d)}`)
  d.stages.forEach((s, i) => {
    const ms = stageMasses(s)
    const e = engine(s.engine)
    const solid = s.propellant === 'solid'
    lines.push('')
    lines.push('[[stages]]')
    lines.push(`name = ${q(s.boosters ? 'Boosters' : `${i + 1 - (d.stages[0].boosters ? 1 : 0)}º estágio`)}`)
    lines.push(`dry_kg = ${num(ms.dry, 0)}`)
    lines.push(`propellant_kg = ${num(ms.prop, 0)}`)
    lines.push(`diameter_m = ${num(s.diameter, 2)}`)
    lines.push(`length_m = ${num(s.length, 2)}`)
    if (PROPELLANTS[s.propellant].cryogenic) lines.push('cryogenic = true')
    if (s.boosters) lines.push('boosters = true')
    lines.push('[[stages.engines]]')
    lines.push(`name = ${q(s.boosters ? `Booster (${e.name})` : e.name)}`)
    if (ms.engines > 1) lines.push(`count = ${ms.engines}`)
    lines.push(`thrust_kn = ${num(e.thrust, 1)}`)
    lines.push(`isp_vac = ${e.ispVac}`)
    lines.push(`isp_sl = ${e.ispSl}`)
    if (solid) lines.push('solid = true')
    else {
      lines.push(`min_throttle = ${e.minThrottle}`)
      lines.push(`ignitions = ${e.ignitions}`)
    }
    lines.push(`pc_bar = ${e.pc}`)
  })
  for (const s of payloadStages(d)) {
    lines.push('')
    lines.push('[[stages]]')
    lines.push(`name = ${q(s.name)}`)
    lines.push(`dry_kg = ${num(s.dry, 0)}`)
    lines.push(`propellant_kg = ${num(s.prop, 0)}`)
    lines.push(`diameter_m = ${num(s.diameter, 2)}`)
    lines.push(`length_m = ${num(s.length, 2)}`)
    lines.push('spacecraft = true')
    if (s.engine) {
      lines.push('[[stages.engines]]')
      lines.push(`name = ${q(s.engine.name)}`)
      lines.push(`thrust_kn = ${s.engine.thrust}`)
      lines.push(`isp_vac = ${s.engine.isp}`)
      lines.push(`isp_sl = ${Math.round(s.engine.isp * 0.8)}`)
      lines.push('min_throttle = 1')
      lines.push(`ignitions = ${s.engine.ign}`)
      lines.push(`pc_bar = ${s.engine.pc}`)
    }
  }
  return lines.join('\n') + '\n'
}

/** A missão em TOML (missoes/cenarios): na plataforma ou já em órbita. */
export function scenarioToml(d: Design, m: Mission, vehicleId: string): string {
  const lines = [`# Missão montada no hangar do Jaci.`, `name = ${q(`${d.name}: ${m.mode === 'pad' ? `lançamento de ${m.site}` : 'em órbita'}`)}`]
  lines.push(`description = ${q(m.mode === 'pad' ? `${d.name} em ${m.site} a T-${Math.round(m.tMinus)} s para ${Math.round(m.pe)} × ${Math.round(Math.max(m.ap, m.pe))} km, ${m.inc}°` : `${d.name} em ${Math.round(m.pe)} × ${Math.round(Math.max(m.ap, m.pe))} km`)}`)
  lines.push('')
  lines.push('[[vehicles]]')
  lines.push(`vehicle = ${q(vehicleId)}`)
  lines.push(`name = ${q(d.name)}`)
  if (m.mode === 'pad') {
    lines.push('place = "pad"')
    lines.push(`site = ${q(m.site)}`)
    lines.push(`t_minus = ${Math.round(m.tMinus)}`)
  } else {
    lines.push('place = "orbit"')
    if (m.body !== 'earth') lines.push(`body = ${q(m.body)}`)
    lines.push(`pe_km = ${num(m.pe, 0)}`)
    lines.push(`ap_km = ${num(Math.max(m.ap, m.pe), 0)}`)
    lines.push(`inc_deg = ${num(m.inc, 2)}`)
  }
  if (m.calm && m.mode === 'pad') {
    lines.push('')
    lines.push('[weather]')
    lines.push('surface_wind = 3')
    lines.push('lightning_km = 150')
    lines.push('ceiling_m = 5000')
    lines.push('rain = false')
    lines.push('evolve = false')
  }
  return lines.join('\n') + '\n'
}

/** O projeto guardado no comentário de um arquivo de veículo do hangar. */
export function parseHangar(text: string): { design: Design; mission: Mission } | null {
  const m = text.match(/^# hangar: (.*)$/m)
  if (!m) return null
  try {
    const o = JSON.parse(m[1])
    return { design: { ...DEFAULT_DESIGN, ...o.design }, mission: { ...DEFAULT_MISSION, ...o.mission } }
  } catch {
    return null
  }
}

/** Um veículo "de mentira" do projeto, para a vista 3D. */
export function previewVehicle(d: Design): Vehicle {
  const p = payload(d.payload)
  const stages = [
    ...d.stages.map((s, i) => {
      const ms = stageMasses(s)
      return {
        name: `${i}`,
        prop: ms.prop,
        capacity: ms.prop,
        dry: ms.dry,
        diameter: s.diameter,
        length: s.length,
        boosters: s.boosters,
        cryogenic: PROPELLANTS[s.propellant].cryogenic,
        engines: Array.from({ length: ms.engines }, () => ({ name: '', state: 'off' as const, thrust: 0, rated: engine(s.engine).thrust, pc: 0, throttle: 1, solid: s.propellant === 'solid', ignitions: 0, burn: 0 })),
      }
    }),
    ...payloadStages(d).map((s) => ({ name: s.name, prop: s.prop, capacity: s.prop, dry: s.dry, diameter: s.diameter, length: s.length, spacecraft: true, engines: s.engine ? [{ name: '', state: 'off' as const, thrust: 0, rated: s.engine.thrust, pc: 0, throttle: 1, solid: false, ignitions: 0, burn: 0 }] : [] })),
  ]
  return {
    id: -1,
    name: d.name,
    kind: p.kind,
    kind_name: '',
    phase: 'prelaunch',
    phase_name: '',
    central: 'earth',
    crew: d.crew,
    lat: 0,
    lon: 0,
    alt: 0,
    speed: 0,
    stages,
    items: p.fairing ? [{ id: 'fairing', name: 'Coifa' }] : [],
    shape: p.kind === 'capsule' ? 'capsule' : 'box',
    les: p.id === 'capsule' ? { available: true, firing: false, tower: true } : null,
    // Em pé na plataforma: o eixo da nave (X) ao longo da vertical local.
    r: [6_378_137, 0, 0],
    v: [0, 0, 0],
    q: [1, 0, 0, 0],
  }
}
