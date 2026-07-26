import { useEffect, useRef, useState } from "react";

interface PromptDialogProps {
  title: string;
  initial?: string;
  placeholder?: string;
  /** Optional folder suggestions shown as a dropdown (datalist) on the input. */
  suggestions?: string[];
  confirmLabel: string;
  cancelLabel: string;
  onSubmit: (value: string) => void;
  onCancel: () => void;
}

/**
 * In-app text prompt. Replaces window.prompt(), which the Tauri/WKWebView
 * desktop shell does not support (it silently returns null).
 */
export default function PromptDialog({
  title,
  initial = "",
  placeholder,
  suggestions,
  confirmLabel,
  cancelLabel,
  onSubmit,
  onCancel,
}: PromptDialogProps) {
  const [value, setValue] = useState(initial);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = "magma-folder-suggestions";

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  return (
    <div
      className="fixed inset-0 z-50 grid place-items-center bg-black/40 p-4"
      onClick={onCancel}
    >
      <div
        className="w-full max-w-sm rounded-2xl bg-magma-bg p-5 shadow-xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <label className="mb-2 block text-sm font-medium">{title}</label>
        <input
          ref={inputRef}
          value={value}
          placeholder={placeholder}
          list={suggestions && suggestions.length ? listId : undefined}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") onSubmit(value);
            if (e.key === "Escape") onCancel();
          }}
          className="w-full rounded-lg border border-black/10 bg-transparent px-3 py-2 text-sm outline-none focus:border-magma-accent dark:border-white/10"
        />
        {suggestions && suggestions.length > 0 && (
          <datalist id={listId}>
            {suggestions.map((s) => (
              <option key={s} value={s} />
            ))}
          </datalist>
        )}
        <div className="mt-4 flex justify-end gap-2">
          <button
            onClick={onCancel}
            className="rounded-lg px-3 py-1.5 text-sm text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
          >
            {cancelLabel}
          </button>
          <button
            onClick={() => onSubmit(value)}
            className="rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90"
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
