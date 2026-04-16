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

const neon: Theme = {
  label: 'Neon',

  '--background': '#0a0612',
  '--foreground': '#f0e0f8',
  '--card': 'rgba(16, 8, 24, 0.95)',
  '--card-foreground': '#f0e0f8',
  '--primary': '#ff3399',
  '--primary-foreground': '#ffffff',
  '--secondary': '#6600cc',
  '--secondary-foreground': '#ffffff',
  '--muted': '#1a0e28',
  '--muted-foreground': '#7a5a8a',
  '--accent': '#3af0e0',
  '--accent-foreground': '#0a0612',
  '--destructive': '#ff2222',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(255, 51, 153, 0.35)',
  '--input': 'rgba(58, 240, 224, 0.25)',
  '--ring': 'rgba(58, 240, 224, 0.5)',

  '--hud-cyan': '#3af0e0',
  '--hud-magenta': '#ff3399',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#f0c040',
  '--hud-purple': '#b467f7',
  '--hud-bg': '#0a0612',
  '--hud-panel-bg': 'rgba(16, 8, 24, 0.95)',

  '--rail-projects-tint': '#ff3399',
  '--rail-sessions-tint': '#4400aa',
  '--center-tint': '#3af0e0',

  '--fg-hot': '#fff0fa',
  '--fg-dim': '#6a4a7a',

  '--surface-0': 'rgba(16, 8, 24, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #ff3399 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#f0c040',
  '--info': '#3af0e0',
  '--danger': '#ff2222',

  '--bg-gradient-1': 'rgba(255, 51, 153, 0.22)',
  '--bg-gradient-2': 'rgba(58, 240, 224, 0.16)',
  '--bg-gradient-3': 'rgba(102, 0, 204, 0.18)',
  '--bg-gradient-4': 'rgba(180, 103, 247, 0.12)',
  '--bg-grid-color': '255, 51, 153',
  '--bg-grid-opacity': '0.06',
}

const aurora: Theme = {
  label: 'Aurora',

  '--background': '#040e08',
  '--foreground': '#d8f5e8',
  '--card': 'rgba(8, 20, 14, 0.95)',
  '--card-foreground': '#d8f5e8',
  '--primary': '#74f3a1',
  '--primary-foreground': '#040e08',
  '--secondary': '#d946b8',
  '--secondary-foreground': '#ffffff',
  '--muted': '#0c2218',
  '--muted-foreground': '#5a8a6e',
  '--accent': '#ff3399',
  '--accent-foreground': '#040e08',
  '--destructive': '#ff4444',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(116, 243, 161, 0.30)',
  '--input': 'rgba(255, 51, 153, 0.25)',
  '--ring': 'rgba(255, 51, 153, 0.5)',

  '--hud-cyan': '#3af0e0',
  '--hud-magenta': '#ff3399',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#f0c040',
  '--hud-purple': '#b467f7',
  '--hud-bg': '#040e08',
  '--hud-panel-bg': 'rgba(8, 20, 14, 0.95)',

  '--rail-projects-tint': '#74f3a1',
  '--rail-sessions-tint': '#ff3399',
  '--center-tint': '#3af0e0',

  '--fg-hot': '#e8fff0',
  '--fg-dim': '#5a7a68',

  '--surface-0': 'rgba(8, 20, 14, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #74f3a1 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#f0c040',
  '--info': '#3af0e0',
  '--danger': '#ff4444',

  '--bg-gradient-1': 'rgba(116, 243, 161, 0.20)',
  '--bg-gradient-2': 'rgba(58, 240, 224, 0.16)',
  '--bg-gradient-3': 'rgba(255, 51, 153, 0.14)',
  '--bg-gradient-4': 'rgba(180, 103, 247, 0.10)',
  '--bg-grid-color': '116, 243, 161',
  '--bg-grid-opacity': '0.05',
}

const synthwave: Theme = {
  label: 'Synthwave',

  '--background': '#0c0620',
  '--foreground': '#f0d8f8',
  '--card': 'rgba(18, 10, 38, 0.95)',
  '--card-foreground': '#f0d8f8',
  '--primary': '#ff3399',
  '--primary-foreground': '#ffffff',
  '--secondary': '#8800cc',
  '--secondary-foreground': '#ffffff',
  '--muted': '#1e1038',
  '--muted-foreground': '#7a5a90',
  '--accent': '#3af0e0',
  '--accent-foreground': '#0c0620',
  '--destructive': '#ff3333',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(255, 51, 153, 0.30)',
  '--input': 'rgba(58, 240, 224, 0.25)',
  '--ring': 'rgba(58, 240, 224, 0.5)',

  '--hud-cyan': '#3af0e0',
  '--hud-magenta': '#ff3399',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#f0c040',
  '--hud-purple': '#b467f7',
  '--hud-bg': '#0c0620',
  '--hud-panel-bg': 'rgba(18, 10, 38, 0.95)',

  '--rail-projects-tint': '#ff3399',
  '--rail-sessions-tint': '#6600cc',
  '--center-tint': '#3af0e0',

  '--fg-hot': '#fff0fa',
  '--fg-dim': '#6a4a80',

  '--surface-0': 'rgba(18, 10, 38, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #ff3399 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#f0c040',
  '--info': '#3af0e0',
  '--danger': '#ff3333',

  '--bg-gradient-1': 'rgba(255, 51, 153, 0.22)',
  '--bg-gradient-2': 'rgba(136, 0, 204, 0.20)',
  '--bg-gradient-3': 'rgba(58, 240, 224, 0.14)',
  '--bg-gradient-4': 'rgba(180, 103, 247, 0.12)',
  '--bg-grid-color': '255, 51, 153',
  '--bg-grid-opacity': '0.06',
}

const matrix: Theme = {
  label: 'Matrix',

  '--background': '#000a00',
  '--foreground': '#c0f0c0',
  '--card': 'rgba(0, 16, 0, 0.95)',
  '--card-foreground': '#c0f0c0',
  '--primary': '#00ff41',
  '--primary-foreground': '#000a00',
  '--secondary': '#008830',
  '--secondary-foreground': '#ffffff',
  '--muted': '#0a1e0a',
  '--muted-foreground': '#407040',
  '--accent': '#aaffaa',
  '--accent-foreground': '#000a00',
  '--destructive': '#ff3333',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(0, 255, 65, 0.30)',
  '--input': 'rgba(170, 255, 170, 0.20)',
  '--ring': 'rgba(170, 255, 170, 0.5)',

  '--hud-cyan': '#aaffaa',
  '--hud-magenta': '#00cc55',
  '--hud-green': '#00ff41',
  '--hud-amber': '#88ff44',
  '--hud-purple': '#00aa66',
  '--hud-bg': '#000a00',
  '--hud-panel-bg': 'rgba(0, 16, 0, 0.95)',

  '--rail-projects-tint': '#00ff41',
  '--rail-sessions-tint': '#00aa66',
  '--center-tint': '#44dd44',

  '--fg-hot': '#e0ffe0',
  '--fg-dim': '#2a5a2a',

  '--surface-0': 'rgba(0, 16, 0, 0.95)',
  '--surface-1': 'rgba(0, 255, 65, 0.04)',
  '--surface-2': 'rgba(0, 255, 65, 0.08)',
  '--surface-3': 'color-mix(in srgb, #00ff41 20%, transparent)',

  '--success': '#00ff41',
  '--warning': '#88ff44',
  '--info': '#aaffaa',
  '--danger': '#ff3333',

  '--bg-gradient-1': 'rgba(0, 255, 65, 0.18)',
  '--bg-gradient-2': 'rgba(0, 170, 102, 0.16)',
  '--bg-gradient-3': 'rgba(68, 221, 68, 0.12)',
  '--bg-gradient-4': 'rgba(0, 136, 48, 0.10)',
  '--bg-grid-color': '0, 255, 65',
  '--bg-grid-opacity': '0.06',
}

const nebula: Theme = {
  label: 'Nebula',

  '--background': '#08060e',
  '--foreground': '#e0d8f0',
  '--card': 'rgba(14, 10, 22, 0.95)',
  '--card-foreground': '#e0d8f0',
  '--primary': '#b467f7',
  '--primary-foreground': '#ffffff',
  '--secondary': '#d946a8',
  '--secondary-foreground': '#ffffff',
  '--muted': '#1a1228',
  '--muted-foreground': '#6a5a80',
  '--accent': '#ffd68f',
  '--accent-foreground': '#08060e',
  '--destructive': '#ff4444',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(180, 103, 247, 0.30)',
  '--input': 'rgba(255, 214, 143, 0.25)',
  '--ring': 'rgba(255, 214, 143, 0.5)',

  '--hud-cyan': '#3af0e0',
  '--hud-magenta': '#d946a8',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#ffd68f',
  '--hud-purple': '#b467f7',
  '--hud-bg': '#08060e',
  '--hud-panel-bg': 'rgba(14, 10, 22, 0.95)',

  '--rail-projects-tint': '#b467f7',
  '--rail-sessions-tint': '#d946a8',
  '--center-tint': '#4ab5c6',

  '--fg-hot': '#f0e8ff',
  '--fg-dim': '#5a4a70',

  '--surface-0': 'rgba(14, 10, 22, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #b467f7 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#ffd68f',
  '--info': '#3af0e0',
  '--danger': '#ff4444',

  '--bg-gradient-1': 'rgba(180, 103, 247, 0.22)',
  '--bg-gradient-2': 'rgba(217, 70, 168, 0.18)',
  '--bg-gradient-3': 'rgba(74, 181, 198, 0.14)',
  '--bg-gradient-4': 'rgba(255, 214, 143, 0.10)',
  '--bg-grid-color': '180, 103, 247',
  '--bg-grid-opacity': '0.05',
}

const vaporwave: Theme = {
  label: 'Vaporwave',

  '--background': '#0e0818',
  '--foreground': '#f0e0f0',
  '--card': 'rgba(20, 12, 30, 0.95)',
  '--card-foreground': '#f0e0f0',
  '--primary': '#ffa0d0',
  '--primary-foreground': '#0e0818',
  '--secondary': '#a060e0',
  '--secondary-foreground': '#ffffff',
  '--muted': '#201430',
  '--muted-foreground': '#7a5a8a',
  '--accent': '#a0e0ff',
  '--accent-foreground': '#0e0818',
  '--destructive': '#ff5555',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(255, 160, 208, 0.30)',
  '--input': 'rgba(160, 224, 255, 0.25)',
  '--ring': 'rgba(160, 224, 255, 0.5)',

  '--hud-cyan': '#a0e0ff',
  '--hud-magenta': '#ffa0d0',
  '--hud-green': '#74f3a1',
  '--hud-amber': '#f0c040',
  '--hud-purple': '#c080ff',
  '--hud-bg': '#0e0818',
  '--hud-panel-bg': 'rgba(20, 12, 30, 0.95)',

  '--rail-projects-tint': '#ffa0d0',
  '--rail-sessions-tint': '#a060e0',
  '--center-tint': '#a0e0ff',

  '--fg-hot': '#fff0f8',
  '--fg-dim': '#6a4a7a',

  '--surface-0': 'rgba(20, 12, 30, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #ffa0d0 20%, transparent)',

  '--success': '#74f3a1',
  '--warning': '#f0c040',
  '--info': '#a0e0ff',
  '--danger': '#ff5555',

  '--bg-gradient-1': 'rgba(255, 160, 208, 0.20)',
  '--bg-gradient-2': 'rgba(160, 96, 224, 0.18)',
  '--bg-gradient-3': 'rgba(160, 224, 255, 0.14)',
  '--bg-gradient-4': 'rgba(192, 128, 255, 0.10)',
  '--bg-grid-color': '255, 160, 208',
  '--bg-grid-opacity': '0.05',
}

const hologram: Theme = {
  label: 'Hologram',

  '--background': '#060810',
  '--foreground': '#e8eef8',
  '--card': 'rgba(10, 14, 24, 0.95)',
  '--card-foreground': '#e8eef8',
  '--primary': '#40e0ff',
  '--primary-foreground': '#060810',
  '--secondary': '#ff60c0',
  '--secondary-foreground': '#ffffff',
  '--muted': '#101828',
  '--muted-foreground': '#5a7090',
  '--accent': '#ffffff',
  '--accent-foreground': '#060810',
  '--destructive': '#ff4444',
  '--destructive-foreground': '#ffffff',
  '--border': 'rgba(64, 224, 255, 0.30)',
  '--input': 'rgba(255, 255, 255, 0.20)',
  '--ring': 'rgba(255, 255, 255, 0.5)',

  '--hud-cyan': '#40e0ff',
  '--hud-magenta': '#ff60c0',
  '--hud-green': '#60ff90',
  '--hud-amber': '#ffe060',
  '--hud-purple': '#c080ff',
  '--hud-bg': '#060810',
  '--hud-panel-bg': 'rgba(10, 14, 24, 0.95)',

  '--rail-projects-tint': '#40e0ff',
  '--rail-sessions-tint': '#ff60c0',
  '--center-tint': '#c080ff',

  '--fg-hot': '#f0f4ff',
  '--fg-dim': '#4a6080',

  '--surface-0': 'rgba(10, 14, 24, 0.95)',
  '--surface-1': 'rgba(255, 255, 255, 0.04)',
  '--surface-2': 'rgba(255, 255, 255, 0.08)',
  '--surface-3': 'color-mix(in srgb, #40e0ff 20%, transparent)',

  '--success': '#60ff90',
  '--warning': '#ffe060',
  '--info': '#40e0ff',
  '--danger': '#ff4444',

  '--bg-gradient-1': 'rgba(64, 224, 255, 0.20)',
  '--bg-gradient-2': 'rgba(255, 96, 192, 0.16)',
  '--bg-gradient-3': 'rgba(192, 128, 255, 0.14)',
  '--bg-gradient-4': 'rgba(96, 255, 144, 0.10)',
  '--bg-grid-color': '64, 224, 255',
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
  neon,
  aurora,
  synthwave,
  matrix,
  nebula,
  vaporwave,
  hologram,
}

export const defaultThemeId = 'command'

export function getTheme(id: string): Theme {
  return themes[id] ?? themes[defaultThemeId]
}
