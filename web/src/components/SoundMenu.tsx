// Botão de som da barra de cima: liga e desliga, escolhe o que toca e o volume.

import { useEffect, useRef, useState } from 'react'
import { sound, type SoundPrefs } from '../lib/audio'
import { t } from '../lib/i18n'
import { useStore } from '../lib/store'

export function SoundMenu() {
  const [prefs, setPrefs] = useState(sound.prefs)
  const [open, setOpen] = useState(false)
  const lang = useStore((s) => s.ui.lang)
  // Redesenha com os instantâneos, para mostrar o botão de silenciar.
  useStore((s) => s.snap?.alarms)
  const box = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const close = (e: MouseEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) setOpen(false)
    }
    window.addEventListener('mousedown', close)
    return () => window.removeEventListener('mousedown', close)
  }, [open])
  const set = (p: Partial<SoundPrefs>) => {
    sound.setPrefs(p)
    setPrefs(sound.prefs)
  }
  const check = (k: 'alarms' | 'countdown' | 'events', label: string) => (
    <label>
      <input type="checkbox" checked={prefs[k]} disabled={!prefs.on} onChange={(e) => set({ [k]: e.target.checked })} /> {label}
    </label>
  )
  return (
    <div className="sound" ref={box}>
      {sound.alarmSounding && (
        <button className="small danger" onClick={() => sound.silence()} title={t('Silencia o alarme mestre nesta tela até chegar outro alarme vermelho (não reconhece o alarme)')}>
          {t('Silenciar')}
        </button>
      )}
      <button className={`small ${prefs.on ? 'active' : ''}`} onClick={() => setOpen(!open)} aria-expanded={open} aria-label={t('Som')} title={`${t('Som')} (m)`}>
        {prefs.on ? '🔊' : '🔇'}
      </button>
      {open && (
        <div className="menu" role="dialog" aria-label={t('Som')}>
          <label>
            <input type="checkbox" checked={prefs.on} onChange={(e) => set({ on: e.target.checked })} /> <b>{t('Som ligado')}</b>
          </label>
          {check('alarms', t('Alarme mestre e avisos'))}
          {check('countdown', t('Locução da contagem'))}
          {check('events', t('Locução dos alarmes e eventos'))}
          <label>
            {t('Volume')}
            <input type="range" min={0} max={1} step={0.05} value={prefs.volume} disabled={!prefs.on} onChange={(e) => set({ volume: Number(e.target.value) })} />
          </label>
          <div className="row">
            <button className="small" disabled={!prefs.on} onClick={() => sound.caution()}>
              {t('Testar aviso')}
            </button>
            <button className="small" disabled={!prefs.on} onClick={() => sound.say(lang === 'en' ? 'T minus 10' : 'T menos 10', lang) || sound.tick()}>
              {t('Testar voz')}
            </button>
          </div>
          {typeof speechSynthesis !== 'undefined' && speechSynthesis.getVoices().length === 0 && <p className="small muted">{t('Sem vozes de síntese no sistema: a contagem vira bipes.')}</p>}
        </div>
      )}
    </div>
  )
}
