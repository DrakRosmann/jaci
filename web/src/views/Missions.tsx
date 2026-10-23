// Editor das missões em arquivos (veículos, cenários e procedimentos em TOML),
// no console do instrutor. O servidor confere (inclusive montando o veículo ou
// o cenário) antes de gravar.

import { useEffect, useState } from 'react'
import { Panel, toast } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { store } from '../lib/store'

type Sub = 'veiculos' | 'cenarios' | 'procedimentos'

interface FileEntry {
  sub: Sub
  id: string
  name: string | null
  error?: string
}

const SUBS: [Sub, string][] = [
  ['cenarios', 'Cenários'],
  ['veiculos', 'Veículos'],
  ['procedimentos', 'Procedimentos'],
]

/** Ponto de partida de um arquivo novo. */
const TEMPLATES: Record<Sub, string> = {
  cenarios: `name = "Meu cenário"
description = "O que acontece neste cenário"
# start = "2026-11-05 12:00:00"   # sem isto: agora (ou a hora pedida)

[[vehicles]]
vehicle = "vsb30"          # do catálogo ou de missoes/veiculos
place = "pad"              # "pad" (plataforma) ou "orbit"
site = "Alcântara"
t_minus = 600

# [[vehicles]]
# vehicle = "amazonia"
# name = "Alvo"
# place = "orbit"
# pe_km = 700
# inc_deg = 98

# [weather]
# surface_wind = 8

# [[faults]]               # falhas planejadas (silenciosas por padrão)
# vehicle = "VSB-30"
# fault = "random"
# met_s = 30
# window_s = 20
`,
  veiculos: `name = "Meu veículo"
description = "De onde ele parte e o que muda"
base = "vsb30"             # veículo do catálogo de onde partir
payload_kg = 450
# propellant_scale = 1.1
# landing_target = [-2.0, -42.0]
# [ascent]                 # para lançadores orbitais
# target_pe_km = 500
# target_ap_km = 500
# inclination_deg = 98
`,
  procedimentos: `name = "Meu procedimento"
description = "Quando usar"
kinds = ["satellite"]       # vazio: todos os tipos

[[steps]]
kind = "check"
text = "Confira a situação no console de subsistemas"

[[steps]]
kind = "cmd"
text = "Apontar ao Sol"
cmd = "att_mode"
args = { mode = "sun" }

[[steps]]
kind = "wait"
text = "Bateria acima de 80 %"
until = "eps.soc > 80"
`,
}

export function Missions() {
  const [files, setFiles] = useState<FileEntry[]>([])
  const [dir, setDir] = useState('')
  const [sub, setSub] = useState<Sub>('cenarios')
  const [id, setId] = useState('')
  const [text, setText] = useState('')
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null)
  const load = () =>
    store
      .sim('files')
      .then((r: any) => {
        setFiles(r.list ?? [])
        setDir(r.dir ?? '')
      })
      .catch(() => {})
  useEffect(() => {
    load()
  }, [])
  const open = async (f: FileEntry) => {
    const r: any = await store.sim('file_get', { sub: f.sub, id: f.id })
    if (!r.ok) return toast(tx(r.error), 'bad')
    setSub(f.sub)
    setId(f.id)
    setText(r.text)
    setMsg(f.error ? { ok: false, text: f.error } : null)
  }
  const fresh = (s: Sub) => {
    setSub(s)
    setId('')
    setText(TEMPLATES[s])
    setMsg(null)
  }
  const run = async (action: 'file_validate' | 'file_save', thenLoad = false) => {
    const r: any = await store.sim(action, { sub, id, text })
    setMsg({ ok: !!r.ok, text: r.ok ? tx(r.message) : tx(r.error) })
    if (r.id) setId(r.id)
    if (r.ok && action === 'file_save') {
      load()
      if (thenLoad && sub === 'cenarios') store.sim('scenario', { value: r.id })
    }
  }
  return (
    <Panel title={t('Missões em arquivos')} className="span-3" right={<span className="small muted mono" title={dir}>{dir.split('/').slice(-2).join('/')}</span>}>
      <div className="missions">
        <div className="files">
          {SUBS.map(([s, label]) => (
            <div key={s}>
              <div className="row" style={{ justifyContent: 'space-between' }}>
                <b className="small">{t(label)}</b>
                <button className="small" onClick={() => fresh(s)} title={t('Arquivo novo a partir de um modelo')}>
                  + {t('novo')}
                </button>
              </div>
              {files
                .filter((f) => f.sub === s)
                .map((f) => (
                  <button key={f.id} className={`file ${f.sub === sub && f.id === id ? 'active' : ''} ${f.error ? 'bad' : ''}`} onClick={() => open(f)} title={f.error ?? f.name ?? ''}>
                    <span className="mono">{f.id}</span>
                    {f.name && <span className="small muted"> {f.name}</span>}
                    {f.error && <span className="small"> ⚠</span>}
                  </button>
                ))}
            </div>
          ))}
          <button className="small" onClick={() => store.sim('files_reload').then(load)} title={t('Lê a pasta de novo (depois de mexer nos arquivos por fora)')}>
            {t('Ler a pasta de novo')}
          </button>
        </div>
        <div className="editor">
          <div className="row">
            <select value={sub} onChange={(e) => setSub(e.target.value as Sub)} aria-label={t('Pasta')}>
              {SUBS.map(([s, label]) => (
                <option key={s} value={s}>
                  {t(label)}
                </option>
              ))}
            </select>
            <input className="mono" value={id} onChange={(e) => setId(e.target.value)} placeholder={t('nome-do-arquivo')} style={{ flex: 1 }} aria-label={t('Nome do arquivo')} />
            <span className="mono small muted">.toml</span>
          </div>
          <textarea className="mono" value={text} onChange={(e) => setText(e.target.value)} spellCheck={false} rows={22} placeholder={t('Abra um arquivo ou comece um novo.')} />
          {msg && <pre className={`small ${msg.ok ? 'good' : 'bad'}`} style={{ whiteSpace: 'pre-wrap', margin: 0 }}>{msg.text}</pre>}
          <div className="row">
            <button disabled={!text.trim()} onClick={() => run('file_validate')}>
              {t('Conferir')}
            </button>
            <button className="primary" disabled={!text.trim() || !id.trim()} onClick={() => run('file_save')}>
              {t('Salvar')}
            </button>
            {sub === 'cenarios' && (
              <button disabled={!text.trim() || !id.trim()} onClick={() => run('file_save', true)}>
                {t('Salvar e carregar o cenário')}
              </button>
            )}
            <span className="small muted">{t('Formato e exemplos: missoes/LEIAME.md')}</span>
          </div>
        </div>
      </div>
    </Panel>
  )
}
