import { useEffect, useState } from "react";
import FlameIcon from "./FlameIcon";

/**
 * Startup splash: the flame and the name, then it fades away. Shown as an
 * in-app overlay so it behaves identically on macOS and Windows.
 */
export default function Splash() {
  const [leaving, setLeaving] = useState(false);
  const [gone, setGone] = useState(false);

  useEffect(() => {
    const t1 = window.setTimeout(() => setLeaving(true), 1100);
    const t2 = window.setTimeout(() => setGone(true), 1600);
    return () => {
      window.clearTimeout(t1);
      window.clearTimeout(t2);
    };
  }, []);

  if (gone) return null;

  return (
    <div
      className={`fixed inset-0 z-50 grid place-items-center bg-[#1a1512] transition-opacity duration-500 ${
        leaving ? "opacity-0" : "opacity-100"
      }`}
    >
      <div className="flex flex-col items-center gap-4">
        <div className="animate-pulse">
          <FlameIcon size={72} />
        </div>
        <div className="text-2xl font-semibold tracking-tight text-[#f3ede4]">Magma</div>
        <div className="text-xs text-[#9a8f82]">Your second brain, minus the setup.</div>
      </div>
    </div>
  );
}
