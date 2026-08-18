/**
 * Readability checks for a colour scheme.
 *
 * The palette editor lets any colour meet any other, which includes grey text
 * on a grey background. These are the pairs where that stops being a taste
 * question and starts being text nobody can read.
 *
 * Ratios follow WCAG 2.1: relative luminance, `(lighter + .05) / (darker + .05)`.
 */
import type { Palette } from "./theme";

/** WCAG AA for normal-size body text. Everything checked here is body text. */
export const AA_NORMAL = 4.5;

function toRgb(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  const v = parseInt(m[1], 16);
  return [(v >> 16) & 255, (v >> 8) & 255, v & 255];
}

/** WCAG relative luminance of an sRGB colour, 0 (black) to 1 (white). */
export function relativeLuminance(hex: string): number | null {
  const rgb = toRgb(hex);
  if (!rgb) return null;
  const [r, g, b] = rgb.map((c) => {
    const x = c / 255;
    return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** Contrast ratio between two colours, 1 (identical) to 21 (black on white). */
export function contrastRatio(a: string, b: string): number | null {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  if (la === null || lb === null) return null;
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

/** `fg` laid over `bg` at `alpha`, as the opaque colour that results. */
export function blendOver(fg: string, bg: string, alpha: number): string | null {
  const f = toRgb(fg);
  const b = toRgb(bg);
  if (!f || !b) return null;
  const mix = f.map((c, i) => Math.round(c * alpha + b[i] * (1 - alpha)));
  return `#${mix.map((c) => c.toString(16).padStart(2, "0")).join("")}`;
}

/**
 * How much of the highlight colour actually lands on the page.
 * Mirrors `color-mix(in srgb, var(--magma-highlight) 45%, transparent)` in
 * styles/index.css — a highlight is never painted at full strength, so
 * checking the raw colour would report a contrast that never occurs.
 */
export const HIGHLIGHT_ALPHA = 0.45;

export interface ContrastIssue {
  /** Names the pair, for the message. */
  pair: "inkOnBg" | "inkOnPanel" | "mutedOnBg" | "mutedOnPanel" | "inkOnHighlight";
  ratio: number;
}

/**
 * Every text-against-background pair in a palette that falls below AA.
 *
 * Deliberately only text pairs. The accent and AI colours are markers, button
 * fills and graph dots rather than prose, and holding them to the body-text
 * standard would flag the shipped defaults — a warning that fires on an
 * untouched install teaches people to ignore warnings.
 */
export function contrastIssues(p: Palette): ContrastIssue[] {
  const highlighted = blendOver(p.highlight, p.bg, HIGHLIGHT_ALPHA);
  const pairs: [ContrastIssue["pair"], string, string | null][] = [
    ["inkOnBg", p.ink, p.bg],
    ["inkOnPanel", p.ink, p.panel],
    ["mutedOnBg", p.muted, p.bg],
    ["mutedOnPanel", p.muted, p.panel],
    ["inkOnHighlight", p.ink, highlighted],
  ];
  const issues: ContrastIssue[] = [];
  for (const [pair, fg, bg] of pairs) {
    if (bg === null) continue;
    const ratio = contrastRatio(fg, bg);
    // An unparseable colour is not a contrast problem — it is a different
    // problem, and guessing a ratio for it would only be noise.
    if (ratio === null || ratio >= AA_NORMAL) continue;
    issues.push({ pair, ratio });
  }
  return issues;
}
