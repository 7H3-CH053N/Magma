import { useEffect } from "react";
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
}

/**
 * True WYSIWYG editor (TipTap / ProseMirror). Formatting is live — you never
 * see raw markdown. Blocks are created the Notion way, by typing shortcuts
 * (`# `, `- `, `[] `, `> `, ``` ``` ), and inline styling via the floating
 * toolbar or `Cmd/Ctrl+B` etc. Content is stored as plain markdown so the vault
 * stays portable.
 */
export default function Editor({ value, onChange, placeholder, onOpenLink, onOpenExternal }: EditorProps) {
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
      WikiLink.configure({ onOpen: (name) => onOpenLink?.(name) }),
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

  if (!editor) return <div className="h-full w-full" />;

  return (
    <div className="h-full w-full overflow-auto">
      <BubbleMenu editor={editor} tippyOptions={{ duration: 120 }}>
        <Toolbar editor={editor} />
      </BubbleMenu>
      <EditorContent editor={editor} className="mx-auto max-w-[var(--magma-reading-width)] px-8 py-6" />
    </div>
  );
}

function Toolbar({ editor }: { editor: TiptapEditor }) {
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
