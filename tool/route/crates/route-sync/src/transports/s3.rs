//! S3-compatible transport — transfers files to S3 / MinIO / R2 / etc.
//!
//! Uses AWS Signature V4 for authentication.
//!
//! The `dest_root` passed to Transport methods is interpreted as a key prefix
//! inside the bucket. For example, if `credentials.bucket` is `my-bucket` and
//! `dest_root` is `/backups/proj1/`, a file at relative path `src/a.txt` is
//! uploaded to the S3 key `backups/proj1/src/a.txt` in `my-bucket`.

use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, HOST};
use reqwest::Method;
use sha2::{Digest, Sha256};

use super::{FileEntry, Transport, TransportType};
use crate::config::RemoteCredentials;

type HmacSha256 = Hmac<Sha256>;

/// Characters that need percent-encoding for S3 object keys.
/// S3 keys allow most chars but we encode spaces and a few reserved chars
/// for safety. Slashes are preserved (they're key separators).
const S3_KEY_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'#')
    .add(b'?')
    .add(b'%')
    .add(b'&')
    .add(b'+');

/// S3 transport.
pub struct S3Transport {
    creds: RemoteCredentials,
    client: Client,
}

impl S3Transport {
    pub fn new(creds: RemoteCredentials) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { creds, client }
    }

    fn bucket(&self) -> Result<&str> {
        self.creds
            .bucket
            .as_deref()
            .ok_or_else(|| anyhow!("S3 credentials.bucket is required"))
    }

    fn region(&self) -> &str {
        self.creds.region.as_deref().unwrap_or("us-east-1")
    }

    fn endpoint(&self) -> Result<String> {
        if self.creds.url.is_empty() {
            // Default AWS endpoint: path-style for simplicity.
            Ok(format!("https://s3.{}.amazonaws.com", self.region()))
        } else {
            Ok(self.creds.url.trim_end_matches('/').to_string())
        }
    }

    fn access_key(&self) -> Result<&str> {
        self.creds
            .access_key
            .as_deref()
            .ok_or_else(|| anyhow!("S3 credentials.access_key is required"))
    }

    fn secret_key(&self) -> Result<&str> {
        self.creds
            .secret_key
            .as_deref()
            .ok_or_else(|| anyhow!("S3 credentials.secret_key is required"))
    }

    /// Compute the S3 object key from dest_root + rel_path.
    fn key_for(&self, dest_root: &Path, rel_path: &str) -> String {
        let prefix = dest_root.to_string_lossy().replace('\\', "/");
        let prefix = prefix.trim_start_matches('/').trim_end_matches('/');
        let rel = rel_path.trim_start_matches('/');
        if prefix.is_empty() {
            rel.to_string()
        } else if rel.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix}/{rel}")
        }
    }

    /// Build the URL for an object (path-style: /bucket/key).
    fn url_for(&self, key: &str) -> Result<String> {
        let endpoint = self.endpoint()?;
        let bucket = self.bucket()?;
        let encoded_key = encode_s3_key(key);
        Ok(format!("{endpoint}/{bucket}/{encoded_key}"))
    }

    /// Sign a request using AWS SigV4 and return the Authorization header value.
    fn sign_request(
        &self,
        method: &Method,
        url: &str,
        payload_hash: &str,
        amz_date: &str,
        date_stamp: &str,
        extra_headers: &[(String, String)],
    ) -> Result<String> {
        let parsed = reqwest::Url::parse(url)?;
        let host = parsed
            .host_str()
            .ok_or_else(|| anyhow!("invalid url: no host"))?;
        let host_header = if let Some(port) = parsed.port() {
            format!("{host}:{port}")
        } else {
            host.to_string()
        };

        // Canonical headers (sorted): host, x-amz-content-sha256, x-amz-date
        // plus any extras the caller needs (e.g., Content-Type).
        let mut headers: Vec<(String, String)> = vec![
            ("host".to_string(), host_header.clone()),
            ("x-amz-content-sha256".to_string(), payload_hash.to_string()),
            ("x-amz-date".to_string(), amz_date.to_string()),
        ];
        for (k, v) in extra_headers {
            headers.push((k.to_lowercase(), v.clone()));
        }
        headers.sort_by(|a, b| a.0.cmp(&b.0));

        let canonical_headers: String = headers.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
        let signed_headers: String = headers
            .iter()
            .map(|(k, _)| k.clone())
            .collect::<Vec<_>>()
            .join(";");

        let canonical_query = canonical_query_string(parsed.query_pairs());

        let canonical_uri = canonical_uri_path(parsed.path(), self.bucket()?);

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.as_str(),
            canonical_uri,
            canonical_query,
            canonical_headers,
            signed_headers,
            payload_hash
        );

        let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, self.region());

        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            hex::encode(sha256_hash(canonical_request.as_bytes()))
        );

        let signing_key = signing_key(self.secret_key()?, date_stamp, self.region(), "s3")?;
        let signature = hex::encode(hmac_sha256(&signing_key, string_to_sign.as_bytes())?);

        let auth = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.access_key()?,
            credential_scope,
            signed_headers,
            signature
        );
        Ok(auth)
    }
}

impl Transport for S3Transport {
    fn send_file(&self, entry: &FileEntry, dest_root: &Path) -> Result<()> {
        let key = self.key_for(dest_root, &entry.rel_path);
        let url = self.url_for(&key)?;
        let bytes = std::fs::read(&entry.source_abs)?;
        let payload_hash = hex::encode(sha256_hash(&bytes));

        let now: DateTime<Utc> = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let auth = self.sign_request(
            &Method::PUT,
            &url,
            &payload_hash,
            &amz_date,
            &date_stamp,
            &[],
        )?;

        let parsed = reqwest::Url::parse(&url)?;
        let host_header = host_with_port(&parsed);

        let resp = self
            .client
            .put(&url)
            .header(HOST, &host_header)
            .header("x-amz-content-sha256", &payload_hash)
            .header("x-amz-date", &amz_date)
            .header(AUTHORIZATION_HEADER, &auth)
            .header(CONTENT_LENGTH, bytes.len().to_string())
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(bytes)
            .send()?;

        check_status(resp, &[200, 201], "PUT")?;
        Ok(())
    }

    fn list_dest(&self, dest_root: &Path) -> Result<Vec<String>> {
        let prefix = self.key_for(dest_root, "");
        let endpoint = self.endpoint()?;
        let bucket = self.bucket()?;
        // ListObjectsV2: GET /bucket?list-type=2&prefix=...
        let mut url = format!("{endpoint}/{bucket}?list-type=2");
        if !prefix.is_empty() {
            url.push_str("&prefix=");
            url.push_str(&encode_s3_key(&prefix));
            url.push('/');
        }

        let payload_hash = hex::encode(sha256_hash(b""));
        let now: DateTime<Utc> = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let auth = self.sign_request(
            &Method::GET,
            &url,
            &payload_hash,
            &amz_date,
            &date_stamp,
            &[],
        )?;

        let parsed = reqwest::Url::parse(&url)?;
        let host_header = host_with_port(&parsed);

        let resp = self
            .client
            .get(&url)
            .header(HOST, &host_header)
            .header("x-amz-content-sha256", &payload_hash)
            .header("x-amz-date", &amz_date)
            .header(AUTHORIZATION_HEADER, &auth)
            .send()?;

        if !resp.status().is_success() {
            return Err(anyhow!("S3 list failed: {}", resp.status()));
        }
        let body = resp.text()?;
        let keys = parse_list_v2(&body);
        // Strip the prefix from each key.
        let stripped: Vec<String> = keys
            .into_iter()
            .map(|k| {
                if !prefix.is_empty() && k.starts_with(&format!("{prefix}/")) {
                    k[prefix.len() + 1..].to_string()
                } else {
                    k
                }
            })
            .filter(|k| !k.is_empty())
            .collect();
        Ok(stripped)
    }

    fn delete_file(&self, rel_path: &str, dest_root: &Path) -> Result<()> {
        let key = self.key_for(dest_root, rel_path);
        let url = self.url_for(&key)?;
        let payload_hash = hex::encode(sha256_hash(b""));
        let now: DateTime<Utc> = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let auth = self.sign_request(
            &Method::DELETE,
            &url,
            &payload_hash,
            &amz_date,
            &date_stamp,
            &[],
        )?;

        let parsed = reqwest::Url::parse(&url)?;
        let host_header = host_with_port(&parsed);

        let resp = self
            .client
            .delete(&url)
            .header(HOST, &host_header)
            .header("x-amz-content-sha256", &payload_hash)
            .header("x-amz-date", &amz_date)
            .header(AUTHORIZATION_HEADER, &auth)
            .send()?;

        check_status(resp, &[200, 204, 404], "DELETE")?;
        Ok(())
    }

    fn mkdir(&self, _rel_path: &str, _dest_root: &Path) -> Result<()> {
        // S3 has no directories — keys are flat. No-op.
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        TransportType::S3
    }
}

const AUTHORIZATION_HEADER: &str = "Authorization";

// ---------------------------------------------------------------------------
// SigV4 helpers
// ---------------------------------------------------------------------------

fn sha256_hash(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&out);
    arr
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<[u8; 32]> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|e| anyhow!("HMAC key error: {e}"))?;
    mac.update(data);
    let out = mac.finalize().into_bytes();
    let mut arr = [0u8; 32];
    if out.len() != 32 {
        return Err(anyhow!("HMAC output length != 32"));
    }
    arr.copy_from_slice(&out);
    Ok(arr)
}

fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> Result<[u8; 32]> {
    let k_secret = format!("AWS4{secret}").into_bytes();
    let k_date = hmac_sha256(&k_secret, date.as_bytes())?;
    let k_region = hmac_sha256(&k_date, region.as_bytes())?;
    let k_service = hmac_sha256(&k_region, service.as_bytes())?;
    let k_signing = hmac_sha256(&k_service, b"aws4_request")?;
    Ok(k_signing)
}

fn canonical_query_string<'a, I>(query: I) -> String
where
    I: Iterator<Item = (std::borrow::Cow<'a, str>, std::borrow::Cow<'a, str>)>,
{
    let mut pairs: Vec<(String, String)> = query
        .map(|(k, v)| {
            (
                utf8_percent_encode(&k, &S3_QUERY_ENCODE).to_string(),
                utf8_percent_encode(&v, &S3_QUERY_ENCODE).to_string(),
            )
        })
        .collect();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// Characters to encode in query string values (RFC 3986).
const S3_QUERY_ENCODE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// Characters to encode in canonical URI path segments.
const S3_PATH_ENCODE: &AsciiSet = &CONTROLS.add(b' ').add(b'#').add(b'?').add(b'%');

fn canonical_uri_path(path: &str, _bucket: &str) -> String {
    // Path-style URL: path is /bucket/key — we percent-encode each segment
    // except '/'.
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    let trimmed = path.trim_start_matches('/');
    let segments: Vec<String> = trimmed
        .split('/')
        .map(|s| utf8_percent_encode(s, S3_PATH_ENCODE).to_string())
        .collect();
    format!("/{}", segments.join("/"))
}

fn host_with_port(url: &reqwest::Url) -> String {
    match (url.host_str(), url.port()) {
        (Some(h), Some(p)) => format!("{h}:{p}"),
        (Some(h), None) => h.to_string(),
        _ => String::new(),
    }
}

fn encode_s3_key(key: &str) -> String {
    // Encode each segment separately so '/' is preserved.
    key.split('/')
        .map(|s| utf8_percent_encode(s, S3_KEY_ENCODE).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn check_status(resp: Response, ok: &[u16], op: &str) -> Result<()> {
    let status = resp.status().as_u16();
    if ok.contains(&status) {
        return Ok(());
    }
    let text = resp.text().unwrap_or_default();
    Err(anyhow!(
        "S3 {op} failed: status={status} body={}",
        truncate_text(&text, 200)
    ))
}

fn truncate_text(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

// ---------------------------------------------------------------------------
// ListObjectsV2 XML parser (minimal)
// ---------------------------------------------------------------------------

fn parse_list_v2(xml: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let lower = xml.to_lowercase();
    let needle = "<key>";
    let close = "</key>";
    let mut idx = 0;
    while let Some(pos) = lower[idx..].find(needle) {
        let abs = idx + pos;
        let start = abs + needle.len();
        if let Some(end_rel) = lower[start..].find(close) {
            let end = start + end_rel;
            keys.push(xml[start..end].to_string());
            idx = end + close.len();
        } else {
            break;
        }
    }
    keys
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RemoteCredentials;

    fn make_transport() -> S3Transport {
        let creds = RemoteCredentials {
            url: "https://s3.us-east-1.amazonaws.com".to_string(),
            access_key: Some("AKIAEXAMPLE".to_string()),
            secret_key: Some("secretexample".to_string()),
            bucket: Some("my-bucket".to_string()),
            region: Some("us-east-1".to_string()),
            ..Default::default()
        };
        S3Transport::new(creds)
    }

    #[test]
    fn key_for_combines_prefix_and_rel() {
        let t = make_transport();
        let key = t.key_for(Path::new("/backups/proj1/"), "src/a.txt");
        assert_eq!(key, "backups/proj1/src/a.txt");
    }

    #[test]
    fn key_for_handles_empty_prefix() {
        let t = make_transport();
        let key = t.key_for(Path::new(""), "a.txt");
        assert_eq!(key, "a.txt");
    }

    #[test]
    fn key_for_strips_leading_slash() {
        let t = make_transport();
        let key = t.key_for(Path::new("/"), "/a/b.txt");
        assert_eq!(key, "a/b.txt");
    }

    #[test]
    fn url_for_uses_path_style() {
        let t = make_transport();
        let url = t.url_for("backups/a.txt").unwrap();
        assert_eq!(
            url,
            "https://s3.us-east-1.amazonaws.com/my-bucket/backups/a.txt"
        );
    }

    #[test]
    fn url_for_encodes_special_chars() {
        let t = make_transport();
        let url = t.url_for("a b#c.txt").unwrap();
        assert!(url.contains("a%20b%23c.txt"));
    }

    #[test]
    fn url_for_preserves_slash_separator() {
        let t = make_transport();
        let url = t.url_for("dir/a.txt").unwrap();
        assert!(url.ends_with("/my-bucket/dir/a.txt"));
    }

    #[test]
    fn endpoint_defaults_to_aws() {
        let creds = RemoteCredentials {
            access_key: Some("k".to_string()),
            secret_key: Some("s".to_string()),
            bucket: Some("b".to_string()),
            region: Some("eu-west-1".to_string()),
            ..Default::default()
        };
        let t = S3Transport::new(creds);
        assert_eq!(t.endpoint().unwrap(), "https://s3.eu-west-1.amazonaws.com");
    }

    #[test]
    fn endpoint_uses_custom_url_for_minio() {
        let creds = RemoteCredentials {
            url: "http://localhost:9000/".to_string(),
            access_key: Some("k".to_string()),
            secret_key: Some("s".to_string()),
            bucket: Some("b".to_string()),
            region: Some("us-east-1".to_string()),
            ..Default::default()
        };
        let t = S3Transport::new(creds);
        assert_eq!(t.endpoint().unwrap(), "http://localhost:9000");
    }

    #[test]
    fn bucket_errors_when_missing() {
        let creds = RemoteCredentials {
            access_key: Some("k".to_string()),
            secret_key: Some("s".to_string()),
            ..Default::default()
        };
        let t = S3Transport::new(creds);
        let err = t.bucket().unwrap_err();
        assert!(err.to_string().contains("bucket"));
    }

    #[test]
    fn access_key_errors_when_missing() {
        let creds = RemoteCredentials {
            secret_key: Some("s".to_string()),
            bucket: Some("b".to_string()),
            ..Default::default()
        };
        let t = S3Transport::new(creds);
        let err = t.access_key().unwrap_err();
        assert!(err.to_string().contains("access_key"));
    }

    #[test]
    fn sha256_hash_is_deterministic() {
        let h1 = hex::encode(sha256_hash(b"hello"));
        let h2 = hex::encode(sha256_hash(b"hello"));
        assert_eq!(h1, h2);
        assert_eq!(
            h1,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn signing_key_is_deterministic() {
        let k1 = signing_key("secret", "20260101", "us-east-1", "s3").unwrap();
        let k2 = signing_key("secret", "20260101", "us-east-1", "s3").unwrap();
        assert_eq!(k1, k2);
        // Different date produces different key
        let k3 = signing_key("secret", "20260102", "us-east-1", "s3").unwrap();
        assert_ne!(k1, k3);
    }

    #[test]
    fn sign_request_produces_well_formed_authorization() {
        let t = make_transport();
        let payload_hash = hex::encode(sha256_hash(b""));
        let auth = t
            .sign_request(
                &Method::GET,
                "https://s3.us-east-1.amazonaws.com/my-bucket?list-type=2",
                &payload_hash,
                "20260101T000000Z",
                "20260101",
                &[],
            )
            .unwrap();
        assert!(auth.starts_with("AWS4-HMAC-SHA256 Credential=AKIAEXAMPLE/"));
        assert!(auth.contains("SignedHeaders=host;x-amz-content-sha256;x-amz-date"));
        assert!(auth.contains("Signature="));
    }

    #[test]
    fn parse_list_v2_extracts_keys() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>my-bucket</Name>
  <Prefix>backups/</Prefix>
  <KeyCount>2</KeyCount>
  <Contents>
    <Key>backups/a.txt</Key>
    <Size>5</Size>
  </Contents>
  <Contents>
    <Key>backups/src/b.txt</Key>
    <Size>11</Size>
  </Contents>
</ListBucketResult>"#;
        let keys = parse_list_v2(xml);
        assert_eq!(keys, vec!["backups/a.txt", "backups/src/b.txt"]);
    }

    #[test]
    fn parse_list_v2_empty_returns_empty() {
        let xml = r#"<?xml version="1.0"?>
<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/">
  <Name>my-bucket</Name>
  <KeyCount>0</KeyCount>
</ListBucketResult>"#;
        let keys = parse_list_v2(xml);
        assert!(keys.is_empty());
    }

    #[test]
    fn canonical_query_string_sorts_params() {
        let url = reqwest::Url::parse("https://h/b?b=2&a=1&c=3").unwrap();
        let qs = canonical_query_string(url.query_pairs());
        assert_eq!(qs, "a=1&b=2&c=3");
    }

    #[test]
    fn canonical_uri_path_encodes_spaces() {
        let u = canonical_uri_path("/b/a b.txt", "b");
        assert_eq!(u, "/b/a%20b.txt");
    }
}
