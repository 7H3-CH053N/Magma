// Live-markdown decorations: the syntax fades away as you type. Formatting
// marks (**, *, #, `, and link brackets/URLs) are hidden with replace
// decorations *except* on the line the cursor sits in — so the text reads like
// a rendered page while staying fully editable the moment you move into it.
import { syntaxTree } from "@codemirror/language";
import type { Range } from "@codemirror/state";
import {
  Decoration,
  type DecorationSet,
  EditorView,
  ViewPlugin,
  type ViewUpdate,
  WidgetType,
} from "@codemirror/view";

const HEADING_CLASS: Record<string, string> = {
  ATXHeading1: "cm-h1",
  ATXHeading2: "cm-h2",
  ATXHeading3: "cm-h3",
  ATXHeading4: "cm-h4",
  ATXHeading5: "cm-h5",
  ATXHeading6: "cm-h6",
};

const hidden = Decoration.replace({});

class BulletWidget extends WidgetType {
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-bullet";
    span.textContent = "•";
    return span;
  }
}
const bullet = Decoration.replace({ widget: new BulletWidget() });

class WikiWidget extends WidgetType {
  constructor(readonly title: string) {
    super();
  }
  eq(other: WikiWidget) {
    return other.title === this.title;
  }
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-wikilink";
    span.textContent = this.title;
    return span;
  }
}

/** All `[[wikilink]]` spans within [from,to), as absolute doc offsets. */
function wikilinkRanges(text: string, offset: number) {
  const out: { from: number; to: number; title: string }[] = [];
  const re = /\[\[([^\]\n]+)\]\]/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text))) {
    const shown = m[1].split("|").pop()!.trim();
    out.push({ from: offset + m.index, to: offset + m.index + m[0].length, title: shown });
  }
  return out;
}

function activeLines(view: EditorView): Set<number> {
  const lines = new Set<number>();
  for (const r of view.state.selection.ranges) {
    const from = view.state.doc.lineAt(r.from).number;
    const to = view.state.doc.lineAt(r.to).number;
    for (let n = from; n <= to; n++) lines.add(n);
  }
  return lines;
}

function buildDecorations(view: EditorView): DecorationSet {
  const marks: Range<Decoration>[] = [];
  const active = activeLines(view);
  const doc = view.state.doc;

  // Wikilinks first: compute their spans so we can (a) render them cleanly and
  // (b) tell the lezer pass to leave link marks inside them alone (avoids
  // overlapping replace decorations).
  const wikis: { from: number; to: number; title: string }[] = [];
  for (const { from, to } of view.visibleRanges) {
    wikis.push(...wikilinkRanges(doc.sliceString(from, to), from));
  }
  const insideWiki = (pos: number) => wikis.some((w) => pos >= w.from && pos < w.to);
  for (const w of wikis) {
    const lineNo = doc.lineAt(w.from).number;
    if (active.has(lineNo)) {
      marks.push(Decoration.mark({ class: "cm-wikilink" }).range(w.from, w.to));
    } else {
      marks.push(
        Decoration.replace({ widget: new WikiWidget(w.title) }).range(w.from, w.to)
      );
    }
  }

  for (const { from, to } of view.visibleRanges) {
    syntaxTree(view.state).iterate({
      from,
      to,
      enter: (node) => {
        const lineNo = doc.lineAt(node.from).number;
        const isActive = active.has(lineNo);
        if (insideWiki(node.from)) return; // handled by the wikilink pass

        // Heading styling applies to the whole line.
        const hClass = HEADING_CLASS[node.name];
        if (hClass) {
          marks.push(Decoration.line({ class: hClass }).range(node.from));
        }

        switch (node.name) {
          case "StrongEmphasis":
            marks.push(Decoration.mark({ class: "cm-strong" }).range(node.from, node.to));
            break;
          case "Emphasis":
            marks.push(Decoration.mark({ class: "cm-em" }).range(node.from, node.to));
            break;
          case "InlineCode":
            marks.push(Decoration.mark({ class: "cm-code" }).range(node.from, node.to));
            break;
          case "Blockquote":
            marks.push(Decoration.mark({ class: "cm-quote" }).range(node.from, node.to));
            break;
        }

        if (isActive) return; // leave marks visible where the cursor is

        switch (node.name) {
          case "HeaderMark": {
            // Hide the "# " including the trailing space.
            let end = node.to;
            if (doc.sliceString(end, end + 1) === " ") end += 1;
            marks.push(hidden.range(node.from, end));
            break;
          }
          case "EmphasisMark":
          case "CodeMark":
          case "LinkMark":
            if (node.to > node.from) marks.push(hidden.range(node.from, node.to));
            break;
          case "QuoteMark": {
            // Hide "> " including a trailing space.
            let end = node.to;
            if (doc.sliceString(end, end + 1) === " ") end += 1;
            marks.push(hidden.range(node.from, end));
            break;
          }
          case "ListMark": {
            // Render "-", "*", "+" bullets as a real bullet glyph.
            const ch = doc.sliceString(node.from, node.to);
            if (ch === "-" || ch === "*" || ch === "+") {
              marks.push(bullet.range(node.from, node.to));
            }
            break;
          }
        }
      },
    });
  }
  // `true` sorts the ranges, sparing us the strict-ordering rules of a builder.
  return Decoration.set(marks, true);
}

export const liveMarkdown = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = buildDecorations(view);
    }
    update(u: ViewUpdate) {
      if (u.docChanged || u.selectionSet || u.viewportChanged) {
        this.decorations = buildDecorations(u.view);
      }
    }
  },
  { decorations: (v) => v.decorations }
);

// Visual styling for the rendered marks. Kept with the extension so the
// look and the logic travel together.
export const liveMarkdownTheme = EditorView.theme({
  // Read like a page, not a code editor. Fonts/colors come from theme vars so
  // Settings can customize them live (see src/lib/theme.tsx).
  "&": { fontSize: "var(--magma-font-size)", height: "100%" },
  ".cm-scroller": { fontFamily: "var(--magma-font-editor)", lineHeight: "1.7" },
  ".cm-content": { caretColor: "var(--magma-accent)", paddingBottom: "40vh" },
  ".cm-cursor": { borderLeftColor: "var(--magma-accent)" },
  ".cm-line": { padding: "1px 0" },
  ".cm-h1": { fontSize: "1.9em", fontWeight: "700", lineHeight: "1.3" },
  ".cm-h2": { fontSize: "1.45em", fontWeight: "700", lineHeight: "1.3" },
  ".cm-h3": { fontSize: "1.2em", fontWeight: "600" },
  ".cm-h4, .cm-h5, .cm-h6": { fontWeight: "600" },
  ".cm-strong": { fontWeight: "700" },
  ".cm-em": { fontStyle: "italic" },
  ".cm-code": {
    fontFamily: "var(--magma-font-mono)",
    fontSize: "0.9em",
    background: "rgba(125,125,125,0.15)",
    borderRadius: "4px",
    padding: "0.05em 0.3em",
  },
  ".cm-bullet": { color: "var(--magma-accent)", paddingRight: "0.4em" },
  ".cm-quote": {
    fontStyle: "italic",
    color: "var(--magma-muted)",
    borderLeft: "3px solid color-mix(in srgb, var(--magma-accent) 45%, transparent)",
    paddingLeft: "0.8em",
  },
  ".cm-wikilink": {
    color: "var(--magma-accent)",
    cursor: "pointer",
    textDecoration: "none",
  },
});
