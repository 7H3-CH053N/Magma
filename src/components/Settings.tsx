import { useEffect, useState } from "react";
import { MagmaMark } from "./MagmaMark";
import { useI18n, type Lang } from "../lib/i18n";
import { contrastIssues } from "../lib/contrast";
import {
  useTheme,
  DEFAULT_DARK,
  DEFAULT_LIGHT,
  FONT_PRESETS,
  PALETTE_KEYS,
  type Palette,
  type ThemeMode,
} from "../lib/theme";
import { usePrefs } from "../lib/prefs";
import {
  codexMcpConfig,
  hasTauri,
  checkForAppUpdate,
  importWordpress,
  onImportProgress,
  type ImportProgress,
  installAppUpdate,
  installCodexMcp,
  installMcp,
  mcpConfig,
  modelStatus,
  downloadModel,
  downloadReranker,
  downloadSizes,
  onModelProgress,
  indexVault,
  onIndexProgress,
  type IndexProgress,
  type DownloadSizes,
  type IndexReport,
  type ModelStatus,
  type ModelProgress,
  type AppUpdate,
  type NoteMeta,
  type RemoteConfig,
  type UpdateProgress,
} from "../lib/api";

interface SettingsProps {
  onClose: () => void;
  vault: string | null;
  folders: string[];
  notes: NoteMeta[];
  onConnectRemote: (cfg: RemoteConfig) => Promise<void>;
  remoteActive: boolean;
  /** Pick a vault folder — the only place this lives now. */
  onOpenVault: () => void;
}

type Tab = "vault" | "notes" | "appearance" | "import" | "claude" | "about";

function savedRemote(): { url: string; username: string } {
  try {
    const raw = localStorage.getItem("magma.remote");
    if (raw) return JSON.parse(raw);
  } catch {
    /* ignore */
  }
  return { url: "", username: "" };
}

function updateErrorKey(error: unknown): string {
  const text = String(error).toLowerCase();
  if (
    text.includes("valid release json") ||
    text.includes("status code: 404") ||
    text.includes("not found")
  ) {
    return "settings.updateNoManifest";
  }
  return "settings.updateFailed";
}

/**
 * Settings dialog: language, an About panel (icon, name, version, build,
 * license/copyright), and a copy-paste MCP config to connect Claude.
 */
export default function Settings({
  onClose,
  vault,
  folders,
  notes,
  onConnectRemote,
  remoteActive,
  onOpenVault,
}: SettingsProps) {
  const [tab, setTab] = useState<Tab>("vault");
  const { t, lang, setLang, saveLang, revertLang, langDirty } = useI18n();
  const { theme, setTheme, save, revert, resetDefaults, dirty: themeDirty } = useTheme();
  /** Patch one colour of one mode, leaving the other mode alone. */
  const setPalette = (m: "light" | "dark", patch: Partial<Palette>) =>
    setTheme(
      m === "light"
        ? { light: { ...theme.light, ...patch } }
        : { dark: { ...theme.dark, ...patch } }
    );
  // Which palette is on screen. Read from the class the theme provider sets,
  // so it is right under "system" too without duplicating that logic here.
  const shownMode = document.documentElement.classList.contains("dark")
    ? "dark"
    : "light";
  const {
    prefs,
    setPrefs,
    save: savePrefs,
    revert: revertPrefs,
    resetDefaults: resetPrefs,
    dirty: prefsDirty,
  } = usePrefs();
  const dirty = themeDirty || langDirty || prefsDirty;
  const [savedNote, setSavedNote] = useState(false);
  const initial = savedRemote();
  const [url, setUrl] = useState(initial.url);
  const [username, setUsername] = useState(initial.username);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [updateBusy, setUpdateBusy] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<string | null>(null);
  const [updateErr, setUpdateErr] = useState<string | null>(null);
  const [availableUpdate, setAvailableUpdate] = useState<AppUpdate | null>(null);

  // MCP setup state.
  const [mcpBusy, setMcpBusy] = useState(false);
  const [mcpDone, setMcpDone] = useState<string | null>(null);
  const [mcpWarn, setMcpWarn] = useState<string | null>(null);
  const [mcpErr, setMcpErr] = useState<string | null>(null);
  const [showManual, setShowManual] = useState(false);
  const [configText, setConfigText] = useState("");
  const [codexBusy, setCodexBusy] = useState(false);
  const [codexDone, setCodexDone] = useState<string | null>(null);
  const [codexWarn, setCodexWarn] = useState<string | null>(null);
  const [codexErr, setCodexErr] = useState<string | null>(null);
  const [showCodexManual, setShowCodexManual] = useState(false);
  const [codexConfigText, setCodexConfigText] = useState("");

  useEffect(() => {
    if (hasTauri && vault) {
      mcpConfig(vault).then(setConfigText).catch(() => {});
      codexMcpConfig(vault).then(setCodexConfigText).catch(() => {});
    }
  }, [vault]);

  // Semantic search: the encoder is an opt-in download, so the panel has to say
  // what it costs before the button and what it is doing during it.
  const [model, setModel] = useState<ModelStatus | null>(null);
  const [modelBusy, setModelBusy] = useState(false);
  const [modelErr, setModelErr] = useState<string | null>(null);
  const [modelProg, setModelProg] = useState<ModelProgress | null>(null);
  const [rerankBusy, setRerankBusy] = useState(false);
  const [sizes, setSizes] = useState<DownloadSizes | null>(null);

  useEffect(() => {
    if (!hasTauri) return;
    // Two calls on purpose. The status is a look at the disk and decides which
    // controls appear, so it must not wait on anything; the sizes go over the
    // network and arrive when they arrive, or not at all.
    modelStatus().then(setModel).catch(() => {});
    downloadSizes().then(setSizes).catch(() => {});
  }, []);

  const [indexBusy, setIndexBusy] = useState(false);
  const [indexProg, setIndexProg] = useState<IndexProgress | null>(null);
  const [indexDone, setIndexDone] = useState<IndexReport | null>(null);
  // Start time and where the bar stood then, so the estimate is built from this
  // run's own rate rather than from passages already in the cache.
  const [indexStart, setIndexStart] = useState<{ at: number; done: number } | null>(null);

  /** Minutes still to go at the rate this run has managed, or null while unknown. */
  function indexEta(p: IndexProgress): number | null {
    if (!indexStart) return null;
    const encoded = p.done - indexStart.done;
    const seconds = (Date.now() - indexStart.at) / 1000;
    // Ten seconds of data is not a rate worth quoting.
    if (encoded <= 0 || seconds < 10) return null;
    const left = p.total - p.done;
    return Math.max(1, Math.round(left / (encoded / seconds) / 60));
  }

  async function runIndex() {
    if (!vault) return;
    setIndexBusy(true);
    setModelErr(null);
    setIndexDone(null);
    let stop: (() => void) | null = null;
    try {
      stop = await onIndexProgress((p) => {
        setIndexStart((s) => s ?? { at: Date.now(), done: p.done });
        setIndexProg(p);
      });
      setIndexDone(await indexVault(vault));
    } catch (e) {
      setModelErr(String(e));
    } finally {
      if (stop) stop();
      setIndexBusy(false);
      setIndexProg(null);
      setIndexStart(null);
    }
  }

  async function fetchModel() {
    if (!vault) return;
    setModelBusy(true);
    setModelErr(null);
    setModelProg(null);
    let stop: (() => void) | null = null;
    try {
      stop = await onModelProgress(setModelProg);
      await downloadModel(vault);
      setModel(await modelStatus());
      downloadSizes().then(setSizes).catch(() => {});
    } catch (e) {
      setModelErr(String(e));
    } finally {
      // Unsubscribe whatever happened, or a second attempt stacks listeners.
      if (stop) stop();
      setModelBusy(false);
      setModelProg(null);
    }
  }

  async function fetchReranker() {
    setRerankBusy(true);
    setModelErr(null);
    setModelProg(null);
    let stop: (() => void) | null = null;
    try {
      stop = await onModelProgress(setModelProg);
      await downloadReranker();
      setModel(await modelStatus());
      downloadSizes().then(setSizes).catch(() => {});
    } catch (e) {
      setModelErr(String(e));
    } finally {
      // Unsubscribe whatever happened, or a second attempt stacks listeners.
      if (stop) stop();
      setRerankBusy(false);
      setModelProg(null);
    }
  }

  // WordPress import state.
  const [impUrl, setImpUrl] = useState("");
  const [impFolder, setImpFolder] = useState("");
  const [impAuthor, setImpAuthor] = useState("");
  const [impAuthorNote, setImpAuthorNote] = useState("");
  const [impBusy, setImpBusy] = useState(false);
  const [impDone, setImpDone] = useState<string | null>(null);
  const [impWarn, setImpWarn] = useState<string | null>(null);
  const [impInfo, setImpInfo] = useState<string | null>(null);
  const [impErr, setImpErr] = useState<string | null>(null);
  const [impProgress, setImpProgress] = useState<ImportProgress | null>(null);

  const runImport = async () => {
    // Saying nothing at all is what makes an import look broken. If there is
    // nothing to import from, say which half is missing.
    if (!vault) {
      setImpErr(t("settings.importNoVault"));
      return;
    }
    if (!impUrl.trim()) {
      setImpErr(t("settings.importNoUrl"));
      return;
    }
    setImpErr(null);
    setImpDone(null);
    setImpWarn(null);
    setImpInfo(null);
    setImpProgress(null);
    setImpBusy(true);
    let unlisten: (() => void) | null = null;
    try {
      unlisten = await onImportProgress(setImpProgress);
      const res = await importWordpress(
        vault,
        impFolder.trim(),
        impUrl.trim(),
        impAuthor.trim(),
        impAuthorNote
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
      unlisten?.();
      setImpBusy(false);
      setImpProgress(null);
    }
  };

  const install = async () => {
    if (!vault) return;
    setMcpErr(null);
    setMcpWarn(null);
    setMcpBusy(true);
    try {
      const res = await installMcp(vault);
      setMcpDone(res.configPath);
      setMcpWarn(res.devBuild ? t("settings.mcpDevBuild", { exe: res.executable }) : null);
    } catch (e) {
      setMcpErr(String(e));
    } finally {
      setMcpBusy(false);
    }
  };

  const installCodex = async () => {
    if (!vault) return;
    setCodexErr(null);
    setCodexWarn(null);
    setCodexBusy(true);
    try {
      const res = await installCodexMcp(vault);
      setCodexDone(res.configPath);
      setCodexWarn(res.devBuild ? t("settings.codexMcpDevBuild", { exe: res.executable }) : null);
    } catch (e) {
      setCodexErr(String(e));
    } finally {
      setCodexBusy(false);
    }
  };

  // Appearance and language preview live so you can judge them; this is the
  // only thing that writes them down.
  const saveAll = () => {
    save();
    saveLang();
    savePrefs();
    setSavedNote(true);
    window.setTimeout(() => setSavedNote(false), 1800);
  };

  // Closing must not leave a half-applied look behind: anything not saved is
  // taken back, which is also what makes the save button mean something.
  const close = () => {
    revert();
    revertLang();
    revertPrefs();
    onClose();
  };

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

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

  const renderUpdateProgress = (progress: UpdateProgress) => {
    if (progress.status === "checking") setUpdateInfo(t("settings.updateChecking"));
    else if (progress.status === "available") {
      setAvailableUpdate(progress.update);
      setUpdateInfo(t("settings.updateFound", { version: progress.update.version }));
    } else if (progress.status === "none") setUpdateInfo(t("settings.updateNone"));
    else if (progress.status === "downloading") {
      const mb = (progress.downloaded / 1024 / 1024).toFixed(1);
      const total = progress.total ? ` / ${(progress.total / 1024 / 1024).toFixed(1)} MB` : " MB";
      setUpdateInfo(t("settings.updateDownloading", { progress: `${mb}${total}` }));
    } else if (progress.status === "installing") setUpdateInfo(t("settings.updateInstalling"));
    else setUpdateInfo(t("settings.updateRelaunching"));
  };

  const checkUpdates = async () => {
    setUpdateErr(null);
    setUpdateInfo(t("settings.updateChecking"));
    setUpdateBusy(true);
    try {
      const update = await checkForAppUpdate();
      setAvailableUpdate(update);
      setUpdateInfo(update ? t("settings.updateFound", { version: update.version }) : t("settings.updateNone"));
    } catch (e) {
      setUpdateErr(t(updateErrorKey(e)));
      setUpdateInfo(null);
    } finally {
      setUpdateBusy(false);
    }
  };

  const installUpdate = async () => {
    setUpdateErr(null);
    setUpdateBusy(true);
    try {
      await installAppUpdate(renderUpdateProgress);
    } catch (e) {
      setUpdateErr(t(updateErrorKey(e)));
    } finally {
      setUpdateBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/40 p-4" onClick={close}>
      <div
        className="flex h-[min(90vh,44rem)] w-full max-w-4xl overflow-hidden rounded-2xl bg-magma-bg shadow-xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Navigation: settings grouped by what you came here to do. */}
        <nav className="flex w-52 shrink-0 flex-col gap-0.5 border-r border-black/5 bg-black/[0.02] p-3 dark:border-white/5 dark:bg-white/[0.03]">
          <p className="px-2 pb-2 pt-1 text-sm font-semibold">{t("settings.title")}</p>
          {(
            [
              ["vault", t("settings.tabVault")],
              ["notes", t("settings.tabNotes")],
              ["appearance", t("settings.tabAppearance")],
              ["import", t("settings.tabImport")],
              ["claude", t("settings.tabClaude")],
              ["about", t("settings.tabAbout")],
            ] as [Tab, string][]
          ).map(([id, label]) => (
            <button
              key={id}
              onClick={() => setTab(id)}
              className={`rounded-lg px-2.5 py-1.5 text-left text-sm transition ${
                tab === id
                  ? "bg-magma-accent/12 text-magma-accent"
                  : "text-magma-muted hover:bg-black/5 dark:hover:bg-white/5"
              }`}
            >
              {label}
            </button>
          ))}
        </nav>

        <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex-1 overflow-auto p-6">

        {tab === "notes" && (<>
        {/* Daily notes */}
        <section className="mb-5">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.dailyTitle")}
          </label>
          <p className="mb-2 text-xs leading-relaxed text-magma-muted">
            {t("settings.dailyBody")}
          </p>
          <div className="grid gap-3 sm:grid-cols-2">
            <label className="block text-sm">
              <span className="mb-1 block text-magma-muted">{t("settings.dailyFolder")}</span>
              <input
                value={prefs.dailyFolder}
                onChange={(e) => setPrefs({ dailyFolder: e.target.value })}
                list="magma-folder-list"
                className="w-full rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
              />
            </label>
            <label className="block text-sm">
              <span className="mb-1 block text-magma-muted">{t("settings.dailyTemplate")}</span>
              <select
                value={prefs.dailyTemplate}
                onChange={(e) => setPrefs({ dailyTemplate: e.target.value })}
                className="w-full rounded-lg border border-black/10 bg-transparent px-2 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
              >
                <option value="">{t("settings.templateNone")}</option>
                {notes.map((n) => (
                  <option key={n.path} value={n.path}>
                    {n.title} — {n.path}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <datalist id="magma-folder-list">
            {folders.map((f) => (
              <option key={f} value={f} />
            ))}
          </datalist>
        </section>

        {/* Templates */}
        <section className="mb-5">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.templatesTitle")}
          </label>
          <p className="mb-2 text-xs leading-relaxed text-magma-muted">
            {t("settings.templatesBody")}
          </p>
          <input
            value={prefs.templateFolder}
            onChange={(e) => setPrefs({ templateFolder: e.target.value })}
            list="magma-folder-list"
            placeholder={t("settings.templateFolder")}
            className="w-full max-w-sm rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
          />
        </section>

        {/* Quick capture */}
        <section className="mb-5">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.captureTitle")}
          </label>
          <p className="mb-2 text-xs leading-relaxed text-magma-muted">
            {t("settings.captureBody")}
          </p>
          <label className="flex cursor-pointer items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={prefs.captureToDaily}
              onChange={(e) => setPrefs({ captureToDaily: e.target.checked })}
              className="accent-magma-accent"
            />
            <span>{t("settings.captureToDaily")}</span>
          </label>
        </section>
        </>)}

        {tab === "appearance" && (<>
        {/* Appearance */}
        <section className="mb-5">
          <label className="mb-2 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.appearance")}
          </label>

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

          {/* Colors — one scheme per mode. Both are shown at once rather than
              following the mode switch, so the two can be kept coherent
              without flipping back and forth to compare them. */}
          <p className="mb-2 text-xs text-magma-muted">{t("settings.colorsHint")}</p>
          <div className="mb-3 grid grid-cols-2 gap-x-6 gap-y-2">
            {(["light", "dark"] as const).map((m) => (
              <div key={m}>
                <div className="mb-1.5 flex items-baseline justify-between gap-2">
                  <span className="text-xs font-medium uppercase tracking-wide text-magma-muted">
                    {t(`theme.${m}`)}
                    {m === shownMode && ` · ${t("settings.colorsActive")}`}
                  </span>
                  <button
                    onClick={() =>
                      setTheme(
                        m === "light"
                          ? { light: DEFAULT_LIGHT }
                          : { dark: DEFAULT_DARK }
                      )
                    }
                    className="shrink-0 text-xs text-magma-muted underline-offset-2 hover:text-magma-accent hover:underline"
                  >
                    {t("settings.resetMode")}
                  </button>
                </div>
                <div className="flex flex-col gap-1.5">
                  {PALETTE_KEYS.map((key) => (
                    <ColorField
                      key={key}
                      label={t(`settings.color.${key}`)}
                      value={theme[m][key]}
                      onChange={(v) => setPalette(m, { [key]: v })}
                    />
                  ))}
                </div>
                {/* Only text against its background. A scheme can be any taste;
                    it just should not end up unreadable without saying so. */}
                {contrastIssues(theme[m]).map((issue) => (
                  <p
                    key={issue.pair}
                    className="mt-1.5 text-xs text-amber-600 dark:text-amber-400"
                  >
                    {t(`settings.contrast.${issue.pair}`)}{" "}
                    {t("settings.contrastRatio", { ratio: issue.ratio.toFixed(1) })}
                  </p>
                ))}
              </div>
            ))}
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
                onClick={() => setLang(l, false)}
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

        </>)}

        {tab === "vault" && (<>
        {/* Where the notes live */}
        <section className="mb-5">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.vaultTitle")}
          </label>
          <p className="mb-2 text-xs text-magma-muted">{t("settings.vaultBody")}</p>
          <div className="mb-2 truncate rounded-lg bg-black/5 px-3 py-2 font-mono text-xs dark:bg-white/10">
            {vault ?? t("settings.vaultNone")}
          </div>
          <button
            onClick={onOpenVault}
            className="rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90"
          >
            {vault ? t("settings.vaultChange") : t("settings.vaultChoose")}
          </button>
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

        </>)}

        {tab === "import" && (
        /* Import WordPress */
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
              <select
                value={impAuthorNote}
                onChange={(e) => setImpAuthorNote(e.target.value)}
                className="rounded-lg border border-black/10 bg-transparent px-3 py-1.5 text-sm outline-none focus:border-magma-accent dark:border-white/10"
              >
                <option value="">{t("settings.importAuthorNoteNone")}</option>
                {notes.map((n) => (
                  <option key={n.path} value={n.path}>
                    {n.title} — {n.path}
                  </option>
                ))}
              </select>
              <button
                onClick={runImport}
                disabled={impBusy || !impUrl.trim()}
                className="self-start rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-50"
              >
                {impBusy ? t("settings.importing") : t("settings.importRun")}
              </button>
              {/* Fetching has no denominator — WordPress only reveals the total
                  when pagination runs out — so that half is an indeterminate
                  bar with a live count, and writing is a real one. Either way
                  something moves, which is the whole point. */}
              {impBusy && (
                <div className="flex flex-col gap-1">
                  <div className="h-1 w-full overflow-hidden rounded-full bg-black/10 dark:bg-white/10">
                    {impProgress?.total ? (
                      <div
                        className="h-full rounded-full bg-magma-accent transition-[width] duration-300"
                        style={{
                          width: `${Math.round((impProgress.done / impProgress.total) * 100)}%`,
                        }}
                      />
                    ) : (
                      <div className="h-full w-1/3 animate-pulse rounded-full bg-magma-accent" />
                    )}
                  </div>
                  <p className="text-xs text-magma-muted">
                    {!impProgress
                      ? t("settings.importConnecting")
                      : impProgress.stage === "writing"
                        ? t("settings.importWriting", {
                            done: String(impProgress.done),
                            total: String(impProgress.total ?? 0),
                          })
                        : impProgress.stage === "authors"
                          ? t("settings.importAuthorLookup", {
                              done: String(impProgress.done),
                              total: String(impProgress.total ?? 0),
                            })
                          : impProgress.stage === "preparing"
                            ? t("settings.importPreparing", {
                                done: String(impProgress.done),
                              })
                            : t("settings.importFetching", { done: String(impProgress.done) })}
                  </p>
                </div>
              )}
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

        )}

        {tab === "claude" && (
        /* Connect Claude and Codex through MCP */
        <>
        <section className="mb-6">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.semanticTitle")}
          </label>
          <p className="mb-2 text-xs leading-relaxed text-magma-muted">
            {t("settings.semanticBody")}
          </p>

          {model?.ready ? (
            <>
              <p className="mb-2 text-xs text-green-600 dark:text-green-400">
                {t("settings.semanticReady", { model: model.model })}
              </p>
              <p className="mb-2 text-xs leading-relaxed text-magma-muted">
                {t("settings.indexBody")}
              </p>
              <button
                onClick={runIndex}
                disabled={indexBusy || !vault}
                className="rounded-lg border border-black/10 px-3 py-1.5 text-sm text-magma-muted transition hover:border-black/20 hover:text-magma-ink disabled:opacity-50 dark:border-white/15 dark:hover:border-white/30"
              >
                {indexBusy ? t("settings.indexBusy") : t("settings.indexRun")}
              </button>
              {indexProg && (
                <div className="mt-2">
                  <div className="h-1 w-full overflow-hidden rounded bg-black/10 dark:bg-white/10">
                    <div
                      className="h-full bg-magma-accent transition-all"
                      style={{
                        width: indexProg.total
                          ? `${Math.round((indexProg.done / indexProg.total) * 100)}%`
                          : "0%",
                      }}
                    />
                  </div>
                  <p className="mt-1 text-xs text-magma-muted">
                    {t("settings.indexProgress", {
                      done: String(indexProg.done),
                      total: String(indexProg.total),
                    })}
                    {indexEta(indexProg) !== null &&
                      " · " + t("settings.indexEta", { minutes: String(indexEta(indexProg)) })}
                  </p>
                </div>
              )}
              {indexDone !== null && !indexBusy && (
                indexDone.passages === 0 ? (
                  // Not a success. A vault the user just pointed at holding no
                  // passages is the most informative thing this can report, and
                  // it was dressed up as the happy path. It names the folder it
                  // read, because the vault the app has open and the one in the
                  // settings file need not be the same — and it names what it
                  // saw at each stage, because "0" alone cannot tell an empty
                  // listing from files it could not open.
                  <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
                    {indexDone.notes > 0 && indexDone.offline === indexDone.notes
                      ? // Every single note a placeholder is not an ambiguous
                        // state, it is a diagnosis, and it has one remedy. The
                        // general message below asks the user to check three
                        // things; this one already knows which of them it is,
                        // and hedging here sent a real diagnosis down two wrong
                        // paths before the counts existed.
                        t("settings.indexAllOffline", {
                          notes: String(indexDone.notes),
                          vault: vault ?? "?",
                        })
                      : t("settings.indexEmpty", { vault: vault ?? "?" }) +
                        " " +
                        t("settings.indexSaw", {
                          notes: String(indexDone.notes),
                          unreadable: String(indexDone.unreadable),
                          offline: String(indexDone.offline),
                        })}
                  </p>
                ) : (
                  <>
                    <p className="mt-2 text-xs text-green-600 dark:text-green-400">
                      {t("settings.indexDone", { count: String(indexDone.passages) })}
                    </p>
                    {indexDone.offline > 0 && (
                      // Success with most of the vault missing is the same trap
                      // one level up: 29 passages out of 777 notes reads as
                      // "done" and is not. Whatever was skipped has to be said
                      // out loud, or the search quietly answers from a fraction
                      // of the vault and nothing looks wrong.
                      <p className="mt-1 text-xs text-amber-600 dark:text-amber-400">
                        {t("settings.indexSkipped", {
                          offline: String(indexDone.offline),
                          notes: String(indexDone.notes),
                        })}
                      </p>
                    )}
                  </>
                )
              )}
              {modelErr && (
                <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">{modelErr}</p>
              )}
            </>
          ) : !vault ? (
            <p className="text-xs text-magma-muted opacity-80">{t("settings.codexMcpNoVault")}</p>
          ) : (
            <>
              <button
                onClick={fetchModel}
                disabled={modelBusy || !hasTauri}
                className="rounded-lg border border-black/10 px-3 py-1.5 text-sm text-magma-muted transition hover:border-black/20 hover:text-magma-ink disabled:opacity-50 dark:border-white/15 dark:hover:border-white/30"
              >
                {modelBusy
                  ? t("settings.semanticBusy")
                  : sizes?.bytes
                    ? t("settings.semanticDownload", {
                        size: String(Math.round(sizes.bytes / 1_000_000)),
                      })
                    : t("settings.semanticDownloadUnknown")}
              </button>
              {modelProg && (
                <div className="mt-2">
                  <div className="h-1 w-full overflow-hidden rounded bg-black/10 dark:bg-white/10">
                    <div
                      className="h-full bg-magma-accent transition-all"
                      style={{
                        width: modelProg.total
                          ? `${Math.round((modelProg.done / modelProg.total) * 100)}%`
                          : "100%",
                      }}
                    />
                  </div>
                  <p className="mt-1 text-xs text-magma-muted">
                    {t("settings.semanticProgress", {
                      file: modelProg.file,
                      done: String(Math.round(modelProg.done / 1_000_000)),
                      total: modelProg.total
                        ? String(Math.round(modelProg.total / 1_000_000))
                        : "?",
                    })}
                  </p>
                </div>
              )}
              {modelErr && (
                <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">{modelErr}</p>
              )}
            </>
          )}
        </section>

        <section className="mb-6">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.rerankTitle")}
          </label>
          <p className="mb-2 text-xs leading-relaxed text-magma-muted">
            {t("settings.rerankBody")}
          </p>
          {model?.rerankReady ? (
            <p className="text-xs text-green-600 dark:text-green-400">
              {t("settings.rerankReady", { model: model.rerankModel })}
            </p>
          ) : (
            <button
              onClick={fetchReranker}
              disabled={rerankBusy || modelBusy || !hasTauri}
              className="rounded-lg border border-black/10 px-3 py-1.5 text-sm text-magma-muted transition hover:border-black/20 hover:text-magma-ink disabled:opacity-50 dark:border-white/15 dark:hover:border-white/30"
            >
              {rerankBusy
                ? t("settings.semanticBusy")
                : sizes?.rerankBytes
                  ? t("settings.rerankDownload", {
                      size: String(Math.round(sizes.rerankBytes / 1_000_000)),
                    })
                  : t("settings.rerankDownloadUnknown")}
            </button>
          )}
        </section>

        <section className="mb-6">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.connectTitle")}
          </label>
          <p className="mb-2 text-xs text-magma-muted">{t("settings.connectBody")}</p>

          {!vault ? (
            <p className="text-xs text-magma-muted opacity-80">{t("settings.codexMcpNoVault")}</p>
          ) : (
            <>
              <button
                onClick={install}
                disabled={mcpBusy}
                className="rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-50"
              >
                {mcpBusy ? t("settings.mcpInstalling") : t("settings.mcpInstall")}
              </button>
              {mcpWarn && (
                <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">{mcpWarn}</p>
              )}
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
        <section className="mb-5 border-t border-black/10 pt-5 dark:border-white/10">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.codexConnectTitle")}
          </label>
          <p className="mb-2 text-xs text-magma-muted">{t("settings.codexConnectBody")}</p>

          {!vault ? (
            <p className="text-xs text-magma-muted opacity-80">{t("settings.mcpNoVault")}</p>
          ) : (
            <>
              <button
                onClick={installCodex}
                disabled={codexBusy}
                className="rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-50"
              >
                {codexBusy ? t("settings.codexMcpInstalling") : t("settings.codexMcpInstall")}
              </button>
              {codexWarn && (
                <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">{codexWarn}</p>
              )}
              {codexDone && (
                <p className="mt-2 text-xs text-green-600 dark:text-green-400">
                  {t("settings.codexMcpInstalled", { path: codexDone })}
                </p>
              )}
              {codexErr && <p className="mt-2 text-xs text-red-500">{codexErr}</p>}

              <button
                onClick={() => setShowCodexManual((v) => !v)}
                className="mt-2 block text-xs text-magma-muted underline-offset-2 hover:text-magma-accent hover:underline"
              >
                {t("settings.codexMcpManual")}
              </button>
              {showCodexManual && codexConfigText && (
                <pre className="mt-2 overflow-auto rounded-lg bg-black/[0.05] p-3 text-xs leading-relaxed dark:bg-black/40">
                  <code>{codexConfigText}</code>
                </pre>
              )}
            </>
          )}
        </section>
        </>

        )}

        {tab === "about" && (
        /* About */
        <section className="flex flex-col items-center gap-2 rounded-xl bg-black/[0.03] p-6 text-center dark:bg-white/[0.04]">
          <MagmaMark size={56} />
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
          <div className="mt-3 w-full max-w-md border-t border-black/10 pt-4 dark:border-white/10">
            <div className="mb-2 text-xs font-medium uppercase tracking-wide text-magma-muted">
              {t("settings.updateTitle")}
            </div>
            <p className="mb-3 text-xs leading-relaxed text-magma-muted">
              {t("settings.updateBody")}
            </p>
            <div className="flex justify-center gap-2">
              <button
                onClick={checkUpdates}
                disabled={updateBusy}
                className="rounded-lg border border-black/10 px-3 py-1.5 text-sm text-magma-muted transition hover:border-black/20 hover:text-magma-ink disabled:opacity-50 dark:border-white/15 dark:hover:border-white/30"
              >
                {updateBusy ? t("settings.updateBusy") : t("settings.updateCheck")}
              </button>
              <button
                onClick={installUpdate}
                disabled={updateBusy || !hasTauri}
                className="rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:opacity-50"
              >
                {availableUpdate
                  ? t("settings.updateInstall", { version: availableUpdate.version })
                  : t("settings.updateInstallLatest")}
              </button>
            </div>
            {updateInfo && (
              <p className="mt-2 whitespace-pre-line text-xs text-green-600 dark:text-green-400">
                {updateInfo}
              </p>
            )}
            {updateErr && <p className="mt-2 text-xs text-red-500">{updateErr}</p>}
          </div>
        </section>
        )}
        </div>

        {/* Everything that leaves this dialog lives down here, where it is hard
            to miss — no small ✕ in a corner. Appearance and language preview
            live, so "save" is what actually commits them. */}
        <footer className="flex items-center gap-2 border-t border-black/5 px-6 py-3.5 dark:border-white/5">
          <button
            onClick={() => {
              resetDefaults();
              resetPrefs();
            }}
            title={t("settings.resetHint")}
            className="whitespace-nowrap rounded-lg border border-black/10 px-3 py-1.5 text-sm text-magma-muted transition hover:border-black/20 hover:text-magma-ink dark:border-white/15 dark:hover:border-white/30"
          >
            {t("settings.reset")}
          </button>
          {/* A dot rather than a sentence: the row is narrow, and "Discard"
              already spells out that something is pending. */}
          <span className="flex flex-1 items-center justify-end gap-1.5 px-1 text-xs text-magma-muted">
            {savedNote ? (
              t("settings.saved")
            ) : dirty ? (
              <>
                <span
                  className="h-1.5 w-1.5 shrink-0 rounded-full bg-magma-accent"
                  aria-hidden
                />
                <span className="truncate">{t("settings.unsaved")}</span>
              </>
            ) : null}
          </span>
          <button
            onClick={close}
            className="whitespace-nowrap rounded-lg px-4 py-1.5 text-sm text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
          >
            {dirty ? t("settings.discard") : t("settings.close")}
          </button>
          <button
            onClick={saveAll}
            disabled={!dirty}
            className="whitespace-nowrap rounded-lg bg-magma-accent px-4 py-1.5 text-sm font-medium text-white transition hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-40"
          >
            {t("settings.save")}
          </button>
        </footer>
        </div>
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
