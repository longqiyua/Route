//! Independent-process storage regressions, not fabricated verification Evidence.
use super::*;
use std::{
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn add(store: &mut EvidenceStore, label: &str) {
    let mut metadata = HashMap::new();
    metadata.insert("check_id".into(), "synthetic-concurrent-check".into());
    store.record(
        "synthetic-session",
        if label == "a" {
            EvidenceKind::CheckPass
        } else {
            EvidenceKind::CheckFail
        },
        EvidenceSource::System,
        label.into(),
        metadata,
    );
}

#[test]
#[ignore = "subprocess storage fixture invoked by parent tests"]
fn writer_fixture() {
    let root = PathBuf::from(std::env::var_os("ROUTE_EVIDENCE_FIXTURE_ROOT").unwrap());
    let label = std::env::var("ROUTE_EVIDENCE_FIXTURE_LABEL").unwrap();
    let mut store = EvidenceStore::load(&root).unwrap();
    add(&mut store, &label);
    std::fs::write(root.join(format!("ready-{label}")), b"ready").unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while !root.join("go").exists() {
        assert!(Instant::now() < deadline, "barrier timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    let mode = std::env::var("ROUTE_EVIDENCE_FIXTURE_MODE").unwrap();
    let result = if mode == "exec" {
        let session_id = std::env::var("ROUTE_EVIDENCE_FIXTURE_SESSION").unwrap();
        let argv: Vec<String> = if cfg!(windows) {
            vec![
                "cmd.exe".into(),
                "/d".into(),
                "/c".into(),
                "echo storage-regression".into(),
            ]
        } else {
            vec!["sh".into(), "-c".into(), "printf storage-regression".into()]
        };
        exec_command_with_evidence(&root, &session_id, &argv, Some("same-concurrent-check")).map(
            |(output, id)| {
                assert_eq!(output.exit_code, 0);
                std::fs::write(root.join(".route").join(format!("receipt-{label}")), id).unwrap();
            },
        )
    } else if mode == "transaction" {
        EvidenceStore::update(&root, |fresh| {
            add(fresh, &label);
            Ok(())
        })
    } else {
        store.save(&root)
    };
    std::process::exit(match result {
        Ok(()) => 0,
        Err(error) if error.to_string().contains("EVIDENCE_WRITE_CONFLICT") => 73,
        Err(error) => panic!("unexpected persistence error: {error:#}"),
    });
}

fn writers(root: &Path, mode: &str) -> [Child; 2] {
    ["a", "b"].map(|label| {
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "execution::evidence_storage_tests::writer_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("ROUTE_EVIDENCE_FIXTURE_ROOT", root)
            .env("ROUTE_EVIDENCE_FIXTURE_LABEL", label)
            .env("ROUTE_EVIDENCE_FIXTURE_MODE", mode)
            .env(
                "ROUTE_EVIDENCE_FIXTURE_SESSION",
                std::fs::read_to_string(root.join("fixture-session")).unwrap_or_default(),
            )
            .stdout(Stdio::null())
            .spawn()
            .unwrap()
    })
}

#[test]
fn concurrent_real_commands_both_persist_evidence() {
    let dir = tempfile::tempdir().unwrap();
    crate::repository::BasicRepository::init(dir.path()).unwrap();
    let session = begin_session(
        dir.path(),
        "Evidence storage regression",
        ApplyTarget::Claude,
        false,
        None,
    )
    .unwrap();
    std::fs::write(dir.path().join("fixture-session"), &session.id).unwrap();
    let mut children = writers(dir.path(), "exec");
    release(dir.path(), &mut children);
    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let store = EvidenceStore::load(dir.path()).unwrap();
    let checks: Vec<_> = store
        .evidence
        .iter()
        .filter(|e| e.session_id == session.id && e.metadata.contains_key("check_id"))
        .collect();
    assert_eq!(checks.len(), 2);
    let ids = ["a", "b"].map(|label| {
        std::fs::read_to_string(dir.path().join(".route").join(format!("receipt-{label}"))).unwrap()
    });
    assert_ne!(ids[0], ids[1]);
    for id in &ids {
        let evidence = checks.iter().find(|e| &e.id == id).unwrap();
        assert_eq!(
            evidence.metadata.get("check_id").map(String::as_str),
            Some("same-concurrent-check")
        );
        assert_eq!(evidence.source, EvidenceSource::System);
    }
    assert!(checks
        .iter()
        .all(|e| e.metadata.get("exit_code").map(String::as_str) == Some("0")));
}

fn release(root: &Path, children: &mut [Child; 2]) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !["ready-a", "ready-b"]
        .iter()
        .all(|name| root.join(name).exists())
    {
        if Instant::now() >= deadline
            || children
                .iter_mut()
                .any(|child| child.try_wait().unwrap().is_some())
        {
            for child in children.iter_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
            panic!("both writers must load the same snapshot before release");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    std::fs::write(root.join("go"), b"go").unwrap();
}

#[test]
fn stale_evidence_save_cannot_overwrite_concurrent_process() {
    let dir = tempfile::tempdir().unwrap();
    let mut children = writers(dir.path(), "snapshot");
    release(dir.path(), &mut children);
    let mut codes = children
        .each_mut()
        .map(|child| child.wait().unwrap().code().unwrap());
    codes.sort();
    let retained = EvidenceStore::load(dir.path()).unwrap().evidence.len();
    assert_eq!(
        codes,
        [0, 73],
        "one stale writer must fail rather than overwrite committed history; retained={retained}"
    );
    assert_eq!(retained, 1);
}

#[test]
fn concurrent_evidence_transactions_preserve_both_results() {
    let dir = tempfile::tempdir().unwrap();
    let mut children = writers(dir.path(), "transaction");
    release(dir.path(), &mut children);
    for child in &mut children {
        assert!(child.wait().unwrap().success());
    }
    let store = EvidenceStore::load(dir.path()).unwrap();
    assert_eq!(store.evidence.len(), 2);
    assert_ne!(store.evidence[0].id, store.evidence[1].id);
    let pass = store
        .evidence
        .iter()
        .position(|e| e.kind == EvidenceKind::CheckPass)
        .unwrap();
    let fail = store
        .evidence
        .iter()
        .position(|e| e.kind == EvidenceKind::CheckFail)
        .unwrap();
    assert_eq!(
        store.has_later_failed_check(&store.evidence[pass]),
        fail > pass
    );
}

#[test]
fn recorded_command_identity_does_not_follow_latest_matching_check() {
    let dir = tempfile::tempdir().unwrap();
    crate::repository::BasicRepository::init(dir.path()).unwrap();
    let session = begin_session(
        dir.path(),
        "Exact check receipts",
        ApplyTarget::Claude,
        false,
        None,
    )
    .unwrap();
    let argv = |code: i32| -> Vec<String> {
        if cfg!(windows) {
            vec![
                "cmd.exe".into(),
                "/d".into(),
                "/c".into(),
                format!("exit {code}"),
            ]
        } else {
            vec!["sh".into(), "-c".into(), format!("exit {code}")]
        }
    };
    let (first, first_id) =
        exec_command_with_evidence(dir.path(), &session.id, &argv(0), Some("same-check")).unwrap();
    let (_, second_id) =
        exec_command_with_evidence(dir.path(), &session.id, &argv(0), Some("same-check")).unwrap();
    let (failed, failed_id) =
        exec_command_with_evidence(dir.path(), &session.id, &argv(7), Some("same-check")).unwrap();
    assert_ne!(first_id, second_id);
    assert_ne!(failed_id, first_id);
    assert_eq!(failed.exit_code, 7);
    let store = EvidenceStore::load(dir.path()).unwrap();
    let proof = store.evidence.iter().find(|e| e.id == first_id).unwrap();
    assert_eq!(proof.kind, EvidenceKind::CheckPass);
    assert_eq!(proof.state_hash.as_deref(), Some(first.state_hash.as_str()));
    let latest_pass = store
        .evidence
        .iter()
        .rev()
        .find(|e| {
            e.kind == EvidenceKind::CheckPass
                && e.metadata.get("check_id").map(String::as_str) == Some("same-check")
        })
        .unwrap();
    assert_eq!(latest_pass.id, second_id);
    assert_ne!(
        latest_pass.id, first_id,
        "latest-by-check-id would select another execution"
    );
    let failure = store.evidence.iter().find(|e| e.id == failed_id).unwrap();
    assert_eq!(failure.kind, EvidenceKind::CheckFail);
    assert_eq!(
        failure.metadata.get("exit_code").map(String::as_str),
        Some("7")
    );
    assert!(
        store.has_later_failed_check(proof),
        "exact identity does not bypass later failure invalidation"
    );
    let old_shape = serde_json::to_value(first).unwrap();
    assert!(
        old_shape.get("evidence_id").is_none(),
        "legacy command JSON stays unchanged"
    );
}

#[test]
fn evidence_versions_are_independent_and_serialization_is_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = EvidenceStore::load(dir.path()).unwrap();
    let stale = store.clone();
    add(&mut store, "a");
    store.save(dir.path()).unwrap();
    add(&mut store, "b");
    store.save(dir.path()).unwrap();
    assert!(stale
        .save(dir.path())
        .unwrap_err()
        .to_string()
        .contains("EVIDENCE_WRITE_CONFLICT"));
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(evidence_path(dir.path())).unwrap()).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 1);
    assert!(json.get("evidence").is_some());
    std::fs::remove_file(evidence_path(dir.path())).unwrap();
    assert!(store.save(dir.path()).is_err());
}

#[test]
fn evidence_transaction_error_and_corrupt_input_do_not_replace_history() {
    let dir = tempfile::tempdir().unwrap();
    EvidenceStore::update(dir.path(), |store| {
        add(store, "a");
        Ok(())
    })
    .unwrap();
    let path = evidence_path(dir.path());
    let before = std::fs::read(&path).unwrap();
    let result: Result<()> = EvidenceStore::update(dir.path(), |store| {
        add(store, "b");
        anyhow::bail!("synthetic callback failure")
    });
    assert!(result.is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::write(&path, b"invalid-json").unwrap();
    assert!(EvidenceStore::update(dir.path(), |_| Ok(())).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"invalid-json");
}
