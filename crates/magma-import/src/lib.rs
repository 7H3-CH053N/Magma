//! WordPress importer: pull every post from a site's REST API, convert it to a
//! markdown note, and link the notes into the graph via their categories and
//! tags — each post links to `[[Category]]` / `[[Tag]]` hub notes, and each hub
//! links back to its posts. The result is a connected sub-graph you can search
//! and that Claude can retrieve from over MCP.
//!
//! The network fetch runs on the user's machine; the pure logic
//! (html_to_markdown, extract_post, build_notes) is unit-tested here.

use magma_core::slugify;
use regex::Regex;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Post {
    pub title: String,
    pub link: String,
    pub date: String,
    pub author: String,
    pub content_html: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
}

/// A note ready to write: vault-relative path + markdown.
pub struct Note {
    pub rel: String,
    pub markdown: String,
}

/// Import a WordPress blog into `folder` inside the vault. Returns how many
/// notes were written (posts + category/tag hubs).
pub fn import_wordpress(vault: &Path, folder: &str, site_url: &str) -> Result<usize, String> {
    let posts = fetch_posts(site_url)?;
    if posts.is_empty() {
        return Err("no posts found — is this a WordPress site with the REST API enabled?".into());
    }
    let notes = build_notes(&posts, folder);
    let count = notes.len();
    for note in notes {
        magma_core::write_note(vault, &note.rel, &note.markdown).map_err(|e| e.to_string())?;
    }
    Ok(count)
}

/// Fetch all posts from `<site>/wp-json/wp/v2/posts`, following pagination.
fn fetch_posts(site_url: &str) -> Result<Vec<Post>, String> {
    let base = normalize_base(site_url);
    let mut posts = Vec::new();
    let mut page = 1;
    loop {
        // `_embed` (all relations) pulls in both the author and the terms
        // (categories/tags) so we can link them without extra requests.
        let url = format!("{base}/wp-json/wp/v2/posts?per_page=100&page={page}&_embed=1");
        let body = match ureq::get(&url).call() {
            Ok(resp) => resp.into_string().map_err(|e| e.to_string())?,
            // WP returns 400 once the page number exceeds the total pages.
            Err(ureq::Error::Status(400, _)) => break,
            Err(e) => return Err(format!("could not reach {url}: {e}")),
        };
        let json: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
        let arr = match json.as_array() {
            Some(a) if !a.is_empty() => a.clone(),
            _ => break,
        };
        for item in &arr {
            posts.push(extract_post(item));
        }
        if arr.len() < 100 {
            break;
        }
        page += 1;
        if page > 50 {
            break; // safety cap: 5000 posts
        }
    }
    Ok(posts)
}

fn normalize_base(site_url: &str) -> String {
    let s = site_url.trim().trim_end_matches('/');
    // Scheme detection is case-insensitive; normalize the scheme to lowercase
    // so e.g. "Https://site" doesn't get a second scheme prepended.
    if s.len() >= 8 && s[..8].eq_ignore_ascii_case("https://") {
        format!("https://{}", &s[8..])
    } else if s.len() >= 7 && s[..7].eq_ignore_ascii_case("http://") {
        format!("http://{}", &s[7..])
    } else {
        format!("https://{s}")
    }
}

/// Parse one WP REST post object into a `Post`.
pub fn extract_post(item: &Value) -> Post {
    let title = decode_entities(rendered(item, "title"));
    let link = item.get("link").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let date = item.get("date").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let content_html = rendered(item, "content");

    // The embedded author is under _embedded.author[0].name.
    let author = item
        .get("_embedded")
        .and_then(|e| e.get("author"))
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| decode_entities(s.to_string()))
        .unwrap_or_default();

    let mut categories = Vec::new();
    let mut tags = Vec::new();
    if let Some(groups) = item
        .get("_embedded")
        .and_then(|e| e.get("wp:term"))
        .and_then(|t| t.as_array())
    {
        for group in groups {
            if let Some(terms) = group.as_array() {
                for term in terms {
                    let name = decode_entities(
                        term.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string(),
                    );
                    if name.is_empty() {
                        continue;
                    }
                    match term.get("taxonomy").and_then(|t| t.as_str()) {
                        Some("category") => categories.push(name),
                        Some("post_tag") => tags.push(name),
                        _ => {}
                    }
                }
            }
        }
    }
    Post {
        title,
        link,
        date,
        author,
        content_html,
        categories,
        tags,
    }
}

fn rendered(item: &Value, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.get("rendered"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Build all notes for the posts: one per post plus a hub note per category and
/// tag. Filenames use the title/term (so `[[Title]]` and `[[Category]]` links
/// resolve), de-duplicated so nothing is overwritten.
pub fn build_notes(posts: &[Post], folder: &str) -> Vec<Note> {
    let dir = folder.trim().trim_matches('/');
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut notes = Vec::new();

    // category/tag/author -> post titles (for hub notes)
    let mut cat_posts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut tag_posts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut author_posts: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for post in posts {
        let stem = unique_stem(&slugify(&post.title), &prefix, &used);
        used.insert(format!("{prefix}{stem}"));
        for c in &post.categories {
            cat_posts.entry(c.clone()).or_default().push(post.title.clone());
        }
        for t in &post.tags {
            tag_posts.entry(t.clone()).or_default().push(post.title.clone());
        }
        if !post.author.is_empty() {
            author_posts
                .entry(post.author.clone())
                .or_default()
                .push(post.title.clone());
        }
        notes.push(Note {
            rel: format!("{prefix}{stem}.md"),
            markdown: render_post(post),
        });
    }

    for (cat, titles) in &cat_posts {
        notes.push(hub_note(&prefix, cat, "Kategorie", titles));
    }
    for (tag, titles) in &tag_posts {
        notes.push(hub_note(&prefix, tag, "Tag", titles));
    }
    for (author, titles) in &author_posts {
        notes.push(hub_note(&prefix, author, "Autor", titles));
    }
    notes
}

fn unique_stem(base: &str, prefix: &str, used: &std::collections::HashSet<String>) -> String {
    let mut stem = base.to_string();
    let mut n = 2;
    while used.contains(&format!("{prefix}{stem}")) {
        stem = format!("{base} {n}");
        n += 1;
    }
    stem
}

fn render_post(post: &Post) -> String {
    let mut fm = String::from("---\nsource: import\n");
    if !post.link.is_empty() {
        fm.push_str(&format!("url: {}\n", post.link));
    }
    if !post.date.is_empty() {
        fm.push_str(&format!("date: {}\n", post.date));
    }
    fm.push_str("---\n\n");

    let mut body = format!("# {}\n", post.title);
    if !post.author.is_empty() {
        // A visible, clickable byline (also builds an author hub in the graph).
        body.push_str(&format!("\n*von [[{}]]*\n", post.author));
    }
    body.push_str(&format!("\n{}", html_to_markdown(&post.content_html)));

    let links = |label: &str, items: &[String]| -> String {
        if items.is_empty() {
            return String::new();
        }
        let joined = items
            .iter()
            .map(|i| format!("[[{i}]]"))
            .collect::<Vec<_>>()
            .join(" ");
        format!("\n\n**{label}:** {joined}")
    };
    body.push_str(&links("Kategorien", &post.categories));
    body.push_str(&links("Tags", &post.tags));
    body.push('\n');
    format!("{fm}{body}")
}

fn hub_note(prefix: &str, name: &str, kind: &str, post_titles: &[String]) -> Note {
    let list = post_titles
        .iter()
        .map(|t| format!("- [[{t}]]"))
        .collect::<Vec<_>>()
        .join("\n");
    let markdown = format!(
        "---\nsource: import\n---\n\n# {name}\n\n{kind} mit {} Beiträgen:\n\n{list}\n",
        post_titles.len()
    );
    Note {
        rel: format!("{prefix}{}.md", slugify(name)),
        markdown,
    }
}

/// Convert post HTML to markdown. Pragmatic (regex-based) — handles the tags
/// WordPress content actually uses; unknown tags are stripped, keeping text.
pub fn html_to_markdown(html: &str) -> String {
    let mut s = html.to_string();
    let re = |p: &str| Regex::new(p).unwrap();

    // Drop scripts/styles entirely.
    s = re(r"(?is)<(script|style)\b[^>]*>.*?</(script|style)>")
        .replace_all(&s, "")
        .to_string();

    // Inline: links, images, bold, italic, code.
    s = re(r#"(?is)<a\b[^>]*?href\s*=\s*["']([^"']*)["'][^>]*>(.*?)</a>"#)
        .replace_all(&s, "[$2]($1)")
        .to_string();
    let img = re(r"(?is)<img\b[^>]*>");
    s = img
        .replace_all(&s, |c: &regex::Captures| {
            let tag = &c[0];
            format!("![{}]({})", attr(tag, "alt"), attr(tag, "src"))
        })
        .to_string();
    s = re(r"(?is)<(strong|b)\b[^>]*>(.*?)</(strong|b)>")
        .replace_all(&s, "**$2**")
        .to_string();
    s = re(r"(?is)<(em|i)\b[^>]*>(.*?)</(em|i)>")
        .replace_all(&s, "*$2*")
        .to_string();
    s = re(r"(?is)<code\b[^>]*>(.*?)</code>")
        .replace_all(&s, "`$1`")
        .to_string();

    // Headings.
    for i in 1..=6 {
        let hashes = "#".repeat(i);
        s = re(&format!(r"(?is)<h{i}\b[^>]*>(.*?)</h{i}>"))
            .replace_all(&s, format!("\n\n{hashes} $1\n\n").as_str())
            .to_string();
    }

    // Blockquotes: prefix each line with "> ".
    s = re(r"(?is)<blockquote\b[^>]*>(.*?)</blockquote>")
        .replace_all(&s, |c: &regex::Captures| {
            let inner = strip_tags(&c[1]);
            let quoted = inner
                .lines()
                .map(|l| format!("> {}", l.trim()))
                .collect::<Vec<_>>()
                .join("\n");
            format!("\n\n{quoted}\n\n")
        })
        .to_string();

    // Lists: each <li> becomes a bullet; list containers are dropped.
    s = re(r"(?is)<li\b[^>]*>(.*?)</li>")
        .replace_all(&s, "- $1\n")
        .to_string();
    s = re(r"(?is)</?(ul|ol)\b[^>]*>").replace_all(&s, "\n").to_string();

    // Paragraphs and line breaks.
    s = re(r"(?is)<p\b[^>]*>(.*?)</p>")
        .replace_all(&s, "\n\n$1\n\n")
        .to_string();
    s = re(r"(?i)<br\s*/?>").replace_all(&s, "\n").to_string();

    // Strip whatever tags remain, decode entities, tidy whitespace.
    s = strip_tags(&s);
    s = decode_entities(s);
    s = re(r"[ \t]+\n").replace_all(&s, "\n").to_string();
    s = re(r"\n{3,}").replace_all(&s, "\n\n").to_string();
    s.trim().to_string()
}

fn strip_tags(s: &str) -> String {
    Regex::new(r"(?is)<[^>]+>").unwrap().replace_all(s, "").to_string()
}

fn attr(tag: &str, name: &str) -> String {
    Regex::new(&format!(r#"(?is){name}\s*=\s*["']([^"']*)["']"#))
        .unwrap()
        .captures(tag)
        .map(|c| c[1].to_string())
        .unwrap_or_default()
}

fn decode_entities(s: String) -> String {
    let mut out = s
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&hellip;", "…")
        .replace("&ndash;", "–")
        .replace("&mdash;", "—")
        .replace("&auml;", "ä")
        .replace("&ouml;", "ö")
        .replace("&uuml;", "ü")
        .replace("&Auml;", "Ä")
        .replace("&Ouml;", "Ö")
        .replace("&Uuml;", "Ü")
        .replace("&szlig;", "ß");
    // Numeric decimal entities like &#8217;
    if out.contains("&#") {
        let re = Regex::new(r"&#(\d+);").unwrap();
        out = re
            .replace_all(&out, |c: &regex::Captures| {
                c[1].parse::<u32>()
                    .ok()
                    .and_then(char::from_u32)
                    .map(|ch| ch.to_string())
                    .unwrap_or_default()
            })
            .to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_html() {
        let html = "<h2>Titel</h2><p>Ein <strong>fetter</strong> und <em>kursiver</em> Text mit <a href=\"https://x.at\">Link</a>.</p><ul><li>eins</li><li>zwei</li></ul>";
        let md = html_to_markdown(html);
        assert!(md.contains("## Titel"));
        assert!(md.contains("**fetter**"));
        assert!(md.contains("*kursiver*"));
        assert!(md.contains("[Link](https://x.at)"));
        assert!(md.contains("- eins"));
        assert!(md.contains("- zwei"));
    }

    #[test]
    fn normalizes_scheme_case_insensitively() {
        assert_eq!(normalize_base("Https://digitalhandwerk.rocks"), "https://digitalhandwerk.rocks");
        assert_eq!(normalize_base("digitalhandwerk.rocks"), "https://digitalhandwerk.rocks");
        assert_eq!(normalize_base("HTTP://x.at/"), "http://x.at");
    }

    #[test]
    fn decodes_entities() {
        assert_eq!(decode_entities("Caf&#233; &amp; Tee".into()), "Café & Tee");
        assert_eq!(decode_entities("Gr&uuml;&szlig;e".into()), "Grüße");
    }

    #[test]
    fn extract_post_reads_terms() {
        let item: Value = serde_json::from_str(
            r#"{
              "title": {"rendered": "Mein &amp; Beitrag"},
              "link": "https://blog.test/mein-beitrag",
              "date": "2026-01-02T10:00:00",
              "content": {"rendered": "<p>Hallo</p>"},
              "_embedded": {
                "author": [{"name": "Alex J."}],
                "wp:term": [
                  [{"taxonomy":"category","name":"KI"}],
                  [{"taxonomy":"post_tag","name":"n8n"},{"taxonomy":"post_tag","name":"RAG"}]
                ]
              }
            }"#,
        )
        .unwrap();
        let p = extract_post(&item);
        assert_eq!(p.title, "Mein & Beitrag");
        assert_eq!(p.author, "Alex J.");
        assert_eq!(p.categories, vec!["KI"]);
        assert_eq!(p.tags, vec!["n8n", "RAG"]);
    }

    #[test]
    fn build_notes_links_posts_and_hubs() {
        let posts = vec![Post {
            title: "Sourdough Guide".into(),
            link: "https://b/t".into(),
            date: "2026-01-01".into(),
            author: "Jane Baker".into(),
            content_html: "<p>bread</p>".into(),
            categories: vec!["Baking".into()],
            tags: vec!["yeast".into()],
        }];
        let notes = build_notes(&posts, "Blog");
        // post + 1 category hub + 1 tag hub + 1 author hub
        assert_eq!(notes.len(), 4);
        let post = notes.iter().find(|n| n.rel == "Blog/Sourdough Guide.md").unwrap();
        assert!(post.markdown.contains("# Sourdough Guide"));
        assert!(post.markdown.contains("*von [[Jane Baker]]*"));
        assert!(post.markdown.contains("[[Baking]]"));
        assert!(post.markdown.contains("[[yeast]]"));
        let hub = notes.iter().find(|n| n.rel == "Blog/Baking.md").unwrap();
        assert!(hub.markdown.contains("[[Sourdough Guide]]"));
        let author_hub = notes.iter().find(|n| n.rel == "Blog/Jane Baker.md").unwrap();
        assert!(author_hub.markdown.contains("[[Sourdough Guide]]"));
        assert!(author_hub.markdown.contains("Autor"));
    }
}
