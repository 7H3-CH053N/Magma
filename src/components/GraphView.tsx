import { useEffect, useRef } from "react";
import type { Graph } from "../lib/api";
import { useI18n } from "../lib/i18n";

interface GraphViewProps {
  graph: Graph;
  activePath: string | null;
  onSelect: (path: string) => void;
}

interface Sim {
  path: string;
  title: string;
  ai: boolean;
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
const GRAVITY = 0.9;
/**
 * Repulsion is ignored beyond this distance. Far-apart clusters stop shoving
 * one another, and skipping distant pairs keeps big vaults fast.
 */
const REPULSION_CUTOFF = K * 10;
/** Node radius in *screen* pixels — constant, so nodes stay visible when zoomed out. */
const nodeRadius = (degree: number) => 2.5 + Math.min(7, degree * 1.2);

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
        degree: node.degree,
        x: Math.cos(a) * seed,
        y: Math.sin(a) * seed,
      };
    });
    interactedRef.current = false; // a fresh graph starts in auto-fit mode

    const indexOf = new Map(nodes.map((s, i) => [s.path, i]));
    const links: [number, number][] = [];
    for (const e of graph.edges) {
      const a = indexOf.get(e.source);
      const b = indexOf.get(e.target);
      if (a !== undefined && b !== undefined && a !== b) links.push([a, b]);
    }
    // Name the busiest hubs even when zoomed out — they orient the whole map.
    const hubs = new Set(
      [...nodes]
        .sort((a, b) => b.degree - a.degree)
        .slice(0, 14)
        .filter((s) => s.degree > 0)
        .map((s) => s.path)
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
      const rect = canvas.getBoundingClientRect();
      ctx.clearRect(0, 0, rect.width, rect.height);
      // Everything below is screen space: positions are transformed, sizes aren't.
      const pts = nodes.map((s) => toScreen(s, rect));

      ctx.strokeStyle = "rgba(125,125,125,0.22)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      for (const [i, j] of links) {
        ctx.moveTo(pts[i].x, pts[i].y);
        ctx.lineTo(pts[j].x, pts[j].y);
      }
      ctx.stroke();

      const scale = viewRef.current.scale;
      ctx.font = "11px Inter, system-ui, sans-serif";
      ctx.textBaseline = "middle";
      for (let i = 0; i < n; i++) {
        const s = nodes[i];
        const p = pts[i];
        // Skip anything comfortably off-screen (cheap at large vault sizes).
        if (p.x < -40 || p.y < -40 || p.x > rect.width + 40 || p.y > rect.height + 40) continue;
        const isActive = s.path === activeRef.current;
        const r = nodeRadius(s.degree) + (isActive ? 2 : 0);
        ctx.beginPath();
        ctx.arc(p.x, p.y, r, 0, Math.PI * 2);
        ctx.fillStyle = s.ai ? AI : ACCENT;
        ctx.globalAlpha = isActive ? 1 : 0.85;
        ctx.fill();
        ctx.globalAlpha = 1;
        if (isActive) {
          ctx.strokeStyle = s.ai ? AI : ACCENT;
          ctx.lineWidth = 2;
          ctx.beginPath();
          ctx.arc(p.x, p.y, r + 4, 0, Math.PI * 2);
          ctx.stroke();
        }
        // Label the hubs always; everything else once you've zoomed in.
        if (isActive || hubs.has(s.path) || (scale > 0.45 && s.degree >= 1)) {
          ctx.fillStyle = "rgba(130,130,130,0.95)";
          ctx.fillText(s.title, p.x + r + 4, p.y);
        }
      }
    };

    const frame = () => {
      if (temp > 0.3 || dragIdx >= 0) {
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
      <button
        onClick={() => {
          interactedRef.current = false; // hand control back to auto-fit
        }}
        title={t("graph.reset")}
        className="absolute left-4 top-3 rounded-md bg-black/5 px-2.5 py-1 text-xs text-magma-muted transition hover:bg-black/10 dark:bg-white/10 dark:hover:bg-white/20"
      >
        {t("graph.reset")}
      </button>
      <div className="pointer-events-none absolute bottom-3 right-4 flex gap-4 text-xs text-magma-muted">
        <span className="flex items-center gap-1">
          <span className="h-2 w-2 rounded-full" style={{ background: ACCENT }} />{" "}
          {t("graph.legendNote")}
        </span>
        <span className="flex items-center gap-1">
          <span className="h-2 w-2 rounded-full" style={{ background: AI }} />{" "}
          {t("graph.legendAi")}
        </span>
      </div>
    </div>
  );
}
