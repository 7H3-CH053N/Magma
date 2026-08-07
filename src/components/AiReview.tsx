import { useMemo } from "react";
import type { NoteMeta } from "../lib/api";
import { useI18n } from "../lib/i18n";

interface AiReviewProps {
  notes: NoteMeta[];
  onSelect: (path: string) => void;
}

/**
 * What Claude wrote, newest first.
 *
 * Letting an LLM write into your vault is only comfortable if you can see
 * exactly what it touched — this is that page. Notes carry `author: ai` in
 * their frontmatter, so the list is derived from the files themselves and
 * stays right even if the notes were written while Magma was closed.
 */
export default function AiReview({ notes, onSelect }: AiReviewProps) {
  const { t, lang } = useI18n();
  const written = useMemo(
    () => notes.filter((n) => n.aiAuthored).sort((a, b) => b.modified - a.modified),
    [notes]
  );

  if (written.length === 0) {
    return (
      <div className="grid flex-1 place-items-center p-8 text-center">
        <div className="max-w-sm">
          <p className="text-sm font-medium">{t("ai.emptyTitle")}</p>
          <p className="mt-1.5 text-sm leading-relaxed text-magma-muted">
            {t("ai.emptyBody")}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-auto p-8">
      <div className="mx-auto max-w-2xl">
        <h1 className="text-lg font-semibold tracking-tight">{t("ai.title")}</h1>
        <p className="mt-1 text-sm text-magma-muted">
          {t("ai.subtitle", { count: String(written.length) })}
        </p>
        <ul className="mt-5 space-y-1">
          {written.map((n) => (
            <li key={n.path}>
              <button
                onClick={() => onSelect(n.path)}
                className="flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition hover:bg-black/5 dark:hover:bg-white/10"
              >
                <span
                  className="h-2 w-2 shrink-0 rounded-full"
                  style={{ background: "var(--magma-ai)" }}
                  aria-hidden
                />
                <span className="min-w-0 flex-1">
                  <span className="flex min-w-0 items-center gap-2">
                    <span className="block truncate text-sm font-medium">{n.title}</span>
                    <span className="shrink-0 rounded border border-black/10 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-magma-muted dark:border-white/10">
                      {clientLabel(n.aiClient)}
                    </span>
                  </span>
                  <span className="block truncate text-xs text-magma-muted">{n.path}</span>
                </span>
                <span className="shrink-0 text-xs text-magma-muted">
                  {n.modified ? formatWhen(n.modified, lang) : ""}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}

function clientLabel(client?: string | null): string {
  if (!client) return "AI";
  if (client.toLowerCase() === "codex") return "Codex";
  if (client.toLowerCase() === "claude") return "Claude";
  return client;
}

function formatWhen(ms: number, lang: string): string {
  return new Date(ms).toLocaleString(lang === "de" ? "de-DE" : "en-GB", {
    day: "2-digit",
    month: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}
