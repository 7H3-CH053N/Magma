import { useCallback, useEffect, useRef, useState } from "react";
import { useEditor, EditorContent, BubbleMenu, type Editor as TiptapEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Link from "@tiptap/extension-link";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";
import Placeholder from "@tiptap/extension-placeholder";
import Table from "@tiptap/extension-table";
import TableRow from "@tiptap/extension-table-row";
import TableHeader from "@tiptap/extension-table-header";
import TableCell from "@tiptap/extension-table-cell";
import { Markdown } from "tiptap-markdown";
import { WikiLink } from "../lib/wikilinkExtension";

interface EditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  /** Open a note by its `[[wikilink]]` name (filename stem). */
  onOpenLink?: (name: string) => void;
  /** Open an http(s) link in the system browser. */
  onOpenExternal?: (url: string) => void;
  /** Every note in the vault, for the link picker and missing-link styling. */
  notes?: { path: string; title: string }[];
}

/** The `[[name]]` a wikilink must use to reach a note: its filename stem. */
function stemOf(path: string): string {
  const file = path.split("/").pop() ?? path;
  return file.replace(/\.md$/i, "");
}

/**
 * True WYSIWYG editor (TipTap / ProseMirror). Formatting is live — you never
 * see raw markdown. Blocks are created the Notion way, by typing shortcuts
 * (`# `, `- `, `[] `, `> `, ``` ``` ), and inline styling via the floating
 * toolbar or `Cmd/Ctrl+B` etc. Content is stored as plain markdown so the vault
 * stays portable.
 */
export default function Editor({
  value,
  onChange,
  placeholder,
  onOpenLink,
  onOpenExternal,
  notes = [],
}: EditorProps) {
  // Read through a ref so the extension sees the current notes without the
  // editor having to be rebuilt every time the vault list changes.
  const stemsRef = useRef<Set<string>>(new Set());
  stemsRef.current = new Set(notes.map((n) => stemOf(n.path).toLowerCase()));
  const [picker, setPicker] = useState<{ text: string } | null>(null);

  const editor = useEditor({
    immediatelyRender: false,
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3] },
      }),
      Link.configure({ openOnClick: false, autolink: true }),
      TaskList,
      TaskItem.configure({ nested: true }),
      Table.configure({ resizable: false }),
      TableRow,
      TableHeader,
      TableCell,
      WikiLink.configure({
        onOpen: (name) => onOpenLink?.(name),
        exists: (name) => stemsRef.current.has(name.toLowerCase()),
      }),
      Placeholder.configure({ placeholder: placeholder ?? "Start writing…" }),
      Markdown.configure({
        html: false,
        transformPastedText: true,
        transformCopiedText: true,
        linkify: true,
        breaks: true,
      }),
    ],
    content: value,
    editorProps: {
      attributes: { class: "magma-prose" },
      // A click on a real link opens it in the system browser instead of
      // navigating the WebView (or just placing the caret). Wikilinks are
      // handled by their own extension.
      handleClick(_view, _pos, event) {
        const el = (event.target as HTMLElement | null)?.closest("a");
        const href = el?.getAttribute("href");
        if (href && /^https?:\/\//i.test(href)) {
          event.preventDefault();
          onOpenExternal?.(href);
          return true;
        }
        return false;
      },
    },
    onUpdate: ({ editor }) => {
      onChange(editor.storage.markdown.getMarkdown());
    },
  });

  // When the note switches, the parent remounts via `key`, so this mainly
  // guards programmatic content changes (e.g. remote sync) without clobbering
  // what the user is typing.
  useEffect(() => {
    if (!editor) return;
    const current = editor.storage.markdown.getMarkdown();
    if (value !== current && !editor.isFocused) {
      editor.commands.setContent(value, false);
    }
  }, [value, editor]);

  // Selected text -> open the picker so it can become a link.
  const startLink = useCallback(() => {
    if (!editor) return;
    const { from, to } = editor.state.selection;
    setPicker({ text: editor.state.doc.textBetween(from, to, " ").trim() });
  }, [editor]);

  // Cmd/Ctrl+K on a selection, the way every other editor does it.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k" && editor?.isFocused) {
        e.preventDefault();
        startLink();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [editor, startLink]);

  // Replace the selection with a wikilink. The link must name the target's
  // filename; when the visible text differs, it is kept as an alias.
  const insertLink = useCallback(
    (stem: string) => {
      if (!editor) return;
      const { from, to } = editor.state.selection;
      const selected = editor.state.doc.textBetween(from, to, " ").trim();
      const markup =
        selected && selected.toLowerCase() !== stem.toLowerCase()
          ? `[[${stem}|${selected}]]`
          : `[[${stem}]]`;
      editor.chain().focus().insertContentAt({ from, to }, markup).run();
      setPicker(null);
    },
    [editor]
  );

  if (!editor) return <div className="h-full w-full" />;

  return (
    <div className="relative h-full w-full overflow-auto">
      <BubbleMenu editor={editor} tippyOptions={{ duration: 120 }}>
        <Toolbar editor={editor} onLink={startLink} />
      </BubbleMenu>
      <EditorContent editor={editor} className="mx-auto max-w-[var(--magma-reading-width)] px-8 py-6" />
      {picker && (
        <LinkPicker
          initial={picker.text}
          notes={notes}
          onPick={insertLink}
          onCancel={() => setPicker(null)}
        />
      )}
    </div>
  );
}

/** Choose the note a selection should link to, or name a new one. */
function LinkPicker({
  initial,
  notes,
  onPick,
  onCancel,
}: {
  initial: string;
  notes: { path: string; title: string }[];
  onPick: (stem: string) => void;
  onCancel: () => void;
}) {
  const [q, setQ] = useState(initial);
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  const needle = q.trim().toLowerCase();
  const matches = notes
    .filter(
      (n) =>
        !needle ||
        n.title.toLowerCase().includes(needle) ||
        stemOf(n.path).toLowerCase().includes(needle)
    )
    .slice(0, 30);
  const exact = matches.some((n) => stemOf(n.path).toLowerCase() === needle);

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4" onClick={onCancel}>
      <div
        className="w-full max-w-md rounded-2xl bg-magma-bg p-4 shadow-xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={q}
          onChange={(e) => setQ(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onCancel();
            if (e.key === "Enter") {
              e.preventDefault();
              if (matches.length) onPick(stemOf(matches[0].path));
              else if (q.trim()) onPick(q.trim());
            }
          }}
          placeholder="Notiz suchen oder neuen Namen eingeben…"
          className="w-full rounded-lg border border-black/10 bg-transparent px-3 py-2 text-sm outline-none focus:border-magma-accent dark:border-white/10"
        />
        <div className="mt-2 max-h-72 overflow-auto">
          {matches.map((n) => (
            <button
              key={n.path}
              onClick={() => onPick(stemOf(n.path))}
              className="block w-full rounded-md px-2 py-1.5 text-left transition hover:bg-black/5 dark:hover:bg-white/10"
            >
              <span className="block truncate text-sm">{n.title}</span>
              <span className="block truncate text-xs text-magma-muted">{n.path}</span>
            </button>
          ))}
          {!exact && q.trim() && (
            <button
              onClick={() => onPick(q.trim())}
              className="block w-full rounded-md px-2 py-1.5 text-left text-sm text-magma-accent transition hover:bg-black/5 dark:hover:bg-white/10"
            >
              Neue Notiz „{q.trim()}" verlinken (wird beim Klick angelegt)
            </button>
          )}
          {matches.length === 0 && !q.trim() && (
            <p className="px-2 py-3 text-sm text-magma-muted">Keine Notizen.</p>
          )}
        </div>
      </div>
    </div>
  );
}

function Toolbar({ editor, onLink }: { editor: TiptapEditor; onLink: () => void }) {
  const btn = (active: boolean) =>
    `px-2 py-1 text-sm rounded-md transition ${
      active ? "text-magma-accent" : "text-white/90 hover:text-white"
    }`;
  return (
    <div className="flex items-center gap-0.5 rounded-lg bg-[#2a2622] px-1 py-1 shadow-lg">
      <button
        className={btn(editor.isActive("bold"))}
        onClick={() => editor.chain().focus().toggleBold().run()}
        title="Bold (Cmd/Ctrl+B)"
      >
        <b>B</b>
      </button>
      <button
        className={btn(editor.isActive("italic"))}
        onClick={() => editor.chain().focus().toggleItalic().run()}
        title="Italic (Cmd/Ctrl+I)"
      >
        <i>I</i>
      </button>
      <button
        className={btn(editor.isActive("strike"))}
        onClick={() => editor.chain().focus().toggleStrike().run()}
        title="Strikethrough"
      >
        <s>S</s>
      </button>
      <button
        className={btn(editor.isActive("code"))}
        onClick={() => editor.chain().focus().toggleCode().run()}
        title="Inline code"
      >
        {"</>"}
      </button>
      <button
        className={btn(false)}
        onClick={onLink}
        title="Als Wikilink verlinken (Cmd/Ctrl+K)"
      >
        🔗
      </button>
      <span className="mx-1 h-4 w-px bg-white/20" />
      <button
        className={btn(editor.isActive("heading", { level: 1 }))}
        onClick={() => editor.chain().focus().toggleHeading({ level: 1 }).run()}
        title="Heading 1"
      >
        H1
      </button>
      <button
        className={btn(editor.isActive("heading", { level: 2 }))}
        onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()}
        title="Heading 2"
      >
        H2
      </button>
      <button
        className={btn(editor.isActive("bulletList"))}
        onClick={() => editor.chain().focus().toggleBulletList().run()}
        title="Bullet list"
      >
        •
      </button>
      <button
        className={btn(editor.isActive("taskList"))}
        onClick={() => editor.chain().focus().toggleTaskList().run()}
        title="Checklist"
      >
        ☑
      </button>
      <button
        className={btn(editor.isActive("blockquote"))}
        onClick={() => editor.chain().focus().toggleBlockquote().run()}
        title="Quote"
      >
        ❝
      </button>
    </div>
  );
}
