import type { NoteMeta } from "../lib/api";

interface BacklinksPanelProps {
  backlinks: NoteMeta[];
  onSelect: (path: string) => void;
}

/** Quiet panel under the editor: which notes point here. */
export default function BacklinksPanel({ backlinks, onSelect }: BacklinksPanelProps) {
  if (backlinks.length === 0) return null;
  return (
    <div className="border-t border-black/5 px-8 py-3 dark:border-white/10">
      <p className="mb-1 text-xs font-medium uppercase tracking-wide text-magma-muted">
        {backlinks.length} linked {backlinks.length === 1 ? "mention" : "mentions"}
      </p>
      <div className="flex flex-wrap gap-2">
        {backlinks.map((b) => (
          <button
            key={b.path}
            onClick={() => onSelect(b.path)}
            className="rounded-md bg-black/5 px-2 py-1 text-sm text-magma-ink transition hover:bg-black/10 dark:bg-white/10 dark:text-[#ece9e4] dark:hover:bg-white/20"
          >
            {b.title}
          </button>
        ))}
      </div>
    </div>
  );
}
