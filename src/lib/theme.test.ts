import { describe, expect, it } from "vitest";
import {
  DEFAULT_DARK,
  DEFAULT_LIGHT,
  DEFAULT_THEME,
  PALETTE_KEYS,
  activeMode,
  migrate,
} from "./theme";

describe("settings saved before light and dark had their own colours", () => {
  // The upgrade must be invisible. Somebody who set a green accent should still
  // have a green accent afterwards — in both schemes, since there was only one.
  it("carries the old shared colours into both palettes", () => {
    const migrated = migrate({
      mode: "dark",
      accent: "#00a86b",
      ai: "#123456",
      highlight: "#abcdef",
      fontSize: 18,
    });
    for (const mode of ["light", "dark"] as const) {
      expect(migrated[mode].accent, mode).toBe("#00a86b");
      expect(migrated[mode].ai, mode).toBe("#123456");
      expect(migrated[mode].highlight, mode).toBe("#abcdef");
    }
    // …and the rest of the settings are untouched.
    expect(migrated.mode).toBe("dark");
    expect(migrated.fontSize).toBe(18);
  });

  it("fills the tokens that were never editable from the defaults", () => {
    const migrated = migrate({ accent: "#00a86b" });
    expect(migrated.light.bg).toBe(DEFAULT_LIGHT.bg);
    expect(migrated.dark.bg).toBe(DEFAULT_DARK.bg);
    expect(migrated.light.ink).toBe(DEFAULT_LIGHT.ink);
    expect(migrated.dark.ink).toBe(DEFAULT_DARK.ink);
  });

  it("leaves an already-split theme alone", () => {
    const stored = {
      mode: "light",
      light: { ...DEFAULT_LIGHT, accent: "#111111" },
      dark: { ...DEFAULT_DARK, accent: "#222222" },
    };
    const migrated = migrate(stored);
    expect(migrated.light.accent).toBe("#111111");
    expect(migrated.dark.accent).toBe("#222222");
  });

  it("prefers a stored palette over a stale shared colour", () => {
    // Both shapes present: the newer one wins, or an upgrade would undo an edit.
    const migrated = migrate({
      accent: "#00a86b",
      dark: { ...DEFAULT_DARK, accent: "#222222" },
    });
    expect(migrated.dark.accent).toBe("#222222");
    expect(migrated.light.accent).toBe("#00a86b");
  });

  it("falls back to the defaults for anything stored empty or broken", () => {
    expect(migrate({})).toEqual(DEFAULT_THEME);
  });
});

describe("which palette is on screen", () => {
  it("follows the explicit modes", () => {
    expect(activeMode({ ...DEFAULT_THEME, mode: "light" }, true)).toBe("light");
    expect(activeMode({ ...DEFAULT_THEME, mode: "dark" }, false)).toBe("dark");
  });

  it("follows the OS under system", () => {
    expect(activeMode({ ...DEFAULT_THEME, mode: "system" }, true)).toBe("dark");
    expect(activeMode({ ...DEFAULT_THEME, mode: "system" }, false)).toBe("light");
  });
});

describe("the two schemes cover the same tokens", () => {
  it("has every key in both defaults, so no token can go unset", () => {
    for (const key of PALETTE_KEYS) {
      expect(DEFAULT_LIGHT[key], `light ${key}`).toMatch(/^#[0-9a-f]{6}$/i);
      expect(DEFAULT_DARK[key], `dark ${key}`).toMatch(/^#[0-9a-f]{6}$/i);
    }
  });

  it("does not read as the same scheme twice", () => {
    expect(DEFAULT_LIGHT.bg).not.toBe(DEFAULT_DARK.bg);
    expect(DEFAULT_LIGHT.ink).not.toBe(DEFAULT_DARK.ink);
  });
});
