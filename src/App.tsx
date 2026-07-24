import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Sidebar from "./components/Sidebar";
import Editor from "./components/Editor";
import GraphView from "./components/GraphView";
import BacklinksPanel from "./components/BacklinksPanel";
import Splash from "./components/Splash";
import Settings from "./components/Settings";
import FlameIcon from "./components/FlameIcon";
import { useI18n } from "./lib/i18n";
import {
  backlinks as fetchBacklinks,
  buildGraph,
  createNote,
  deleteNote,
  hasTauri,
  listNotes,
  pickVault,
  readNote,
  remoteConnect,
  remoteDelete,
  remotePut,
  renameNote,
  saveAsset,
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
  const [activePath, setActivePath] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const [view, setView] = useState<View>("editor");
  const [graph, setGraph] = useState<Graph>({ nodes: [], edges: [] });
  const [links, setLinks] = useState<NoteMeta[]>([]);
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [showSettings, setShowSettings] = useState(false);
  const [remote, setRemote] = useState<RemoteConfig | null>(null);
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

  const titles = useMemo(() => notes.map((n) => n.title), [notes]);

  const refreshNotes = useCallback(async (v: string) => {
    setNotes(await listNotes(v));
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
      setActivePath(path);
      setContent(note.content);
      setView("editor");
      setLinks(await fetchBacklinks(vault, path));
    },
    [vault]
  );

  // Open a note by its title (from a clicked wikilink). Falls back to creating
  // the note if it doesn't exist yet — links stay useful even before the
  // target is written.
  const openByTitle = useCallback(
    async (title: string) => {
      if (!vault) return;
      const found = notes.find((n) => n.title.toLowerCase() === title.toLowerCase());
      if (found) {
        await selectNote(found.path);
      } else {
        const path = await createNote(vault, title);
        await refreshNotes(vault);
        await selectNote(path);
        if (remote) {
          const note = await readNote(vault, path);
          pushRemote(path, note.content);
        }
      }
    },
    [vault, notes, selectNote, refreshNotes, remote, pushRemote]
  );

  const flushSave = useCallback(() => {
    if (saveTimer.current) {
      window.clearTimeout(saveTimer.current);
      saveTimer.current = null;
    }
  }, []);

  const handleChange = useCallback(
    (next: string) => {
      setContent(next);
      if (!vault || !activePath) return;
      if (saveTimer.current) window.clearTimeout(saveTimer.current);
      saveTimer.current = window.setTimeout(() => {
        void writeNote(vault, activePath, next).then(() => pushRemote(activePath, next));
      }, AUTOSAVE_MS);
    },
    [vault, activePath, pushRemote]
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
    async (path: string, currentTitle: string) => {
      if (!vault) return;
      const next = window.prompt(t("prompt.rename"), currentTitle);
      if (!next || next === currentTitle) return;
      flushSave();
      const newPath = await renameNote(vault, path, next);
      await refreshNotes(vault);
      if (activePath === path) await selectNote(newPath);
      if (remote) {
        const note = await readNote(vault, newPath);
        pushRemote(newPath, note.content);
        deleteRemote(path);
      }
    },
    [vault, activePath, flushSave, refreshNotes, selectNote, remote, pushRemote, deleteRemote, t]
  );

  const handleDelete = useCallback(
    async (path: string, title: string) => {
      if (!vault) return;
      if (!window.confirm(t("confirm.delete", { title }))) return;
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
    [vault, activePath, flushSave, refreshNotes, deleteRemote, t]
  );

  const handlePasteImage = useCallback(
    async (file: File): Promise<string | null> => {
      if (!vault) return null;
      const bytes = new Uint8Array(await file.arrayBuffer());
      const name = file.name && file.name !== "image.png" ? file.name : "pasted.png";
      const rel = await saveAsset(vault, name, bytes);
      return `![](${rel})`;
    },
    [vault]
  );

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

  useEffect(() => {
    return () => {
      if (saveTimer.current) window.clearTimeout(saveTimer.current);
    };
  }, []);

  return (
    <div className="flex h-screen w-screen overflow-hidden">
      <Splash />
      {showSettings && (
        <Settings
          onClose={() => setShowSettings(false)}
          vault={vault}
          onConnectRemote={connectRemote}
          remoteActive={!!remote}
        />
      )}
      <Sidebar
        vault={vault}
        notes={notes}
        activePath={activePath}
        onOpenVault={openVault}
        onSelect={selectNote}
        onCreate={createNewNote}
        onRename={handleRename}
        onDelete={handleDelete}
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
            <GraphView graph={graph} activePath={activePath} onSelect={selectNote} />
          ) : activePath ? (
            <>
              <div className="flex-1 overflow-hidden">
                <Editor
                  key={activePath}
                  value={content}
                  onChange={handleChange}
                  onPasteImage={handlePasteImage}
                  getNoteTitles={() => titles}
                  onOpenLink={openByTitle}
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
