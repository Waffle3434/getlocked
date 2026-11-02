// Color themes (Settings → Appearance).
//
// index.css declares every color token in Tailwind's `@theme` with the dark
// values, and Tailwind utilities read them as CSS variables. Applying a theme
// overrides those variables on <html>, which re-colors the whole UI.
//
// Everything here except `applyTheme` is pure so it can be unit-tested.

import type { AccentName, Theme, ThemeMode } from "./types";

export type Shade = "300" | "400" | "500" | "600";
/** Accent shades. In dark mode 300 is the lightest; text uses 300/400. */
export type AccentScale = Record<Shade, string>;

export const DEFAULT_THEME: Theme = { mode: "dark", accent: "terracotta", customAccent: null };

/** Neutral surfaces and text. Same token names in both modes:
 * 1000 sidebar, 950 page, 900 cards, 850 inputs, 800/700 borders and raised
 * surfaces, 600 faint lines, 400 muted text, 300 secondary text, 100 main text. */
export const INK: Record<ThemeMode, Record<string, string>> = {
  dark: {
    "1000": "#1f1e1d",
    "950": "#262624",
    "900": "#2f2e2b",
    "850": "#34332f",
    "800": "#3c3b37",
    "700": "#4a4944",
    "600": "#5e5c56",
    "400": "#a3a097",
    "300": "#c8c5bb",
    "100": "#f5f4ee",
  },
  light: {
    "1000": "#f0eee6",
    "950": "#faf9f5",
    "900": "#ffffff",
    "850": "#f5f4ef",
    "800": "#e8e6dc",
    "700": "#d9d6cb",
    "600": "#b8b5aa",
    "400": "#6b6a64",
    "300": "#45443f",
    "100": "#1f1e1d",
  },
};

/** Status colors. `*-tint` is the base for translucent fills and borders
 * (`bg-warn-tint/10`, `border-danger-tint/30`); the rest are text and icons.
 * The dark values are Tailwind's amber/emerald/red/orange, as before themes. */
export const STATUS: Record<ThemeMode, Record<string, string>> = {
  dark: {
    warn: "oklch(87.9% 0.169 91.605)", // amber-300
    "warn-soft": "oklch(92.4% 0.12 95.746)", // amber-200
    "warn-tint": "oklch(76.9% 0.188 70.08)", // amber-500
    ok: "oklch(84.5% 0.143 164.978)", // emerald-300
    "ok-icon": "oklch(76.5% 0.177 163.223)", // emerald-400
    danger: "oklch(80.8% 0.114 19.571)", // red-300
    "danger-tint": "oklch(63.7% 0.237 25.331)", // red-500
    streak: "oklch(75% 0.183 55.934)", // orange-400
    backdrop: "rgb(0 0 0 / 0.6)",
  },
  light: {
    warn: "#9a5b00",
    "warn-soft": "#8a5000",
    "warn-tint": "#e8a317",
    ok: "#18794e",
    "ok-icon": "#1f8a5c",
    danger: "#b42318",
    "danger-tint": "#e5484d",
    streak: "#c2410c",
    backdrop: "rgb(31 30 29 / 0.35)",
  },
};

/** Preset accents, as dark-mode scales. Light mode derives darker text shades. */
export const ACCENTS: Record<Exclude<AccentName, "custom">, AccentScale> = {
  terracotta: { "300": "#eba98d", "400": "#e08b6c", "500": "#d97757", "600": "#c6613f" },
  ocean: { "300": "#9cc3f0", "400": "#6fa6e6", "500": "#4a8ad9", "600": "#2f6fc0" },
  forest: { "300": "#9fd4ac", "400": "#72bd88", "500": "#4fa36a", "600": "#2f8550" },
  violet: { "300": "#c7b2f2", "400": "#ab8cea", "500": "#9170de", "600": "#7652c8" },
  rose: { "300": "#f2a7b8", "400": "#ea8098", "500": "#e05a7a", "600": "#c63d5f" },
};

export const ACCENT_NAMES = ["terracotta", "ocean", "forest", "violet", "rose"] as const;

const WHITE = "#ffffff";
const BLACK = "#000000";
/** Dark text used on light accents (the light theme's main text). */
const ON_ACCENT_DARK = "#1f1e1d";

// ---- Color math (sRGB hex) -------------------------------------------------

export function isHex(s: string | null | undefined): s is string {
  return typeof s === "string" && /^#[0-9a-fA-F]{6}$/.test(s);
}

function rgb(hex: string): [number, number, number] {
  const n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function toHex([r, g, b]: [number, number, number]): string {
  return "#" + [r, g, b].map((c) => Math.round(Math.min(255, Math.max(0, c))).toString(16).padStart(2, "0")).join("");
}

/** `a` moved `t` (0…1) of the way to `b`, in sRGB. */
export function mix(a: string, b: string, t: number): string {
  const [ar, ag, ab] = rgb(a);
  const [br, bg, bb] = rgb(b);
  return toHex([ar + (br - ar) * t, ag + (bg - ag) * t, ab + (bb - ab) * t]);
}

/** WCAG 2 relative luminance. */
export function luminance(hex: string): number {
  const [r, g, b] = rgb(hex).map((c) => {
    const s = c / 255;
    return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 2 contrast ratio, 1…21. */
export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/** `fg`, moved toward white (on dark `bg`) or black (on light `bg`) just far
 * enough to reach `ratio` against `bg`. Unchanged if it already does. */
export function ensureContrast(fg: string, bg: string, ratio: number): string {
  if (contrast(fg, bg) >= ratio) return fg;
  const target = luminance(bg) < 0.18 ? WHITE : BLACK;
  for (let t = 0.02; t < 1; t += 0.02) {
    const c = mix(fg, target, t);
    if (contrast(c, bg) >= ratio) return c;
  }
  return target;
}

/**
 * Text color for the accent-600 fill (primary buttons, ticked boxes). Leans
 * to white, which is how the app has always looked: WCAG 2 math slightly
 * favors black on mid-tone oranges (terracotta) though white reads as well
 * there, so white wins whenever it reaches 3:1, and the better of the two
 * otherwise.
 */
export function onAccent(hex: string): string {
  const white = contrast(WHITE, hex);
  if (white >= 3) return WHITE;
  return contrast(ON_ACCENT_DARK, hex) > white ? ON_ACCENT_DARK : WHITE;
}

// ---- Accents ----------------------------------------------------------------

/** A dark-mode scale from one color, which becomes the 500 shade. */
export function deriveScale(hex: string): AccentScale {
  return {
    "300": mix(hex, WHITE, 0.35),
    "400": mix(hex, WHITE, 0.15),
    "500": hex,
    "600": mix(hex, BLACK, 0.1),
  };
}

/** The scale a theme starts from (bad custom colors fall back to terracotta). */
export function baseScale(theme: Theme): AccentScale {
  if (theme.accent === "custom") return isHex(theme.customAccent) ? deriveScale(theme.customAccent.toLowerCase()) : ACCENTS.terracotta;
  return ACCENTS[theme.accent] ?? ACCENTS.terracotta;
}

/**
 * The accent tokens for a mode. 600 is a fill (with `onAccent` text on it),
 * 500 lines and charts, 400 icons and links, 300 text on tinted surfaces.
 * Text shades are pushed until they're readable on that mode's cards: in
 * light mode that makes 300/400 *darker* than 500, the reverse of dark mode.
 */
export function accentTokens(scale: AccentScale, mode: ThemeMode): AccentScale {
  const card = INK[mode]["900"];
  const page = INK[mode]["950"];
  if (mode === "dark") {
    return {
      "300": ensureContrast(scale["300"], card, 4.5),
      "400": ensureContrast(scale["400"], card, 3),
      "500": ensureContrast(scale["500"], page, 2),
      "600": scale["600"],
    };
  }
  return {
    "300": ensureContrast(scale["600"], card, 6),
    "400": ensureContrast(scale["600"], card, 4.5),
    "500": ensureContrast(scale["500"], page, 2),
    "600": scale["600"],
  };
}

// ---- Applying -------------------------------------------------------------

/** Missing or unknown fields fall back to the default (dark terracotta). */
export function normalizeTheme(theme: Partial<Theme> | null | undefined): Theme {
  const mode: ThemeMode = theme?.mode === "light" ? "light" : "dark";
  const known = theme?.accent === "custom" || (ACCENT_NAMES as readonly string[]).includes(theme?.accent ?? "");
  const accent: AccentName = known ? (theme!.accent as AccentName) : "terracotta";
  return { mode, accent, customAccent: theme?.customAccent ?? null };
}

/** Every CSS custom property the theme sets, e.g. `--color-ink-950`. */
export function themeVars(input: Partial<Theme> | null | undefined): Record<string, string> {
  const theme = normalizeTheme(input);
  const vars: Record<string, string> = {};
  for (const [k, v] of Object.entries(INK[theme.mode])) vars[`--color-ink-${k}`] = v;
  const accent = accentTokens(baseScale(theme), theme.mode);
  for (const [k, v] of Object.entries(accent)) vars[`--color-accent-${k}`] = v;
  vars["--color-on-accent"] = onAccent(accent["600"]);
  for (const [k, v] of Object.entries(STATUS[theme.mode])) vars[`--color-${k}`] = v;
  return vars;
}

const CACHE_KEY = "getlocked.theme";
let applied = "";

/** Sets the theme's variables and `color-scheme` (scrollbars, form controls)
 * on <html>. Cheap to call repeatedly: an unchanged theme is skipped. */
export function applyTheme(theme: Partial<Theme> | null | undefined): void {
  const normalized = normalizeTheme(theme);
  const key = JSON.stringify(normalized);
  if (key === applied) return;
  applied = key;
  const root = document.documentElement;
  for (const [k, v] of Object.entries(themeVars(normalized))) root.style.setProperty(k, v);
  root.style.colorScheme = normalized.mode;
  try {
    localStorage.setItem(CACHE_KEY, key);
  } catch {
    // Only a startup nicety; the snapshot re-applies the real theme.
  }
}

/** Applies the last theme seen, before the first snapshot arrives, so a light
 * theme doesn't open with a dark flash. */
export function applyCachedTheme(): void {
  try {
    const cached = localStorage.getItem(CACHE_KEY);
    if (cached) applyTheme(JSON.parse(cached) as Partial<Theme>);
  } catch {
    // Unreadable cache: keep the CSS defaults.
  }
}
