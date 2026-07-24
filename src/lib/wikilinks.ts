// Wikilink support for the editor: autocomplete note titles inside `[[ … ]]`
// and open a linked note on click. Both read the live list of note titles via
// a getter so the editor never has to be rebuilt when the vault changes.
import {
  autocompletion,
  type CompletionContext,
  type CompletionResult,
} from "@codemirror/autocomplete";
import { EditorView } from "@codemirror/view";

/** Autocomplete `[[` with the vault's note titles. */
export function wikilinkCompletion(getTitles: () => string[]) {
  return autocompletion({
    override: [
      (ctx: CompletionContext): CompletionResult | null => {
        // Match an open "[[" up to the cursor, without a closing "]]" yet.
        const before = ctx.matchBefore(/\[\[([^\]\n]*)$/);
        if (!before) return null;
        const typed = before.text.slice(2).toLowerCase();
        const options = getTitles()
          .filter((t) => t.toLowerCase().includes(typed))
          .slice(0, 50)
          .map((t) => ({ label: t, type: "text", apply: `${t}]]` }));
        if (options.length === 0) return null;
        return { from: before.from + 2, options, filter: false };
      },
    ],
  });
}

/** Find the wikilink target under a document position, if any. */
export function wikilinkAt(doc: string, pos: number): string | null {
  const open = doc.lastIndexOf("[[", pos);
  if (open === -1) return null;
  const close = doc.indexOf("]]", open);
  if (close === -1 || pos > close + 2) return null;
  // Ensure no newline breaks the link between open and close.
  const inner = doc.slice(open + 2, close);
  if (inner.includes("\n")) return null;
  const target = inner.split("|")[0].split(/[#^]/)[0].trim();
  return target || null;
}

/** Cmd/Ctrl-click a wikilink to open the target note. */
export function wikilinkNavigation(onOpen: (title: string) => void) {
  return EditorView.domEventHandlers({
    mousedown(event, view) {
      if (!(event.metaKey || event.ctrlKey)) return false;
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
      if (pos == null) return false;
      const target = wikilinkAt(view.state.doc.toString(), pos);
      if (target) {
        event.preventDefault();
        onOpen(target);
        return true;
      }
      return false;
    },
  });
}
