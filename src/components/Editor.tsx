import { useCallback, useEffect, useMemo, useRef, useState, type MutableRefObject } from "react";
import CodeBlock from "@tiptap/extension-code-block";
import {
  useEditor,
  EditorContent,
  BubbleMenu,
  NodeViewContent,
  NodeViewWrapper,
  ReactNodeViewRenderer,
  type Editor as TiptapEditor,
  type NodeViewProps,
} from "@tiptap/react";
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
import { queryDataview, type DataviewResult } from "../lib/api";
import { useI18n } from "../lib/i18n";

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
  /** Current vault path, used to evaluate Dataview blocks read-only. */
  vault?: string | null;
  /** Active note path; changes reset Dataview results for the new note. */
  path?: string | null;
}

/**
 * Markdown for the file. The serializer escapes `[` and `]`, which turns every
 * `[[Note]]` into `\[\[Note\]\]` — the string `[[` then no longer exists, so
 * link parsing finds nothing and editing a note silently breaks all its links.
 * Undo that escaping for wikilinks only.
 */
function toMarkdown(editor: TiptapEditor): string {
  return editor.storage.markdown
    .getMarkdown()
    .replace(/\\\[\\\[/g, "[[")
    .replace(/\\\]\\\]/g, "]]");
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
  vault,
  path,
}: EditorProps) {
  const { t } = useI18n();
  // Read through a ref so the extension sees the current notes without the
  // editor having to be rebuilt every time the vault list changes.
  const stemsRef = useRef<Set<string>>(new Set());
  stemsRef.current = new Set(notes.map((n) => stemOf(n.path).toLowerCase()));
  const [picker, setPicker] = useState<{ text: string } | null>(null);
  const dataviewContext = useRef<DataviewContext>({ vault, notePath: path, onOpenLink, t });
  dataviewContext.current = { vault, notePath: path, onOpenLink, t };
  const dataviewCodeBlock = useMemo(
    () => createDataviewCodeBlock(dataviewContext),
    [t]
  );

  /**
   * The markdown this editor itself last handed upwards.
   *
   * Without it, every keystroke came back as a changed `value` and the effect
   * below rebuilt the document — which puts the caret at the end. It looked
   * like a comparison against the editor's own text, but the two sides were
   * never the same string: what goes up is unescaped by `toMarkdown`, while
   * `getMarkdown()` returns `\[\[Note\]\]`. So in any note containing a
   * wikilink the two differed permanently, and the only thing standing between
   * the user and a jumping caret was the `isFocused` guard — which loses the
   * moment focus is anywhere else, exactly what happens when a link is
   * inserted from the picker.
   */
  const emitted = useRef(value);

  const editor = useEditor({
    immediatelyRender: false,
    extensions: [
      StarterKit.configure({
        heading: { levels: [1, 2, 3] },
        codeBlock: false,
      }),
      dataviewCodeBlock,
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
      const markdown = toMarkdown(editor);
      emitted.current = markdown;
      onChange(markdown);
    },
  }, [dataviewCodeBlock]);

  // Apply changes that came from outside the editor — a restored version, a
  // vault-wide replace, a remote sync — and nothing else.
  useEffect(() => {
    if (!editor) return;
    if (value === emitted.current || value === toMarkdown(editor)) return;
    // Keep the caret where it was; an external edit should not move the user.
    const { from, to } = editor.state.selection;
    editor.commands.setContent(value, false);
    const max = Math.max(0, editor.state.doc.content.size - 1);
    editor.commands.setTextSelection({
      from: Math.min(from, max),
      to: Math.min(to, max),
    });
    emitted.current = value;
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

interface DataviewContext {
  vault?: string | null;
  notePath?: string | null;
  onOpenLink?: (name: string) => void;
  t: (key: string, vars?: Record<string, string>) => string;
}

function createDataviewCodeBlock(context: MutableRefObject<DataviewContext>) {
  return CodeBlock.extend({
    addNodeView() {
      return ReactNodeViewRenderer((props) => (
        <DataviewCodeBlockView
          {...props}
          context={context}
        />
      ));
    },
  });
}

function DataviewCodeBlockView({
  node,
  context,
}: NodeViewProps & {
  context: MutableRefObject<DataviewContext>;
}) {
  const { vault, notePath, onOpenLink, t } = context.current;
  const language = String(node.attrs.language ?? "").toLowerCase();
  const query = node.textContent.trim();
  const [results, setResults] = useState<DataviewResult[]>([]);

  useEffect(() => {
    if (language !== "dataview" || !vault || !query) {
      setResults([]);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void queryDataview(vault, query)
        .then((next) => {
          if (!cancelled) setResults([next]);
        })
        .catch((error) => {
          if (!cancelled) {
            setResults([
              {
                kind: "error",
                columns: [],
                rows: [],
                items: [],
                error: String(error),
              },
            ]);
          }
        });
    }, 250);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [language, vault, notePath, query]);

  if (language !== "dataview") {
    return (
      <NodeViewWrapper as="pre">
        <NodeViewContent as="code" className={node.attrs.language ? `language-${node.attrs.language}` : ""} />
      </NodeViewWrapper>
    );
  }

  return (
    <NodeViewWrapper as="section" className="dataview-result">
      <DataviewResultView
        query={query}
        result={results[0]}
        onOpenLink={onOpenLink}
        t={t}
      />
      <details className="dataview-query">
        <summary>{t("dataview.editQuery")}</summary>
        <NodeViewContent as="pre" className="dataview-query-source" />
      </details>
    </NodeViewWrapper>
  );
}

function DataviewResultView({
  query,
  result,
  onOpenLink,
  t,
}: {
  query: string;
  result?: DataviewResult;
  onOpenLink?: (name: string) => void;
  t: (key: string, vars?: Record<string, string>) => string;
}) {
  const firstLine = query.split(/\n/).find(Boolean) ?? "dataview";
  const error = result?.error;
  const knownErrors = new Set([
    "desktop_required",
    "sort_field_required",
    "table_columns_required",
    "unsupported_query",
    "where_comparison_required",
  ]);
  return (
    <>
      <div className="mb-2 flex items-center justify-between gap-3">
        <span className="font-mono text-[11px] uppercase tracking-wide text-magma-muted">
          Dataview
        </span>
        <span className="truncate font-mono text-[11px] text-magma-muted">{firstLine}</span>
      </div>
      {!result ? (
        <p className="text-sm text-magma-muted">{t("dataview.loading")}</p>
      ) : result.error ? (
        <p className="text-sm text-red-600 dark:text-red-300">
          {error && knownErrors.has(error) ? t(`dataview.error.${error}`) : t("dataview.error.unknown")}
        </p>
      ) : result.kind === "table" ? (
        <div className="overflow-x-auto">
          <table>
            <thead>
              <tr>
                {result.columns.map((column) => (
                  <th key={column}>{column}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {result.rows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {row.map((cell, cellIndex) => (
                    <td key={cellIndex}>
                      <DataviewCell value={cell} onOpenLink={onOpenLink} />
                    </td>
                  ))}
                </tr>
              ))}
              {result.rows.length === 0 && (
                <tr>
                  <td colSpan={Math.max(1, result.columns.length)} className="text-magma-muted">
                    {t("dataview.empty")}
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      ) : (
        <ul>
          {result.items.map((item, index) => (
            <li key={`${item}:${index}`}>
              <DataviewCell value={item} onOpenLink={onOpenLink} />
            </li>
          ))}
          {result.items.length === 0 && <li className="text-magma-muted">{t("dataview.empty")}</li>}
        </ul>
      )}
    </>
  );
}

function DataviewCell({
  value,
  onOpenLink,
}: {
  value: string;
  onOpenLink?: (name: string) => void;
}) {
  const link = value.match(/^\[\[([^|\]]+)(?:\|([^\]]+))?\]\]$/);
  if (!link) return <>{value}</>;
  const target = link[1].trim();
  const label = (link[2] ?? target).trim();
  return (
    <button
      type="button"
      onClick={() => onOpenLink?.(target)}
      className="text-left text-magma-accent hover:underline"
    >
      {label}
    </button>
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
      {/* H3 was configured in the editor and styled in the stylesheet, but had
          no button — reachable only by typing "### ". */}
      <button
        className={btn(editor.isActive("heading", { level: 3 }))}
        onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()}
        title="Heading 3"
      >
        H3
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
