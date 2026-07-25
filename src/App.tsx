import { useCallback, useEffect, useRef, useState } from "react";
import Sidebar from "./components/Sidebar";
import Editor from "./components/Editor";
import GraphView from "./components/GraphView";
import BacklinksPanel from "./components/BacklinksPanel";
import Splash from "./components/Splash";
import Settings from "./components/Settings";
import FlameIcon from "./components/FlameIcon";
import PromptDialog from "./components/PromptDialog";
import ConfirmDialog from "./components/ConfirmDialog";
import NodePreview from "./components/NodePreview";
import { useI18n } from "./lib/i18n";
import { splitFrontmatter, joinFrontmatter } from "./lib/markdown";
import {
  backlinks as fetchBacklinks,
  buildGraph,
  createFolder,
  createNote,
  deleteFolder,
  deleteNote,
  hasTauri,
  listFolders,
  listNotes,
  moveNote,
  openExternal,
  pickVault,
  readNote,
  remoteConnect,
  remoteDelete,
  remotePut,
  renameNote,
  search as searchNotes,
  writeNote,
  type Graph,
  type NoteMeta,
  type RemoteConfig,
  type SearchHit,
} from "./lib/api";

const AUTOSAVE_MS = 600;
type View = "editor" | "graph";

export default function App() {
  const { t } = useI18n();
  const [vault, setVault] = useState<string | null>(null);
  const [notes, setNotes] = useState<NoteMeta[]>([]);
  const [folders, setFolders] = useState<string[]>([]);
  const [activePath, setActivePath] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const [view, setView] = useState<View>("editor");
  const [graph, setGraph] = useState<Graph>({ nodes: [], edges: [] });
  const [links, setLinks] = useState<NoteMeta[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  // Graph node being previewed (path + title); null closes the panel.
  const [preview, setPreview] = useState<{ path: string; title: string } | null>(null);
  const [remote, setRemote] = useState<RemoteConfig | null>(null);
  // In-app text prompt (window.prompt doesn't work in the Tauri webview).
  const [dialog, setDialog] = useState<{
    title: string;
    initial: string;
    suggestions?: string[];
    onSubmit: (value: string) => void;
  } | null>(null);
  // Destructive actions route through an in-app confirmation (window.confirm
  // is not surfaced by the desktop webview).
  const [confirm, setConfirm] = useState<{
    title: string;
    detail?: string;
    onConfirm: () => void;
  } | null>(null);
  // The active note's frontmatter, kept out of the editor and re-attached on save.
  const frontmatter = useRef("");
  const saveTimer = useRef<number | null>(null);

  // Write-through to a remote vault, best-effort — a failed push is logged but
  // never blocks the local edit (which already succeeded).
  const pushRemote = useCallback(
    (path: string, content: string) => {
      if (!remote) return;
      void remotePut(remote, path, content).catch((e) =>
        console.error("remote push failed:", e)
      );
    },
    [remote]
  );
  const deleteRemote = useCallback(
    (path: string) => {
      if (!remote) return;
      void remoteDelete(remote, path).catch((e) =>
        console.error("remote delete failed:", e)
      );
    },
    [remote]
  );

  const refreshNotes = useCallback(async (v: string) => {
    setNotes(await listNotes(v));
    // Folders are optional — never let their loading break the note list.
    try {
      setFolders(await listFolders(v));
    } catch {
      setFolders([]);
    }
  }, []);

  const openVault = useCallback(async () => {
    const picked = await pickVault();
    if (picked) {
      setVault(picked);
      await refreshNotes(picked);
    }
  }, [refreshNotes]);

  const selectNote = useCallback(
    async (path: string) => {
      if (!vault) return;
      const note = await readNote(vault, path);
      const { frontmatter: fm, body } = splitFrontmatter(note.content);
      frontmatter.current = fm;
      setActivePath(path);
      setContent(body);
      setView("editor");
      setLinks(await fetchBacklinks(vault, path));
    },
    [vault]
  );

  const flushSave = useCallback(() => {
    if (saveTimer.current) {
      window.clearTimeout(saveTimer.current);
      saveTimer.current = null;
    }
  }, []);

  // Open a note by its `[[wikilink]]` name (filename stem).
  const openByName = useCallback(
    async (name: string) => {
      if (!vault) return;
      const stem = (p: string) => {
        const f = p.split("/").pop() ?? p;
        return f.replace(/\.md$/i, "");
      };
      const found = notes.find((n) => stem(n.path).toLowerCase() === name.toLowerCase());
      if (found) {
        await selectNote(found.path);
        return;
      }
      // Wikipedia-style: a link to a note that doesn't exist yet creates it,
      // named after the link, and opens it.
      flushSave();
      const path = await createNote(vault, name);
      await refreshNotes(vault);
      await selectNote(path);
    },
    [vault, notes, selectNote, flushSave, refreshNotes]
  );

  const handleChange = useCallback(
    (next: string) => {
      setContent(next);
      if (!vault || !activePath) return;
      // Re-attach the note's frontmatter (kept out of the editor) before saving.
      const full = joinFrontmatter(frontmatter.current, next);
      if (saveTimer.current) window.clearTimeout(saveTimer.current);
      saveTimer.current = window.setTimeout(() => {
        void writeNote(vault, activePath, full).then(() => {
          pushRemote(activePath, full);
          // Refresh the list so the sidebar title (first heading) updates live.
          void refreshNotes(vault);
        });
      }, AUTOSAVE_MS);
    },
    [vault, activePath, pushRemote, refreshNotes]
  );

  const createNewNote = useCallback(async () => {
    if (!vault) return;
    flushSave();
    const path = await createNote(vault, "Untitled");
    await refreshNotes(vault);
    await selectNote(path);
    if (remote) {
      const note = await readNote(vault, path);
      pushRemote(path, note.content);
    }
  }, [vault, flushSave, refreshNotes, selectNote, remote, pushRemote]);

  const handleRename = useCallback(
    (path: string, currentTitle: string) => {
      if (!vault) return;
      setDialog({
        title: t("prompt.rename"),
        initial: currentTitle,
        onSubmit: async (next) => {
          if (!next.trim() || next === currentTitle) return;
          flushSave();
          const newPath = await renameNote(vault, path, next.trim());
          await refreshNotes(vault);
          if (activePath === path) await selectNote(newPath);
          if (remote) {
            const note = await readNote(vault, newPath);
            pushRemote(newPath, note.content);
            deleteRemote(path);
          }
        },
      });
    },
    [vault, activePath, flushSave, refreshNotes, selectNote, remote, pushRemote, deleteRemote, t]
  );

  const handleDelete = useCallback(
    (path: string, title: string) => {
      if (!vault) return;
      setConfirm({
        title: t("confirm.delete", { title }),
        detail: t("confirm.undone"),
        onConfirm: async () => {
          flushSave();
          await deleteNote(vault, path);
          deleteRemote(path);
          await refreshNotes(vault);
          if (activePath === path) {
            setActivePath(null);
            setContent("");
            setLinks([]);
          }
        },
      });
    },
    [vault, activePath, flushSave, refreshNotes, deleteRemote, t]
  );

  const handleDeleteFolder = useCallback(
    (folder: string) => {
      if (!vault || !folder) return;
      const inFolder = notes.filter((n) => n.path.startsWith(`${folder}/`));
      setConfirm({
        title: t("confirm.deleteFolder", { folder, count: String(inFolder.length) }),
        detail: t("confirm.undone"),
        onConfirm: async () => {
          flushSave();
          await deleteFolder(vault, folder);
          for (const n of inFolder) deleteRemote(n.path);
          await refreshNotes(vault);
          if (activePath && activePath.startsWith(`${folder}/`)) {
            setActivePath(null);
            setContent("");
            setLinks([]);
          }
        },
      });
    },
    [vault, notes, activePath, flushSave, refreshNotes, deleteRemote, t]
  );

  const handleCreateFolder = useCallback(() => {
    if (!vault) return;
    setDialog({
      title: t("sidebar.newFolderPrompt"),
      initial: "",
      onSubmit: async (name) => {
        if (!name.trim()) return;
        await createFolder(vault, name.trim());
        await refreshNotes(vault);
      },
    });
  }, [vault, refreshNotes, t]);

  // Move a note into a folder ("" = root). Used by both the dialog and drag-drop.
  const moveTo = useCallback(
    async (path: string, folder: string) => {
      if (!vault) return;
      flushSave();
      const newPath = await moveNote(vault, path, folder.trim());
      await refreshNotes(vault);
      if (remote) {
        const note = await readNote(vault, newPath);
        pushRemote(newPath, note.content);
        deleteRemote(path);
      }
      if (activePath === path) setActivePath(newPath);
    },
    [vault, activePath, flushSave, refreshNotes, remote, pushRemote, deleteRemote]
  );

  const handleMove = useCallback(
    (path: string) => {
      if (!vault) return;
      setDialog({
        title: t("sidebar.movePrompt"),
        initial: "",
        suggestions: folders,
        onSubmit: (folder) => void moveTo(path, folder),
      });
    },
    [vault, folders, moveTo, t]
  );

  // Keep the graph current while it is on screen: moving a note changes its
  // path, and the node's colour comes from its folder — so a move has to be
  // reflected immediately, not only after leaving and re-entering the view.
  // Only the set of paths matters here: editing a note's text must not restart
  // the layout, but moving one (which changes its folder colour) must.
  const notePathsKey = notes.map((n) => n.path).join("|");
  useEffect(() => {
    if (view !== "graph" || !vault) return;
    void buildGraph(vault).then(setGraph);
  }, [notePathsKey, view, vault]);

  const showGraph = useCallback(async () => {
    if (!vault) return;
    setGraph(await buildGraph(vault));
    setView("graph");
  }, [vault]);

  // Connect a remote WebDAV vault: sync it into a local cache and open that.
  const connectRemote = useCallback(async (cfg: RemoteConfig) => {
    const cacheDir = await remoteConnect(cfg);
    setRemote(cfg);
    setVault(cacheDir);
    setNotes(await listNotes(cacheDir));
    setActivePath(null);
    setContent("");
    setLinks([]);
    // Remember URL + username (never the password) to prefill next time.
    try {
      localStorage.setItem(
        "magma.remote",
        JSON.stringify({ url: cfg.url, username: cfg.username })
      );
    } catch {
      /* ignore */
    }
  }, []);

  // Debounced search as the user types.
  useEffect(() => {
    if (!vault || query.trim() === "") {
      setHits([]);
      return;
    }
    const t = window.setTimeout(async () => {
      setHits(await searchNotes(vault, query));
    }, 150);
    return () => window.clearTimeout(t);
  }, [vault, query]);

  // Cmd/Ctrl+N → new note.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "n") {
        e.preventDefault();
        void createNewNote();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [createNewNote]);

  // Suppress the webview's own context menu (Look Up / Translate / Inspect
  // Element — the system's, in the system's language). Text you can actually
  // edit keeps its menu, so copy and paste still work where they matter.
  useEffect(() => {
    const onContextMenu = (e: MouseEvent) => {
      const el = e.target as HTMLElement | null;
      const editable =
        el?.closest("input, textarea, [contenteditable='true'], .magma-prose") !== null;
      if (!editable) e.preventDefault();
    };
    window.addEventListener("contextmenu", onContextMenu);
    return () => window.removeEventListener("contextmenu", onContextMenu);
  }, []);

  // Keep the sidebar in sync with the files on disk — notes created by Claude
  // (via MCP) or edited elsewhere appear without restarting. Refresh when the
  // window regains focus and on a gentle interval while a vault is open.
  useEffect(() => {
    if (!vault) return;
    const sync = () => void refreshNotes(vault);
    window.addEventListener("focus", sync);
    const id = window.setInterval(sync, 4000);
    return () => {
      window.removeEventListener("focus", sync);
      window.clearInterval(id);
    };
  }, [vault, refreshNotes]);

  useEffect(() => {
    return () => {
      if (saveTimer.current) window.clearTimeout(saveTimer.current);
    };
  }, []);

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Splash />
      {dialog && (
        <PromptDialog
          title={dialog.title}
          initial={dialog.initial}
          suggestions={dialog.suggestions}
          confirmLabel={t("dialog.ok")}
          cancelLabel={t("dialog.cancel")}
          onSubmit={(v) => {
            dialog.onSubmit(v);
            setDialog(null);
          }}
          onCancel={() => setDialog(null)}
        />
      )}
      {confirm && (
        <ConfirmDialog
          title={confirm.title}
          detail={confirm.detail}
          destructive
          confirmLabel={t("dialog.delete")}
          cancelLabel={t("dialog.cancel")}
          onConfirm={() => {
            confirm.onConfirm();
            setConfirm(null);
          }}
          onCancel={() => setConfirm(null)}
        />
      )}
      {showSettings && (
        <Settings
          onClose={() => setShowSettings(false)}
          vault={vault}
          folders={folders}
          notes={notes}
          onConnectRemote={connectRemote}
          remoteActive={!!remote}
        />
      )}
      <Sidebar
        vault={vault}
        notes={notes}
        folders={folders}
        activePath={activePath}
        onOpenVault={openVault}
        onSelect={selectNote}
        onCreate={createNewNote}
        onCreateFolder={handleCreateFolder}
        onRename={handleRename}
        onDelete={handleDelete}
        onMove={handleMove}
        onMoveTo={moveTo}
        onDeleteFolder={handleDeleteFolder}
        query={query}
        onQuery={setQuery}
        searchHits={hits}
        onOpenSettings={() => setShowSettings(true)}
      />

      <main className="flex flex-1 flex-col overflow-hidden">
        {vault && (
          <div className="flex items-center gap-1 border-b border-black/5 px-3 py-1.5 dark:border-white/10">
            <ViewTab label={t("view.editor")} active={view === "editor"} onClick={() => setView("editor")} />
            <ViewTab label={t("view.graph")} active={view === "graph"} onClick={showGraph} />
          </div>
        )}

        <div className="flex flex-1 flex-col overflow-hidden">
          {view === "graph" ? (
            <GraphView
              graph={graph}
              activePath={activePath}
              onSelect={(path) => {
                const node = graph.nodes.find((n) => n.path === path);
                setPreview({ path, title: node?.title ?? path });
              }}
            />
          ) : activePath ? (
            <>
              <div className="flex-1 overflow-hidden">
                <Editor
                  key={activePath}
                  value={content}
                  onChange={handleChange}
                  onOpenLink={openByName}
                  onOpenExternal={(url) => {
                    void openExternal(url);
                  }}
                  notes={notes}
                />
              </div>
              <BacklinksPanel backlinks={links} onSelect={selectNote} />
            </>
          ) : (
            <EmptyState
              connected={hasTauri}
              hasVault={!!vault}
              onCreate={createNewNote}
              onOpenVault={openVault}
            />
          )}
        </div>
      </main>

      {view === "graph" && preview && vault && (
        <NodePreview
          vault={vault}
          nodePath={preview.path}
          title={preview.title}
          onOpenEditor={(p) => {
            setPreview(null);
            void selectNote(p);
          }}
          onCreate={(name) => {
            setPreview(null);
            void openByName(name);
          }}
          onClose={() => setPreview(null)}
        />
      )}
    </div>
  );
}

function ViewTab({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`rounded-md px-3 py-1 text-sm transition ${
        active
          ? "bg-magma-accent/10 text-magma-accent"
          : "text-magma-muted hover:bg-black/5 dark:hover:bg-white/10"
      }`}
    >
      {label}
    </button>
  );
}

function EmptyState({
  connected,
  hasVault,
  onCreate,
  onOpenVault,
}: {
  connected: boolean;
  hasVault: boolean;
  onCreate: () => void;
  onOpenVault: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 px-8 text-center text-magma-muted">
      <FlameIcon size={44} />
      <h1 className="text-xl font-semibold tracking-tight text-magma-ink dark:text-[#ece9e4]">
        {t("app.tagline")}
      </h1>
      <p className="max-w-sm text-sm leading-relaxed">
        {hasVault ? t("empty.pickOrCreate") : t("empty.openVault")}
      </p>
      <button
        onClick={hasVault ? onCreate : onOpenVault}
        className="mt-1 rounded-xl bg-magma-accent px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:opacity-90 active:scale-[0.98]"
      >
        {hasVault ? t("empty.newNote") : t("sidebar.openVault")}
      </button>
      {!connected && (
        <p className="mt-2 text-xs opacity-60">{t("empty.browserPreview")}</p>
      )}
    </div>
  );
}
