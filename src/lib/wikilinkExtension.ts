// Renders `[[wikilinks]]` inside the WYSIWYG editor as styled, clickable links
// without turning them into a custom node — the text stays literal `[[Name]]`
// in the document, so markdown round-trips perfectly. A ProseMirror decoration
// colors them; a click handler opens the target note.
import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

const WIKI = /\[\[([^\]\n]+)\]\]/g;

export interface WikiLinkOptions {
  onOpen: (name: string) => void;
}

export const WikiLink = Extension.create<WikiLinkOptions>({
  name: "wikilink",

  addOptions() {
    return { onOpen: () => {} };
  },

  addProseMirrorPlugins() {
    const onOpen = this.options.onOpen;
    return [
      new Plugin({
        key: new PluginKey("wikilink"),
        props: {
          decorations(state) {
            const decos: Decoration[] = [];
            state.doc.descendants((node, pos) => {
              if (!node.isText || !node.text) return;
              WIKI.lastIndex = 0;
              let m: RegExpExecArray | null;
              while ((m = WIKI.exec(node.text))) {
                const from = pos + m.index;
                const to = from + m[0].length;
                const name = m[1].split("|")[0].split(/[#^]/)[0].trim();
                decos.push(
                  Decoration.inline(from, to, {
                    class: "wikilink",
                    "data-name": name,
                  })
                );
              }
            });
            return DecorationSet.create(state.doc, decos);
          },
          handleClick(_view, _pos, event) {
            const el = event.target as HTMLElement | null;
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
