import { useEffect, useState } from 'react'
import { Empty, Panel, sendCommand } from '../components/ui'
import { t } from '../lib/i18n'
import { utc } from '../lib/format'
import { PROCEDURES, type Procedure } from '../lib/procedures'
import { store, useStore } from '../lib/store'

interface Run {
  proc: string
  vehicle: number
  step: number
  done: { step: number; t: number }[]
  auto: boolean
}

export function Procedures() {
  const v = useStore((s) => s.selected)
  const snap = useStore((s) => s.snap)!
  const [runs, setRuns] = useState<Record<string, Run>>({})
  const [sel, setSel] = useState<string>('countdown')
  const list = PROCEDURES.filter((p) => !v || p.kinds.length === 0 || p.kinds.includes(v.kind))
  const proc = list.find((p) => p.id === sel) ?? list[0]
  const key = v && proc ? `${proc.id}:${v.id}` : ''
  const run = runs[key]

  const advance = (r: Run) => {
    setRuns((all) => ({ ...all, [key]: { ...r, step: r.step + 1, done: [...r.done, { step: r.step, t: store.snap!.t }] } }))
  }

  // Avança sozinho nas esperas cumpridas.
  useEffect(() => {
    if (!run || !proc || !v || !run.auto) return
    const step = proc.steps[run.step]
    if (step?.kind === 'wait' && step.until(v, snap)) advance(run)
  })

  if (!v || !proc) return <Empty>{t('Selecione um veículo.')}</Empty>

  const start = () => setRuns({ ...runs, [key]: { proc: proc.id, vehicle: v.id, step: 0, done: [], auto: true } })

  const exec = async (p: Procedure, r: Run) => {
    const step = p.steps[r.step]
    if (step.kind === 'cmd') {
      const args = typeof step.args === 'function' ? step.args(v, snap) : (step.args ?? {})
      const ok = await sendCommand(v.id, step.cmd, args, { armed: step.hazardous })
      if (ok) advance(r)
    } else advance(r)
  }

  return (
    <div className="grid cols-3">
      <Panel title={t('Procedimentos')} flush>
        {list.map((p) => {
          const r = runs[`${p.id}:${v.id}`]
          return (
            <div key={p.id} onClick={() => setSel(p.id)} style={{ padding: '8px 12px', cursor: 'pointer', background: p.id === proc.id ? '#16274a' : undefined, borderBottom: '1px solid #1a2540' }}>
              <div>
                {t(p.name)}{' '}
                {r && <span className="small muted">({r.step >= p.steps.length ? t('concluído') : `${r.step}/${p.steps.length}`})</span>}
              </div>
              <div className="small muted">{t(p.description)}</div>
            </div>
          )
        })}
      </Panel>
      <Panel
        title={`${t(proc.name)} · ${v.name}`}
        className="span-2"
        right={
          <span className="row">
            {run && (
              <label className="small">
                <input type="checkbox" checked={run.auto} onChange={(e) => setRuns({ ...runs, [key]: { ...run, auto: e.target.checked } })} /> {t('avançar sozinho nas esperas')}
              </label>
            )}
            <button className="small" onClick={start}>
              {run ? t('Recomeçar') : t('Começar')}
            </button>
          </span>
        }
        flush
      >
        <table>
          <tbody>
            {proc.steps.map((s, i) => {
              const done = run?.done.find((d) => d.step === i)
              const current = run && run.step === i
              const ok = s.kind === 'wait' && s.until(v, snap)
              return (
                <tr key={i} className={current ? 'sel' : ''} style={{ opacity: run && i < run.step ? 0.6 : 1 }}>
                  <td className="mono">{i + 1}</td>
                  <td>
                    <span className="small muted">{t(kindName(s.kind))}</span>
                    <div>{t(s.text)}</div>
                    {s.kind === 'cmd' && <div className="small mono muted">{s.cmd}</div>}
                  </td>
                  <td className="small">
                    {done ? (
                      <span className="good">✓ {utc(done.t, false)}</span>
                    ) : s.kind === 'wait' ? (
                      <span className={ok ? 'good' : 'muted'}>{ok ? t('cumprida') : t('aguardando')}</span>
                    ) : null}
                  </td>
                  <td>
                    {current && (
                      <span className="row">
                        <button className={s.kind === 'cmd' && s.hazardous ? 'danger small' : 'primary small'} onClick={() => exec(proc, run!)} disabled={s.kind === 'wait' && !ok}>
                          {s.kind === 'cmd' ? t('Enviar') : s.kind === 'check' ? t('Confirmar') : t('Seguir')}
                        </button>
                        <button className="small" onClick={() => advance(run!)}>
                          {t('Pular')}
                        </button>
                      </span>
                    )}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
        {run && run.step >= proc.steps.length && <div className="good" style={{ padding: 10 }}>{t('Procedimento concluído.')}</div>}
        {!run && <div className="muted" style={{ padding: 10 }}>{t('Clique em Começar para seguir o procedimento passo a passo.')}</div>}
      </Panel>
    </div>
  )
}

function kindName(k: string): string {
  return ({ note: 'Nota', check: 'Verificação', cmd: 'Comando', wait: 'Espera' } as Record<string, string>)[k] ?? k
}
