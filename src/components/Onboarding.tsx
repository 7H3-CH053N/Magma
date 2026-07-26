import { useState } from "react";
import FlameIcon from "./FlameIcon";
import { useI18n } from "../lib/i18n";

interface OnboardingProps {
  onOpenVault: () => void;
  onOpenSettings: () => void;
  onClose: () => void;
}

const SEEN_KEY = "magma.onboarded";

/** Whether the first-run tour has already been dismissed. */
export function onboardingSeen(): boolean {
  try {
    return localStorage.getItem(SEEN_KEY) === "1";
  } catch {
    return true;
  }
}

export function markOnboarded() {
  try {
    localStorage.setItem(SEEN_KEY, "1");
  } catch {
    /* ignore */
  }
}

/**
 * Three cards on first run: pick a folder, learn the two keys that matter,
 * connect Claude. Deliberately not a tutorial — the promise of Magma is that
 * there is nothing to set up, and a ten-step tour would contradict it.
 */
export default function Onboarding({
  onOpenVault,
  onOpenSettings,
  onClose,
}: OnboardingProps) {
  const { t } = useI18n();
  const [step, setStep] = useState(0);
  const steps = [
    { title: t("onboarding.vaultTitle"), body: t("onboarding.vaultBody") },
    { title: t("onboarding.keysTitle"), body: t("onboarding.keysBody") },
    { title: t("onboarding.claudeTitle"), body: t("onboarding.claudeBody") },
  ];
  const last = step === steps.length - 1;

  const finish = () => {
    markOnboarded();
    onClose();
  };

  return (
    <div className="fixed inset-0 z-40 grid place-items-center bg-black/50 p-4">
      <div className="w-full max-w-md rounded-2xl bg-magma-bg p-6 shadow-2xl dark:bg-[#201c19]">
        <div className="mb-4 flex items-center gap-3">
          <FlameIcon size={32} />
          <div>
            <div className="text-base font-semibold tracking-tight">Magma</div>
            <div className="text-xs text-magma-muted">{t("app.tagline")}</div>
          </div>
        </div>

        <h2 className="text-sm font-semibold">{steps[step].title}</h2>
        <p className="mt-1.5 whitespace-pre-line text-sm leading-relaxed text-magma-muted">
          {steps[step].body}
        </p>

        {step === 0 && (
          <button
            onClick={() => {
              onOpenVault();
              setStep(1);
            }}
            className="mt-4 rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90"
          >
            {t("settings.vaultChoose")}
          </button>
        )}
        {last && (
          <button
            onClick={() => {
              markOnboarded();
              onClose();
              onOpenSettings();
            }}
            className="mt-4 rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90"
          >
            {t("onboarding.openClaudeSetup")}
          </button>
        )}

        <div className="mt-6 flex items-center gap-2">
          <div className="flex flex-1 gap-1.5">
            {steps.map((_, i) => (
              <span
                key={i}
                className={`h-1.5 w-1.5 rounded-full transition ${
                  i === step ? "bg-magma-accent" : "bg-black/15 dark:bg-white/20"
                }`}
              />
            ))}
          </div>
          <button
            onClick={finish}
            className="rounded-lg px-3 py-1.5 text-sm text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
          >
            {t("onboarding.skip")}
          </button>
          {!last && (
            <button
              onClick={() => setStep((s) => s + 1)}
              className="rounded-lg bg-magma-accent px-3 py-1.5 text-sm font-medium text-white transition hover:opacity-90"
            >
              {t("onboarding.next")}
            </button>
          )}
          {last && (
            <button
              onClick={finish}
              className="rounded-lg px-3 py-1.5 text-sm font-medium text-magma-accent transition hover:bg-magma-accent/10"
            >
              {t("onboarding.done")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
