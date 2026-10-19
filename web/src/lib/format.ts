// Formatação de números, tempos e datas.

import { store } from './store'

const locale = () => (store.ui.lang === 'en' ? 'en-US' : 'pt-BR')

const J2000_UNIX = 946_728_000

/** Instante da simulação (s desde J2000) em data UTC. */
export function utc(t: number, withDate = true): string {
  const d = new Date((t + J2000_UNIX) * 1000)
  const iso = d.toISOString()
  return withDate ? `${iso.slice(0, 10)} ${iso.slice(11, 19)}` : iso.slice(11, 19)
}

/** Para campos de data (AAAA-MM-DD HH:MM:SS). */
export function utcInput(t: number): string {
  return utc(t)
}

/** T±hh:mm:ss (ou dias). */
export function met(t: number | null | undefined): string {
  if (t == null || !isFinite(t)) return '—'
  const sign = t < 0 ? '-' : '+'
  let a = Math.abs(t)
  const days = Math.floor(a / 86400)
  a -= days * 86400
  const h = Math.floor(a / 3600)
  const m = Math.floor((a % 3600) / 60)
  const s = Math.floor(a % 60)
  const hms = `${pad(h)}:${pad(m)}:${pad(s)}`
  return days > 0 ? `T${sign}${days}d ${hms}` : `T${sign}${hms}`
}

/** Duração curta: 45 s, 12 min 03 s, 3 h 20 min, 2 d 04 h. */
export function duration(s: number | null | undefined): string {
  if (s == null || !isFinite(s)) return '—'
  const neg = s < 0
  let a = Math.abs(s)
  let out: string
  if (a < 60) out = `${a.toFixed(a < 10 ? 1 : 0)} s`
  else if (a < 3600) out = `${Math.floor(a / 60)} min ${pad(Math.floor(a % 60))} s`
  else if (a < 86400) out = `${Math.floor(a / 3600)} h ${pad(Math.floor((a % 3600) / 60))} min`
  else {
    const d = Math.floor(a / 86400)
    a -= d * 86400
    out = `${d} d ${pad(Math.floor(a / 3600))} h`
  }
  return neg ? `-${out}` : out
}

function pad(n: number): string {
  return String(n).padStart(2, '0')
}

/** Número com casas decimais conforme a grandeza; travessão se indefinido. */
export function num(x: number | null | undefined, digits?: number): string {
  if (x == null || !isFinite(x)) return '—'
  if (digits == null) {
    const a = Math.abs(x)
    digits = a >= 1000 ? 0 : a >= 100 ? 1 : a >= 10 ? 1 : a >= 1 ? 2 : a === 0 ? 0 : 3
  }
  return x.toLocaleString(locale(), { minimumFractionDigits: digits, maximumFractionDigits: digits })
}

/** Distância (km) que cresce para mil km ou UA. */
export function distance(km: number | null | undefined): string {
  if (km == null || !isFinite(km)) return '—'
  const a = Math.abs(km)
  const en = store.ui.lang === 'en'
  if (a > 1e7) return `${num(km / 149_597_870.7, 3)} ${en ? 'AU' : 'UA'}`
  if (a > 1e5) return `${num(km / 1000, 0)} ${en ? 'thousand km' : 'mil km'}`
  return `${num(km, a < 100 ? 2 : 0)} km`
}

export function latlon(lat: number, lon: number): string {
  const ns = lat >= 0 ? 'N' : 'S'
  const ew = lon >= 0 ? (store.ui.lang === 'en' ? 'E' : 'L') : store.ui.lang === 'en' ? 'W' : 'O'
  return `${Math.abs(lat).toFixed(2)}° ${ns}, ${Math.abs(lon).toFixed(2)}° ${ew}`
}

export function rate(bps: number | null | undefined): string {
  if (bps == null || !isFinite(bps)) return '—'
  if (bps >= 1e6) return `${num(bps / 1e6, 1)} Mbit/s`
  if (bps >= 1e3) return `${num(bps / 1e3, 1)} kbit/s`
  return `${num(bps, 0)} bit/s`
}
