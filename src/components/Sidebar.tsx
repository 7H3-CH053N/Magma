import { useCallback, useEffect, useRef, useState, type DragEvent } from "react";
import type { NoteMeta, SearchHit } from "../lib/api";
import { useI18n } from "../lib/i18n";
import FlameIcon from "./FlameIcon";
import {
  ChevronIcon,
  FolderIcon,
  NewFolderIcon,
  PencilIcon,
  PlusIcon,
  SearchIcon,
  SettingsIcon,
  TrashIcon,
} from "./Icons";

const MIN_WIDTH = 200;
const MAX_WIDTH = 520;
const WIDTH_KEY = "magma.sidebarWidth";

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
  onMoveTo: (path: string, folder: string) => void;
  onDeleteFolder: (folder: string) => void;
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
  onMoveTo,
  onDeleteFolder,
  query,
  onQuery,
  searchHits,
  onOpenSettings,
}: SidebarProps) {
  const { t } = useI18n();
  const [hovered, setHovered] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const searching = query.trim().length > 0;

  // Drag the right edge to resize; the width is remembered across restarts.
  const [width, setWidth] = useState(() => {
    const saved = Number(localStorage.getItem(WIDTH_KEY));
    return saved >= MIN_WIDTH && saved <= MAX_WIDTH ? saved : 260;
  });
  const resizing = useRef(false);
  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      if (!resizing.current) return;
      setWidth(Math.min(MAX_WIDTH, Math.max(MIN_WIDTH, e.clientX)));
    };
    const onUp = () => {
      if (!resizing.current) return;
      resizing.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, []);
  useEffect(() => {
    localStorage.setItem(WIDTH_KEY, String(width));
  }, [width]);
  const startResize = useCallback(() => {
    resizing.current = true;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }, []);

  const toggleFolder = (f: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev);
      next.has(f) ? next.delete(f) : next.add(f);
      return next;
    });

  // Drag-and-drop: a note carries its path; folder headers and the root are drops.
  const onDropInto = (folder: string) => (e: DragEvent) => {
    e.preventDefault();
    setDropTarget(null);
    const path = e.dataTransfer.getData("text/plain");
    if (path) onMoveTo(path, folder);
  };
  const dragProps = (n: NoteMeta) => ({
    draggable: true,
    onDragStart: (e: DragEvent) => e.dataTransfer.setData("text/plain", n.path),
  });

  // Real hierarchy: "Blog/KI-Wissen" becomes KI-Wissen *inside* Blog, not a
  // top-level row labelled with its whole path. Empty folders you created are
  // included so you can drop notes into them.
  const rootNotes = notes.filter((n) => folderOf(n.path) === "");
  const tree = buildTree(notes, folders);

  // One folder and everything beneath it. Depth drives the indent, so nesting
  // is visible instead of being spelled out as "Blog/KI-Wissen".
  const renderFolder = (node: FolderNode, depth: number) => {
    const isOpen = !collapsed.has(node.path);
    const isDrop = dropTarget === node.path;
    return (
      <div key={node.path}>
        <div
          onDragOver={(e) => {
            e.preventDefault();
            setDropTarget(node.path);
          }}
          onDragLeave={() => setDropTarget((d) => (d === node.path ? null : d))}
          onDrop={onDropInto(node.path)}
          style={{ paddingLeft: depth * 12 }}
          className={`group flex w-full select-none items-center rounded-md pr-1 transition ${
            isDrop ? "bg-magma-accent/15" : "hover:bg-black/5 dark:hover:bg-white/10"
          }`}
        >
          <button
            onClick={() => toggleFolder(node.path)}
            className="flex min-w-0 flex-1 items-center gap-1 px-1.5 py-1 text-left text-[13px] text-magma-muted"
          >
            <ChevronIcon size={12} open={isOpen} className="shrink-0 opacity-70" />
            <span className="truncate">{node.name}</span>
            <span className="ml-auto pl-1 text-xs tabular-nums opacity-50">
              {node.total || ""}
            </span>
          </button>
          <button
            onClick={() => onDeleteFolder(node.path)}
            title={t("sidebar.deleteFolder")}
            className="grid h-6 w-6 shrink-0 place-items-center rounded-md text-magma-muted opacity-0 transition hover:bg-red-500/15 hover:text-red-500 group-hover:opacity-100 dark:hover:bg-red-500/20"
          >
            <TrashIcon size={14} />
          </button>
        </div>
        {isOpen && (
          <div>
            {node.children.map((c) => renderFolder(c, depth + 1))}
            <div style={{ paddingLeft: (depth + 1) * 12 }}>
              {node.notes.map(renderNote)}
            </div>
          </div>
        )}
      </div>
    );
  };

  const renderNote = (n: NoteMeta) => (
    <div
      key={n.path}
      {...dragProps(n)}
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
            className="grid h-6 w-6 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 hover:text-magma-text dark:hover:bg-white/20"
          >
            <FolderIcon size={14} />
          </button>
          <button
            onClick={() => onRename(n.path, n.title)}
            title={t("sidebar.rename")}
            className="grid h-6 w-6 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 hover:text-magma-text dark:hover:bg-white/20"
          >
            <PencilIcon size={14} />
          </button>
          <button
            onClick={() => onDelete(n.path, n.title)}
            title={t("sidebar.delete")}
            className="grid h-6 w-6 place-items-center rounded-md text-magma-muted transition hover:bg-red-500/15 hover:text-red-500 dark:hover:bg-red-500/20"
          >
            <TrashIcon size={14} />
          </button>
        </div>
      )}
    </div>
  );

  return (
    <aside
      style={{ width }}
      className="relative flex h-full shrink-0 flex-col border-r border-black/5 bg-magma-panel/60 dark:border-white/5 dark:bg-white/5"
    >
      {/* Drag handle for resizing — invisible until you reach for it. */}
      <div
        onPointerDown={startResize}
        onDoubleClick={() => setWidth(260)}
        title={t("sidebar.resize")}
        className="absolute right-0 top-0 z-10 h-full w-1.5 cursor-col-resize transition-colors hover:bg-magma-accent/40"
      />
      <div className="flex items-center gap-2 px-4 py-3">
        <FlameIcon size={18} />
        <span className="font-semibold tracking-tight">Magma</span>
        <div className="ml-auto flex items-center gap-0.5">
          {vault && (
            <>
              <button
                onClick={onCreateFolder}
                title={t("sidebar.newFolder")}
                className="grid h-7 w-7 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 hover:text-magma-text dark:hover:bg-white/10"
              >
                <NewFolderIcon size={16} />
              </button>
              <button
                onClick={onCreate}
                title={t("sidebar.newNote")}
                className="grid h-7 w-7 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 hover:text-magma-text dark:hover:bg-white/10"
              >
                <PlusIcon size={16} />
              </button>
            </>
          )}
          <button
            onClick={onOpenSettings}
            title={t("sidebar.settings")}
            className="grid h-7 w-7 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 hover:text-magma-text dark:hover:bg-white/10"
          >
            <SettingsIcon size={16} />
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
        <div className="relative mx-3 mb-2">
          <SearchIcon
            size={14}
            className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-magma-muted"
          />
          <input
            value={query}
            onChange={(e) => onQuery(e.target.value)}
            placeholder={t("sidebar.search")}
            className="w-full rounded-lg border border-black/10 bg-transparent py-1.5 pl-8 pr-3 text-sm outline-none placeholder:text-magma-muted focus:border-magma-accent dark:border-white/10"
          />
        </div>
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
          {/* Root notes (also the drop target for moving a note back to root). */}
          <div
            onDragOver={(e) => {
              e.preventDefault();
              setDropTarget("");
            }}
            onDrop={onDropInto("")}
            className={`rounded-md ${dropTarget === "" ? "bg-magma-accent/10" : ""}`}
          >
            {rootNotes.map(renderNote)}
          </div>

          {/* Nested folders; every header is a drop target. */}
          {tree.map((node) => renderFolder(node, 0))}
        </nav>
      )}
    </aside>
  );
}

function folderOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i === -1 ? "" : path.slice(0, i);
}

interface FolderNode {
  /** Last path segment — what's shown. */
  name: string;
  /** Full vault-relative path — used for collapse state and drops. */
  path: string;
  notes: NoteMeta[];
  children: FolderNode[];
  /** Notes in this folder and everything under it. */
  total: number;
}

/** Turn flat "a/b/c" paths into a nested tree, sorted folders-then-notes. */
function buildTree(notes: NoteMeta[], folders: string[]): FolderNode[] {
  const roots: FolderNode[] = [];
  const byPath = new Map<string, FolderNode>();

  const ensure = (dir: string): FolderNode => {
    const found = byPath.get(dir);
    if (found) return found;
    const cut = dir.lastIndexOf("/");
    const node: FolderNode = {
      name: dir.slice(cut + 1),
      path: dir,
      notes: [],
      children: [],
      total: 0,
    };
    byPath.set(dir, node);
    if (cut === -1) roots.push(node);
    else ensure(dir.slice(0, cut)).children.push(node);
    return node;
  };

  // Folders with no notes yet still need a row, so they can be dropped into.
  for (const f of folders) if (f) ensure(f);
  for (const n of notes) {
    const dir = folderOf(n.path);
    if (dir) ensure(dir).notes.push(n);
  }

  const finish = (node: FolderNode): number => {
    node.children.sort((a, b) => a.name.localeCompare(b.name));
    node.notes.sort((a, b) => a.title.localeCompare(b.title));
    node.total = node.notes.length + node.children.reduce((s, c) => s + finish(c), 0);
    return node.total;
  };
  roots.forEach(finish);
  roots.sort((a, b) => a.name.localeCompare(b.name));
  return roots;
}

function shortenPath(p: string): string {
  const parts = p.split(/[\\/]/);
  return parts.length <= 2 ? p : "…/" + parts.slice(-2).join("/");
}
