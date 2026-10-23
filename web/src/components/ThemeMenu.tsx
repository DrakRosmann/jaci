// Visual das telas, inspirado nas salas reais: o moderno, o claro (para a sala
// de dia), o automático (que segue o sistema), o fósforo verde dos consoles do
// Apollo, o âmbar dos monitores dos anos 70 e 80, o vermelho da sala no escuro
// e o alto contraste; e a faixa de relógios da sala.

import { useEffect, useRef, useState } from 'react'
import { t } from '../lib/i18n'
import { store, useStore, type UiState } from '../lib/store'

/** Tema, nome, descrição e as cores da amostra: fundo, painel, texto, destaque e alerta. */
const THEMES: [UiState['theme'], string, string, string[]][] = [
  ['moderno', 'Moderno', 'O de sempre: painéis escuros e cores de alerta', ['#070c18', '#0e1628', '#d9e2f2', '#4cc3ff', '#ff8a3d']],
  ['claro', 'Claro', 'Painéis claros para a sala iluminada ou de dia', ['#e9eef5', '#ffffff', '#182131', '#0b6dc0', '#d0600e']],
  ['auto', 'Automático', 'Claro ou escuro, como o sistema', ['#070c18', '#ffffff', '#d9e2f2', '#0b6dc0', '#ff8a3d']],
  ['fosforo', 'Fósforo (Apollo)', 'Verde de tubo e letras de máquina, como os consoles do MOCR', ['#010603', '#03100a', '#8dffb2', '#5cffc0', '#d6ff8a']],
  ['ambar', 'Âmbar', 'Monitores âmbar, como os terminais dos anos 70 e 80', ['#080400', '#120a01', '#ffbf47', '#ffcf66', '#ffd27a']],
  ['noite', 'Noite', 'Só vermelho e pouca luz, para não ofuscar a sala no escuro', ['#050000', '#0e0202', '#ff6b57', '#ff5b45', '#ffd2c8']],
  ['contraste', 'Alto contraste', 'Preto e branco com bordas grossas, para ler de longe ou com pouca visão', ['#000000', '#000000', '#ffffff', '#4fc3ff', '#ffe100']],
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
          <div className="theme-cards" role="radiogroup" aria-label={t('Visual das telas')}>
            {THEMES.map(([id, name, desc, sw]) => (
              <button key={id} role="radio" aria-checked={theme === id} className={`theme-card ${theme === id ? 'on' : ''}`} onClick={() => store.setUi({ theme: id })} title={t(desc)}>
                <span className="swatch" style={{ background: sw[0] }}>
                  <span className="sw-panel" style={{ background: sw[1], borderColor: sw[3] }}>
                    <span style={{ background: sw[2] }} />
                    <span style={{ background: sw[3] }} />
                    <span style={{ background: sw[4] }} />
                  </span>
                </span>
                <span className="name">{t(name)}</span>
              </button>
            ))}
          </div>
          <label>
            <input type="checkbox" checked={clocks} onChange={(e) => store.setUi({ clocks: e.target.checked })} /> {t('Faixa de relógios da sala')}
          </label>
        </div>
      )}
    </div>
  )
}
