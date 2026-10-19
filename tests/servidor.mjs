// Teste de ponta a ponta: sobe o servidor do Jaci e conversa com ele pelo
// WebSocket, como a interface. Uso (depois de `cargo build --release`):
//
//     node tests/servidor.mjs
//
// JACI_BIN aponta outro binário; ONLY=texto roda só os cenários com o texto no nome.

import { spawn } from 'node:child_process'
import { mkdtempSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const BIN = process.env.JACI_BIN ?? new URL('../target/release/jaci', import.meta.url).pathname
const ONLY = process.env.ONLY
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
let port = 28800

async function server(scenario) {
  const p = port++
  const states = mkdtempSync(join(tmpdir(), 'jaci-teste-'))
  const proc = spawn(BIN, ['--addr', `127.0.0.1:${p}`, '--scenario', scenario, '--start', '2026-10-19 15:00:00', '--states-dir', states, '--web-dir', '/nao-existe'], { stdio: ['ignore', 'ignore', 'pipe'] })
  let log = ''
  proc.stderr.on('data', (d) => (log += d))
  for (let i = 0; i < 50; i++) {
    await sleep(100)
    try {
      const ws = await connect(p)
      return { ws, port: p, close: () => (ws.close(), proc.kill(), rmSync(states, { recursive: true, force: true })), log: () => log }
    } catch {}
  }
  proc.kill()
  throw new Error(`o servidor não subiu:\n${log}`)
}

function connect(p) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${p}/ws`)
    const c = { ws, snap: null, hello: null, events: [], waiters: new Map(), req: 1 }
    ws.onerror = reject
    ws.onmessage = (m) => {
      const d = JSON.parse(m.data)
      if (d.type === 'hello') {
        c.hello = d
        c.events.push(...d.events)
        resolve(c)
      }
      if (d.type === 'snapshot') {
        c.snap = d
        c.events.push(...d.events)
      }
      if (d.req != null && c.waiters.has(d.req)) {
        c.waiters.get(d.req)(d)
        c.waiters.delete(d.req)
      }
    }
    c.request = (msg) =>
      new Promise((res, rej) => {
        const req = c.req++
        c.waiters.set(req, res)
        ws.send(JSON.stringify({ ...msg, req }))
        setTimeout(() => rej(new Error(`sem resposta: ${JSON.stringify(msg)}`)), 60000)
      })
    c.close = () => ws.close()
  })
}

async function until(c, what, cond, timeoutMs = 60000) {
  const t0 = Date.now()
  while (Date.now() - t0 < timeoutMs) {
    if (c.snap && cond(c.snap)) return c.snap
    await sleep(100)
  }
  throw new Error(`esperando ${what}; últimos eventos:\n${c.events.slice(-15).map((e) => e.text).join('\n')}`)
}

function assert(cond, msg) {
  if (!cond) throw new Error(msg)
}

const cases = [
  [
    'sondagem: contagem, voo e amerissagem',
    async () => {
      const s = await server('sondagem')
      try {
        const c = s.ws
        await until(c, 'instantâneo', (x) => x.vehicles.length > 0)
        const vid = c.snap.launch.vehicle
        // Perigoso sem armar é recusado.
        const r0 = await c.request({ type: 'command', vehicle: vid, cmd: { id: 'count_scrub' } })
        assert(!r0.ok, 'comando perigoso passou sem armar')
        await c.request({ type: 'sim', action: 'weather', surface_wind: 3, lightning_km: 120, ceiling_m: 5000, evolve: false })
        const r1 = await c.request({ type: 'command', vehicle: vid, cmd: { id: 'count_start' } })
        assert(r1.ok, `contagem: ${r1.error}`)
        await until(c, 'espera em T-10', (x) => x.launch.state === 'hold')
        await c.request({ type: 'command', vehicle: vid, cmd: { id: 'poll' } })
        await until(c, 'todos GO', (x) => x.launch.all_go)
        await c.request({ type: 'command', vehicle: vid, cmd: { id: 'count_start' } })
        await c.request({ type: 'sim', action: 'warp', value: 200 })
        await until(c, 'decolagem', (x) => x.vehicles.find((v) => v.id === vid).phase !== 'prelaunch', 30000)
        await c.request({ type: 'sim', action: 'warp_stop', value: false })
        await c.request({ type: 'sim', action: 'warp', value: 200 })
        const end = await until(c, 'amerissagem', (x) => ['splashdown', 'destroyed', 'landed'].includes(x.vehicles.find((v) => v.id === vid).phase), 90000)
        const v = end.vehicles.find((v) => v.id === vid)
        assert(v.phase === 'splashdown', `terminou em ${v.phase}`)
        const texts = c.events.map((e) => e.text).join('\n')
        for (const must of ['Decolagem', 'Separação do estágio 1', 'Ignição do estágio 2', 'Início da microgravidade', 'Apogeu', 'Amerissagem']) {
          assert(texts.includes(must), `faltou o evento "${must}"`)
        }
      } finally {
        s.close()
      }
    },
  ],
  [
    'retorno: plano de saída de órbita aplicado',
    async () => {
      const s = await server('retorno')
      try {
        const c = s.ws
        await until(c, 'instantâneo', (x) => x.vehicles.length > 0)
        const vid = c.snap.vehicles[0].id
        await c.request({ type: 'command', vehicle: vid, cmd: { id: 'landing_target', args: { lat: 29.3, lon: -79.8 } } })
        const p = await c.request({ type: 'plan', vehicle: vid, kind: 'deorbit', params: { pe: 40 } })
        assert(p.ok && p.plan.nodes.length === 1, `plano: ${p.error}`)
        assert(p.plan.total_dv > 60 && p.plan.total_dv < 200, `Δv ${p.plan.total_dv}`)
        await c.request({ type: 'plan_apply', vehicle: vid, nodes: p.plan.nodes, landing: p.plan.landing })
        await until(c, 'queima na fila', (x) => (x.vehicles[0].nodes ?? []).length === 1)
        const h = await c.request({ type: 'plan', vehicle: vid, kind: 'hohmann', params: { alt: 600 } })
        assert(h.ok && h.plan.nodes.length === 2, 'Hohmann')
        const pr = await c.request({ type: 'predict', vehicle: vid, span: 6000 })
        assert(pr.points.length > 100, 'previsão vazia')
      } finally {
        s.close()
      }
    },
  ],
  [
    'estação: salvar, carregar e falhas',
    async () => {
      const s = await server('estacao')
      try {
        const c = s.ws
        await until(c, 'instantâneo', (x) => x.vehicles.length > 1)
        const st = c.snap.vehicles.find((v) => v.kind === 'station')
        const f = await c.request({ type: 'sim', action: 'fault', vehicle: st.id, fault: 'cabin_leak', arg: '4' })
        assert(f.ok, `falha: ${f.error}`)
        await c.request({ type: 'sim', action: 'warp', value: 1000 })
        await until(c, 'pressão caindo', (x) => (x.vehicles.find((v) => v.id === st.id).tm['ecl.pressure'] ?? 101) < 99, 30000)
        const mod = c.snap.vehicles.find((v) => v.id === st.id).systems.eclss.modules[3].name
        const h = await c.request({ type: 'command', vehicle: st.id, cmd: { id: 'hatch', args: { module: mod, closed: 'on' } } })
        assert(h.ok, `escotilha: ${h.error}`)
        const sv = await c.request({ type: 'sim', action: 'save', value: 'teste' })
        assert(sv.ok, `salvar: ${sv.error}`)
        const list = await c.request({ type: 'sim', action: 'saves' })
        assert(list.list.includes('teste'), 'estado salvo não aparece')
        const ld = await c.request({ type: 'sim', action: 'load', value: 'teste' })
        assert(ld.type === 'reload', `carregar: ${ld.error}`)
        const p = await c.request({ type: 'passes', vehicle: st.id, span: 43200 })
        assert(p.passes.length > 3, 'poucas passagens')
      } finally {
        s.close()
      }
    },
  ],
  [
    'marte: porkchop',
    async () => {
      const s = await server('marte')
      try {
        const c = s.ws
        await until(c, 'instantâneo', (x) => x.vehicles.length > 0)
        const pc = await c.request({ type: 'porkchop', from: 'earth', to: 'mars', start: '2026-09-01 00:00:00', span_days: 180, tof_min: 150, tof_max: 350, n: 25 })
        const best = pc.data.best
        assert(best && best[2] > 5 && best[2] < 25, `C3 ${best?.[2]}`)
      } finally {
        s.close()
      }
    },
  ],
]

let failed = 0
for (const [name, fn] of cases) {
  if (ONLY && !name.includes(ONLY)) continue
  const t0 = Date.now()
  try {
    await fn()
    console.log(`  ok   ${name} (${((Date.now() - t0) / 1000).toFixed(1)} s)`)
  } catch (e) {
    failed++
    console.log(`  FALHOU ${name}: ${e.message}`)
  }
}
console.log(failed ? `${failed} cenário(s) falharam` : 'Servidor: todos os cenários ok')
process.exit(failed ? 1 : 0)
