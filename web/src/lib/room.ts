// Sala de controle com posições: o que a posição desta tela pode fazer. As
// regras vêm do servidor (que confere tudo de novo); aqui só servem para
// apagar o que não é da posição.

import type { CmdDef, Role } from '../types'
import { store } from './store'

/** Posição desta tela (OBS sem posição); null com a sala desligada. */
export function myPosition(): string | null {
  if (!store.snap?.room?.on) return null
  return store.me?.position ?? 'OBS'
}

export function role(id: string | null | undefined): Role | undefined {
  return store.hello?.roles?.find((r) => r.id === id)
}

/** Pode mandar este comando? (Sala desligada: sim.) */
export function canCommand(def: Pick<CmdDef, 'id' | 'group'> | undefined, launchPosition?: string): boolean {
  const pos = myPosition()
  if (pos == null || !def) return true
  if (def.id === 'vote' || def.id === 'position') return canVote(launchPosition ?? '')
  return !!role(pos)?.groups.some((g) => g === '*' || g === def.group)
}

const ANYONE = ['ack', 'proc_log', 'saves', 'recordings']

export function canSim(action: string): boolean {
  const pos = myPosition()
  if (pos == null) return true
  if (pos === 'OBS') return false
  return ANYONE.includes(action) || !!role(pos)?.sim.some((a) => a === '*' || a === action)
}

/** Pode votar (ou forçar) esta posição do lançamento? */
export function canVote(launchPosition: string): boolean {
  const pos = myPosition()
  if (pos == null) return true
  return pos === 'FLIGHT' || pos === 'LCC' || !!role(pos)?.launch.includes(launchPosition)
}

/** As posições do lançamento que esta tela ocupa (para votar). */
export function myLaunchPositions(): string[] {
  const pos = myPosition()
  return pos ? (role(pos)?.launch ?? []) : []
}
