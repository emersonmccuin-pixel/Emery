/**
 * Appearance overrides engine.
 *
 * Stores user customizations in localStorage and applies them as inline
 * CSS custom properties on <html>, which override theme-level variables.
 */

const STORAGE_KEY = "emery.appearance";

// Base font-size values (rem) from the design system — used to scale
const BASE_FONT_SIZES: Record<string, number> = {
  "--text-xs": 0.69,
  "--text-sm": 0.77,
  "--text-base": 0.85,
  "--text-md": 1.0,
  "--text-lg": 1.15,
  "--text-xl": 1.38,
};

export interface AppearanceOverrides {
  // Exposed controls
  brightness: number;       // 0.5 – 1.5, default 1.0
  fontScale: number;        // 0.75 – 1.5, default 1.0
  fontSans: string;         // empty = theme default
  fontMono: string;         // empty = theme default
  accentColor: string;      // empty = theme default, or hex
  uiDensity: "compact" | "default" | "comfortable";

  // Advanced: per-token overrides (CSS variable name → value)
  tokens: Record<string, string>;
}

const DEFAULTS: AppearanceOverrides = {
  brightness: 1.0,
  fontScale: 1.0,
  fontSans: "",
  fontMono: "",
  accentColor: "",
  uiDensity: "default",
  tokens: {},
};

/** Load overrides from localStorage (returns defaults for missing keys). */
export function loadOverrides(): AppearanceOverrides {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULTS, tokens: {} };
    const parsed = JSON.parse(raw);
    return {
      ...DEFAULTS,
      ...parsed,
      tokens: { ...parsed.tokens },
    };
  } catch {
    return { ...DEFAULTS, tokens: {} };
  }
}

/** Persist overrides to localStorage. */
export function saveOverrides(overrides: AppearanceOverrides): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(overrides));
}

/** Apply all overrides to the document. Call after theme change too. */
export function applyOverrides(overrides: AppearanceOverrides): void {
  const el = document.documentElement;

  // --- Brightness (CSS filter on body) ---
  if (overrides.brightness !== 1.0) {
    document.body.style.filter = `brightness(${overrides.brightness})`;
  } else {
    document.body.style.filter = "";
  }

  // --- Font scale ---
  if (overrides.fontScale !== 1.0) {
    for (const [varName, base] of Object.entries(BASE_FONT_SIZES)) {
      el.style.setProperty(varName, `${(base * overrides.fontScale).toFixed(3)}rem`);
    }
  } else {
    for (const varName of Object.keys(BASE_FONT_SIZES)) {
      el.style.removeProperty(varName);
    }
  }

  // --- Font family ---
  if (overrides.fontSans) {
    el.style.setProperty("--font-sans", overrides.fontSans);
  } else {
    el.style.removeProperty("--font-sans");
  }

  if (overrides.fontMono) {
    el.style.setProperty("--font-mono", overrides.fontMono);
  } else {
    el.style.removeProperty("--font-mono");
  }

  // --- Accent color ---
  if (overrides.accentColor) {
    el.style.setProperty("--accent", overrides.accentColor);
    el.style.setProperty("--accent-hover", lighten(overrides.accentColor, 15));
    el.style.setProperty("--accent-muted", withAlpha(overrides.accentColor, 0.18));
    el.style.setProperty("--accent-subtle", withAlpha(overrides.accentColor, 0.08));
    el.style.setProperty("--border-focus", withAlpha(overrides.accentColor, 0.5));
  } else {
    for (const v of ["--accent", "--accent-hover", "--accent-muted", "--accent-subtle", "--border-focus"]) {
      el.style.removeProperty(v);
    }
  }

  // --- UI density ---
  applyDensity(el, overrides.uiDensity);

  // --- Per-token overrides ---
  // First, clear any previously-set token overrides that are no longer present
  const prevTokens = (el.dataset.appearanceTokens ?? "").split(",").filter(Boolean);
  for (const varName of prevTokens) {
    if (!(varName in overrides.tokens)) {
      el.style.removeProperty(varName);
    }
  }
  // Then apply current tokens
  const tokenNames: string[] = [];
  for (const [varName, value] of Object.entries(overrides.tokens)) {
    if (value) {
      el.style.setProperty(varName, value);
      tokenNames.push(varName);
    } else {
      el.style.removeProperty(varName);
    }
  }
  el.dataset.appearanceTokens = tokenNames.join(",");
}

function applyDensity(el: HTMLElement, density: string): void {
  const scales: Record<string, Record<string, string>> = {
    compact: {
      "--space-1": "2px", "--space-2": "4px", "--space-3": "8px",
      "--space-4": "12px", "--space-5": "16px", "--space-6": "20px", "--space-8": "24px",
      "--radius-sm": "3px", "--radius-md": "4px", "--radius-lg": "6px", "--radius-xl": "8px",
    },
    comfortable: {
      "--space-1": "6px", "--space-2": "10px", "--space-3": "14px",
      "--space-4": "20px", "--space-5": "24px", "--space-6": "28px", "--space-8": "40px",
      "--radius-sm": "5px", "--radius-md": "8px", "--radius-lg": "10px", "--radius-xl": "14px",
    },
  };

  // Clear any previously set density vars
  const allVars = [
    "--space-1", "--space-2", "--space-3", "--space-4", "--space-5", "--space-6", "--space-8",
    "--radius-sm", "--radius-md", "--radius-lg", "--radius-xl",
  ];

  if (density === "default" || !scales[density]) {
    for (const v of allVars) el.style.removeProperty(v);
  } else {
    for (const [v, val] of Object.entries(scales[density])) {
      el.style.setProperty(v, val);
    }
  }
}

/** Clear all overrides and reset to theme defaults. */
export function resetOverrides(): void {
  localStorage.removeItem(STORAGE_KEY);
  const el = document.documentElement;
  // Clear inline styles we may have set
  document.body.style.filter = "";
  const allVars = [
    ...Object.keys(BASE_FONT_SIZES),
    "--font-sans", "--font-mono",
    "--accent", "--accent-hover", "--accent-muted", "--accent-subtle", "--border-focus",
    "--space-1", "--space-2", "--space-3", "--space-4", "--space-5", "--space-6", "--space-8",
    "--radius-sm", "--radius-md", "--radius-lg", "--radius-xl",
  ];
  for (const v of allVars) el.style.removeProperty(v);
  // Clear token overrides
  const prevTokens = (el.dataset.appearanceTokens ?? "").split(",").filter(Boolean);
  for (const v of prevTokens) el.style.removeProperty(v);
  el.dataset.appearanceTokens = "";
}

// ---- Color helpers ----

function hexToRgb(hex: string): [number, number, number] | null {
  const m = hex.replace("#", "").match(/^([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i);
  if (!m) return null;
  return [parseInt(m[1], 16), parseInt(m[2], 16), parseInt(m[3], 16)];
}

function rgbToHex(r: number, g: number, b: number): string {
  return "#" + [r, g, b].map(c => Math.max(0, Math.min(255, Math.round(c))).toString(16).padStart(2, "0")).join("");
}

function lighten(hex: string, pct: number): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return hex;
  const [r, g, b] = rgb;
  const factor = 1 + pct / 100;
  return rgbToHex(r * factor, g * factor, b * factor);
}

function withAlpha(hex: string, alpha: number): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return hex;
  return `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, ${alpha})`;
}
