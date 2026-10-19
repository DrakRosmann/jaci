import { useMemo, useState } from 'react'
import { Empty, Panel, sendCommand } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { duration, utc } from '../lib/format'
import { useStore } from '../lib/store'
import type { CmdDef } from '../types'

export function Commands() {
  const v = useStore((s) => s.selected)
  const hello = useStore((s) => s.hello)
  const snap = useStore((s) => s.snap)!
  const [sel, setSel] = useState<string>('att_mode')
  const [filter, setFilter] = useState('')
  // Comandos de lançamento só fazem sentido na plataforma ou na subida.
  const launching = v ? ['prelaunch', 'ascent', 'coast'].includes(v.phase) : false
  const available = useMemo(
    () =>
      (hello?.commands ?? []).filter(
        (c) => (!v || c.kinds.length === 0 || c.kinds.includes(v.kind)) && (launching || !['Lançamento', 'Segurança de voo'].includes(c.group)),
      ),
    [hello, v, launching],
  )
  const groups = useMemo(() => {
    const m = new Map<string, CmdDef[]>()
    for (const c of available) {
      if (filter && !`${c.name} ${c.id} ${c.description}`.toLowerCase().includes(filter.toLowerCase())) continue
      m.set(c.group, [...(m.get(c.group) ?? []), c])
    }
    return [...m.entries()]
  }, [available, filter])
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  const def = available.find((c) => c.id === sel) ?? available[0]
  const mine = snap.commands.filter((c) => c.vehicle === v.id)
  return (
    <div className="grid cols-3">
      <Panel title={t('Dicionário de comandos')} flush>
        <div style={{ padding: 8 }}>
          <input placeholder={t('procurar…')} value={filter} onChange={(e) => setFilter(e.target.value)} style={{ width: '100%' }} />
        </div>
        <div style={{ maxHeight: 560, overflowY: 'auto' }}>
          {groups.map(([g, list]) => (
            <div key={g}>
              <div className="small muted" style={{ padding: '6px 10px', textTransform: 'uppercase', letterSpacing: '0.06em' }}>
                {tx(g)}
              </div>
              {list.map((c) => (
                <div key={c.id} onClick={() => setSel(c.id)} style={{ padding: '4px 14px', cursor: 'pointer', background: c.id === def?.id ? '#16274a' : undefined }}>
                  {tx(c.name)} {c.hazardous && <span className="bad small">⚠</span>} {c.ground && <span className="muted small">({t('solo')})</span>}
                </div>
              ))}
            </div>
          ))}
        </div>
      </Panel>
      {def && <CommandForm key={def.id + v.id} def={def} vehicle={v.id} />}
      <Panel title={t('Histórico')} flush>
        <div style={{ maxHeight: 600, overflowY: 'auto' }}>
          <table>
            <thead>
              <tr>
                <th>{t('Enviado')}</th>
                <th>{t('Comando')}</th>
                <th>{t('Estado')}</th>
              </tr>
            </thead>
            <tbody>
              {mine.map((c) => (
                <tr key={c.id}>
                  <td className="mono small">{utc(c.sent, false)}</td>
                  <td className="mono small">
                    {c.text}
                    {c.message && <div className="muted">{tx(c.message)}</div>}
                  </td>
                  <td className={c.status === 'executed' ? 'good' : c.status === 'failed' ? 'bad' : 'warn'}>
                    {t(statusName(c.status))}
                    {c.status === 'sent' && c.exec_at > snap.t && <div className="small muted">{t('em')} {duration(c.exec_at - snap.t)}</div>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </Panel>
    </div>
  )
}

function statusName(s: string): string {
  return ({ queued: 'na fila', sent: 'a caminho', executed: 'executado', failed: 'falhou' } as Record<string, string>)[s] ?? s
}

function CommandForm({ def, vehicle }: { def: CmdDef; vehicle: number }) {
  const snap = useStore((s) => s.snap)!
  const [args, setArgs] = useState<Record<string, string>>({})
  const [when, setWhen] = useState<'now' | 'in' | 'at'>('now')
  const [inSec, setInSec] = useState('60')
  const [at, setAt] = useState('')
  const [armed, setArmed] = useState(false)
  const send = async () => {
    if (def.hazardous && !armed) {
      setArmed(true)
      setTimeout(() => setArmed(false), 5000)
      return
    }
    setArmed(false)
    const a: Record<string, unknown> = {}
    for (const d of def.args) {
      const raw = args[d.name] ?? (d.kind === 'enum' ? d.options?.[0]?.[0] : undefined)
      if (raw == null || raw === '') continue
      a[d.name] = d.kind === 'number' || d.kind === 'vehicle' ? Number(String(raw).replace(',', '.')) : raw
    }
    const opts = { armed: def.hazardous, ...(when === 'in' ? { in: Number(inSec) } : when === 'at' ? { at } : {}) }
    await sendCommand(vehicle, def.id, a, opts)
  }
  const others = snap.vehicles.filter((x) => x.kind !== 'debris' && x.id !== vehicle)
  return (
    <Panel title={tx(def.name)}>
      <div className="small muted" style={{ marginBottom: 8 }}>
        {tx(def.description)}
      </div>
      <div className="form">
        {def.args.map((d) => (
          <Arg key={d.name} label={`${tx(d.label)}${d.unit ? ` (${d.unit})` : ''}${d.optional ? ` · ${t('opcional')}` : ''}`}>
            {d.kind === 'enum' ? (
              <select value={args[d.name] ?? d.options?.[0]?.[0]} onChange={(e) => setArgs({ ...args, [d.name]: e.target.value })}>
                {d.options?.map(([id, name]) => (
                  <option key={id} value={id}>
                    {tx(name)}
                  </option>
                ))}
              </select>
            ) : d.kind === 'vehicle' ? (
              <select value={args[d.name] ?? ''} onChange={(e) => setArgs({ ...args, [d.name]: e.target.value })}>
                <option value="">—</option>
                {others.map((x) => (
                  <option key={x.id} value={x.id}>
                    {x.name}
                  </option>
                ))}
              </select>
            ) : (
              <input value={args[d.name] ?? ''} onChange={(e) => setArgs({ ...args, [d.name]: e.target.value })} placeholder={d.kind === 'utc' ? 'AAAA-MM-DD HH:MM:SS' : ''} />
            )}
          </Arg>
        ))}
        {!def.ground && (
          <Arg label={t('Execução')}>
            <span className="row">
              <select value={when} onChange={(e) => setWhen(e.target.value as 'now')}>
                <option value="now">{t('agora')}</option>
                <option value="in">{t('daqui a (s)')}</option>
                <option value="at">{t('na hora (UTC)')}</option>
              </select>
              {when === 'in' && <input value={inSec} onChange={(e) => setInSec(e.target.value)} style={{ width: 80 }} />}
              {when === 'at' && <input value={at} onChange={(e) => setAt(e.target.value)} placeholder="AAAA-MM-DD HH:MM:SS" style={{ width: 170 }} />}
            </span>
          </Arg>
        )}
      </div>
      <div className="row" style={{ marginTop: 10 }}>
        <button className={armed ? 'armed' : def.hazardous ? 'danger' : 'primary'} onClick={send}>
          {armed ? t('CONFIRMAR') : def.hazardous ? t('Armar') : t('Enviar')}
        </button>
        {def.hazardous && <span className="small warn">{t('Comando perigoso: dois passos (armar e confirmar em 5 s).')}</span>}
      </div>
      {!def.ground && <div className="small muted" style={{ marginTop: 8 }}>{t('Comandos de bordo exigem enlace e chegam com o atraso da luz.')}</div>}
    </Panel>
  )
}

function Arg({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <label>{label}</label>
      {children}
    </>
  )
}
