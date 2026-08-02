//! White-box tests for the example seeder.
//!
//! We test the seeder by running it against a tempdir and inspecting
//! the resulting repo's structure. We do NOT depend on Tauri — the
//! seeder is just a sequence of `BasicRepository` calls, so we can
//! exercise it without booting the whole IPC stack.
//!
//! The deep Tauri + rusqlite + tokio call chain trips Windows'
//! 1 MB default thread stack. We run each test on a 32 MB thread
//! to dodge STATUS_STACK_OVERFLOW.

use route_basic::BranchKind;
use route_tauri::example_seed::seed_example;
use std::thread;
use tempfile::TempDir;

/// Run `f` on a fresh thread with a 32 MB stack.
fn on_big_stack<F: FnOnce() + Send + 'static>(f: F) {
    thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(f)
        .expect("spawn big-stack thread")
        .join()
        .expect("big-stack thread should not panic")
}

/// Smoke: the seeder produces a repo with three branches, at least
/// one AI commit, at least one checkpoint, and the right kinds.
#[test]
fn seed_produces_full_feature_set() {
    on_big_stack(|| {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("example");

        let repo = seed_example(&project).expect("seed_example should succeed");

        // Branches: main + sandbox + at least one inherited.
        let branches = repo.list_branches().expect("list_branches");
        let names: Vec<&str> = branches.iter().map(|b| b.name.as_str()).collect();
        assert!(names.contains(&"main"), "main branch must exist");
        assert!(
            names.iter().any(|n| n.starts_with("sandbox/")),
            "at least one sandbox branch must exist; got: {:?}",
            names
        );
        assert!(
            names
                .iter()
                .any(|n| n.starts_with("feature/") || n.starts_with("feat/")),
            "at least one inherited branch must exist; got: {:?}",
            names
        );

        // Commits: at least one AI commit, at least one checkpoint.
        let all = repo.list_commits(None, 200).expect("list_commits");
        let ai_count = all.iter().filter(|c| c.is_ai).count();
        let cp_count = all.iter().filter(|c| c.is_checkpoint).count();
        assert!(
            ai_count >= 1,
            "expected at least one AI commit, found {ai_count}"
        );
        assert!(
            cp_count >= 1,
            "expected at least one checkpoint, found {cp_count}"
        );

        // Sandbox branch must be kind=Sandbox.
        let sandbox = branches
            .iter()
            .find(|b| b.name.starts_with("sandbox/"))
            .expect("sandbox branch must exist");
        assert_eq!(sandbox.kind, BranchKind::Sandbox);

        // Inherited branch must be kind=Inherited.
        let inherited = branches
            .iter()
            .find(|b| {
                b.kind == BranchKind::Inherited
                    && (b.name.starts_with("feature/") || b.name.starts_with("feat/"))
            })
            .expect("inherited branch must exist");
        assert_eq!(inherited.kind, BranchKind::Inherited);

        // Re-opening the seeded directory must succeed (idempotent).
        // `seed_example` re-applies `with_serial_scanner` internally so
        // subsequent operations don't trip the 1 MB stack on Windows.
        let reopened = seed_example(&project).expect("reopen should succeed");
        let again = reopened.list_branches().expect("branches after reopen");
        assert_eq!(
            again.len(),
            branches.len(),
            "re-opening should not duplicate branches"
        );
    });
}

/// The seeder must drop a real source tree on disk so the user
/// can see the resulting files. We don't diff the contents — just
/// assert that the expected top-level files exist.
#[test]
fn seed_writes_expected_files() {
    on_big_stack(|| {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("example");
        let _ = seed_example(&project).unwrap();

        for rel in ["README.md", "app.py", "tests.py", "config.json", "notes.md"] {
            assert!(
                project.join(rel).exists(),
                "expected file {} to exist on disk",
                rel
            );
        }
    });
}
