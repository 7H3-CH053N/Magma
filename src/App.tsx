import { useCallback, useEffect, useRef, useState } from "react";
import Sidebar from "./components/Sidebar";
import Editor from "./components/Editor";
import {
  hasTauri,
  listNotes,
  pickVault,
  readNote,
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

  // Debounced autosave — no save button, no dirty state to reason about.
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
      />

      <main className="flex-1 overflow-hidden">
        {activePath ? (
          <Editor key={activePath} value={content} onChange={handleChange} />
        ) : (
          <EmptyState connected={hasTauri} />
        )}
      </main>
    </div>
  );
}

function EmptyState({ connected }: { connected: boolean }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-center text-magma-muted">
      <div className="h-10 w-10 rounded-full bg-magma-accent/20" />
      <h1 className="text-lg font-medium text-magma-ink dark:text-[#ece9e4]">
        Your second brain, minus the setup.
      </h1>
      <p className="max-w-sm text-sm">
        Open a folder of markdown files to start. Your notes stay plain files on
        your disk — readable by you and, when you turn it on, by Claude.
      </p>
      {!connected && (
        <p className="mt-2 text-xs opacity-60">
          (Running in the browser preview — launch the desktop app for full
          vault access.)
        </p>
      )}
    </div>
  );
}
