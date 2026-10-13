//! Windows 10+: atomically assign the process to a unique Job at creation.
//! KILL_ON_JOB_CLOSE plus a query-only lease inherited by the root process.
//! A live lease preserves restart inspection; losing every lease terminates
//! the contained tree instead of leaving an unopenable orphaned Job.
use super::*;
use anyhow::{bail, Context};
use std::os::windows::process::ExitStatusExt;
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::{File, OpenOptions},
    mem::{size_of, zeroed},
    os::windows::{
        ffi::{OsStrExt, OsStringExt},
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    process::{Command, ExitStatus},
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{
        SetHandleInformation, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_INVALID_PARAMETER,
        FILETIME, HANDLE, HANDLE_FLAG_INHERIT, WAIT_OBJECT_0,
    },
    Security::SECURITY_ATTRIBUTES,
    System::{JobObjects::*, Pipes::CreatePipe, Threading::*},
};
const JOB_QUERY: u32 = 4;
const JOB_TERMINATE: u32 = 8;
#[cfg(test)]
thread_local! { pub(super) static CRASH_POINT: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) }; }
fn seam(point: &str) {
    #[cfg(test)]
    CRASH_POINT.with(|value| {
        if value.borrow().as_str() == point {
            std::process::exit(23);
        }
    });
    #[cfg(not(test))]
    let _ = point;
}
fn check(ok: i32, context: &str) -> Result<()> {
    if ok == 0 {
        Err(std::io::Error::last_os_error()).context(context.to_owned())
    } else {
        Ok(())
    }
}
fn handle(raw: HANDLE) -> Result<OwnedHandle> {
    ensure!(
        !raw.is_null() && raw as isize != -1,
        "WINDOWS_HANDLE_UNAVAILABLE: {}",
        std::io::Error::last_os_error()
    );
    // Ownership transferred exactly once from a successful Win32 creation/open.
    Ok(unsafe { OwnedHandle::from_raw_handle(raw) })
}
fn raw(h: &OwnedHandle) -> HANDLE {
    h.as_raw_handle()
}
fn wide(s: &OsStr) -> Result<Vec<u16>> {
    let mut w: Vec<_> = s.encode_wide().collect();
    ensure!(!w.contains(&0), "NUL_IN_PROCESS_ARGUMENT");
    w.push(0);
    Ok(w)
}
// Microsoft CRT argv convention, not shell text. Explicit .exe only.
fn quote(s: &OsStr) -> Result<Vec<u16>> {
    let w = wide(s)?;
    // Match ordinary Command argv formatting: unnecessary quotes around
    // switches are observable to interpreters such as cmd.exe.
    if w.len() > 1
        && !w[..w.len() - 1]
            .iter()
            .any(|&c| c == 32 || c == 9 || c == 34)
    {
        return Ok(w[..w.len() - 1].to_vec());
    }
    let mut out = vec![34];
    let mut slashes = 0;
    for &c in &w[..w.len() - 1] {
        if c == 92 {
            slashes += 1;
            continue;
        }
        out.extend(std::iter::repeat_n(
            92,
            if c == 34 { slashes * 2 + 1 } else { slashes },
        ));
        out.push(c);
        slashes = 0;
    }
    out.extend(std::iter::repeat_n(92, slashes * 2));
    out.push(34);
    Ok(out)
}
fn pipe() -> Result<(File, OwnedHandle)> {
    let sa = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    let (mut read, mut write) = (null_mut(), null_mut());
    check(
        unsafe { CreatePipe(&mut read, &mut write, &sa, 0) },
        "CREATE_OUTPUT_PIPE",
    )?;
    let read = handle(read)?;
    let write = handle(write)?;
    check(
        unsafe { SetHandleInformation(raw(&read), HANDLE_FLAG_INHERIT, 0) },
        "PIPE_NO_INHERIT",
    )?;
    Ok((File::from(read), write))
}
fn count(job: &OwnedHandle) -> Result<u32> {
    let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
    check(
        unsafe {
            QueryInformationJobObject(
                raw(job),
                JobObjectBasicAccountingInformation,
                &mut info as *mut _ as *mut _,
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                null_mut(),
            )
        },
        "JOB_QUERY_FAILED",
    )?;
    Ok(info.ActiveProcesses)
}
fn identity(process: &OwnedHandle, pid: u32) -> Result<ProcessIdentity> {
    let (mut created, mut exited, mut kernel, mut user): (FILETIME, FILETIME, FILETIME, FILETIME) =
        unsafe { zeroed() };
    check(
        unsafe {
            GetProcessTimes(
                raw(process),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        },
        "PROCESS_CREATION_IDENTITY_UNAVAILABLE",
    )?;
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    check(
        unsafe { QueryFullProcessImageNameW(raw(process), 0, buf.as_mut_ptr(), &mut len) },
        "PROCESS_EXECUTABLE_UNAVAILABLE",
    )?;
    let executable = PathBuf::from(OsString::from_wide(&buf[..len as usize])).canonicalize()?;
    Ok(ProcessIdentity {
        pid,
        creation_time: ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64,
        executable: executable.to_string_lossy().into_owned(),
        executable_hash: route_core::hash::content_hash_file(&executable)?.0,
    })
}
fn in_job(process: &OwnedHandle, job: &OwnedHandle) -> Result<bool> {
    let mut member = 0;
    check(
        unsafe { IsProcessInJob(raw(process), raw(job), &mut member) },
        "JOB_MEMBERSHIP_UNAVAILABLE",
    )?;
    Ok(member != 0)
}
struct Attributes {
    storage: Vec<usize>,
    ptr: LPPROC_THREAD_ATTRIBUTE_LIST,
}
impl Attributes {
    fn new() -> Result<Self> {
        let mut bytes = 0;
        unsafe { InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes) };
        ensure!(bytes > 0, "PROCESS_ATTRIBUTE_SIZE_UNAVAILABLE");
        let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        let ptr = storage.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
        check(
            unsafe { InitializeProcThreadAttributeList(ptr, 2, 0, &mut bytes) },
            "PROCESS_ATTRIBUTE_INIT",
        )?;
        Ok(Self { storage, ptr })
    }
    fn set(&self, kind: u32, handles: &[HANDLE]) -> Result<()> {
        check(
            unsafe {
                UpdateProcThreadAttribute(
                    self.ptr,
                    0,
                    kind as usize,
                    handles.as_ptr() as *const _,
                    std::mem::size_of_val(handles),
                    null_mut(),
                    null(),
                )
            },
            "PROCESS_ATTRIBUTE_SET",
        )
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe { DeleteProcThreadAttributeList(self.ptr) };
        let _ = &self.storage;
    }
}

pub struct OwnedProcess {
    process: OwnedHandle,
    job: OwnedHandle,
    supervisor: Supervisor,
    receipt: Receipt,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
}
impl OwnedProcess {
    pub fn id(&self) -> u32 {
        self.receipt.process.as_ref().expect("identified child").pid
    }
    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        if count(&self.job)? != 0 {
            return Ok(None);
        }
        let mut code = 0;
        check(
            unsafe { GetExitCodeProcess(raw(&self.process), &mut code) },
            "PROCESS_EXIT_UNAVAILABLE",
        )?;
        // Entire managed tree is gone, but this is NOT yet durable command output/Evidence.
        Ok(Some(ExitStatus::from_raw(code)))
    }
    pub fn kill(&mut self) -> Result<()> {
        self.supervisor.cancel_inner(
            &self.receipt.run.run_id,
            &format!("owned-stop:{}", self.receipt.run.owner),
            Some(&self.receipt),
        )?;
        Ok(())
    }
    pub fn wait(&mut self) -> Result<ExitStatus> {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.try_wait()? {
                return Ok(status);
            }
            ensure!(Instant::now() < until, "TREE_TERMINATION_UNCONFIRMED");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub(crate) fn persist_result(&self, code: u32) -> Result<()> {
        let _lock = self.supervisor.lock(&self.receipt.run.run_id)?;
        self.supervisor.fence(&self.receipt)?;
        let mut current = self.supervisor.load(&self.receipt.run.run_id)?;
        ensure!(count(&self.job)? == 0, "TREE_STILL_RUNNING");
        let mut actual = 0;
        check(
            unsafe { GetExitCodeProcess(raw(&self.process), &mut actual) },
            "PROCESS_EXIT_UNAVAILABLE",
        )?;
        ensure!(code == actual, "PROCESS_RESULT_MISMATCH");
        current.stage = "PROCESS_EXIT_CAPTURED".into();
        current.exit_code = Some(code);
        current.tree_termination_confirmed = true;
        self.supervisor.save(&current)
    }
}

impl Supervisor {
    pub fn spawn_owned(
        &self,
        run: RunIdentity,
        command: &Command,
        capture: bool,
    ) -> Result<OwnedProcess> {
        let setup_start = Instant::now();
        ensure!(
            !run.run_id.is_empty()
                && !run.operation_id.is_empty()
                && !run.owner.is_empty()
                && run.attempt > 0,
            "INVALID_RUN_IDENTITY"
        );
        let _lock = self.lock(&run.run_id)?;
        if self.path(&run.run_id).exists() {
            let old = self.load(&run.run_id)?;
            let canonical = crate::check_run::inspect(&self.root, &run.operation_id)?;
            ensure!(
                old.stage == "BEFORE_SPAWN"
                    && old.process.is_none()
                    && old.run.team_goal.is_none()
                    && run.team_goal.is_none()
                    && old.run.operation_id == run.operation_id
                    && run.attempt > old.run.attempt
                    && run.owner != old.run.owner
                    && canonical.run_id == run.run_id
                    && canonical.owner == run.owner
                    && canonical.attempt == run.attempt
                    && canonical.state == crate::check_run::CheckRunState::PossiblyRunning
                    && self.inspect(&run.run_id)?.active_processes == Some(0),
                "PROCESS_ALREADY_RESERVED: inspect/recover; never relaunch uncertain process"
            );
            let history = self.path(&run.run_id).with_file_name(format!(
                "{}.launch-history-{}.json",
                route_core::sha256_hex(run.run_id.as_bytes()),
                route_core::sha256_hex(old.nonce.as_bytes())
            ));
            crate::ownership_lock::atomic_replace(&history, &serde_json::to_vec_pretty(&old)?)?;
        }
        let exe = PathBuf::from(command.get_program()).canonicalize()?;
        let approved_hash = route_core::hash::content_hash_file(&exe)?.0;
        ensure!(
            exe.extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("exe")),
            "EXPLICIT_EXE_REQUIRED: scripts require an approved interpreter"
        );
        let application = wide(exe.as_os_str())?;
        let mut argv = quote(exe.as_os_str())?;
        for arg in command.get_args() {
            argv.push(32);
            argv.extend(quote(arg)?);
        }
        argv.push(0);
        let cwd = wide(command.get_current_dir().unwrap_or(&self.root).as_os_str())?;
        // Keep inherited environment, apply removals/overrides case-insensitively.
        let mut env: BTreeMap<String, (OsString, OsString)> = std::env::vars_os()
            .map(|(k, v)| (k.to_string_lossy().to_uppercase(), (k, v)))
            .collect();
        for (k, v) in command.get_envs() {
            let key = k.to_string_lossy().to_uppercase();
            if let Some(v) = v {
                env.insert(key, (k.into(), v.into()));
            } else {
                env.remove(&key);
            }
        }
        let mut environment = Vec::<u16>::new();
        for (_, (key, value)) in env {
            let mut pair = key;
            pair.push("=");
            pair.push(value);
            environment.extend(wide(&pair)?);
        }
        if environment.is_empty() {
            environment.push(0);
        }
        environment.push(0);
        let nonce = route_core::new_id();
        let containment = job_name(&self.root, &run, &nonce);
        let mut receipt = Receipt {
            schema: 1,
            project: self.root.to_string_lossy().into_owned(),
            run,
            nonce,
            containment,
            stage: "BEFORE_SPAWN".into(),
            process: None,
            exit_code: None,
            cancellation: None,
            tree_termination_confirmed: false,
            overhead_ms: BTreeMap::new(),
        };
        self.save(&receipt)?;
        seam("before_spawn");
        let name = wide(OsStr::new(&receipt.containment))?;
        let job_raw = unsafe { CreateJobObjectW(null(), name.as_ptr()) };
        let creation_error = std::io::Error::last_os_error();
        let job = handle(job_raw)?;
        ensure!(
            creation_error.raw_os_error() != Some(ERROR_ALREADY_EXISTS as i32),
            "JOB_NAME_COLLISION"
        );
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        check(
            unsafe {
                SetInformationJobObject(
                    raw(&job),
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            },
            "JOB_LIFETIME_POLICY_FAILED",
        )?;
        // Root owns only a query lease, never an inherited terminate/set-limit
        // capability. It keeps the named object open after Route crashes.
        let lease = handle(unsafe { OpenJobObjectW(JOB_QUERY, 1, name.as_ptr()) })?;
        let null_file = OpenOptions::new().read(true).write(true).open("NUL")?;
        check(
            unsafe {
                SetHandleInformation(
                    null_file.as_raw_handle(),
                    HANDLE_FLAG_INHERIT,
                    HANDLE_FLAG_INHERIT,
                )
            },
            "NUL_HANDLE_INHERIT",
        )?;
        let (stdout, out_write) = if capture {
            let (r, w) = pipe()?;
            (Some(r), Some(w))
        } else {
            (None, None)
        };
        let (stderr, err_write) = if capture {
            let (r, w) = pipe()?;
            (Some(r), Some(w))
        } else {
            (None, None)
        };
        let out = out_write
            .as_ref()
            .map(raw)
            .unwrap_or(null_file.as_raw_handle());
        let err = err_write
            .as_ref()
            .map(raw)
            .unwrap_or(null_file.as_raw_handle());
        let mut handles = vec![null_file.as_raw_handle(), out, err, raw(&lease)];
        handles.sort_by_key(|h| *h as usize);
        handles.dedup();
        let attrs = Attributes::new()?;
        let jobs = [raw(&job)];
        attrs.set(PROC_THREAD_ATTRIBUTE_JOB_LIST, &jobs)?;
        attrs.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &handles)?;
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = null_file.as_raw_handle();
        startup.StartupInfo.hStdOutput = out;
        startup.StartupInfo.hStdError = err;
        startup.lpAttributeList = attrs.ptr;
        receipt.stage = "SPAWN_ATTEMPTED_BUT_IDENTITY_NOT_DURABLE".into();
        self.save(&receipt)?;
        let mut pi: PROCESS_INFORMATION = unsafe { zeroed() };
        receipt.overhead_ms.insert(
            "supervision_setup".into(),
            setup_start.elapsed().as_secs_f64() * 1000.0,
        );
        check(
            unsafe {
                CreateProcessW(
                    application.as_ptr(),
                    argv.as_mut_ptr(),
                    null(),
                    null(),
                    1,
                    EXTENDED_STARTUPINFO_PRESENT
                        | CREATE_SUSPENDED
                        | CREATE_UNICODE_ENVIRONMENT
                        | CREATE_NO_WINDOW,
                    environment.as_ptr() as *const _,
                    cwd.as_ptr(),
                    &startup.StartupInfo,
                    &mut pi,
                )
            },
            "ATOMIC_JOB_SPAWN_FAILED: no uncontained fallback",
        )?;
        let process = handle(pi.hProcess)?;
        let thread = handle(pi.hThread)?;
        seam("after_create");
        let identified = (|| -> Result<()> {
            ensure!(in_job(&process, &job)?, "ATOMIC_JOB_MEMBERSHIP_FAILED");
            receipt.process = Some(identity(&process, pi.dwProcessId)?);
            ensure!(
                receipt.process.as_ref().unwrap().executable_hash == approved_hash,
                "LAUNCH_EXECUTABLE_CHANGED"
            );
            receipt.stage = "OWNED_PROCESS_IDENTIFIED".into();
            let persist_start = Instant::now();
            self.save(&receipt)?;
            receipt.overhead_ms.insert(
                "identity_persistence".into(),
                persist_start.elapsed().as_secs_f64() * 1000.0,
            );
            seam("after_identity");
            ensure!(
                unsafe { ResumeThread(raw(&thread)) } != u32::MAX,
                "PROCESS_RESUME_FAILED"
            );
            receipt.stage = "RUNNING".into();
            self.save(&receipt)
        })();
        if let Err(e) = identified {
            // This exact newly created Job, never a PID discovered from metadata.
            unsafe { TerminateJobObject(raw(&job), 1) };
            return Err(e);
        }
        drop(thread);
        drop(out_write);
        drop(err_write);
        Ok(OwnedProcess {
            process,
            job,
            receipt,
            stdout,
            stderr,
            supervisor: Supervisor {
                root: self.root.clone(),
                authority: self.authority.clone(),
            },
        })
    }
    fn open_job(&self, receipt: &Receipt, terminate: bool) -> Result<Option<OwnedHandle>> {
        let name = wide(OsStr::new(&receipt.containment))?;
        let h = unsafe {
            OpenJobObjectW(
                JOB_QUERY | if terminate { JOB_TERMINATE } else { 0 },
                0,
                name.as_ptr(),
            )
        };
        if h.is_null() {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) {
                return Ok(None);
            }
            return Err(error).context("JOB_OWNERSHIP_UNAVAILABLE");
        }
        Ok(Some(handle(h)?))
    }
    fn prove(&self, receipt: &Receipt, job: &OwnedHandle) -> Result<()> {
        if let Some(expected) = &receipt.process {
            let h = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZATION_SYNCHRONIZE,
                    0,
                    expected.pid,
                )
            };
            if h.is_null() {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() == Some(ERROR_INVALID_PARAMETER as i32) {
                    return Ok(());
                } // Parent gone; owned Job may contain descendants.
                return Err(error).context("PROCESS_IDENTITY_UNKNOWN");
            }
            let process = handle(h)?;
            // An exited root may no longer expose its image path. Creation
            // identity remains mandatory; a reused PID cannot enter this path.
            if unsafe { WaitForSingleObject(raw(&process), 0) } == WAIT_OBJECT_0 {
                let (mut created, mut exited, mut kernel, mut user): (
                    FILETIME,
                    FILETIME,
                    FILETIME,
                    FILETIME,
                ) = unsafe { zeroed() };
                check(
                    unsafe {
                        GetProcessTimes(
                            raw(&process),
                            &mut created,
                            &mut exited,
                            &mut kernel,
                            &mut user,
                        )
                    },
                    "EXITED_ROOT_IDENTITY_UNKNOWN",
                )?;
                let creation =
                    ((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64;
                ensure!(
                    creation == expected.creation_time,
                    "PROCESS_IDENTITY_MISMATCH"
                );
                return Ok(());
            }
            let actual = identity(&process, expected.pid)?;
            ensure!(
                actual.creation_time == expected.creation_time
                    && actual.executable == expected.executable
                    && actual.executable_hash == expected.executable_hash
                    && in_job(&process, job)?,
                "PROCESS_IDENTITY_MISMATCH: no PID-only control"
            );
        }
        Ok(())
    }
    pub fn inspect(&self, run_id: &str) -> Result<Inspection> {
        let receipt = self.load(run_id)?;
        let observed = (|| -> Result<(String, Option<u32>)> {
            if let Some(job) = self.open_job(&receipt, false)? {
                let active = count(&job)?;
                if active > 0 {
                    self.prove(&receipt, &job)?;
                }
                return Ok((
                    if active > 0 {
                        "RUNNING"
                    } else if receipt.stage == "RESULT_PERSISTED" {
                        "RESULT_PERSISTED"
                    } else {
                        "LOST_RESULT"
                    }
                    .into(),
                    Some(active),
                ));
            }
            // Last-handle close requests kernel termination, but a missing name
            // is not an observation that asynchronous termination completed.
            Ok((
                if receipt.stage == "BEFORE_SPAWN" {
                    "BEFORE_SPAWN"
                } else if receipt.stage == "RESULT_PERSISTED" {
                    "RESULT_PERSISTED"
                } else {
                    "LOST_RESULT"
                }
                .into(),
                if receipt.stage == "BEFORE_SPAWN" || receipt.tree_termination_confirmed {
                    Some(0)
                } else {
                    None
                },
            ))
        })();
        let (mut knowledge, active) = match observed {
            Ok(v) => v,
            Err(_) => ("RECOVERY_REQUIRED".into(), None),
        };
        if active == Some(0) {
            if let Ok(check) = crate::check_run::inspect(&self.root, &receipt.run.operation_id) {
                if check.run_id == run_id
                    && check.result.is_some()
                    && matches!(
                        check.state,
                        crate::check_run::CheckRunState::ResultPersisted
                            | crate::check_run::CheckRunState::EvidenceCommitted
                    )
                {
                    knowledge = "RESULT_PERSISTED".into();
                }
            }
        }
        let mut blocked = knowledge != "RESULT_PERSISTED";
        if active == Some(0) {
            if crate::check_run::inspect(&self.root, &receipt.run.operation_id).is_ok_and(|c| {
                c.run_id == run_id && c.state == crate::check_run::CheckRunState::LostResult
            }) {
                blocked = false;
            }
            if let Some(goal) = receipt.run.team_goal.as_deref() {
                if let Ok(view) = crate::team::status(&self.root, goal) {
                    if let Some(run) = view.runs.get(run_id) {
                        if !run.state.active() {
                            blocked = false;
                        }
                    }
                }
            }
        }
        Ok(Inspection { receipt, knowledge, active_processes: active,
            capacity_blocked_by_uncertain_run: blocked, output_after_restart: "UNKNOWN: supervisor pipes are not continuous",
            safe_next_action: "inspect first; keyed cancel only owned Job; keyed recover records knowledge, never relaunches or fabricates Evidence" })
    }
    pub fn cancel(&self, run_id: &str, operation_key: &str) -> Result<Inspection> {
        self.cancel_inner(run_id, operation_key, None)
    }
    fn cancel_inner(
        &self,
        run_id: &str,
        operation_key: &str,
        expected: Option<&Receipt>,
    ) -> Result<Inspection> {
        ensure!(!operation_key.is_empty(), "OPERATION_KEY_REQUIRED");
        let _lock = self.lock(run_id)?;
        if let Some(expected) = expected {
            self.fence(expected)?;
        }
        let mut receipt = self.load(run_id)?;
        if let Some(cancel) = &receipt.cancellation {
            ensure!(
                cancel.operation_key == operation_key,
                "CANCEL_OPERATION_CONFLICT"
            );
            if cancel.state == "TERMINATION_CONFIRMED" {
                return self.inspect(run_id);
            }
        }
        let job = self.open_job(&receipt, true)?;
        if let Some(job) = &job {
            if count(job)? > 0 {
                self.prove(&receipt, job)?;
            }
        }
        let Some(job) = job else {
            receipt.cancellation = Some(Cancellation {
                operation_key: operation_key.into(),
                state: "CANCEL_UNCONFIRMED".into(),
            });
            self.save(&receipt)?;
            return self.inspect(run_id);
        };
        if count(&job)? == 0 {
            receipt.tree_termination_confirmed = true;
            if receipt
                .cancellation
                .as_ref()
                .is_some_and(|c| c.state != "TERMINATION_CONFIRMED")
            {
                receipt.cancellation.as_mut().unwrap().state = "TERMINATION_CONFIRMED".into();
                if receipt.stage != "RESULT_PERSISTED" {
                    receipt.stage = "TERMINATED_WITHOUT_RESULT".into();
                }
            }
            self.save(&receipt)?;
            // Preserve any real result and do not call natural completion cancellation.
            return self.inspect(run_id);
        }
        receipt.cancellation = Some(Cancellation {
            operation_key: operation_key.into(),
            state: "CANCEL_REQUESTED".into(),
        });
        let request_start = Instant::now();
        self.save(&receipt)?;
        receipt.overhead_ms.insert(
            "cancel_request_persistence".into(),
            request_start.elapsed().as_secs_f64() * 1000.0,
        );
        seam("cancel_requested");
        if unsafe { TerminateJobObject(raw(&job), 1) } == 0 {
            receipt.cancellation.as_mut().unwrap().state = "CANCEL_UNCONFIRMED".into();
            self.save(&receipt)?;
            bail!("TREE_TERMINATION_UNCONFIRMED");
        }
        receipt.cancellation.as_mut().unwrap().state = "CANCEL_SIGNALLED".into();
        self.save(&receipt)?;
        let until = Instant::now() + Duration::from_secs(5);
        let confirm_start = Instant::now();
        while count(&job)? > 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        let confirmed = count(&job)? == 0;
        receipt.tree_termination_confirmed = confirmed;
        receipt.overhead_ms.insert(
            "termination_confirmation".into(),
            confirm_start.elapsed().as_secs_f64() * 1000.0,
        );
        receipt.cancellation.as_mut().unwrap().state = if confirmed {
            "TERMINATION_CONFIRMED"
        } else {
            "CANCEL_UNCONFIRMED"
        }
        .into();
        if confirmed && receipt.stage != "RESULT_PERSISTED" {
            receipt.stage = "TERMINATED_WITHOUT_RESULT".into();
        }
        self.save(&receipt)?;
        self.inspect(run_id)
    }
}
