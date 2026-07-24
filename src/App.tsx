import { useCallback, useEffect, useRef, useState } from "react";
import Sidebar from "./components/Sidebar";
import Editor from "./components/Editor";
import {
  createNote,
  deleteNote,
  hasTauri,
  listNotes,
  pickVault,
  readNote,
  renameNote,
  saveAsset,
  writeNote,
  type NoteMeta,
} from "./lib/api";

const AUTOSAVE_MS = 600;

export default function App() {
  const [vault, setVault] = useState<string | null>(null);
  const [notes, setNotes] = useState<NoteMeta[]>([]);
  const [activePath, setActivePath] = useState<string | null>(null);
  const [content, setContent] = useState("");
  const saveTimer = useRef<number | null>(null);

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
    },
    [vault]
  );

  // Flush any pending autosave immediately — used before structural changes
  // (create/rename/delete) so we never race the debounce.
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
        void writeNote(vault, activePath, next);
      }, AUTOSAVE_MS);
    },
    [vault, activePath]
  );

  const createNewNote = useCallback(async () => {
    if (!vault) return;
    flushSave();
    const path = await createNote(vault, "Untitled");
    await refreshNotes(vault);
    await selectNote(path);
  }, [vault, flushSave, refreshNotes, selectNote]);

  const handleRename = useCallback(
    async (path: string, currentTitle: string) => {
      if (!vault) return;
      const next = window.prompt("Rename note", currentTitle);
      if (!next || next === currentTitle) return;
      flushSave();
      const newPath = await renameNote(vault, path, next);
      await refreshNotes(vault);
      if (activePath === path) await selectNote(newPath);
    },
    [vault, activePath, flushSave, refreshNotes, selectNote]
  );

  const handleDelete = useCallback(
    async (path: string, title: string) => {
      if (!vault) return;
      if (!window.confirm(`Delete "${title}"? This cannot be undone.`)) return;
      flushSave();
      await deleteNote(vault, path);
      await refreshNotes(vault);
      if (activePath === path) {
        setActivePath(null);
        setContent("");
      }
    },
    [vault, activePath, flushSave, refreshNotes]
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
      <Sidebar
        vault={vault}
        notes={notes}
        activePath={activePath}
        onOpenVault={openVault}
        onSelect={selectNote}
        onCreate={createNewNote}
        onRename={handleRename}
        onDelete={handleDelete}
      />

      <main className="flex-1 overflow-hidden">
        {activePath ? (
          <Editor
            key={activePath}
            value={content}
            onChange={handleChange}
            onPasteImage={handlePasteImage}
          />
        ) : (
          <EmptyState connected={hasTauri} hasVault={!!vault} onCreate={createNewNote} />
        )}
      </main>
    </div>
  );
}

function EmptyState({
  connected,
  hasVault,
  onCreate,
}: {
  connected: boolean;
  hasVault: boolean;
  onCreate: () => void;
}) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-center text-magma-muted">
      <div className="h-10 w-10 rounded-full bg-magma-accent/20" />
      <h1 className="text-lg font-medium text-magma-ink dark:text-[#ece9e4]">
        Your second brain, minus the setup.
      </h1>
      <p className="max-w-sm text-sm">
        {hasVault
          ? "Pick a note on the left, or create a new one."
          : "Open a folder of markdown files to start. Your notes stay plain files on your disk — readable by you and, when you turn it on, by Claude."}
      </p>
      {hasVault && (
        <button
          onClick={onCreate}
          className="mt-1 rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90"
        >
          New note
        </button>
      )}
      {!connected && (
        <p className="mt-2 text-xs opacity-60">
          (Running in the browser preview — launch the desktop app for full vault
          access.)
        </p>
      )}
    </div>
  );
}
