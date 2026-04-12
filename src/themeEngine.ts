/**
 * Theme engine — applies a Theme's CSS variables to the document root.
 */

import type { Theme } from './themes'

/**
 * Set every CSS variable defined in a Theme on `document.documentElement.style`.
 * Call on startup and whenever the active theme changes.
 */
export function applyTheme(theme: Theme): void {
  const root = document.documentElement.style

  for (const [key, value] of Object.entries(theme)) {
    if (key === 'label') continue
    root.setProperty(key, value)
  }
}
