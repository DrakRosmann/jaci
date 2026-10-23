// Formato das mensagens do servidor (veja server/src/snapshot.rs).

export type Level = 'info' | 'good' | 'warn' | 'alarm' | 'chat'

export interface EventMsg {
  t: number
  vehicle: number | null
  level: Level
  text: string
  /** Conversa nos loops: quem falou e em qual loop. */
  from?: string
  channel?: string
}

/** Posição da sala de controle e o que ela comanda (vem do servidor). */
export interface Role {
  id: string
  name: string
  description: string
  groups: string[]
  launch: string[]
  sim: string[]
}

export interface Room {
  on: boolean
  members: { name: string; position: string }[]
}

export interface TmDef {
  id: string
  name: string
  unit: string
  group: string
  yellow: [number, number] | null
  red: [number, number] | null
}

export interface ArgDef {
  name: string
  label: string
  kind: 'number' | 'bool' | 'enum' | 'text' | 'vehicle' | 'utc'
  unit?: string
  options?: [string, string][]
  optional: boolean
}

export interface CmdDef {
  id: string
  name: string
  group: string
  description: string
  args: ArgDef[]
  hazardous: boolean
  ground: boolean
  kinds: string[]
}

export interface Station {
  id: string
  name: string
  lat: number
  lon: number
  alt: number
  dish_m: number
  tsys: number
  min_el: number
  deep_space: boolean
  enabled: boolean
}

export interface Hello {
  type: 'hello'
  version: string
  t: number
  scenarios: { id: string; name: string; description: string }[]
  catalog: { id: string; name: string; kind: string; description: string }[]
  commands: CmdDef[]
  faults: { id: string; name: string; arg: string }[]
  stations: Station[]
  sites: { name: string; lat: number; lon: number; alt: number }[]
  bodies: { id: string; name: string; radius: number; parent: string | null; soi: number | null }[]
  events: EventMsg[]
  dictionary: Record<string, TmDef[]>
  roles: Role[]
  /** Na reprodução: o estado, as marcas da linha do tempo e a hora de alguns quadros. */
  replay?: ReplayInfo & { markers: ReplayMarker[]; times: [number, number][] }
}

export interface Engine {
  name: string
  state: 'off' | 'starting' | 'running' | 'stopping' | 'failed'
  thrust: number
  rated: number
  pc: number
  throttle: number
  solid: boolean
  ignitions: number
  burn: number
}

export interface Stage {
  name: string
  prop?: number
  capacity?: number
  dry?: number
  boosters?: boolean
  spacecraft?: boolean
  cryogenic?: boolean
  leak?: boolean
  engines?: Engine[]
}

export interface Node {
  id: number
  label: string
  t: number
  dv: [number, number, number]
  dv_total: number
  executing: boolean
  done: boolean
  remaining: number
  rcs: boolean
  burn: number | null
}

export interface Link {
  station: string
  elevation_deg: number
  azimuth_deg: number
  range_m: number
  margin_db: number
  rate_bps: number
  owlt: number
  via_hga: boolean
  via_relay: boolean
}

export interface Systems {
  loads: { id: string; name: string; watts: number; on: boolean; essential: boolean }[]
  solar: { deployed: boolean; tracking: boolean; stuck: boolean; degraded: number; area: number } | null
  fuel_cell: { on: boolean; reactant: number; max: number } | null
  battery_wh: number
  brownout: boolean
  thermal: { id: string; name: string; temp: number; heater: boolean; heater_w: number; auto: boolean; failed: boolean; min: number; max: number }[]
  radiator: boolean
  has_radiator: boolean
  adcs: {
    mode: string
    mode_name: string
    target: number | null
    wheels: { h: [number, number, number]; max: number; failed: [boolean, boolean, boolean] } | null
    rcs_prop: number
    desat: boolean
    star_tracker: boolean
    rate: number
  }
  comms: {
    tx_on: boolean
    auto_rate: boolean
    rate: number
    rates: number[]
    dumping: boolean
    relay: boolean
    hga: boolean
    hga_deployed: boolean
    stored: number
    storage: number
    gen: number
    blackout: boolean
    failed: boolean
    link: Link | null
  }
  eclss: {
    crew: number
    modules: { name: string; volume: number; isolated: boolean; leak: boolean; pressure: number }[]
    o2_gen: boolean
    scrubber: boolean
    auto_press: boolean
    has_o2_gen: boolean
    lioh: boolean
  } | null
  instruments: { id: string; name: string; watts: number; mbps: number; on: boolean }[]
}

export interface Iip {
  lat: number
  lon: number
  t: number
  cross_km: number
  inside: boolean
}

export interface Vehicle {
  id: number
  name: string
  kind: string
  kind_name: string
  def?: string
  phase: string
  phase_name: string
  central: string
  parent?: number | null
  attached_to?: number | null
  crew: number
  met?: number | null
  lat: number
  lon: number
  alt: number
  speed: number
  r?: [number, number, number]
  v?: [number, number, number]
  q?: [number, number, number, number]
  pe?: number | null
  ap?: number | null
  inc?: number
  ecc?: number
  tm?: Record<string, number | null>
  stages?: Stage[]
  items?: { id: string; name: string }[]
  chutes?: { name: string; state: string; inflation: number; alt: number }[]
  les?: { available: boolean; firing: boolean; tower: boolean } | null
  heat_shield?: { ablator: number; temp: number; flux: number; peak: number } | null
  landing?: string
  gear_down?: boolean
  ports?: { name: string; occupant: number | null }[]
  nodes?: Node[]
  guidance?: {
    program: string
    closed_loop: boolean
    t_go: number
    bank: number
    landing_target: [number, number] | null
    dock_target: number | null
    inserted: boolean
    ballistic: boolean
    auto_stage: boolean
    auto_chutes: boolean
    kick_wait: boolean
    ascent: { pe: number; ap: number; inc: number } | null
    azimuth: number | null
  }
  fts?: { installed: boolean; armed: boolean; autonomous: boolean }
  aborts?: [string, boolean][]
  iip?: Iip | null
  safe_mode?: boolean
  faults?: string[]
  site?: { name: string; lat: number; lon: number; alt: number } | null
  systems?: Systems
  packets?: number
  age?: number
  central_name?: string
  central_radius?: number
  central_rotation?: number
  ksp?: KspVessel
  /** Satélite real: os elementos NORAD em uso. */
  tle?: { norad: number; line1: string; line2: string; epoch: number | null; age: number }
}

/** Nave do Kerbal Space Program, vista pelo kOS (server/src/ksp.rs). */
export interface KspVessel {
  uid: string
  situation: string
  mode: 'manual' | 'ascent' | 'coast' | 'node' | string
  steer: string
  target: string
  sas: boolean
  rcs: boolean
  gear: boolean
  lights: boolean
  brakes: boolean
  stage: number | null
  warp: number | null
  age: number | null
  packets: number
  ut: number
  body: { name: string; radius: number; mu: number; atm: number; rotation: number }
  resources: { name: string; amount: number | null; capacity: number | null }[]
  countdown: { t_minus: number; apo: number; heading: number } | null
}

export interface KspStatus {
  dir: string
  scripts: string
  kos: boolean
  module_manager: boolean
  patch: boolean
  vessels: number
  live: number
}

export interface SeqItem {
  at: number
  name: string
  action: string
  done: boolean
}

export interface Launch {
  vehicle: number | null
  state: 'idle' | 'counting' | 'hold' | 'scrubbed' | 'launched'
  t_minus: number
  clock: string
  hold_reason: string
  sequence: SeqItem[]
  positions: { id: string; name: string; go: boolean; manual: boolean | null; reason: string | null; polled: boolean | null; awaiting?: boolean }[]
  criteria: { id: string; name: string; position: string; ok: boolean; value: string; limit: string }[]
  window: [number, number] | null
  loading: boolean
  all_go: boolean
  range_fouled: boolean
  auto_sequence: boolean
}

export interface Weather {
  surface_wind: number
  wind_from: number
  jet_wind: number
  lightning_km: number
  ceiling_m: number
  rain: boolean
  temperature_c: number
  evolve: boolean
}

export interface Alarm {
  id: number
  t: number
  vehicle: number
  point: string
  name: string
  level: 'yellow' | 'red' | 'nominal'
  value: number | null
  unit: string
  acked: boolean
  active: boolean
}

export interface CmdRecord {
  id: number
  vehicle: number
  text: string
  sent: number
  exec_at: number
  status: 'queued' | 'sent' | 'executed' | 'failed'
  message: string
}

export interface Snapshot {
  type: 'snapshot'
  t: number
  utc: string
  gmst: number
  warp: number
  paused: boolean
  warp_stop: boolean
  scenario: string
  mission: string
  vehicles: Vehicle[]
  launch: Launch
  weather: Weather
  alarms: Alarm[]
  commands: CmdRecord[]
  events: EventMsg[]
  corridor: { lat: number; lon: number; azimuth: number; half_width_km: number } | null
  bodies: { id: string; r: [number, number, number]; rel: [number, number, number] }[]
  sun: [number, number, number]
  stations_live: { id: string; enabled: boolean }[]
  ksp: KspStatus | null
  /** Gravação em andamento. */
  recording?: RecordingStatus | null
  /** Presente quando o servidor está reproduzindo uma gravação. */
  replay?: ReplayInfo | null
  room?: Room
}

export interface RecordingStatus {
  name: string
  frames: number
  /** Tempo de simulação coberto (s). */
  span: number
  bytes: number
}

export interface ReplayInfo {
  name: string
  frames: number
  index: number
  playing: boolean
  speed: number
  t0: number
  t1: number
  t: number
}

/** Marca na linha do tempo: quadro, nível e texto do evento. */
export type ReplayMarker = [number, Level, string]

export interface RecordingEntry {
  name: string
  bytes: number
  modified: string | null
  created: string | null
  mission: string | null
  scenario: string | null
}

export interface PlanNode {
  id: number
  label: string
  t: number
  dv_rtn: { x: number; y: number; z: number }
}

export interface Plan {
  nodes: PlanNode[]
  total_dv: number
  summary: string
  landing?: [number, number]
}

export interface Pass {
  station: string
  aos: number
  los: number
  max_el: number
  t_max: number
}
