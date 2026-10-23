// Telão: a parede da sala de controle. Tela cheia, letra grande, sem menus:
// relógios, mapa da frota, contagem, alarmes, estado dos veículos e os
// últimos eventos. Abre em /#telao; Esc ou o botão volta aos consoles.

import { useEffect, useRef, useState } from 'react'
import { AlarmsWidget, CountdownWidget, EventsWidget, FleetWidget, MapWidget } from '../components/Widgets'
import { MissionClocks } from '../components/MissionClocks'
import { utc } from '../lib/format'
import { useUpcoming } from '../lib/upcoming'
import { t, tx } from '../lib/i18n'
import { store, useStore } from '../lib/store'

export function Wall() {
  const mission = useStore((s) => s.snap?.mission)
  const counting = useStore((s) => {
    const lc = s.snap?.launch
    return !!lc && lc.vehicle != null && lc.state !== 'idle' && lc.state !== 'launched'
  })
  const red = useStore((s) => (s.snap?.alarms ?? []).some((a) => a.active && !a.acked && a.level === 'red'))
  const connected = useStore((s) => s.connected)
  // O mapa ocupa o que couber no espaço dele, na proporção 2:1.
  const mapBox = useRef<HTMLElement>(null)
  const [mapH, setMapH] = useState(400)
  useEffect(() => {
    const el = mapBox.current
    if (!el) return
    const fit = () => setMapH(Math.max(150, Math.floor(Math.min(el.clientHeight, el.clientWidth / 2))))
    const ro = new ResizeObserver(fit)
    ro.observe(el)
    fit()
    return () => ro.disconnect()
  }, [])
  return (
    <div className={`wall ${red ? 'red' : ''}`}>
      <header>
        <img src="/jaci.svg" alt="" width={34} height={34} />
        <b>JACI</b>
        <span className="mission">{tx(mission)}</span>
        {!connected && <span className="bad">{t('Sem conexão com o servidor')}</span>}
        <span style={{ flex: 1 }} />
        <button className="small" onClick={() => store.setUi({ view: 'overview' })} title={t('Voltar aos consoles (Esc)')}>
          ✕
        </button>
      </header>
      <MissionClocks />
      <div className="wall-grid">
        <section className="wall-map" ref={mapBox}>
          <MapWidget height={mapH} />
        </section>
        <section className="wall-side">
          {counting && (
            <div className="wall-block">
              <CountdownWidget big />
            </div>
          )}
          <div className="wall-block">
            <h2>{t('Alarmes')}</h2>
            <AlarmsWidget big />
          </div>
          <div className="wall-block">
            <h2>{t('Frota')}</h2>
            <FleetWidget big />
          </div>
          <WorkProgram />
        </section>
        <section className="wall-events">
          <EventsWidget count={8} />
        </section>
      </div>
    </div>
  )
}

/** Programa de trabalho (como o painel do TsUP): os próximos eventos com a hora. */
function WorkProgram() {
  const up = useUpcoming(12 * 3_600)
  const now = useStore((s) => s.snap?.t ?? 0)
  const name = useStore((s) => s.selected?.name)
  const list = up.events.filter((e) => e.t > now).slice(0, 8)
  if (list.length === 0) return null
  return (
    <div className="wall-block program">
      <h2>
        {t('Programa de trabalho')} · {name}
      </h2>
      <table>
        <tbody>
          {list.map((e, i) => (
            <tr key={i}>
              <td className="mono">{utc(e.t, false).slice(0, 5)}</td>
              <td>{e.text}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
