/**
 * Colours for the graph's folder view. Pure functions, no DOM — the picker
 * lives in GraphView, the arithmetic lives here where it can be tested.
 */

/** Distinct hues, one per top-level folder. */
const HUES = [205, 145, 332, 40, 265, 190, 355, 95, 22, 240, 170, 300];

export function dirOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? "" : path.slice(0, i);
}

export interface Hsl {
  h: number;
  s: number;
  l: number;
}

/**
 * A `#rrggbb` colour as HSL — all three parts of it.
 *
 * Taking only the hue and rebuilding the colour at a fixed saturation and
 * lightness is what made a picked pale red come back as a full red, and a grey
 * come back as red outright: grey has no hue at all, so it collapsed to 0.
 * Nothing is rounded here, so the value the picker returned is the value that
 * gets drawn.
 */
export function hexToHsl(hex: string): Hsl | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const v = parseInt(m[1], 16);
  const r = ((v >> 16) & 255) / 255;
  const g = ((v >> 8) & 255) / 255;
  const b = (v & 255) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  const l = (max + min) / 2;
  // Achromatic: no hue to speak of, and saturation 0 is what keeps it grey.
  if (d === 0) return { h: 0, s: 0, l: l * 100 };
  const s = d / (1 - Math.abs(2 * l - 1));
  let h: number;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  h *= 60;
  if (h < 0) h += 360;
  return { h, s: s * 100, l: l * 100 };
}

/**
 * Lightness for the `step`-th folder in a family, fanned out around the
 * family's own lightness rather than a fixed ladder — so the first folder is
 * exactly the colour that was picked and the rest stay recognisably related.
 * Clamped to what still reads on both a light and a dark canvas.
 */
export function shadeLightness(base: number, step: number): number {
  if (step <= 0) return base;
  const magnitude = 12 * Math.ceil(step / 2);
  const offset = step % 2 === 1 ? -magnitude : magnitude;
  return Math.max(24, Math.min(80, base + offset));
}

/** `hsl(h s% l%)` -> `#rrggbb`, so the colour input can show the current value. */
export function hslToHex(hsl: string): string {
  const m = /hsl\(\s*([\d.]+)\s+([\d.]+)%\s+([\d.]+)%/.exec(hsl);
  if (!m) return "#4aa8ff";
  const h = Number(m[1]) / 360;
  const s = Number(m[2]) / 100;
  const l = Number(m[3]) / 100;
  const f = (n: number) => {
    const k = (n + h * 12) % 12;
    const a = s * Math.min(l, 1 - l);
    const v = l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(v * 255)
      .toString(16)
      .padStart(2, "0");
  };
  return `#${f(0)}${f(8)}${f(4)}`;
}

/**
 * Colour every note by the folder it lives in. Notes sharing a top-level folder
 * share a hue, and each subfolder shifts the lightness — so an imported blog
 * reads as one family of colours whose categories are still told apart, rather
 * than a flat wall of one colour.
 */
export function folderColors(
  paths: string[],
  custom: Record<string, string> = {}
): {
  colorOf: Map<string, string>;
  legend: { name: string; color: string }[];
} {
  const dirs = Array.from(new Set(paths.map(dirOf))).sort();
  const tops = Array.from(new Set(dirs.map((d) => d.split("/")[0]))).sort();
  // A picked colour is used whole. Only the untouched folders fall back to the
  // generated palette, where root notes stay deliberately neutral.
  const baseOf = new Map<string, Hsl>(
    tops.map((t, i) => {
      const picked = custom[t] !== undefined ? hexToHsl(custom[t]) : null;
      if (picked) return [t, picked];
      return [
        t,
        t === ""
          ? { h: HUES[i % HUES.length], s: 8, l: 62 }
          : { h: HUES[i % HUES.length], s: 66, l: 56 },
      ];
    })
  );
  const fallback: Hsl = { h: 205, s: 66, l: 56 };
  const seenPerTop = new Map<string, number>();
  const colorOf = new Map<string, string>();
  for (const dir of dirs) {
    const top = dir.split("/")[0];
    const base = baseOf.get(top) ?? fallback;
    const n = seenPerTop.get(top) ?? 0;
    seenPerTop.set(top, n + 1);
    // `dirs` is sorted, so a family's own folder comes first and keeps the
    // colour exactly; its subfolders fan out around it.
    colorOf.set(dir, `hsl(${base.h} ${base.s}% ${shadeLightness(base.l, n)}%)`);
  }
  const legend = tops.map((t) => {
    const base = baseOf.get(t) ?? fallback;
    return {
      name: t === "" ? "—" : t,
      color: `hsl(${base.h} ${base.s}% ${base.l}%)`,
    };
  });
  return { colorOf, legend };
}
