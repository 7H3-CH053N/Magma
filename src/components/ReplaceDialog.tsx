import { useCallback, useEffect, useRef, useState } from "react";
import { replaceAll, type ReplaceReport } from "../lib/api";
import { useI18n } from "../lib/i18n";
import { SearchIcon } from "./Icons";

interface ReplaceDialogProps {
  vault: string;
  /** Prefills the find field — usually whatever is in the search box. */
  initialFind: string;
  onClose: () => void;
  /** Called after a successful apply so the vault can be reloaded. */
  onApplied: (report: ReplaceReport) => void;
}

/**
 * Vault-wide find & replace.
 *
 * Nothing is written until the preview has been seen: the dialog runs a dry
 * run on every keystroke and only the explicit apply button touches files.
 * Rewriting hundreds of notes is not undoable from inside the app, so the
 * count of affected notes is always on screen before the button is reachable.
 */
export default function ReplaceDialog({
  vault,
  initialFind,
  onClose,
  onApplied,
}: ReplaceDialogProps) {
  const { t } = useI18n();
  const [find, setFind] = useState(initialFind);
  const [replace, setReplace] = useState("");
  const [renameNotes, setRenameNotes] = useState(true);
  const [report, setReport] = useState<ReplaceReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const findRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    findRef.current?.focus();
    findRef.current?.select();
  }, []);

  // Preview, debounced: a dry run walks the whole vault, so it should not fire
  // on every single character of a long name.
  useEffect(() => {
    if (!find.trim()) {
      setReport(null);
      return;
    }
    let cancelled = false;
    const id = window.setTimeout(() => {
      replaceAll(vault, find, replace, true, renameNotes)
        .then((r) => !cancelled && setReport(r))
        .catch((e) => !cancelled && setError(String(e)));
    }, 220);
    return () => {
      cancelled = true;
      window.clearTimeout(id);
    };
  }, [vault, find, replace, renameNotes]);

  const apply = useCallback(async () => {
    if (!find.trim() || busy) return;
    setBusy(true);
    setError(null);
    try {
      const done = await replaceAll(vault, find, replace, false, renameNotes);
      onApplied(done);
      onClose();
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }, [vault, find, replace, renameNotes, busy, onApplied, onClose]);

  // Distinct notes touched: a renamed note usually also holds the term in its
  // text, so the two lists overlap and must not be added up.
  const affected = report
    ? new Set([...report.hits.map((h) => h.path), ...report.renames.map((r) => r.path)]).size
    : 0;
  const nothing = !!report && affected === 0;

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4"
      onClick={onClose}
      onKeyDown={(e) => {
        if (e.key === "Escape") onClose();
      }}
    >
      <div
        className="flex max-h-[80vh] w-full max-w-lg flex-col rounded-2xl bg-magma-bg shadow-xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-black/10 px-5 py-3.5 dark:border-white/10">
          <SearchIcon size={15} className="text-magma-muted" />
          <h2 className="text-sm font-semibold">{t("replace.title")}</h2>
        </div>

        <div className="space-y-3 px-5 py-4">
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-magma-muted">
              {t("replace.find")}
            </span>
            <input
              ref={findRef}
              value={find}
              onChange={(e) => setFind(e.target.value)}
              className="w-full rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
            />
          </label>
          <label className="block">
            <span className="mb-1 block text-xs font-medium text-magma-muted">
              {t("replace.with")}
            </span>
            <input
              value={replace}
              onChange={(e) => setReplace(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") apply();
              }}
              className="w-full rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
            />
          </label>

          <label className="flex cursor-pointer items-start gap-2.5">
            <input
              type="checkbox"
              checked={renameNotes}
              onChange={(e) => setRenameNotes(e.target.checked)}
              className="mt-0.5 accent-magma-accent"
            />
            <span className="text-xs leading-relaxed">
              <span className="font-medium">{t("replace.renameNotes")}</span>
              <span className="block text-magma-muted">
                {t("replace.renameNotesHint")}
              </span>
            </span>
          </label>
        </div>

        {/* Preview. Everything below is read-only until the apply button. */}
        <div className="min-h-[3rem] flex-1 overflow-auto border-t border-black/10 px-5 py-3 dark:border-white/10">
          {error && <p className="text-sm text-red-500">{error}</p>}
          {!error && !find.trim() && (
            <p className="text-xs text-magma-muted">{t("replace.typeToPreview")}</p>
          )}
          {!error && nothing && (
            <p className="text-xs text-magma-muted">{t("replace.noMatches")}</p>
          )}
          {!error && report && affected > 0 && (
            <>
              <p className="mb-2 text-xs text-magma-muted">
                {t("replace.summary", {
                  total: String(report.total),
                  notes: String(report.hits.length),
                })}
              </p>
              {report.renames.length > 0 && (
                <ul className="mb-2 space-y-1">
                  {report.renames.map((r) => (
                    <li key={r.path} className="truncate text-xs">
                      <span className="mr-1.5 rounded bg-magma-accent/15 px-1.5 py-0.5 text-[10px] font-medium text-magma-accent">
                        {t("replace.renameBadge")}
                      </span>
                      <span className="text-magma-muted line-through">{r.from}</span>
                      <span className="mx-1 text-magma-muted">→</span>
                      <span>{r.to}</span>
                    </li>
                  ))}
                </ul>
              )}
              <ul className="space-y-0.5">
                {report.hits.map((h) => (
                  <li key={h.path} className="flex gap-2 text-xs">
                    <span className="w-6 shrink-0 text-right tabular-nums text-magma-muted">
                      {h.count}×
                    </span>
                    <span className="truncate">{h.title}</span>
                  </li>
                ))}
              </ul>
            </>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 border-t border-black/10 px-5 py-3 dark:border-white/10">
          <button
            onClick={onClose}
            className="rounded-lg px-3 py-1.5 text-sm text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
          >
            {t("replace.cancel")}
          </button>
          <button
            onClick={apply}
            disabled={busy || affected === 0}
            className="rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {busy
              ? t("replace.applying")
              : t("replace.apply", { notes: String(affected) })}
          </button>
        </div>
      </div>
    </div>
  );
}
