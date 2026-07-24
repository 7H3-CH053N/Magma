// Split a note's YAML frontmatter from its body so the editor only ever shows
// the body (the `---\nauthor: ai\n---` block must not leak into the WYSIWYG
// view), while the frontmatter is preserved and re-attached on save.

export interface SplitNote {
  frontmatter: string; // includes the delimiters, or "" if none
  body: string;
}

export function splitFrontmatter(md: string): SplitNote {
  if (!md.startsWith("---")) return { frontmatter: "", body: md };
  // Find the closing delimiter line.
  const close = md.indexOf("\n---", 3);
  if (close === -1) return { frontmatter: "", body: md };
  const afterClose = md.indexOf("\n", close + 1);
  const fmEnd = afterClose === -1 ? md.length : afterClose + 1;
  return {
    frontmatter: md.slice(0, fmEnd),
    body: md.slice(fmEnd).replace(/^\n+/, ""),
  };
}

/** Re-attach frontmatter to an edited body. */
export function joinFrontmatter(frontmatter: string, body: string): string {
  if (!frontmatter) return body;
  return `${frontmatter.trimEnd()}\n\n${body}`;
}
