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
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct Post {
    pub title: String,
    pub link: String,
    pub date: String,
    pub author: String,
    /// WordPress user id, kept so the author name can be resolved later even
    /// when the REST API refuses to hand out the name itself.
    pub author_id: u64,
    pub content_html: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
}

/// A note ready to write: vault-relative path + markdown.
pub struct Note {
    pub rel: String,
    pub markdown: String,
}

/// What an import actually produced, so the UI can report it honestly instead
/// of silently succeeding with missing data.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    /// Notes written (posts + category/tag/author hubs).
    pub notes: usize,
    /// Posts imported.
    pub posts: usize,
    /// Distinct author names found (empty when the site hides them).
    pub authors: Vec<String>,
}

/// Import a WordPress blog into `folder` inside the vault.
///
/// `author_override` (empty = auto-detect) forces the byline for every post.
/// It exists because plenty of hardened WordPress sites block the `users`
/// endpoint *and* author embedding, leaving the REST API with no author name
/// at all — in that case the user can supply it once instead of getting notes
/// with no byline.
pub fn import_wordpress(
    vault: &Path,
    folder: &str,
    site_url: &str,
    author_override: &str,
) -> Result<ImportSummary, String> {
    let mut posts = fetch_posts(site_url)?;
    if posts.is_empty() {
        return Err("no posts found — is this a WordPress site with the REST API enabled?".into());
    }
    let forced = author_override.trim();
    if !forced.is_empty() {
        for p in &mut posts {
            p.author = forced.to_string();
        }
    }
    let mut authors: Vec<String> = posts
        .iter()
        .filter(|p| !p.author.is_empty())
        .map(|p| p.author.clone())
        .collect();
    authors.sort();
    authors.dedup();

    let notes = build_notes(&posts, folder);
    let summary = ImportSummary {
        notes: notes.len(),
        posts: posts.len(),
        authors,
    };
    for note in notes {
        magma_core::write_note(vault, &note.rel, &note.markdown).map_err(|e| e.to_string())?;
    }
    Ok(summary)
}

/// Fetch all posts from `<site>/wp-json/wp/v2/posts`, following pagination.
fn fetch_posts(site_url: &str) -> Result<Vec<Post>, String> {
    let base = normalize_base(site_url);
    // Resolve author id -> name up front. Embedding the author per-post relies
    // on the /users endpoint, which many security plugins block; a single list
    // call is more robust and lets us map each post's `author` id to a name.
    let authors = fetch_authors(&base);
    let mut posts = Vec::new();
    let mut page = 1;
    loop {
        // `_embed=1` (all relations) pulls in both the author and the terms
        // (categories/tags); the `authors` map is the fallback when the author
        // relation isn't embeddable.
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
            posts.push(extract_post(item, &authors));
        }
        if arr.len() < 100 {
            break;
        }
        page += 1;
        if page > 50 {
            break; // safety cap: 5000 posts
        }
    }
    // Last resort for the author name: the public RSS feed. Runs only for posts
    // the REST API left without one.
    let by_feed = resolve_authors_via_feed(&base, &posts);
    if !by_feed.is_empty() {
        for p in &mut posts {
            if p.author.is_empty() {
                if let Some(name) = by_feed.get(&p.author_id) {
                    p.author = name.clone();
                }
            }
        }
    }
    Ok(posts)
}

/// Normalize a post URL so REST links and RSS links compare equal.
fn normalize_link(link: &str) -> String {
    link.trim().trim_end_matches('/').to_lowercase()
}

/// Pull the text of the first `<tag>` in an XML fragment, unwrapping CDATA.
fn tag_text(xml: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = match xml.find(&open) {
        Some(i) => i + open.len(),
        None => return String::new(),
    };
    let end = match xml[start..].find(&close) {
        Some(i) => start + i,
        None => return String::new(),
    };
    let raw = xml[start..end].trim();
    let raw = raw
        .strip_prefix("<![CDATA[")
        .and_then(|r| r.strip_suffix("]]>"))
        .unwrap_or(raw);
    decode_entities(raw.trim().to_string())
}

/// Map author id -> display name using the site's RSS feed.
///
/// This is the fallback that actually works on hardened sites: security plugins
/// (Wordfence and friends) routinely return 401 for `/wp/v2/users` *and* for the
/// embedded author, but the RSS feed is public by design and carries a
/// `<dc:creator>` per item. We match feed items to posts by URL to recover the
/// id -> name mapping, and stop as soon as every author is resolved — a
/// single-author blog costs exactly one request.
fn resolve_authors_via_feed(base: &str, posts: &[Post]) -> HashMap<u64, String> {
    let mut link_to_id: HashMap<String, u64> = HashMap::new();
    let mut needed: std::collections::HashSet<u64> = std::collections::HashSet::new();
    for p in posts {
        if p.author.is_empty() && p.author_id != 0 && !p.link.is_empty() {
            link_to_id.insert(normalize_link(&p.link), p.author_id);
            needed.insert(p.author_id);
        }
    }
    let mut out: HashMap<u64, String> = HashMap::new();
    if needed.is_empty() {
        return out;
    }
    for page in 1..=30 {
        let url = if page == 1 {
            format!("{base}/feed/")
        } else {
            format!("{base}/feed/?paged={page}")
        };
        let body = match ureq::get(&url).call() {
            Ok(resp) => match resp.into_string() {
                Ok(b) => b,
                Err(_) => break,
            },
            Err(_) => break,
        };
        let mut items = 0;
        for item in body.split("<item>").skip(1) {
            items += 1;
            let link = tag_text(item, "link");
            let creator = tag_text(item, "dc:creator");
            if link.is_empty() || creator.is_empty() {
                continue;
            }
            if let Some(id) = link_to_id.get(&normalize_link(&link)) {
                out.insert(*id, creator);
            }
        }
        if items == 0 || needed.iter().all(|id| out.contains_key(id)) {
            break;
        }
    }
    out
}

/// Fetch the site's authors as an id -> display-name map. Best-effort: if the
/// `/users` endpoint is unavailable (blocked, empty), returns an empty map and
/// the import proceeds without author links rather than failing.
fn fetch_authors(base: &str) -> HashMap<u64, String> {
    let mut map = HashMap::new();
    let mut page = 1;
    loop {
        let url = format!("{base}/wp-json/wp/v2/users?per_page=100&page={page}");
        let body = match ureq::get(&url).call() {
            Ok(resp) => match resp.into_string() {
                Ok(b) => b,
                Err(_) => break,
            },
            Err(_) => break,
        };
        let arr = match serde_json::from_str::<Value>(&body) {
            Ok(Value::Array(a)) if !a.is_empty() => a,
            _ => break,
        };
        for u in &arr {
            if let (Some(id), Some(name)) = (
                u.get("id").and_then(|v| v.as_u64()),
                u.get("name").and_then(|v| v.as_str()),
            ) {
                if !name.is_empty() {
                    map.insert(id, decode_entities(name.to_string()));
                }
            }
        }
        if arr.len() < 100 {
            break;
        }
        page += 1;
        if page > 20 {
            break;
        }
    }
    map
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

/// Parse one WP REST post object into a `Post`. `authors` maps user id -> name
/// as a fallback when the author isn't embedded in the post.
pub fn extract_post(item: &Value, authors: &HashMap<u64, String>) -> Post {
    let title = decode_entities(rendered(item, "title"));
    let link = item.get("link").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let date = item.get("date").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let content_html = rendered(item, "content");

    // Prefer the embedded author (_embedded.author[0].name); fall back to
    // mapping the post's top-level `author` id through the authors list.
    let author = item
        .get("_embedded")
        .and_then(|e| e.get("author"))
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .map(|s| decode_entities(s.to_string()))
        .filter(|s| !s.is_empty())
        .or_else(|| {
            item.get("author")
                .and_then(|v| v.as_u64())
                .and_then(|id| authors.get(&id).cloned())
        })
        .unwrap_or_default();
    let author_id = item.get("author").and_then(|v| v.as_u64()).unwrap_or(0);

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
        author_id,
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
        let p = extract_post(&item, &HashMap::new());
        assert_eq!(p.title, "Mein & Beitrag");
        assert_eq!(p.author, "Alex J.");
        assert_eq!(p.categories, vec!["KI"]);
        assert_eq!(p.tags, vec!["n8n", "RAG"]);
    }

    #[test]
    fn parses_dc_creator_and_link_from_a_feed_item() {
        // Verbatim shape of a WordPress RSS item (as served by digitalhandwerk.rocks).
        let feed = r#"<channel><link>https://site.test</link>
        <item>
        <title>Out of Blog</title>
        <link>https://site.test/persoenliches/out-of-blog/</link>
        <comments>https://site.test/persoenliches/out-of-blog/#respond</comments>
        <dc:creator><![CDATA[Alex Januschewsky]]></dc:creator>
        <pubDate>Fri, 17 Jul 2026 05:14:45 +0000</pubDate>
        </item></channel>"#;
        let item = feed.split("<item>").nth(1).unwrap();
        assert_eq!(tag_text(item, "link"), "https://site.test/persoenliches/out-of-blog/");
        assert_eq!(tag_text(item, "dc:creator"), "Alex Januschewsky");
        // Trailing-slash and case differences must not break the match.
        assert_eq!(
            normalize_link("https://site.test/Persoenliches/Out-Of-Blog"),
            normalize_link("https://site.test/persoenliches/out-of-blog/")
        );
    }

    #[test]
    fn extract_post_keeps_author_id_when_name_is_blocked() {
        // Wordfence returns an error object instead of the embedded author.
        let item: Value = serde_json::from_str(
            r#"{
              "title": {"rendered": "Gesperrt"},
              "content": {"rendered": "<p>x</p>"},
              "author": 1,
              "_embedded": {"author": [{"code": "rest_user_cannot_view", "data": {"status": 401}}]}
            }"#,
        )
        .unwrap();
        let p = extract_post(&item, &HashMap::new());
        assert_eq!(p.author, "", "no name is available");
        assert_eq!(p.author_id, 1, "but the id survives for the feed lookup");
    }

    #[test]
    fn extract_post_falls_back_to_author_id_map() {
        // No embedded author (the /users endpoint was blocked), only the id.
        let item: Value = serde_json::from_str(
            r#"{
              "title": {"rendered": "Ohne Embed"},
              "content": {"rendered": "<p>x</p>"},
              "author": 1
            }"#,
        )
        .unwrap();
        let mut authors = HashMap::new();
        authors.insert(1u64, "Alex Januschewsky".to_string());
        let p = extract_post(&item, &authors);
        assert_eq!(p.author, "Alex Januschewsky");
    }

    #[test]
    fn build_notes_links_posts_and_hubs() {
        let posts = vec![Post {
            title: "Sourdough Guide".into(),
            link: "https://b/t".into(),
            date: "2026-01-01".into(),
            author: "Jane Baker".into(),
            author_id: 1,
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
