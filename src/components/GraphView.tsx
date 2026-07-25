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
  vx: number;
  vy: number;
}

// Read the live theme colors so the graph follows accent/AI customization.
function themeColor(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v || fallback;
}
const ACCENT_FALLBACK = "#e0533d";
const AI_FALLBACK = "#7c5cff";
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/**
 * The graph — Magma's headline view. A small custom force simulation on a
 * canvas keeps large vaults smooth without pulling in a graph library. Nodes
 * grow with their link degree; AI-authored notes glow violet so you can see
 * what Claude contributed.
 *
 * Interaction is Obsidian-style: scroll to zoom (toward the cursor), drag the
 * empty canvas to pan, drag a node to reposition it. The pan/zoom transform
 * lives in a ref so it survives graph rebuilds.
 */
export default function GraphView({ graph, activePath, onSelect }: GraphViewProps) {
  const { t } = useI18n();
  const ACCENT = themeColor("--magma-accent", ACCENT_FALLBACK);
  const AI = themeColor("--magma-ai", AI_FALLBACK);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const nodesRef = useRef<Sim[]>([]);
  const activeRef = useRef(activePath);
  activeRef.current = activePath;
  // Persistent viewport: scale + screen-space pan offset (ox, oy).
  const viewRef = useRef({ scale: 1, ox: 0, oy: 0 });

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Seed positions on a circle so the layout unfolds predictably (no RNG).
    const n = graph.nodes.length;
    const nodes: Sim[] = graph.nodes.map((node, i) => {
      const a = (i / Math.max(1, n)) * Math.PI * 2;
      return {
        path: node.path,
        title: node.title,
        ai: node.aiAuthored,
        degree: node.degree,
        x: Math.cos(a) * 180,
        y: Math.sin(a) * 180,
        vx: 0,
        vy: 0,
      };
    });
    nodesRef.current = nodes;
    const index = new Map(nodes.map((s) => [s.path, s]));
    const edges = graph.edges
      .map((e) => ({ s: index.get(e.source), t: index.get(e.target) }))
      .filter((e): e is { s: Sim; t: Sim } => !!e.s && !!e.t);

    let raf = 0;
    let alpha = 1;
    let needsDraw = true;
    let dragNode: Sim | null = null;

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      needsDraw = true;
    };
    resize();
    window.addEventListener("resize", resize);

    const radius = (d: number) => 4 + Math.min(10, d * 1.5);

    // Screen (canvas-local) point -> world coordinates, given the transform.
    const toWorld = (mx: number, my: number, rect: DOMRect) => {
      const { scale, ox, oy } = viewRef.current;
      const cx = rect.width / 2;
      const cy = rect.height / 2;
      return { x: (mx - cx - ox) / scale, y: (my - cy - oy) / scale };
    };
    const nodeAt = (mx: number, my: number, rect: DOMRect): Sim | null => {
      const w = toWorld(mx, my, rect);
      const tol = 6 / viewRef.current.scale;
      let best: Sim | null = null;
      let bestD = Infinity;
      for (const s of nodes) {
        const d = Math.hypot(s.x - w.x, s.y - w.y);
        if (d < radius(s.degree) + tol && d < bestD) {
          best = s;
          bestD = d;
        }
      }
      return best;
    };

    const physics = () => {
      // Repulsion between every pair (fine for personal-vault sizes).
      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          const a = nodes[i];
          const b = nodes[j];
          let dx = a.x - b.x;
          let dy = a.y - b.y;
          const d2 = dx * dx + dy * dy || 0.01;
          const f = (2200 * alpha) / d2;
          const d = Math.sqrt(d2);
          dx /= d;
          dy /= d;
          a.vx += dx * f;
          a.vy += dy * f;
          b.vx -= dx * f;
          b.vy -= dy * f;
        }
      }
      // Springs along edges.
      for (const { s, t } of edges) {
        const dx = t.x - s.x;
        const dy = t.y - s.y;
        const d = Math.sqrt(dx * dx + dy * dy) || 0.01;
        const f = (d - 90) * 0.02 * alpha;
        const ux = (dx / d) * f;
        const uy = (dy / d) * f;
        s.vx += ux;
        s.vy += uy;
        t.vx -= ux;
        t.vy -= uy;
      }
      // Gentle pull to center + integrate + damping. The dragged node is
      // pinned to the cursor, so we never integrate it.
      for (const s of nodes) {
        if (s === dragNode) {
          s.vx = 0;
          s.vy = 0;
          continue;
        }
        s.vx += -s.x * 0.002 * alpha;
        s.vy += -s.y * 0.002 * alpha;
        s.vx *= 0.85;
        s.vy *= 0.85;
        s.x += s.vx;
        s.y += s.vy;
      }
      alpha *= 0.99;
    };

    const draw = () => {
      const rect = canvas.getBoundingClientRect();
      const { scale, ox, oy } = viewRef.current;
      const cx = rect.width / 2;
      const cy = rect.height / 2;
      ctx.clearRect(0, 0, rect.width, rect.height);
      ctx.save();
      ctx.translate(cx + ox, cy + oy);
      ctx.scale(scale, scale);

      ctx.strokeStyle = "rgba(125,125,125,0.25)";
      ctx.lineWidth = 1 / scale;
      for (const { s, t } of edges) {
        ctx.beginPath();
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(t.x, t.y);
        ctx.stroke();
      }
      const showLabels = scale > 0.5;
      ctx.font = `${11 / scale}px Inter, system-ui, sans-serif`;
      for (const s of nodes) {
        const r = radius(s.degree);
        const isActive = s.path === activeRef.current;
        ctx.beginPath();
        ctx.arc(s.x, s.y, isActive ? r + 2 : r, 0, Math.PI * 2);
        ctx.fillStyle = s.ai ? AI : ACCENT;
        ctx.globalAlpha = isActive ? 1 : 0.85;
        ctx.fill();
        ctx.globalAlpha = 1;
        if (showLabels && (isActive || s.degree >= 2)) {
          ctx.fillStyle = "rgba(120,120,120,0.9)";
          ctx.fillText(s.title, s.x + r + 3 / scale, s.y + 3 / scale);
        }
      }
      ctx.restore();
    };

    const frame = () => {
      if (alpha > 0.02 || dragNode) {
        physics();
        needsDraw = true;
      }
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

    const localXY = (e: PointerEvent, rect: DOMRect) => ({
      mx: e.clientX - rect.left,
      my: e.clientY - rect.top,
    });

    const onPointerDown = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      const { mx, my } = localXY(e, rect);
      downX = lastX = mx;
      downY = lastY = my;
      moved = false;
      const hit = nodeAt(mx, my, rect);
      if (hit) {
        dragNode = hit;
        alpha = Math.max(alpha, 0.25); // reheat so neighbors follow
        canvas.style.cursor = "grabbing";
      } else {
        panning = true;
        canvas.style.cursor = "grabbing";
      }
      canvas.setPointerCapture(e.pointerId);
    };

    const onPointerMove = (e: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      const { mx, my } = localXY(e, rect);
      if (!dragNode && !panning) {
        // Hover feedback: pointer over a node hints it's grabbable.
        canvas.style.cursor = nodeAt(mx, my, rect) ? "grab" : "default";
        return;
      }
      if (Math.hypot(mx - downX, my - downY) > 3) moved = true;
      if (dragNode) {
        const w = toWorld(mx, my, rect);
        dragNode.x = w.x;
        dragNode.y = w.y;
        dragNode.vx = 0;
        dragNode.vy = 0;
        alpha = Math.max(alpha, 0.25);
      } else if (panning) {
        viewRef.current.ox += mx - lastX;
        viewRef.current.oy += my - lastY;
        needsDraw = true;
      }
      lastX = mx;
      lastY = my;
    };

    const onPointerUp = (e: PointerEvent) => {
      // A press without drag is a click → open the note.
      if (!moved && dragNode) onSelect(dragNode.path);
      dragNode = null;
      panning = false;
      canvas.style.cursor = "default";
      try {
        canvas.releasePointerCapture(e.pointerId);
      } catch {
        /* pointer may already be released */
      }
    };

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      const cx = rect.width / 2;
      const cy = rect.height / 2;
      const view = viewRef.current;
      // World point under the cursor stays fixed as we zoom.
      const wx = (mx - cx - view.ox) / view.scale;
      const wy = (my - cy - view.oy) / view.scale;
      const factor = Math.exp(-e.deltaY * 0.0015);
      view.scale = clamp(view.scale * factor, 0.15, 5);
      view.ox = mx - cx - wx * view.scale;
      view.oy = my - cy - wy * view.scale;
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

  const resetView = () => {
    viewRef.current = { scale: 1, ox: 0, oy: 0 };
  };

  return (
    <div className="relative h-full w-full">
      {graph.nodes.length === 0 && (
        <div className="absolute inset-0 grid place-items-center text-sm text-magma-muted">
          {t("graph.empty")}
        </div>
      )}
      <canvas ref={canvasRef} className="h-full w-full touch-none" />
      <button
        onClick={resetView}
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
