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
    pub fn list_markdown(&self) -> Result<Vec<String>> {
        let body = r#"<?xml version="1.0"?><d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>"#;
        let resp = ureq::request("PROPFIND", &self.cfg.base_url)
            .set("Authorization", &self.auth)
            .set("Depth", "infinity")
            .set("Content-Type", "application/xml")
            .send_string(body)
            .map_err(|e| Error::Http(e.to_string()))?;
        let mut xml = String::new();
        resp.into_reader()
            .read_to_string(&mut xml)
            .map_err(Error::Io)?;
        let base_path = url_path(&self.cfg.base_url);
        Ok(parse_markdown_rel_paths(&xml, &base_path))
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

// --- pure helpers (unit-tested) --------------------------------------------

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
    // Accept either a bare path or a full URL for the base.
    let base_owned = url_path_of_href(base_path);
    let base = base_owned.trim_end_matches('/');
    let mut out = Vec::new();
    for href in parse_hrefs(xml) {
        let path = url_path_of_href(&href);
        if path.ends_with('/') {
            continue; // a collection, not a file
        }
        let decoded = percent_decode(&path);
        let base_dec = percent_decode(base);
        let rel = decoded
            .strip_prefix(&base_dec)
            .map(|s| s.trim_start_matches('/').to_string())
            .unwrap_or_default();
        if !rel.is_empty() && rel.to_lowercase().ends_with(".md") {
            out.push(rel);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Pull the contents of every `<...href>...</...href>` element, namespace-agnostic.
fn parse_hrefs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = xml.to_lowercase();
    let mut i = 0;
    while let Some(open_rel) = lower[i..].find("href") {
        let open = i + open_rel;
        // find the '>' that closes this opening tag
        if let Some(gt_rel) = xml[open..].find('>') {
            let content_start = open + gt_rel + 1;
            if let Some(close_rel) = lower[content_start..].find("</") {
                let content_end = content_start + close_rel;
                out.push(xml[content_start..content_end].trim().to_string());
                i = content_end;
                continue;
            }
        }
        i = open + 4;
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
}
