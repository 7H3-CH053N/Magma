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

const ACCENT = "#e0533d";
const AI = "#7c5cff";

/**
 * The graph — Magma's headline view. A small custom force simulation on a
 * canvas keeps large vaults smooth without pulling in a graph library. Nodes
 * grow with their link degree; AI-authored notes glow violet so you can see
 * what Claude contributed.
 */
export default function GraphView({ graph, activePath, onSelect }: GraphViewProps) {
  const { t } = useI18n();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const nodesRef = useRef<Sim[]>([]);
  const activeRef = useRef(activePath);
  activeRef.current = activePath;

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

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      canvas.width = rect.width * dpr;
      canvas.height = rect.height * dpr;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();
    window.addEventListener("resize", resize);

    const radius = (d: number) => 4 + Math.min(10, d * 1.5);

    const step = () => {
      const rect = canvas.getBoundingClientRect();
      const cx = rect.width / 2;
      const cy = rect.height / 2;

      // Repulsion between every pair (fine for personal-vault sizes).
      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          const a = nodes[i];
          const b = nodes[j];
          let dx = a.x - b.x;
          let dy = a.y - b.y;
          let d2 = dx * dx + dy * dy || 0.01;
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
      // Gentle pull to center + integrate + damping.
      for (const s of nodes) {
        s.vx += -s.x * 0.002 * alpha;
        s.vy += -s.y * 0.002 * alpha;
        s.vx *= 0.85;
        s.vy *= 0.85;
        s.x += s.vx;
        s.y += s.vy;
      }
      alpha *= 0.99;

      // Draw.
      ctx.clearRect(0, 0, rect.width, rect.height);
      ctx.save();
      ctx.translate(cx, cy);
      ctx.strokeStyle = "rgba(125,125,125,0.25)";
      ctx.lineWidth = 1;
      for (const { s, t } of edges) {
        ctx.beginPath();
        ctx.moveTo(s.x, s.y);
        ctx.lineTo(t.x, t.y);
        ctx.stroke();
      }
      for (const s of nodes) {
        const r = radius(s.degree);
        const isActive = s.path === activeRef.current;
        ctx.beginPath();
        ctx.arc(s.x, s.y, isActive ? r + 2 : r, 0, Math.PI * 2);
        ctx.fillStyle = s.ai ? AI : ACCENT;
        ctx.globalAlpha = isActive ? 1 : 0.85;
        ctx.fill();
        ctx.globalAlpha = 1;
        if (isActive || s.degree >= 2) {
          ctx.fillStyle = "rgba(120,120,120,0.9)";
          ctx.font = "11px Inter, system-ui, sans-serif";
          ctx.fillText(s.title, s.x + r + 3, s.y + 3);
        }
      }
      ctx.restore();

      if (alpha > 0.02) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);

    const onClick = (e: MouseEvent) => {
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left - rect.width / 2;
      const my = e.clientY - rect.top - rect.height / 2;
      let best: Sim | null = null;
      let bestD = Infinity;
      for (const s of nodes) {
        const d = Math.hypot(s.x - mx, s.y - my);
        if (d < radius(s.degree) + 6 && d < bestD) {
          best = s;
          bestD = d;
        }
      }
      if (best) onSelect(best.path);
    };
    canvas.addEventListener("click", onClick);

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
      canvas.removeEventListener("click", onClick);
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
      <canvas ref={canvasRef} className="h-full w-full cursor-pointer" />
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
