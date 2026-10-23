// Sala de controle com posições: entrar numa posição, o crachá na barra de
// cima e os loops de conversa.

import { useEffect, useMemo, useRef, useState } from 'react'
import { t, tx } from '../lib/i18n'
import { utc } from '../lib/format'
import { shallow, store, useStore } from '../lib/store'
import type { EventMsg } from '../types'
import { toast } from './ui'

/** Janela para escolher nome e posição. */
export function JoinDialog({ onClose }: { onClose: () => void }) {
  const roles = useStore((s) => s.hello?.roles ?? [])
  const members = useStore((s) => s.snap?.room?.members ?? [], shallow)
  const me = useStore((s) => s.me)
  const [name, setName] = useState(me?.name ?? '')
  const [position, setPosition] = useState(me?.position ?? 'FLIGHT')
  const join = async () => {
    const r = await store.join({ name: name.trim(), position })
    if (r.ok) onClose()
    else toast(tx(r.error), 'bad')
  }
  return (
    <div className="modal-back" onClick={onClose}>
      <div className="modal room-dialog" role="dialog" aria-modal="true" aria-label={t('Entrar na sala')} onClick={(e) => e.stopPropagation()}>
        <h3>
          {t('Entrar na sala')}
          <button className="small" onClick={onClose} aria-label={t('Fechar')}>
            ✕
          </button>
        </h3>
        <div className="form">
          <label>{t('Seu nome')}</label>
          <input autoFocus value={name} onChange={(e) => setName(e.target.value)} maxLength={40} onKeyDown={(e) => e.key === 'Enter' && name.trim() && join()} />
        </div>
        <div className="roles">
          {roles.map((r) => {
            const here = members.filter((m) => m.position === r.id).map((m) => m.name)
            return (
              <label key={r.id} className={position === r.id ? 'sel' : ''}>
                <input type="radio" name="position" value={r.id} checked={position === r.id} onChange={() => setPosition(r.id)} />
                <span>
                  <b className="mono">{r.id}</b> {tx(r.name)}
                  <div className="small muted">{tx(r.description)}</div>
                  {here.length > 0 && <div className="small">{here.join(', ')}</div>}
                </span>
              </label>
            )
          })}
        </div>
        <div className="row" style={{ marginTop: 10 }}>
          <button className="primary" disabled={!name.trim()} onClick={join}>
            {t('Entrar')}
          </button>
          {me && (
            <button
              onClick={async () => {
                await store.join(null)
                onClose()
              }}
            >
              {t('Sair da posição')}
            </button>
          )}
        </div>
      </div>
    </div>
  )
}

/** Crachá na barra de cima: posição e nome (clique para trocar). */
export function RoomChip({ onOpen }: { onOpen: () => void }) {
  const on = useStore((s) => !!s.snap?.room?.on)
  const me = useStore((s) => s.me)
  const count = useStore((s) => s.snap?.room?.members.length ?? 0)
  if (!on && !me) return null
  return (
    <button className={`small room-chip ${me ? 'active' : on ? 'danger' : ''}`} onClick={onOpen} title={t('Posição na sala ({n} pessoas)', { n: count })}>
      {me ? (
        <>
          <b className="mono">{me.position}</b> {me.name}
        </>
      ) : (
        t('Escolher posição')
      )}
    </button>
  )
}

/** Os loops de conversa: geral e um por posição. */
export function ChatDock({ open, onToggle }: { open: boolean; onToggle: () => void }) {
  const events = useStore((s) => s.events)
  const me = useStore((s) => s.me)
  const roles = useStore((s) => s.hello?.roles ?? [])
  const replay = useStore((s) => s.snap?.replay != null)
  const msgs = useMemo(() => events.filter((e) => e.level === 'chat'), [events])
  const [channel, setChannel] = useState('geral')
  const [only, setOnly] = useState<'all' | 'mine'>('all')
  const [text, setText] = useState('')
  const [seen, setSeen] = useState(0)
  const list = useRef<HTMLDivElement>(null)
  const mine = (e: EventMsg) => e.channel === 'geral' || e.channel === me?.position || e.from?.endsWith(`(${me?.position})`)
  const shown = only === 'mine' && me ? msgs.filter(mine) : msgs
  useEffect(() => {
    const el = list.current
    if (el) el.scrollTop = el.scrollHeight
  }, [shown.length, open])
  const send = async () => {
    const msg = text.trim()
    if (!msg) return
    const r = await store.chat(msg, channel)
    if (r.ok) setText('')
    else toast(tx(r.error), 'bad')
  }
  // Lidas: as que estavam lá quando o painel abriu ou fechou (aberto, nenhuma pendente).
  const unread = open ? 0 : Math.max(0, msgs.length - seen)
  const toggle = () => {
    setSeen(msgs.length)
    onToggle()
  }
  return (
    <>
      <button className={`small ${open ? 'active' : ''}`} onClick={toggle} title={t('Loops de conversa')} aria-label={t('Loops de conversa')} aria-expanded={open}>
        💬
        {unread > 0 && !open && <span className="badge">{unread}</span>}
      </button>
      {open && (
        <section className="chat-dock" aria-label={t('Loops de conversa')}>
          <header>
            <b>{t('Loops')}</b>
            <select className="small" value={only} onChange={(e) => setOnly(e.target.value as 'all' | 'mine')} aria-label={t('Ouvir')}>
              <option value="all">{t('ouvir todos')}</option>
              <option value="mine">{t('só geral e o meu')}</option>
            </select>
            <button className="small" onClick={toggle} aria-label={t('Fechar')}>
              ✕
            </button>
          </header>
          <div className="msgs" ref={list} role="log">
            {shown.length === 0 && <div className="muted small">{t('Nenhuma mensagem.')}</div>}
            {shown.map((e, i) => (
              <div key={i} className={`msg ${e.from?.endsWith(`(${me?.position})`) && e.from.startsWith(me?.name ?? '\u0000') ? 'me' : ''}`}>
                <span className="muted mono small">{utc(e.t, false)}</span> <b>{e.from}</b>
                {e.channel !== 'geral' && <span className="chan"> → {e.channel}</span>}
                <div>{e.text}</div>
              </div>
            ))}
          </div>
          <footer>
            <select className="small" value={channel} onChange={(e) => setChannel(e.target.value)} aria-label={t('Loop')}>
              <option value="geral">{t('geral')}</option>
              {roles
                .filter((r) => r.id !== 'OBS')
                .map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.id}
                  </option>
                ))}
            </select>
            <input value={text} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && send()} placeholder={replay ? t('em reprodução') : t('mensagem…')} disabled={replay} maxLength={500} />
            <button className="small primary" onClick={send} disabled={replay || !text.trim()}>
              {t('Falar')}
            </button>
          </footer>
        </section>
      )}
    </>
  )
}
