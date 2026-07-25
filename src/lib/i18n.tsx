import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";

export type Lang = "en" | "de";

// Flat key -> string per language. Keep keys grouped by area for readability.
const STRINGS: Record<Lang, Record<string, string>> = {
  en: {
    "app.tagline": "Your second brain, minus the setup.",
    "sidebar.openVault": "Open vault…",
    "sidebar.search": "Search notes…",
    "sidebar.newNote": "New note (Cmd/Ctrl+N)",
    "sidebar.settings": "Settings",
    "sidebar.resize": "Drag to resize · double-click to reset",
    "sidebar.noNotes": "No notes yet.",
    "sidebar.openToBegin": "Open a folder of markdown files to begin.",
    "sidebar.noMatches": "No matches.",
    "sidebar.rename": "Rename",
    "sidebar.delete": "Delete",
    "sidebar.deleteFolder": "Delete folder",
    "sidebar.move": "Move to folder",
    "sidebar.newFolder": "New folder",
    "sidebar.newFolderPrompt": "New folder name",
    "sidebar.movePrompt": "Move to folder (leave empty for root)",
    "view.editor": "Editor",
    "view.graph": "Graph",
    "graph.empty": "No notes to graph yet.",
    "graph.legendNote": "note",
    "graph.legendAi": "AI-written",
    "graph.reset": "Reset view",
    "backlinks.one": "linked mention",
    "backlinks.many": "linked mentions",
    "empty.pickOrCreate": "Pick a note on the left, or create a new one.",
    "empty.openVault":
      "Open a folder of markdown files to start. Your notes stay plain files on your disk — readable by you and, when you turn it on, by Claude.",
    "empty.newNote": "New note",
    "empty.browserPreview":
      "(Running in the browser preview — launch the desktop app for full vault access.)",
    "prompt.rename": "Rename note",
    "confirm.delete": "Delete \"{title}\"?",
    "confirm.deleteFolder": "Delete the folder \"{folder}\" and all {count} notes inside it?",
    "dialog.ok": "OK",
    "dialog.cancel": "Cancel",
    "dialog.delete": "Delete",
    "confirm.undone": "This cannot be undone.",
    "settings.title": "Settings",
    "settings.appearance": "Appearance",
    "settings.theme": "Theme",
    "theme.system": "System",
    "theme.light": "Light",
    "theme.dark": "Dark",
    "settings.accent": "Accent color",
    "settings.aiColor": "AI note color",
    "settings.uiFont": "Interface font",
    "settings.editorFont": "Editor font",
    "settings.fontSize": "Font size",
    "settings.readingWidth": "Reading width",
    "settings.reset": "Reset to defaults",
    "settings.language": "Language",
    "settings.about": "About",
    "settings.version": "Version {version} · Build {build}",
    "settings.description":
      "A beautiful, local-first, LLM-native second brain. Your notes stay plain markdown files on your disk.",
    "settings.license": "Released under the terms in the project LICENSE.",
    "settings.importTitle": "Import WordPress blog",
    "settings.importBody":
      "Pull every post into a folder as linked notes — grouped by category and tag, then searchable and available to Claude.",
    "settings.importFolder": "Target folder",
    "settings.importAuthor": "Author (optional — normally detected automatically)",
    "settings.importAuthors": "author: {authors}",
    "settings.importNoAuthor":
      "Neither the REST API nor the RSS feed named an author for these posts. Enter one above to add bylines and an author note.",
    "settings.importUrl": "Blog URL (e.g. https://myblog.com)",
    "settings.importRun": "Import",
    "settings.importing": "Importing… this can take a minute",
    "settings.importDone": "Imported {count} notes into \"{folder}\".",
    "settings.connectTitle": "Connect to Claude",
    "settings.connectBody":
      "Let Claude read and co-author your vault. One click sets up Claude Desktop for you.",
    "settings.mcpInstall": "Set up Claude Desktop",
    "settings.mcpInstalling": "Setting up…",
    "settings.mcpInstalled": "Done — restart Claude Desktop. Written to {path}",
    "settings.mcpManual": "Manual setup (other MCP clients)",
    "settings.mcpNoVault": "Open a vault first to connect Claude.",
    "settings.close": "Close",
    "settings.remoteTitle": "Remote vault (WebDAV)",
    "settings.remoteBody":
      "Host your vault on a webserver so it's available on every machine with Magma.",
    "settings.remoteActive": "Connected to a remote vault. Edits sync back to the server.",
    "settings.remoteUser": "Username",
    "settings.remotePass": "Password",
    "settings.remoteConnect": "Connect & sync",
    "settings.remoteConnecting": "Syncing…",
    "settings.remoteNote":
      "Requires an https:// WebDAV URL. Notes are cached locally and changes are pushed back on save. The password is kept only for this session.",
  },
  de: {
    "app.tagline": "Dein zweites Gehirn – ohne Einrichtungsaufwand.",
    "sidebar.openVault": "Vault öffnen…",
    "sidebar.search": "Notizen durchsuchen…",
    "sidebar.newNote": "Neue Notiz (Cmd/Ctrl+N)",
    "sidebar.settings": "Einstellungen",
    "sidebar.resize": "Ziehen zum Verbreitern · Doppelklick setzt zurück",
    "sidebar.noNotes": "Noch keine Notizen.",
    "sidebar.openToBegin": "Öffne einen Ordner mit Markdown-Dateien zum Starten.",
    "sidebar.noMatches": "Keine Treffer.",
    "sidebar.rename": "Umbenennen",
    "sidebar.delete": "Löschen",
    "sidebar.deleteFolder": "Ordner löschen",
    "sidebar.move": "In Ordner verschieben",
    "sidebar.newFolder": "Neuer Ordner",
    "sidebar.newFolderPrompt": "Name des neuen Ordners",
    "sidebar.movePrompt": "In Ordner verschieben (leer = Wurzel)",
    "view.editor": "Editor",
    "view.graph": "Graph",
    "graph.empty": "Noch keine Notizen für den Graphen.",
    "graph.legendNote": "Notiz",
    "graph.legendAi": "KI-geschrieben",
    "graph.reset": "Ansicht zurücksetzen",
    "backlinks.one": "verlinkte Erwähnung",
    "backlinks.many": "verlinkte Erwähnungen",
    "empty.pickOrCreate": "Wähle links eine Notiz oder erstelle eine neue.",
    "empty.openVault":
      "Öffne einen Ordner mit Markdown-Dateien. Deine Notizen bleiben einfache Dateien auf deiner Festplatte – lesbar für dich und, wenn du willst, für Claude.",
    "empty.newNote": "Neue Notiz",
    "empty.browserPreview":
      "(Läuft in der Browser-Vorschau – starte die Desktop-App für vollen Vault-Zugriff.)",
    "prompt.rename": "Notiz umbenennen",
    "confirm.delete": "„{title}“ löschen?",
    "confirm.deleteFolder": "Ordner „{folder}“ mit allen {count} Notizen darin löschen?",
    "dialog.ok": "OK",
    "dialog.cancel": "Abbrechen",
    "dialog.delete": "Löschen",
    "confirm.undone": "Das kann nicht rückgängig gemacht werden.",
    "settings.title": "Einstellungen",
    "settings.appearance": "Darstellung",
    "settings.theme": "Modus",
    "theme.system": "System",
    "theme.light": "Hell",
    "theme.dark": "Dunkel",
    "settings.accent": "Akzentfarbe",
    "settings.aiColor": "Farbe KI-Notizen",
    "settings.uiFont": "Oberflächen-Schrift",
    "settings.editorFont": "Editor-Schrift",
    "settings.fontSize": "Schriftgröße",
    "settings.readingWidth": "Lesebreite",
    "settings.reset": "Auf Standard zurücksetzen",
    "settings.language": "Sprache",
    "settings.about": "Über",
    "settings.version": "Version {version} · Build {build}",
    "settings.description":
      "Ein schönes, lokales, LLM-natives zweites Gehirn. Deine Notizen bleiben einfache Markdown-Dateien auf deiner Festplatte.",
    "settings.license": "Veröffentlicht unter den Bedingungen der LICENSE des Projekts.",
    "settings.importTitle": "WordPress-Blog importieren",
    "settings.importBody":
      "Hol alle Beiträge als verlinkte Notizen in einen Ordner — gruppiert nach Kategorie und Tag, dann durchsuchbar und für Claude verfügbar.",
    "settings.importFolder": "Zielordner",
    "settings.importAuthor": "Autor (optional — wird normalerweise automatisch erkannt)",
    "settings.importAuthors": "Autor: {authors}",
    "settings.importNoAuthor":
      "Weder die REST-API noch der RSS-Feed haben einen Autor genannt. Du kannst oben einen eintragen, dann gibt es Autorenzeilen und eine Autor-Notiz.",
    "settings.importUrl": "Blog-URL (z. B. https://meinblog.at)",
    "settings.importRun": "Importieren",
    "settings.importing": "Importiere… das kann eine Minute dauern",
    "settings.importDone": "{count} Notizen in „{folder}“ importiert.",
    "settings.connectTitle": "Mit Claude verbinden",
    "settings.connectBody":
      "Lass Claude deinen Vault lesen und mitschreiben. Ein Klick richtet Claude Desktop für dich ein.",
    "settings.mcpInstall": "Claude Desktop einrichten",
    "settings.mcpInstalling": "Richte ein…",
    "settings.mcpInstalled": "Fertig – starte Claude Desktop neu. Geschrieben nach {path}",
    "settings.mcpManual": "Manuelle Einrichtung (andere MCP-Clients)",
    "settings.mcpNoVault": "Öffne zuerst einen Vault, um Claude zu verbinden.",
    "settings.close": "Schließen",
    "settings.remoteTitle": "Remote-Vault (WebDAV)",
    "settings.remoteBody":
      "Lege deinen Vault auf einem Webserver ab, damit er auf jedem Rechner mit Magma verfügbar ist.",
    "settings.remoteActive": "Mit einem Remote-Vault verbunden. Änderungen werden zum Server synchronisiert.",
    "settings.remoteUser": "Benutzername",
    "settings.remotePass": "Passwort",
    "settings.remoteConnect": "Verbinden & synchronisieren",
    "settings.remoteConnecting": "Synchronisiere…",
    "settings.remoteNote":
      "Erfordert eine https://-WebDAV-URL. Notizen werden lokal zwischengespeichert und Änderungen beim Speichern zurück übertragen. Das Passwort wird nur für diese Sitzung behalten.",
  },
};

interface I18n {
  lang: Lang;
  setLang: (l: Lang) => void;
  t: (key: string, vars?: Record<string, string>) => string;
}

const I18nContext = createContext<I18n | null>(null);
const STORAGE_KEY = "magma.lang";

function detectLang(): Lang {
  const saved = typeof localStorage !== "undefined" ? localStorage.getItem(STORAGE_KEY) : null;
  if (saved === "en" || saved === "de") return saved;
  const nav = typeof navigator !== "undefined" ? navigator.language.toLowerCase() : "en";
  return nav.startsWith("de") ? "de" : "en";
}

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLangState] = useState<Lang>(detectLang);

  const setLang = useCallback((l: Lang) => {
    setLangState(l);
    try {
      localStorage.setItem(STORAGE_KEY, l);
    } catch {
      /* ignore storage errors */
    }
  }, []);

  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);

  const t = useCallback(
    (key: string, vars?: Record<string, string>) => {
      let s = STRINGS[lang][key] ?? STRINGS.en[key] ?? key;
      if (vars) {
        for (const [k, v] of Object.entries(vars)) s = s.replace(`{${k}}`, v);
      }
      return s;
    },
    [lang]
  );

  return <I18nContext.Provider value={{ lang, setLang, t }}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within I18nProvider");
  return ctx;
}
