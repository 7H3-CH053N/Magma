import { useEffect, useState } from "react";
import { readNote } from "../lib/api";
import { useI18n } from "../lib/i18n";

interface NodePreviewProps {
  vault: string;
  /** Vault path, or "missing:<name>" for a link target with no note yet. */
  nodePath: string;
  title: string;
  onOpenEditor: (path: string) => void;
  onCreate: (name: string) => void;
  onClose: () => void;
}

/**
 * Side panel for a graph node: read what's in the note without leaving the
 * graph. Opening the editor stays a deliberate click, so exploring the map
 * doesn't keep throwing you out of it.
 */
export default function NodePreview({
  vault,
  nodePath,
  title,
  onOpenEditor,
  onCreate,
  onClose,
}: NodePreviewProps) {
  const { t } = useI18n();
  const missing = nodePath.startsWith("missing:");
  const [body, setBody] = useState("");

  useEffect(() => {
    if (missing) return;
    let live = true;
    readNote(vault, nodePath)
      .then((n) => live && setBody(n.content))
      .catch(() => live && setBody(""));
    return () => {
      live = false;
    };
  }, [vault, nodePath, missing]);

  return (
    <aside className="flex h-full w-80 shrink-0 flex-col border-l border-black/5 bg-magma-panel/60 dark:border-white/5 dark:bg-white/5">
      <div className="flex items-start gap-2 px-4 py-3">
        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">{title}</p>
          {!missing && <p className="truncate text-xs text-magma-muted">{nodePath}</p>}
        </div>
        <button
          onClick={onClose}
          className="grid h-6 w-6 shrink-0 place-items-center rounded-md text-magma-muted transition hover:bg-black/10 dark:hover:bg-white/10"
          aria-label={t("settings.close")}
        >
          ✕
        </button>
      </div>
      <div className="flex-1 overflow-auto px-4 pb-3">
        {missing ? (
          <p className="text-sm text-magma-muted">{t("graph.previewMissing")}</p>
        ) : (
          <pre className="whitespace-pre-wrap break-words font-[inherit] text-sm leading-relaxed text-magma-ink dark:text-[#ece9e4]">
            {body}
          </pre>
        )}
      </div>
      <div className="border-t border-black/5 p-3 dark:border-white/10">
        <button
          onClick={() => (missing ? onCreate(title) : onOpenEditor(nodePath))}
          className="w-full rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90"
        >
          {missing ? t("graph.createNote") : t("graph.openEditor")}
        </button>
      </div>
    </aside>
  );
}
