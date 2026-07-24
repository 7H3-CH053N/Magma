import { useState } from "react";
import FlameIcon from "./FlameIcon";
import { useI18n, type Lang } from "../lib/i18n";
import type { RemoteConfig } from "../lib/api";

interface SettingsProps {
  onClose: () => void;
  vault: string | null;
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

const MCP_CONFIG = (vault: string) =>
  JSON.stringify(
    {
      mcpServers: {
        magma: {
          command: "magma-mcp",
          args: [vault],
        },
      },
    },
    null,
    2
  );

/**
 * Settings dialog: language, an About panel (icon, name, version, build,
 * license/copyright), and a copy-paste MCP config to connect Claude.
 */
export default function Settings({
  onClose,
  vault,
  onConnectRemote,
  remoteActive,
}: SettingsProps) {
  const { t, lang, setLang } = useI18n();
  const initial = savedRemote();
  const [url, setUrl] = useState(initial.url);
  const [username, setUsername] = useState(initial.username);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

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

        {/* Connect to Claude */}
        <section className="mb-5">
          <label className="mb-1 block text-xs font-medium uppercase tracking-wide text-magma-muted">
            {t("settings.connectTitle")}
          </label>
          <p className="mb-2 text-xs text-magma-muted">{t("settings.connectBody")}</p>
          <pre className="overflow-auto rounded-lg bg-black/[0.05] p-3 text-xs leading-relaxed dark:bg-black/40">
            <code>{MCP_CONFIG(vault ?? "/path/to/your/vault")}</code>
          </pre>
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
