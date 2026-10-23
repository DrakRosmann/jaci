// Satélites reais: busca os elementos NORAD (TLE) no CelesTrak e manda ao
// servidor, que propaga com o SGP4. Também aceita TLEs colados.

import { useState } from 'react'
import { Panel, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { distance, num } from '../lib/format'
import { shallow, store, useStore } from '../lib/store'

const CELESTRAK = 'https://celestrak.org/NORAD/elements/gp.php'

/** Atalhos: [nome, consulta ao CelesTrak]. */
const PRESETS: [string, string][] = [
  ['ISS', 'CATNR=25544'],
  ['Tiangong', 'CATNR=48274'],
  ['Hubble', 'CATNR=20580'],
  ['Amazonia-1', 'CATNR=47699'],
  ['CBERS-4A', 'CATNR=44883'],
  ['CBERS-4', 'CATNR=40336'],
  ['SCD-1', 'CATNR=22490'],
  ['SCD-2', 'CATNR=25504'],
  ['SGDC-1', 'CATNR=42692'],
  ['NOAA 19', 'CATNR=33591'],
  ['Landsat 9', 'CATNR=49260'],
  ['GOES-16', 'CATNR=41866'],
  ['Estações espaciais', 'GROUP=stations'],
]

async function fetchTle(query: string): Promise<string> {
  const r = await fetch(`${CELESTRAK}?${query}&FORMAT=tle`)
  const text = await r.text()
  if (!r.ok || !/^1 /m.test(text)) throw new Error(text.trim().slice(0, 120) || `HTTP ${r.status}`)
  return text
}

export function RealSatellites() {
  const sats = useStore((s) => (s.snap?.vehicles ?? []).filter((v) => v.kind === 'tracked'), shallow)
  const selected = useStore((s) => s.ui.selected)
  const [query, setQuery] = useState('')
  const [paste, setPaste] = useState('')
  const [busy, setBusy] = useState(false)
  const replay = useStore((s) => s.snap?.replay != null)
  const track = async (text: string) => {
    const r: any = await store.sim('track', { value: text })
    toast(r.ok ? tx(r.message) : tx(r.error), r.ok ? 'good' : 'bad')
    if (r.errors?.length) toast(r.errors.join('; '), 'bad')
    return r.ok
  }
  const run = async (fn: () => Promise<unknown>) => {
    setBusy(true)
    try {
      await fn()
    } catch (e) {
      toast(`${t('CelesTrak')}: ${e instanceof Error ? e.message : String(e)}`, 'bad')
    } finally {
      setBusy(false)
    }
  }
  const fromCelestrak = (q: string) => run(async () => track(await fetchTle(q)))
  const search = () => {
    const q = query.trim()
    if (!q) return
    fromCelestrak(/^\d+$/.test(q) ? `CATNR=${q}` : `NAME=${encodeURIComponent(q)}`)
  }
  // Busca de novo os elementos de todos, um por vez (o CelesTrak limita os pedidos).
  const refresh = () =>
    run(async () => {
      let text = ''
      for (const s of sats) if (s.tle) text += await fetchTle(`CATNR=${s.tle.norad}`)
      if (text) await track(text)
    })
  const norads = new Set(sats.map((s) => s.tle?.norad))
  return (
    <Panel
      title={`${t('Satélites reais (TLE)')} (${sats.length})`}
      right={
        sats.length > 0 && (
          <button className="small" disabled={busy || replay} onClick={refresh} title={t('Busca de novo os elementos de todos no CelesTrak')}>
            {t('Atualizar TLEs')}
          </button>
        )
      }
    >
      <div className="grid">
        <div className="row">
          {PRESETS.map(([name, q]) => {
            const have = q.startsWith('CATNR=') && norads.has(Number(q.slice(6)))
            return (
              <button key={q} className={`small ${have ? 'active' : ''}`} disabled={busy || replay} onClick={() => fromCelestrak(q)} title={have ? t('Já acompanhado: clique para atualizar o TLE') : q}>
                {have && '✓ '}
                {tx(name)}
              </button>
            )
          })}
        </div>
        <div className="row">
          <input value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && search()} placeholder={t('número NORAD ou nome')} style={{ flex: 1 }} disabled={replay} />
          <button className="primary" disabled={busy || replay || !query.trim()} onClick={search}>
            {busy ? t('Buscando…') : t('Buscar no CelesTrak')}
          </button>
        </div>
        <details>
          <summary className="small muted">{t('Colar TLEs (sem internet ou de outra fonte)')}</summary>
          <textarea value={paste} onChange={(e) => setPaste(e.target.value)} rows={4} style={{ width: '100%', fontFamily: 'var(--mono)', fontSize: 11 }} placeholder={'ISS (ZARYA)\n1 25544U ...\n2 25544 ...'} />
          <button disabled={!paste.trim() || replay} onClick={() => track(paste).then((ok) => ok && setPaste(''))}>
            {t('Acompanhar')}
          </button>
        </details>
        {sats.length > 0 && (
          <table>
            <thead>
              <tr>
                <th>{t('Satélite')}</th>
                <th className="num">NORAD</th>
                <th className="num">{t('Altitude')}</th>
                <th className="num">{t('Idade do TLE')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {sats.map((s) => {
                const age = s.tle?.age ?? NaN
                return (
                  <tr key={s.id} className={s.id === selected ? 'sel' : ''} onClick={() => store.setUi({ selected: s.id })} style={{ cursor: 'pointer' }}>
                    <td>{s.name}</td>
                    <td className="num">{s.tle?.norad}</td>
                    <td className="num">{distance(s.alt)}</td>
                    <td className={`num ${Math.abs(age) > 30 ? 'bad' : Math.abs(age) > 7 ? 'warn' : ''}`}>
                      {num(age, 1)} {t('dias')}
                    </td>
                    <td>
                      <button
                        className="small"
                        disabled={replay}
                        onClick={(e) => {
                          e.stopPropagation()
                          store.sim('untrack', { vehicle: s.id })
                        }}
                        aria-label={`${t('Remover')} ${s.name}`}
                      >
                        ✕
                      </button>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        )}
        <p className="small muted">{t('Posições pelo SGP4 a partir dos elementos NORAD; o erro cresce alguns km por dia de idade do TLE. A lista fica guardada no servidor.')}</p>
      </div>
    </Panel>
  )
}
