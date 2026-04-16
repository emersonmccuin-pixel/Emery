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

  /* ── Foreground contrast ── */
  '--fg-hot': string
  '--fg-dim': string

  /* ── Surface elevation layers ── */
  '--surface-0': string
  '--surface-1': string
  '--surface-2': string
  '--surface-3': string

  /* ── State colors ── */
  '--success': string
  '--warning': string
  '--info': string
  '--danger': string

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

  '--fg-hot': '#e8fff0',
  '--fg-dim': '#6b8a78',

  '--surface-0': 'rgba(10, 22, 18, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #74f3a1 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#f0c040',
  '--info': '#3af0e0',
  '--danger': '#ff4444',

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
  '--primary': '#4ab5c6',
  '--primary-foreground': '#060b18',
  '--secondary': '#7c5cb8',
  '--secondary-foreground': '#ffffff',
  '--muted': '#141e3a',
  '--muted-foreground': '#5a6e7a',
  '--accent': '#f0c040',
  '--accent-foreground': '#060b18',
  '--destructive': '#ff5555',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(74, 181, 198, 0.3)',
  '--input': 'rgba(240, 192, 64, 0.25)',
  '--ring': 'rgba(240, 192, 64, 0.5)',

  '--hud-cyan': '#4ab5c6',
  '--hud-magenta': '#7c5cb8',
  '--hud-green': '#4ab5c6',
  '--hud-amber': '#f0c040',
  '--hud-purple': '#7c5cb8',
  '--hud-bg': '#060b18',
  '--hud-panel-bg': 'rgba(12, 18, 42, 0.95)',

  '--rail-projects-tint': '#5c7ca0',
  '--rail-sessions-tint': '#7c5cb8',
  '--center-tint': '#4ab5c6',

  '--fg-hot': '#e3f0f4',
  '--fg-dim': '#5a6e7a',

  '--surface-0': 'rgba(12, 18, 42, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #4ab5c6 20%, transparent)',

  '--success': '#4ab5c6',
  '--warning': '#f0c040',
  '--info': '#4ab5c6',
  '--danger': '#ff5555',

  '--bg-gradient-1': 'rgba(74, 181, 198, 0.20)',
  '--bg-gradient-2': 'rgba(124, 92, 184, 0.18)',
  '--bg-gradient-3': 'rgba(92, 124, 160, 0.14)',
  '--bg-gradient-4': 'rgba(240, 192, 64, 0.10)',
  '--bg-grid-color': '74, 181, 198',
  '--bg-grid-opacity': '0.05',
}

const ember: Theme = {
  label: 'Ember',

  '--background': '#0c0806',
  '--foreground': '#f0e0d0',
  '--card': 'rgba(20, 12, 8, 0.95)',
  '--card-foreground': '#f0e0d0',
  '--primary': '#f08040',
  '--primary-foreground': '#0c0806',
  '--secondary': '#a83360',
  '--secondary-foreground': '#ffffff',
  '--muted': '#2a1810',
  '--muted-foreground': '#6b4535',
  '--accent': '#ffd68f',
  '--accent-foreground': '#0c0806',
  '--destructive': '#ff3333',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(240, 128, 64, 0.3)',
  '--input': 'rgba(255, 214, 143, 0.25)',
  '--ring': 'rgba(255, 214, 143, 0.5)',

  '--hud-cyan': '#ffd68f',
  '--hud-magenta': '#a83360',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#ffb540',
  '--hud-purple': '#a83360',
  '--hud-bg': '#0c0806',
  '--hud-panel-bg': 'rgba(28, 14, 10, 0.95)',

  '--rail-projects-tint': '#c54030',
  '--rail-sessions-tint': '#a83360',
  '--center-tint': '#f08040',

  '--fg-hot': '#ffe8cf',
  '--fg-dim': '#6b4535',

  '--surface-0': 'rgba(28, 14, 10, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #f08040 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#ffb540',
  '--info': '#ffd68f',
  '--danger': '#ff3333',

  '--bg-gradient-1': 'rgba(197, 64, 48, 0.20)',
  '--bg-gradient-2': 'rgba(168, 51, 96, 0.18)',
  '--bg-gradient-3': 'rgba(240, 128, 64, 0.14)',
  '--bg-gradient-4': 'rgba(255, 214, 143, 0.10)',
  '--bg-grid-color': '240, 128, 64',
  '--bg-grid-opacity': '0.05',
}

const arctic: Theme = {
  label: 'Arctic',

  '--background': '#070b10',
  '--foreground': '#e8f0f8',
  '--card': 'rgba(10, 18, 28, 0.95)',
  '--card-foreground': '#e8f0f8',
  '--primary': '#4ab8ff',
  '--primary-foreground': '#070b10',
  '--secondary': '#8c78e8',
  '--secondary-foreground': '#ffffff',
  '--muted': '#132030',
  '--muted-foreground': '#6a7e90',
  '--accent': '#ffa0b0',
  '--accent-foreground': '#070b10',
  '--destructive': '#ff5555',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(74, 184, 255, 0.3)',
  '--input': 'rgba(255, 160, 176, 0.25)',
  '--ring': 'rgba(255, 160, 176, 0.5)',

  '--hud-cyan': '#3fc4d8',
  '--hud-magenta': '#ffa0b0',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#ffb540',
  '--hud-purple': '#8c78e8',
  '--hud-bg': '#070b10',
  '--hud-panel-bg': 'rgba(10, 22, 34, 0.95)',

  '--rail-projects-tint': '#5ca8ef',
  '--rail-sessions-tint': '#8c78e8',
  '--center-tint': '#3fc4d8',

  '--fg-hot': '#e8f4ff',
  '--fg-dim': '#6a7e90',

  '--surface-0': 'rgba(10, 22, 34, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #4ab8ff 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#ffb540',
  '--info': '#3fc4d8',
  '--danger': '#ff5555',

  '--bg-gradient-1': 'rgba(92, 168, 239, 0.20)',
  '--bg-gradient-2': 'rgba(140, 120, 232, 0.16)',
  '--bg-gradient-3': 'rgba(63, 196, 216, 0.14)',
  '--bg-gradient-4': 'rgba(255, 160, 176, 0.10)',
  '--bg-grid-color': '74, 184, 255',
  '--bg-grid-opacity': '0.05',
}

const phantom: Theme = {
  label: 'Phantom',

  '--background': '#08080a',
  '--foreground': '#c8c8cc',
  '--card': 'rgba(14, 14, 18, 0.95)',
  '--card-foreground': '#c8c8cc',
  '--primary': '#b467f7',
  '--primary-foreground': '#ffffff',
  '--secondary': '#6c5cf5',
  '--secondary-foreground': '#ffffff',
  '--muted': '#1a1a20',
  '--muted-foreground': '#6a5a7a',
  '--accent': '#3af0e0',
  '--accent-foreground': '#08080a',
  '--destructive': '#ef4444',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(180, 103, 247, 0.25)',
  '--input': 'rgba(58, 240, 224, 0.2)',
  '--ring': 'rgba(58, 240, 224, 0.5)',

  '--hud-cyan': '#3af0e0',
  '--hud-magenta': '#8c4fd4',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#f0c040',
  '--hud-purple': '#b467f7',
  '--hud-bg': '#08080a',
  '--hud-panel-bg': 'rgba(18, 14, 28, 0.95)',

  '--rail-projects-tint': '#8c4fd4',
  '--rail-sessions-tint': '#6c5cf5',
  '--center-tint': '#b467f7',

  '--fg-hot': '#f0e6ff',
  '--fg-dim': '#6a5a7a',

  '--surface-0': 'rgba(18, 14, 28, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #b467f7 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#f0c040',
  '--info': '#3af0e0',
  '--danger': '#ef4444',

  '--bg-gradient-1': 'rgba(140, 79, 212, 0.18)',
  '--bg-gradient-2': 'rgba(108, 92, 245, 0.16)',
  '--bg-gradient-3': 'rgba(180, 103, 247, 0.12)',
  '--bg-gradient-4': 'rgba(58, 240, 224, 0.08)',
  '--bg-grid-color': '180, 103, 247',
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
