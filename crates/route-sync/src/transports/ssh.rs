//! SSH transport — transfers files to a remote host via `scp`/`ssh`.
//!
//! The destination URL has the form `ssh://[user@]host[:port]/path`.
//! Files are copied with `scp -P <port> <local> <user>@<host>:<remote>`,
//! directories are created with `ssh ... mkdir -p`, and listing is
//! done with `ssh ... find <remote_root> -type f`.
//!
//! Auth is delegated to the user's `~/.ssh/config` and `ssh-agent` —
//! we never handle private keys in-process. Passwords are passed via
//! `sshpass` when available, but the recommended way is to set up
//! key-based auth.
//!
//! This transport is intentionally minimal: it depends only on the
//! system `scp` and `ssh` binaries (always present on Linux/macOS,
//! available via OpenSSH on Windows). It does not implement SFTP
//! directly because that would pull in a new dependency.

use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, Result};

use super::{FileEntry, Transport, TransportType};
use crate::config::RemoteCredentials;

// route-sync is linked into both console binaries (route-cli, route-mcp) and
// the windowed desktop app (route-tauri, built with `windows_subsystem =
// "windows"`). In the desktop app, spawning `ssh` / `scp` / `whoami` without
// `CREATE_NO_WINDOW` would flash a black console window on every cloud-backup
// operation. The flag is harmless for the console binaries — their child
// processes already inherit the parent's console, and all output here is
// captured via pipes anyway, so nothing the user sees changes.
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a `Command` for `program` with the no-console flag pre-applied on
/// Windows. Every external process spawn in this transport goes through here.
fn silent_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// SSH transport.
#[derive(Debug, Clone)]
pub struct SshTransport {
    creds: RemoteCredentials,
}

/// Parsed `ssh://[user@]host[:port]/path` URL.
#[derive(Debug, Clone)]
struct SshTarget {
    user: String,
    host: String,
    port: u16,
    path: String,
}

impl SshTransport {
    pub fn new(creds: RemoteCredentials) -> Self {
        Self { creds }
    }

    /// Parse the `url` field of `creds` into an `SshTarget`. The format
    /// is `ssh://[user@]host[:port]/path`. We accept `scp://` as an alias.
    ///
    /// Path semantics:
    /// - `ssh://host`     → no path was supplied, treat as root `"/"`
    /// - `ssh://host/`    → trailing slash with no path, treat as `""` (unset)
    /// - `ssh://host/x/y` → absolute remote path `/x/y`
    fn parse(&self) -> Result<SshTarget> {
        let raw = self.creds.url.trim();
        if raw.is_empty() {
            return Err(anyhow!("SSH credentials.url is empty"));
        }
        let stripped = raw
            .strip_prefix("ssh://")
            .or_else(|| raw.strip_prefix("scp://"))
            .unwrap_or(raw);
        // Split off the path first. The trailing-slash vs no-slash forms
        // carry different meaning (root vs unset), so we capture the
        // original "had slash" bit and resolve below.
        let (authority, had_slash, path) = match stripped.split_once('/') {
            Some((a, p)) => (a, true, format!("/{}", p)),
            None => (stripped, false, "/".to_string()),
        };
        let (user_host, port) = match authority.rsplit_once(':') {
            Some((uh, port_str)) => {
                // Make sure the part after ':' is actually a port number,
                // not part of an IPv6 address (which would have brackets).
                if let Ok(p) = port_str.parse::<u16>() {
                    (uh, p)
                } else {
                    (authority, 22)
                }
            }
            None => (authority, 22),
        };
        let (user, host) = match user_host.split_once('@') {
            Some((u, h)) => (u.to_string(), h.to_string()),
            None => {
                let user = self
                    .creds
                    .username
                    .clone()
                    .unwrap_or_else(|| whoami_user().unwrap_or_else(|_| "root".to_string()));
                (user, user_host.to_string())
            }
        };
        // Path normalization:
        // - `had_slash=false` (no slash in URL) → user did not provide a
        //   path, keep the `"/"` sentinel for "use remote root".
        // - `had_slash=true` and trailing component empty (e.g. `host/`)
        //   → treat as "unset" so downstream code skips the leading `/`.
        // - `had_slash=true` with a real component → strip any redundant
        //   trailing slashes for cleaner remote paths.
        let normalized_path = if !had_slash {
            "/".to_string()
        } else if path == "/" {
            String::new()
        } else {
            path.trim_end_matches('/').to_string()
        };
        Ok(SshTarget {
            user,
            host,
            port,
            path: normalized_path,
        })
    }

    fn run_ssh(&self, args: &[&str], remote_cmd: &str) -> Result<String> {
        let target = self.parse()?;
        let mut cmd = silent_command("ssh");
        cmd.arg("-p").arg(target.port.to_string());
        cmd.arg("-o").arg("BatchMode=yes");
        cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
        cmd.arg(format!("{}@{}", target.user, target.host));
        for a in args {
            cmd.arg(a);
        }
        cmd.arg(remote_cmd);
        let out = cmd.output().map_err(|e| {
            anyhow!("failed to spawn `ssh`: {e}. Is OpenSSH installed and on PATH?")
        })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(anyhow!(
                "ssh failed: {}",
                stderr.trim(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    fn run_scp_put(&self, local: &Path, remote_path: &str) -> Result<()> {
        let target = self.parse()?;
        let mut cmd = silent_command("scp");
        cmd.arg("-P").arg(target.port.to_string());
        cmd.arg("-o").arg("BatchMode=yes");
        cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
        cmd.arg(local);
        cmd.arg(format!(
            "{}@{}:{}/{}",
            target.user,
            target.host,
            target.path,
            remote_path
        ));
        let out = cmd.output().map_err(|e| {
            anyhow!("failed to spawn `scp`: {e}. Is OpenSSH installed and on PATH?")
        })?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Err(anyhow!("scp failed: {}", stderr.trim()));
        }
        Ok(())
    }
}

impl Transport for SshTransport {
    fn send_file(&self, entry: &FileEntry, dest_root: &Path) -> Result<()> {
        // The `dest_root` is appended to the path stored in `creds.url`.
        // For SSH we treat the URL path as the repo root and use
        // `dest_root` for any extra per-target prefix.
        let target = self.parse()?;
        let dest_root_clean = dest_root
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        let mut remote = target.path.clone();
        if !dest_root_clean.is_empty() && dest_root_clean != "." {
            if !remote.is_empty() && remote != "/" {
                remote.push('/');
            }
            remote.push_str(&dest_root_clean);
        }
        if !remote.is_empty() && remote != "/" {
            remote.push('/');
        }
        remote.push_str(&entry.rel_path);

        // Ensure parent directories exist (mkdir -p ...).
        let parent = parent_dir(&remote);
        if !parent.is_empty() {
            let cmd = format!("mkdir -p '{}'", shell_escape(&parent));
            self.run_ssh(&[], &cmd)?;
        }

        self.run_scp_put(&entry.source_abs, &remote)
    }

    fn list_dest(&self, dest_root: &Path) -> Result<Vec<String>> {
        let target = self.parse()?;
        let dest_root_clean = dest_root
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        let mut remote = target.path.clone();
        if !dest_root_clean.is_empty() && dest_root_clean != "." {
            if !remote.is_empty() && remote != "/" {
                remote.push('/');
            }
            remote.push_str(&dest_root_clean);
        }
        if remote.is_empty() {
            remote = ".".to_string();
        }
        let cmd = format!(
            "find '{}' -type f -not -path '*/\\.*' 2>/dev/null",
            shell_escape(&remote)
        );
        let out = self.run_ssh(&[], &cmd)?;
        let mut files: Vec<String> = out
            .lines()
            .map(|l| l.trim_start_matches(&remote).trim_start_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        files.sort();
        Ok(files)
    }

    fn delete_file(&self, rel_path: &str, dest_root: &Path) -> Result<()> {
        let target = self.parse()?;
        let dest_root_clean = dest_root
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        let mut remote = target.path.clone();
        if !dest_root_clean.is_empty() && dest_root_clean != "." {
            if !remote.is_empty() && remote != "/" {
                remote.push('/');
            }
            remote.push_str(&dest_root_clean);
        }
        if !remote.is_empty() && remote != "/" {
            remote.push('/');
        }
        remote.push_str(rel_path);
        let cmd = format!("rm -f '{}'", shell_escape(&remote));
        self.run_ssh(&[], &cmd)?;
        Ok(())
    }

    fn mkdir(&self, rel_path: &str, dest_root: &Path) -> Result<()> {
        let target = self.parse()?;
        let dest_root_clean = dest_root
            .to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
            .to_string();
        let mut remote = target.path.clone();
        if !dest_root_clean.is_empty() && dest_root_clean != "." {
            if !remote.is_empty() && remote != "/" {
                remote.push('/');
            }
            remote.push_str(&dest_root_clean);
        }
        if !remote.is_empty() && remote != "/" {
            remote.push('/');
        }
        remote.push_str(rel_path);
        let cmd = format!("mkdir -p '{}'", shell_escape(&remote));
        self.run_ssh(&[], &cmd)?;
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        TransportType::Ssh
    }
}

fn parent_dir(path: &str) -> String {
    match path.rsplit_once('/') {
        Some((p, _)) => p.to_string(),
        None => String::new(),
    }
}

/// Escape a string for use in a single-quoted shell command. Single
/// quotes inside the string are turned into `'\''`.
fn shell_escape(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Best-effort current OS user (for the default `user@host`).
fn whoami_user() -> Result<String> {
    let out = silent_command("whoami").output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make(url: &str) -> SshTransport {
        SshTransport::new(RemoteCredentials {
            url: url.to_string(),
            username: Some("alice".to_string()),
            ..Default::default()
        })
    }

    #[test]
    fn parse_minimal_url() {
        let t = make("ssh://example.com/var/backups");
        let p = t.parse().unwrap();
        assert_eq!(p.user, "alice");
        assert_eq!(p.host, "example.com");
        assert_eq!(p.port, 22);
        assert_eq!(p.path, "/var/backups");
    }

    #[test]
    fn parse_full_url() {
        let t = make("ssh://bob@host:2222/srv/data");
        let p = t.parse().unwrap();
        assert_eq!(p.user, "bob");
        assert_eq!(p.host, "host");
        assert_eq!(p.port, 2222);
        assert_eq!(p.path, "/srv/data");
    }

    #[test]
    fn parse_scp_alias() {
        let t = make("scp://user@example.com/");
        let p = t.parse().unwrap();
        assert_eq!(p.user, "user");
        assert_eq!(p.path, "");
    }

    #[test]
    fn parse_no_path() {
        let t = make("ssh://example.com");
        let p = t.parse().unwrap();
        assert_eq!(p.path, "/");
    }

    #[test]
    fn parse_empty_url_errors() {
        let t = SshTransport::new(RemoteCredentials::default());
        assert!(t.parse().is_err());
    }

    #[test]
    fn shell_escape_handles_single_quotes() {
        assert_eq!(shell_escape("a'b"), "a'\\''b");
        assert_eq!(shell_escape("hello"), "hello");
    }

    #[test]
    fn parent_dir_strips_filename() {
        assert_eq!(parent_dir("/var/backups/a.txt"), "/var/backups");
        assert_eq!(parent_dir("a.txt"), "");
    }
}
