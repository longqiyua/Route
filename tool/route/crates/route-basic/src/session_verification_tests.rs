//! Session acceptance regressions use temporary projects, never production history.
use super::*;

fn setup(policy: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    crate::repository::BasicRepository::init(dir.path()).unwrap();
    let session = begin_session(
        dir.path(),
        "Session verification regression",
        ApplyTarget::Claude,
        false,
        None,
    )
    .unwrap();
    let mut protocol = constitutive::Protocol::read(dir.path()).unwrap();
    protocol.body.push_str(&format!(
        "\n<!-- route-verification-policy:\n{policy}\n-->\n"
    ));
    protocol.write(dir.path()).unwrap();
    (dir, session.id)
}

fn command(root: &Path, session: &str, code: i32) {
    let argv = if cfg!(windows) {
        vec![
            "cmd.exe".into(),
            "/d".into(),
            "/c".into(),
            format!("exit {code}"),
        ]
    } else {
        vec!["sh".into(), "-c".into(), format!("exit {code}")]
    };
    assert_eq!(
        exec_command(root, session, &argv, Some("regression-check"))
            .unwrap()
            .exit_code,
        code
    );
}

#[test]
fn session_verification_rejects_newer_real_failure() {
    let (dir, session) = setup("  required_checks: [\"regression-check\"]");
    command(dir.path(), &session, 0);
    assert_eq!(
        verify_session(dir.path(), &session).unwrap(),
        VerifyResult::Verified
    );
    command(dir.path(), &session, 7);
    assert_eq!(
        verify_session(dir.path(), &session).unwrap(),
        VerifyResult::Failed
    );
    assert_eq!(
        end_session(dir.path(), &session, "success").unwrap().status,
        SessionStatus::Failed
    );
}

#[test]
fn session_verification_requires_a_positive_check_outcome() {
    for kind in [EvidenceKind::CheckFail, EvidenceKind::Manual] {
        let (dir, session) = setup("  required_checks: [\"regression-check\"]");
        EvidenceStore::update(dir.path(), |store| {
            store.record(
                &session,
                kind,
                EvidenceSource::System,
                "synthetic".into(),
                HashMap::from([("check_id".into(), "regression-check".into())]),
            );
            Ok(())
        })
        .unwrap();
        assert_ne!(
            verify_session(dir.path(), &session).unwrap(),
            VerifyResult::Verified,
            "{kind:?} cannot satisfy required_checks"
        );
    }
}

fn record(
    root: &Path,
    session: &str,
    kind: EvidenceKind,
    source: EvidenceSource,
    key: Option<(&str, &str)>,
) {
    let hash = compute_state_hash(root).unwrap();
    EvidenceStore::update(root, |store| {
        let metadata = key
            .map(|(key, value)| HashMap::from([(key.into(), value.into())]))
            .unwrap_or_default();
        store.record_with_state(
            session,
            kind,
            source,
            "synthetic-verification-fixture".into(),
            metadata,
            Some(hash),
            None,
            None,
            None,
        );
        // Identical timestamps must not change durable outcome order.
        store.evidence.last_mut().unwrap().created_at = 1;
        Ok(())
    })
    .unwrap();
}

#[test]
fn required_check_recovery_ignores_unrelated_or_agent_failures() {
    let (dir, session) = setup("  required_checks: [\"regression-check\"]");
    record(
        dir.path(),
        &session,
        EvidenceKind::CheckPass,
        EvidenceSource::System,
        Some(("check_id", "regression-check")),
    );
    record(
        dir.path(),
        &session,
        EvidenceKind::CheckFail,
        EvidenceSource::System,
        Some(("check_id", "unrelated-check")),
    );
    record(
        dir.path(),
        &session,
        EvidenceKind::CheckFail,
        EvidenceSource::Agent,
        Some(("check_id", "regression-check")),
    );
    assert_eq!(
        verify_session(dir.path(), &session).unwrap(),
        VerifyResult::Verified
    );
    record(
        dir.path(),
        &session,
        EvidenceKind::TestFail,
        EvidenceSource::System,
        Some(("check_id", "regression-check")),
    );
    assert_eq!(
        verify_session(dir.path(), &session).unwrap(),
        VerifyResult::Failed
    );
    record(
        dir.path(),
        &session,
        EvidenceKind::TestPass,
        EvidenceSource::System,
        Some(("check_id", "regression-check")),
    );
    assert_eq!(
        verify_session(dir.path(), &session).unwrap(),
        VerifyResult::Verified
    );
}

#[test]
fn legacy_argv_and_anonymous_kind_proofs_respect_failure_order() {
    for (policy, key) in [
        (
            "  required_checks: [\"cargo test\"]",
            Some(("argv", "cargo test --workspace")),
        ),
        (
            "  success_requires: [\"test_pass\"]",
            Some(("check_id", "unit-check")),
        ),
        (
            "  success_requires: [\"test_pass\"]",
            Some(("argv", "cargo test --workspace")),
        ),
        ("  success_requires: [\"test_pass\"]", None),
    ] {
        let (dir, session) = setup(policy);
        record(
            dir.path(),
            &session,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            key,
        );
        assert_eq!(
            verify_session(dir.path(), &session).unwrap(),
            VerifyResult::Verified
        );
        record(
            dir.path(),
            &session,
            EvidenceKind::TestFail,
            EvidenceSource::System,
            key,
        );
        assert_eq!(
            verify_session(dir.path(), &session).unwrap(),
            VerifyResult::Failed
        );
        record(
            dir.path(),
            &session,
            EvidenceKind::TestPass,
            EvidenceSource::System,
            key,
        );
        assert_eq!(
            verify_session(dir.path(), &session).unwrap(),
            VerifyResult::Verified
        );
    }
}

#[test]
fn session_end_uses_current_verification_and_keeps_no_policy_compatibility() {
    let (dir, session) = setup("  required_checks: [\"regression-check\"]");
    assert_eq!(
        verify_session(dir.path(), &session).unwrap(),
        VerifyResult::Incomplete
    );
    assert_eq!(
        end_session(dir.path(), &session, "success").unwrap().status,
        SessionStatus::Failed
    );
    let (dir, session) = setup("  success_requires: [\"test_pass\"]");
    record(
        dir.path(),
        &session,
        EvidenceKind::TestPass,
        EvidenceSource::System,
        Some(("check_id", "unit-check")),
    );
    std::fs::write(dir.path().join("changed-source.txt"), b"changed").unwrap();
    assert_eq!(
        verify_session(dir.path(), &session).unwrap(),
        VerifyResult::Failed
    );
    assert_eq!(
        end_session(dir.path(), &session, "success").unwrap().status,
        SessionStatus::Failed
    );
    let (dir, session) = setup("");
    assert_eq!(
        end_session(dir.path(), &session, "success").unwrap().status,
        SessionStatus::Succeeded
    );
}
