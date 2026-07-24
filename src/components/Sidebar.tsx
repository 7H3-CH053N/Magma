import { useState } from "react";
import type { NoteMeta, SearchHit } from "../lib/api";
import { useI18n } from "../lib/i18n";
import FlameIcon from "./FlameIcon";

interface SidebarProps {
  vault: string | null;
  notes: NoteMeta[];
  folders: string[];
  activePath: string | null;
  onOpenVault: () => void;
  onSelect: (path: string) => void;
  onCreate: () => void;
  onCreateFolder: () => void;
  onRename: (path: string, currentTitle: string) => void;
  onDelete: (path: string, title: string) => void;
  onMove: (path: string) => void;
  query: string;
  onQuery: (q: string) => void;
  searchHits: SearchHit[];
  onOpenSettings: () => void;
}

export default function Sidebar({
  vault,
  notes,
  folders,
  activePath,
  onOpenVault,
  onSelect,
  onCreate,
  onCreateFolder,
  onRename,
  onDelete,
  onMove,
  query,
  onQuery,
  searchHits,
  onOpenSettings,
}: SidebarProps) {
  const { t } = useI18n();
  const [hovered, setHovered] = useState<string | null>(null);
  const searching = query.trim().length > 0;

  // Group notes: root-level ones, then a section per folder — including empty
  // folders you created, so you can move notes into them.
  const rootNotes = notes.filter((n) => folderOf(n.path) === "");
  const notesByFolder = notes.reduce((map, n) => {
    const f = folderOf(n.path);
    if (f) map.set(f, [...(map.get(f) ?? []), n]);
    return map;
  }, new Map<string, NoteMeta[]>());
  const folderSections = Array.from(
    new Set([...notesByFolder.keys(), ...folders])
  )
    .sort((a, b) => a.localeCompare(b))
    .map((f) => [f, notesByFolder.get(f) ?? []] as const);

  const renderNote = (n: NoteMeta) => (
    <div
      key={n.path}
      onMouseEnter={() => setHovered(n.path)}
      onMouseLeave={() => setHovered((h) => (h === n.path ? null : h))}
      className={`group flex items-center gap-1 rounded-md pr-1 transition ${
        activePath === n.path
          ? "bg-magma-accent/10"
          : "hover:bg-black/5 dark:hover:bg-white/10"
      }`}
    >
      <button
        onClick={() => onSelect(n.path)}
        className={`flex min-w-0 flex-1 items-center gap-2 px-2 py-1.5 text-left text-sm ${
          activePath === n.path ? "text-magma-accent" : ""
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
      {hovered === n.path && (
        <div className="flex shrink-0 items-center">
          <button
            onClick={() => onMove(n.path)}
            title={t("sidebar.move")}
            className="grid h-6 w-6 place-items-center rounded text-magma-muted hover:bg-black/10 dark:hover:bg-white/20"
          >
            📁
          </button>
          <button
            onClick={() => onRename(n.path, n.title)}
            title={t("sidebar.rename")}
            className="grid h-6 w-6 place-items-center rounded text-magma-muted hover:bg-black/10 dark:hover:bg-white/20"
          >
            ✎
          </button>
          <button
            onClick={() => onDelete(n.path, n.title)}
            title={t("sidebar.delete")}
            className="grid h-6 w-6 place-items-center rounded text-magma-muted hover:bg-black/10 dark:hover:bg-white/20"
          >
            🗑
          </button>
        </div>
      )}
    </div>
  );

  return (
    <aside className="flex h-full w-64 shrink-0 flex-col border-r border-black/5 bg-magma-panel/60 dark:border-white/5 dark:bg-white/5">
      <div className="flex items-center gap-2 px-4 py-3">
        <FlameIcon size={18} />
        <span className="font-semibold tracking-tight">Magma</span>
        <div className="ml-auto flex items-center">
          {vault && (
            <>
              <button
                onClick={onCreateFolder}
                title={t("sidebar.newFolder")}
                className="grid h-6 w-6 place-items-center rounded-md text-sm leading-none text-magma-muted transition hover:bg-black/10 dark:hover:bg-white/10"
              >
                🗀
              </button>
              <button
                onClick={onCreate}
                title={t("sidebar.newNote")}
                className="grid h-6 w-6 place-items-center rounded-md text-lg leading-none text-magma-muted transition hover:bg-black/10 dark:hover:bg-white/10"
              >
                +
              </button>
            </>
          )}
          <button
            onClick={onOpenSettings}
            title={t("sidebar.settings")}
            className="grid h-6 w-6 place-items-center rounded-md text-sm leading-none text-magma-muted transition hover:bg-black/10 dark:hover:bg-white/10"
          >
            ⚙
          </button>
        </div>
      </div>

      <button
        onClick={onOpenVault}
        className="mx-3 mb-2 rounded-lg bg-black/5 px-3 py-1.5 text-left text-sm text-magma-muted transition hover:bg-black/10 dark:bg-white/10 dark:hover:bg-white/20"
      >
        {vault ? shortenPath(vault) : t("sidebar.openVault")}
      </button>

      {vault && (
        <input
          value={query}
          onChange={(e) => onQuery(e.target.value)}
          placeholder={t("sidebar.search")}
          className="mx-3 mb-2 rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none placeholder:text-magma-muted focus:border-magma-accent dark:border-white/10"
        />
      )}

      {searching ? (
        <nav className="flex-1 overflow-auto px-2 pb-4">
          {searchHits.length === 0 && (
            <p className="px-2 py-4 text-sm text-magma-muted">{t("sidebar.noMatches")}</p>
          )}
          {searchHits.map((h) => (
            <button
              key={h.path}
              onClick={() => onSelect(h.path)}
              className={`block w-full rounded-md px-2 py-1.5 text-left transition ${
                activePath === h.path
                  ? "bg-magma-accent/10"
                  : "hover:bg-black/5 dark:hover:bg-white/10"
              }`}
            >
              <span className="block truncate text-sm font-medium">{h.title}</span>
              <span className="block truncate text-xs text-magma-muted">{h.snippet}</span>
            </button>
          ))}
        </nav>
      ) : (
        <nav className="flex-1 overflow-auto px-2 pb-4">
          {notes.length === 0 && (
            <p className="px-2 py-4 text-sm text-magma-muted">
              {vault ? t("sidebar.noNotes") : t("sidebar.openToBegin")}
            </p>
          )}
          {/* Root-level notes first, then one section per folder. */}
          {rootNotes.map(renderNote)}
          {folderSections.map(([folder, items]) => (
            <div key={folder} className="mt-2">
              <div className="flex items-center gap-1 px-2 py-1 text-xs font-medium uppercase tracking-wide text-magma-muted">
                <span>🗀</span>
                <span className="truncate">{folder}</span>
              </div>
              <div className="pl-2">
                {items.length === 0 ? (
                  <p className="px-2 py-1 text-xs text-magma-muted/70">—</p>
                ) : (
                  items.map(renderNote)
                )}
              </div>
            </div>
          ))}
        </nav>
      )}
    </aside>
  );
}

function folderOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? "" : path.slice(0, i);
}

function shortenPath(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts.length <= 2 ? p : "…/" + parts.slice(-2).join("/");
}
