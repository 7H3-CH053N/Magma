import FlameIcon from "./FlameIcon";

interface SettingsProps {
  onClose: () => void;
}

/**
 * Settings dialog. For now it centers on the About panel — icon, name, version,
 * build, and the license/copyright notice.
 */
export default function Settings({ onClose }: SettingsProps) {
  return (
    <div
      className="fixed inset-0 z-40 grid place-items-center bg-black/40 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-md rounded-2xl bg-magma-bg p-6 shadow-xl dark:bg-[#201c19]"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-5 flex items-center justify-between">
          <h2 className="text-base font-semibold">Settings</h2>
          <button
            onClick={onClose}
            className="grid h-7 w-7 place-items-center rounded-md text-magma-muted hover:bg-black/10 dark:hover:bg-white/10"
            aria-label="Close"
          >
            ✕
          </button>
        </div>

        <section className="flex flex-col items-center gap-2 rounded-xl bg-black/[0.03] p-6 text-center dark:bg-white/[0.04]">
          <FlameIcon size={56} />
          <div className="text-lg font-semibold tracking-tight">Magma</div>
          <div className="text-sm text-magma-muted">
            Version {__APP_VERSION__} · Build {__BUILD_ID__}
          </div>
          <p className="mt-1 max-w-xs text-xs leading-relaxed text-magma-muted">
            A beautiful, local-first, LLM-native second brain. Your notes stay
            plain markdown files on your disk.
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
            <div className="mt-1 opacity-80">
              Released under the terms in the project LICENSE.
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
