// Teste de ponta a ponta: sobe o servidor do Jaci e conversa com ele pelo
// WebSocket, como a interface. Uso (depois de `cargo build --release`):
//
//     node tests/servidor.mjs
//
// JACI_BIN aponta outro binário; ONLY=texto roda só os cenários com o texto no nome.

import { spawn } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync, appendFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const BIN = process.env.JACI_BIN ?? new URL('../target/release/jaci', import.meta.url).pathname
const ONLY = process.env.ONLY
const sleep = (ms) => new Promise((r) => setTimeout(r, ms))
let port = 28800

async function server(scenario, extra = []) {
  const p = port++
  const states = mkdtempSync(join(tmpdir(), 'jaci-teste-'))
  const start = extra.includes('--start') ? [] : ['--start', '2026-10-19 15:00:00']
  const proc = spawn(BIN, ['--addr', `127.0.0.1:${p}`, '--scenario', scenario, ...start, '--states-dir', states, '--web-dir', '/nao-existe', ...extra], { stdio: ['ignore', 'ignore', 'pipe'] })
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
      if (d.type === 'rewind') c.rewind = d
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

/** Um kOS de mentira: escreve a telemetria como o jaci.ks e responde aos comandos. */
function fakeKos(dir, uid) {
  const tm = join(dir, `tm-${uid}.txt`)
  const cmd = join(dir, `cmd-${uid}.txt`)
  writeFileSync(tm, `H ${uid} Kerbal X\nB Kerbin 600000 3531600000000 70000 21549.425\nE Ponte do Jaci ativa no kOS\n`)
  const state = { seq: 0, ut: 1000, last: -1, received: [], status: 'PRELAUNCH', stage: 3 }
  const tick = () => {
    if (existsSync(cmd)) {
      for (const line of readFileSync(cmd, 'utf8').split('\n')) {
        const p = line.trim().split(' ')
        if (p.length < 2 || p[0] === 'S') continue
        const n = Number(p[0])
        if (n <= state.last) continue
        state.last = n
        state.received.push(p.slice(1).join(' '))
        if (p[1] === 'stage') state.stage--
        if (p[1] === 'launch') state.status = 'FLYING'
        appendFileSync(tm, `R ${n} True ${p[1]}\n`)
      }
    }
    state.ut += 0.25
    const f = [state.seq++, state.ut, 5, state.status, 120, -0.0972, -74.5577, 175, 0, 0, 80000, -590000, 0.1, 0.99, 1200, 10, 20, 180, 645000,
      30.5, 0, 650, 0, 0, state.stage, 400, 800, 488, 976, 30, 30, 150, 200, 0, 0, 90, 90, 'True', 'False', 'True', 'False', 'True', 30, 600, -1, 0, '-', -1, 0, 3, 'manual', 'free']
    appendFileSync(tm, `T ${f.join(' ')}\n`)
  }
  const h = setInterval(tick, 250)
  return { state, stop: () => clearInterval(h) }
}

const cases = [
  [
    'ksp: ponte do kOS, telemetria, comandos e contagem',
    async () => {
      const ksp = mkdtempSync(join(tmpdir(), 'jaci-ksp-'))
      mkdirSync(join(ksp, 'GameData', 'kOS'), { recursive: true })
      const s = await server('ksp', ['--ksp', ksp])
      const kos = fakeKos(join(ksp, 'Ships', 'Script', 'jaci'), '4242')
      try {
        assert(existsSync(join(ksp, 'Ships', 'Script', 'jaci', 'jaci.ks')), 'jaci.ks não foi instalado')
        assert(existsSync(join(ksp, 'Ships', 'Script', 'boot', 'jaci.ks')), 'boot/jaci.ks não foi instalado')
        const c = s.ws
        const snap = await until(c, 'nave do KSP', (x) => x.vehicles.some((v) => v.kind === 'ksp' && v.tm['nav.alt'] != null))
        assert(snap.ksp && snap.ksp.kos, 'estado da ligação com o KSP')
        const v = snap.vehicles.find((v) => v.kind === 'ksp')
        assert(v.name === 'Kerbal X' && v.central === 'kerbin' && v.phase === 'prelaunch', `nave: ${v.name} ${v.central} ${v.phase}`)
        assert(Math.abs(v.tm['res.lf'] - 50) < 1e-6 && v.ksp.sas && v.ksp.gear, 'telemetria')
        const d = await c.request({ type: 'dictionary' })
        assert(d.dictionary[v.id].some((x) => x.id === 'orb.ap'), 'dicionário da nave')
        assert(c.hello.commands.some((x) => x.id === 'ksp_stage'), 'comandos do KSP no dicionário')
        const r0 = await c.request({ type: 'command', vehicle: v.id, cmd: { id: 'ksp_stage' } })
        assert(!r0.ok, 'separação passou sem armar')
        const r1 = await c.request({ type: 'command', vehicle: v.id, cmd: { id: 'ksp_stage' }, armed: true })
        assert(r1.ok, `separação: ${r1.error}`)
        await until(c, 'resposta do kOS', (x) => x.commands.some((k) => k.id === r1.id && k.status === 'executed'), 10000)
        await until(c, 'estágio na telemetria', (x) => x.vehicles.find((k) => k.id === v.id).ksp.stage === 2, 10000)
        const r2 = await c.request({ type: 'command', vehicle: v.id, cmd: { id: 'ksp_steer', args: { mode: 'heading', heading: 90, pitch: 80 } } })
        assert(r2.ok, `atitude: ${r2.error}`)
        const r3 = await c.request({ type: 'command', vehicle: v.id, cmd: { id: 'ksp_count', args: { t_minus: 2, apo: 90, heading: 90 } }, armed: true })
        assert(r3.ok, `contagem: ${r3.error}`)
        await until(c, 'lançamento no T-0', (x) => x.vehicles.find((k) => k.id === v.id).phase === 'ascent', 15000)
        assert(kos.state.received.includes('steer heading 90 80'), `comandos recebidos: ${kos.state.received}`)
        assert(kos.state.received.includes('launch 90 90'), `comandos recebidos: ${kos.state.received}`)
        const pr = await c.request({ type: 'predict', vehicle: v.id, span: 600 })
        assert(pr.central === 'kerbin', 'previsão: corpo')
        const sim = await c.request({ type: 'command', vehicle: v.id, cmd: { id: 'att_mode', args: { mode: 'sun' } } })
        assert(!sim.ok, 'comando da simulação aceito numa nave do KSP')
      } finally {
        kos.stop()
        s.close()
        rmSync(ksp, { recursive: true, force: true })
      }
    },
  ],
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
    'gravação: gravar, reproduzir, pular e CSV',
    async () => {
      const recs = mkdtempSync(join(tmpdir(), 'jaci-gravacoes-'))
      const s = await server('sondagem', ['--record', '--recordings-dir', recs])
      try {
        const c = s.ws
        const snap = await until(c, 'gravando', (x) => x.recording?.frames >= 1)
        const id = snap.launch.vehicle
        const r = await c.request({ type: 'command', vehicle: id, cmd: { id: 'count_start', args: {} } })
        assert(r.ok, `contagem: ${r.error}`)
        await c.request({ type: 'sim', action: 'warp', value: 100 })
        await until(c, 'quadros gravados', (x) => x.recording?.frames >= 5, 20000)
        const stop = await c.request({ type: 'sim', action: 'record_stop' })
        assert(stop.ok, `parar: ${stop.error}`)
        const list = await c.request({ type: 'sim', action: 'recordings' })
        assert(list.list.length === 1, `gravações: ${JSON.stringify(list.list)}`)
        const name = list.list[0].name
        const rp = await c.request({ type: 'sim', action: 'replay_start', value: name })
        assert(rp.type === 'reload', `reproduzir: ${rp.error}`)
        // Quem conecta agora vê a gravação, desde o começo.
        const c2 = await connect(s.port)
        try {
          assert(c2.hello.replay?.frames >= 5, `quadros: ${c2.hello.replay?.frames}`)
          assert(c2.hello.replay.markers.length > 0, 'sem marcas na linha do tempo')
          // Parada no começo, a reprodução não manda quadros: o atual vem ao conectar.
          await until(c2, 'quadro da reprodução', (x) => x.replay?.index === 0, 3000)
          const c4 = await connect(s.port)
          await until(c4, 'quadro ao conectar', (x) => x.replay?.index === 0, 3000)
          c4.close()
          const cmd = await c2.request({ type: 'command', vehicle: id, cmd: { id: 'count_hold', args: {} } })
          assert(!cmd.ok, 'comando aceito na reprodução')
          await c2.request({ type: 'sim', action: 'replay_control', playing: true, speed: 30 })
          const end = await until(c2, 'fim da reprodução', (x) => x.replay && !x.replay.playing && x.replay.index === x.replay.frames - 1, 10000)
          assert(end.paused, 'reprodução no fim deveria estar parada')
          assert(c2.events.some((e) => /contagem/i.test(e.text)), 'eventos da gravação não chegaram')
          await c2.request({ type: 'sim', action: 'replay_control', seek: 1 })
          for (let i = 0; i < 30 && !c2.rewind; i++) await sleep(100)
          assert(c2.rewind, 'sem rewind ao pular')
          await until(c2, 'quadro 1', (x) => x.replay?.index === 1)
          const pr = await c2.request({ type: 'predict', vehicle: id, span: 600 })
          assert(Array.isArray(pr.points), 'predict na reprodução')
          const csv = await fetch(`http://127.0.0.1:${s.port}/api/csv?vehicle=${id}`)
          const text = await csv.text()
          assert(csv.status === 200 && text.startsWith('utc,t_j2000_s,'), `CSV: ${csv.status} ${text.slice(0, 80)}`)
          assert(text.trim().split('\n').length >= 6, 'CSV curto')
          const out = await c2.request({ type: 'sim', action: 'replay_stop' })
          assert(out.type === 'reload', 'sair da reprodução')
        } finally {
          c2.close()
        }
        const c3 = await connect(s.port)
        assert(!c3.hello.replay, 'ainda em reprodução')
        c3.close()
      } finally {
        s.close()
        rmSync(recs, { recursive: true, force: true })
      }
    },
  ],
  [
    'satélites reais: TLE, SGP4, passagens e lista guardada',
    async () => {
      const iss = `ISS (ZARYA)
1 25544U 98067A   26294.04791053  .00009575  00000+0  18188-3 0  9996
2 25544  51.6327  37.2623 0006759 289.5321  70.4938 15.48967427591263`
      // Começa um dia depois da época do TLE, sem internet: o TLE vai colado.
      const s = await server('sondagem', ['--start', '2026-10-22 01:00:00'])
      try {
        const c = s.ws
        await until(c, 'instantâneo', (x) => x.vehicles.length > 0)
        const bad = await c.request({ type: 'sim', action: 'track', value: 'nada aqui' })
        assert(!bad.ok, 'aceitou texto sem TLE')
        const r = await c.request({ type: 'sim', action: 'track', value: iss })
        assert(r.ok, `track: ${r.error}`)
        const snap = await until(c, 'ISS no instantâneo', (x) => x.vehicles.some((v) => v.kind === 'tracked'))
        const sat = snap.vehicles.find((v) => v.kind === 'tracked')
        assert(sat.tle.norad === 25544 && Math.abs(sat.alt - 420) < 30 && Math.abs(sat.inc - 51.6) < 0.3, `ISS: ${sat.alt} km, ${sat.inc}°`)
        assert(sat.tm['tle.age'] > 0.5 && sat.tm['tle.age'] < 1.5, `idade ${sat.tm['tle.age']}`)
        // De novo: atualiza, não duplica.
        await c.request({ type: 'sim', action: 'track', value: iss })
        await sleep(400)
        assert(c.snap.vehicles.filter((v) => v.kind === 'tracked').length === 1, 'duplicou o satélite')
        const p = await c.request({ type: 'passes', vehicle: sat.id, span: 86400 })
        assert(p.passes.length > 5, `passagens: ${p.passes.length}`)
        const pr = await c.request({ type: 'predict', vehicle: sat.id, span: 5600, step: 60 })
        assert(pr.points.length > 80 && pr.central === 'earth', 'traço previsto')
        const cmd = await c.request({ type: 'command', vehicle: sat.id, cmd: { id: 'tx', args: {} } })
        assert(!cmd.ok, 'comando aceito para satélite real')
        // Trocar de cenário mantém a lista.
        const ld = await c.request({ type: 'sim', action: 'scenario', value: 'estacao', start: '2026-10-22 01:00:00' })
        assert(ld.type === 'reload', 'cenário')
        const c2 = await connect(s.port)
        try {
          await until(c2, 'ISS no cenário novo', (x) => x.vehicles.some((v) => v.kind === 'tracked' && v.tle.norad === 25544))
          const rm = await c2.request({ type: 'sim', action: 'untrack', vehicle: c2.snap.vehicles.find((v) => v.kind === 'tracked').id })
          assert(rm.ok, 'remover')
        } finally {
          c2.close()
        }
      } finally {
        s.close()
      }
    },
  ],
  [
    'treino: falha planejada silenciosa, procedimento e relatório',
    async () => {
      const s = await server('estacao')
      try {
        const c = s.ws
        const snap = await until(c, 'instantâneo', (x) => x.vehicles.length > 0)
        const st = snap.vehicles.find((v) => v.kind === 'station')
        const pf = await c.request({ type: 'sim', action: 'plan_fault', vehicle: st.id, fault: 'cabin_leak', arg: '1', when: 'in', value: 20, announce: false })
        assert(pf.ok, `planejar: ${pf.error}`)
        const plan = await c.request({ type: 'sim', action: 'fault_plan' })
        assert(plan.plan.length === 1 && plan.plan[0].due > snap.t, 'plano')
        assert(!JSON.stringify(c.snap).includes('cabin_leak'), 'o plano vazou no instantâneo')
        await c.request({ type: 'sim', action: 'proc_log', vehicle: st.id, procedure: 'Despressurização', kind: 'start', total: 5 })
        await c.request({ type: 'sim', action: 'warp', value: 100 })
        for (let i = 0; i < 50; i++) {
          const p = await c.request({ type: 'sim', action: 'fault_plan' })
          if (p.plan[0].plan.state.state === 'done') break
          await sleep(100)
        }
        const done = await c.request({ type: 'sim', action: 'fault_plan' })
        assert(done.plan[0].plan.state.state === 'done', `não disparou: ${JSON.stringify(done.plan[0].plan.state)}`)
        assert(!c.events.some((e) => e.text.includes('[Instrutor]')), 'falha silenciosa avisou a equipe')
        assert(c.events.some((e) => e.text.startsWith('Procedimento iniciado')), 'procedimento não apareceu nos eventos')
        const rep = await c.request({ type: 'sim', action: 'report' })
        assert(rep.report.faults.length === 1 && rep.report.faults[0].planned, 'relatório sem a falha')
        assert(rep.report.procedures.length === 1, 'relatório sem o procedimento')
        assert(rep.report.score.total >= 0 && rep.report.score.total <= 100, 'nota')
        await c.request({ type: 'sim', action: 'report_reset' })
        const rep2 = await c.request({ type: 'sim', action: 'report' })
        assert(rep2.report.faults.length === 0, 'recomeçar não limpou')
      } finally {
        s.close()
      }
    },
  ],
  [
    'sala: posições, permissões, votação por pessoas e loops',
    async () => {
      const s = await server('sondagem', ['--room'])
      const others = []
      try {
        const flight = s.ws
        const meteo = await connect(s.port)
        const obs = await connect(s.port)
        others.push(meteo, obs)
        const snap = await until(flight, 'instantâneo', (x) => x.room?.on)
        assert(flight.hello.roles.some((r) => r.id === 'RSO'), 'papéis na saudação')
        const id = snap.launch.vehicle
        const j1 = await flight.request({ type: 'join', name: 'Ana', position: 'FLIGHT' })
        const j2 = await meteo.request({ type: 'join', name: 'Bia', position: 'METEO' })
        assert(j1.ok && j2.ok, 'entrar na sala')
        assert(!(await flight.request({ type: 'join', name: 'X', position: 'NAO' })).ok, 'posição inexistente aceita')
        // Observador e meteorologia não comandam a contagem; o FLIGHT sim.
        const count = { type: 'command', vehicle: id, cmd: { id: 'count_start', args: { t_minus: 120 } } }
        assert(!(await obs.request(count)).ok, 'observador comandou')
        const r2 = await meteo.request(count)
        assert(!r2.ok && /METEO/.test(r2.error), `meteorologia comandou: ${r2.error}`)
        assert((await flight.request(count)).ok, 'FLIGHT não comandou')
        // Votação: a meteorologia tem gente, então espera o voto.
        assert((await flight.request({ type: 'command', vehicle: id, cmd: { id: 'poll', args: {} } })).ok, 'votação')
        await until(flight, 'meteorologia aguardando', (x) => x.launch.positions.find((p) => p.id === 'wx')?.awaiting)
        assert(!(await obs.request({ type: 'command', vehicle: id, cmd: { id: 'vote', args: { position: 'wx', go: 'go' } } })).ok, 'observador votou')
        assert(!(await meteo.request({ type: 'command', vehicle: id, cmd: { id: 'vote', args: { position: 'rso', go: 'go' } } })).ok, 'votou por outra posição')
        const v = await meteo.request({ type: 'command', vehicle: id, cmd: { id: 'vote', args: { position: 'wx', go: 'go' } } })
        assert(v.ok, `voto: ${v.error}`)
        await until(flight, 'voto registrado', (x) => { const p = x.launch.positions.find((p) => p.id === 'wx'); return !p.awaiting && p.polled === true })
        // Loops.
        assert((await meteo.request({ type: 'chat', text: 'Tempo bom para o lançamento', channel: 'geral' })).ok, 'falar')
        assert(!(await obs.request({ type: 'chat', text: 'oi', channel: 'geral' })).ok, 'observador falou')
        await until(flight, 'mensagem no loop', () => flight.events.some((e) => e.level === 'chat' && e.from === 'Bia (METEO)' && e.text.startsWith('Tempo bom')))
        // Simulador: só o instrutor muda cenário; o FLIGHT acelera o tempo.
        assert(!(await flight.request({ type: 'sim', action: 'scenario', value: 'vlm' })).ok, 'FLIGHT trocou cenário')
        assert(!(await meteo.request({ type: 'sim', action: 'warp', value: 10 })).ok, 'meteorologia acelerou')
        assert((await flight.request({ type: 'sim', action: 'warp', value: 1 })).ok, 'FLIGHT não acelerou')
        // Quem sai some da sala.
        meteo.close()
        await until(flight, 'saída', (x) => x.room.members.length === 1)
        // Sala desligada: qualquer tela comanda.
        assert((await flight.request({ type: 'sim', action: 'room_mode', value: false })).ok, 'desligar a sala')
        assert((await obs.request({ type: 'command', vehicle: id, cmd: { id: 'count_hold', args: {} } })).ok, 'sala desligada ainda bloqueia')
      } finally {
        others.forEach((c) => c.close())
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
