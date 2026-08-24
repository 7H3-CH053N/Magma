import { describe, expect, it } from "vitest";
import {
  folderColors,
  hexToHsl,
  hslToHex,
  notePaths,
  shadeLightness,
} from "./graphColors";

/** The colour the graph actually paints for the notes directly in `folder`. */
function colorOf(folder: string, picked: string): string {
  const { colorOf: map } = folderColors([`${folder}/a.md`], { [folder]: picked });
  return map.get(folder) ?? "";
}

describe("a picked colour is the colour that gets drawn", () => {
  // The reported bug: the picker returned a pale red and the graph drew a full
  // one. Only the hue survived, and saturation and lightness were replaced by
  // fixed values.
  it("keeps a pale colour pale", () => {
    const pale = hexToHsl("#ff9999");
    expect(pale).not.toBeNull();
    // Pale means light and not fully saturated — if either is lost the dot
    // comes back as a primary.
    expect(pale!.l).toBeGreaterThan(70);
    expect(Math.round(pale!.h)).toBe(0);

    const drawn = colorOf("Blog", "#ff9999");
    expect(drawn).toBe(`hsl(${pale!.h} ${pale!.s}% ${pale!.l}%)`);
    // And it survives the round trip back into the picker.
    expect(hslToHex(drawn)).toBe("#ff9999");
  });

  // The second half of the report. Grey has no hue at all, and the old code
  // returned 0 for it — which is red. Picking grey produced a red dot.
  it("keeps a grey grey instead of turning it red", () => {
    const grey = hexToHsl("#9a9a9a");
    expect(grey).not.toBeNull();
    expect(grey!.s).toBe(0);

    const drawn = colorOf("Archiv", "#9a9a9a");
    expect(drawn).toContain(" 0%");
    expect(hslToHex(drawn)).toBe("#9a9a9a");
  });

  it("round-trips the colours a user is most likely to reach for", () => {
    for (const hex of ["#ff9999", "#9a9a9a", "#000000", "#ffffff", "#e0533d", "#7c5cff", "#1c1a17"]) {
      const hsl = hexToHsl(hex);
      expect(hsl, hex).not.toBeNull();
      expect(hslToHex(`hsl(${hsl!.h} ${hsl!.s}% ${hsl!.l}%)`), hex).toBe(hex);
    }
  });

  it("refuses what is not a colour rather than inventing one", () => {
    expect(hexToHsl("")).toBeNull();
    expect(hexToHsl("rot")).toBeNull();
    expect(hexToHsl("#abc")).toBeNull();
  });
});

describe("subfolders stay a family", () => {
  it("gives the folder itself exactly the picked lightness", () => {
    expect(shadeLightness(80, 0)).toBe(80);
    expect(shadeLightness(31, 0)).toBe(31);
  });

  it("walks the band instead of drifting off in one direction", () => {
    expect([1, 2, 3, 4].map((n) => shadeLightness(56, n))).toEqual([40, 66, 50, 34]);
  });

  // The failure this replaced: a widening fan clamped to the band edges put
  // every step past the fourth on one of two values, so a folder with a dozen
  // subfolders — which is what a real vault has — came out as two flat blocks.
  it("keeps a dozen subfolders a dozen different shades", () => {
    const shades = Array.from({ length: 12 }, (_, i) => shadeLightness(56, i + 1));
    expect(new Set(shades).size).toBe(12);
  });

  it("does that from any starting lightness, including the extremes", () => {
    for (const base of [0, 12, 45, 88, 100]) {
      const shades = Array.from({ length: 10 }, (_, i) => shadeLightness(base, i + 1));
      expect(new Set(shades).size, `base ${base}`).toBe(10);
    }
  });

  it("keeps shades legible on both a light and a dark canvas", () => {
    for (const base of [0, 5, 50, 95, 100]) {
      for (let n = 0; n <= 12; n++) {
        const l = shadeLightness(base, n);
        if (n > 0) {
          expect(l, `base ${base} step ${n}`).toBeGreaterThanOrEqual(30);
          expect(l, `base ${base} step ${n}`).toBeLessThanOrEqual(72);
        }
      }
    }
  });

  it("shades a subfolder from its parent's hue and saturation", () => {
    const { colorOf: map } = folderColors(["Blog/a.md", "Blog/KI/b.md"], {
      Blog: "#ff9999",
    });
    const parent = hexToHsl("#ff9999")!;
    expect(map.get("Blog")).toBe(`hsl(${parent.h} ${parent.s}% ${parent.l}%)`);
    // Same hue and saturation, different lightness — recognisably related.
    expect(map.get("Blog/KI")).toBe(
      `hsl(${parent.h} ${parent.s}% ${shadeLightness(parent.l, 1)}%)`
    );
  });
});

describe("untouched folders keep the generated palette", () => {
  it("leaves root notes deliberately neutral", () => {
    const { colorOf: map, legend } = folderColors(["a.md", "Blog/b.md"]);
    expect(map.get("")).toContain(" 8%");
    expect(legend.find((l) => l.name === "—")).toBeDefined();
  });

  it("gives two folders two different colours", () => {
    const { legend } = folderColors(["Blog/a.md", "Projekte/b.md"]);
    const colors = legend.map((l) => l.color);
    expect(new Set(colors).size).toBe(colors.length);
  });

  it("shows the picked colour in the legend, unchanged", () => {
    const { legend } = folderColors(["Blog/a.md"], { Blog: "#ff9999" });
    const entry = legend.find((l) => l.name === "Blog");
    expect(hslToHex(entry!.color)).toBe("#ff9999");
  });
});

describe("ghost nodes are not folders", () => {
  // Straight from a real vault: a wikilink whose target is itself a markdown
  // link. The slash in `http://` used to read as a folder boundary, so the
  // legend grew an entry called `missing:[ai.rs](http:` with a colour of its
  // own — for a node that is drawn hollow and never uses one.
  const nodes = [
    { path: "Blog/a.md" },
    { path: "missing:[ai.rs](http://example.test/ai.rs)", missing: true },
    { path: "missing:[history.rs](http://example.test/history.rs)", missing: true },
  ];

  it("leaves them out of the paths the palette is built from", () => {
    expect(notePaths(nodes)).toEqual(["Blog/a.md"]);
  });

  it("keeps them out of the legend", () => {
    const { legend } = folderColors(notePaths(nodes));
    expect(legend.map((l) => l.name)).toEqual(["Blog"]);
  });

  it("would otherwise invent a folder from a URL", () => {
    // Kept as the reason the filter exists: without it, this is the legend.
    const { legend } = folderColors(nodes.map((n) => n.path));
    expect(legend.map((l) => l.name)).toContain("missing:[ai.rs](http:");
  });
});
