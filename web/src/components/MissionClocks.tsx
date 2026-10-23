// Faixa de relógios da sala, como os painéis numéricos do MOCR do Apollo:
// GMT (com o dia do ano), GET (tempo desde o lançamento), o próximo evento
// com a contagem até ele e AOS/LOS (quanto falta para ganhar ou perder o
// sinal com uma estação).

import { t, tx } from '../lib/i18n'
import { useUpcoming } from '../lib/upcoming'
import { useStore } from '../lib/store'

const J2000_UNIX = 946_728_000

const two = (n: number) => String(Math.floor(n)).padStart(2, '0')

/** "DDD HH:MM:SS" a partir de segundos. */
function dhms(s: number): [string, string, string, string] {
  const a = Math.abs(s)
  const d = Math.floor(a / 86_400)
  const h = Math.floor((a % 86_400) / 3_600)
  const m = Math.floor((a % 3_600) / 60)
  return [String(d).padStart(3, '0'), two(h), two(m), two(a % 60)]
}

function Digits({ label, value, units, tone, sign }: { label: string; value: string[]; units: string[]; tone?: string; sign?: string }) {
  return (
    <div className={`mclock ${tone ?? ''}`}>
      <span className="mclock-label">{label}</span>
      <span className="mclock-digits">
        {sign && <span className="sign">{sign}</span>}
        {value.map((v, i) => (
          <span key={i} className="grp">
            <b>{v}</b>
            <small>{units[i]}</small>
          </span>
        ))}
      </span>
    </div>
  )
}

export function MissionClocks() {
  const now = useStore((s) => s.snap?.t ?? 0)
  const v = useStore((s) => s.selected)
  const lc = useStore((s) => s.snap?.launch)
  const up = useUpcoming()
  const date = new Date((now + J2000_UNIX) * 1000)
  const doy = Math.floor((date.getTime() - Date.UTC(date.getUTCFullYear(), 0, 1)) / 86_400_000) + 1
  const gmt = [String(doy).padStart(3, '0'), two(date.getUTCHours()), two(date.getUTCMinutes()), two(date.getUTCSeconds())]
  const met = v?.met
  const counting = lc && lc.vehicle != null && lc.state !== 'idle' && lc.state !== 'launched'
  const next = up.events.find((e) => e.t > now)
  const pass = up.passes.find((p) => p.los > now)
  const inPass = pass && pass.aos <= now
  const units = [t('DIA'), 'H', 'MIN', 'SEG']
  return (
    <div className="mission-clocks" role="timer" aria-label={t('Relógios da sala')}>
      <Digits label="GMT" value={gmt} units={units} />
      {counting ? (
        <Digits label={lc.state === 'hold' ? t('CONTAGEM (ESPERA)') : t('CONTAGEM')} value={dhms(lc.t_minus)} units={units} tone={lc.state === 'hold' ? 'amber' : 'orange'} sign="T−" />
      ) : (
        <Digits label="GET" value={met != null ? dhms(met) : ['---', '--', '--', '--']} units={units} sign={met != null && met < 0 ? '−' : ''} />
      )}
      <Digits label={next ? `${t('PRÓX')}: ${tx(next.text).toUpperCase()}` : t('PRÓX')} value={next ? dhms(next.t - now) : ['---', '--', '--', '--']} units={units} tone="cyan" />
      <Digits label={pass ? `${inPass ? 'LOS' : 'AOS'} ${tx(pass.station).toUpperCase()}` : 'AOS/LOS'} value={pass ? dhms((inPass ? pass.los : pass.aos) - now) : ['---', '--', '--', '--']} units={units} tone={inPass ? 'green' : 'amber'} />
    </div>
  )
}
