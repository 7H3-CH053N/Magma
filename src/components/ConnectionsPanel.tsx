import { useCallback, useEffect, useState } from "react";
import {
  linkMentions,
  outgoingLinks,
  relatedNotes,
  unlinkedMentions,
  type Mention,
  type NoteMeta,
  type OutgoingLink,
  type RelatedNote,
} from "../lib/api";
import { useI18n } from "../lib/i18n";
import { ChevronIcon } from "./Icons";

type Tab = "backlinks" | "outgoing" | "mentions" | "related";

interface ConnectionsPanelProps {
  vault: string | null;
  /** Path of the open note; null hides the panel. */
  path: string | null;
  /** Name used when linking a mention (the note's filename stem). */
  name: string;
  backlinks: NoteMeta[];
  onSelect: (path: string) => void;
  onOpenByName: (name: string) => void;
  /** Called after mentions were linked, so the note and graph reload. */
  onChanged: () => void;
}

/**
 * Everything around the open note, under the editor: what links here, what it
 * links to, who names it without linking, and what reads like it.
 *
 * The panel loads a tab's data the first time that tab is opened. Unlinked
 * mentions and related notes both scan every note in the vault, and doing that
 * on every keystroke of an autosave would make the editor stutter.
 */
export default function ConnectionsPanel({
  vault,
  path,
  name,
  backlinks,
  onSelect,
  onOpenByName,
  onChanged,
}: ConnectionsPanelProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(true);
  const [tab, setTab] = useState<Tab>("backlinks");
  const [outgoing, setOutgoing] = useState<OutgoingLink[] | null>(null);
  const [mentions, setMentions] = useState<Mention[] | null>(null);
  const [related, setRelated] = useState<RelatedNote[] | null>(null);
  const [busy, setBusy] = useState(false);

  // A different note means everything loaded here is stale.
  useEffect(() => {
    setOutgoing(null);
    setMentions(null);
    setRelated(null);
    setTab("backlinks");
  }, [path]);

  const load = useCallback(
    async (which: Tab) => {
      if (!vault || !path) return;
      setBusy(true);
      try {
        if (which === "outgoing" && outgoing === null) {
          setOutgoing(await outgoingLinks(vault, path));
        } else if (which === "mentions" && mentions === null) {
          setMentions(await unlinkedMentions(vault, path));
        } else if (which === "related" && related === null) {
          setRelated(await relatedNotes(vault, path));
        }
      } catch {
        // An empty tab is better than a broken panel; the note stays readable.
        if (which === "outgoing") setOutgoing([]);
        if (which === "mentions") setMentions([]);
        if (which === "related") setRelated([]);
      } finally {
        setBusy(false);
      }
    },
    [vault, path, outgoing, mentions, related]
  );

  const show = (which: Tab) => {
    setTab(which);
    setOpen(true);
    void load(which);
  };

  const link = async (mention: Mention) => {
    if (!vault) return;
    setBusy(true);
    try {
      await linkMentions(vault, mention.path, name);
      setMentions((m) => (m ?? []).filter((x) => x.path !== mention.path));
      onChanged();
    } finally {
      setBusy(false);
    }
  };

  const linkAll = async () => {
    if (!vault || !mentions?.length) return;
    setBusy(true);
    try {
      for (const m of mentions) await linkMentions(vault, m.path, name);
      setMentions([]);
      onChanged();
    } finally {
      setBusy(false);
    }
  };

  if (!path) return null;

  const counts: Record<Tab, number | null> = {
    backlinks: backlinks.length,
    outgoing: outgoing?.length ?? null,
    mentions: mentions?.length ?? null,
    related: related?.length ?? null,
  };

  return (
    <div className="shrink-0 border-t border-black/5 dark:border-white/10">
      <div className="flex items-center gap-1 px-6 py-1.5">
        <button
          onClick={() => setOpen((o) => !o)}
          className="grid h-6 w-6 place-items-center rounded text-magma-muted transition hover:bg-black/5 dark:hover:bg-white/10"
          aria-label={t("connections.toggle")}
        >
          <ChevronIcon size={12} open={open} />
        </button>
        {(
          [
            ["backlinks", t("connections.backlinks")],
            ["outgoing", t("connections.outgoing")],
            ["mentions", t("connections.mentions")],
            ["related", t("connections.related")],
          ] as [Tab, string][]
        ).map(([id, label]) => (
          <button
            key={id}
            onClick={() => show(id)}
            className={`rounded-md px-2 py-1 text-xs transition ${
              tab === id && open
                ? "bg-magma-accent/12 text-magma-accent"
                : "text-magma-muted hover:bg-black/5 dark:hover:bg-white/10"
            }`}
          >
            {label}
            {counts[id] !== null && (
              <span className="ml-1.5 opacity-70">{counts[id]}</span>
            )}
          </button>
        ))}
      </div>

      {open && (
        <div className="max-h-[33vh] overflow-auto px-6 pb-3">
          {tab === "backlinks" &&
            (backlinks.length === 0 ? (
              <Empty text={t("connections.noBacklinks")} />
            ) : (
              <Chips>
                {backlinks.map((b) => (
                  <Chip key={b.path} onClick={() => onSelect(b.path)}>
                    {b.title}
                  </Chip>
                ))}
              </Chips>
            ))}

          {tab === "outgoing" &&
            (outgoing === null ? (
              <Empty text={busy ? t("connections.loading") : ""} />
            ) : outgoing.length === 0 ? (
              <Empty text={t("connections.noOutgoing")} />
            ) : (
              <Chips>
                {outgoing.map((l) => (
                  <Chip
                    key={l.name}
                    muted={l.missing}
                    onClick={() =>
                      l.missing ? onOpenByName(l.name) : onSelect(l.path)
                    }
                    title={l.missing ? t("connections.missingHint") : undefined}
                  >
                    {l.title || l.name}
                  </Chip>
                ))}
              </Chips>
            ))}

          {tab === "mentions" &&
            (mentions === null ? (
              <Empty text={busy ? t("connections.loading") : ""} />
            ) : mentions.length === 0 ? (
              <Empty text={t("connections.noMentions")} />
            ) : (
              <>
                <div className="mb-2 flex items-center gap-2">
                  <p className="flex-1 text-xs text-magma-muted">
                    {t("connections.mentionsHint")}
                  </p>
                  <button
                    onClick={linkAll}
                    disabled={busy}
                    className="shrink-0 rounded-md bg-magma-accent px-2 py-1 text-xs font-medium text-white transition hover:opacity-90 disabled:opacity-50"
                  >
                    {t("connections.linkAll", { count: String(mentions.length) })}
                  </button>
                </div>
                <ul className="space-y-1">
                  {mentions.map((m) => (
                    <li
                      key={m.path}
                      className="group flex items-center gap-2 rounded-md px-2 py-1 hover:bg-black/5 dark:hover:bg-white/10"
                    >
                      <button
                        onClick={() => onSelect(m.path)}
                        className="min-w-0 flex-1 text-left"
                      >
                        <span className="block truncate text-sm">{m.title}</span>
                        <span className="block truncate text-xs text-magma-muted">
                          {m.snippet}
                        </span>
                      </button>
                      <button
                        onClick={() => link(m)}
                        disabled={busy}
                        className="shrink-0 rounded-md border border-black/10 px-2 py-1 text-xs text-magma-muted transition hover:border-magma-accent hover:text-magma-accent disabled:opacity-50 dark:border-white/15"
                      >
                        {t("connections.link")}
                      </button>
                    </li>
                  ))}
                </ul>
              </>
            ))}

          {tab === "related" &&
            (related === null ? (
              <Empty text={busy ? t("connections.loading") : ""} />
            ) : related.length === 0 ? (
              <Empty text={t("connections.noRelated")} />
            ) : (
              <ul className="space-y-0.5">
                {related.map((r) => (
                  <li key={r.path}>
                    <button
                      onClick={() => onSelect(r.path)}
                      className="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left transition hover:bg-black/5 dark:hover:bg-white/10"
                    >
                      {/* A bar reads faster than a number, and the number
                          itself (a cosine) means nothing to anyone. */}
                      <span className="h-1 w-10 shrink-0 overflow-hidden rounded-full bg-black/10 dark:bg-white/15">
                        <span
                          className="block h-full rounded-full bg-magma-accent"
                          style={{ width: `${Math.min(100, r.score * 140)}%` }}
                        />
                      </span>
                      <span className="min-w-0 flex-1 truncate text-sm">{r.title}</span>
                      {r.linked && (
                        <span className="shrink-0 text-[10px] uppercase tracking-wide text-magma-muted">
                          {t("connections.alreadyLinked")}
                        </span>
                      )}
                    </button>
                  </li>
                ))}
              </ul>
            ))}
        </div>
      )}
    </div>
  );
}

function Empty({ text }: { text: string }) {
  if (!text) return null;
  return <p className="py-2 text-xs text-magma-muted">{text}</p>;
}

function Chips({ children }: { children: React.ReactNode }) {
  return <div className="flex flex-wrap gap-2">{children}</div>;
}

function Chip({
  children,
  onClick,
  muted,
  title,
}: {
  children: React.ReactNode;
  onClick: () => void;
  muted?: boolean;
  title?: string;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`rounded-md px-2 py-1 text-sm transition ${
        muted
          ? "border border-dashed border-black/20 text-magma-muted hover:text-magma-ink dark:border-white/20"
          : "bg-black/5 text-magma-ink hover:bg-black/10 dark:bg-white/10 dark:text-[#ece9e4] dark:hover:bg-white/20"
      }`}
    >
      {children}
    </button>
  );
}
