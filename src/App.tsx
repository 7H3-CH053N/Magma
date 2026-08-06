import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import Sidebar from "./components/Sidebar";
import Editor from "./components/Editor";
import GraphView from "./components/GraphView";
import ConnectionsPanel from "./components/ConnectionsPanel";
import CommandPalette, { type Command } from "./components/CommandPalette";
import CalendarPanel from "./components/CalendarPanel";
import QuickCapture from "./components/QuickCapture";
import HistoryDialog from "./components/HistoryDialog";
import Onboarding, { onboardingSeen } from "./components/Onboarding";
import AiReview from "./components/AiReview";
import Splash from "./components/Splash";
import Settings from "./components/Settings";
import FlameIcon from "./components/FlameIcon";
import PromptDialog from "./components/PromptDialog";
import ConfirmDialog from "./components/ConfirmDialog";
import NodePreview from "./components/NodePreview";
import ReplaceDialog from "./components/ReplaceDialog";
import { useI18n } from "./lib/i18n";
import { applyTemplate, dayKey, usePrefs } from "./lib/prefs";
import { pluginCommands } from "./lib/plugins";
import { splitFrontmatter, joinFrontmatter } from "./lib/markdown";
import {
  appendNote,
  backlinks as fetchBacklinks,
  buildGraph,
  createFolder,
  createNote,
  deleteFolder,
  deleteNote,
  hasTauri,
  lastVault,
  listFolders,
  listNotes,
  moveFolder,
  moveNote,
  openExternal,
  openOrCreate,
  pickVault,
  readNote,
  remoteConnect,
  remoteDelete,
  remotePut,
  renameNote,
  search as searchNotes,
  setLanguage,
  setLastVault,
  writeNote,
  type Graph,
  type NoteMeta,
  type RemoteConfig,
  type ReplaceReport,
  type SearchHit,
} from "./lib/api";

const AUTOSAVE_MS = 600;
type View = "editor" | "graph" | "ai";

export default function App() {
  const { t, lang } = useI18n();
  const { prefs } = usePrefs();
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
  const [showReplace, setShowReplace] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [showCapture, setShowCapture] = useState(false);
  const [showHistory, setShowHistory] = useState(false);
  const [showOnboarding, setShowOnboarding] = useState(() => !onboardingSeen());
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
    /** An error or notice: one button, nothing destructive about it. */
    notice?: boolean;
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
      // Remembered on the Rust side, so the next start opens it straight away.
      void setLastVault(picked).catch(() => {});
    }
  }, [refreshNotes]);

  // Reopen the vault from the last session on start.
  useEffect(() => {
    void (async () => {
      const last = await lastVault().catch(() => null);
      if (last) {
        setVault(last);
        await refreshNotes(last);
      }
    })();
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

  // After a vault-wide replace: everything on disk changed underneath us.
  const handleReplaced = useCallback(
    async (report: ReplaceReport) => {
      if (!vault) return;
      // A queued autosave still holds the pre-replace text — letting it fire
      // would write the old wording straight back into the open note.
      flushSave();
      const fresh = await listNotes(vault);
      setNotes(fresh);
      try {
        setFolders(await listFolders(vault));
      } catch {
        setFolders([]);
      }
      setQuery("");
      if (!activePath) return;
      // The open note may have been renamed along with the text, in which case
      // its path is gone; find it again under its new name.
      if (fresh.some((n) => n.path === activePath)) {
        await selectNote(activePath);
        return;
      }
      const renamed = report.renames.find((r) => r.path === activePath);
      const stem = (p: string) => (p.split("/").pop() ?? p).replace(/\.md$/i, "");
      const moved =
        renamed && fresh.find((n) => stem(n.path).toLowerCase() === renamed.to.toLowerCase());
      if (moved) await selectNote(moved.path);
      else setActivePath(null);
    },
    [vault, activePath, flushSave, selectNote]
  );

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

  /** Read a template note's body (frontmatter stripped), or "" if there is none. */
  const templateBody = useCallback(
    async (path: string): Promise<string> => {
      if (!vault || !path) return "";
      try {
        const note = await readNote(vault, path);
        return splitFrontmatter(note.content).body;
      } catch {
        return "";
      }
    },
    [vault]
  );

  /** Open (or create) the note for a day — the calendar and the palette both land here. */
  const openDay = useCallback(
    async (date: Date) => {
      if (!vault) return;
      flushSave();
      const title = dayKey(date);
      const raw = await templateBody(prefs.dailyTemplate);
      const seed = raw ? applyTemplate(raw, title, date) : `# ${title}\n\n`;
      const { path } = await openOrCreate(vault, prefs.dailyFolder, title, seed);
      await refreshNotes(vault);
      await selectNote(path);
    },
    [vault, prefs.dailyFolder, prefs.dailyTemplate, flushSave, refreshNotes, selectNote, templateBody]
  );

  /** New note seeded from a template note, asking for the title first. */
  const newFromTemplate = useCallback(
    (templatePath: string, templateName: string) => {
      if (!vault) return;
      setDialog({
        title: t("template.prompt", { name: templateName }),
        initial: "",
        onSubmit: async (title) => {
          if (!title.trim()) return;
          flushSave();
          const raw = await templateBody(templatePath);
          const seed = applyTemplate(raw, title.trim());
          const path = await createNote(vault, title.trim());
          await writeNote(vault, path, seed || `# ${title.trim()}\n\n`);
          await refreshNotes(vault);
          await selectNote(path);
        },
      });
    },
    [vault, flushSave, refreshNotes, selectNote, templateBody, t]
  );

  /** Quick capture: append to today's note, without opening it. */
  const captureText = useCallback(
    async (text: string) => {
      if (!vault) return;
      const stamp = new Date();
      const line = prefs.captureToDaily
        ? `- ${String(stamp.getHours()).padStart(2, "0")}:${String(
            stamp.getMinutes()
          ).padStart(2, "0")} ${text.trim()}`
        : text.trim();
      const title = dayKey(stamp);
      const { path } = await openOrCreate(
        vault,
        prefs.dailyFolder,
        title,
        `# ${title}\n\n`
      );
      await appendNote(vault, path, line);
      await refreshNotes(vault);
      // If that note happens to be open, pull the appended text in.
      if (activePath === path) await selectNote(path);
    },
    [vault, prefs.dailyFolder, prefs.captureToDaily, activePath, refreshNotes, selectNote]
  );

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

  /** New folder; `parent` is "" for the vault root, or a folder to nest under. */
  const handleCreateFolder = useCallback(
    (parent = "") => {
      if (!vault) return;
      setDialog({
        title: parent
          ? t("sidebar.newSubfolderPrompt", { parent })
          : t("sidebar.newFolderPrompt"),
        initial: "",
        onSubmit: async (name) => {
          if (!name.trim()) return;
          const full = parent ? `${parent}/${name.trim()}` : name.trim();
          await createFolder(vault, full);
          await refreshNotes(vault);
        },
      });
    },
    [vault, refreshNotes, t]
  );

  /** Move a folder (with everything in it) into another; "" is the vault root. */
  const moveFolderTo = useCallback(
    async (folder: string, into: string) => {
      if (!vault) return;
      flushSave();
      try {
        const moved = await moveFolder(vault, folder, into);
        await refreshNotes(vault);
        // The open note may have travelled with the folder.
        if (activePath?.startsWith(`${folder}/`)) {
          setActivePath(`${moved}${activePath.slice(folder.length)}`);
        }
      } catch (e) {
        // The Rust side refuses a move into itself or onto an existing name.
        setConfirm({
          title: t("sidebar.moveFolderFailed"),
          detail: String(e),
          notice: true,
          onConfirm: () => {},
        });
      }
    },
    [vault, activePath, flushSave, refreshNotes, t]
  );

  /** Ask which folder to move a folder into — drag & drop without the drag. */
  const handleMoveFolder = useCallback(
    (folder: string) => {
      if (!vault) return;
      setDialog({
        title: t("sidebar.moveFolderPrompt", { folder }),
        initial: "",
        // Its own subtree would be a paradox, so those are not offered.
        suggestions: folders.filter(
          (f) => f !== folder && !f.startsWith(`${folder}/`)
        ),
        onSubmit: (into) => void moveFolderTo(folder, into.trim()),
      });
    },
    [vault, folders, moveFolderTo, t]
  );

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
  // Templates are scaffolding, not knowledge: their placeholder links would
  // put a node in the graph for every "{{title}}" and connect nothing.
  const graphExclude = useMemo(
    () => [prefs.templateFolder.trim()].filter(Boolean),
    [prefs.templateFolder]
  );

  const notePathsKey = notes.map((n) => n.path).join("|");
  useEffect(() => {
    if (view !== "graph" || !vault) return;
    void buildGraph(vault, graphExclude).then(setGraph);
  }, [notePathsKey, view, vault, graphExclude]);

  const showGraph = useCallback(async () => {
    if (!vault) return;
    setGraph(await buildGraph(vault, graphExclude));
    setView("graph");
  }, [vault, graphExclude]);

  // Connect a remote WebDAV vault: sync it into a local cache and open that.
  const connectRemote = useCallback(async (cfg: RemoteConfig) => {
    const cacheDir = await remoteConnect(cfg);
    setRemote(cfg);
    setVault(cacheDir);
    setNotes(await listNotes(cacheDir));
    void setLastVault(cacheDir).catch(() => {});
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

  /** Notes inside the template folder, offered as their own palette entries. */
  const templates = useMemo(() => {
    const folder = prefs.templateFolder.trim();
    if (!folder) return [];
    const prefix = `${folder}/`;
    return notes.filter((n) => n.path.startsWith(prefix));
  }, [notes, prefs.templateFolder]);

  /** Everything the palette can do, besides jumping to a note. */
  const commands = useMemo<Command[]>(() => {
    const pluginItems = pluginCommands(prefs.enabledPluginIds, {
      notes,
      activePath,
      content,
      t,
      openNote: (path) => void selectNote(path),
      notice: (title, detail) =>
        setConfirm({
          title,
          detail,
          notice: true,
          onConfirm: () => {},
        }),
    });
    const list: Command[] = [
      { id: "new", label: t("cmd.newNote"), hint: "⌘N", run: () => void createNewNote() },
      { id: "today", label: t("cmd.today"), run: () => void openDay(new Date()) },
      {
        id: "yesterday",
        label: t("cmd.yesterday"),
        run: () => {
          const d = new Date();
          d.setDate(d.getDate() - 1);
          void openDay(d);
        },
      },
      {
        id: "capture",
        label: t("cmd.capture"),
        hint: "⌘⇧N",
        run: () => setShowCapture(true),
      },
      { id: "graph", label: t("cmd.graph"), run: () => void showGraph() },
      { id: "editor", label: t("cmd.editor"), run: () => setView("editor") },
      { id: "ai", label: t("cmd.aiReview"), run: () => setView("ai") },
      { id: "replace", label: t("cmd.replace"), run: () => setShowReplace(true) },
      { id: "settings", label: t("cmd.settings"), run: () => setShowSettings(true) },
      { id: "folder", label: t("cmd.newFolder"), run: () => handleCreateFolder("") },
      { id: "openVault", label: t("cmd.openVault"), run: () => void openVault() },
      ...pluginItems,
    ];
    if (activePath) {
      list.push(
        { id: "history", label: t("cmd.history"), run: () => setShowHistory(true) },
        {
          id: "rename",
          label: t("cmd.rename"),
          run: () => {
            const note = notes.find((n) => n.path === activePath);
            if (note) handleRename(note.path, note.title);
          },
        },
        {
          id: "move",
          label: t("cmd.move"),
          run: () => handleMove(activePath),
        }
      );
    }
    for (const tpl of templates) {
      // A template's first heading is usually "{{title}}" — a placeholder, not
      // a name. Its filename is what the user actually called it.
      const name = (tpl.path.split("/").pop() ?? tpl.path).replace(/\.md$/i, "");
      list.push({
        id: `tpl:${tpl.path}`,
        label: t("cmd.fromTemplate", { name }),
        hint: t("cmd.templateHint"),
        run: () => newFromTemplate(tpl.path, name),
      });
    }
    return list;
  }, [
    t,
    createNewNote,
    openDay,
    showGraph,
    handleCreateFolder,
    openVault,
    activePath,
    notes,
    handleRename,
    handleMove,
    templates,
    newFromTemplate,
    prefs.enabledPluginIds,
    content,
    selectNote,
  ]);

  // Keyboard: new note, command palette, quick capture.
  //
  // The palette is on Cmd/Ctrl+P, not Cmd+K as originally planned — inside the
  // editor Cmd+K already inserts a link, the way it does in every other editor,
  // and taking that away to gain a palette would be a bad trade.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      const key = e.key.toLowerCase();
      if (key === "n" && e.shiftKey) {
        e.preventDefault();
        setShowCapture(true);
      } else if (key === "n") {
        e.preventDefault();
        void createNewNote();
      } else if (key === "p" || key === "o") {
        e.preventDefault();
        setShowPalette(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [createNewNote]);

  // Keep the native menu bar in the language the app is showing. Runs on start
  // too, so a fresh install matches before anything is saved in Settings.
  useEffect(() => {
    void setLanguage(lang).catch(() => {});
  }, [lang]);

  // Suppress the webview's own context menu everywhere — "Look Up", "Translate",
  // "Inspect Element", "Take Photo", in the system's language. It belongs to
  // Safari, not to Magma. Exempting the editor (so paste kept its menu) was the
  // wrong trade: that is exactly where you meet it most. Cut/copy/paste stay
  // available on Cmd/Ctrl+X/C/V.
  useEffect(() => {
    const onContextMenu = (e: MouseEvent) => e.preventDefault();
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
          destructive={!confirm.notice}
          confirmLabel={confirm.notice ? t("dialog.ok") : t("dialog.delete")}
          cancelLabel={confirm.notice ? "" : t("dialog.cancel")}
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
          onOpenVault={openVault}
        />
      )}
      {showReplace && vault && (
        <ReplaceDialog
          vault={vault}
          initialFind={query}
          onClose={() => setShowReplace(false)}
          onApplied={handleReplaced}
        />
      )}
      {showPalette && (
        <CommandPalette
          notes={notes}
          commands={commands}
          onOpenNote={(p) => void selectNote(p)}
          onClose={() => setShowPalette(false)}
        />
      )}
      {showCapture && vault && (
        <QuickCapture
          target={`${prefs.dailyFolder ? `${prefs.dailyFolder}/` : ""}${dayKey(new Date())}`}
          onSubmit={captureText}
          onClose={() => setShowCapture(false)}
        />
      )}
      {showHistory && vault && activePath && (
        <HistoryDialog
          vault={vault}
          path={activePath}
          title={notes.find((n) => n.path === activePath)?.title ?? activePath}
          current={joinFrontmatter(frontmatter.current, content)}
          onClose={() => setShowHistory(false)}
          onRestored={() => {
            flushSave();
            void selectNote(activePath);
          }}
        />
      )}
      {showOnboarding && (
        <Onboarding
          onOpenVault={openVault}
          onOpenSettings={() => setShowSettings(true)}
          onClose={() => setShowOnboarding(false)}
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
        onMoveFolder={(folder, into) => void moveFolderTo(folder, into)}
        onMoveFolderPrompt={handleMoveFolder}
        onRename={handleRename}
        onDelete={handleDelete}
        onMove={handleMove}
        onMoveTo={moveTo}
        onDeleteFolder={handleDeleteFolder}
        query={query}
        onQuery={setQuery}
        searchHits={hits}
        onReplace={() => setShowReplace(true)}
        onOpenSettings={() => setShowSettings(true)}
        onToday={() => void openDay(new Date())}
        calendar={
          <CalendarPanel
            notes={notes}
            folder={prefs.dailyFolder}
            onOpenDay={(d) => void openDay(d)}
          />
        }
      />

      <main className="flex flex-1 flex-col overflow-hidden">
        {vault && (
          <div className="flex items-center gap-1 border-b border-black/5 px-3 py-1.5 dark:border-white/10">
            <ViewTab label={t("view.editor")} active={view === "editor"} onClick={() => setView("editor")} />
            <ViewTab label={t("view.graph")} active={view === "graph"} onClick={showGraph} />
            <ViewTab label={t("view.ai")} active={view === "ai"} onClick={() => setView("ai")} />
            <div className="flex-1" />
            {activePath && (
              <button
                onClick={() => setShowHistory(true)}
                title={t("cmd.history")}
                className="rounded-md px-2 py-1 text-xs text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
              >
                {t("view.history")}
              </button>
            )}
          </div>
        )}

        <div className="flex flex-1 flex-col overflow-hidden">
          {view === "ai" ? (
            <AiReview notes={notes} onSelect={selectNote} />
          ) : view === "graph" ? (
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
              <ConnectionsPanel
                vault={vault}
                path={activePath}
                name={(activePath.split("/").pop() ?? "").replace(/\.md$/i, "")}
                backlinks={links}
                onSelect={selectNote}
                onOpenByName={openByName}
                onChanged={() => {
                  if (vault) void refreshNotes(vault);
                  void selectNote(activePath);
                }}
              />
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
