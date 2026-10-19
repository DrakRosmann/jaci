// Ganchos que pedem dados ao servidor de tempos em tempos.

import { useEffect, useState } from 'react'
import { store } from './store'

/** Ponto previsto: t, lat, lon, alt (km), x, y, z (m, inercial no corpo central). */
export type PredPoint = [number, number, number, number, number, number, number]

/** Trajetória prevista de um veículo, renovada a cada `everyMs`. */
export function usePrediction(vehicle: number | null | undefined, span: number, everyMs = 4000, step?: number): { points: PredPoint[]; central: string | null } {
  const [data, setData] = useState<{ points: PredPoint[]; central: string | null }>({ points: [], central: null })
  useEffect(() => {
    if (vehicle == null) return
    let alive = true
    const load = () =>
      store
        .request<{ points: PredPoint[]; central: string }>({ type: 'predict', vehicle, span, step })
        .then((d) => alive && setData({ points: d.points, central: d.central }))
        .catch(() => {})
    load()
    const h = setInterval(load, everyMs)
    return () => {
      alive = false
      clearInterval(h)
    }
  }, [vehicle, span, everyMs, step])
  return data
}

/** Repete uma função a cada `ms` enquanto o componente existe. */
export function useInterval(fn: () => void, ms: number, deps: unknown[] = []) {
  useEffect(() => {
    fn()
    const h = setInterval(fn, ms)
    return () => clearInterval(h)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps)
}
