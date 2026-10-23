// A lição em andamento da escola de controle: um cartão sobre qualquer
// console, com o objetivo atual, a dica, o "me leve lá" e o progresso. A
// condição de cada objetivo é conferida a cada instantâneo.

import { useEffect, useMemo } from 'react'
import { t, tx } from '../lib/i18n'
import { compile } from '../lib/expr'
import { duration } from '../lib/format'
import { school, starsFor, useSchool } from '../lib/school'
import { VIEWS, store, useStore, type View } from '../lib/store'
import type { LessonFile, Snapshot, Vehicle } from '../types'

/** Veículo das condições: o do objetivo, o da lição, o da plataforma ou o selecionado. */
function target(lesson: LessonFile, step: number, s: Snapshot): Vehicle | undefined {
  const name = lesson.objectives[step]?.vehicle ?? lesson.vehicle
  if (name) return s.vehicles.find((v) => v.name === name)
  return (s.launch.vehicle != null ? s.vehicles.find((v) => v.id === s.launch.vehicle) : undefined) ?? store.selected
}

export function LessonPanel() {
  useSchool()
  const run = school.run
  const lessons = useStore((s) => s.hello?.lessons)
  const lesson = lessons?.find((l) => l.id === run?.id)
  // Relógio do cartão: o do último instantâneo (redesenha junto com ele).
  const now = useStore((s) => s.lastSnapAt)
  const checks = useMemo(() => {
    if (!lesson) return []
    return lesson.objectives.map((o) => {
      try {
        return compile(o.until)
      } catch {
        return () => false
      }
    })
  }, [lesson])
  // Confere o objetivo atual a cada mudança do estado.
  useEffect(() => {
    if (!lesson) return
    const check = () => {
      const r = school.run
      const s = store.snap
      if (!r || r.finished || !s) return
      // Só depois que o cenário da lição carregou (não vale o mundo de antes).
      if (lesson.scenario && s.scenario !== lesson.scenario) return
      const v = target(lesson, r.step, s)
      if (!v) return
      const ui = { view: store.ui.view, selected: store.selected?.name }
      if (checks[r.step]?.(v, s, ui)) school.advance(lesson)
    }
    check()
    const off = store.subscribe(check)
    return () => {
      off()
    }
  }, [lesson, checks])
  if (!run || !lesson) return null
  const obj = lesson.objectives[run.step]
  const go = obj?.console && (VIEWS as string[]).includes(obj.console) ? (obj.console as View) : null
  const elapsed = Math.max(0, ((run.finished ?? now) - run.started) / 1000)
  return (
    <aside className="lesson-panel" role="complementary" aria-label={t('Lição')}>
      <header>
        <span className="pill replay-tag">{t('ESCOLA')}</span>
        <b>{tx(lesson.name)}</b>
        <span className="muted small">
          {Math.min(run.step, lesson.objectives.length)}/{lesson.objectives.length}
        </span>
        <button className="small" onClick={() => school.stop()} aria-label={t('Sair da lição')} title={t('Sair da lição')}>
          ✕
        </button>
      </header>
      <ol className="lesson-steps">
        {lesson.objectives.map((o, i) => (
          <li key={i} className={i < run.step ? 'ok' : i === run.step ? 'now' : ''}>
            {i < run.step ? '✓ ' : ''}
            {tx(o.text)}
          </li>
        ))}
      </ol>
      {run.finished ? (
        <div className="lesson-done">
          <div className="stars" aria-label={`${starsFor(elapsed, run.hints, lesson.par_s)} ${t('estrelas')}`}>
            {'★'.repeat(starsFor(elapsed, run.hints, lesson.par_s))}
            <span className="muted">{'★'.repeat(3 - starsFor(elapsed, run.hints, lesson.par_s))}</span>
          </div>
          <p>{tx(lesson.done)}</p>
          <p className="small muted">
            {t('Tempo')}: {duration(elapsed)} · {t('dicas')}: {run.hints}
          </p>
          <div className="row">
            <button className="primary" onClick={() => store.setUi({ view: 'school' })}>
              {t('Voltar à escola')}
            </button>
            <button onClick={() => school.stop()}>{t('Fechar')}</button>
          </div>
        </div>
      ) : (
        <div className="row">
          {go && store.ui.view !== go && (
            <button className="small primary" onClick={() => store.setUi({ view: go })}>
              {t('Me leve lá')}
            </button>
          )}
          {obj?.hint && !run.hintOpen && (
            <button className="small" onClick={() => school.hint()}>
              💡 {t('Dica')}
            </button>
          )}
          <span className="muted small mono">{duration(elapsed)}</span>
        </div>
      )}
      {run.hintOpen && obj?.hint && !run.finished && <p className="hint small">💡 {tx(obj.hint)}</p>}
    </aside>
  )
}
