import { useEffect, useRef, useState } from "react";
import { useI18n } from "../lib/i18n";

interface QuickCaptureProps {
  /** Where the text will land, shown so nothing disappears unexplained. */
  target: string;
  onSubmit: (text: string) => Promise<void> | void;
  onClose: () => void;
}

/**
 * Catch a thought without leaving what you were doing: a box, one keystroke
 * away, that appends to today's note and closes again.
 *
 * Enter sends, Shift+Enter makes a new line — the opposite of the editor,
 * because the whole point is that this is over in two seconds.
 */
export default function QuickCapture({ target, onSubmit, onClose }: QuickCaptureProps) {
  const { t } = useI18n();
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const ref = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    ref.current?.focus();
  }, []);

  const send = async () => {
    if (!text.trim() || busy) return;
    setBusy(true);
    try {
      await onSubmit(text);
      onClose();
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 p-4 pt-[18vh]"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg overflow-hidden rounded-2xl bg-magma-bg shadow-2xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <textarea
          ref={ref}
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              void send();
            }
          }}
          rows={3}
          placeholder={t("capture.placeholder")}
          className="w-full resize-none bg-transparent px-5 py-4 text-base outline-none placeholder:text-magma-muted"
        />
        <div className="flex items-center gap-2 border-t border-black/10 px-5 py-2.5 dark:border-white/10">
          <span className="min-w-0 flex-1 truncate text-xs text-magma-muted">
            {t("capture.target", { target })}
          </span>
          <span className="shrink-0 text-[11px] text-magma-muted opacity-70">
            {t("capture.hint")}
          </span>
          <button
            onClick={send}
            disabled={!text.trim() || busy}
            className="shrink-0 rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-40"
          >
            {t("capture.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
