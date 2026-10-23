// Cores do tema para quem desenha em canvas: as mesmas variáveis do CSS.

/** Cor do tema (variável CSS), ou a padrão. */
export function cssVar(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim()
  return v || fallback
}

/** As cores de texto, linhas e fundo dos gráficos, do tema em uso. */
export function chartColors() {
  return {
    text: cssVar('--text', '#d9e2f2'),
    muted: cssVar('--muted', '#8494b3'),
    grid: cssVar('--line-2', '#1a2540'),
    line: cssVar('--line', '#22304d'),
    panel: cssVar('--panel', '#0e1628'),
  }
}
