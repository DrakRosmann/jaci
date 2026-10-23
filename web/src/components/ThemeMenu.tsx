// Visual das telas, inspirado nas salas reais: o moderno, o fósforo verde dos
// consoles do Apollo e o âmbar dos monitores dos anos 70 e 80; e a faixa de
// relógios da sala.

import { useEffect, useRef, useState } from 'react'
import { t } from '../lib/i18n'
import { store, useStore, type UiState } from '../lib/store'

const THEMES: [UiState['theme'], string, string][] = [
  ['moderno', 'Moderno', 'O de sempre: painéis escuros e cores de alerta'],
  ['fosforo', 'Fósforo (Apollo)', 'Verde de tubo e letras de máquina, como os consoles do MOCR'],
  ['ambar', 'Âmbar', 'Monitores âmbar, como os terminais dos anos 70 e 80'],
]

export function ThemeMenu() {
  const theme = useStore((s) => s.ui.theme)
  const clocks = useStore((s) => s.ui.clocks)
  const [open, setOpen] = useState(false)
  const box = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const close = (e: MouseEvent) => box.current && !box.current.contains(e.target as Node) && setOpen(false)
    window.addEventListener('mousedown', close)
    return () => window.removeEventListener('mousedown', close)
  }, [open])
  return (
    <div className="sound" ref={box}>
      <button className="small" onClick={() => setOpen(!open)} aria-expanded={open} aria-label={t('Tela')} title={t('Tela: telão, janela nova, visual e relógios')}>
        🖥
      </button>
      {open && (
        <div className="menu" role="dialog" aria-label={t('Tela')}>
          <div className="row">
            <button
              className="small"
              onClick={() => {
                setOpen(false)
                store.setUi({ view: 'wall' })
              }}
              title={`${t('Telão: tela cheia para a parede da sala')} (w)`}
            >
              ⛶ {t('Telão')}
            </button>
            <button className="small" onClick={() => window.open(location.href, '_blank', 'noopener')} title={t('Abrir este console numa janela nova (para outro monitor)')}>
              ⧉ {t('Janela nova')}
            </button>
          </div>
          <b>{t('Visual das telas')}</b>
          {THEMES.map(([id, name, desc]) => (
            <label key={id} title={t(desc)}>
              <input type="radio" name="theme" checked={theme === id} onChange={() => store.setUi({ theme: id })} /> {t(name)}
            </label>
          ))}
          <label>
            <input type="checkbox" checked={clocks} onChange={(e) => store.setUi({ clocks: e.target.checked })} /> {t('Faixa de relógios da sala')}
          </label>
        </div>
      )}
    </div>
  )
}
