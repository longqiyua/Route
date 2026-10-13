//! Real filesystem faults in isolated projects; no environment-global injection.
use super::*;

#[test]
fn existing_corrupt_basic_metadata_cannot_be_a_context_only_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    crate::repository::BasicRepository::init(dir.path()).unwrap();
    std::fs::write(dir.path().join(".route-basic/config.json"), b"invalid-json").unwrap();
    assert!(
        compute_state_hash(dir.path()).is_err(),
        "corrupt metadata must not silently omit the working tree"
    );
}

#[test]
fn unreadable_context_components_cannot_be_defaults_or_omitted() {
    for component in [
        "constitution",
        "protocol",
        "reference",
        "active-profile",
        "profile-store",
    ] {
        let dir = tempfile::tempdir().unwrap();
        crate::repository::BasicRepository::init(dir.path()).unwrap();
        let path = match component {
            "constitution" => constitutive::constitution_path(dir.path()),
            "protocol" => constitutive::protocol_path(dir.path()),
            "reference" => constitutive::registry_path(dir.path()),
            "active-profile" => crate::profile::active_profile_path(dir.path()),
            "profile-store" => {
                std::fs::write(
                    crate::profile::active_profile_path(dir.path()),
                    b"selected-profile",
                )
                .unwrap();
                crate::profile::profile_path(dir.path())
            }
            _ => unreachable!(),
        };
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, [0xff]).unwrap();
        assert!(
            compute_state_hash(dir.path()).is_err(),
            "{component} read failure must not produce a partial fingerprint"
        );
    }
}

#[cfg(windows)]
#[test]
fn locked_source_cannot_generate_passing_command_evidence() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    crate::repository::BasicRepository::init(dir.path()).unwrap();
    let session = begin_session(
        dir.path(),
        "Locked source regression",
        ApplyTarget::Claude,
        false,
        None,
    )
    .unwrap();
    let source = dir.path().join("source.rs");
    std::fs::write(&source, b"fn main() {}").unwrap();
    let mut protocol = constitutive::Protocol::read(dir.path()).unwrap();
    protocol
        .body
        .push_str("\n<!-- route-verification-policy:\n  required_checks: [\"before-lock\"]\n-->\n");
    protocol.write(dir.path()).unwrap();
    let argv = vec!["cmd.exe".into(), "/d".into(), "/c".into(), "exit 0".into()];
    assert!(
        exec_command(dir.path(), &session.id, &argv, Some("before-lock"))
            .unwrap()
            .state_stable
    );
    assert_eq!(
        verify_session(dir.path(), &session.id).unwrap(),
        VerifyResult::Verified
    );
    let _exclusive = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&source)
        .unwrap();
    assert!(
        compute_state_hash(dir.path()).is_err(),
        "OS sharing violation must propagate"
    );
    assert!(verify_session(dir.path(), &session.id).is_err());
    assert!(end_session(dir.path(), &session.id, "success").is_err());
    assert_eq!(
        SessionStore::load(dir.path())
            .unwrap()
            .get(&session.id)
            .unwrap()
            .status,
        SessionStatus::Active
    );
    assert!(exec_command(dir.path(), &session.id, &argv, Some("locked-source")).is_err());
    assert!(EvidenceStore::load(dir.path())
        .unwrap()
        .evidence
        .iter()
        .all(|e| e.metadata.get("check_id").map(String::as_str) != Some("locked-source")));
}

#[test]
fn unknown_empty_fingerprints_never_validate_old_passes() {
    let mut store = EvidenceStore::default();
    store.record_with_state(
        "synthetic",
        EvidenceKind::CheckPass,
        EvidenceSource::System,
        "synthetic".into(),
        HashMap::new(),
        Some(String::new()),
        None,
        None,
        None,
    );
    assert!(store.is_pass_stale(&store.evidence[0], ""));
}

#[test]
fn absent_optional_metadata_keeps_readonly_context_fingerprint() {
    let dir = tempfile::tempdir().unwrap();
    let before = std::fs::read_dir(dir.path()).unwrap().count();
    let first = compute_state_hash(dir.path()).unwrap();
    assert_eq!(first.len(), 64);
    assert_eq!(first, compute_state_hash(dir.path()).unwrap());
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), before);
}

#[test]
fn sidecar_fingerprint_tracks_file_bytes_without_creating_an_archive() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.rs");
    std::fs::write(&source, b"fn before() {}").unwrap();
    let before = compute_state_hash(dir.path()).unwrap();
    std::fs::write(&source, b"fn after_() {}").unwrap();
    let after = compute_state_hash(dir.path()).unwrap();
    assert_ne!(
        before, after,
        "same-size edits without Basic must invalidate old evidence"
    );
    assert!(!dir.path().join(".route-basic").exists());
    assert!(!dir.path().join(".route").exists());
}

#[test]
fn sidecar_source_manifest_tracks_names_additions_and_deletions_but_not_generated_data() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("source.rs"), b"source").unwrap();
    let initial = compute_state_hash(root).unwrap();
    for ignored in [
        ".route",
        ".git",
        "target",
        "target-local",
        "node_modules",
        "Wiki",
        "Release",
        "dist",
        "build",
    ] {
        std::fs::create_dir(root.join(ignored)).unwrap();
        std::fs::write(root.join(ignored).join("generated.bin"), b"generated").unwrap();
    }
    assert_eq!(initial, compute_state_hash(root).unwrap());
    std::fs::rename(root.join("source.rs"), root.join("renamed.rs")).unwrap();
    let renamed = compute_state_hash(root).unwrap();
    assert_ne!(initial, renamed);
    std::fs::write(root.join("extra.txt"), b"extra").unwrap();
    assert_ne!(renamed, compute_state_hash(root).unwrap());
    std::fs::remove_file(root.join("extra.txt")).unwrap();
    assert_eq!(renamed, compute_state_hash(root).unwrap());
    assert!(!root.join(".route-basic").exists());
}
