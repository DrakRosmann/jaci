import { useState } from 'react'
import { Chart, COLORS } from '../components/Chart'
import { Empty, Panel, limitState } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { num, utc } from '../lib/format'
import { store, useStore } from '../lib/store'

export function Telemetry() {
  const v = useStore((s) => s.selected)
  const snap = useStore((s) => s.snap)!
  const dict = useStore((s) => (v ? s.dictionary[String(v.id)] : undefined))
  const [filter, setFilter] = useState('')
  const [chosen, setChosen] = useState<string[]>(['nav.alt'])
  const [hours, setHours] = useState(1)
  const alarms = snap.alarms.slice().reverse()
  const name = (id: number) => snap.vehicles.find((x) => x.id === id)?.name ?? `#${id}`
  return (
    <div className="grid">
      <Panel
        title={`${t('Alarmes')} (${alarms.filter((a) => a.active && !a.acked).length})`}
        right={
          <button className="small" onClick={() => store.sim('ack')}>
            {t('Reconhecer todos')}
          </button>
        }
        flush
      >
        {alarms.length === 0 ? (
          <div className="muted" style={{ padding: 10 }}>{t('Nenhum alarme.')}</div>
        ) : (
          <div style={{ maxHeight: 220, overflowY: 'auto' }}>
            <table>
              <thead>
                <tr>
                  <th>{t('Hora')}</th>
                  <th>{t('Veículo')}</th>
                  <th>{t('Ponto')}</th>
                  <th className="num">{t('Valor')}</th>
                  <th>{t('Nível')}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {alarms.map((a) => (
                  <tr key={a.id} style={{ opacity: a.active ? 1 : 0.5 }}>
                    <td className="mono small">{utc(a.t)}</td>
                    <td>{name(a.vehicle)}</td>
                    <td>{tx(a.name)}</td>
                    <td className="num">
                      {num(a.value, 2)} {a.unit}
                    </td>
                    <td className={a.level === 'red' ? 'bad' : 'warn'}>{a.level === 'red' ? t('vermelho') : t('amarelo')}{!a.active && ` · ${t('normalizado')}`}</td>
                    <td>
                      {!a.acked && (
                        <button className="small" onClick={() => store.sim('ack', { value: a.id })}>
                          {t('Reconhecer')}
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Panel>
      {!v || !dict ? (
        <Empty>{t('Selecione um veículo com telemetria.')}</Empty>
      ) : (
        <div className="grid cols-2">
          <Panel title={`${t('Pontos de telemetria')} · ${v.name} (${dict.length})`} flush>
            <div style={{ padding: 8 }}>
              <input placeholder={t('procurar…')} value={filter} onChange={(e) => setFilter(e.target.value)} style={{ width: '100%' }} />
            </div>
            <div style={{ maxHeight: 560, overflowY: 'auto' }}>
              <table>
                <thead>
                  <tr>
                    <th />
                    <th>{t('Ponto')}</th>
                    <th className="num">{t('Valor')}</th>
                    <th>{t('Unidade')}</th>
                    <th>{t('Limites')}</th>
                  </tr>
                </thead>
                <tbody>
                  {dict
                    .filter((d) => !filter || `${d.name} ${d.id} ${d.group}`.toLowerCase().includes(filter.toLowerCase()))
                    .map((d) => {
                      const val = v.tm?.[d.id]
                      const st = limitState(d, val)
                      return (
                        <tr key={d.id}>
                          <td>
                            <input
                              type="checkbox"
                              checked={chosen.includes(d.id)}
                              onChange={(e) => setChosen(e.target.checked ? [...chosen, d.id].slice(-6) : chosen.filter((x) => x !== d.id))}
                            />
                          </td>
                          <td>
                            {tx(d.name)}
                            <div className="small muted mono">
                              {d.id} · {tx(d.group)}
                            </div>
                          </td>
                          <td className={`num ${st === 'red' ? 'bad' : st === 'yellow' ? 'warn' : ''}`}>{num(val)}</td>
                          <td className="muted">{d.unit}</td>
                          <td className="small muted mono">{d.red ? `${fmt(d.red[0])}…${fmt(d.red[1])}` : ''}</td>
                        </tr>
                      )
                    })}
                </tbody>
              </table>
            </div>
          </Panel>
          <Panel
            title={t('Gráfico')}
            right={
              <span className="row">
                <a className="button small" href={store.csvUrl(v.id)} download title={snap.replay ? t('Telemetria do veículo selecionado, a gravação inteira') : t('Telemetria dos últimos 30 min do veículo selecionado')}>
                  CSV
                </a>
                <select value={hours} onChange={(e) => setHours(Number(e.target.value))}>
                  {[0.1, 0.5, 1, 3, 12, 48, 1e6].map((h) => (
                    <option key={h} value={h}>
                      {h >= 1e6 ? t('tudo') : h < 1 ? `${h * 60} min` : `${h} h`}
                    </option>
                  ))}
                </select>
              </span>
            }
            flush
          >
            <Chart
              height={520}
              series={chosen.map((id, i) => {
                const [ts, vs] = store.history.series(v.id, id, snap.t - hours * 3600)
                const d = dict.find((x) => x.id === id)
                return { label: `${tx(d?.name ?? id)}${d?.unit ? ` (${d.unit})` : ''}`, color: COLORS[i % COLORS.length], t: ts, v: vs }
              })}
            />
          </Panel>
        </div>
      )}
    </div>
  )
}

function fmt(x: number): string {
  return Math.abs(x) > 1e8 ? '∞' : String(Number(x.toPrecision(4)))
}
