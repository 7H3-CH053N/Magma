import { describe, expect, it } from "vitest";
import { DEFAULT_DARK, DEFAULT_LIGHT, type Palette } from "./theme";
import {
  AA_NORMAL,
  HIGHLIGHT_ALPHA,
  blendOver,
  contrastIssues,
  contrastRatio,
  relativeLuminance,
} from "./contrast";

describe("the ratio itself", () => {
  it("matches the WCAG anchors", () => {
    expect(contrastRatio("#000000", "#ffffff")).toBeCloseTo(21, 5);
    expect(contrastRatio("#ffffff", "#ffffff")).toBeCloseTo(1, 5);
    // Symmetric: which one is the text makes no difference to the ratio.
    expect(contrastRatio("#1c1a17", "#faf9f7")).toBe(contrastRatio("#faf9f7", "#1c1a17"));
  });

  it("weights green the way luminance does, not the way a byte average would", () => {
    // Pure green is far brighter than pure blue at the same byte value; an
    // unweighted mean would call them equal and pass unreadable schemes.
    expect(relativeLuminance("#00ff00")!).toBeGreaterThan(relativeLuminance("#0000ff")!);
  });

  it("refuses what is not a colour instead of returning a number", () => {
    expect(contrastRatio("nope", "#ffffff")).toBeNull();
    expect(relativeLuminance("#abc")).toBeNull();
    expect(blendOver("#fff", "#000000", 0.5)).toBeNull();
  });
});

describe("a highlight is checked as it is actually painted", () => {
  it("blends toward the background, not the raw colour", () => {
    expect(blendOver("#ffffff", "#000000", 0.5)).toBe("#808080");
    expect(blendOver("#d7d323", "#faf9f7", 0.45)).toBe("#eae898");
  });

  it("spares the default dark scheme a false alarm", () => {
    // Measured: the raw highlight against dark ink is 1.31, which would look
    // like a serious problem. What is actually painted is 4.59 and perfectly
    // readable. Checking the unblended colour would warn on a fresh install.
    const raw = contrastRatio(DEFAULT_DARK.ink, DEFAULT_DARK.highlight)!;
    const painted = contrastRatio(
      DEFAULT_DARK.ink,
      blendOver(DEFAULT_DARK.highlight, DEFAULT_DARK.bg, HIGHLIGHT_ALPHA)!
    )!;
    expect(raw).toBeLessThan(AA_NORMAL);
    expect(painted).toBeGreaterThan(AA_NORMAL);
  });
});

describe("the shipped schemes", () => {
  // A warning that fires on an untouched install teaches people to ignore
  // warnings. If a default ever drops below AA, this is where it gets caught.
  it("pass without a single warning", () => {
    expect(contrastIssues(DEFAULT_LIGHT)).toEqual([]);
    expect(contrastIssues(DEFAULT_DARK)).toEqual([]);
  });
});

describe("a scheme somebody has broken", () => {
  const grey: Palette = {
    ...DEFAULT_LIGHT,
    bg: "#9a9a9a",
    panel: "#9a9a9a",
    ink: "#8f8f8f",
    muted: "#a4a4a4",
  };

  it("names every unreadable pair", () => {
    const pairs = contrastIssues(grey).map((i) => i.pair);
    expect(pairs).toContain("inkOnBg");
    expect(pairs).toContain("inkOnPanel");
    expect(pairs).toContain("mutedOnBg");
    expect(pairs).toContain("mutedOnPanel");
  });

  it("reports the ratio, so the message can say how far off it is", () => {
    const issue = contrastIssues(grey).find((i) => i.pair === "inkOnBg")!;
    expect(issue.ratio).toBeLessThan(AA_NORMAL);
    expect(issue.ratio).toBeGreaterThan(1);
  });

  it("catches a highlight that swallows the text on it", () => {
    // A near-white highlight on the dark scheme: the blend lands close to the
    // light ink sitting on it. Measured at 3.30.
    const issues = contrastIssues({ ...DEFAULT_DARK, highlight: "#ffffff" });
    expect(issues.map((i) => i.pair)).toEqual(["inkOnHighlight"]);
  });

  it("cannot fire on an otherwise-default light scheme, and that is arithmetic", () => {
    // Worth stating rather than leaving as a puzzle: at 45% the blend always
    // pulls toward the background, so on a near-white page the darkest possible
    // highlight still leaves 4.97 against the ink. The check is one-sided
    // because the maths is, not because it was forgotten.
    let worst = Infinity;
    for (let v = 0; v <= 255; v++) {
      const grey = `#${v.toString(16).padStart(2, "0").repeat(3)}`;
      const painted = blendOver(grey, DEFAULT_LIGHT.bg, HIGHLIGHT_ALPHA)!;
      worst = Math.min(worst, contrastRatio(DEFAULT_LIGHT.ink, painted)!);
    }
    expect(worst).toBeGreaterThan(AA_NORMAL);
  });

  it("says nothing about a pair that is merely bold rather than unreadable", () => {
    // Accent and AI are markers, not prose — they are not checked at all, and
    // the default accent sits below AA on light, so this would fire on every
    // fresh install if they were.
    expect(contrastIssues({ ...DEFAULT_LIGHT, accent: "#faf9f7", ai: "#faf9f7" })).toEqual([]);
  });
});
