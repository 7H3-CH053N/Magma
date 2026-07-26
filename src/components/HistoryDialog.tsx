import { useCallback, useEffect, useState } from "react";
import { listVersions, readVersion, restoreVersion, type Version } from "../lib/api";
import { useI18n } from "../lib/i18n";

interface HistoryDialogProps {
  vault: string;
  path: string;
  title: string;
  /** The text as it stands right now, to diff each version against. */
  current: string;
  onClose: () => void;
  /** Called after a restore so the editor reloads from disk. */
  onRestored: () => void;
}

/**
 * A note's earlier versions, with the change highlighted.
 *
 * Restoring is offered without a confirmation on purpose: restoring itself
 * snapshots what it replaces, so the way back from a wrong click is one more
 * click in the same list.
 */
export default function HistoryDialog({
  vault,
  path,
  title,
  current,
  onClose,
  onRestored,
}: HistoryDialogProps) {
  const { t, lang } = useI18n();
  const [versions, setVersions] = useState<Version[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [text, setText] = useState<string>("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listVersions(vault, path)
      .then((v) => {
        setVersions(v);
        if (v[0]) setSelected(v[0].id);
      })
      .catch((e) => setError(String(e)));
  }, [vault, path]);

  useEffect(() => {
    if (!selected) return;
    readVersion(vault, path, selected)
      .then(setText)
      .catch((e) => setError(String(e)));
  }, [vault, path, selected]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const restore = useCallback(async () => {
    if (!selected) return;
    setBusy(true);
    try {
      await restoreVersion(vault, path, selected);
      onRestored();
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }, [vault, path, selected, onRestored, onClose]);

  const rows = diffLines(text, current);

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4" onClick={onClose}>
      <div
        className="flex h-[min(85vh,40rem)] w-full max-w-4xl overflow-hidden rounded-2xl bg-magma-bg shadow-xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <nav className="flex w-56 shrink-0 flex-col border-r border-black/5 bg-black/[0.02] dark:border-white/5 dark:bg-white/[0.03]">
          <p className="truncate px-4 pb-1 pt-3 text-sm font-semibold" title={title}>
            {t("history.title")}
          </p>
          <p className="truncate px-4 pb-2 text-xs text-magma-muted">{title}</p>
          <div className="flex-1 overflow-auto px-2 pb-2">
            {versions === null && (
              <p className="px-2 py-2 text-xs text-magma-muted">{t("history.loading")}</p>
            )}
            {versions?.length === 0 && (
              <p className="px-2 py-2 text-xs leading-relaxed text-magma-muted">
                {t("history.none")}
              </p>
            )}
            {versions?.map((v) => (
              <button
                key={v.id}
                onClick={() => setSelected(v.id)}
                className={`block w-full rounded-md px-2 py-1.5 text-left text-xs transition ${
                  selected === v.id
                    ? "bg-magma-accent/12 text-magma-accent"
                    : "text-magma-muted hover:bg-black/5 dark:hover:bg-white/10"
                }`}
              >
                <span className="block">{formatWhen(v.takenAt, lang)}</span>
                <span className="block opacity-70">{formatBytes(v.bytes)}</span>
              </button>
            ))}
          </div>
        </nav>

        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex-1 overflow-auto p-5">
            {error && <p className="text-sm text-red-500">{error}</p>}
            {!error && versions?.length === 0 && (
              <p className="text-sm text-magma-muted">{t("history.noneBody")}</p>
            )}
            {!error && !!versions?.length && (
              <>
                <p className="mb-3 text-xs text-magma-muted">{t("history.diffHint")}</p>
                <pre className="overflow-x-auto rounded-lg bg-black/[0.03] p-3 text-xs leading-relaxed dark:bg-black/30">
                  {rows.map((row, i) => (
                    <div
                      key={i}
                      className={
                        row.kind === "add"
                          ? "bg-green-500/15 text-green-700 dark:text-green-300"
                          : row.kind === "del"
                            ? "bg-red-500/15 text-red-700 dark:text-red-300"
                            : "text-magma-muted"
                      }
                    >
                      <span className="select-none opacity-60">
                        {row.kind === "add" ? "+ " : row.kind === "del" ? "− " : "  "}
                      </span>
                      {row.text || " "}
                    </div>
                  ))}
                </pre>
              </>
            )}
          </div>

          <footer className="flex items-center justify-end gap-2 border-t border-black/5 px-5 py-3 dark:border-white/5">
            <button
              onClick={onClose}
              className="rounded-lg px-4 py-1.5 text-sm text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
            >
              {t("history.close")}
            </button>
            <button
              onClick={restore}
              disabled={!selected || busy}
              className="whitespace-nowrap rounded-lg bg-magma-accent px-4 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40"
            >
              {t("history.restore")}
            </button>
          </footer>
        </div>
      </div>
    </div>
  );
}

interface Row {
  kind: "same" | "add" | "del";
  text: string;
}

/**
 * Line diff via the classic longest-common-subsequence table.
 *
 * Notes are short enough (a few hundred lines) that the O(n·m) table costs
 * nothing, and it gives a *minimal* diff — a heuristic would mark a whole
 * paragraph changed when one word moved.
 */
export function diffLines(before: string, after: string): Row[] {
  const a = before.split("\n");
  const b = after.split("\n");
  const n = a.length;
  const m = b.length;
  const lcs: number[][] = Array.from({ length: n + 1 }, () => new Array(m + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = m - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const rows: Row[] = [];
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (a[i] === b[j]) {
      rows.push({ kind: "same", text: a[i] });
      i++;
      j++;
    } else if (lcs[i + 1][j] >= lcs[i][j + 1]) {
      rows.push({ kind: "del", text: a[i] });
      i++;
    } else {
      rows.push({ kind: "add", text: b[j] });
      j++;
    }
  }
  while (i < n) rows.push({ kind: "del", text: a[i++] });
  while (j < m) rows.push({ kind: "add", text: b[j++] });
  return rows;
}

function formatWhen(ms: number, lang: string): string {
  const d = new Date(ms);
  return d.toLocaleString(lang === "de" ? "de-DE" : "en-GB", {
    day: "2-digit",
    month: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatBytes(b: number): string {
  return b < 1024 ? `${b} B` : `${(b / 1024).toFixed(1)} kB`;
}
