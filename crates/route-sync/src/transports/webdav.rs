//! WebDAV transport — transfers files to a WebDAV server via HTTP.
//!
//! Uses HTTP PUT / GET / DELETE / MKCOL / PROPFIND methods.
//! Auth: HTTP Basic (username + password).
//!
//! The `dest_root` passed to Transport methods is interpreted as a path prefix
//! (relative to `credentials.url`). For example, if `credentials.url` is
//! `https://dav.example.com/` and `dest_root` is `/backups/proj1/`, a file at
//! relative path `src/a.txt` is uploaded to
//! `https://dav.example.com/backups/proj1/src/a.txt`.

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use base64::Engine;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::blocking::{Client, Response};
use reqwest::header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HeaderName};
use reqwest::Method;

use super::{FileEntry, Transport, TransportType};
use crate::config::RemoteCredentials;

/// WebDAV Depth header name (not in reqwest's standard headers).
const DEPTH: HeaderName = HeaderName::from_static("depth");

/// Characters to percent-encode in WebDAV URL path segments.
/// Encodes spaces and reserved chars but preserves letters/digits/`.`, `-`, `_`, `~`.
const WEBDAV_PATH_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// WebDAV transport.
pub struct WebdavTransport {
    creds: RemoteCredentials,
    client: Client,
}

impl WebdavTransport {
    pub fn new(creds: RemoteCredentials) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { creds, client }
    }

    fn auth_header(&self) -> Option<String> {
        match (&self.creds.username, &self.creds.password) {
            (Some(u), Some(p)) => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{u}:{p}"));
                Some(format!("Basic {encoded}"))
            }
            _ => None,
        }
    }

    fn base_url(&self) -> Result<&str> {
        if self.creds.url.is_empty() {
            return Err(anyhow!("WebDAV credentials.url is empty"));
        }
        Ok(&self.creds.url)
    }

    /// Build a fully-qualified URL for a relative path under dest_root.
    fn url_for(&self, dest_root: &Path, rel_path: &str) -> Result<String> {
        let base = self.base_url()?;
        let prefix = dest_root.to_string_lossy().replace('\\', "/");
        let mut url = base.trim_end_matches('/').to_string();
        if !prefix.is_empty() && prefix != "." && prefix != "/" {
            // Always add a separator — base was trimmed and prefix slashes are
            // stripped below, so we need exactly one '/' between them.
            url.push('/');
            url.push_str(prefix.trim_matches('/'));
        }
        if !rel_path.is_empty() {
            url.push('/');
            // Encode each segment individually so '/' is preserved as a separator.
            let segments: Vec<&str> = rel_path.split('/').collect();
            let encoded: Vec<String> = segments
                .iter()
                .map(|s| utf8_percent_encode(s, WEBDAV_PATH_ENCODE).to_string())
                .collect();
            url.push_str(&encoded.join("/"));
        }
        Ok(url)
    }

    fn add_auth(&self, mut req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        if let Some(auth) = self.auth_header() {
            req = req.header(AUTHORIZATION, auth);
        }
        req
    }

    fn ensure_parent_dirs(&self, dest_root: &Path, rel_path: &str) -> Result<()> {
        let path = std::path::Path::new(rel_path);
        let mut accum = String::new();
        let mut components = path.components().peekable();
        // Skip the final file component — only MKCOL directories.
        while let Some(comp) = components.next() {
            if components.peek().is_none() {
                break;
            }
            let seg = comp.as_os_str().to_string_lossy().to_string();
            if !accum.is_empty() {
                accum.push('/');
            }
            accum.push_str(&seg);
            let url = self.url_for(dest_root, &accum)?;
            let req = self.add_auth(
                self.client
                    .request(Method::from_bytes(b"MKCOL").unwrap(), &url)
                    .header(CONTENT_LENGTH, "0"),
            );
            let _ = req.send(); // Ignore errors — directory may already exist.
        }
        Ok(())
    }

    fn parse_propfind(body: &str, base_path: &str) -> Vec<String> {
        // Minimal PROPFIND response parser. Extracts <D:href> or <d:href> values
        // and strips the base_path prefix to return relative paths.
        let mut out = Vec::new();
        let lower = body.to_lowercase();
        let needle = "<d:href>";
        let alt_needle = "<d:href ";
        let mut idx = 0;
        while let Some(pos) = lower[idx..].find(needle).or_else(|| lower[idx..].find(alt_needle)) {
            let abs = idx + pos;
            // Find end of opening tag
            let after_open = if let Some(gt) = lower[abs..].find('>') {
                abs + gt + 1
            } else {
                break;
            };
            // Find closing tag
            let close = if let Some(c) = lower[after_open..].find("</d:href>") {
                after_open + c
            } else {
                break;
            };
            let href = &body[after_open..close];
            let decoded = percent_encoding::percent_decode_str(href)
                .decode_utf8_lossy()
                .to_string();
            let stripped = decoded
                .trim_start_matches(base_path)
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string();
            if !stripped.is_empty() {
                out.push(stripped);
            }
            idx = close + 9;
        }
        out
    }
}

impl Transport for WebdavTransport {
    fn send_file(&self, entry: &FileEntry, dest_root: &Path) -> Result<()> {
        self.ensure_parent_dirs(dest_root, &entry.rel_path)?;
        let url = self.url_for(dest_root, &entry.rel_path)?;
        let bytes = std::fs::read(&entry.source_abs)?;
        let req = self.add_auth(
            self.client
                .put(&url)
                .header(CONTENT_LENGTH, bytes.len().to_string())
                .body(bytes),
        );
        let resp = req.send()?;
        check_status(resp, &[200, 201, 204], "PUT")?;
        Ok(())
    }

    fn list_dest(&self, dest_root: &Path) -> Result<Vec<String>> {
        // PROPFIND with Depth: 1 on the dest_root itself; this lists immediate
        // children. We then recursively PROPFIND subdirectories on-demand is
        // expensive, so we use Depth: infinity (most servers support it).
        let url = self.url_for(dest_root, "")?;
        let propfind_body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:resourcetype/>
  </D:prop>
</D:propfind>"#;
        let req = self.add_auth(
            self.client
                .request(Method::from_bytes(b"PROPFIND").unwrap(), &url)
                .header(DEPTH, "infinity")
                .header(CONTENT_TYPE, "application/xml; charset=utf-8")
                .body(propfind_body.to_string()),
        );
        let resp = req.send()?;
        if !resp.status().is_success() && resp.status().as_u16() != 207 {
            return Err(anyhow!("PROPFIND failed: {}", resp.status()));
        }
        let body = resp.text()?;
        // Compute the path prefix to strip from hrefs.
        let parsed_url = reqwest::Url::parse(&url)?;
        let base_path = parsed_url.path().to_string();
        let mut files = Self::parse_propfind(&body, &base_path);
        files.sort();
        files.dedup();
        // Filter out directory entries — they end with '/'.
        files.retain(|p| !p.ends_with('/'));
        Ok(files)
    }

    fn delete_file(&self, rel_path: &str, dest_root: &Path) -> Result<()> {
        let url = self.url_for(dest_root, rel_path)?;
        let req = self.add_auth(self.client.delete(&url));
        let resp = req.send()?;
        check_status(resp, &[200, 204, 404], "DELETE")?;
        Ok(())
    }

    fn mkdir(&self, rel_path: &str, dest_root: &Path) -> Result<()> {
        let url = self.url_for(dest_root, rel_path)?;
        let req = self.add_auth(
            self.client
                .request(Method::from_bytes(b"MKCOL").unwrap(), &url)
                .header(CONTENT_LENGTH, "0"),
        );
        let resp = req.send()?;
        check_status(resp, &[200, 201, 405], "MKCOL")?; // 405 = already exists
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Webdav
    }
}

fn check_status(resp: Response, ok: &[u16], op: &str) -> Result<()> {
    let status = resp.status().as_u16();
    if ok.contains(&status) {
        return Ok(());
    }
    let text = resp.text().unwrap_or_default();
    Err(anyhow!("WebDAV {op} failed: status={status} body={}", truncate_text(&text, 200)))
}

fn truncate_text(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RemoteCredentials;

    fn make_transport() -> WebdavTransport {
        let creds = RemoteCredentials {
            url: "https://dav.example.com/base/".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            ..Default::default()
        };
        WebdavTransport::new(creds)
    }

    #[test]
    fn url_for_combines_base_prefix_and_rel() {
        let t = make_transport();
        let url = t.url_for(Path::new("/backups/proj1/"), "src/a.txt").unwrap();
        assert_eq!(url, "https://dav.example.com/base/backups/proj1/src/a.txt");
    }

    #[test]
    fn url_for_handles_empty_dest_root() {
        let t = make_transport();
        let url = t.url_for(Path::new(""), "a.txt").unwrap();
        assert_eq!(url, "https://dav.example.com/base/a.txt");
    }

    #[test]
    fn url_for_encodes_special_chars() {
        let t = make_transport();
        let url = t.url_for(Path::new("/d/"), "a b#c.txt").unwrap();
        // Space → %20, # → %23
        assert!(url.contains("a%20b%23c.txt"));
    }

    #[test]
    fn auth_header_present_when_credentials_set() {
        let t = make_transport();
        let auth = t.auth_header().unwrap();
        assert!(auth.starts_with("Basic "));
        let encoded = &auth["Basic ".len()..];
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();
        assert_eq!(decoded, b"user:pass");
    }

    #[test]
    fn auth_header_absent_when_credentials_missing() {
        let creds = RemoteCredentials {
            url: "https://dav.example.com/".to_string(),
            ..Default::default()
        };
        let t = WebdavTransport::new(creds);
        assert!(t.auth_header().is_none());
    }

    #[test]
    fn base_url_errors_when_empty() {
        let creds = RemoteCredentials::default();
        let t = WebdavTransport::new(creds);
        let err = t.url_for(Path::new("/"), "a.txt").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn parse_propfind_extracts_relative_paths() {
        let body = r#"<?xml version="1.0"?>
<D:multistatus xmlns:D="DAV:">
  <D:response>
    <D:href>/base/backups/proj1/</D:href>
  </D:response>
  <D:response>
    <D:href>/base/backups/proj1/a.txt</D:href>
  </D:response>
  <D:response>
    <D:href>/base/backups/proj1/src/</D:href>
  </D:response>
  <D:response>
    <D:href>/base/backups/proj1/src/b.txt</D:href>
  </D:response>
</D:multistatus>"#;
        let mut files = WebdavTransport::parse_propfind(body, "/base/backups/proj1/");
        files.sort();
        assert_eq!(files, vec!["a.txt", "src", "src/b.txt"]);
    }

    #[test]
    fn parse_propfind_handles_lowercase_namespace() {
        let body = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/base/x.txt</d:href>
  </d:response>
</d:multistatus>"#;
        let files = WebdavTransport::parse_propfind(body, "/base/");
        assert_eq!(files, vec!["x.txt"]);
    }

    #[test]
    fn parse_propfind_empty_body_returns_empty() {
        let files = WebdavTransport::parse_propfind("", "/base/");
        assert!(files.is_empty());
    }
}
