import { useEffect, useRef } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import { markdown } from "@codemirror/lang-markdown";
import { liveMarkdown, liveMarkdownTheme } from "../lib/liveMarkdown";

interface EditorProps {
  value: string;
  onChange: (value: string) => void;
  /** Called when an image is pasted; returns markdown to insert, or null. */
  onPasteImage?: (file: File) => Promise<string | null>;
}

/**
 * Live-markdown editor. One mode only — no edit/preview toggle. The
 * `liveMarkdown` extension hides syntax as you type, so it reads like a clean
 * page while staying plain text underneath.
 */
export default function Editor({ value, onChange, onPasteImage }: EditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  const onPasteRef = useRef(onPasteImage);
  onChangeRef.current = onChange;
  onPasteRef.current = onPasteImage;

  useEffect(() => {
    if (!host.current) return;

    const pasteHandler = EditorView.domEventHandlers({
      paste(event, v) {
        const items = event.clipboardData?.items;
        if (!items || !onPasteRef.current) return false;
        for (const item of items) {
          if (item.type.startsWith("image/")) {
            const file = item.getAsFile();
            if (!file) continue;
            event.preventDefault();
            void onPasteRef.current(file).then((md) => {
              if (md) {
                const pos = v.state.selection.main.from;
                v.dispatch({
                  changes: { from: pos, insert: md },
                  selection: { anchor: pos + md.length },
                });
              }
            });
            return true;
          }
        }
        return false;
      },
    });

    const state = EditorState.create({
      doc: value,
      extensions: [
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        markdown(),
        liveMarkdown,
        liveMarkdownTheme,
        EditorView.lineWrapping,
        pasteHandler,
        EditorView.updateListener.of((u) => {
          if (u.docChanged) onChangeRef.current(u.state.doc.toString());
        }),
      ],
    });

    const v = new EditorView({ state, parent: host.current });
    view.current = v;
    return () => v.destroy();
    // Editor is remounted per note via a `key` prop on the parent.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const v = view.current;
    if (v && value !== v.state.doc.toString()) {
      v.dispatch({
        changes: { from: 0, to: v.state.doc.length, insert: value },
      });
    }
  }, [value]);

  return <div ref={host} className="h-full w-full overflow-auto" />;
}
