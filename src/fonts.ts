/**
 * Font catalog for Project Commander.
 *
 * All fonts rely on system-installed faces with graceful CSS fallback stacks.
 * No woff2 files are bundled — if a preferred font isn't installed, the
 * browser falls through the stack to the next available face.
 */

export type FontOption = {
  id: string
  label: string
  stack: string
  isMono: boolean
  /** true when the font ships with every major OS (no install needed) */
  systemOnly: boolean
}

// ── Monospaced fonts (available for both UI and terminal) ────────────────────

const monoFonts: FontOption[] = [
  {
    id: 'jetbrains-mono',
    label: 'JetBrains Mono',
    stack: "'JetBrains Mono', 'Cascadia Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'fira-code',
    label: 'Fira Code',
    stack: "'Fira Code', 'JetBrains Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'cascadia-mono',
    label: 'Cascadia Mono',
    stack: "'Cascadia Mono', Consolas, 'Courier New', monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'cascadia-code',
    label: 'Cascadia Code',
    stack: "'Cascadia Code', 'Cascadia Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'consolas',
    label: 'Consolas',
    stack: "Consolas, 'Cascadia Mono', 'Courier New', monospace",
    isMono: true,
    systemOnly: true,
  },
  {
    id: 'courier-new',
    label: 'Courier New',
    stack: "'Courier New', Courier, monospace",
    isMono: true,
    systemOnly: true,
  },
  {
    id: 'sf-mono',
    label: 'SF Mono',
    stack: "'SF Mono', Menlo, Monaco, monospace",
    isMono: true,
    systemOnly: true,
  },
  {
    id: 'monaco',
    label: 'Monaco',
    stack: "Monaco, Menlo, 'Courier New', monospace",
    isMono: true,
    systemOnly: true,
  },
  {
    id: 'menlo',
    label: 'Menlo',
    stack: "Menlo, Monaco, 'Courier New', monospace",
    isMono: true,
    systemOnly: true,
  },
  {
    id: 'source-code-pro',
    label: 'Source Code Pro',
    stack: "'Source Code Pro', 'JetBrains Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'roboto-mono',
    label: 'Roboto Mono',
    stack: "'Roboto Mono', 'Cascadia Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'ibm-plex-mono',
    label: 'IBM Plex Mono',
    stack: "'IBM Plex Mono', 'JetBrains Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'hack',
    label: 'Hack',
    stack: "Hack, 'JetBrains Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'victor-mono',
    label: 'Victor Mono',
    stack: "'Victor Mono', 'JetBrains Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
  {
    id: 'ubuntu-mono',
    label: 'Ubuntu Mono',
    stack: "'Ubuntu Mono', 'Cascadia Mono', Consolas, monospace",
    isMono: true,
    systemOnly: false,
  },
]

// ── Proportional fonts (UI only) ────────────────────────────────────────────

const proportionalFonts: FontOption[] = [
  {
    id: 'system-ui',
    label: 'System UI',
    stack: "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif",
    isMono: false,
    systemOnly: true,
  },
  {
    id: 'inter',
    label: 'Inter',
    stack: "Inter, system-ui, -apple-system, sans-serif",
    isMono: false,
    systemOnly: false,
  },
  {
    id: 'roboto',
    label: 'Roboto',
    stack: "Roboto, system-ui, -apple-system, sans-serif",
    isMono: false,
    systemOnly: false,
  },
  {
    id: 'segoe-ui',
    label: 'Segoe UI',
    stack: "'Segoe UI', system-ui, -apple-system, sans-serif",
    isMono: false,
    systemOnly: true,
  },
  {
    id: 'sf-pro',
    label: 'SF Pro',
    stack: "'SF Pro Display', 'SF Pro', system-ui, -apple-system, sans-serif",
    isMono: false,
    systemOnly: true,
  },
  {
    id: 'helvetica-neue',
    label: 'Helvetica Neue',
    stack: "'Helvetica Neue', Helvetica, Arial, sans-serif",
    isMono: false,
    systemOnly: true,
  },
]

/** UI fonts — mono + proportional options */
export const uiFonts: FontOption[] = [...monoFonts, ...proportionalFonts]

/** Terminal fonts — monospaced only */
export const terminalFonts: FontOption[] = [...monoFonts]

export const defaultUiFontId = 'jetbrains-mono'
export const defaultTerminalFontId = 'jetbrains-mono'

export function getFontById(id: string, catalog: FontOption[]): FontOption | undefined {
  return catalog.find((f) => f.id === id)
}

export function getFontStack(id: string, catalog: FontOption[]): string {
  const font = getFontById(id, catalog)
  return font?.stack ?? catalog[0].stack
}
