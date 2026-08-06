import { useEffect, useMemo, useRef, useState } from "react";
import type { Command } from "../components/CommandPalette";
import type { NoteMeta, VaultPluginBundle } from "./api";

export interface PluginManifest {
  id: string;
  nameKey: string;
  descriptionKey: string;
  author: string;
}

export interface PluginListItem {
  id: string;
  name: string;
  description: string;
  author: string;
  source: "core" | "vault";
  version?: string;
  error?: string;
}

export interface ExternalPluginManifest {
  id: string;
  name: string;
  description: string;
  author?: string;
  version?: string;
}

export interface PluginContext {
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

export function corePluginList(t: PluginContext["t"]): PluginListItem[] {
  return CORE_PLUGINS.map((plugin) => ({
    id: plugin.manifest.id,
    name: t(plugin.manifest.nameKey),
    description: t(plugin.manifest.descriptionKey),
    author: plugin.manifest.author,
    source: "core",
  }));
}

export function parseExternalManifest(bundle: VaultPluginBundle): PluginListItem {
  const manifest = bundle.manifest;
  if (!manifest || typeof manifest !== "object") {
    return invalidExternal(bundle.id, "manifest.json is not an object");
  }
  const raw = manifest as Record<string, unknown>;
  const id = typeof raw.id === "string" ? raw.id : bundle.id;
  const name = typeof raw.name === "string" ? raw.name : id;
  const description =
    typeof raw.description === "string" ? raw.description : "No description provided.";
  const author = typeof raw.author === "string" ? raw.author : "Local plugin";
  const version = typeof raw.version === "string" ? raw.version : undefined;
  if (id !== bundle.id) {
    return invalidExternal(bundle.id, "manifest id must match the plugin folder name");
  }
  return { id, name, description, author, version, source: "vault" };
}

function invalidExternal(id: string, error: string): PluginListItem {
  return {
    id,
    name: id,
    description: error,
    author: "Local plugin",
    source: "vault",
    error,
  };
}

interface WorkerCommand {
  id: string;
  label: string;
  hint?: string;
}

interface ExternalRuntimeProps {
  bundles: VaultPluginBundle[];
  enabledIds: string[];
  context: Omit<PluginContext, "t" | "openNote" | "notice">;
  t: PluginContext["t"];
  openNote: PluginContext["openNote"];
  notice: PluginContext["notice"];
}

interface WorkerState {
  pluginId: string;
  worker: Worker;
  commands: WorkerCommand[];
}

export function useExternalPluginCommands({
  bundles,
  enabledIds,
  context,
  t,
  openNote,
  notice,
}: ExternalRuntimeProps): Command[] {
  const [workers, setWorkers] = useState<WorkerState[]>([]);
  const contextRef = useRef(context);
  const hostRef = useRef({ openNote, notice, t });

  useEffect(() => {
    contextRef.current = context;
  }, [context]);

  useEffect(() => {
    hostRef.current = { openNote, notice, t };
  }, [openNote, notice, t]);

  useEffect(() => {
    const enabled = new Set(enabledIds);
    const active = bundles.filter((bundle) => enabled.has(bundle.id));
    const nextWorkers: WorkerState[] = [];
    const urls: string[] = [];

    for (const bundle of active) {
      const item = parseExternalManifest(bundle);
      if (item.error) {
        notice(t("plugins.externalLoadErrorTitle"), `${bundle.id}: ${item.error}`);
        continue;
      }
      try {
        const source = workerSource(bundle.id, bundle.source);
        const blob = new Blob([source], { type: "text/javascript" });
        const url = URL.createObjectURL(blob);
        urls.push(url);
        const worker = new Worker(url);
        const state: WorkerState = { pluginId: bundle.id, worker, commands: [] };
        nextWorkers.push(state);
        worker.onmessage = (event: MessageEvent) => {
          const msg = event.data;
          if (!msg || typeof msg !== "object") return;
          if (msg.type === "registerCommand" && isWorkerCommand(msg.command)) {
            state.commands = [...state.commands, msg.command];
            setWorkers([...nextWorkers]);
            return;
          }
          if (msg.type === "hostRequest") {
            void handleHostRequest(worker, msg.requestId, msg.method, msg.args, {
              openNote: hostRef.current.openNote,
              notice: hostRef.current.notice,
              context: contextRef.current,
            });
          }
        };
        worker.onerror = (event) => {
          notice(t("plugins.externalLoadErrorTitle"), `${bundle.id}: ${event.message}`);
        };
      } catch (e) {
        notice(t("plugins.externalLoadErrorTitle"), `${bundle.id}: ${String(e)}`);
      }
    }

    setWorkers(nextWorkers);
    return () => {
      for (const state of nextWorkers) state.worker.terminate();
      for (const url of urls) URL.revokeObjectURL(url);
    };
  }, [bundles, enabledIds, notice, t]);

  return useMemo(
    () =>
      workers.flatMap((state) =>
        state.commands.map((command) => ({
          id: `plugin:${state.pluginId}:${command.id}`,
          label: command.label,
          hint: command.hint || t("plugins.commandHint"),
          run: () => {
            state.worker.postMessage({
              type: "runCommand",
              commandId: command.id,
              context: contextRef.current,
            });
          },
        }))
      ),
    [workers, t]
  );
}

function isWorkerCommand(value: unknown): value is WorkerCommand {
  if (!value || typeof value !== "object") return false;
  const command = value as Record<string, unknown>;
  return typeof command.id === "string" && typeof command.label === "string";
}

async function handleHostRequest(
  worker: Worker,
  requestId: number,
  method: unknown,
  args: unknown,
  host: {
    openNote: PluginContext["openNote"];
    notice: PluginContext["notice"];
    context: Omit<PluginContext, "t" | "openNote" | "notice">;
  }
) {
  try {
    let result: unknown = null;
    if (method === "notice") {
      const [title, detail] = Array.isArray(args) ? args : [];
      host.notice(String(title ?? ""), detail === undefined ? undefined : String(detail));
    } else if (method === "openNote") {
      const [path] = Array.isArray(args) ? args : [];
      if (typeof path !== "string") throw new Error("openNote requires a path");
      host.openNote(path);
    } else if (method === "getContext") {
      result = host.context;
    } else {
      throw new Error(`Unknown plugin API method: ${String(method)}`);
    }
    worker.postMessage({ type: "hostResponse", requestId, result });
  } catch (e) {
    worker.postMessage({ type: "hostResponse", requestId, error: String(e) });
  }
}

function workerSource(pluginId: string, pluginSource: string): string {
  return `
const handlers = new Map();
const pending = new Map();
let nextRequestId = 1;
let currentContext = { notes: [], activePath: null, content: "" };

function request(method, args) {
  const requestId = nextRequestId++;
  self.postMessage({ type: "hostRequest", requestId, method, args });
  return new Promise((resolve, reject) => {
    pending.set(requestId, { resolve, reject });
  });
}

self.magma = {
  pluginId: ${JSON.stringify(pluginId)},
  registerCommand(command, handler) {
    if (!command || typeof command.id !== "string" || typeof command.label !== "string") {
      throw new Error("registerCommand needs { id, label }");
    }
    if (typeof handler !== "function") {
      throw new Error("registerCommand needs a function handler");
    }
    handlers.set(command.id, handler);
    self.postMessage({ type: "registerCommand", command: {
      id: command.id,
      label: command.label,
      hint: command.hint
    }});
  },
  notice(title, detail) {
    return request("notice", [title, detail]);
  },
  openNote(path) {
    return request("openNote", [path]);
  },
  getContext() {
    return request("getContext", []);
  }
};

self.onmessage = async (event) => {
  const msg = event.data;
  if (!msg || typeof msg !== "object") return;
  if (msg.type === "hostResponse") {
    const pendingRequest = pending.get(msg.requestId);
    if (!pendingRequest) return;
    pending.delete(msg.requestId);
    if (msg.error) pendingRequest.reject(new Error(msg.error));
    else pendingRequest.resolve(msg.result);
    return;
  }
  if (msg.type === "runCommand") {
    currentContext = msg.context || currentContext;
    const handler = handlers.get(msg.commandId);
    if (!handler) {
      await self.magma.notice("Plugin error", "Unknown command: " + msg.commandId);
      return;
    }
    try {
      await handler(currentContext);
    } catch (error) {
      await self.magma.notice("Plugin error", String(error && error.message ? error.message : error));
    }
  }
};

${pluginSource}
`;
}
