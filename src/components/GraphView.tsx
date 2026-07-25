import { useEffect, useRef, useState } from "react";
import type { Graph } from "../lib/api";
import { useI18n } from "../lib/i18n";

interface GraphViewProps {
  graph: Graph;
  activePath: string | null;
  /** Clicking a node previews it; the panel's button opens it in the editor. */
  onSelect: (path: string) => void;
}

interface Sim {
  path: string;
  title: string;
  ai: boolean;
  missing: boolean;
  degree: number;
  x: number;
  y: number;
}

// Read the live theme colors so the graph follows accent/AI customization.
function themeColor(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}
const ACCENT_FALLBACK = "#e0533d";
const AI_FALLBACK = "#7c5cff";
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Ideal link length in world units. The view auto-fits, so only ratios matter. */
const K = 55;
/**
 * Pull toward the centre, proportional to distance. Without it, groups that
 * share no links (an imported blog and your own notes, say) only ever repel
 * each other and drift apart until they sit in opposite corners.
 */
const GRAVITY = 0.22;
/**
 * Repulsion is ignored beyond this distance. Far-apart clusters stop shoving
 * one another, and skipping distant pairs keeps big vaults fast.
 */
const REPULSION_CUTOFF = K * 25;
/** Node radius in *screen* pixels — constant, so nodes stay visible when zoomed out. */
const nodeRadius = (degree: number) => 2.5 + Math.min(7, degree * 1.2);

/** Distinct hues, one per top-level folder. */
const HUES = [205, 145, 332, 40, 265, 190, 355, 95, 22, 240, 170, 300];

function dirOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? "" : path.slice(0, i);
}

/** Hue of a #rrggbb colour, so a picked colour still drives the subfolder shades. */
function hueFromHex(hex: string): number {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return 205;
  const v = parseInt(m[1], 16);
  const r = ((v >> 16) & 255) / 255;
  const g = ((v >> 8) & 255) / 255;
  const b = (v & 255) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  if (d === 0) return 0;
  let h: number;
  if (max === r) h = ((g - b) / d) % 6;
  else if (max === g) h = (b - r) / d + 2;
  else h = (r - g) / d + 4;
  h *= 60;
  return h < 0 ? h + 360 : h;
}

const COLOR_KEY = "magma.folderColors";

/** `hsl(h s% l%)` -> `#rrggbb`, so the colour input can show the current value. */
function hslToHex(hsl: string): string {
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
function folderColors(
  paths: string[],
  custom: Record<string, string> = {}
): {
  colorOf: Map<string, string>;
  legend: { name: string; color: string }[];
} {
  const dirs = Array.from(new Set(paths.map(dirOf))).sort();
  const tops = Array.from(new Set(dirs.map((d) => d.split("/")[0]))).sort();
  const hueOf = new Map(
    tops.map((t, i) => [t, custom[t] !== undefined ? hueFromHex(custom[t]) : HUES[i % HUES.length]])
  );
  const seenPerTop = new Map<string, number>();
  const colorOf = new Map<string, string>();
  for (const dir of dirs) {
    const top = dir.split("/")[0];
    const hue = hueOf.get(top) ?? 205;
    const n = seenPerTop.get(top) ?? 0;
    seenPerTop.set(top, n + 1);
    // Root notes stay neutral; subfolders fan out in lightness.
    const light = dir === "" ? 62 : 46 + ((n * 9) % 30);
    const sat = dir === "" ? 8 : 66;
    colorOf.set(dir, `hsl(${hue} ${sat}% ${light}%)`);
  }
  const legend = tops.map((t) => ({
    name: t === "" ? "—" : t,
    color: `hsl(${hueOf.get(t) ?? 205} ${t === "" ? 8 : 66}% 56%)`,
  }));
  return { colorOf, legend };
}

/**
 * The graph — Magma's headline view. A Fruchterman-Reingold force layout on a
 * canvas: repulsion `k²/d` between every pair, attraction `d²/k` along links,
 * with a cooling temperature. Because `k` is a fixed ideal distance rather than
 * a hand-tuned constant, the layout behaves the same for 20 notes and for 2000.
 *
 * Nodes, links and labels are drawn in screen space, so zooming out shrinks the
 * layout but never the dots — a 650-note vault stays readable instead of
 * collapsing into invisible specks.
 *
 * Interaction is Obsidian-style: scroll to zoom (toward the cursor), drag the
 * empty canvas to pan, drag a node to reposition it. Until you touch it, the
 * view auto-fits so every node is on screen.
 */
export default function GraphView({ graph, activePath, onSelect }: GraphViewProps) {
  const { t } = useI18n();
  const ACCENT = themeColor("--magma-accent", ACCENT_FALLBACK);
  const AI = themeColor("--magma-ai", AI_FALLBACK);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const activeRef = useRef(activePath);
  activeRef.current = activePath;
  // Persistent viewport: scale + screen-space pan offset (ox, oy).
  const viewRef = useRef({ scale: 1, ox: 0, oy: 0 });
  // While false, the view auto-fits to show every node; any pan/zoom/drag hands
  // control to the user. "Reset view" gives it back.
  const interactedRef = useRef(false);
  const [legend, setLegend] = useState<{ name: string; color: string }[]>([]);
  const [picker, setPicker] = useState(false);
  // Per-top-level-folder colour overrides, remembered across restarts.
  const [custom, setCustom] = useState<Record<string, string>>(() => {
    try {
      return JSON.parse(localStorage.getItem(COLOR_KEY) ?? "{}");
    } catch {
      return {};
    }
  });
  // The render loop reads colours through a ref, so changing one repaints
  // without rebuilding (and re-laying-out) the whole simulation.
  const [showAiRing, setShowAiRing] = useState(() => localStorage.getItem("magma.aiRing") !== "0");
  const showAiRingRef = useRef(showAiRing);
  useEffect(() => {
    showAiRingRef.current = showAiRing;
    localStorage.setItem("magma.aiRing", showAiRing ? "1" : "0");
  }, [showAiRing]);
  const customRef = useRef(custom);
  const colorVersion = useRef(0);
  useEffect(() => {
    customRef.current = custom;
    colorVersion.current++;
    localStorage.setItem(COLOR_KEY, JSON.stringify(custom));
  }, [custom]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Seed on a circle sized to the vault (no RNG, so layouts are reproducible).
    const n = graph.nodes.length;
    const seed = K * Math.sqrt(Math.max(1, n)) * 0.5;
    const nodes: Sim[] = graph.nodes.map((node, i) => {
      const a = (i / Math.max(1, n)) * Math.PI * 2;
      return {
        path: node.path,
        title: node.title,
        ai: node.aiAuthored,
        missing: !!node.missing,
        degree: node.degree,
        x: Math.cos(a) * seed,
        y: Math.sin(a) * seed,
      };
    });
    interactedRef.current = false; // a fresh graph starts in auto-fit mode

    // Colours are recomputed only when an override changes, not every frame.
    let nodeColor: string[] = [];
    let colorsBuiltAt = -1;
    const ensureColors = () => {
      if (colorsBuiltAt === colorVersion.current) return;
      colorsBuiltAt = colorVersion.current;
      const { colorOf, legend: folderLegend } = folderColors(
        graph.nodes.map((n) => n.path),
        customRef.current
      );
      setLegend(folderLegend);
      nodeColor = graph.nodes.map((n) => colorOf.get(dirOf(n.path)) ?? ACCENT);
      linksByColor.clear();
      for (const [a, b] of links) {
        const key = nodeColor[nodes[a].degree >= nodes[b].degree ? a : b];
        const group = linksByColor.get(key);
        if (group) group.push([a, b]);
        else linksByColor.set(key, [[a, b]]);
      }
    };

    const indexOf = new Map(nodes.map((s, i) => [s.path, i]));
    const links: [number, number][] = [];
    for (const e of graph.edges) {
      const a = indexOf.get(e.source);
      const b = indexOf.get(e.target);
      if (a !== undefined && b !== undefined && a !== b) links.push([a, b]);
    }
    // Links are grouped by colour (of their busier end) so a big vault still
    // draws in a handful of paths. Rebuilt whenever a colour changes.
    const linksByColor = new Map<string, [number, number][]>();

    // Node indices, most-connected first: labels are placed in this order so the
    // hubs that orient the map win the space.
    const labelOrder = nodes
      .map((_, i) => i)
      .sort((a, b) => nodes[b].degree - nodes[a].degree);
    // Name the busiest hubs even when zoomed out.
    const hubs = new Set(
      labelOrder
        .slice(0, 14)
        .filter((i) => nodes[i].degree > 0)
        .map((i) => nodes[i].path)
    );

    let raf = 0;
    let needsDraw = true;
    let dragIdx = -1;
    // Max displacement per step; cools until the layout settles.
    let temp = K * 2;
    const disp = new Float64Array(n * 2);

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = Math.max(1, rect.width * dpr);
      canvas.height = Math.max(1, rect.height * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      needsDraw = true;
    };
    resize();
    window.addEventListener("resize", resize);

    const step = () => {
      disp.fill(0);
      // Repulsion between every pair: k²/d.
      for (let i = 0; i < n; i++) {
        const a = nodes[i];
        for (let j = i + 1; j < n; j++) {
          const b = nodes[j];
          let dx = a.x - b.x;
          let dy = a.y - b.y;
          let d2 = dx * dx + dy * dy;
          if (d2 > REPULSION_CUTOFF * REPULSION_CUTOFF) continue;
          if (d2 < 0.01) {
            // Coincident nodes: nudge them apart deterministically.
            dx = ((i % 13) + 1) * 0.05;
            dy = ((j % 7) + 1) * 0.05;
            d2 = dx * dx + dy * dy;
          }
          const d = Math.sqrt(d2);
          const f = (K * K) / d;
          const ux = (dx / d) * f;
          const uy = (dy / d) * f;
          disp[i * 2] += ux;
          disp[i * 2 + 1] += uy;
          disp[j * 2] -= ux;
          disp[j * 2 + 1] -= uy;
        }
      }
      // Attraction along links: d²/k.
      for (const [i, j] of links) {
        const a = nodes[i];
        const b = nodes[j];
        const dx = a.x - b.x;
        const dy = a.y - b.y;
        const d = Math.sqrt(dx * dx + dy * dy) || 0.01;
        const f = (d * d) / K;
        const ux = (dx / d) * f;
        const uy = (dy / d) * f;
        disp[i * 2] -= ux;
        disp[i * 2 + 1] -= uy;
        disp[j * 2] += ux;
        disp[j * 2 + 1] += uy;
      }
      // Gravity toward the centre keeps unlinked groups in one picture.
      for (let i = 0; i < n; i++) {
        disp[i * 2] -= nodes[i].x * GRAVITY;
        disp[i * 2 + 1] -= nodes[i].y * GRAVITY;
      }
      // Apply, capped by the temperature. The dragged node stays under the cursor.
      let sx = 0;
      let sy = 0;
      for (let i = 0; i < n; i++) {
        if (i !== dragIdx) {
          const dx = disp[i * 2];
          const dy = disp[i * 2 + 1];
          const dl = Math.sqrt(dx * dx + dy * dy) || 1;
          const m = Math.min(dl, temp);
          nodes[i].x += (dx / dl) * m;
          nodes[i].y += (dy / dl) * m;
        }
        sx += nodes[i].x;
        sy += nodes[i].y;
      }
      // Re-centre on the centroid so the layout never drifts away.
      const cx = sx / n;
      const cy = sy / n;
      for (let i = 0; i < n; i++) {
        nodes[i].x -= cx;
        nodes[i].y -= cy;
      }
      temp *= 0.98;
    };

    // World -> screen and back, for hit-testing and dragging.
    const toScreen = (s: Sim, rect: DOMRect) => {
      const { scale, ox, oy } = viewRef.current;
      return { x: rect.width / 2 + ox + s.x * scale, y: rect.height / 2 + oy + s.y * scale };
    };
    const toWorld = (mx: number, my: number, rect: DOMRect) => {
      const { scale, ox, oy } = viewRef.current;
      return { x: (mx - rect.width / 2 - ox) / scale, y: (my - rect.height / 2 - oy) / scale };
    };
    const nodeAt = (mx: number, my: number, rect: DOMRect): number => {
      let best = -1;
      let bestD = Infinity;
      for (let i = 0; i < n; i++) {
        const p = toScreen(nodes[i], rect);
        const d = Math.hypot(p.x - mx, p.y - my);
        if (d < nodeRadius(nodes[i].degree) + 5 && d < bestD) {
          best = i;
          bestD = d;
        }
      }
      return best;
    };

    // Fit every node into view (until the user takes control).
    const fitView = () => {
      if (!n) return;
      let minX = Infinity;
      let minY = Infinity;
      let maxX = -Infinity;
      let maxY = -Infinity;
      for (const s of nodes) {
        if (s.x < minX) minX = s.x;
        if (s.x > maxX) maxX = s.x;
        if (s.y < minY) minY = s.y;
        if (s.y > maxY) maxY = s.y;
      }
      const rect = canvas.getBoundingClientRect();
      // Leave room for the largest node's screen radius plus a margin.
      const pad = 28;
      const w = maxX - minX || 1;
      const h = maxY - minY || 1;
      const scale = clamp(
        Math.min((rect.width - pad * 2) / w, (rect.height - pad * 2) / h),
        0.02,
        2
      );
      const v = viewRef.current;
      const ox = -((minX + maxX) / 2) * scale;
      const oy = -((minY + maxY) / 2) * scale;
      if (
        Math.abs(v.scale - scale) > 1e-4 ||
        Math.abs(v.ox - ox) > 0.5 ||
        Math.abs(v.oy - oy) > 0.5
      ) {
        v.scale = scale;
        v.ox = ox;
        v.oy = oy;
        needsDraw = true;
      }
    };

    const draw = () => {
      ensureColors();
      const rect = canvas.getBoundingClientRect();
      ctx.clearRect(0, 0, rect.width, rect.height);
      // Everything below is screen space: positions are transformed, sizes aren't.
      const pts = nodes.map((s) => toScreen(s, rect));

      // Links carry their target's colour, batched per colour so a big vault
      // still draws in a handful of paths.
      ctx.lineWidth = 1;
      ctx.globalAlpha = 0.22;
      for (const [color, group] of linksByColor) {
        ctx.strokeStyle = color;
        ctx.beginPath();
        for (const [i, j] of group) {
          ctx.moveTo(pts[i].x, pts[i].y);
          ctx.lineTo(pts[j].x, pts[j].y);
        }
        ctx.stroke();
      }
      ctx.globalAlpha = 1;

      const onScreen = (p: { x: number; y: number }) =>
        p.x > -40 && p.y > -40 && p.x < rect.width + 40 && p.y < rect.height + 40;

      for (let i = 0; i < n; i++) {
        const s = nodes[i];
        const p = pts[i];
        if (!onScreen(p)) continue; // cheap win on large vaults
        const isActive = s.path === activeRef.current;
        const r = nodeRadius(s.degree) + (isActive ? 2 : 0);
        const color = nodeColor[i];
        if (s.missing) {
          // A link target with no note: hollow, so it reads as "not there yet".
          ctx.beginPath();
          ctx.arc(p.x, p.y, r, 0, Math.PI * 2);
          ctx.strokeStyle = "rgba(150,145,138,0.85)";
          ctx.lineWidth = 1.25;
          ctx.setLineDash([2, 2]);
          ctx.stroke();
          ctx.setLineDash([]);
        } else {
          // Soft halo first, solid dot on top: gives the map some depth instead
          // of reading as flat stickers.
          ctx.beginPath();
          ctx.arc(p.x, p.y, r * 2.1, 0, Math.PI * 2);
          ctx.fillStyle = color;
          ctx.globalAlpha = 0.13;
          ctx.fill();
          ctx.beginPath();
          ctx.arc(p.x, p.y, r, 0, Math.PI * 2);
          ctx.globalAlpha = isActive ? 1 : 0.92;
          ctx.fill();
          ctx.globalAlpha = 1;
          // AI-written notes keep a signal, but a quiet one — the folder colour
          // is the primary reading, the ring must not shout over it.
          if (s.ai && showAiRingRef.current) {
            ctx.strokeStyle = AI;
            ctx.globalAlpha = 0.5;
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.arc(p.x, p.y, r + 2.5, 0, Math.PI * 2);
            ctx.stroke();
            ctx.globalAlpha = 1;
          }
        }
        if (isActive) {
          ctx.strokeStyle = color;
          ctx.lineWidth = 2;
          ctx.beginPath();
          ctx.arc(p.x, p.y, r + 5, 0, Math.PI * 2);
          ctx.stroke();
        }
      }

      // Labels last, most-connected first, and never on top of each other.
      // Without this a 600-note vault draws 600 overlapping titles — a grey
      // smear that hides the graph completely.
      const scale = viewRef.current.scale;
      const budget = scale > 0.6 ? 70 : 22;
      const boxes: { x1: number; y1: number; x2: number; y2: number }[] = [];
      ctx.font = "11px Inter, system-ui, sans-serif";
      ctx.textBaseline = "middle";
      ctx.fillStyle = "rgba(140,140,140,0.95)";
      let drawn = 0;
      for (const i of labelOrder) {
        if (drawn >= budget) break;
        const s = nodes[i];
        const p = pts[i];
        if (!onScreen(p)) continue;
        const isActive = s.path === activeRef.current;
        if (!isActive && !hubs.has(s.path) && s.degree < 1) continue;
        const x = p.x + nodeRadius(s.degree) + 4;
        const w = ctx.measureText(s.title).width;
        const box = { x1: x - 2, y1: p.y - 8, x2: x + w + 2, y2: p.y + 8 };
        if (
          boxes.some(
            (b) => !(box.x2 < b.x1 || box.x1 > b.x2 || box.y2 < b.y1 || box.y1 > b.y2)
          )
        ) {
          continue; // would overlap a label already drawn
        }
        boxes.push(box);
        ctx.fillText(s.title, x, p.y);
        drawn++;
      }
    };

    // Once settled the layout keeps breathing very gently rather than freezing —
    // a still graph looks dead. The residual temperature is small enough that
    // nodes only drift within their cluster, and stepping every other frame
    // halves the cost of doing so forever.
    const IDLE_TEMP = 0.4;
    let frameNo = 0;
    const frame = () => {
      frameNo++;
      const settling = temp > IDLE_TEMP;
      if (settling || dragIdx >= 0 || frameNo % 2 === 0) {
        if (!settling && dragIdx < 0) temp = IDLE_TEMP; // keep it just alive
        step();
        needsDraw = true;
      }
      if (!interactedRef.current) fitView();
      if (needsDraw) {
        draw();
        needsDraw = false;
      }
      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);

    // --- Interaction ---------------------------------------------------------
    let panning = false;
    let moved = false;
    let downX = 0;
    let downY = 0;
    let lastX = 0;
    let lastY = 0;

    const onPointerDown = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      downX = lastX = mx;
      downY = lastY = my;
      moved = false;
      const hit = nodeAt(mx, my, rect);
      if (hit >= 0) {
        dragIdx = hit;
        temp = Math.max(temp, K * 0.25); // reheat so neighbours make room
      } else {
        panning = true;
        interactedRef.current = true; // panning takes over the viewport
      }
      canvas.style.cursor = "grabbing";
      canvas.setPointerCapture(e.pointerId);
    };

    const onPointerMove = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      if (dragIdx < 0 && !panning) {
        canvas.style.cursor = nodeAt(mx, my, rect) >= 0 ? "grab" : "default";
        return;
      }
      if (Math.hypot(mx - downX, my - downY) > 3) moved = true;
      if (dragIdx >= 0) {
        if (moved) interactedRef.current = true; // don't refit mid-drag
        const w = toWorld(mx, my, rect);
        nodes[dragIdx].x = w.x;
        nodes[dragIdx].y = w.y;
        needsDraw = true;
      } else if (panning) {
        viewRef.current.ox += mx - lastX;
        viewRef.current.oy += my - lastY;
        needsDraw = true;
      }
      lastX = mx;
      lastY = my;
    };

    const onPointerUp = (e: PointerEvent) => {
      // A press without a drag is a click → open the note.
      if (!moved && dragIdx >= 0) onSelect(nodes[dragIdx].path);
      dragIdx = -1;
      panning = false;
      canvas.style.cursor = "default";
      try {
        canvas.releasePointerCapture(e.pointerId);
      } catch {
        /* already released */
      }
    };

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      interactedRef.current = true; // zooming takes over the viewport
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const view = viewRef.current;
      // The world point under the cursor stays put as we zoom.
      const w = toWorld(mx, my, rect);
      view.scale = clamp(view.scale * Math.exp(-e.deltaY * 0.0015), 0.02, 8);
      view.ox = mx - rect.width / 2 - w.x * view.scale;
      view.oy = my - rect.height / 2 - w.y * view.scale;
      needsDraw = true;
    };

    canvas.addEventListener("pointerdown", onPointerDown);
    canvas.addEventListener("pointermove", onPointerMove);
    canvas.addEventListener("pointerup", onPointerUp);
    canvas.addEventListener("wheel", onWheel, { passive: false });

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      canvas.removeEventListener("pointerdown", onPointerDown);
      canvas.removeEventListener("pointermove", onPointerMove);
      canvas.removeEventListener("pointerup", onPointerUp);
      canvas.removeEventListener("wheel", onWheel);
    };
    // Rebuild the simulation whenever the graph shape changes.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [graph]);

  return (
    <div className="relative h-full w-full">
      {graph.nodes.length === 0 && (
        <div className="absolute inset-0 grid place-items-center text-sm text-magma-muted">
          {t("graph.empty")}
        </div>
      )}
      <canvas ref={canvasRef} className="h-full w-full touch-none" />
      <div className="absolute left-4 top-3 flex items-center gap-2">
        <button
          onClick={() => {
            interactedRef.current = false; // hand control back to auto-fit
          }}
          title={t("graph.reset")}
          className="rounded-md bg-black/5 px-2.5 py-1 text-xs text-magma-muted transition hover:bg-black/10 dark:bg-white/10 dark:hover:bg-white/20"
        >
          {t("graph.reset")}
        </button>
        <button
          onClick={() => setPicker((p) => !p)}
          className="rounded-md bg-black/5 px-2.5 py-1 text-xs text-magma-muted transition hover:bg-black/10 dark:bg-white/10 dark:hover:bg-white/20"
        >
          {t("graph.colors")}
        </button>
      </div>

      {picker && (
        <div className="absolute left-4 top-12 z-10 w-60 rounded-xl bg-magma-bg/95 p-3 shadow-xl backdrop-blur dark:bg-[#201c19]/95">
          <p className="mb-2 text-xs text-magma-muted">{t("graph.colorsHint")}</p>
          <div className="flex max-h-64 flex-col gap-1.5 overflow-auto">
            {legend.map((l) => (
              <label key={l.name} className="flex items-center gap-2 text-sm">
                <input
                  type="color"
                  value={custom[l.name] ?? hslToHex(l.color)}
                  onChange={(e) =>
                    setCustom((c) => ({ ...c, [l.name]: e.target.value }))
                  }
                  className="h-6 w-8 cursor-pointer rounded border-0 bg-transparent p-0"
                />
                <span className="truncate">{l.name}</span>
              </label>
            ))}
          </div>
          <label className="mt-2 flex items-center gap-2 border-t border-black/5 pt-2 text-sm dark:border-white/10">
            <input
              type="checkbox"
              checked={showAiRing}
              onChange={(e) => setShowAiRing(e.target.checked)}
            />
            <span>{t("graph.aiRing")}</span>
          </label>
          <button
            onClick={() => setCustom({})}
            className="mt-2 w-full rounded-md bg-black/5 px-2 py-1 text-xs text-magma-muted transition hover:bg-black/10 dark:bg-white/10 dark:hover:bg-white/20"
          >
            {t("graph.colorsReset")}
          </button>
        </div>
      )}
      {/* One entry per top-level folder, plus the AI ring. */}
      <div className="pointer-events-none absolute bottom-3 right-4 flex max-w-[70%] flex-wrap justify-end gap-x-4 gap-y-1 text-xs text-magma-muted">
        {legend.slice(0, 8).map((l) => (
          <span key={l.name} className="flex items-center gap-1.5">
            <span className="h-2 w-2 rounded-full" style={{ background: l.color }} />
            <span className="max-w-[10rem] truncate">{l.name}</span>
          </span>
        ))}
        {showAiRing && (
          <span className="flex items-center gap-1.5">
            <span
              className="h-2.5 w-2.5 rounded-full border-2"
              style={{ borderColor: AI }}
            />
            {t("graph.legendAi")}
          </span>
        )}
        <span className="flex items-center gap-1.5">
          <span className="h-2.5 w-2.5 rounded-full border border-dashed border-magma-muted" />
          {t("graph.legendMissing")}
        </span>
      </div>
    </div>
  );
}
