import { useEffect } from 'react'
import { Toasts } from './components/ui'
import { EventLog } from './components/EventLog'
import { TopBar } from './components/TopBar'
import { t } from './lib/i18n'
import { store, useStore, type View } from './lib/store'
import { Overview } from './views/Overview'
import { LaunchView } from './views/Launch'
import { Trajectory } from './views/Trajectory'
import { OrbitView } from './views/OrbitView'
import { Rendezvous } from './views/Rendezvous'
import { SystemsView } from './views/Systems'
import { Commands } from './views/Commands'
import { Telemetry } from './views/Telemetry'
import { Stations } from './views/Stations'
import { Interplanetary } from './views/Interplanetary'
import { Procedures } from './views/Procedures'
import { Instructor } from './views/Instructor'

const CONSOLES: { id: View; code: string; name: string }[] = [
  { id: 'overview', code: 'FLIGHT', name: 'Visão geral' },
  { id: 'launch', code: 'LCC', name: 'Lançamento' },
  { id: 'trajectory', code: 'BOOSTER', name: 'Subida e reentrada' },
  { id: 'orbit', code: 'FIDO', name: 'Órbita e manobras' },
  { id: 'rendezvous', code: 'RNDZ', name: 'Encontro' },
  { id: 'systems', code: 'EECOM', name: 'Subsistemas' },
  { id: 'commands', code: 'CMD', name: 'Comandos' },
  { id: 'telemetry', code: 'TLM', name: 'Telemetria e alarmes' },
  { id: 'stations', code: 'INCO', name: 'Estações de solo' },
  { id: 'interplanetary', code: 'NAV', name: 'Interplanetário' },
  { id: 'procedures', code: 'PROC', name: 'Procedimentos' },
  { id: 'instructor', code: 'SIM', name: 'Instrutor' },
]

export default function App() {
  useEffect(() => {
    store.connect()
  }, [])
  const view = useStore((s) => s.ui.view)
  const alarms = useStore((s) => s.snap?.alarms.filter((a) => !a.acked && a.active).length ?? 0)
  const connected = useStore((s) => s.connected)
  const hasSnap = useStore((s) => s.snap != null)
  useStore((s) => s.ui.lang)
  return (
    <div className="app">
      <TopBar />
      <div className="body">
        <nav className="consoles">
          {CONSOLES.map((c) => (
            <button key={c.id} className={view === c.id ? 'active' : ''} onClick={() => store.setUi({ view: c.id })}>
              <span className="code">{c.code}</span>
              <span>{t(c.name)}</span>
              {c.id === 'telemetry' && alarms > 0 && <span className="badge">{alarms}</span>}
            </button>
          ))}
        </nav>
        <main>
          {!hasSnap ? (
            <div className="muted" style={{ padding: 40, textAlign: 'center' }}>
              {connected ? t('Recebendo o estado da simulação…') : t('Conectando ao servidor do Jaci…')}
            </div>
          ) : (
            <Current view={view} />
          )}
        </main>
      </div>
      <EventLog />
      <Toasts />
    </div>
  )
}

function Current({ view }: { view: View }) {
  switch (view) {
    case 'overview':
      return <Overview />
    case 'launch':
      return <LaunchView />
    case 'trajectory':
      return <Trajectory />
    case 'orbit':
      return <OrbitView />
    case 'rendezvous':
      return <Rendezvous />
    case 'systems':
      return <SystemsView />
    case 'commands':
      return <Commands />
    case 'telemetry':
      return <Telemetry />
    case 'stations':
      return <Stations />
    case 'interplanetary':
      return <Interplanetary />
    case 'procedures':
      return <Procedures />
    case 'instructor':
      return <Instructor />
  }
}
