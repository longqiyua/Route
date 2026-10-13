//! Bounded output capture for verification commands; no transcript or spool file.
use anyhow::Result;
#[cfg(not(any(windows, unix)))]
use anyhow::{anyhow, Context};
#[cfg(not(any(windows, unix)))]
use std::process::Stdio;
use std::process::{Command, ExitStatus};

pub(crate) struct CapturedOutput {
    pub status: ExitStatus,
    pub stdout_hash: String,
    pub stderr_hash: String,
    pub stdout_preview: String,
    pub stderr_preview: String,
}

#[cfg_attr(any(windows, unix), allow(dead_code))]
pub(crate) fn capture(command: &mut Command) -> Result<CapturedOutput> {
    #[cfg(any(windows, unix))]
    {
        return crate::bounded_runner::capture(
            command,
            std::time::Duration::from_secs(900),
            || Ok(false),
        );
    }
    #[cfg(not(any(windows, unix)))]
    capture_legacy(command)
}

#[cfg(not(any(windows, unix)))]
fn capture_legacy(command: &mut Command) -> Result<CapturedOutput> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().context("missing stdout pipe")?;
    let stderr = child.stderr.take().context("missing stderr pipe")?;
    // Drain both pipes concurrently so either stream can exceed OS pipe capacity.
    let streams = std::thread::scope(|scope| -> Result<_> {
        let stderr_reader = match std::thread::Builder::new()
            .name("route-check-stderr".into())
            .spawn_scoped(scope, move || {
                route_core::hash::content_hash_with_preview(stderr, 2048)
            }) {
            Ok(reader) => reader,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error.into());
            }
        };
        let stdout_result = route_core::hash::content_hash_with_preview(stdout, 2048);
        if stdout_result.is_err() {
            let _ = child.kill();
        }
        let stderr_result = stderr_reader
            .join()
            .map_err(|_| anyhow!("stderr reader panicked"));
        Ok((stdout_result?, stderr_result??))
    });
    if streams.is_err() {
        let _ = child.kill();
    }
    // Always reap our child, including output-read failures. Never produce Evidence
    // from a partial capture. Deadline/process-tree supervision remains separate work.
    let status = child.wait();
    let ((stdout_hash, stdout_preview), (stderr_hash, stderr_preview)) = streams?;
    Ok(CapturedOutput {
        status: status?,
        stdout_hash,
        stderr_hash,
        stdout_preview: String::from_utf8_lossy(&stdout_preview).into_owned(),
        stderr_preview: String::from_utf8_lossy(&stderr_preview).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    #[ignore = "output producer invoked by the parent regression"]
    fn dual_pipe_fixture() {
        let chunk = [b'x'; 64 * 1024];
        for _ in 0..256 {
            std::io::stderr().write_all(&chunk).unwrap();
            std::io::stdout().write_all(&chunk).unwrap();
        }
        std::process::exit(7);
    }

    #[test]
    fn large_dual_pipe_output_preserves_full_hash_and_nonzero_exit() {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "command_output::tests::dual_pipe_fixture",
            "--ignored",
            "--nocapture",
        ]);
        let output = capture(&mut command).unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stderr_preview, "x".repeat(2048));
        assert_eq!(output.stdout_preview.len(), 2048);
        assert_eq!(
            output.stderr_hash,
            route_core::sha256_hex(&vec![b'x'; 16 * 1024 * 1024])
        );
        assert_ne!(
            output.stdout_hash,
            route_core::sha256_hex(output.stdout_preview.as_bytes())
        );
    }
}

#[cfg(all(test, windows))]
#[path = "command_output_resource_tests.rs"]
mod resource_tests;
