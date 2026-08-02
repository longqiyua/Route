//! Scheduler — runs sync targets on a periodic interval.
//!
//! For now, this is a simple interval-based scheduler that runs in a background thread.
//! Phase 5 will add cron expressions and event hooks.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::config::{SyncConfig, SyncTarget};
use crate::engine::{SyncEngine, SyncResult};

/// A scheduled sync job.
pub struct ScheduledJob {
    pub target_name: String,
    pub interval_secs: u64,
    pub running: Arc<AtomicBool>,
    pub handle: Option<thread::JoinHandle<()>>,
    pub last_result: Arc<Mutex<Option<SyncResult>>>,
}

impl ScheduledJob {
    /// Start a new scheduled job that runs the target periodically.
    /// The job uses its own `SyncEngine` instance; pass an event bus via
    /// `with_event_bus` if you need plugin dispatch.
    pub fn start(target: SyncTarget, interval_secs: u64) -> Self {
        Self::start_with_engine(target, interval_secs, Arc::new(SyncEngine::new()))
    }

    /// Start a scheduled job with a pre-built engine (event-bus attached).
    pub fn start_with_engine(
        target: SyncTarget,
        interval_secs: u64,
        engine: Arc<SyncEngine>,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let last_result = Arc::new(Mutex::new(None::<SyncResult>));

        let running_clone = running.clone();
        let last_result_clone = last_result.clone();
        let target_name = target.name.clone();

        let handle = thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                let result = engine.run(&target);
                *last_result_clone.lock().unwrap() = Some(result);
                // Sleep in small increments so we can stop quickly
                for _ in 0..interval_secs {
                    if !running_clone.load(Ordering::Relaxed) {
                        return;
                    }
                    thread::sleep(Duration::from_secs(1));
                }
            }
        });

        Self {
            target_name,
            interval_secs,
            running,
            handle: Some(handle),
            last_result,
        }
    }

    /// Stop the scheduled job.
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    /// Get the last sync result.
    pub fn last_result(&self) -> Option<SyncResult> {
        self.last_result.lock().unwrap().clone()
    }
}

impl Drop for ScheduledJob {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Manages multiple scheduled jobs.
pub struct Scheduler {
    jobs: Vec<ScheduledJob>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self { jobs: vec![] }
    }

    /// Add a sync target with a periodic interval.
    pub fn add(&mut self, target: SyncTarget, interval_secs: u64) {
        let job = ScheduledJob::start(target, interval_secs);
        self.jobs.push(job);
    }

    /// Start all targets from a config with the given default interval.
    pub fn start_all(config: &SyncConfig, default_interval_secs: u64) -> Self {
        let mut scheduler = Self::new();
        for target in &config.targets {
            if target.enabled {
                scheduler.add(target.clone(), default_interval_secs);
            }
        }
        scheduler
    }

    /// Stop all jobs.
    pub fn stop_all(&mut self) {
        for job in &mut self.jobs {
            job.stop();
        }
        self.jobs.clear();
    }

    /// Run all targets once (manual trigger).
    pub fn run_all_once(config: &SyncConfig) -> Vec<SyncResult> {
        let engine = SyncEngine::new();
        config
            .targets
            .iter()
            .filter(|t| t.enabled)
            .map(|t| engine.run(t))
            .collect()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::SyncMode;
    use tempfile::TempDir;

    #[test]
    fn run_all_once_executes_targets() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.txt"), b"hello").unwrap();

        let config = SyncConfig {
            targets: vec![SyncTarget {
                name: "t1".into(),
                source: src,
                destination: dst,
                mode: SyncMode::Mirror,
                transport: Default::default(),
                conflict: Default::default(),
                archive_rule: Default::default(),
                ignore_patterns: vec![],
                enabled: true,
                credentials: None,
            }],
            default_transport: Default::default(),
        };

        let results = Scheduler::run_all_once(&config);
        assert_eq!(results.len(), 1);
        assert!(results[0].error.is_none(), "{:?}", results[0].error);
        assert!(results[0].stats.files_copied > 0);
    }

    #[test]
    fn scheduler_stops_cleanly() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.txt"), b"x").unwrap();

        let target = SyncTarget {
            name: "t1".into(),
            source: src,
            destination: dst,
            mode: SyncMode::Backup,
            transport: Default::default(),
            conflict: Default::default(),
            archive_rule: Default::default(),
            ignore_patterns: vec![],
            enabled: true,
            credentials: None,
        };

        let mut job = ScheduledJob::start(target, 60);
        // Give it a moment to run once
        thread::sleep(Duration::from_millis(500));
        job.stop();
        // Should not hang
        assert!(!job.running.load(Ordering::Relaxed));
    }
}
