//! Optional remote vault over WebDAV.
//!
//! A Magma vault is just a folder of `.md` files, and WebDAV exposes exactly
//! that over HTTP — so a user can host their vault on ordinary webspace,
//! Nextcloud, a Synology, etc., point Magma at it, and have the same notes on
//! every machine. Magma keeps a local cache directory and treats it as the
//! vault; all existing note/link/graph/search/AI logic runs unchanged on the
//! cache. This crate handles the sync: list, download, upload, delete.
//!
//! Only the network methods need a server; the URL/auth/XML helpers are pure
//! and unit-tested. HTTPS is required — plain HTTP is rejected so credentials
//! are never sent in the clear.

use std::io::Read;
use std::path::Path;

#[derive(Clone)]
pub struct WebDavConfig {
    /// Collection URL of the vault, e.g. `https://host/dav/my-vault/`.
    pub base_url: String,
    pub username: String,
    pub password: String,
}

pub struct WebDavClient {
    cfg: WebDavConfig,
    auth: String,
}

#[derive(Debug)]
pub enum Error {
    InsecureUrl,
    Http(String),
    Io(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InsecureUrl => write!(f, "remote vault URL must use https://"),
            Error::Http(m) => write!(f, "webdav error: {m}"),
            Error::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

impl WebDavClient {
    pub fn new(cfg: WebDavConfig) -> Result<Self> {
        if !cfg.base_url.starts_with("https://") {
            return Err(Error::InsecureUrl);
        }
        let auth = basic_auth(&cfg.username, &cfg.password);
        Ok(Self { cfg, auth })
    }

    /// List `.md` files in the vault (vault-relative paths), via PROPFIND.
    ///
    /// One request per folder, each with `Depth: 1`, rather than a single
    /// `Depth: infinity` over the whole tree. The single request is what the
    /// protocol is for, but almost nothing accepts it: Apache's `mod_dav`
    /// defaults `DavDepthInfinity` to off and answers 403, and sabre/dav —
    /// which is also what Nextcloud and ownCloud run — defaults
    /// `enablePropfindDepthInfinity` to false and quietly serves it as
    /// `Depth: 1` instead. Both do it to stop one request from walking an
    /// entire repository into memory.
    ///
    /// The 403 would at least be visible. The silent downgrade is the
    /// dangerous one: the server answers normally, so the sync reports success
    /// having seen only the top level, and every note in a subfolder is missing
    /// without anything looking wrong.
    pub fn list_markdown(&self) -> Result<Vec<String>> {
        let base = self.cfg.base_url.clone();
        let auth = self.auth.clone();
        collect_markdown(&url_path(&base), |dir| {
            let url = if dir.is_empty() {
                base.clone()
            } else {
                format!("{}/", join_url(&base, dir))
            };
            propfind_depth_1(&url, &auth)
        })
    }

    /// Download every `.md` file into `dir`, creating parent folders. Returns the
    /// number of files written.
    pub fn download_all(&self, dir: &Path) -> Result<usize> {
        let files = self.list_markdown()?;
        for rel in &files {
            let text = self.get_text(rel)?;
            let dest = dir.join(rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(dest, text)?;
        }
        Ok(files.len())
    }

    pub fn get_text(&self, rel: &str) -> Result<String> {
        let url = join_url(&self.cfg.base_url, rel);
        let resp = ureq::get(&url)
            .set("Authorization", &self.auth)
            .call()
            .map_err(|e| Error::Http(e.to_string()))?;
        resp.into_string().map_err(Error::Io)
    }

    pub fn put_text(&self, rel: &str, content: &str) -> Result<()> {
        let url = join_url(&self.cfg.base_url, rel);
        ureq::put(&url)
            .set("Authorization", &self.auth)
            .set("Content-Type", "text/markdown; charset=utf-8")
            .send_string(content)
            .map_err(|e| Error::Http(e.to_string()))?;
        Ok(())
    }

    pub fn delete(&self, rel: &str) -> Result<()> {
        let url = join_url(&self.cfg.base_url, rel);
        ureq::request("DELETE", &url)
            .set("Authorization", &self.auth)
            .call()
            .map_err(|e| Error::Http(e.to_string()))?;
        Ok(())
    }
}

/// PROPFIND one collection, asking only for `resourcetype` — enough to tell a
/// file from a folder, and nothing else to build in memory.
fn propfind_depth_1(url: &str, auth: &str) -> Result<String> {
    const BODY: &str = r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>"#;
    let resp = ureq::request("PROPFIND", url)
        .set("Authorization", auth)
        .set("Depth", "1")
        .set("Content-Type", "application/xml")
        .send_string(BODY)
        .map_err(|e| Error::Http(e.to_string()))?;
    let mut xml = String::new();
    resp.into_reader()
        .read_to_string(&mut xml)
        .map_err(Error::Io)?;
    Ok(xml)
}

// --- pure helpers (unit-tested) --------------------------------------------

/// One entry of a PROPFIND multistatus body, relative to the collection the
/// request was made against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DavEntry {
    pub rel: String,
    pub is_dir: bool,
}

/// A vault with more folders than this is treated as a mistake — a symlink
/// loop, or a URL pointing at something far larger than a vault — rather than
/// walked until the request count becomes the user's problem.
const MAX_COLLECTIONS: usize = 5_000;

/// Walk a WebDAV collection tree breadth-first and collect every `.md` file as
/// a path relative to the root.
///
/// `propfind` is handed a vault-relative folder path (`""` is the root) and
/// returns that folder's multistatus body. Splitting it out this way keeps the
/// traversal testable against a server that answers one level at a time, which
/// is what real servers do.
///
/// A server that *does* honour `Depth: infinity` is handled too: the deeper
/// entries simply arrive early, and revisiting them is suppressed rather than
/// looping.
pub fn collect_markdown<F>(base_path: &str, mut propfind: F) -> Result<Vec<String>>
where
    F: FnMut(&str) -> Result<String>,
{
    let root = url_path_of_href(base_path)
        .trim_end_matches('/')
        .to_string();
    let mut files: Vec<String> = Vec::new();
    let mut queue: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    queue.push_back(String::new());
    seen.insert(String::new());

    let mut visited = 0usize;
    while let Some(dir) = queue.pop_front() {
        visited += 1;
        if visited > MAX_COLLECTIONS {
            return Err(Error::Http(format!(
                "remote vault has more than {MAX_COLLECTIONS} folders — refusing to keep walking"
            )));
        }
        let xml = propfind(&dir)?;
        let dir_base = if dir.is_empty() {
            root.clone()
        } else {
            format!("{root}/{dir}")
        };
        for entry in parse_entries(&xml, &dir_base) {
            let rel = if dir.is_empty() {
                entry.rel
            } else {
                format!("{dir}/{}", entry.rel)
            };
            if entry.is_dir {
                if seen.insert(rel.clone()) {
                    queue.push_back(rel);
                }
            } else if rel.to_lowercase().ends_with(".md") {
                files.push(rel);
            }
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

/// Build the `Authorization: Basic …` header value.
pub fn basic_auth(user: &str, pass: &str) -> String {
    format!(
        "Basic {}",
        base64_encode(format!("{user}:{pass}").as_bytes())
    )
}

/// Join a WebDAV collection URL with a vault-relative path, encoding spaces and
/// avoiding double slashes.
pub fn join_url(base: &str, rel: &str) -> String {
    let base = base.trim_end_matches('/');
    let rel = rel.trim_start_matches('/');
    let encoded: Vec<String> = rel.split('/').map(encode_segment).collect();
    format!("{base}/{}", encoded.join("/"))
}

/// The path portion of a URL (everything from the first `/` after the host).
pub fn url_path(url: &str) -> String {
    let after_scheme = url.splitn(2, "://").nth(1).unwrap_or(url);
    match after_scheme.find('/') {
        Some(i) => after_scheme[i..].to_string(),
        None => "/".to_string(),
    }
}

/// Extract `.md` file paths (vault-relative) from a PROPFIND multistatus body.
pub fn parse_markdown_rel_paths(xml: &str, base_path: &str) -> Vec<String> {
    parse_entries(xml, base_path)
        .into_iter()
        .filter(|e| !e.is_dir && e.rel.to_lowercase().ends_with(".md"))
        .map(|e| e.rel)
        .collect()
}

/// Extract every entry of a PROPFIND multistatus body, files and folders alike,
/// as paths relative to `base_path`. The collection the request was made
/// against reports itself and is dropped.
pub fn parse_entries(xml: &str, base_path: &str) -> Vec<DavEntry> {
    // Accept either a bare path or a full URL for the base.
    let base_owned = url_path_of_href(base_path);
    let base_dec = percent_decode(base_owned.trim_end_matches('/'));
    let mut out: Vec<DavEntry> = Vec::new();
    for (href, block) in parse_href_blocks(xml) {
        let path = url_path_of_href(&href);
        let trailing_slash = path.ends_with('/');
        let decoded = percent_decode(&path);
        let rel = match decoded.trim_end_matches('/').strip_prefix(&base_dec) {
            Some(s) => s.trim_start_matches('/').to_string(),
            None => continue,
        };
        if rel.is_empty() {
            continue; // the collection itself
        }
        // A trailing slash is the common marker for a collection, but it is a
        // courtesy, not a rule — `resourcetype` is the answer the protocol
        // actually gives, so prefer it and fall back to the slash.
        let is_dir = block_is_collection(&block).unwrap_or(trailing_slash);
        out.push(DavEntry { rel, is_dir });
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out.dedup_by(|a, b| a.rel == b.rel);
    out
}

/// Read `<resourcetype>` out of one response's markup. `None` when the response
/// carries no `resourcetype` at all, so the caller can fall back.
fn block_is_collection(block_lower: &str) -> Option<bool> {
    let at = block_lower.find("resourcetype")?;
    let rest = &block_lower[at + "resourcetype".len()..];
    let gt = rest.find('>')?;
    // `<d:resourcetype/>` — self-closing, so it has no `<collection/>` child.
    if rest[..gt].trim_end().ends_with('/') {
        return Some(false);
    }
    let inner_end = rest[gt..].find("</").map(|i| gt + i).unwrap_or(rest.len());
    Some(rest[gt..inner_end].contains("collection"))
}

/// Pull every `<...href>…</...href>` element, namespace-agnostic, each paired
/// with the markup that follows it up to the next href.
///
/// RFC 4918 puts `<href>` first inside its `<response>`, so that trailing slice
/// holds this entry's properties and nothing of the next one's — enough to read
/// `resourcetype` without taking on an XML parser. The href's own text is
/// outside the slice, so a note actually named `collection.md` cannot be
/// mistaken for a folder.
fn parse_href_blocks(xml: &str) -> Vec<(String, String)> {
    let lower = xml.to_lowercase();
    let mut found: Vec<(usize, String, usize)> = Vec::new();
    let mut i = 0;
    while let Some(open_rel) = lower[i..].find("href") {
        let open = i + open_rel;
        // find the '>' that closes this opening tag
        if let Some(gt_rel) = xml[open..].find('>') {
            let content_start = open + gt_rel + 1;
            if let Some(close_rel) = lower[content_start..].find("</") {
                let content_end = content_start + close_rel;
                found.push((
                    open,
                    xml[content_start..content_end].trim().to_string(),
                    content_end,
                ));
                // Step over the whole `</…href>` tag. Resuming at `content_end`
                // would find the "href" inside that closing tag and treat it as
                // the start of the next entry, cutting this entry's block off
                // before its `resourcetype`.
                i = xml[content_end..]
                    .find('>')
                    .map(|g| content_end + g + 1)
                    .unwrap_or(content_end + 2);
                continue;
            }
        }
        i = open + 4;
    }
    let mut out = Vec::with_capacity(found.len());
    for (n, (_, href, end)) in found.iter().enumerate() {
        let block_end = found.get(n + 1).map(|(o, _, _)| *o).unwrap_or(xml.len());
        out.push((href.clone(), lower[*end..block_end.max(*end)].to_string()));
    }
    out
}

fn url_path_of_href(href: &str) -> String {
    if href.contains("://") {
        url_path(href)
    } else {
        href.to_string()
    }
}

fn encode_segment(seg: &str) -> String {
    let mut s = String::new();
    for b in seg.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                s.push(b as char)
            }
            _ => s.push_str(&format!("%{b:02X}")),
        }
    }
    s
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn base64_encode(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn basic_auth_header() {
        assert_eq!(basic_auth("user", "pass"), "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn join_encodes_spaces_and_trims_slashes() {
        assert_eq!(
            join_url("https://h/dav/vault/", "/notes/second brain.md"),
            "https://h/dav/vault/notes/second%20brain.md"
        );
    }

    #[test]
    fn url_path_extracts_path() {
        assert_eq!(url_path("https://host/dav/vault/"), "/dav/vault/");
        assert_eq!(url_path("/already/a/path"), "/already/a/path");
    }

    #[test]
    fn requires_https() {
        let cfg = WebDavConfig {
            base_url: "http://insecure/dav/".into(),
            username: "u".into(),
            password: "p".into(),
        };
        assert!(matches!(WebDavClient::new(cfg), Err(Error::InsecureUrl)));
    }

    #[test]
    fn parses_markdown_paths_from_propfind() {
        let xml = r#"<?xml version="1.0"?>
        <d:multistatus xmlns:d="DAV:">
          <d:response><d:href>/dav/vault/</d:href></d:response>
          <d:response><d:href>/dav/vault/Alpha.md</d:href></d:response>
          <d:response><d:href>/dav/vault/notes/second%20brain.md</d:href></d:response>
          <d:response><d:href>/dav/vault/assets/</d:href></d:response>
          <d:response><d:href>/dav/vault/image.png</d:href></d:response>
        </d:multistatus>"#;
        let rels = parse_markdown_rel_paths(xml, "/dav/vault/");
        assert_eq!(rels, vec!["Alpha.md", "notes/second brain.md"]);
    }

    #[test]
    fn parses_hrefs_with_full_urls() {
        let xml = r#"<D:multistatus xmlns:D="DAV:">
          <D:response><D:href>https://host/dav/vault/Note.md</D:href></D:response>
        </D:multistatus>"#;
        let rels = parse_markdown_rel_paths(xml, "https://host/dav/vault/");
        assert_eq!(rels, vec!["Note.md"]);
    }

    fn response(href: &str, is_dir: bool) -> String {
        let rt = if is_dir {
            "<d:resourcetype><d:collection/></d:resourcetype>"
        } else {
            "<d:resourcetype/>"
        };
        format!(
            "<d:response><d:href>{href}</d:href>\
             <d:propstat><d:prop>{rt}</d:prop>\
             <d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>"
        )
    }

    /// A server that behaves the way real ones do out of the box: whatever
    /// `Depth` is asked for, only the immediate children of the requested
    /// collection come back. This is sabre/dav's default (and so Nextcloud's),
    /// and what `mod_dav` allows once `DavDepthInfinity` stays off.
    fn one_level_at_a_time(dir: &str) -> Result<String> {
        let children: &[(&str, bool)] = match dir {
            "" => &[("Alpha.md", false), ("notes", true), ("assets", true)],
            "notes" => &[("Beta.md", false), ("deep", true)],
            "notes/deep" => &[("Gamma.md", false)],
            "assets" => &[("img.png", false)],
            other => panic!("PROPFIND on a folder that was never listed: {other:?}"),
        };
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{dir}/")
        };
        let mut xml = String::from(r#"<?xml version="1.0"?><d:multistatus xmlns:d="DAV:">"#);
        // Every server reports the requested collection itself first.
        xml.push_str(&response(&format!("/dav/vault/{prefix}"), true));
        for (name, is_dir) in children {
            let slash = if *is_dir { "/" } else { "" };
            xml.push_str(&response(
                &format!("/dav/vault/{prefix}{name}{slash}"),
                *is_dir,
            ));
        }
        xml.push_str("</d:multistatus>");
        Ok(xml)
    }

    #[test]
    fn walks_into_subfolders_when_the_server_answers_one_level_at_a_time() {
        let mut asked = Vec::new();
        let files = collect_markdown("/dav/vault/", |dir| {
            asked.push(dir.to_string());
            one_level_at_a_time(dir)
        })
        .unwrap();

        // A single `Depth: infinity` request against such a server yields only
        // "Alpha.md" — the whole point of walking the tree.
        assert_eq!(
            files,
            vec!["Alpha.md", "notes/Beta.md", "notes/deep/Gamma.md"]
        );
        asked.sort();
        assert_eq!(asked, vec!["", "assets", "notes", "notes/deep"]);
    }

    #[test]
    fn a_server_honouring_depth_infinity_is_not_walked_twice() {
        // Everything arrives on the first request; the folders are still
        // visited, must not loop, and must not duplicate the files.
        let seen_deep = std::cell::Cell::new(0);
        let files = collect_markdown("/dav/vault/", |dir| {
            if dir == "notes" {
                seen_deep.set(seen_deep.get() + 1);
            }
            let mut xml = String::from(r#"<d:multistatus xmlns:d="DAV:">"#);
            xml.push_str(&response("/dav/vault/", true));
            if dir.is_empty() {
                xml.push_str(&response("/dav/vault/Alpha.md", false));
                xml.push_str(&response("/dav/vault/notes/", true));
                xml.push_str(&response("/dav/vault/notes/Beta.md", false));
            }
            xml.push_str("</d:multistatus>");
            Ok(xml)
        })
        .unwrap();

        assert_eq!(files, vec!["Alpha.md", "notes/Beta.md"]);
        assert_eq!(seen_deep.get(), 1, "each folder is visited exactly once");
    }

    #[test]
    fn a_collection_without_a_trailing_slash_is_still_a_folder() {
        let xml = format!(
            r#"<d:multistatus xmlns:d="DAV:">{}{}</d:multistatus>"#,
            response("/dav/vault/", true),
            response("/dav/vault/notes", true),
        );
        assert_eq!(
            parse_entries(&xml, "/dav/vault/"),
            vec![DavEntry {
                rel: "notes".into(),
                is_dir: true
            }]
        );
    }

    #[test]
    fn a_note_named_collection_is_not_mistaken_for_a_folder() {
        let xml = format!(
            r#"<d:multistatus xmlns:d="DAV:">{}</d:multistatus>"#,
            response("/dav/vault/collection.md", false),
        );
        assert_eq!(
            parse_entries(&xml, "/dav/vault/"),
            vec![DavEntry {
                rel: "collection.md".into(),
                is_dir: false
            }]
        );
    }

    #[test]
    fn an_endless_folder_tree_is_stopped_rather_than_walked_forever() {
        // A symlink loop on the server side means every folder reports one more
        // folder inside it. Without a ceiling this walks until the user gives
        // up, having sent thousands of requests.
        let result = collect_markdown("/dav/vault/", |dir| {
            let prefix = if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            };
            let mut xml = String::from(r#"<d:multistatus xmlns:d="DAV:">"#);
            xml.push_str(&response(&format!("/dav/vault/{dir}"), true));
            xml.push_str(&response(&format!("/dav/vault/{prefix}sub/"), true));
            xml.push_str("</d:multistatus>");
            Ok(xml)
        });
        assert!(matches!(result, Err(Error::Http(_))));
    }
}
