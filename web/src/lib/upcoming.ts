// Próximos acontecimentos do veículo selecionado (do pedido "timeline"),
// para a faixa de relógios e o programa de trabalho do telão.

import { useEffect, useState } from 'react'
import { tx, t } from './i18n'
import { store, useStore } from './store'
import type { Pass } from '../types'

export interface Upcoming {
  passes: Pass[]
  events: { t: number; text: string }[]
}

export function useUpcoming(span = 6 * 3_600): Upcoming {
  const vid = useStore((s) => s.ui.selected)
  const replay = useStore((s) => s.snap?.replay != null)
  const [data, setData] = useState<Upcoming>({ passes: [], events: [] })
  useEffect(() => {
    if (vid == null || replay) return
    let alive = true
    const load = () =>
      store
        .request<any>({ type: 'timeline', vehicle: vid, span })
        .then((d) => {
          if (!alive) return
          const events = [
            ...(d.countdown ?? []).map((c: any) => ({ t: c.t, text: tx(c.name) })),
            ...(d.burns ?? []).map((b: any) => ({ t: b.t, text: `${tx(b.label)} (${b.dv.toFixed(1)} m/s)` })),
            ...(d.commands ?? []).map((c: any) => ({ t: c.t, text: tx(c.text) })),
            ...(d.passes ?? []).map((p: Pass) => ({ t: p.aos, text: `${t('AOS')} ${tx(p.station)}` })),
            ...(d.eclipses ?? []).map((e: [number, number]) => ({ t: e[0], text: t('Entrada no eclipse') })),
            ...(d.apsides ?? []).map((a: any) => ({ t: a.t, text: a.kind === 'pe' ? t('Periastro') : t('Apoastro') })),
          ].sort((a, b) => a.t - b.t)
          setData({ passes: d.passes ?? [], events })
        })
        .catch(() => {})
    load()
    const h = setInterval(load, 30_000)
    return () => {
      alive = false
      clearInterval(h)
    }
  }, [vid, span, replay])
  return data
}
