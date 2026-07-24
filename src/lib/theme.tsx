import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";

export type ThemeMode = "system" | "light" | "dark";

export interface ThemeSettings {
  mode: ThemeMode;
  accent: string;
  ai: string;
  uiFont: string;
  editorFont: string;
  fontSize: number; // px
  readingWidth: number; // px
}

// Defaults mirror the CSS variables in styles/index.css.
// Named font choices offered in Settings. Values use only system-available
// fonts so nothing has to be downloaded (works offline, no CSP issues).
export const FONT_PRESETS: { label: string; value: string }[] = [
  {
    label: "System",
    value: '-apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif',
  },
  { label: "Inter / Sans", value: "Inter, system-ui, sans-serif" },
  { label: "Serif", value: 'Georgia, "Iowan Old Style", "Times New Roman", serif' },
  { label: "Mono", value: '"JetBrains Mono", ui-monospace, SFMono-Regular, monospace' },
  {
    label: "Rounded",
    value: '"SF Pro Rounded", "Nunito", "Segoe UI", system-ui, sans-serif',
  },
];

export const DEFAULT_THEME: ThemeSettings = {
  mode: "system",
  accent: "#e0533d",
  ai: "#7c5cff",
  uiFont: FONT_PRESETS[1].value, // Inter / Sans
  editorFont: FONT_PRESETS[0].value, // System
  fontSize: 16,
  readingWidth: 720,
};

const STORAGE_KEY = "magma.theme";

function load(): ThemeSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return { ...DEFAULT_THEME, ...JSON.parse(raw) };
  } catch {
    /* ignore */
  }
  return DEFAULT_THEME;
}

/** Apply theme settings to the document root (CSS vars + the .dark class). */
function apply(s: ThemeSettings, systemDark: boolean) {
  const root = document.documentElement;
  const dark = s.mode === "dark" || (s.mode === "system" && systemDark);
  root.classList.toggle("dark", dark);
  root.style.setProperty("--magma-accent", s.accent);
  root.style.setProperty("--magma-ai", s.ai);
  root.style.setProperty("--magma-font-ui", s.uiFont);
  root.style.setProperty("--magma-font-editor", s.editorFont);
  root.style.setProperty("--magma-font-size", `${s.fontSize}px`);
  root.style.setProperty("--magma-reading-width", `${s.readingWidth}px`);
}

interface ThemeCtx {
  theme: ThemeSettings;
  setTheme: (patch: Partial<ThemeSettings>) => void;
  reset: () => void;
}

const Context = createContext<ThemeCtx | null>(null);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [theme, setThemeState] = useState<ThemeSettings>(load);
  const media = useMemo(() => window.matchMedia("(prefers-color-scheme: dark)"), []);

  // Apply on change and follow the OS when in "system" mode.
  useEffect(() => {
    apply(theme, media.matches);
    const onChange = () => {
      if (theme.mode === "system") apply(theme, media.matches);
    };
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, [theme, media]);

  const setTheme = useCallback((patch: Partial<ThemeSettings>) => {
    setThemeState((prev) => {
      const next = { ...prev, ...patch };
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(next));
      } catch {
        /* ignore */
      }
      return next;
    });
  }, []);

  const reset = useCallback(() => {
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      /* ignore */
    }
    setThemeState(DEFAULT_THEME);
  }, []);

  return <Context.Provider value={{ theme, setTheme, reset }}>{children}</Context.Provider>;
}

export function useTheme(): ThemeCtx {
  const ctx = useContext(Context);
  if (!ctx) throw new Error("useTheme must be used within ThemeProvider");
  return ctx;
}
