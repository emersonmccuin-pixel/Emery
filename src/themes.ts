/**
 * Theme system — type definitions and preset themes.
 *
 * Every theme defines the full set of CSS custom properties that drive
 * the application's visual appearance. The "Command" preset extracts
 * the original hardcoded values so the default look is identical.
 */

export type Theme = {
  /** Human-readable display name */
  label: string

  /* ── Core semantic tokens ── */
  '--background': string
  '--foreground': string
  '--card': string
  '--card-foreground': string
  '--primary': string
  '--primary-foreground': string
  '--secondary': string
  '--secondary-foreground': string
  '--muted': string
  '--muted-foreground': string
  '--accent': string
  '--accent-foreground': string
  '--destructive': string
  '--destructive-foreground': string
  '--border': string
  '--input': string
  '--ring': string

  /* ── HUD accent palette ── */
  '--hud-cyan': string
  '--hud-magenta': string
  '--hud-green': string
  '--hud-amber': string
  '--hud-purple': string
  '--hud-bg': string
  '--hud-panel-bg': string

  /* ── Panel tint vars ── */
  '--rail-projects-tint': string
  '--rail-sessions-tint': string
  '--center-tint': string

  /* ── Background vars ── */
  '--bg-gradient-1': string
  '--bg-gradient-2': string
  '--bg-gradient-3': string
  '--bg-gradient-4': string
  '--bg-grid-color': string
  '--bg-grid-opacity': string
}

/** All CSS variable keys that a theme must define */
export type ThemeKey = keyof Omit<Theme, 'label'>

/* ════════════════════════════════════════════════════════════════════
   Preset themes
   ════════════════════════════════════════════════════════════════════ */

const command: Theme = {
  label: 'Command',

  '--background': '#050a0e',
  '--foreground': '#d7f5dd',
  '--card': 'rgba(8, 18, 25, 0.95)',
  '--card-foreground': '#d7f5dd',
  '--primary': '#74f3a1',
  '--primary-foreground': '#050a0e',
  '--secondary': '#ff3399',
  '--secondary-foreground': '#ffffff',
  '--muted': '#162a1e',
  '--muted-foreground': '#93b89e',
  '--accent': '#3af0e0',
  '--accent-foreground': '#050a0e',
  '--destructive': '#ff4444',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(116, 243, 161, 0.35)',
  '--input': 'rgba(58, 240, 224, 0.25)',
  '--ring': 'rgba(58, 240, 224, 0.5)',

  '--hud-cyan': '#3af0e0',
  '--hud-magenta': '#ff3399',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#f0c040',
  '--hud-purple': '#b467f7',
  '--hud-bg': '#050a0e',
  '--hud-panel-bg': 'rgba(10, 22, 18, 0.95)',

  '--rail-projects-tint': '#3af0e0',
  '--rail-sessions-tint': '#ff3399',
  '--center-tint': '#74f3a1',

  '--bg-gradient-1': 'rgba(58, 240, 224, 0.18)',
  '--bg-gradient-2': 'rgba(255, 51, 153, 0.16)',
  '--bg-gradient-3': 'rgba(116, 243, 161, 0.14)',
  '--bg-gradient-4': 'rgba(180, 103, 247, 0.12)',
  '--bg-grid-color': '116, 243, 161',
  '--bg-grid-opacity': '0.06',
}

const midnight: Theme = {
  label: 'Midnight',

  '--background': '#060b18',
  '--foreground': '#c8d6e5',
  '--card': 'rgba(10, 16, 36, 0.95)',
  '--card-foreground': '#c8d6e5',
  '--primary': '#7b8cff',
  '--primary-foreground': '#060b18',
  '--secondary': '#a855f7',
  '--secondary-foreground': '#ffffff',
  '--muted': '#141e3a',
  '--muted-foreground': '#7a8ba8',
  '--accent': '#5b7fff',
  '--accent-foreground': '#060b18',
  '--destructive': '#ff5555',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(123, 140, 255, 0.3)',
  '--input': 'rgba(91, 127, 255, 0.25)',
  '--ring': 'rgba(91, 127, 255, 0.5)',

  '--hud-cyan': '#5b7fff',
  '--hud-magenta': '#a855f7',
  '--hud-green': '#7b8cff',
  '--hud-amber': '#f0c040',
  '--hud-purple': '#c084fc',
  '--hud-bg': '#060b18',
  '--hud-panel-bg': 'rgba(12, 18, 42, 0.95)',

  '--rail-projects-tint': '#5b7fff',
  '--rail-sessions-tint': '#a855f7',
  '--center-tint': '#7b8cff',

  '--bg-gradient-1': 'rgba(91, 127, 255, 0.20)',
  '--bg-gradient-2': 'rgba(168, 85, 247, 0.18)',
  '--bg-gradient-3': 'rgba(123, 140, 255, 0.14)',
  '--bg-gradient-4': 'rgba(192, 132, 252, 0.12)',
  '--bg-grid-color': '123, 140, 255',
  '--bg-grid-opacity': '0.05',
}

const ember: Theme = {
  label: 'Ember',

  '--background': '#0c0806',
  '--foreground': '#f0e0d0',
  '--card': 'rgba(20, 12, 8, 0.95)',
  '--card-foreground': '#f0e0d0',
  '--primary': '#ff8c42',
  '--primary-foreground': '#0c0806',
  '--secondary': '#e63946',
  '--secondary-foreground': '#ffffff',
  '--muted': '#2a1810',
  '--muted-foreground': '#b09080',
  '--accent': '#ff6b35',
  '--accent-foreground': '#0c0806',
  '--destructive': '#ff3333',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(255, 140, 66, 0.3)',
  '--input': 'rgba(255, 107, 53, 0.25)',
  '--ring': 'rgba(255, 107, 53, 0.5)',

  '--hud-cyan': '#ff6b35',
  '--hud-magenta': '#e63946',
  '--hud-green': '#ff8c42',
  '--hud-amber': '#ffb347',
  '--hud-purple': '#d4553a',
  '--hud-bg': '#0c0806',
  '--hud-panel-bg': 'rgba(28, 14, 10, 0.95)',

  '--rail-projects-tint': '#ff6b35',
  '--rail-sessions-tint': '#e63946',
  '--center-tint': '#ff8c42',

  '--bg-gradient-1': 'rgba(255, 107, 53, 0.20)',
  '--bg-gradient-2': 'rgba(230, 57, 70, 0.18)',
  '--bg-gradient-3': 'rgba(255, 140, 66, 0.14)',
  '--bg-gradient-4': 'rgba(212, 85, 58, 0.12)',
  '--bg-grid-color': '255, 140, 66',
  '--bg-grid-opacity': '0.05',
}

const arctic: Theme = {
  label: 'Arctic',

  '--background': '#070b10',
  '--foreground': '#e8f0f8',
  '--card': 'rgba(10, 18, 28, 0.95)',
  '--card-foreground': '#e8f0f8',
  '--primary': '#88d8f8',
  '--primary-foreground': '#070b10',
  '--secondary': '#a0d0e8',
  '--secondary-foreground': '#070b10',
  '--muted': '#132030',
  '--muted-foreground': '#7a9ab0',
  '--accent': '#60c8f0',
  '--accent-foreground': '#070b10',
  '--destructive': '#ff5555',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(136, 216, 248, 0.3)',
  '--input': 'rgba(96, 200, 240, 0.25)',
  '--ring': 'rgba(96, 200, 240, 0.5)',

  '--hud-cyan': '#60c8f0',
  '--hud-magenta': '#a0d0e8',
  '--hud-green': '#88d8f8',
  '--hud-amber': '#e0d890',
  '--hud-purple': '#90b8d8',
  '--hud-bg': '#070b10',
  '--hud-panel-bg': 'rgba(10, 22, 34, 0.95)',

  '--rail-projects-tint': '#60c8f0',
  '--rail-sessions-tint': '#a0d0e8',
  '--center-tint': '#88d8f8',

  '--bg-gradient-1': 'rgba(96, 200, 240, 0.20)',
  '--bg-gradient-2': 'rgba(160, 208, 232, 0.16)',
  '--bg-gradient-3': 'rgba(136, 216, 248, 0.14)',
  '--bg-gradient-4': 'rgba(144, 184, 216, 0.12)',
  '--bg-grid-color': '136, 216, 248',
  '--bg-grid-opacity': '0.05',
}

const phantom: Theme = {
  label: 'Phantom',

  '--background': '#08080a',
  '--foreground': '#c8c8cc',
  '--card': 'rgba(14, 14, 18, 0.95)',
  '--card-foreground': '#c8c8cc',
  '--primary': '#8b5cf6',
  '--primary-foreground': '#ffffff',
  '--secondary': '#3f3f46',
  '--secondary-foreground': '#c8c8cc',
  '--muted': '#1a1a20',
  '--muted-foreground': '#71717a',
  '--accent': '#8b5cf6',
  '--accent-foreground': '#ffffff',
  '--destructive': '#ef4444',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(139, 92, 246, 0.25)',
  '--input': 'rgba(139, 92, 246, 0.2)',
  '--ring': 'rgba(139, 92, 246, 0.5)',

  '--hud-cyan': '#8b5cf6',
  '--hud-magenta': '#6d28d9',
  '--hud-green': '#a78bfa',
  '--hud-amber': '#94a3b8',
  '--hud-purple': '#7c3aed',
  '--hud-bg': '#08080a',
  '--hud-panel-bg': 'rgba(18, 14, 28, 0.95)',

  '--rail-projects-tint': '#8b5cf6',
  '--rail-sessions-tint': '#6d28d9',
  '--center-tint': '#a78bfa',

  '--bg-gradient-1': 'rgba(139, 92, 246, 0.18)',
  '--bg-gradient-2': 'rgba(109, 40, 217, 0.16)',
  '--bg-gradient-3': 'rgba(167, 139, 250, 0.12)',
  '--bg-gradient-4': 'rgba(124, 58, 237, 0.12)',
  '--bg-grid-color': '139, 92, 246',
  '--bg-grid-opacity': '0.05',
}

/* ════════════════════════════════════════════════════════════════════
   Exports
   ════════════════════════════════════════════════════════════════════ */

export const themes: Record<string, Theme> = {
  command,
  midnight,
  ember,
  arctic,
  phantom,
}

export const defaultThemeId = 'command'

export function getTheme(id: string): Theme {
  return themes[id] ?? themes[defaultThemeId]
}
