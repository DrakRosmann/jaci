// Geometria do mapa: projeção, círculos de visibilidade, terminador e
// quebra de traços na linha de data.

export const R_EARTH = 6378.137

/** Divide uma sequência de (lon, lat) onde ela cruza ±180°. */
export function splitDateline(points: [number, number][]): [number, number][][] {
  const out: [number, number][][] = []
  let cur: [number, number][] = []
  for (let i = 0; i < points.length; i++) {
    const p = points[i]
    if (cur.length && Math.abs(p[0] - cur[cur.length - 1][0]) > 180) {
      out.push(cur)
      cur = []
    }
    cur.push(p)
  }
  if (cur.length) out.push(cur)
  return out
}

/** Raio (graus de arco na superfície) do círculo visto a `alt_km` com elevação mínima. */
export function footprintDeg(altKm: number, minElDeg = 0): number {
  const el = (minElDeg * Math.PI) / 180
  const rho = Math.asin(R_EARTH / (R_EARTH + Math.max(altKm, 1)))
  const eta = Math.asin(Math.sin(rho) * Math.cos(el))
  const lambda = Math.PI / 2 - el - eta
  return (lambda * 180) / Math.PI
}

/** Pontos de um círculo de raio `deg` em torno de (lat, lon). */
export function circle(lat: number, lon: number, deg: number, n = 72): [number, number][] {
  const out: [number, number][] = []
  const la = (lat * Math.PI) / 180
  const lo = (lon * Math.PI) / 180
  const d = (deg * Math.PI) / 180
  for (let i = 0; i <= n; i++) {
    const b = (i / n) * 2 * Math.PI
    const lat2 = Math.asin(Math.sin(la) * Math.cos(d) + Math.cos(la) * Math.sin(d) * Math.cos(b))
    const lon2 = lo + Math.atan2(Math.sin(b) * Math.sin(d) * Math.cos(la), Math.cos(d) - Math.sin(la) * Math.sin(lat2))
    out.push([wrapLon((lon2 * 180) / Math.PI), (lat2 * 180) / Math.PI])
  }
  return out
}

export function wrapLon(lon: number): number {
  return ((((lon + 180) % 360) + 360) % 360) - 180
}

/** Ponto subsolar (lat, lon) a partir da direção do Sol no inercial e do GMST. */
export function subsolar(sun: [number, number, number], gmst: number): [number, number] {
  const [x, y, z] = sun
  const lat = Math.asin(z / Math.hypot(x, y, z))
  const ra = Math.atan2(y, x)
  return [(lat * 180) / Math.PI, wrapLon(((ra - gmst) * 180) / Math.PI)]
}

/** Latitude do terminador em cada longitude (para pintar a noite). */
export function terminator(subLat: number, subLon: number, lon: number): number {
  const d = ((lon - subLon) * Math.PI) / 180
  const la = (subLat * Math.PI) / 180
  // tan(lat) = -cos(d) / tan(subLat)
  const t = Math.atan(-Math.cos(d) / Math.tan(Math.abs(la) < 1e-6 ? 1e-6 : la))
  return (t * 180) / Math.PI
}

/** Distância de grande círculo (km). */
export function greatCircle(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const p1 = (lat1 * Math.PI) / 180
  const p2 = (lat2 * Math.PI) / 180
  const dl = ((lon2 - lon1) * Math.PI) / 180
  const a = Math.sin((p2 - p1) / 2) ** 2 + Math.cos(p1) * Math.cos(p2) * Math.sin(dl / 2) ** 2
  return 2 * 6371 * Math.asin(Math.min(1, Math.sqrt(a)))
}
