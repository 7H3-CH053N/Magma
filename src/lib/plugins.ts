import type { Command } from "../components/CommandPalette";
import type { NoteMeta } from "./api";

export interface PluginManifest {
  id: string;
  nameKey: string;
  descriptionKey: string;
  author: string;
}

interface PluginContext {
  notes: NoteMeta[];
  activePath: string | null;
  content: string;
  t: (key: string, vars?: Record<string, string>) => string;
  openNote: (path: string) => void;
  notice: (title: string, detail?: string) => void;
}

interface CorePlugin {
  manifest: PluginManifest;
  commands?: (ctx: PluginContext) => Command[];
}

export const CORE_PLUGINS: CorePlugin[] = [
  {
    manifest: {
      id: "random-note",
      nameKey: "plugins.randomNote.name",
      descriptionKey: "plugins.randomNote.description",
      author: "Magma",
    },
    commands: ({ notes, t, openNote, notice }) => [
      {
        id: "plugin:random-note:open",
        label: t("plugins.randomNote.command"),
        hint: t("plugins.commandHint"),
        run: () => {
          if (notes.length === 0) {
            notice(t("plugins.randomNote.emptyTitle"), t("plugins.randomNote.emptyBody"));
            return;
          }
          const note = notes[Math.floor(Math.random() * notes.length)];
          openNote(note.path);
        },
      },
    ],
  },
  {
    manifest: {
      id: "word-count",
      nameKey: "plugins.wordCount.name",
      descriptionKey: "plugins.wordCount.description",
      author: "Magma",
    },
    commands: ({ activePath, content, t, notice }) => [
      {
        id: "plugin:word-count:active",
        label: t("plugins.wordCount.command"),
        hint: t("plugins.commandHint"),
        run: () => {
          if (!activePath) {
            notice(t("plugins.wordCount.emptyTitle"), t("plugins.wordCount.emptyBody"));
            return;
          }
          const words = content.trim().match(/\S+/g)?.length ?? 0;
          const chars = content.length;
          notice(
            t("plugins.wordCount.resultTitle"),
            t("plugins.wordCount.resultBody", {
              words: String(words),
              chars: String(chars),
            })
          );
        },
      },
    ],
  },
];

export function enabledPlugins(enabledIds: string[]): CorePlugin[] {
  const enabled = new Set(enabledIds);
  return CORE_PLUGINS.filter((plugin) => enabled.has(plugin.manifest.id));
}

export function pluginCommands(enabledIds: string[], ctx: PluginContext): Command[] {
  return enabledPlugins(enabledIds).flatMap((plugin) => plugin.commands?.(ctx) ?? []);
}
