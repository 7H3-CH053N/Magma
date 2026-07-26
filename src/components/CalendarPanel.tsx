import { useMemo, useState } from "react";
import type { NoteMeta } from "../lib/api";
import { dayKey } from "../lib/prefs";
import { useI18n } from "../lib/i18n";
import { ChevronIcon } from "./Icons";

interface CalendarPanelProps {
  notes: NoteMeta[];
  /** Folder daily notes live in, to know which notes are days. */
  folder: string;
  onOpenDay: (date: Date) => void;
}

/**
 * A month at a glance in the sidebar. Days that already have a note are
 * filled; clicking any day opens that day's note, creating it if needed —
 * which is the whole point: the calendar is the way in, not a report.
 */
export default function CalendarPanel({ notes, folder, onOpenDay }: CalendarPanelProps) {
  const { t, lang } = useI18n();
  const [cursor, setCursor] = useState(() => new Date());
  const [open, setOpen] = useState(false);

  // Which days already exist, by their YYYY-MM-DD name.
  const existing = useMemo(() => {
    const prefix = folder ? `${folder}/` : "";
    const days = new Set<string>();
    for (const n of notes) {
      if (folder && !n.path.startsWith(prefix)) continue;
      const stem = (n.path.split("/").pop() ?? "").replace(/\.md$/i, "");
      if (/^\d{4}-\d{2}-\d{2}$/.test(stem)) days.add(stem);
    }
    return days;
  }, [notes, folder]);

  const locale = lang === "de" ? "de-DE" : "en-GB";
  const year = cursor.getFullYear();
  const month = cursor.getMonth();
  const today = dayKey(new Date());

  // Monday-first grid: JS getDay() is Sunday-first, hence the +6 %7.
  const first = new Date(year, month, 1);
  const lead = (first.getDay() + 6) % 7;
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const cells: (number | null)[] = [
    ...Array.from({ length: lead }, () => null),
    ...Array.from({ length: daysInMonth }, (_, i) => i + 1),
  ];

  const step = (delta: number) => setCursor(new Date(year, month + delta, 1));

  return (
    <div className="border-t border-black/5 px-3 py-2 dark:border-white/5">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-1.5 rounded-md px-1 py-1 text-xs font-medium uppercase tracking-wide text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
      >
        <ChevronIcon size={12} open={open} className="shrink-0" />
        <span>{t("calendar.title")}</span>
      </button>

      {open && (
        <>
          <div className="mb-1 mt-1.5 flex items-center gap-1">
            <button
              onClick={() => step(-1)}
              aria-label={t("calendar.prev")}
              className="grid h-6 w-6 place-items-center rounded text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
            >
              <ChevronIcon size={12} className="rotate-180" />
            </button>
            <span className="flex-1 text-center text-xs font-medium">
              {first.toLocaleDateString(locale, { month: "long", year: "numeric" })}
            </span>
            <button
              onClick={() => step(1)}
              aria-label={t("calendar.next")}
              className="grid h-6 w-6 place-items-center rounded text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
            >
              <ChevronIcon size={12} />
            </button>
          </div>

          <div className="grid grid-cols-7 gap-0.5 text-center">
            {weekdayInitials(locale).map((d, i) => (
              <span key={i} className="py-0.5 text-[10px] text-magma-muted">
                {d}
              </span>
            ))}
            {cells.map((day, i) => {
              if (day === null) return <span key={`x${i}`} />;
              const date = new Date(year, month, day);
              const key = dayKey(date);
              const has = existing.has(key);
              const isToday = key === today;
              return (
                <button
                  key={key}
                  onClick={() => onOpenDay(date)}
                  title={date.toLocaleDateString(locale, { dateStyle: "full" })}
                  className={`aspect-square rounded text-[11px] leading-none transition ${
                    has
                      ? "bg-magma-accent/20 font-medium text-magma-accent"
                      : "text-magma-muted hover:bg-black/5 dark:hover:bg-white/10"
                  } ${isToday ? "ring-1 ring-magma-accent" : ""}`}
                >
                  {day}
                </button>
              );
            })}
          </div>
        </>
      )}
    </div>
  );
}

/** One-letter weekday headers, Monday first, in the user's language. */
function weekdayInitials(locale: string): string[] {
  // 2024-01-01 was a Monday — a fixed anchor beats hardcoding names per language.
  return Array.from({ length: 7 }, (_, i) =>
    new Date(2024, 0, 1 + i).toLocaleDateString(locale, { weekday: "short" }).slice(0, 2)
  );
}
