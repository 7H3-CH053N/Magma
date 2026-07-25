// Renders `[[wikilinks]]` inside the WYSIWYG editor as styled, clickable links
// without turning them into a custom node — the text stays literal `[[Name]]`
// in the document, so markdown round-trips perfectly. A ProseMirror decoration
// colors them; a click handler opens the target note.
import { Extension } from "@tiptap/core";
import { Plugin, PluginKey, TextSelection } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

const WIKI = /\[\[([^\]\n]+)\]\]/g;

export interface WikiLinkOptions {
  onOpen: (name: string) => void;
  /** Whether a note of that name exists — drives the "missing" styling. */
  exists: (name: string) => boolean;
}

export const WikiLink = Extension.create<WikiLinkOptions>({
  name: "wikilink",

  addOptions() {
    return { onOpen: () => {}, exists: () => true };
  },

  addProseMirrorPlugins() {
    const onOpen = this.options.onOpen;
    const exists = this.options.exists;
    return [
      new Plugin({
        key: new PluginKey("wikilink"),
        props: {
          decorations(state) {
            const decos: Decoration[] = [];
            const sel = state.selection.from;
            state.doc.descendants((node, pos) => {
              if (!node.isText || !node.text) return;
              WIKI.lastIndex = 0;
              let m: RegExpExecArray | null;
              while ((m = WIKI.exec(node.text))) {
                const from = pos + m.index;
                const to = from + m[0].length;
                const inner = m[1];
                const name = inner.split("|")[0].split(/[#^]/)[0].trim();
                const cls = exists(name) ? "wikilink" : "wikilink wikilink-missing";
                decos.push(
                  Decoration.inline(from, to, { class: cls, "data-name": name })
                );

                // Show only the readable part: `[[Birgit Januschewsky|Birgit]]`
                // reads as "Birgit". The text stays literal in the document, so
                // markdown round-trips — the brackets are only hidden visually,
                // and reappear while the caret is inside so it stays editable.
                // Strictly inside: sitting right before or after a link must
                // not unmask it, or the last link in a note is always raw.
                const caretInside = sel > from && sel < to;
                if (!caretInside) {
                  // A pencil right after the link: clicking the link itself
                  // opens the note, so without this there is no way to put the
                  // caret in and change the target.
                  decos.push(
                    Decoration.widget(
                      to,
                      () => {
                        const el = document.createElement("span");
                        el.className = "wikilink-edit";
                        el.setAttribute("data-edit-pos", String(from + 2));
                        el.title = "Link bearbeiten";
                        el.textContent = "✎";
                        return el;
                      },
                      { side: 1 }
                    )
                  );
                  const pipe = inner.indexOf("|");
                  // Hide "[[" plus, for an aliased link, "Target|".
                  const headEnd = from + 2 + (pipe >= 0 ? pipe + 1 : 0);
                  decos.push(
                    Decoration.inline(from, headEnd, { class: "wikilink-syntax" })
                  );
                  decos.push(
                    Decoration.inline(to - 2, to, { class: "wikilink-syntax" })
                  );
                }
              }
            });
            return DecorationSet.create(state.doc, decos);
          },
          handleClick(view, _pos, event) {
            const el = event.target as HTMLElement | null;
            // The pencil: drop the caret inside the link, which also unmasks
            // the raw [[…]] so it can be edited.
            if (el && el.classList.contains("wikilink-edit")) {
              const at = Number(el.getAttribute("data-edit-pos"));
              if (Number.isFinite(at)) {
                const tr = view.state.tr.setSelection(
                  TextSelection.create(view.state.doc, at)
                );
                view.dispatch(tr.scrollIntoView());
                view.focus();
              }
              return true;
            }
            // Never swallow a selection: dragging across a link must select it.
            if (!view.state.selection.empty) return false;
            if (el && el.classList.contains("wikilink")) {
              const name = el.getAttribute("data-name");
              if (name) {
                onOpen(name);
                return true;
              }
            }
            return false;
          },
        },
      }),
    ];
  },
});
