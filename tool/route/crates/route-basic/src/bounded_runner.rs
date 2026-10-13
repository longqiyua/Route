//! Pipe supervision for owned check processes. No detached reader threads.
//! Only the direct Child handle is terminated; arbitrary descendants are NOT contained.
#[cfg(any(windows, unix))]
use crate::command_output::CapturedOutput;
#[cfg(any(windows, unix))]
use anyhow::{bail, Context, Result};
#[cfg(any(windows, unix))]
use sha2::{Digest, Sha256};
#[cfg(any(windows, unix))]
use std::{
    io::Read,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};
#[cfg(test)]
thread_local! { pub(crate) static CRASH_AFTER_SPAWN: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn PeekNamedPipe(
        handle: *mut std::ffi::c_void,
        buffer: *mut std::ffi::c_void,
        size: u32,
        read: *mut u32,
        available: *mut u32,
        remaining: *mut u32,
    ) -> i32;
}

#[cfg(all(test, any(windows, unix)))]
mod tests {
    use super::*;
    fn helper(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "bounded_runner::tests::process_fixture",
            "--ignored",
            "--nocapture",
            "--skip",
            mode,
        ]);
        command
    }
    #[test]
    #[ignore = "owned process fixture for bounded cancellation and descendant pipe tests"]
    fn process_fixture() {
        let args: Vec<_> = std::env::args().collect();
        let mode = &args[args.iter().position(|s| s == "--skip").unwrap() + 1];
        match mode.as_str() {
            "sleep" => std::thread::sleep(Duration::from_secs(2)),
            "descendant" => {
                let _child = helper("sleep").spawn().unwrap();
                std::process::exit(0);
            }
            "fail" => std::process::exit(7),
            _ => panic!("unknown fixture mode"),
        }
    }
    #[test]
    fn cancellation_is_bounded_and_only_terminates_exact_owned_child() {
        let start = Instant::now();
        let error = capture(&mut helper("sleep"), Duration::from_secs(10), || Ok(true))
            .err()
            .unwrap();
        assert!(start.elapsed() < Duration::from_secs(6));
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("RUN_CANCEL_REQUESTED"));
        assert!(diagnostic.contains("parent_termination_confirmed=true"));
        assert!(diagnostic.contains("descendants=UNKNOWN"));
    }
    #[test]
    fn exited_parent_with_inherited_pipe_is_not_fabricated_complete_output() {
        let start = Instant::now();
        let error = capture(&mut helper("descendant"), Duration::from_secs(10), || {
            Ok(false)
        })
        .err()
        .unwrap();
        assert!(start.elapsed() < Duration::from_secs(6));
        assert!(error.to_string().contains("OUTPUT_INCOMPLETE"));
        // Descendant self-exits after two seconds. No discovered PID is killed.
    }
    #[test]
    fn actual_nonzero_exit_is_retained() {
        let output = capture(&mut helper("fail"), Duration::from_secs(10), || Ok(false)).unwrap();
        assert_eq!(output.status.code(), Some(7));
    }
}
#[cfg(any(windows, unix))]
#[derive(Default)]
struct Stream {
    hash: Sha256,
    preview: Vec<u8>,
    closed: bool,
}
#[cfg(any(windows, unix))]
impl Stream {
    #[cfg(windows)]
    fn drain(&mut self, pipe: &mut (impl Read + AsRawHandle)) -> Result<usize> {
        if self.closed {
            return Ok(0);
        }
        let mut available = 0;
        // Exclusive read end: no concurrent I/O or other reader can consume peeked bytes.
        let ok = unsafe {
            PeekNamedPipe(
                pipe.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let error = std::io::Error::last_os_error();
            if matches!(error.raw_os_error(), Some(109 | 232)) {
                self.closed = true;
                return Ok(0);
            }
            return Err(error).context("CHECK_OUTPUT_READ_FAILED");
        }
        if available == 0 {
            return Ok(0);
        }
        let mut bytes = [0u8; 64 * 1024];
        let n = pipe.read(&mut bytes[..(available as usize).min(64 * 1024)])?;
        if n == 0 {
            self.closed = true;
        }
        self.hash.update(&bytes[..n]);
        self.preview
            .extend_from_slice(&bytes[..n.min(2048 - self.preview.len())]);
        Ok(n)
    }
    #[cfg(unix)]
    fn drain(&mut self, pipe: &mut (impl Read + AsRawFd)) -> Result<usize> {
        if self.closed {
            return Ok(0);
        }
        let fd = pipe.as_raw_fd();
        // Only our read descriptor is changed; the child's write end stays blocking.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        anyhow::ensure!(flags >= 0, "PIPE_FLAGS_UNAVAILABLE");
        anyhow::ensure!(
            unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } >= 0,
            "PIPE_NONBLOCKING_UNAVAILABLE"
        );
        let mut bytes = [0u8; 64 * 1024];
        let n = match pipe.read(&mut bytes) {
            Ok(n) => n,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) =>
            {
                return Ok(0)
            }
            Err(e) => return Err(e.into()),
        };
        if n == 0 {
            self.closed = true;
        }
        self.hash.update(&bytes[..n]);
        self.preview
            .extend_from_slice(&bytes[..n.min(2048 - self.preview.len())]);
        Ok(n)
    }
}
#[cfg(any(windows, unix))]
trait ProcessControl {
    fn poll(&mut self) -> Result<Option<std::process::ExitStatus>>;
    fn stop(&mut self) -> Result<()>;
    fn record_exit(&self, _status: std::process::ExitStatus) -> Result<()> {
        Ok(())
    }
    fn descendants(&self) -> &'static str {
        "UNKNOWN"
    }
}
#[cfg(any(windows, unix))]
impl ProcessControl for Child {
    fn poll(&mut self) -> Result<Option<std::process::ExitStatus>> {
        Ok(self.try_wait()?)
    }
    fn stop(&mut self) -> Result<()> {
        Ok(self.kill()?)
    }
}
#[cfg(windows)]
impl ProcessControl for crate::process_supervision::OwnedProcess {
    fn poll(&mut self) -> Result<Option<std::process::ExitStatus>> {
        self.try_wait()
    }
    fn stop(&mut self) -> Result<()> {
        self.kill()
    }
    fn record_exit(&self, status: std::process::ExitStatus) -> Result<()> {
        self.persist_result(status.code().unwrap_or(-1) as u32)
    }
    fn descendants(&self) -> &'static str {
        "JOB_CONTAINED"
    }
}
#[cfg(any(windows, unix))]
fn stop_process(child: &mut impl ProcessControl) -> bool {
    if child.poll().ok().flatten().is_some() {
        return true;
    }
    let _ = child.stop(); // Exact owned Child/Job, never a discovered PID.
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match child.poll() {
            Ok(Some(_)) => return true,
            Err(_) => return false,
            _ if Instant::now() >= until => return false,
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}
#[cfg(any(windows, unix))]
pub(crate) fn capture(
    command: &mut Command,
    deadline: Duration,
    cancelled: impl FnMut() -> Result<bool>,
) -> Result<CapturedOutput> {
    anyhow::ensure!(!deadline.is_zero(), "INVALID_DEADLINE");
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    #[cfg(test)]
    CRASH_AFTER_SPAWN.with(|crash| {
        if crash.get() {
            std::process::exit(23);
        }
    });
    let mut stdout = child.stdout.take().context("MISSING_STDOUT")?;
    let mut stderr = child.stderr.take().context("MISSING_STDERR")?;
    supervise(&mut child, &mut stdout, &mut stderr, deadline, cancelled)
}
#[cfg(windows)]
pub(crate) fn capture_owned(
    root: &std::path::Path,
    run: crate::process_supervision::RunIdentity,
    command: &Command,
    deadline: Duration,
    cancelled: impl FnMut() -> Result<bool>,
) -> Result<CapturedOutput> {
    let mut child =
        crate::process_supervision::Supervisor::open(root)?.spawn_owned(run, command, true)?;
    #[cfg(test)]
    CRASH_AFTER_SPAWN.with(|crash| {
        if crash.get() {
            std::process::exit(23);
        }
    });
    let mut stdout = child.stdout.take().context("MISSING_STDOUT")?;
    let mut stderr = child.stderr.take().context("MISSING_STDERR")?;
    supervise(&mut child, &mut stdout, &mut stderr, deadline, cancelled)
}
#[cfg(any(windows, unix))]
fn supervise<P: ProcessControl, R: Read + PipeHandle, S: Read + PipeHandle>(
    child: &mut P,
    stdout: &mut R,
    stderr: &mut S,
    deadline: Duration,
    mut cancelled: impl FnMut() -> Result<bool>,
) -> Result<CapturedOutput> {
    let mut out = Stream::default();
    let mut err = Stream::default();
    let start = Instant::now();
    let mut status = None;
    let mut exited_at = None;
    loop {
        let read = match out
            .drain(stdout)
            .and_then(|a| err.drain(stderr).map(|b| a + b))
        {
            Ok(read) => read,
            Err(error) => {
                let confirmed = stop_process(child);
                return Err(error).context(format!("OUTPUT_INCOMPLETE: parent_termination_confirmed={confirmed}; descendants=UNKNOWN"));
            }
        };
        if status.is_none() {
            status = match child.poll() {
                Ok(status) => status,
                Err(error) => {
                    let confirmed = stop_process(child);
                    return Err(error).context(format!(
                        "PROCESS_STATUS_UNKNOWN: parent_termination_confirmed={confirmed}"
                    ));
                }
            };
            if status.is_some() {
                exited_at = Some(Instant::now());
            }
        }
        if let Some(status) = status {
            if out.closed && err.closed {
                child.record_exit(status)?;
                return Ok(CapturedOutput {
                    status,
                    stdout_hash: format!("{:x}", out.hash.finalize()),
                    stderr_hash: format!("{:x}", err.hash.finalize()),
                    stdout_preview: String::from_utf8_lossy(&out.preview).into_owned(),
                    stderr_preview: String::from_utf8_lossy(&err.preview).into_owned(),
                });
            }
            if exited_at.unwrap().elapsed() > Duration::from_secs(1) {
                bail!("OUTPUT_INCOMPLETE: direct_parent_exited=true; descendant_pipe_ownership=UNKNOWN; full_hashes=false");
            }
        } else {
            let cancellation = match cancelled() {
                Ok(value) => value,
                Err(error) => {
                    let confirmed = stop_process(child);
                    return Err(error).context(format!("CANCEL_STATUS_UNKNOWN: parent_termination_confirmed={confirmed}; descendants=UNKNOWN"));
                }
            };
            if cancellation || start.elapsed() >= deadline {
                let confirmed = stop_process(child);
                // Diagnostic, not CheckFail/PASS/CANCELLED Evidence.
                bail!("{}: parent_termination_confirmed={confirmed}; descendants={}; full_hashes=false",
                    if cancellation { "RUN_CANCEL_REQUESTED" } else { "RUN_DEADLINE_EXCEEDED" }, child.descendants());
            }
        }
        if read == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

#[cfg(windows)]
trait PipeHandle: AsRawHandle {}
#[cfg(windows)]
impl<T: AsRawHandle> PipeHandle for T {}
#[cfg(unix)]
trait PipeHandle: AsRawFd {}
#[cfg(unix)]
impl<T: AsRawFd> PipeHandle for T {}
