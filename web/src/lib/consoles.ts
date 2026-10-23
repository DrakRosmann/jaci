// Os consoles da sala de controle, na ordem do menu, e as teclas de cada um.

import type { View } from './store'

export const CONSOLES: { id: View; code: string; name: string }[] = [
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
  { id: 'ksp', code: 'KSP', name: 'Kerbal Space Program' },
  { id: 'timeline', code: 'FAO', name: 'Linha do tempo' },
  { id: 'custom', code: 'MEU', name: 'Meu console' },
]

/** Teclas dos consoles, na ordem do menu. */
export const CONSOLE_KEYS = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '-', '=', '\\', 't', 'c']

/** Acelerações do tempo da barra de cima (e das teclas [ e ]). */
export const WARPS = [1, 10, 100, 1_000, 10_000, 100_000]
