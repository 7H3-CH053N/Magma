import type { NoteMeta } from "../lib/api";

interface SidebarProps {
  vault: string | null;
  notes: NoteMeta[];
  activePath: string | null;
  onOpenVault: () => void;
  onSelect: (path: string) => void;
}

export default function Sidebar({
  vault,
  notes,
  activePath,
  onOpenVault,
  onSelect,
}: SidebarProps) {
  return (
    <aside className="flex h-full w-64 shrink-0 flex-col border-r border-black/5 bg-magma-panel/60 dark:border-white/5 dark:bg-white/5">
      <div className="flex items-center gap-2 px-4 py-3">
        <div className="h-3 w-3 rounded-full bg-magma-accent" />
        <span className="font-semibold tracking-tight">Magma</span>
      </div>

      <button
        onClick={onOpenVault}
        className="mx-3 mb-2 rounded-lg bg-black/5 px-3 py-1.5 text-left text-sm text-magma-muted transition hover:bg-black/10 dark:bg-white/10 dark:hover:bg-white/20"
      >
        {vault ? shortenPath(vault) : "Open vault…"}
      </button>

      <nav className="flex-1 overflow-auto px-2 pb-4">
        {notes.length === 0 && (
          <p className="px-2 py-4 text-sm text-magma-muted">
            {vault ? "No notes yet." : "Open a folder of markdown files to begin."}
          </p>
        )}
        {notes.map((n) => (
          <button
            key={n.path}
            onClick={() => onSelect(n.path)}
            className={`flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition ${
              activePath === n.path
                ? "bg-magma-accent/10 text-magma-accent"
                : "hover:bg-black/5 dark:hover:bg-white/10"
            }`}
          >
            <span className="truncate">{n.title}</span>
            {n.aiAuthored && (
              <span
                title="Written by an AI assistant"
                className="ml-auto h-1.5 w-1.5 shrink-0 rounded-full bg-magma-ai"
              />
            )}
          </button>
        ))}
      </nav>
    </aside>
  );
}

function shortenPath(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts.length <= 2 ? p : "…/" + parts.slice(-2).join("/");
}
