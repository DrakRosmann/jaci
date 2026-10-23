// Janela com os atalhos de teclado (tecla ?).

import { useEffect, useRef } from 'react'
import { CONSOLES, CONSOLE_KEYS } from '../lib/consoles'
import { t } from '../lib/i18n'

export function Shortcuts({ onClose }: { onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    ref.current?.focus()
  }, [])
  const rows: [string, string][] = [
    ...CONSOLES.map((c, i) => [CONSOLE_KEYS[i], `${c.code} · ${t(c.name)}`] as [string, string]),
    [t('espaço'), t('Pausar ou continuar a simulação')],
    ['[  ]', t('Diminuir ou aumentar a aceleração do tempo')],
    ['e', t('Mostrar ou esconder os eventos')],
    ['w', t('Telão (tela cheia para a parede da sala)')],
    ['m', t('Ligar ou desligar o som')],
    ['l', 'Português / English'],
    ['?', t('Esta ajuda')],
    ['Esc', t('Fechar')],
  ]
  return (
    <div className="modal-back" onClick={onClose}>
      <div className="modal" role="dialog" aria-modal="true" aria-label={t('Atalhos de teclado')} tabIndex={-1} ref={ref} onClick={(e) => e.stopPropagation()}>
        <h3>
          {t('Atalhos de teclado')}
          <button className="small" onClick={onClose} aria-label={t('Fechar')}>
            ✕
          </button>
        </h3>
        <table>
          <tbody>
            {rows.map(([k, what]) => (
              <tr key={k}>
                <td className="mono">
                  <kbd>{k}</kbd>
                </td>
                <td>{what}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="small muted">{t('Nenhum atalho manda comandos aos veículos.')}</p>
      </div>
    </div>
  )
}
