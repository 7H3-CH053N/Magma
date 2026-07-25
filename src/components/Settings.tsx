import { useEffect, useState } from "react";
import FlameIcon from "./FlameIcon";
import { useI18n, type Lang } from "../lib/i18n";
import { useTheme, FONT_PRESETS, type ThemeMode } from "../lib/theme";
import {
  hasTauri,
  importWordpress,
  installMcp,
  mcpConfig,
  type RemoteConfig,
} from "../lib/api";

interface SettingsProps {
  onClose: () => void;
  vault: string | null;
  folders: string[];
  onConnectRemote: (cfg: RemoteConfig) => Promise<void>;
  remoteActive: boolean;
}

function savedRemote(): { url: string; username: string } {
  try {
    const raw = localStorage.getItem("magma.remote");
    if (raw) return JSON.parse(raw);
  } catch {
    /* ignore */
  }
  return { url: "", username: "" };
}

/**
 * Settings dialog: language, an About panel (icon, name, version, build,
 * license/copyright), and a copy-paste MCP config to connect Claude.
 */
export default function Settings({
  onClose,
  vault,
  folders,
  onConnectRemote,
  remoteActive,
}: SettingsProps) {
  const { t, lang, setLang } = useI18n();
  const { theme, setTheme, reset } = useTheme();
  const initial = savedRemote();
  const [url, setUrl] = useState(initial.url);
  const [username, setUsername] = useState(initial.username);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  // MCP setup state.
  const [mcpBusy, setMcpBusy] = useState(false);
  const [mcpDone, setMcpDone] = useState<string | null>(null);
  const [mcpErr, setMcpErr] = useState<string | null>(null);
  const [showManual, setShowManual] = useState(false);
  const [configText, setConfigText] = useState("");

  useEffect(() => {
    if (hasTauri && vault) mcpConfig(vault).then(setConfigText).catch(() => {});
  }, [vault]);

  // WordPress import state.
  const [impUrl, setImpUrl] = useState("");
  const [impFolder, setImpFolder] = useState("");
  const [impAuthor, setImpAuthor] = useState("");
  const [impBusy, setImpBusy] = useState(false);
  const [impDone, setImpDone] = useState<string | null>(null);
  const [impWarn, setImpWarn] = useState<string | null>(null);
  const [impInfo, setImpInfo] = useState<string | null>(null);
  const [impErr, setImpErr] = useState<string | null>(null);

  const runImport = async () => {
    if (!vault || !impUrl.trim()) return;
    setImpErr(null);
    setImpDone(null);
    setImpWarn(null);
    setImpInfo(null);
    setImpBusy(true);
    try {
      const res = await importWordpress(
        vault,
        impFolder.trim(),
        impUrl.trim(),
        impAuthor.trim()
      );
      setImpDone(
        t("settings.importDone", {
          count: String(res.notes),
          folder: impFolder.trim() || "/",
        })
      );
      // Say so when the site gave us no author, rather than silently omitting it.
      if (res.authors.length === 0) setImpWarn(t("settings.importNoAuthor"));
      else {
        setImpDone((d) => `${d} · ${t("settings.importAuthors", { authors: res.authors.join(", ") })}`);
        // Spell out where the author ended up — merged into your own note, or
        // in a note the import had to create because no name matched.
        const lines = [
          ...res.merged.map((m) => `↳ ${t("settings.importMerged", { info: m })}`),
          ...res.created.map((c) => `↳ ${t("settings.importCreated", { info: c })}`),
        ];
        if (lines.length) setImpInfo(lines.join("\n"));
      }
    } catch (e) {
      setImpErr(String(e));
    } finally {
      setImpBusy(false);
    }
  };

  const install = async () => {
    if (!vault) return;
    setMcpErr(null);
    setMcpBusy(true);
    try {
      const path = await installMcp(vault);
      setMcpDone(path);
    } catch (e) {
      setMcpErr(String(e));
    } finally {
      setMcpBusy(false);
    }
  };

  const connect = async () => {
    setErr(null);
    setBusy(true);
    try {
      await onConnectRemote({ url: url.trim(), username: username.trim(), password });
      onClose();
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/40 p-4" onClick={onClose}>
      <div
        className="max-h-[90vh] w-full max-w-md overflow-auto rounded-2xl bg-magma-bg p-6 shadow-xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-base font-semibold">{t("settings.title")}</h2>
          <button
            onClick={onClose}
            className="grid h-7 w-7 place-items-center rounded-md text-magma-muted hover:bg-black/10 dark:hover:bg-white/10"
            aria-label={t("settings.close")}
          >
            ✕
          </button>
        </div>

        {/* Appearance */}
        <section className="mb-5">
          <div className="mb-2 flex items-center justify-between">
            <label className="block text-xs font-medium uppercase tracking-wide text-magma-muted">
              {t("settings.appearance")}
            </label>
            <button
              onClick={reset}
              className="text-xs text-magma-muted underline-offset-2 hover:text-magma-accent hover:underline"
            >
              {t("settings.reset")}
            </button>
          </div>

          {/* Theme mode */}
          <div className="mb-3 inline-flex rounded-lg bg-black/[0.04] p-1 dark:bg-white/[0.06]">
            {(["system", "light", "dark"] as ThemeMode[]).map((m) => (
              <button
                key={m}
                onClick={() => setTheme({ mode: m })}
                className={`rounded-md px-3 py-1 text-sm transition ${
                  theme.mode === m
                    ? "bg-magma-bg text-magma-ink shadow-sm dark:bg-[#332d28] dark:text-[#ece9e4]"
                    : "text-magma-muted"
                }`}
              >
                {t(`theme.${m}`)}
              </button>
            ))}
          </div>

          {/* Colors */}
          <div className="mb-3 flex gap-4">
            <ColorField
              label={t("settings.accent")}
              value={theme.accent}
              onChange={(accent) => setTheme({ accent })}
            />
            <ColorField
              label={t("settings.aiColor")}
              value={theme.ai}
              onChange={(ai) => setTheme({ ai })}
            />
          </div>

          {/* Fonts */}
          <div className="mb-3 grid grid-cols-2 gap-3">
            <FontField
              label={t("settings.uiFont")}
              value={theme.uiFont}
              onChange={(uiFont) => setTheme({ uiFont })}
            />
            <FontField
              label={t("settings.editorFont")}
              value={theme.editorFont}
              onChange={(editorFont) => setTheme({ editorFont })}
            />
          </div>

          {/* Sizing */}
          <RangeField
            label={t("settings.fontSize")}
            suffix="px"
            min={13}
            max={22}
            value={theme.fontSize}
            onChange={(fontSize) => setTheme({ fontSize })}
          />
          <RangeField
            label={t("settings.readingWidth")}
            suffix="px"
            min={560}
            max={960}
            step={20}
            value={theme.readingWidth}
            onChange={(readingWidth) => setTheme({ readingWidth })}
          />
        </section>

        {/* Language */}
        <section className="mb-5">
          <label className="mb-2 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.language")}
          </label>
          <div className="inline-flex rounded-lg bg-black/[0.04] p-1 dark:bg-white/[0.06]">
            {(["en", "de"] as Lang[]).map((l) => (
              <button
                key={l}
                onClick={() => setLang(l)}
                className={`rounded-md px-3 py-1 text-sm transition ${
                  lang === l
                    ? "bg-magma-bg text-magma-ink shadow-sm dark:bg-[#332d28] dark:text-[#ece9e4]"
                    : "text-magma-muted"
                }`}
              >
                {l === "en" ? "English" : "Deutsch"}
              </button>
            ))}
          </div>
        </section>

        {/* Remote (WebDAV) vault */}
        <section className="mb-5">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.remoteTitle")}
          </label>
          <p className="mb-2 text-xs text-magma-muted">
            {remoteActive ? t("settings.remoteActive") : t("settings.remoteBody")}
          </p>
          <div className="flex flex-col gap-2">
            <input
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="https://host/dav/my-vault/"
              className="rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
            />
            <div className="flex gap-2">
              <input
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder={t("settings.remoteUser")}
                className="min-w-0 flex-1 rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
              />
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder={t("settings.remotePass")}
                className="min-w-0 flex-1 rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
              />
            </div>
            <button
              onClick={connect}
              disabled={busy || !url.trim()}
              className="self-start rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-50"
            >
              {busy ? t("settings.remoteConnecting") : t("settings.remoteConnect")}
            </button>
            {err && <p className="text-xs text-red-500">{err}</p>}
            <p className="text-[11px] leading-relaxed text-magma-muted opacity-80">
              {t("settings.remoteNote")}
            </p>
          </div>
        </section>

        {/* Import WordPress */}
        <section className="mb-5">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.importTitle")}
          </label>
          <p className="mb-2 text-xs text-magma-muted">{t("settings.importBody")}</p>
          {!vault ? (
            <p className="text-xs text-magma-muted opacity-80">{t("settings.mcpNoVault")}</p>
          ) : (
            <div className="flex flex-col gap-2">
              <input
                value={impUrl}
                onChange={(e) => setImpUrl(e.target.value)}
                placeholder={t("settings.importUrl")}
                className="rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
              />
              <input
                value={impFolder}
                onChange={(e) => setImpFolder(e.target.value)}
                placeholder={t("settings.importFolder")}
                list="magma-import-folders"
                className="rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
              />
              <datalist id="magma-import-folders">
                {folders.map((f) => (
                  <option key={f} value={f} />
                ))}
              </datalist>
              <input
                value={impAuthor}
                onChange={(e) => setImpAuthor(e.target.value)}
                placeholder={t("settings.importAuthor")}
                className="rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
              />
              <button
                onClick={runImport}
                disabled={impBusy || !impUrl.trim()}
                className="self-start rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-50"
              >
                {impBusy ? t("settings.importing") : t("settings.importRun")}
              </button>
              {impDone && (
                <p className="text-xs text-green-600 dark:text-green-400">{impDone}</p>
              )}
              {impInfo && (
                <p className="whitespace-pre-line text-xs text-magma-muted">{impInfo}</p>
              )}
              {impWarn && <p className="text-xs text-amber-600 dark:text-amber-400">{impWarn}</p>}
              {impErr && <p className="text-xs text-red-500">{impErr}</p>}
            </div>
          )}
        </section>

        {/* Connect to Claude */}
        <section className="mb-5">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.connectTitle")}
          </label>
          <p className="mb-2 text-xs text-magma-muted">{t("settings.connectBody")}</p>

          {!vault ? (
            <p className="text-xs text-magma-muted opacity-80">{t("settings.mcpNoVault")}</p>
          ) : (
            <>
              <button
                onClick={install}
                disabled={mcpBusy}
                className="rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-50"
              >
                {mcpBusy ? t("settings.mcpInstalling") : t("settings.mcpInstall")}
              </button>
              {mcpDone && (
                <p className="mt-2 text-xs text-green-600 dark:text-green-400">
                  {t("settings.mcpInstalled", { path: mcpDone })}
                </p>
              )}
              {mcpErr && <p className="mt-2 text-xs text-red-500">{mcpErr}</p>}

              <button
                onClick={() => setShowManual((v) => !v)}
                className="mt-2 block text-xs text-magma-muted underline-offset-2 hover:text-magma-accent hover:underline"
              >
                {t("settings.mcpManual")}
              </button>
              {showManual && configText && (
                <pre className="mt-2 overflow-auto rounded-lg bg-black/[0.05] p-3 text-xs leading-relaxed dark:bg-black/40">
                  <code>{configText}</code>
                </pre>
              )}
            </>
          )}
        </section>

        {/* About */}
        <section className="flex flex-col items-center gap-2 rounded-xl bg-black/[0.03] p-6 text-center dark:bg-white/[0.04]">
          <FlameIcon size={56} />
          <div className="text-lg font-semibold tracking-tight">Magma</div>
          <div className="text-sm text-magma-muted">
            {t("settings.version", { version: __APP_VERSION__, build: __BUILD_ID__ })}
          </div>
          <p className="mt-1 max-w-xs text-xs leading-relaxed text-magma-muted">
            {t("settings.description")}
          </p>
          <div className="mt-3 border-t border-black/10 pt-3 text-xs text-magma-muted dark:border-white/10">
            © 2026 Alex Januschewsky ·{" "}
            <a
              href="https://vibecraft.rocks"
              target="_blank"
              rel="noreferrer"
              className="text-magma-accent hover:underline"
            >
              vibecraft.rocks
            </a>
            <div className="mt-1 opacity-80">{t("settings.license")}</div>
          </div>
        </section>
      </div>
    </div>
  );
}

function ColorField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <label className="flex items-center gap-2 text-sm">
      <input
        type="color"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="h-7 w-7 cursor-pointer rounded-md border border-black/10 bg-transparent p-0 dark:border-white/10"
      />
      <span className="text-magma-muted">{label}</span>
    </label>
  );
}

function FontField({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  // A value not in the presets still shows (custom), keyed to itself.
  const known = FONT_PRESETS.some((p) => p.value === value);
  return (
    <label className="block text-sm">
      <span className="mb-1 block text-magma-muted">{label}</span>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        style={{ fontFamily: value }}
        className="w-full rounded-lg border border-black/10 bg-transparent px-2 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
      >
        {!known && <option value={value}>Custom</option>}
        {FONT_PRESETS.map((p) => (
          <option key={p.label} value={p.value} style={{ fontFamily: p.value }}>
            {p.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function RangeField({
  label,
  value,
  min,
  max,
  step = 1,
  suffix,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  suffix?: string;
  onChange: (v: number) => void;
}) {
  return (
    <label className="mb-2 block text-sm">
      <span className="mb-1 flex justify-between text-magma-muted">
        <span>{label}</span>
        <span>
          {value}
          {suffix}
        </span>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-full accent-magma-accent"
      />
    </label>
  );
}
