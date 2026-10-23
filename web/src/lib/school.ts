// Escola de controle: a lição em andamento, os objetivos cumpridos e as
// estrelas de cada lição (guardadas no navegador).

import { useSyncExternalStore } from 'react'
import type { LessonFile } from '../types'

export interface LessonRun {
  id: string
  step: number
  started: number
  hints: number
  /** Dica do objetivo atual à mostra. */
  hintOpen: boolean
  done: { step: number; at: number }[]
  finished?: number
}

type Listener = () => void

class School {
  run: LessonRun | null = null
  stars: Record<string, number> = load()
  version = 0
  private listeners = new Set<Listener>()

  subscribe = (l: Listener) => {
    this.listeners.add(l)
    return () => this.listeners.delete(l)
  }
  private emit() {
    this.version++
    for (const l of this.listeners) l()
  }

  start(id: string) {
    this.run = { id, step: 0, started: Date.now(), hints: 0, hintOpen: false, done: [] }
    this.emit()
  }

  stop() {
    this.run = null
    this.emit()
  }

  hint() {
    if (!this.run || this.run.hintOpen) return
    this.run = { ...this.run, hints: this.run.hints + 1, hintOpen: true }
    this.emit()
  }

  /** Objetivo atual cumprido: passa ao próximo, ou termina e dá as estrelas. */
  advance(lesson: LessonFile) {
    const r = this.run
    if (!r || r.finished) return
    const done = [...r.done, { step: r.step, at: Date.now() }]
    const step = r.step + 1
    if (step >= lesson.objectives.length) {
      const finished = Date.now()
      const n = starsFor((finished - r.started) / 1000, r.hints, lesson.par_s)
      this.stars = { ...this.stars, [r.id]: Math.max(this.stars[r.id] ?? 0, n) }
      save(this.stars)
      this.run = { ...r, step, done, finished, hintOpen: false }
    } else {
      this.run = { ...r, step, done, hintOpen: false }
    }
    this.emit()
  }
}

/** 3 estrelas sem dicas e no tempo de referência; 2 com até duas dicas ou até o dobro do tempo; 1 nos outros casos. */
export function starsFor(secs: number, hints: number, par?: number | null): number {
  const onTime = par == null || secs <= par
  if (hints === 0 && onTime) return 3
  if (hints <= 2 && (par == null || secs <= 2 * par)) return 2
  return 1
}

function load(): Record<string, number> {
  try {
    return JSON.parse(localStorage.getItem('jaci.escola') ?? '{}')
  } catch {
    return {}
  }
}

function save(s: Record<string, number>) {
  try {
    localStorage.setItem('jaci.escola', JSON.stringify(s))
  } catch {
    /* sem armazenamento */
  }
}

export const school = new School()

/** Redesenha o componente quando a lição muda. */
export function useSchool() {
  return useSyncExternalStore(school.subscribe, () => school.version)
}
