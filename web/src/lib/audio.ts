// Sons da sala de controle: alarme mestre, aviso amarelo, bipes e locução da
// contagem e dos eventos. Tudo sintetizado (Web Audio) e falado pelo próprio
// navegador (speechSynthesis): nenhum arquivo de som.
//
// Os navegadores só deixam tocar som depois de um clique; por isso o som é
// ligado por um botão (a escolha fica guardada, mas o primeiro clique na página
// ainda é preciso para destravar o áudio).

import type { EventMsg, Snapshot } from '../types'

export interface SoundPrefs {
  /** Liga e desliga tudo. */
  on: boolean
  /** Tom do alarme mestre (vermelho) e aviso (amarelo). */
  alarms: boolean
  /** Locução da contagem (T menos…). */
  countdown: boolean
  /** Locução dos alarmes e dos eventos importantes. */
  events: boolean
  volume: number
}

export const DEFAULT_PREFS: SoundPrefs = { on: false, alarms: true, countdown: true, events: false, volume: 0.6 }

/** Instantes da contagem que são falados (segundos antes do T-0). */
export const CALLOUTS = [600, 300, 120, 60, 30, 20, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1]

/** O que dizer ao cruzar `prev` → `now` na contagem (ou nada). */
export function countdownCallout(prev: number, now: number, lang: 'pt' | 'en'): string | null {
  if (!(now < prev)) return null
  if (prev > 0 && now <= 0) return lang === 'en' ? 'Liftoff' : 'Decolagem'
  const crossed = CALLOUTS.filter((c) => prev > c && now <= c)
  if (crossed.length === 0) return null
  // Pulou vários (aceleração do tempo): fala só o último.
  const c = crossed[crossed.length - 1]
  if (c >= 60) {
    const min = c / 60
    return lang === 'en' ? `T minus ${min} minute${min > 1 ? 's' : ''}` : `T menos ${min} minuto${min > 1 ? 's' : ''}`
  }
  if (c > 10) return lang === 'en' ? `T minus ${c} seconds` : `T menos ${c} segundos`
  return String(c)
}

class Sound {
  prefs: SoundPrefs = loadPrefs()
  private ctx: AudioContext | null = null
  private master: GainNode | null = null
  private alarmTimer: ReturnType<typeof setInterval> | null = null
  /** Alarme mestre silenciado até chegar um vermelho novo. */
  silencedUntilNew = false
  private seenAlarms = new Set<number>()
  private lastTMinus: number | null = null
  private lastCountState: string | null = null
  private primed = false
  private lastSaid = { text: '', at: 0 }

  setPrefs(p: Partial<SoundPrefs>) {
    this.prefs = { ...this.prefs, ...p }
    try {
      localStorage.setItem('jaci.sound', JSON.stringify(this.prefs))
    } catch {
      /* sem armazenamento */
    }
    if (this.master) this.master.gain.value = this.prefs.volume
    if (this.prefs.on) this.unlock()
    else this.stopAlarm()
  }

  /** Cria ou destrava o áudio (precisa vir de um clique). */
  unlock() {
    if (typeof AudioContext === 'undefined') return
    if (!this.ctx) {
      this.ctx = new AudioContext()
      this.master = this.ctx.createGain()
      this.master.gain.value = this.prefs.volume
      this.master.connect(this.ctx.destination)
    }
    if (this.ctx.state === 'suspended') void this.ctx.resume()
  }

  /** Um tom: frequência (Hz), duração (s), forma, começo (s a partir de agora). */
  private tone(freq: number, dur: number, type: OscillatorType = 'sine', at = 0, gain = 0.25) {
    if (!this.ctx || !this.master || this.ctx.state !== 'running') return
    const t0 = this.ctx.currentTime + at
    const osc = this.ctx.createOscillator()
    const g = this.ctx.createGain()
    osc.type = type
    osc.frequency.value = freq
    // Envelope curto nas pontas, para não estalar.
    g.gain.setValueAtTime(0, t0)
    g.gain.linearRampToValueAtTime(gain, t0 + 0.01)
    g.gain.setValueAtTime(gain, t0 + dur - 0.02)
    g.gain.linearRampToValueAtTime(0, t0 + dur)
    osc.connect(g).connect(this.master)
    osc.start(t0)
    osc.stop(t0 + dur + 0.01)
  }

  /** Alarme mestre: dois tons alternados. */
  private masterBurst() {
    for (let i = 0; i < 4; i++) this.tone(i % 2 ? 1000 : 750, 0.22, 'square', i * 0.25, 0.12)
  }

  caution() {
    this.tone(880, 0.18, 'triangle', 0, 0.3)
    this.tone(660, 0.3, 'triangle', 0.2, 0.3)
  }

  tick(last = false) {
    this.tone(last ? 440 : 1000, last ? 0.7 : 0.08, 'sine', 0, 0.3)
  }

  /** Toca o alarme mestre até parar (ou ser silenciado). */
  private startAlarm() {
    if (this.alarmTimer) return
    this.masterBurst()
    this.alarmTimer = setInterval(() => this.masterBurst(), 1600)
  }

  stopAlarm() {
    if (this.alarmTimer) clearInterval(this.alarmTimer)
    this.alarmTimer = null
  }

  get alarmSounding(): boolean {
    return this.alarmTimer != null
  }

  silence() {
    this.silencedUntilNew = true
    this.stopAlarm()
  }

  say(text: string, lang: 'pt' | 'en', urgent = false) {
    if (typeof speechSynthesis === 'undefined') return false
    const voices = speechSynthesis.getVoices()
    const want = lang === 'en' ? 'en' : 'pt'
    const voice = voices.find((v) => v.lang.toLowerCase().startsWith(lang === 'en' ? 'en-us' : 'pt-br')) ?? voices.find((v) => v.lang.toLowerCase().startsWith(want))
    if (!voice) return false
    if (urgent) speechSynthesis.cancel()
    this.lastSaid = { text, at: Date.now() }
    const u = new SpeechSynthesisUtterance(text)
    u.voice = voice
    u.lang = voice.lang
    u.volume = this.prefs.volume
    u.rate = 1.05
    speechSynthesis.speak(u)
    return true
  }

  /** Olha o instantâneo novo e toca o que for preciso. */
  update(snap: Snapshot, events: EventMsg[], lang: 'pt' | 'en', translate: (s: string) => string, myPosition?: string) {
    const p = this.prefs
    // Na primeira passada só aprende o estado (não toca o que já estava lá).
    const first = !this.primed
    this.primed = true
    // Alarmes.
    const red = snap.alarms.filter((a) => a.active && !a.acked && a.level === 'red')
    const fresh = snap.alarms.filter((a) => !this.seenAlarms.has(a.id))
    for (const a of fresh) this.seenAlarms.add(a.id)
    const newRed = fresh.some((a) => a.level === 'red' && a.active && !a.acked)
    const newYellow = fresh.some((a) => a.level === 'yellow' && a.active && !a.acked)
    if (newRed) this.silencedUntilNew = false
    if (p.on && p.alarms && red.length > 0 && !this.silencedUntilNew) this.startAlarm()
    else this.stopAlarm()
    if (!first && p.on && p.alarms && newYellow && !newRed) this.caution()
    if (!first && p.on && p.events) {
      for (const a of fresh.filter((a) => a.active && !a.acked)) this.say(`${lang === 'en' ? 'Alarm' : 'Alarme'}: ${translate(a.name)}`, lang, a.level === 'red')
    }
    // Contagem: falada só em tempo real (ou quase), senão vira uma enxurrada.
    const lc = snap.launch
    const realtime = (snap.replay ? snap.replay.speed : snap.warp) <= 2 && !snap.paused
    const counting = lc.vehicle != null && (lc.state === 'counting' || lc.state === 'hold')
    if (!first && p.on && p.countdown && lc.state !== this.lastCountState && this.lastCountState) {
      if (lc.state === 'hold') this.say(lang === 'en' ? 'Hold, hold, hold' : 'Espera, espera, espera', lang, true)
      if (lc.state === 'scrubbed') this.say(lang === 'en' ? 'Launch scrubbed' : 'Lançamento cancelado', lang, true)
      if (lc.state === 'counting' && this.lastCountState === 'hold') this.say(lang === 'en' ? 'Count resumed' : 'Contagem retomada', lang)
    }
    if (!first && p.on && p.countdown && counting && realtime && this.lastTMinus != null) {
      const line = countdownCallout(this.lastTMinus, lc.t_minus, lang)
      if (line && !this.say(line, lang, lc.t_minus <= 10)) {
        // Sem voz no sistema: bipes nos últimos 10 s e um tom longo no T-0.
        if (lc.t_minus <= 10) this.tick(lc.t_minus <= 0)
      }
    }
    if (!first && p.on && p.countdown && lc.state === 'launched' && this.lastCountState === 'counting' && realtime && this.lastTMinus != null && this.lastTMinus > 0) {
      if (!this.say(lang === 'en' ? 'Liftoff' : 'Decolagem', lang, true)) this.tick(true)
    }
    this.lastTMinus = counting || lc.state === 'launched' ? lc.t_minus : null
    this.lastCountState = lc.state
    // Chamada no loop da minha posição: dois toques curtos.
    if (!first && p.on && myPosition && events.some((e) => e.level === 'chat' && e.channel === myPosition && !e.from?.endsWith(`(${myPosition})`))) {
      this.tone(1320, 0.07, 'sine', 0, 0.2)
      this.tone(1760, 0.07, 'sine', 0.09, 0.2)
    }
    // Eventos importantes (bons e alarmes vêm acima).
    if (!first && p.on && p.events) {
      for (const e of events) {
        if (e.level !== 'good') continue
        const text = translate(e.text)
        // A contagem acabou de dizer o mesmo ("Decolagem"): não repete.
        if (Date.now() - this.lastSaid.at < 3000 && text.startsWith(this.lastSaid.text)) continue
        this.say(text, lang)
      }
    }
  }

  /** Recomeço (cenário novo, reprodução): esquece o que já viu. */
  reset() {
    this.primed = false
    this.seenAlarms.clear()
    this.lastTMinus = null
    this.lastCountState = null
    this.stopAlarm()
  }
}

function loadPrefs(): SoundPrefs {
  try {
    return { ...DEFAULT_PREFS, ...JSON.parse(localStorage.getItem('jaci.sound') ?? '{}') }
  } catch {
    return DEFAULT_PREFS
  }
}

export const sound = new Sound()
