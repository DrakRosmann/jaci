// Histórico de telemetria em buffers circulares, por veículo e por ponto.

interface Series {
  t: Float64Array
  values: Map<string, Float32Array>
  head: number
  len: number
}

export class History {
  private data = new Map<number, Series>()
  readonly capacity: number

  constructor(capacity: number) {
    this.capacity = capacity
  }

  clear() {
    this.data.clear()
  }

  push(vehicle: number, t: number, tm: Record<string, number | null>) {
    let s = this.data.get(vehicle)
    if (!s) {
      s = { t: new Float64Array(this.capacity), values: new Map(), head: 0, len: 0 }
      this.data.set(vehicle, s)
    }
    // Instantes repetidos (simulação pausada) não entram.
    if (s.len > 0 && s.t[(s.head - 1 + this.capacity) % this.capacity] === t) return
    const i = s.head
    s.t[i] = t
    for (const [k, v] of Object.entries(tm)) {
      let arr = s.values.get(k)
      if (!arr) {
        arr = new Float32Array(this.capacity).fill(NaN)
        s.values.set(k, arr)
      }
      arr[i] = v ?? NaN
    }
    s.head = (s.head + 1) % this.capacity
    s.len = Math.min(s.len + 1, this.capacity)
  }

  /** Pontos (t, valor) de um ponto, do mais antigo ao mais novo, desde `since`. */
  series(vehicle: number, id: string, since = -Infinity): [number[], number[]] {
    const s = this.data.get(vehicle)
    const arr = s?.values.get(id)
    if (!s || !arr) return [[], []]
    const ts: number[] = []
    const vs: number[] = []
    for (let k = 0; k < s.len; k++) {
      const i = (s.head - s.len + k + this.capacity) % this.capacity
      if (s.t[i] < since) continue
      ts.push(s.t[i])
      vs.push(arr[i])
    }
    return [ts, vs]
  }

  keys(vehicle: number): string[] {
    return [...(this.data.get(vehicle)?.values.keys() ?? [])]
  }
}
