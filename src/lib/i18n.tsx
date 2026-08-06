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
    "sidebar.replace": "Find & replace",
    "sidebar.today": "Today's note",
    "sidebar.newSubfolder": "New subfolder",
    "sidebar.newSubfolderPrompt": "Name of the new folder inside \"{parent}\"",
    "sidebar.moveFolder": "Move folder",
    "sidebar.moveFolderPrompt": "Move \"{folder}\" into which folder? (empty = vault root)",
    "sidebar.moveFolderFailed": "That folder could not be moved.",
    "view.ai": "AI",
    "view.history": "History",
    "palette.placeholder": "Jump to a note, or type a command…",
    "palette.noMatches": "Nothing matches.",
    "cmd.newNote": "New note",
    "cmd.today": "Open today's note",
    "cmd.yesterday": "Open yesterday's note",
    "cmd.capture": "Quick capture",
    "cmd.graph": "Show graph",
    "cmd.editor": "Show editor",
    "cmd.aiReview": "What AI clients wrote",
    "cmd.replace": "Find & replace",
    "cmd.settings": "Settings",
    "cmd.newFolder": "New folder",
    "cmd.openVault": "Open a different vault…",
    "cmd.history": "Version history",
    "cmd.rename": "Rename this note",
    "cmd.move": "Move this note",
    "cmd.fromTemplate": "New note from: {name}",
    "cmd.templateHint": "Template",
    "template.prompt": "Title for the new note from \"{name}\"",
    "calendar.title": "Calendar",
    "calendar.prev": "Previous month",
    "calendar.next": "Next month",
    "capture.placeholder": "What is on your mind?",
    "capture.target": "→ {target}",
    "capture.hint": "Enter saves · Shift+Enter new line",
    "capture.save": "Save",
    "connections.toggle": "Show or hide connections",
    "connections.backlinks": "Backlinks",
    "connections.outgoing": "Links out",
    "connections.mentions": "Mentions",
    "connections.related": "Similar",
    "connections.loading": "Reading the vault…",
    "connections.noBacklinks": "No note links here yet.",
    "connections.noOutgoing": "This note doesn't link anywhere yet.",
    "connections.noMentions": "No note names this one without linking it.",
    "connections.noRelated": "Nothing in the vault reads like this note.",
    "connections.mentionsHint": "These notes say the name but never link it.",
    "connections.link": "Link",
    "connections.linkAll": "Link all {count}",
    "connections.alreadyLinked": "linked",
    "connections.missingHint": "No note of this name yet — click to create it.",
    "history.title": "Version history",
    "history.loading": "Loading…",
    "history.none": "No earlier versions.",
    "history.noneBody": "Magma keeps a copy before every larger change. As soon as this note is edited again, its previous state shows up here.",
    "history.diffHint": "Green is in the note now, red was in this version.",
    "history.restore": "Restore this version",
    "history.close": "Close",
    "ai.title": "What AI clients wrote",
    "ai.subtitle": "{count} notes carry author: ai in their frontmatter.",
    "ai.emptyTitle": "Nothing written by an AI yet.",
    "ai.emptyBody": "Notes AI clients create or edit through MCP are marked in their frontmatter and collected here, so you can see exactly what was touched.",
    "onboarding.vaultTitle": "1 · Pick a folder",
    "onboarding.vaultBody": "Your notes are ordinary markdown files in a folder you choose. Nothing is uploaded, and any other editor can open them — including Obsidian.",
    "onboarding.keysTitle": "2 · Two keys worth knowing",
    "onboarding.keysBody": "⌘/Ctrl+P opens everything: notes, commands, templates.\n⌘/Ctrl+⇧+N catches a thought into today's note without leaving what you're doing.",
    "onboarding.claudeTitle": "3 · Let Claude in (optional)",
    "onboarding.claudeBody": "Magma has an MCP server built in. One click in Settings and Claude can read your notes and write new ones — properly linked. Everything an AI writes is marked and listed under \"AI\".",
    "onboarding.openClaudeSetup": "Open settings",
    "onboarding.next": "Next",
    "onboarding.skip": "Skip",
    "onboarding.done": "Start",
    "settings.tabNotes": "Notes & templates",
    "settings.dailyTitle": "Daily notes",
    "settings.dailyBody": "One note per day, named 2026-07-26 — opened by the calendar in the sidebar and by quick capture.",
    "settings.dailyFolder": "Folder for daily notes",
    "settings.dailyTemplate": "Template for a new day",
    "settings.templateNone": "None — start with just the date",
    "settings.templatesTitle": "Templates",
    "settings.templatesBody": "Every note in this folder shows up in the command palette as \"New note from: …\". Placeholders {{date}}, {{time}}, {{title}}, {{weekday}}, {{month}} and {{year}} are filled in.",
    "settings.templateFolder": "Template folder",
    "settings.captureTitle": "Quick capture",
    "settings.captureToDaily": "Prefix captured lines with the time",
    "settings.captureBody": "⌘/Ctrl+⇧+N appends to today's note without opening it.",
    "replace.title": "Find & replace",
    "replace.find": "Find",
    "replace.with": "Replace with",
    "replace.renameNotes": "Rename matching notes too",
    "replace.renameNotesHint":
      "Wikilinks point at a note's name. Without this, links are rewritten but their target note keeps the old name — and the links break.",
    "replace.typeToPreview": "Type something to search for; you'll see what changes before anything is written.",
    "replace.noMatches": "Nothing in this vault matches.",
    "replace.summary": "{total} occurrences in {notes} notes",
    "replace.renameBadge": "Rename",
    "replace.cancel": "Cancel",
    "replace.apply": "Replace in {notes} notes",
    "replace.applying": "Replacing…",
    "replace.done": "{total} occurrences replaced in {notes} notes.",
    "view.editor": "Editor",
    "view.graph": "Graph",
    "graph.empty": "No notes to graph yet.",
    "graph.legendNote": "note",
    "graph.legendAi": "AI-written",
    "graph.reset": "Reset view",
    "graph.colors": "Colours",
    "graph.colorsHint": "One colour per top-level folder; subfolders shade it.",
    "graph.colorsReset": "Back to default colours",
    "graph.aiRing": "Ring around AI-written notes",
    "graph.legendMissing": "not created yet",
    "graph.openEditor": "Open in editor",
    "graph.createNote": "Create this note",
    "graph.previewMissing": "No note of this name exists yet — something links to it.",
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
    "settings.tabVault": "Vault & sync",
    "settings.tabAppearance": "Appearance & language",
    "settings.tabImport": "Import",
    "settings.tabClaude": "AI clients",
    "settings.tabAbout": "About",
    "settings.vaultTitle": "Vault location",
    "settings.vaultBody": "The folder your notes live in — plain markdown files you can open with anything.",
    "settings.vaultNone": "No vault chosen yet.",
    "settings.vaultChoose": "Choose folder…",
    "settings.vaultChange": "Choose a different folder…",
    "settings.appearance": "Appearance",
    "settings.theme": "Theme",
    "theme.system": "System",
    "theme.light": "Light",
    "theme.dark": "Dark",
    "settings.accent": "Accent color",
    "settings.aiColor": "AI note color",
    "settings.highlightColor": "Text highlight color",
    "settings.uiFont": "Interface font",
    "settings.editorFont": "Editor font",
    "settings.fontSize": "Font size",
    "settings.readingWidth": "Reading width",
    "settings.reset": "Reset to defaults",
    "settings.resetHint":
      "Puts appearance and language back to Magma's defaults — save to keep it.",
    "settings.save": "Save",
    "settings.saved": "Saved.",
    "settings.unsaved": "Unsaved",
    "settings.discard": "Discard",
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
    "settings.importAuthorNoteNone": "Author note: detect automatically",
    "settings.importMerged": "linked into your existing note: {info}",
    "settings.importCreated": "new author note created (no note of that name existed): {info}",
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
    "settings.mcpDevBuild":
      "Registered a development build ({exe}). That file is rebuilt on every `npm run tauri dev`, which makes Claude Desktop report \"Server disconnected\". For everyday use, install Magma and run this setup from the installed app.",
    "settings.mcpManual": "Manual setup (other MCP clients)",
    "settings.mcpNoVault": "Open a vault first to connect an AI client.",
    "settings.codexConnectTitle": "Connect to Codex",
    "settings.codexConnectBody":
      "Let Codex use the same Magma MCP server to read and co-author your vault.",
    "settings.codexMcpInstall": "Set up Codex",
    "settings.codexMcpInstalling": "Setting up…",
    "settings.codexMcpInstalled": "Done — restart Codex. Written to {path}",
    "settings.codexMcpDevBuild":
      "Registered a development build ({exe}). That file is rebuilt on every `npm run tauri dev`, which can disconnect Codex. For everyday use, install Magma and run this setup from the installed app.",
    "settings.codexMcpManual": "Manual Codex setup",
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
    "sidebar.replace": "Suchen & ersetzen",
    "sidebar.today": "Notiz von heute",
    "sidebar.newSubfolder": "Neuer Unterordner",
    "sidebar.newSubfolderPrompt": "Name des neuen Ordners in \"{parent}\"",
    "sidebar.moveFolder": "Ordner verschieben",
    "sidebar.moveFolderPrompt": "„{folder}\" in welchen Ordner verschieben? (leer = Vault-Wurzel)",
    "sidebar.moveFolderFailed": "Dieser Ordner konnte nicht verschoben werden.",
    "view.ai": "KI",
    "view.history": "Verlauf",
    "palette.placeholder": "Notiz springen oder Befehl tippen…",
    "palette.noMatches": "Nichts gefunden.",
    "cmd.newNote": "Neue Notiz",
    "cmd.today": "Notiz von heute öffnen",
    "cmd.yesterday": "Notiz von gestern öffnen",
    "cmd.capture": "Schnellnotiz",
    "cmd.graph": "Graph anzeigen",
    "cmd.editor": "Editor anzeigen",
    "cmd.aiReview": "Was KI-Clients geschrieben haben",
    "cmd.replace": "Suchen & ersetzen",
    "cmd.settings": "Einstellungen",
    "cmd.newFolder": "Neuer Ordner",
    "cmd.openVault": "Anderen Vault öffnen…",
    "cmd.history": "Versionsverlauf",
    "cmd.rename": "Diese Notiz umbenennen",
    "cmd.move": "Diese Notiz verschieben",
    "cmd.fromTemplate": "Neue Notiz aus: {name}",
    "cmd.templateHint": "Vorlage",
    "template.prompt": "Titel für die neue Notiz aus \"{name}\"",
    "calendar.title": "Kalender",
    "calendar.prev": "Voriger Monat",
    "calendar.next": "Nächster Monat",
    "capture.placeholder": "Was geht dir durch den Kopf?",
    "capture.target": "→ {target}",
    "capture.hint": "Enter speichert · Umschalt+Enter neue Zeile",
    "capture.save": "Speichern",
    "connections.toggle": "Verbindungen ein- oder ausblenden",
    "connections.backlinks": "Backlinks",
    "connections.outgoing": "Links raus",
    "connections.mentions": "Erwähnungen",
    "connections.related": "Ähnlich",
    "connections.loading": "Vault wird gelesen…",
    "connections.noBacklinks": "Noch verlinkt nichts hierher.",
    "connections.noOutgoing": "Diese Notiz verlinkt noch nirgendwohin.",
    "connections.noMentions": "Keine Notiz nennt diese hier, ohne sie zu verlinken.",
    "connections.noRelated": "Nichts im Vault liest sich wie diese Notiz.",
    "connections.mentionsHint": "Diese Notizen nennen den Namen, verlinken ihn aber nicht.",
    "connections.link": "Verlinken",
    "connections.linkAll": "Alle {count} verlinken",
    "connections.alreadyLinked": "verlinkt",
    "connections.missingHint": "Dazu gibt es noch keine Notiz — Klick legt sie an.",
    "history.title": "Versionsverlauf",
    "history.loading": "Lade…",
    "history.none": "Keine früheren Fassungen.",
    "history.noneBody": "Magma legt vor jeder größeren Änderung eine Kopie an. Sobald diese Notiz wieder bearbeitet wird, steht ihr voriger Stand hier.",
    "history.diffHint": "Grün steht jetzt in der Notiz, Rot stand in dieser Fassung.",
    "history.restore": "Diese Fassung wiederherstellen",
    "history.close": "Schließen",
    "ai.title": "Was KI-Clients geschrieben haben",
    "ai.subtitle": "{count} Notizen tragen author: ai im Frontmatter.",
    "ai.emptyTitle": "Noch nichts von einer KI geschrieben.",
    "ai.emptyBody": "Notizen, die KI-Clients über MCP anlegen oder ändern, werden im Frontmatter markiert und hier gesammelt — damit du genau siehst, was angefasst wurde.",
    "onboarding.vaultTitle": "1 · Ordner wählen",
    "onboarding.vaultBody": "Deine Notizen sind ganz normale Markdown-Dateien in einem Ordner deiner Wahl. Nichts wird hochgeladen, und jeder andere Editor kann sie öffnen — auch Obsidian.",
    "onboarding.keysTitle": "2 · Zwei Tasten, die sich lohnen",
    "onboarding.keysBody": "⌘/Strg+P öffnet alles: Notizen, Befehle, Vorlagen.\n⌘/Strg+⇧+N hält einen Gedanken in der Notiz von heute fest, ohne dass du wegmusst.",
    "onboarding.claudeTitle": "3 · Claude dazuholen (optional)",
    "onboarding.claudeBody": "Magma hat einen MCP-Server eingebaut. Ein Klick in den Einstellungen und Claude kann deine Notizen lesen und neue schreiben — richtig verlinkt. Alles, was eine KI schreibt, ist markiert und steht unter „KI\".",
    "onboarding.openClaudeSetup": "Einstellungen öffnen",
    "onboarding.next": "Weiter",
    "onboarding.skip": "Überspringen",
    "onboarding.done": "Los geht's",
    "settings.tabNotes": "Notizen & Vorlagen",
    "settings.dailyTitle": "Tagesnotizen",
    "settings.dailyBody": "Eine Notiz pro Tag, benannt nach dem Datum (2026-07-26) — geöffnet über den Kalender in der Seitenleiste und über die Schnellnotiz.",
    "settings.dailyFolder": "Ordner für Tagesnotizen",
    "settings.dailyTemplate": "Vorlage für einen neuen Tag",
    "settings.templateNone": "Keine — nur das Datum",
    "settings.templatesTitle": "Vorlagen",
    "settings.templatesBody": "Jede Notiz in diesem Ordner erscheint in der Befehlspalette als „Neue Notiz aus: …\". Die Platzhalter {{date}}, {{time}}, {{title}}, {{weekday}}, {{month}} und {{year}} werden eingesetzt.",
    "settings.templateFolder": "Vorlagen-Ordner",
    "settings.captureTitle": "Schnellnotiz",
    "settings.captureToDaily": "Erfasste Zeilen mit der Uhrzeit beginnen",
    "settings.captureBody": "⌘/Strg+⇧+N hängt an die Notiz von heute an, ohne sie zu öffnen.",
    "replace.title": "Suchen & ersetzen",
    "replace.find": "Suchen nach",
    "replace.with": "Ersetzen durch",
    "replace.renameNotes": "Passende Notizen mit umbenennen",
    "replace.renameNotesHint":
      "Wikilinks zeigen auf den Namen einer Notiz. Ohne das werden die Links umgeschrieben, die Zielnotiz behält aber ihren alten Namen — und die Links gehen ins Leere.",
    "replace.typeToPreview": "Suchbegriff eingeben — du siehst vorher, was sich ändert.",
    "replace.noMatches": "Nichts in diesem Vault passt dazu.",
    "replace.summary": "{total} Vorkommen in {notes} Notizen",
    "replace.renameBadge": "Umbenennen",
    "replace.cancel": "Abbrechen",
    "replace.apply": "In {notes} Notizen ersetzen",
    "replace.applying": "Ersetze…",
    "replace.done": "{total} Vorkommen in {notes} Notizen ersetzt.",
    "view.editor": "Editor",
    "view.graph": "Graph",
    "graph.empty": "Noch keine Notizen für den Graphen.",
    "graph.legendNote": "Notiz",
    "graph.legendAi": "KI-geschrieben",
    "graph.reset": "Ansicht zurücksetzen",
    "graph.colors": "Farben",
    "graph.colorsHint": "Eine Farbe je Hauptordner; Unterordner schattieren sie.",
    "graph.colorsReset": "Zurück zu den Standardfarben",
    "graph.aiRing": "Ring um KI-geschriebene Notizen",
    "graph.legendMissing": "noch nicht angelegt",
    "graph.openEditor": "Im Editor öffnen",
    "graph.createNote": "Notiz anlegen",
    "graph.previewMissing": "Es gibt noch keine Notiz dieses Namens — etwas verlinkt darauf.",
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
    "settings.tabVault": "Vault & Sync",
    "settings.tabAppearance": "Darstellung & Sprache",
    "settings.tabImport": "Import",
    "settings.tabClaude": "KI-Clients",
    "settings.tabAbout": "Info",
    "settings.vaultTitle": "Speicherort",
    "settings.vaultBody": "Der Ordner, in dem deine Notizen liegen — einfache Markdown-Dateien, die du mit allem öffnen kannst.",
    "settings.vaultNone": "Noch kein Vault gewählt.",
    "settings.vaultChoose": "Ordner wählen…",
    "settings.vaultChange": "Anderen Ordner wählen…",
    "settings.appearance": "Darstellung",
    "settings.theme": "Modus",
    "theme.system": "System",
    "theme.light": "Hell",
    "theme.dark": "Dunkel",
    "settings.accent": "Akzentfarbe",
    "settings.aiColor": "Farbe KI-Notizen",
    "settings.highlightColor": "Farbe Textmarkierung",
    "settings.uiFont": "Oberflächen-Schrift",
    "settings.editorFont": "Editor-Schrift",
    "settings.fontSize": "Schriftgröße",
    "settings.readingWidth": "Lesebreite",
    "settings.reset": "Auf Standard zurücksetzen",
    "settings.resetHint":
      "Setzt Aussehen und Sprache auf Magmas Standard zurück — mit Speichern übernehmen.",
    "settings.save": "Speichern",
    "settings.saved": "Gespeichert.",
    "settings.unsaved": "Nicht gespeichert",
    "settings.discard": "Verwerfen",
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
    "settings.importAuthorNoteNone": "Autor-Notiz: automatisch erkennen",
    "settings.importMerged": "mit deiner vorhandenen Notiz verknüpft: {info}",
    "settings.importCreated": "neue Autor-Notiz angelegt (es gab keine Notiz dieses Namens): {info}",
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
    "settings.mcpDevBuild":
      "Es wurde ein Entwicklungs-Build eingetragen ({exe}). Diese Datei wird bei jedem `npm run tauri dev` neu gebaut – genau deshalb meldet Claude Desktop „Server disconnected“. Für den Alltag Magma installieren und die Einrichtung aus der installierten App starten.",
    "settings.mcpManual": "Manuelle Einrichtung (andere MCP-Clients)",
    "settings.mcpNoVault": "Öffne zuerst einen Vault, um einen KI-Client zu verbinden.",
    "settings.codexConnectTitle": "Mit Codex verbinden",
    "settings.codexConnectBody":
      "Lass Codex denselben Magma-MCP-Server nutzen, um deinen Vault zu lesen und mitzuschreiben.",
    "settings.codexMcpInstall": "Codex einrichten",
    "settings.codexMcpInstalling": "Richte ein…",
    "settings.codexMcpInstalled": "Fertig – starte Codex neu. Geschrieben nach {path}",
    "settings.codexMcpDevBuild":
      "Es wurde ein Entwicklungs-Build eingetragen ({exe}). Diese Datei wird bei jedem `npm run tauri dev` neu gebaut – dadurch kann Codex die Verbindung verlieren. Für den Alltag Magma installieren und die Einrichtung aus der installierten App starten.",
    "settings.codexMcpManual": "Manuelle Codex-Einrichtung",
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
  /** Switch the language. `persist: false` previews it without storing it. */
  setLang: (l: Lang, persist?: boolean) => void;
  /** Store the language currently shown (used by Settings' save button). */
  saveLang: () => void;
  /** Go back to the stored language, dropping an unsaved preview. */
  revertLang: () => void;
  /** True while the shown language differs from the stored one. */
  langDirty: boolean;
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
  // What is actually stored, so an unsaved preview can be taken back.
  const [savedLang, setSavedLang] = useState<Lang>(lang);

  const store = (l: Lang) => {
    try {
      localStorage.setItem(STORAGE_KEY, l);
    } catch {
      /* ignore storage errors */
    }
    setSavedLang(l);
  };

  const setLang = useCallback((l: Lang, persist = true) => {
    setLangState(l);
    if (persist) store(l);
  }, []);

  const saveLang = useCallback(() => store(lang), [lang]);
  const revertLang = useCallback(() => setLangState(savedLang), [savedLang]);

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

  return (
    <I18nContext.Provider
      value={{ lang, setLang, saveLang, revertLang, langDirty: lang !== savedLang, t }}
    >
      {children}
    </I18nContext.Provider>
  );
}

export function useI18n(): I18n {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used within I18nProvider");
  return ctx;
}
