import { useCallback, useEffect, useRef, useState, type DragEvent } from "react";
import type { NoteMeta, SearchHit } from "../lib/api";
import { useI18n } from "../lib/i18n";
import FlameIcon from "./FlameIcon";
import {
  ChevronIcon,
  FolderIcon,
  NewFolderIcon,
  PencilIcon,
  CalendarIcon,
  PlusIcon,
  ReplaceIcon,
  SearchIcon,
  SettingsIcon,
  TrashIcon,
} from "./Icons";

const MIN_WIDTH = 200;
const MAX_WIDTH = 520;
const WIDTH_KEY = "magma.sidebarWidth";
/** One nesting level, in px. */
const INDENT = 14;
/**
 * Notes have no chevron, so without a matching gutter they would sit left of
 * the folder names around them and the tree would read as a flat stack. This
 * is the width of that gutter: a note at nesting level L lines up exactly with
 * a subfolder name at the same level.
 */
const NOTE_GUTTER = 16;

interface SidebarProps {
  vault: string | null;
  notes: NoteMeta[];
  folders: string[];
  activePath: string | null;
  onOpenVault: () => void;
  onSelect: (path: string) => void;
  onCreate: () => void;
  /** Create a folder; the argument is the parent ("" = vault root). */
  onCreateFolder: (parent: string) => void;
  /** Move a folder (with everything in it) into another ("" = root). */
  onMoveFolder: (folder: string, into: string) => void;
  /** Ask where to move a folder — the path that does not need dragging. */
  onMoveFolderPrompt: (folder: string) => void;
  onRename: (path: string, currentTitle: string) => void;
  onDelete: (path: string, title: string) => void;
  onMove: (path: string) => void;
  onMoveTo: (path: string, folder: string) => void;
  onDeleteFolder: (folder: string) => void;
  query: string;
  onQuery: (q: string) => void;
  searchHits: SearchHit[];
  onReplace: () => void;
  onOpenSettings: () => void;
  /** Open (or create) today's daily note. */
  onToday: () => void;
  /** The month calendar, passed in so the sidebar stays free of date logic. */
  calendar?: React.ReactNode;
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
  onMoveFolder,
  onMoveFolderPrompt,
  onRename,
  onDelete,
  onMove,
  onMoveTo,
  onDeleteFolder,
  query,
  onQuery,
  searchHits,
  onReplace,
  onOpenSettings,
  onToday,
  calendar,
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

  // Drag-and-drop. Notes and folders both drop onto folder headers and onto the
  // root, so the payload says which it is: "note:<path>" or "folder:<path>".
  const onDropInto = (folder: string) => (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDropTarget(null);
    const payload = e.dataTransfer.getData("text/plain");
    if (payload.startsWith("folder:")) {
      const from = payload.slice(7);
      // Into itself or into its own subtree is not a move, it's a paradox.
      if (from && folder !== from && !folder.startsWith(`${from}/`)) {
        onMoveFolder(from, folder);
      }
    } else if (payload.startsWith("note:")) {
      onMoveTo(payload.slice(5), folder);
    }
  };
  const dragProps = (n: NoteMeta) => ({
    draggable: true,
    onDragStart: (e: DragEvent) => e.dataTransfer.setData("text/plain", `note:${n.path}`),
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
          // Folders are draggable too — that is how one becomes a subfolder of
          // another without a dialog.
          draggable
          onDragStart={(e) => {
            e.stopPropagation();
            e.dataTransfer.setData("text/plain", `folder:${node.path}`);
          }}
          onDragOver={(e) => {
            e.preventDefault();
            e.stopPropagation();
            setDropTarget(node.path);
          }}
          onDragLeave={() => setDropTarget((d) => (d === node.path ? null : d))}
          onDrop={onDropInto(node.path)}
          style={{ paddingLeft: depth * INDENT }}
          className={`group flex w-full select-none items-center rounded-md pr-1 transition ${
            isDrop ? "bg-magma-accent/15" : "hover:bg-black/5 dark:hover:bg-white/10"
          }`}
        >
          <button
            onClick={() => toggleFolder(node.path)}
            // An open folder is the one you are working in — it carries the
            // accent, the same way the open note does.
            className={`flex min-w-0 flex-1 items-center gap-1 px-1.5 py-1 text-left text-[13px] transition-colors ${
              isOpen ? "text-magma-accent" : "text-magma-muted"
            }`}
          >
            <ChevronIcon size={12} open={isOpen} className="shrink-0 opacity-70" />
            <span className="truncate">{node.name}</span>
            <span className="ml-auto pl-1 text-xs tabular-nums opacity-50">
              {node.total || ""}
            </span>
          </button>
          <button
            onClick={() => onCreateFolder(node.path)}
            title={t("sidebar.newSubfolder")}
            aria-label={`${t("sidebar.newSubfolder")}: ${node.name}`}
            className="grid h-6 w-6 shrink-0 place-items-center rounded-md text-magma-muted opacity-0 transition hover:bg-black/10 hover:text-magma-ink group-hover:opacity-100 dark:hover:bg-white/20"
          >
            <NewFolderIcon size={14} />
          </button>
          <button
            onClick={() => onMoveFolderPrompt(node.path)}
            title={t("sidebar.moveFolder")}
            aria-label={`${t("sidebar.moveFolder")}: ${node.name}`}
            className="grid h-6 w-6 shrink-0 place-items-center rounded-md text-magma-muted opacity-0 transition hover:bg-black/10 hover:text-magma-ink group-hover:opacity-100 dark:hover:bg-white/20"
          >
            <FolderIcon size={14} />
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
            {/* Notes line up under their folder's *name*, past the chevron, so
                the tree reads as a tree instead of a stack of rows. */}
            <div style={{ paddingLeft: (depth + 1) * INDENT + NOTE_GUTTER }}>
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
        className={`flex min-w-0 flex-1 items-center gap-2 px-1.5 py-1.5 text-left text-sm ${
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
                onClick={() => onCreateFolder("")}
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
            onClick={onToday}
            title={t("sidebar.today")}
            className="grid h-7 w-7 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 hover:text-magma-text dark:hover:bg-white/10"
          >
            <CalendarIcon size={16} />
          </button>
          <button
            onClick={onOpenSettings}
            title={t("sidebar.settings")}
            className="grid h-7 w-7 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 hover:text-magma-text dark:hover:bg-white/10"
          >
            <SettingsIcon size={16} />
          </button>
        </div>
      </div>

      {!vault && (
        <button
          onClick={onOpenVault}
          className="mx-3 mb-2 rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90"
        >
          {t("sidebar.openVault")}
        </button>
      )}

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
            className="w-full rounded-lg border border-black/10 bg-transparent py-1.5 pl-8 pr-9 text-sm outline-none placeholder:text-magma-muted focus:border-magma-accent dark:border-white/10"
          />
          {/* Sits in the search field: replacing is what you reach for once
              searching has shown you how often the term actually occurs. */}
          <button
            onClick={onReplace}
            title={t("sidebar.replace")}
            aria-label={t("sidebar.replace")}
            className="absolute right-1.5 top-1/2 grid h-6 w-6 -translate-y-1/2 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 hover:text-magma-ink dark:hover:bg-white/10"
          >
            <ReplaceIcon size={14} />
          </button>
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
              <span className="block truncate text-sm font-medium">
                <Highlight text={h.title} term={query} />
              </span>
              <span className="block truncate text-xs text-magma-muted">
                <Highlight text={h.snippet} term={query} />
              </span>
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
            style={{ paddingLeft: NOTE_GUTTER }}
            className={`rounded-md ${dropTarget === "" ? "bg-magma-accent/10" : ""}`}
          >
            {rootNotes.map(renderNote)}
          </div>

          {/* Nested folders; every header is a drop target. */}
          {tree.map((node) => renderFolder(node, 0))}
        </nav>
      )}

      {/* Pinned to the bottom: the calendar is a way in, not part of the list. */}
      {vault && calendar}
    </aside>
  );
}

/**
 * Show where the match actually is. A search result that only says "some note
 * mentions this" is half an answer — the hit itself has to be visible, in the
 * title and in the surrounding text.
 */
function Highlight({ text, term }: { text: string; term: string }) {
  const needle = term.trim();
  if (!needle) return <>{text}</>;
  const regex = searchRegex(needle);
  const parts: (string | { hit: string })[] = [];
  if (regex) {
    let at = 0;
    for (;;) {
      const match = regex.exec(text);
      if (!match) {
        parts.push(text.slice(at));
        break;
      }
      const hit = match[0];
      const i = match.index;
      if (i > at) parts.push(text.slice(at, i));
      parts.push({ hit });
      at = i + hit.length;
      if (hit.length === 0) regex.lastIndex += 1;
    }
    return <HighlightedParts parts={parts} />;
  }
  const hay = text.toLowerCase();
  const nee = needle.toLowerCase();
  let at = 0;
  for (;;) {
    const i = hay.indexOf(nee, at);
    if (i === -1) {
      parts.push(text.slice(at));
      break;
    }
    if (i > at) parts.push(text.slice(at, i));
    parts.push({ hit: text.slice(i, i + needle.length) });
    at = i + needle.length;
  }
  return <HighlightedParts parts={parts} />;
}

function HighlightedParts({ parts }: { parts: (string | { hit: string })[] }) {
  return (
    <>
      {parts.map((p, i) =>
        typeof p === "string" ? (
          <span key={i}>{p}</span>
        ) : (
          <mark
            key={i}
            className="magma-text-highlight rounded-[3px] px-0.5"
          >
            {p.hit}
          </mark>
        )
      )}
    </>
  );
}

function searchRegex(term: string): RegExp | null {
  const parsed = parseRegexQuery(term);
  if (!parsed) return null;
  try {
    const flags = parsed.flags.includes("g") ? parsed.flags : `${parsed.flags}g`;
    return new RegExp(parsed.pattern, flags);
  } catch {
    return null;
  }
}

function parseRegexQuery(term: string): { pattern: string; flags: string } | null {
  if (term.startsWith("re:")) return { pattern: term.slice(3), flags: "" };
  if (!term.startsWith("/")) return null;
  let escaped = false;
  for (let i = 1; i < term.length; i += 1) {
    const ch = term[i];
    if (escaped) {
      escaped = false;
    } else if (ch === "\\") {
      escaped = true;
    } else if (ch === "/") {
      const pattern = term.slice(1, i);
      const flags = term.slice(i + 1);
      if (!pattern || !/^[ims]*$/.test(flags)) return null;
      return { pattern, flags };
    }
  }
  return null;
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
