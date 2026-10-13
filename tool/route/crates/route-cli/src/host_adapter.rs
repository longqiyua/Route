//! Minimal argv-based Host boundary. Only Codex is implemented/certified here.
use anyhow::{ensure, Context, Result};
use std::{
    path::Path,
    process::{Command, Stdio},
    time::Instant,
};
#[cfg(not(windows))]
type Child = std::process::Child;
#[cfg(windows)]
type Child = route_basic::process_supervision::OwnedProcess;
pub(crate) struct HostLaunch {
    pub child: Child,
    pub verification_ms: f64,
    pub spawn_ms: f64,
}
pub(crate) trait HostAdapter {
    fn spawn(
        &self,
        root: &Path,
        prompt: &str,
        run: route_basic::process_supervision::RunIdentity,
    ) -> Result<HostLaunch>;
}
pub(crate) struct CodexAdapter<'a> {
    pub path: &'a Path,
    pub approved_sha256: &'a str,
}
impl HostAdapter for CodexAdapter<'_> {
    fn spawn(
        &self,
        root: &Path,
        prompt: &str,
        run: route_basic::process_supervision::RunIdentity,
    ) -> Result<HostLaunch> {
        let start = Instant::now();
        ensure!(
            route_core::hash::content_hash_file(self.path)?.0 == self.approved_sha256,
            "HOST_BINARY_CHANGED: explicit new approval required"
        );
        let verification_ms = start.elapsed().as_secs_f64() * 1000.0;
        let mut cmd = Command::new(self.path);
        if cfg!(windows) {
            cmd.args(["-c", "windows.sandbox=mxc"]);
        }
        let start = Instant::now();
        cmd.args([
            "-a",
            "never",
            "exec",
            "--ephemeral",
            "--json",
            "-s",
            "workspace-write",
            "-C",
        ])
        .arg(root)
        .arg(prompt)
        .env_remove("ROUTE_WORKER_CREDENTIAL")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
        #[cfg(windows)]
        let child = route_basic::process_supervision::Supervisor::open(root)?
            .spawn_owned(run, &cmd, false)
            .context("starting approved contained Codex")?;
        #[cfg(not(windows))]
        let child = {
            let _ = run;
            cmd.spawn()
                .context("starting approved Codex (direct-child LIMITED)")?
        };
        Ok(HostLaunch {
            child,
            verification_ms,
            spawn_ms: start.elapsed().as_secs_f64() * 1000.0,
        })
    }
}
