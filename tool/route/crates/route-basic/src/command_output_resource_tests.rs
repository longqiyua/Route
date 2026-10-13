//! Explicit Windows resource baseline. No production supervision or RSS guarantee.
use super::capture;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    os::windows::io::AsRawHandle,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[repr(C)]
#[derive(Default)]
struct Counters {
    cb: u32,
    faults: u32,
    peak: usize,
    working: usize,
    paged_peak: usize,
    paged: usize,
    nonpaged_peak: usize,
    nonpaged: usize,
    pagefile: usize,
    pagefile_peak: usize,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn K32GetProcessMemoryInfo(
        process: *mut std::ffi::c_void,
        counters: *mut Counters,
        size: u32,
    ) -> i32;
    fn GetCurrentProcess() -> *mut std::ffi::c_void;
}
fn peak_working_set(process: *mut std::ffi::c_void) -> usize {
    let mut counters = Counters {
        cb: std::mem::size_of::<Counters>() as u32,
        ..Default::default()
    };
    // The supplied handle remains live; Counters has the Windows ABI layout.
    assert_ne!(
        unsafe {
            K32GetProcessMemoryInfo(
                process,
                &mut counters,
                std::mem::size_of::<Counters>() as u32,
            )
        },
        0
    );
    counters.peak
}
const PREFIX: &str = "command_output::resource_tests::";
fn fixture_command(test: &str) -> Command {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            &format!("{PREFIX}{test}"),
            "--ignored",
            "--nocapture",
        ])
        .env_remove("ROUTE_WORKER_CREDENTIAL");
    command
}

#[test]
#[ignore = "private resource producer; invoked by isolated collector"]
fn resource_output_producer() {
    let bytes: u64 = std::env::var("ROUTE_CAPTURE_BYTES")
        .unwrap()
        .parse()
        .unwrap();
    let chunk = [b'o'; 64 * 1024];
    let error_chunk = [b'e'; 64 * 1024];
    let mut out = std::io::stdout().lock();
    let mut err = std::io::stderr().lock();
    let mut remaining = bytes;
    while remaining != 0 {
        let count = remaining.min(chunk.len() as u64) as usize;
        // Writing stderr first exercises the pipe-deadlock failure mode.
        err.write_all(&error_chunk[..count]).unwrap();
        out.write_all(&chunk[..count]).unwrap();
        remaining -= count as u64;
    }
    out.write_all(b"stdout-tail\n").unwrap();
    err.write_all(b"stderr-tail\n").unwrap();
    out.flush().unwrap();
    err.flush().unwrap();
    // Only small measurement metadata is written, never the generated output.
    let peak = peak_working_set(unsafe { GetCurrentProcess() });
    std::fs::write(
        std::env::var("ROUTE_CAPTURE_METRICS").unwrap(),
        serde_json::to_vec(&json!({"producer_peak_working_set_bytes":peak})).unwrap(),
    )
    .unwrap();
    std::process::exit(7);
}

#[test]
#[ignore = "private isolated collector; invoked by resource baseline"]
fn resource_output_collector() {
    let bytes: u64 = std::env::var("ROUTE_CAPTURE_BYTES")
        .unwrap()
        .parse()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let metrics = dir.path().join("metrics.json");
    let mut baseline = fixture_command("resource_output_producer");
    baseline
        .env("ROUTE_CAPTURE_BYTES", "0")
        .env("ROUTE_CAPTURE_METRICS", &metrics);
    let baseline = capture(&mut baseline).unwrap();
    assert_eq!(baseline.status.code(), Some(7));
    let header = baseline
        .stdout_preview
        .strip_suffix("stdout-tail\n")
        .unwrap()
        .as_bytes();
    assert_eq!(
        baseline.stdout_hash,
        route_core::sha256_hex(baseline.stdout_preview.as_bytes())
    );
    let mut command = fixture_command("resource_output_producer");
    command
        .env("ROUTE_CAPTURE_BYTES", bytes.to_string())
        .env("ROUTE_CAPTURE_METRICS", &metrics);
    let start = Instant::now();
    let output = capture(&mut command).unwrap();
    let capture_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(output.status.code(), Some(7));
    let stdout = std::io::Cursor::new(header)
        .chain(std::io::repeat(b'o').take(bytes))
        .chain(std::io::Cursor::new(b"stdout-tail\n"));
    let stderr = std::io::repeat(b'e')
        .take(bytes)
        .chain(std::io::Cursor::new(b"stderr-tail\n"));
    let (expected_stdout, _) = route_core::hash::content_hash_with_preview(stdout, 0).unwrap();
    let (expected_stderr, _) = route_core::hash::content_hash_with_preview(stderr, 0).unwrap();
    assert_eq!(output.stdout_hash, expected_stdout);
    assert_eq!(output.stderr_hash, expected_stderr);
    assert_eq!(output.stdout_preview.len(), 2048);
    assert_eq!(output.stderr_preview, "e".repeat(2048));
    let producer: Value = serde_json::from_slice(&std::fs::read(&metrics).unwrap()).unwrap();
    let metadata_bytes = std::fs::metadata(&metrics).unwrap().len();
    assert!(metadata_bytes < 1024);
    println!(
        "CAPTURE_RESOURCE={}",
        json!({"bytes_per_channel":bytes,
        "stdout_bytes":header.len() as u64 + bytes + 12,"stderr_bytes":bytes+12,
        "capture_ms":capture_ms,"preview_bytes_per_channel":2048,"exit_code":7,
        "producer_peak_working_set_bytes":producer["producer_peak_working_set_bytes"],
        "temporary_metadata_bytes":metadata_bytes,"output_spool_bytes":0,"full_hash_and_tail":"PASS"})
    );
}

#[test]
#[ignore = "explicit 2 GiB dual-pipe output and isolated Windows resource measurement"]
fn bounded_capture_resource_baseline() {
    let mut rows = Vec::new();
    for bytes in [16_u64 * 1024 * 1024, 1024 * 1024 * 1024] {
        let mut command = fixture_command("resource_output_collector");
        let mut child = command
            .env("ROUTE_CAPTURE_BYTES", bytes.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        // Collector diagnostics only, not generated data: keep the watchdog nonblocking.
        let out = std::thread::spawn(move || {
            route_core::hash::content_hash_with_preview(stdout, 16 * 1024).unwrap()
        });
        let err = std::thread::spawn(move || {
            route_core::hash::content_hash_with_preview(stderr, 16 * 1024).unwrap()
        });
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if start.elapsed() > Duration::from_secs(180) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned collector exceeded test watchdog; production deadlines remain unimplemented");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let peak = peak_working_set(child.as_raw_handle());
        let (_, stdout) = out.join().unwrap();
        let (_, stderr) = err.join().unwrap();
        assert!(status.success(), "{}", String::from_utf8_lossy(&stderr));
        let stdout = String::from_utf8(stdout).unwrap();
        let line = stdout
            .lines()
            .find_map(|line| line.strip_prefix("CAPTURE_RESOURCE="))
            .unwrap();
        let mut row: Value = serde_json::from_str(line).unwrap();
        row["collector_peak_working_set_bytes"] = json!(peak);
        rows.push(row);
    }
    let exe = std::env::current_exe().unwrap();
    println!(
        "BOUNDED_CAPTURE_BASELINE={}",
        json!({"binary_sha256":route_core::hash::content_hash_file(&exe).unwrap().0,
        "binary_bytes":std::fs::metadata(&exe).unwrap().len(),"profile":"debug test binary, not release route.exe",
        "samples_per_size":1,"rows":rows,"deadline_and_process_tree":"NOT_IMPLEMENTED"})
    );
}
