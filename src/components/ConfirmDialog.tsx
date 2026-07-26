import { useEffect, useRef } from "react";

interface ConfirmDialogProps {
  title: string;
  /** Optional second line spelling out the consequence. */
  detail?: string;
  confirmLabel: string;
  cancelLabel: string;
  /** Style the confirm button as destructive (delete). */
  destructive?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

/**
 * In-app confirmation. Replaces window.confirm(), which the Tauri/WKWebView
 * desktop shell does not surface — the same reason PromptDialog exists.
 * Cancel is focused by default so a stray Enter never deletes anything.
 */
export default function ConfirmDialog({
  title,
  detail,
  confirmLabel,
  cancelLabel,
  destructive,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelRef.current?.focus();
  }, []);
  // With no cancel button there is nothing to hold the focus, so the confirm
  // button takes it — Escape and Enter both then do the harmless thing.
  const confirmRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!cancelLabel) confirmRef.current?.focus();
  }, [cancelLabel]);

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4"
      onClick={onCancel}
      onKeyDown={(e) => {
        if (e.key === "Escape") onCancel();
      }}
    >
      <div
        className="w-full max-w-sm rounded-2xl bg-magma-bg p-5 shadow-xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <p className="text-sm font-medium">{title}</p>
        {detail && <p className="mt-1.5 text-sm text-magma-muted">{detail}</p>}
        <div className="mt-4 flex justify-end gap-2">
          {/* An empty cancel label means this is a notice, not a choice. */}
          <button
            hidden={!cancelLabel}
            ref={cancelRef}
            onClick={onCancel}
            className="rounded-lg px-3 py-1.5 text-sm text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
          >
            {cancelLabel}
          </button>
          <button
            ref={confirmRef}
            onClick={onConfirm}
            className={`rounded-lg px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 ${
              destructive ? "bg-red-600" : "bg-magma-accent"
            }`}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
