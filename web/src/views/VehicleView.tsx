// A nave em 3D numa aba própria: o modelo grande, com a atitude no
// referencial local, e ao lado os estágios e os motores.

import { Suspense, lazy } from 'react'
import { Empty, Panel } from '../components/ui'
import { t, tx } from '../lib/i18n'
import { num } from '../lib/format'
import { useStore } from '../lib/store'

const Vehicle3D = lazy(() => import('../components/Vehicle3D').then((m) => ({ default: m.Vehicle3D })))

export function VehicleView() {
  const v = useStore((s) => s.selected)
  if (!v) return <Empty>{t('Selecione um veículo.')}</Empty>
  return (
    <div className="grid cols-3">
      <Panel title={`${t('Nave em 3D')} · ${v.name}`} className="span-2" flush>
        <Suspense fallback={<div className="loading muted" style={{ height: 620 }}>{t('Carregando a vista 3D…')}</div>}>
          <Vehicle3D v={v} height={620} />
        </Suspense>
      </Panel>
      <Panel title={t('Estágios e motores')} flush>
        <table>
          <tbody>
            {(v.stages ?? [])
              .slice()
              .reverse()
              .map((s, i) => (
                <tr key={i}>
                  <td>
                    <b>{tx(s.name)}</b>
                    <div className="small muted">
                      {s.diameter != null && `${num(s.diameter, 2)} m × ${num(s.length, 1)} m · `}
                      {s.capacity ? `${num(((s.prop ?? 0) / s.capacity) * 100, 0)} % ${t('propelente')}` : t('sem propelente')}
                    </div>
                    <div className="small">
                      {(s.engines ?? []).map((e, k) => (
                        <span key={k} className={`eng ${e.state}`} title={`${tx(e.name)} · ${num(e.thrust, 0)} kN`}>
                          ●
                        </span>
                      ))}
                    </div>
                  </td>
                </tr>
              ))}
          </tbody>
        </table>
        <p className="small muted" style={{ padding: '0 10px' }}>
          {t('Arraste para girar a câmera, role para aproximar. A Terra fica embaixo: a atitude é a do referencial local.')}
        </p>
      </Panel>
    </div>
  )
}
