import { useEffect, useState } from "react";
import type { NoteMeta } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { ChevronIcon } from "./Icons";

interface BacklinksPanelProps {
  backlinks: NoteMeta[];
  onSelect: (path: string) => void;
}

/** Collapse by default past this many — a hub note can have hundreds. */
const MANY = 12;

/**
 * Quiet panel under the editor: which notes point here. It never takes more
 * than a third of the height and scrolls internally, so a hub note with 500
 * backlinks still leaves the note itself readable.
 */
export default function BacklinksPanel({ backlinks, onSelect }: BacklinksPanelProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(true);

  // Re-decide whenever the note changes: short lists stay open, long ones fold.
  useEffect(() => {
    setOpen(backlinks.length <= MANY);
  }, [backlinks]);

  if (backlinks.length === 0) return null;
  return (
    <div className="shrink-0 border-t border-black/5 dark:border-white/10">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-1.5 px-8 py-2 text-left text-xs font-medium uppercase tracking-wide text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/5"
      >
        <ChevronIcon size={12} open={open} className="shrink-0" />
        <span>
          {backlinks.length}{" "}
          {backlinks.length === 1 ? t("backlinks.one") : t("backlinks.many")}
        </span>
      </button>
      {open && (
        <div className="max-h-[33vh] overflow-auto px-8 pb-3">
          <div className="flex flex-wrap gap-2">
            {backlinks.map((b) => (
              <button
                key={b.path}
                onClick={() => onSelect(b.path)}
                className="rounded-md bg-black/5 px-2 py-1 text-sm text-magma-ink transition hover:bg-black/10 dark:bg-white/10 dark:text-[#ece9e4] dark:hover:bg-white/20"
              >
                {b.title}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
