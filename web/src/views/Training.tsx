// Treino: falhas planejadas pelo instrutor e o relatório do treino.

import { useEffect, useState } from 'react'
import { Panel, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { duration, num, utc } from '../lib/format'
import { store, useStore } from '../lib/store'

interface PlanItem {
  plan: {
    id: number
    vehicle: number
    fault: string
    arg: string | null
    when: { kind: 'at' | 'met'; value: number }
    window: number
    probability: number
    announce: boolean
    state: { state: 'pending'; offset: number } | { state: 'done'; t: number; message: string } | { state: 'skipped' } | { state: 'failed'; t: number; error: string }
  }
  vehicle_name: string
  due: number | null
}

interface Report {
  mission: string
  from: number
  to: number
  alarms: { t: number; vehicle: string; name: string; level: 'yellow' | 'red'; reaction: number | null }[]
  faults: { t: number; vehicle: string; fault: string; message: string; planned: boolean; detected: number | null; responded: number | null }[]
  procedures: { procedure: string; vehicle: string; started: number; finished: number | null; steps: number; total: number | null }[]
  commands: number
  vehicles: [string, string][]
  lost: string[]
  score: { total: number; reaction: number | null; response: number | null; outcome: number }
}

const result = (r: { ok?: boolean; error?: string; message?: string }) => toast(r.ok === false ? tx(r.error) : tx(r.message) || t('feito'), r.ok === false ? 'bad' : 'good')

/** Falhas planejadas: numa hora, depois do lançamento, sorteadas ou não. */
export function FaultPlan() {
  const hello = useStore((s) => s.hello)!
  const v = useStore((s) => s.selected)
  const now = useStore((s) => s.snap?.t ?? 0)
  const [fault, setFault] = useState('random')
  const [arg, setArg] = useState('')
  const [when, setWhen] = useState<'in' | 'met' | 'at'>('in')
  const [value, setValue] = useState('60')
  const [at, setAt] = useState('')
  const [win, setWin] = useState('0')
  const [prob, setProb] = useState('100')
  const [announce, setAnnounce] = useState(false)
  const [plan, setPlan] = useState<PlanItem[]>([])
  const load = () =>
    store
      .sim('fault_plan')
      .then((r: any) => setPlan(r.plan ?? []))
      .catch(() => {})
  useEffect(() => {
    load()
    const h = setInterval(load, 2000)
    return () => clearInterval(h)
  }, [])
  const add = () =>
    store
      .sim('plan_fault', { vehicle: v?.id, fault, arg, when, value: Number(value.replace(',', '.')), at, window: Number(win.replace(',', '.')), probability: Number(prob.replace(',', '.')) / 100, announce })
      .then((r) => {
        result(r)
        load()
      })
  const faultName = (id: string) => (id === 'random' ? t('Aleatória') : tx(hello.faults.find((f) => f.id === id)?.name ?? id))
  const faultDef = hello.faults.find((f) => f.id === fault)
  return (
    <Panel title={t('Falhas planejadas')} className="span-2">
      <div className="form">
        <label>{t('Veículo')}</label>
        <span>{v?.name ?? '—'}</span>
        <label>{t('Falha')}</label>
        <select value={fault} onChange={(e) => setFault(e.target.value)}>
          <option value="random">{t('Aleatória (uma que sirva ao veículo)')}</option>
          {hello.faults.map((f) => (
            <option key={f.id} value={f.id}>
              {tx(f.name)}
            </option>
          ))}
        </select>
        {faultDef?.arg && (
          <>
            <label>{tx(faultDef.arg)}</label>
            <input value={arg} onChange={(e) => setArg(e.target.value)} />
          </>
        )}
        <label>{t('Quando')}</label>
        <div className="row">
          <select value={when} onChange={(e) => setWhen(e.target.value as 'in' | 'met' | 'at')}>
            <option value="in">{t('daqui a (s)')}</option>
            <option value="met">{t('T+ (s depois do lançamento)')}</option>
            <option value="at">{t('na hora (UTC)')}</option>
          </select>
          {when === 'at' ? <input value={at} onChange={(e) => setAt(e.target.value)} placeholder="AAAA-MM-DD HH:MM:SS" style={{ width: 170 }} /> : <input value={value} onChange={(e) => setValue(e.target.value)} style={{ width: 80 }} />}
        </div>
        <label>{t('Janela aleatória (s)')}</label>
        <input value={win} onChange={(e) => setWin(e.target.value)} title={t('O instante é sorteado entre o escolhido e ele mais a janela')} />
        <label>{t('Chance (%)')}</label>
        <input value={prob} onChange={(e) => setProb(e.target.value)} title={t('Sorteada ao planejar: com menos de 100%, pode não acontecer')} />
        <label />
        <label className="small">
          <input type="checkbox" checked={announce} onChange={(e) => setAnnounce(e.target.checked)} /> {t('avisar a equipe no registro de eventos')}
        </label>
      </div>
      <div className="row" style={{ marginTop: 8 }}>
        <button className="primary" disabled={!v} onClick={add}>
          {t('Planejar')}
        </button>
        <span className="small muted">{t('A equipe não vê este plano: só quem abre o console do instrutor.')}</span>
      </div>
      {plan.length > 0 && (
        <table style={{ marginTop: 8 }}>
          <thead>
            <tr>
              <th>#</th>
              <th>{t('Veículo')}</th>
              <th>{t('Falha')}</th>
              <th>{t('Quando')}</th>
              <th>{t('Situação')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {plan.map(({ plan: p, vehicle_name, due }) => (
              <tr key={p.id}>
                <td className="mono">{p.id}</td>
                <td>{vehicle_name}</td>
                <td>
                  {faultName(p.fault)}
                  {p.arg ? ` (${p.arg})` : ''}
                  <div className="small muted">
                    {p.probability < 1 && `${num(p.probability * 100, 0)}% · `}
                    {p.window > 0 && `${t('janela')} ${duration(p.window)} · `}
                    {p.announce ? t('avisada') : t('silenciosa')}
                  </div>
                </td>
                <td className="small">
                  {p.when.kind === 'met' ? `T+${duration(p.when.value)}` : utc(p.when.value)}
                  {due != null && p.state.state === 'pending' && <div className="muted">{due > now ? `${t('em')} ${duration(due - now)}` : t('agora')}</div>}
                </td>
                <td className="small">
                  {p.state.state === 'pending' && <span className="muted">{due == null ? t('esperando o lançamento') : t('pendente')}</span>}
                  {p.state.state === 'done' && (
                    <span className="warn">
                      {utc(p.state.t, false)} · {tx(p.state.message)}
                    </span>
                  )}
                  {p.state.state === 'skipped' && <span className="muted">{t('não sorteada')}</span>}
                  {p.state.state === 'failed' && <span className="bad">{tx(p.state.error)}</span>}
                </td>
                <td>
                  {p.state.state === 'pending' && (
                    <button className="small" onClick={() => store.sim('unplan_fault', { id: p.id }).then(load)} aria-label={t('Remover')}>
                      ✕
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </Panel>
  )
}

/** Relatório do treino: reação aos alarmes, falhas, procedimentos e desfecho. */
export function TrainingReport() {
  const [r, setR] = useState<Report | null>(null)
  const load = () => store.sim('report').then((x: any) => setR(x.report))
  const pct = (x: number | null) => (x == null ? '—' : `${num(x * 100, 0)}%`)
  const secs = (x: number | null) => (x == null ? t('não') : duration(x))
  return (
    <Panel
      title={t('Relatório do treino')}
      className="span-3"
      right={
        <span className="row">
          <button className="small primary" onClick={load}>
            {r ? t('Atualizar') : t('Gerar relatório')}
          </button>
          {r && (
            <button className="small" onClick={() => download(r)}>
              {t('Baixar (.md)')}
            </button>
          )}
          <button
            className="small"
            onClick={() =>
              store.sim('report_reset').then((x) => {
                result(x)
                setR(null)
              })
            }
            title={t('Começa um treino novo agora (as falhas planejadas pendentes continuam)')}
          >
            {t('Recomeçar')}
          </button>
        </span>
      }
    >
      {!r ? (
        <p className="muted small">
          {t('Reação aos alarmes, detecção e resposta às falhas, procedimentos seguidos e o desfecho, desde o começo do treino (o cenário carregado ou "Recomeçar").')}{' '}
          {t('Os tempos são do relógio da simulação: deixe ligado "voltar para 1× num alarme" para medir a reação de verdade.')}
        </p>
      ) : (
        <div className="grid">
          <div className="row" style={{ alignItems: 'flex-start', gap: 24 }}>
            <div style={{ textAlign: 'center' }}>
              <div className="big-clock" style={{ color: r.score.total >= 80 ? 'var(--good)' : r.score.total >= 50 ? 'var(--warn)' : 'var(--bad)' }}>{r.score.total}</div>
              <div className="small muted">{t('nota indicativa')}</div>
            </div>
            <div className="tiles" style={{ flex: 1 }}>
              <Tile2 label={t('Reação aos alarmes')} value={pct(r.score.reaction)} hint={t('100% até 30 s; zero a partir de 3 min ou sem reconhecer')} />
              <Tile2 label={t('Resposta às falhas')} value={pct(r.score.response)} hint={t('100% com comando ao veículo em até 2 min; 50% em até 10 min')} />
              <Tile2 label={t('Desfecho')} value={r.lost.length ? `${t('perdido')}: ${r.lost.join(', ')}` : t('nenhum veículo perdido')} hint={t('Veículo perdido corta a nota pela metade')} />
              <Tile2 label={t('Duração')} value={duration(r.to - r.from)} hint={`${utc(r.from)} → ${utc(r.to)}`} />
              <Tile2 label={t('Comandos')} value={String(r.commands)} />
              <Tile2 label={t('Alarmes')} value={String(r.alarms.length)} />
            </div>
          </div>
          {r.faults.length > 0 && (
            <table>
              <thead>
                <tr>
                  <th>{t('Falha')}</th>
                  <th>{t('Veículo')}</th>
                  <th>{t('Hora')}</th>
                  <th className="num">{t('Detecção (1º alarme)')}</th>
                  <th className="num">{t('Resposta (1º comando)')}</th>
                </tr>
              </thead>
              <tbody>
                {r.faults.map((f, i) => (
                  <tr key={i}>
                    <td>
                      {tx(f.message)} <span className="small muted">{f.planned ? t('planejada') : t('manual')}</span>
                    </td>
                    <td>{f.vehicle}</td>
                    <td className="mono small">{utc(f.t)}</td>
                    <td className="num">{secs(f.detected)}</td>
                    <td className={`num ${f.responded == null ? 'bad' : f.responded > 120 ? 'warn' : 'good'}`}>{secs(f.responded)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          {r.alarms.length > 0 && (
            <div style={{ maxHeight: 260, overflowY: 'auto' }}>
              <table>
                <thead>
                  <tr>
                    <th>{t('Alarme')}</th>
                    <th>{t('Veículo')}</th>
                    <th>{t('Hora')}</th>
                    <th className="num">{t('Reconhecido em')}</th>
                  </tr>
                </thead>
                <tbody>
                  {r.alarms.map((a, i) => (
                    <tr key={i}>
                      <td className={a.level === 'red' ? 'bad' : 'warn'}>{tx(a.name)}</td>
                      <td>{a.vehicle}</td>
                      <td className="mono small">{utc(a.t)}</td>
                      <td className={`num ${a.reaction == null ? 'bad' : a.reaction > 30 ? 'warn' : 'good'}`}>{secs(a.reaction)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {r.procedures.length > 0 && (
            <table>
              <thead>
                <tr>
                  <th>{t('Procedimento')}</th>
                  <th>{t('Veículo')}</th>
                  <th>{t('Início')}</th>
                  <th className="num">{t('Passos')}</th>
                  <th className="num">{t('Duração')}</th>
                </tr>
              </thead>
              <tbody>
                {r.procedures.map((p, i) => (
                  <tr key={i}>
                    <td>{t(p.procedure)}</td>
                    <td>{p.vehicle}</td>
                    <td className="mono small">{utc(p.started)}</td>
                    <td className="num">
                      {p.steps}
                      {p.total ? `/${p.total}` : ''}
                    </td>
                    <td className="num">{p.finished != null ? duration(p.finished - p.started) : <span className="warn">{t('não concluído')}</span>}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </Panel>
  )
}

function Tile2({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return (
    <div className="tile" title={hint}>
      <div className="label">{label}</div>
      <div className="value" style={{ fontSize: 15, whiteSpace: 'normal' }}>
        {value}
      </div>
    </div>
  )
}

/** O relatório em Markdown, para arquivar ou mandar à equipe. */
function reportMarkdown(r: Report): string {
  const s = (x: number | null) => (x == null ? '—' : duration(x))
  const p = (x: number | null) => (x == null ? '—' : `${Math.round(x * 100)}%`)
  const lines = [
    `# ${t('Relatório do treino')} — ${tx(r.mission)}`,
    '',
    `${utc(r.from)} → ${utc(r.to)} (${duration(r.to - r.from)})`,
    '',
    `**${t('nota indicativa')}: ${r.score.total}** · ${t('Reação aos alarmes')}: ${p(r.score.reaction)} · ${t('Resposta às falhas')}: ${p(r.score.response)} · ${t('Desfecho')}: ${r.lost.length ? `${t('perdido')}: ${r.lost.join(', ')}` : t('nenhum veículo perdido')}`,
    '',
    `${t('Comandos')}: ${r.commands} · ${t('Alarmes')}: ${r.alarms.length}`,
  ]
  if (r.faults.length) {
    lines.push('', `## ${t('Falhas')}`, '', `| ${t('Falha')} | ${t('Veículo')} | ${t('Hora')} | ${t('Detecção (1º alarme)')} | ${t('Resposta (1º comando)')} |`, '|---|---|---|---|---|')
    for (const f of r.faults) lines.push(`| ${tx(f.message)} (${f.planned ? t('planejada') : t('manual')}) | ${f.vehicle} | ${utc(f.t)} | ${s(f.detected)} | ${s(f.responded)} |`)
  }
  if (r.alarms.length) {
    lines.push('', `## ${t('Alarmes')}`, '', `| ${t('Alarme')} | ${t('Veículo')} | ${t('Hora')} | ${t('Reconhecido em')} |`, '|---|---|---|---|')
    for (const a of r.alarms) lines.push(`| ${tx(a.name)} (${a.level === 'red' ? t('vermelho') : t('amarelo')}) | ${a.vehicle} | ${utc(a.t)} | ${s(a.reaction)} |`)
  }
  if (r.procedures.length) {
    lines.push('', `## ${t('Procedimentos')}`, '', `| ${t('Procedimento')} | ${t('Veículo')} | ${t('Início')} | ${t('Passos')} | ${t('Duração')} |`, '|---|---|---|---|---|')
    for (const x of r.procedures) lines.push(`| ${t(x.procedure)} | ${x.vehicle} | ${utc(x.started)} | ${x.steps}${x.total ? `/${x.total}` : ''} | ${x.finished != null ? duration(x.finished - x.started) : t('não concluído')} |`)
  }
  lines.push('', `## ${t('Veículos')}`, '', ...r.vehicles.map(([n, ph]) => `- ${n}: ${tx(ph)}`), '')
  return lines.join('\n')
}

function download(r: Report) {
  const blob = new Blob([reportMarkdown(r)], { type: 'text/markdown;charset=utf-8' })
  const a = document.createElement('a')
  a.href = URL.createObjectURL(blob)
  a.download = `jaci-relatorio-${utc(r.to).replace(/[ :]/g, '-')}.md`
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 1000)
}
