import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";

/**
 * Preferences that are about *how you work* rather than how Magma looks:
 * where daily notes go, which folder holds templates, what quick capture
 * writes into.
 *
 * Same draft/save/revert shape as the theme store, so Settings' single save
 * button covers everything on screen and closing without saving really does
 * take back what you changed.
 */
export interface Prefs {
  /** Folder for daily notes; "" puts them in the vault root. */
  dailyFolder: string;
  /** Vault-relative path of a note used as the template for a new day. */
  dailyTemplate: string;
  /** Folder scanned for templates offered when creating a note. */
  templateFolder: string;
  /** Quick capture appends to today's note instead of making a new one. */
  captureToDaily: boolean;
  /** Enabled plugin ids. Unknown ids are ignored by the plugin registry. */
  enabledPluginIds: string[];
}

export const DEFAULT_PREFS: Prefs = {
  dailyFolder: "Journal",
  dailyTemplate: "",
  templateFolder: "Templates",
  captureToDaily: true,
  enabledPluginIds: ["random-note", "word-count"],
};

const STORAGE_KEY = "magma.prefs";

function load(): Prefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_PREFS, ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return DEFAULT_PREFS;
}

interface PrefsCtx {
  prefs: Prefs;
  setPrefs: (patch: Partial<Prefs>) => void;
  save: () => void;
  revert: () => void;
  resetDefaults: () => void;
  dirty: boolean;
}

const Context = createContext<PrefsCtx | null>(null);

export function PrefsProvider({ children }: { children: ReactNode }) {
  const [prefs, setState] = useState<Prefs>(load);
  const [saved, setSaved] = useState<Prefs>(prefs);

  const setPrefs = useCallback((patch: Partial<Prefs>) => {
    setState((prev) => ({ ...prev, ...patch }));
  }, []);

  const save = useCallback(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs));
    } catch {
      /* ignore */
    }
    setSaved(prefs);
  }, [prefs]);

  const revert = useCallback(() => setState(saved), [saved]);
  const resetDefaults = useCallback(() => setState(DEFAULT_PREFS), []);
  const dirty = useMemo(
    () => JSON.stringify(prefs) !== JSON.stringify(saved),
    [prefs, saved]
  );

  return (
    <Context.Provider value={{ prefs, setPrefs, save, revert, resetDefaults, dirty }}>
      {children}
    </Context.Provider>
  );
}

export function usePrefs(): PrefsCtx {
  const ctx = useContext(Context);
  if (!ctx) throw new Error("usePrefs must be used within PrefsProvider");
  return ctx;
}

/** "2026-07-26" — a daily note's name, sortable and locale-independent. */
export function dayKey(d: Date): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/**
 * Fill a template's placeholders. Deliberately a handful of obvious ones
 * rather than an expression language: `{{date}}`, `{{time}}`, `{{title}}`,
 * `{{weekday}}`, and `{{date:...}}` with a few tokens.
 */
export function applyTemplate(template: string, title: string, now = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  const tokens: Record<string, string> = {
    date: dayKey(now),
    time: `${p(now.getHours())}:${p(now.getMinutes())}`,
    title,
    weekday: now.toLocaleDateString(undefined, { weekday: "long" }),
    month: now.toLocaleDateString(undefined, { month: "long" }),
    year: String(now.getFullYear()),
  };
  return template.replace(/\{\{\s*([a-z]+)\s*\}\}/gi, (whole, key: string) => {
    const v = tokens[key.toLowerCase()];
    return v === undefined ? whole : v;
  });
}
