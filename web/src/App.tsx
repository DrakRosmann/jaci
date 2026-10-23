import { Component, Suspense, lazy, useEffect, useState, type ComponentType, type ReactNode } from 'react'
import { Toasts } from './components/ui'
import { EventLog } from './components/EventLog'
import { TopBar } from './components/TopBar'
import { ReplayBar } from './components/ReplayBar'
import { MissionClocks } from './components/MissionClocks'
import { JoinDialog } from './components/Room'
import { LessonPanel } from './components/LessonPanel'
import { Shortcuts } from './components/Shortcuts'
import { sound } from './lib/audio'
import { pilot } from './lib/pilot'
import { t, tx } from './lib/i18n'
import { utc } from './lib/format'
import { CONSOLES, CONSOLE_KEYS, WARPS } from './lib/consoles'
import { store, useStore, type View } from './lib/store'

// Cada console é carregado na primeira vez que é aberto: o three.js (vista 3D)
// e os consoles pesados não atrasam a abertura da interface.
type Lazy = React.LazyExoticComponent<ComponentType> & { preload: () => void }

function view<M>(load: () => Promise<M>, pick: (m: M) => ComponentType): Lazy {
  const C = lazy(() => load().then((m) => ({ default: pick(m) }))) as Lazy
  C.preload = () => void load()
  return C
}

const VIEWS: Record<View, Lazy> = {
  overview: view(() => import('./views/Overview'), (m) => m.Overview),
  launch: view(() => import('./views/Launch'), (m) => m.LaunchView),
  trajectory: view(() => import('./views/Trajectory'), (m) => m.Trajectory),
  orbit: view(() => import('./views/OrbitView'), (m) => m.OrbitView),
  rendezvous: view(() => import('./views/Rendezvous'), (m) => m.Rendezvous),
  systems: view(() => import('./views/Systems'), (m) => m.SystemsView),
  commands: view(() => import('./views/Commands'), (m) => m.Commands),
  telemetry: view(() => import('./views/Telemetry'), (m) => m.Telemetry),
  stations: view(() => import('./views/Stations'), (m) => m.Stations),
  interplanetary: view(() => import('./views/Interplanetary'), (m) => m.Interplanetary),
  procedures: view(() => import('./views/Procedures'), (m) => m.Procedures),
  instructor: view(() => import('./views/Instructor'), (m) => m.Instructor),
  ksp: view(() => import('./views/Ksp'), (m) => m.KspView),
  timeline: view(() => import('./views/Timeline'), (m) => m.Timeline),
  custom: view(() => import('./views/Custom'), (m) => m.Custom),
  wall: view(() => import('./views/Wall'), (m) => m.Wall),
  school: view(() => import('./views/School'), (m) => m.School),
  manual: view(() => import('./views/Manual'), (m) => m.Manual),
  vehicle3d: view(() => import('./views/VehicleView'), (m) => m.VehicleView),
}

/** Sem instantâneo há mais que isso, os dados são dados como congelados. */
const STALE_MS = 3000

export default function App() {
  useEffect(() => {
    store.connect()
    window.addEventListener('hashchange', store.followHash)
    window.addEventListener('popstate', store.followHash)
    return () => {
      window.removeEventListener('hashchange', store.followHash)
      window.removeEventListener('popstate', store.followHash)
    }
  }, [])
  const view = useStore((s) => s.ui.view)
  const alarms = useStore((s) => s.snap?.alarms.filter((a) => !a.acked && a.active).length ?? 0)
  const hasSnap = useStore((s) => s.snap != null)
  const collapsed = useStore((s) => s.ui.navCollapsed)
  const lang = useStore((s) => s.ui.lang)
  const [help, setHelp] = useState(false)
  const [joinOpen, setJoin] = useState(false)
  // Sala com posições ligada e esta tela sem posição: pergunta (até fecharem).
  const roomOn = useStore((s) => !!s.snap?.room?.on)
  const me = useStore((s) => s.me)
  const [dismissed, setDismissed] = useState(false)
  const join = joinOpen || (roomOn && !me && !dismissed)
  useTitle()
  useKeys(setHelp)
  useSound()
  useEffect(() => {
    document.documentElement.lang = lang === 'en' ? 'en' : 'pt-BR'
  }, [lang])
  const theme = useStore((s) => s.ui.theme)
  const clocks = useStore((s) => s.ui.clocks)
  useEffect(() => {
    // Automático: claro ou escuro como o sistema, e muda junto com ele.
    if (theme !== 'auto') {
      document.documentElement.dataset.theme = theme
      return
    }
    const mq = window.matchMedia('(prefers-color-scheme: light)')
    const apply = () => (document.documentElement.dataset.theme = mq.matches ? 'claro' : 'moderno')
    apply()
    mq.addEventListener('change', apply)
    return () => mq.removeEventListener('change', apply)
  }, [theme])
  const Current = VIEWS[view]
  // Telão: tela cheia, sem menus nem barras.
  if (view === 'wall') {
    return (
      <>
        {hasSnap ? (
          <Suspense fallback={null}>
            <Current />
          </Suspense>
        ) : (
          <Waiting />
        )}
        <Toasts />
      </>
    )
  }
  return (
    <div className="app">
      <TopBar onHelp={() => setHelp(true)} onRoom={() => setJoin(true)} />
      {clocks && hasSnap && <MissionClocks />}
      <ReplayBar />
      <div className={`body ${collapsed ? 'collapsed' : ''}`}>
        <nav className="consoles" aria-label={t('Consoles')}>
          <button className="collapse" onClick={() => store.setUi({ navCollapsed: !collapsed })} title={collapsed ? t('Mostrar os nomes dos consoles') : t('Recolher o menu')} aria-label={collapsed ? t('Mostrar os nomes dos consoles') : t('Recolher o menu')}>
            {collapsed ? '»' : '«'}
          </button>
          {CONSOLES.map((c, i) => (
            <button
              key={c.id}
              className={view === c.id ? 'active' : ''}
              aria-current={view === c.id ? 'page' : undefined}
              onClick={() => store.setUi({ view: c.id })}
              title={`${t(c.name)} (${CONSOLE_KEYS[i]})`}
              onMouseEnter={VIEWS[c.id].preload}
            >
              <span className="code">{c.code}</span>
              <span className="name">{t(c.name)}</span>
              {c.id === 'telemetry' && alarms > 0 && (
                <span className="badge" aria-label={t('{n} alarmes não reconhecidos', { n: alarms })}>
                  {alarms}
                </span>
              )}
            </button>
          ))}
        </nav>
        <main>
          <Stale />
          {!hasSnap ? (
            <Waiting />
          ) : (
            <ViewBoundary key={view}>
              <Suspense fallback={<div className="muted loading">{t('Carregando o console…')}</div>}>
                <Current />
              </Suspense>
            </ViewBoundary>
          )}
        </main>
      </div>
      <EventLog />
      <LessonPanel />
      <Toasts />
      {help && <Shortcuts onClose={() => setHelp(false)} />}
      {join && (
        <JoinDialog
          onClose={() => {
            setJoin(false)
            setDismissed(true)
          }}
        />
      )}
    </div>
  )
}

function Waiting() {
  const connected = useStore((s) => s.connected)
  return <div className="muted loading">{connected ? t('Recebendo o estado da simulação…') : t('Conectando ao servidor do Jaci…')}</div>
}

/** Faixa de aviso quando a conexão cai ou os instantâneos param de chegar. */
function Stale() {
  const connected = useStore((s) => s.connected)
  const hasSnap = useStore((s) => s.snap != null)
  const last = useStore((s) => s.lastSnapAt)
  const t0 = useStore((s) => s.snap?.t)
  const [now, setNow] = useState(() => Date.now())
  useEffect(() => {
    const h = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(h)
  }, [])
  if (!hasSnap) return null
  const frozen = !connected || now - last > STALE_MS
  if (!frozen) return null
  return (
    <div className="banner alarm stale" role="alert">
      <b>{connected ? t('Telemetria parada') : t('Sem conexão com o servidor')}</b> — {t('os dados abaixo estão congelados desde')} <span className="mono">{t0 != null ? utc(t0) : '—'}</span>
      {!connected && <> · {t('reconectando…')}</>}
    </div>
  )
}

/** Um erro num console não derruba a interface inteira. */
class ViewBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null }
  static getDerivedStateFromError(error: Error) {
    return { error }
  }
  componentDidCatch(error: Error) {
    console.error(error)
  }
  render() {
    if (!this.state.error) return this.props.children
    return (
      <div className="banner alarm">
        <b>{t('Este console falhou')}</b>: <span className="mono">{String(this.state.error.message)}</span>
        <div className="row" style={{ marginTop: 8 }}>
          <button onClick={() => this.setState({ error: null })}>{t('Tentar de novo')}</button>
          <button onClick={() => location.reload()}>{t('Recarregar a página')}</button>
        </div>
      </div>
    )
  }
}

/** Liga os sons aos instantâneos e destrava o áudio no primeiro clique. */
function useSound() {
  useEffect(() => {
    let last: typeof store.snap = null
    const off = store.subscribe(() => {
      const s = store.snap
      if (!s) {
        if (last) sound.reset()
        last = null
        return
      }
      if (s === last) return
      last = s
      sound.update(s, s.events, store.ui.lang, tx, store.me?.position)
    })
    const unlock = () => sound.prefs.on && sound.unlock()
    window.addEventListener('pointerdown', unlock)
    window.addEventListener('keydown', unlock)
    return () => {
      off()
      window.removeEventListener('pointerdown', unlock)
      window.removeEventListener('keydown', unlock)
      sound.stopAlarm()
    }
  }, [])
}

/** Título da aba: alarmes e contagem, para ver de outra janela. */
function useTitle() {
  const alarms = useStore((s) => s.snap?.alarms.filter((a) => !a.acked && a.active).length ?? 0)
  const clock = useStore((s) => {
    const lc = s.snap?.launch
    return lc && lc.vehicle != null && lc.state !== 'launched' && lc.state !== 'idle' ? lc.clock : null
  })
  const mission = useStore((s) => s.snap?.mission)
  useEffect(() => {
    const parts = [alarms > 0 ? `(${alarms}) ` : '', clock ? `${clock} · ` : '', 'Jaci', mission ? ` — ${mission}` : '']
    document.title = parts.join('')
  }, [alarms, clock, mission])
}

/** Atalhos de teclado (fora de campos de texto). Nenhum manda comando a um veículo. */
function useKeys(setHelp: (b: boolean) => void) {
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey) return
      const el = e.target as HTMLElement | null
      if (el && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName))) return
      // Pilotando pelo teclado: as teclas de voo não são atalhos.
      if (pilot.active && pilot.keys().includes(e.key.toLowerCase())) return
      const i = CONSOLE_KEYS.indexOf(e.key)
      if (i >= 0 && CONSOLES[i]) {
        store.setUi({ view: CONSOLES[i].id })
      } else if (e.key === ' ') {
        if (el?.tagName === 'BUTTON') return
        store.sim(store.snap?.paused ? 'resume' : 'pause')
      } else if (e.key === '[' || e.key === ']') {
        const cur = store.snap?.warp ?? 1
        const idx = WARPS.findIndex((w) => w >= cur)
        const at = idx < 0 ? WARPS.length - 1 : idx
        const next = e.key === ']' ? WARPS[Math.min(at + (WARPS[at] === cur ? 1 : 0), WARPS.length - 1)] : WARPS[Math.max(at - 1, 0)]
        store.sim('warp', { value: next })
      } else if (e.key === 'e') {
        store.setUi({ eventsOpen: !store.ui.eventsOpen })
      } else if (e.key === 'm') {
        sound.setPrefs({ on: !sound.prefs.on })
      } else if (e.key === 'l') {
        store.setUi({ lang: store.ui.lang === 'pt' ? 'en' : 'pt' })
      } else if (e.key === '?') {
        setHelp(true)
      } else if (e.key === 'w') {
        store.setUi({ view: store.ui.view === 'wall' ? 'overview' : 'wall' })
      } else if (e.key === 'Escape') {
        setHelp(false)
        if (store.ui.view === 'wall') store.setUi({ view: 'overview' })
      } else {
        return
      }
      e.preventDefault()
    }
    window.addEventListener('keydown', down)
    return () => window.removeEventListener('keydown', down)
  }, [setHelp])
}
