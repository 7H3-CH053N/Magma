import { useEffect, useMemo, useRef, useState } from "react";
import type { NoteMeta } from "../lib/api";
import { useI18n } from "../lib/i18n";

export interface Command {
  id: string;
  label: string;
  /** Shown right-aligned: a shortcut, or the folder a note sits in. */
  hint?: string;
  run: () => void;
}

interface CommandPaletteProps {
  notes: NoteMeta[];
  commands: Command[];
  onOpenNote: (path: string) => void;
  onClose: () => void;
}

/**
 * One field for everything: jump to a note, or run a command.
 *
 * Matching is subsequence-based ("grph" finds "Graph anzeigen"), which is what
 * makes a palette worth reaching for — you type the letters you remember, not
 * the string you'd have to look up.
 */
export default function CommandPalette({
  notes,
  commands,
  onOpenNote,
  onClose,
}: CommandPaletteProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  const items = useMemo(() => {
    const q = query.trim();
    const noteItems: Command[] = notes.map((n) => ({
      id: `note:${n.path}`,
      label: n.title,
      hint: n.path.includes("/") ? n.path.slice(0, n.path.lastIndexOf("/")) : undefined,
      run: () => onOpenNote(n.path),
    }));
    // Commands first when the field is empty — an empty palette should show
    // what Magma can do, not the first 200 notes in alphabetical order.
    const all = q ? [...commands, ...noteItems] : commands;
    const scored = all
      // The hint is searchable too: it holds a note's folder and a template's
      // "Template" tag, so typing either finds the row you meant.
      .map((item) => ({ item, score: score(`${item.label} ${item.hint ?? ""}`, q) }))
      .filter((r) => r.score > -Infinity);
    scored.sort((a, b) => b.score - a.score);
    return scored.slice(0, 40).map((r) => r.item);
  }, [query, notes, commands, onOpenNote]);

  useEffect(() => setActive(0), [query]);

  // Keep the highlighted row in view when arrowing past the fold.
  useEffect(() => {
    const el = listRef.current?.children[active] as HTMLElement | undefined;
    el?.scrollIntoView({ block: "nearest" });
  }, [active]);

  const choose = (i: number) => {
    const item = items[i];
    if (!item) return;
    onClose();
    item.run();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 p-4 pt-[12vh]"
      onClick={onClose}
    >
      <div
        className="w-full max-w-xl overflow-hidden rounded-2xl bg-magma-bg shadow-2xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "ArrowDown") {
              e.preventDefault();
              setActive((a) => Math.min(a + 1, items.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setActive((a) => Math.max(a - 1, 0));
            } else if (e.key === "Enter") {
              e.preventDefault();
              choose(active);
            } else if (e.key === "Escape") {
              onClose();
            }
          }}
          placeholder={t("palette.placeholder")}
          className="w-full border-b border-black/10 bg-transparent px-5 py-4 text-base outline-none placeholder:text-magma-muted dark:border-white/10"
        />
        <div ref={listRef} className="max-h-[50vh] overflow-auto py-1.5">
          {items.length === 0 && (
            <p className="px-5 py-6 text-center text-sm text-magma-muted">
              {t("palette.noMatches")}
            </p>
          )}
          {items.map((item, i) => (
            <button
              key={item.id}
              onMouseEnter={() => setActive(i)}
              onClick={() => choose(i)}
              className={`flex w-full items-center gap-3 px-5 py-2 text-left text-sm transition ${
                i === active ? "bg-magma-accent/12" : ""
              }`}
            >
              <span className="min-w-0 flex-1 truncate">{item.label}</span>
              {item.hint && (
                <span className="shrink-0 text-xs text-magma-muted">{item.hint}</span>
              )}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

/**
 * Subsequence match with a bonus for matches at word starts, so "ng" ranks
 * "Neue Notiz" (n-eue **g**…) below "Notiz aus Vorlage" only when it really
 * fits better. -Infinity means "no match at all".
 */
function score(label: string, query: string): number {
  if (!query) return 0;
  const l = label.toLowerCase();
  const q = query.toLowerCase();
  // A straight substring hit always beats a scattered one.
  const direct = l.indexOf(q);
  if (direct >= 0) return 1000 - direct;

  let li = 0;
  let points = 0;
  for (const ch of q) {
    const at = l.indexOf(ch, li);
    if (at < 0) return -Infinity;
    // Start of a word is what the user most likely typed.
    if (at === 0 || l[at - 1] === " " || l[at - 1] === "/" || l[at - 1] === "-") points += 6;
    points += at === li ? 3 : 1;
    li = at + 1;
  }
  return points;
}
