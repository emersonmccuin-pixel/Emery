/**
 * Theme engine — applies a Theme's CSS variables and theme class to the document root.
 */

import type { Theme } from './themes'

/** Prefix for theme classes on <html>. */
const THEME_CLASS_PREFIX = 'theme-'

/**
 * Set every CSS variable defined in a Theme on `document.documentElement.style`,
 * and apply a `.theme-<id>` class to `<html>` for CSS-scoped composition layers.
 *
 * Call on startup and whenever the active theme changes.
 */
export function applyTheme(theme: Theme, themeId?: string): void {
  const root = document.documentElement

  // Apply CSS variables
  for (const [key, value] of Object.entries(theme)) {
    if (key === 'label') continue
    root.style.setProperty(key, value)
  }

  // Apply theme class for background composition scoping
  if (themeId) {
    // Remove any existing theme class
    const existing = Array.from(root.classList).filter((c) => c.startsWith(THEME_CLASS_PREFIX))
    for (const cls of existing) {
      root.classList.remove(cls)
    }
    root.classList.add(`${THEME_CLASS_PREFIX}${themeId}`)
  }
}

/**
 * Apply user-selected font stacks as CSS custom properties.
 * `--font-ui` controls all non-terminal UI text.
 * `--font-mono` controls terminal, code blocks, and monospaced elements.
 */
export function applyFonts(uiStack: string, monoStack: string): void {
  const root = document.documentElement
  root.style.setProperty('--font-ui', uiStack)
  root.style.setProperty('--font-mono', monoStack)
}
