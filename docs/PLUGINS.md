# Magma plugins

Magma loads trusted local plugins from the open vault:

```text
.magma/
  plugins/
    my-plugin/
      manifest.json
      main.js
```

`manifest.json`:

```json
{
  "id": "my-plugin",
  "name": "My plugin",
  "description": "Adds a command to Magma.",
  "author": "You",
  "version": "0.1.0"
}
```

The folder name must match `manifest.id`. IDs may contain ASCII letters,
numbers, dashes, underscores and dots.

`main.js`:

```js
magma.registerCommand(
  {
    id: "hello",
    label: "Say hello",
    hint: "Plugin"
  },
  async () => {
    const context = await magma.getContext();
    await magma.notice(
      "Hello from a plugin",
      `The vault has ${context.notes.length} notes.`
    );
  }
);
```

After adding the folder, reopen Magma or focus the window. The plugin appears in
Settings > Plugins. Enable it, save settings, then run its command from the
command palette.

## Runtime

Vault plugins run in a Web Worker. They do not receive direct DOM, Tauri,
filesystem or shell access. They can only call the host methods Magma exposes:

- `magma.registerCommand(command, handler)`
- `magma.notice(title, detail?)`
- `magma.openNote(path)`
- `magma.getContext()`

`getContext()` returns:

```ts
{
  notes: { path: string; title: string; aiAuthored: boolean; modified: number }[];
  activePath: string | null;
  content: string;
}
```

Treat vault plugins as trusted local code: the Worker boundary limits Magma app
access, but plugin code is still JavaScript you chose to install.
