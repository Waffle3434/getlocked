import { describe, expect, it } from "vitest";
import {
  ACCENTS,
  ACCENT_NAMES,
  INK,
  STATUS,
  accentTokens,
  contrast,
  deriveScale,
  ensureContrast,
  isHex,
  luminance,
  mix,
  normalizeTheme,
  onAccent,
  themeVars,
} from "./theme";

describe("color math", () => {
  it("mixes in sRGB", () => {
    expect(mix("#000000", "#ffffff", 0.5)).toBe("#808080");
    expect(mix("#d97757", "#d97757", 0.3)).toBe("#d97757");
    expect(mix("#102030", "#ffffff", 0)).toBe("#102030");
    expect(mix("#102030", "#ffffff", 1)).toBe("#ffffff");
  });

  it("computes WCAG luminance and contrast", () => {
    expect(luminance("#ffffff")).toBeCloseTo(1);
    expect(luminance("#000000")).toBe(0);
    expect(contrast("#000000", "#ffffff")).toBeCloseTo(21);
    expect(contrast("#ffffff", "#000000")).toBeCloseTo(21);
    expect(contrast("#777777", "#777777")).toBe(1);
  });

  it("checks #rrggbb", () => {
    expect(isHex("#d97757")).toBe(true);
    expect(isHex("#ABCDEF")).toBe(true);
    for (const bad of ["d97757", "#fff", "#gg0000", "#d977577", "", null, undefined]) expect(isHex(bad)).toBe(false);
  });

  it("only moves a color as far as needed for contrast", () => {
    expect(ensureContrast("#ffffff", "#000000", 4.5)).toBe("#ffffff");
    const lifted = ensureContrast("#202060", "#262624", 4.5);
    expect(contrast(lifted, "#262624")).toBeGreaterThanOrEqual(4.5);
    expect(luminance(lifted)).toBeGreaterThan(luminance("#202060"));
    const sunk = ensureContrast("#ffe066", "#ffffff", 4.5);
    expect(contrast(sunk, "#ffffff")).toBeGreaterThanOrEqual(4.5);
    expect(luminance(sunk)).toBeLessThan(luminance("#ffe066"));
  });
});

describe("onAccent", () => {
  it("keeps white on every preset button, as before themes", () => {
    for (const name of ACCENT_NAMES) expect(onAccent(ACCENTS[name]["600"])).toBe("#ffffff");
  });

  it("uses dark text on light accents and white on dark ones", () => {
    expect(onAccent("#ffff00")).toBe("#1f1e1d");
    expect(onAccent("#ffffff")).toBe("#1f1e1d");
    expect(onAccent("#f5c518")).toBe("#1f1e1d");
    expect(onAccent("#000000")).toBe("#ffffff");
    expect(onAccent("#1a237e")).toBe("#ffffff");
  });
});

describe("accents", () => {
  it("derives a scale around the given color", () => {
    const s = deriveScale("#3366cc");
    expect(s["500"]).toBe("#3366cc");
    expect(luminance(s["300"])).toBeGreaterThan(luminance(s["400"]));
    expect(luminance(s["400"])).toBeGreaterThan(luminance(s["500"]));
    expect(luminance(s["500"])).toBeGreaterThan(luminance(s["600"]));
  });

  it("leaves the dark terracotta exactly as it was", () => {
    expect(accentTokens(ACCENTS.terracotta, "dark")).toEqual(ACCENTS.terracotta);
  });

  it("keeps accent text readable on cards in both modes", () => {
    const scales = [...ACCENT_NAMES.map((n) => ACCENTS[n]), deriveScale("#ffee00"), deriveScale("#101040"), deriveScale("#00ff88")];
    for (const scale of scales) {
      const dark = accentTokens(scale, "dark");
      expect(contrast(dark["300"], INK.dark["900"])).toBeGreaterThanOrEqual(4.5);
      expect(contrast(dark["400"], INK.dark["900"])).toBeGreaterThanOrEqual(3);
      const light = accentTokens(scale, "light");
      expect(contrast(light["300"], INK.light["900"])).toBeGreaterThanOrEqual(6);
      expect(contrast(light["400"], INK.light["900"])).toBeGreaterThanOrEqual(4.5);
      expect(contrast(light["500"], INK.light["950"])).toBeGreaterThanOrEqual(2);
    }
  });

  it("makes light-mode text shades darker than the dark-mode ones", () => {
    for (const name of ACCENT_NAMES) {
      const light = accentTokens(ACCENTS[name], "light");
      expect(luminance(light["300"])).toBeLessThan(luminance(ACCENTS[name]["300"]));
      expect(luminance(light["300"])).toBeLessThanOrEqual(luminance(light["400"]));
    }
  });
});

describe("themeVars", () => {
  it("reproduces the original dark look by default", () => {
    const vars = themeVars(undefined);
    expect(vars["--color-ink-950"]).toBe("#262624");
    expect(vars["--color-ink-100"]).toBe("#f5f4ee");
    expect(vars["--color-accent-600"]).toBe("#c6613f");
    expect(vars["--color-accent-300"]).toBe("#eba98d");
    expect(vars["--color-on-accent"]).toBe("#ffffff");
    expect(vars["--color-warn"]).toBe(STATUS.dark.warn);
  });

  it("sets the same variables in both modes", () => {
    const dark = themeVars({ mode: "dark", accent: "ocean", customAccent: null });
    const light = themeVars({ mode: "light", accent: "ocean", customAccent: null });
    expect(Object.keys(light).sort()).toEqual(Object.keys(dark).sort());
    expect(light["--color-ink-950"]).toBe("#faf9f5");
    expect(light["--color-ink-100"]).toBe("#1f1e1d");
    expect(light["--color-danger"]).toBe(STATUS.light.danger);
  });

  it("keeps main and muted text readable in light mode", () => {
    for (const bg of [INK.light["950"], INK.light["900"], INK.light["850"]]) {
      expect(contrast(INK.light["100"], bg)).toBeGreaterThanOrEqual(7);
      expect(contrast(INK.light["400"], bg)).toBeGreaterThanOrEqual(4.5);
    }
    for (const key of ["warn", "warn-soft", "ok", "danger"]) {
      expect(contrast(STATUS.light[key], INK.light["900"])).toBeGreaterThanOrEqual(4.5);
    }
  });

  it("uses a custom color and picks text that reads on it", () => {
    const vars = themeVars({ mode: "dark", accent: "custom", customAccent: "#FFEE00" });
    expect(vars["--color-accent-500"]).toBe("#ffee00");
    expect(vars["--color-on-accent"]).toBe("#1f1e1d");
  });

  it("falls back to terracotta for a missing or bad custom color", () => {
    for (const customAccent of [null, "red", "#12345"]) {
      const vars = themeVars({ mode: "dark", accent: "custom", customAccent });
      expect(vars["--color-accent-600"]).toBe("#c6613f");
    }
  });

  it("normalizes unknown values", () => {
    expect(normalizeTheme({ mode: "sepia" as never, accent: "teal" as never })).toEqual({
      mode: "dark",
      accent: "terracotta",
      customAccent: null,
    });
  });
});
