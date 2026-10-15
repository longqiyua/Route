//! Versioned DevelopmentEvent persistence.
//!
//! The journal deliberately keeps the existing event JSON as its payload.  A
//! binary prepare frame gives every event a deterministic length and digest;
//! a separate commit trailer is published only after the prepare bytes have
//! been flushed.  Readers accept only complete prepare+commit pairs and still
//! verify the pre-existing event hash chain.

use super::{DevelopmentAppendLock, DevelopmentEvent, DevelopmentLedger, LEDGER_SCHEMA};
use crate::project_identity::{identity_path, ProjectIdentity};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const MANIFEST_NAME: &str = "ledger.manifest.json";
const SEGMENT_NAME: &str = "ledger-000001.rlog";
const LEGACY_NAME: &str = "ledger.json";
const LEGACY_BACKUP_NAME: &str = "ledger.legacy.json";
const PENDING_NAME: &str = "ledger.pending.json";
const FILE_MAGIC: &[u8] = b"ROUTELEDGER/1\n";
const PREPARE_MAGIC: &[u8; 4] = b"EVT1";
const COMMIT_MAGIC: &[u8; 4] = b"COM1";
const MAX_HEADER_BYTES: usize = 64 * 1024;
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoadMode {
    ReadOnly,
    RecoverTail,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct JournalManifest {
    format: String,
    version: u8,
    project_id: String,
    ledger_id: String,
    segment: String,
    created_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    migrated_from_sha256: Option<String>,
    #[serde(default)]
    migrated_revision: u64,
    acknowledged_revision: u64,
    acknowledged_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct JournalHeader {
    format: String,
    version: u8,
    project_id: String,
    ledger_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct BackupManifest {
    format: String,
    version: u8,
    project_id: String,
    storage_format: String,
    revision: u64,
    head_hash: String,
    files: Vec<BackupFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct BackupFile {
    name: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevelopmentStorageInspection {
    pub format: String,
    pub format_version: u8,
    pub project_id: String,
    pub ledger_id: Option<String>,
    pub authoritative_files: Vec<String>,
    pub revision: u64,
    pub event_count: usize,
    pub head_hash: String,
    pub incomplete_trailing_record: bool,
    pub recovery_backup_present: bool,
    pub read_scaling: String,
}

struct ParsedJournal {
    ledger: DevelopmentLedger,
    committed_len: u64,
    incomplete_tail: bool,
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join(MANIFEST_NAME)
}

fn legacy_path(dir: &Path) -> PathBuf {
    dir.join(LEGACY_NAME)
}

fn legacy_backup_path(dir: &Path) -> PathBuf {
    dir.join(LEGACY_BACKUP_NAME)
}

fn segment_path(dir: &Path, manifest: &JournalManifest) -> Result<PathBuf> {
    if manifest.segment != SEGMENT_NAME {
        bail!(
            "MISSING_SEGMENT: unsupported or missing journal segment '{}' in {}",
            manifest.segment,
            dir.display()
        );
    }
    Ok(dir.join(&manifest.segment))
}

fn validate_manifest(manifest: &JournalManifest, expected_project_id: &str) -> Result<()> {
    if manifest.format != "route.development-journal" || manifest.version != 1 {
        bail!(
            "UNKNOWN_FORMAT_VERSION: {} version {}",
            manifest.format,
            manifest.version
        );
    }
    if manifest.project_id != expected_project_id {
        bail!("development journal project identity mismatch");
    }
    if manifest.ledger_id.trim().is_empty() {
        bail!("development journal ledger identity is missing");
    }
    Ok(())
}

fn read_manifest(dir: &Path, expected_project_id: &str) -> Result<Option<JournalManifest>> {
    let path = manifest_path(dir);
    if !path.try_exists()? {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let manifest: JournalManifest = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing journal manifest at {}", path.display()))?;
    validate_manifest(&manifest, expected_project_id)?;
    validate_legacy_fence(dir, &manifest)?;
    Ok(Some(manifest))
}

fn validate_legacy_fence(dir: &Path, manifest: &JournalManifest) -> Result<()> {
    let path = legacy_path(dir);
    let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).with_context(|| {
        format!(
            "LEGACY_FENCE_MISSING: {} is required to stop old binaries creating a second ledger",
            path.display()
        )
    })?)
    .context("LEGACY_FENCE_CORRUPT: invalid compatibility fence")?;
    if value.get("schema").and_then(serde_json::Value::as_u64) != Some(255)
        || value.get("migrated_to").and_then(serde_json::Value::as_str)
            != Some(manifest.ledger_id.as_str())
    {
        bail!("LEGACY_FENCE_CORRUPT: compatibility fence does not bind the active ledger");
    }
    Ok(())
}

fn load_legacy(path: &Path, expected_project_id: &str) -> Result<DevelopmentLedger> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let ledger: DevelopmentLedger = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing development ledger at {}", path.display()))?;
    ledger.verify(expected_project_id)?;
    Ok(ledger)
}

pub(super) fn active_ledger_path(dir: &Path) -> Result<PathBuf> {
    if manifest_path(dir).try_exists()? {
        let bytes = fs::read(manifest_path(dir))?;
        let manifest: JournalManifest = serde_json::from_slice(&bytes)?;
        return segment_path(dir, &manifest);
    }
    if legacy_path(dir).try_exists()? {
        return Ok(legacy_path(dir));
    }
    Ok(dir.join(SEGMENT_NAME))
}

pub(super) fn load(
    dir: &Path,
    expected_project_id: &str,
    mode: LoadMode,
) -> Result<DevelopmentLedger> {
    if mode == LoadMode::RecoverTail && !manifest_path(dir).try_exists()? {
        resume_initialization(dir, expected_project_id)?;
    }
    if let Some(manifest) = read_manifest(dir, expected_project_id)? {
        let path = segment_path(dir, &manifest)?;
        if !path.try_exists()? {
            bail!(
                "MISSING_SEGMENT: ledger {} expects {}",
                manifest.ledger_id,
                path.display()
            );
        }
        let mut parsed = parse_journal(&path, &manifest)?;
        // A reader never repairs. If it raced a writer between PREPARE and
        // COMMIT, briefly retry the immutable byte snapshot; a crashed writer
        // remains a diagnosed incomplete tail after this bounded window.
        if mode == LoadMode::ReadOnly && parsed.incomplete_tail {
            for _ in 0..8 {
                std::thread::sleep(std::time::Duration::from_millis(2));
                parsed = parse_journal(&path, &manifest)?;
                if !parsed.incomplete_tail {
                    break;
                }
            }
        }
        if parsed.incomplete_tail {
            match mode {
                LoadMode::ReadOnly => bail!(
                    "INCOMPLETE_TRAILING_RECORD: ledger {} segment {} after revision {}; retry the interrupted write or run storage inspect",
                    manifest.ledger_id,
                    path.display(),
                    parsed.ledger.revision()
                ),
                LoadMode::RecoverTail => {
                    let file = OpenOptions::new().write(true).open(&path)?;
                    file.set_len(parsed.committed_len)?;
                    file.sync_all()?;
                    failpoint("recovery_after_truncate")?;
                    parsed.incomplete_tail = false;
                }
            }
        }
        if mode == LoadMode::RecoverTail
            && parsed.ledger.revision() > manifest.acknowledged_revision
        {
            OpenOptions::new().write(true).open(&path)?.sync_all()?;
            let mut recovered = manifest.clone();
            recovered.acknowledged_revision = parsed.ledger.revision();
            recovered.acknowledged_hash = parsed.ledger.head_hash.clone();
            crate::constitutive::write_atomic(
                &manifest_path(dir),
                &serde_json::to_vec_pretty(&recovered)?,
            )?;
        }
        return Ok(parsed.ledger);
    }

    let legacy = legacy_path(dir);
    if legacy.try_exists()? {
        return load_legacy(&legacy, expected_project_id);
    }
    if dir.join(SEGMENT_NAME).try_exists()? {
        bail!(
            "UNKNOWN_COMMIT_STATUS: journal segment exists without authoritative manifest at {}; run storage inspect or resume migration",
            dir.display()
        );
    }
    Ok(DevelopmentLedger::empty(expected_project_id.to_string()))
}

fn resume_initialization(dir: &Path, project_id: &str) -> Result<()> {
    let pending = dir.join(PENDING_NAME);
    if !pending.try_exists()? {
        return Ok(());
    }
    let manifest: JournalManifest = serde_json::from_slice(&fs::read(&pending)?)?;
    validate_manifest(&manifest, project_id)?;
    if manifest.migrated_from_sha256.is_some() {
        return Ok(());
    }
    let path = segment_path(dir, &manifest)?;
    if !path.try_exists()? {
        write_complete_journal(
            &path,
            &manifest,
            &DevelopmentLedger::empty(project_id.into()),
        )?;
    }
    let parsed = parse_journal(&path, &manifest)?;
    anyhow::ensure!(
        parsed.ledger.events.is_empty() && !parsed.incomplete_tail,
        "UNKNOWN_COMMIT_STATUS: interrupted initialization contains unexpected events"
    );
    crate::constitutive::write_atomic(
        &legacy_path(dir),
        &migration_sentinel(&parsed.ledger, &manifest.ledger_id)?,
    )?;
    crate::constitutive::write_atomic(&manifest_path(dir), &serde_json::to_vec_pretty(&manifest)?)?;
    Ok(())
}

fn encode_header(manifest: &JournalManifest) -> Result<Vec<u8>> {
    let header = JournalHeader {
        format: manifest.format.clone(),
        version: manifest.version,
        project_id: manifest.project_id.clone(),
        ledger_id: manifest.ledger_id.clone(),
    };
    let body = serde_json::to_vec(&header)?;
    let mut bytes = Vec::with_capacity(FILE_MAGIC.len() + 4 + body.len() + 64);
    bytes.extend_from_slice(FILE_MAGIC);
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&body);
    bytes.extend_from_slice(route_core::sha256_hex(&body).as_bytes());
    Ok(bytes)
}

fn encode_prepare(event: &DevelopmentEvent) -> Result<(Vec<u8>, String)> {
    let payload = serde_json::to_vec(event)?;
    anyhow::ensure!(
        payload.len() <= MAX_EVENT_BYTES,
        "event frame exceeds maximum size"
    );
    let digest = route_core::sha256_hex(&payload);
    let mut bytes = Vec::with_capacity(4 + 8 + 8 + 64 + payload.len());
    bytes.extend_from_slice(PREPARE_MAGIC);
    bytes.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&event.sequence.to_le_bytes());
    bytes.extend_from_slice(digest.as_bytes());
    let header_digest = route_core::sha256_hex(&bytes);
    bytes.extend_from_slice(header_digest.as_bytes());
    bytes.extend_from_slice(&payload);
    Ok((bytes, digest))
}

fn encode_commit(sequence: u64, digest: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + 8 + 64);
    bytes.extend_from_slice(COMMIT_MAGIC);
    bytes.extend_from_slice(&sequence.to_le_bytes());
    bytes.extend_from_slice(digest.as_bytes());
    bytes
}

fn read_u32(bytes: &[u8], offset: &mut usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let value = u32::from_le_bytes(bytes.get(*offset..end)?.try_into().ok()?);
    *offset = end;
    Some(value)
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    let value = u64::from_le_bytes(bytes.get(*offset..end)?.try_into().ok()?);
    *offset = end;
    Some(value)
}

fn parse_journal(path: &Path, manifest: &JournalManifest) -> Result<ParsedJournal> {
    let bytes = fs::read(path).with_context(|| format!("reading journal {}", path.display()))?;
    if !bytes.starts_with(FILE_MAGIC) {
        bail!(
            "CORRUPT_HEADER: ledger {} segment {} has invalid magic",
            manifest.ledger_id,
            path.display()
        );
    }
    let mut offset = FILE_MAGIC.len();
    let header_len = read_u32(&bytes, &mut offset)
        .ok_or_else(|| anyhow::anyhow!("CORRUPT_HEADER: truncated length in {}", path.display()))?
        as usize;
    if header_len > MAX_HEADER_BYTES {
        bail!(
            "CORRUPT_HEADER: header length exceeds bound in {}",
            path.display()
        );
    }
    let header_end = offset
        .checked_add(header_len)
        .ok_or_else(|| anyhow::anyhow!("CORRUPT_HEADER: length overflow"))?;
    let digest_end = header_end + 64;
    let header_bytes = bytes
        .get(offset..header_end)
        .ok_or_else(|| anyhow::anyhow!("CORRUPT_HEADER: truncated body in {}", path.display()))?;
    let stored_digest = bytes
        .get(header_end..digest_end)
        .ok_or_else(|| anyhow::anyhow!("CORRUPT_HEADER: truncated digest in {}", path.display()))?;
    if stored_digest != route_core::sha256_hex(header_bytes).as_bytes() {
        bail!(
            "CORRUPT_HEADER: header digest mismatch in {}",
            path.display()
        );
    }
    let header: JournalHeader = serde_json::from_slice(header_bytes)
        .with_context(|| format!("CORRUPT_HEADER: invalid JSON in {}", path.display()))?;
    if header.format != manifest.format
        || header.version != manifest.version
        || header.project_id != manifest.project_id
        || header.ledger_id != manifest.ledger_id
    {
        bail!(
            "CORRUPT_HEADER: manifest/header binding mismatch in {}",
            path.display()
        );
    }
    offset = digest_end;
    let mut committed_len = offset as u64;
    let mut ledger = DevelopmentLedger::empty(manifest.project_id.clone());
    ledger.schema = LEDGER_SCHEMA;
    let mut incomplete_tail = false;
    while offset < bytes.len() {
        let record_start = offset;
        if bytes.len() - offset < 4 + 8 + 8 + 64 + 64 {
            let remaining = &bytes[offset..];
            if remaining[..remaining.len().min(4)] != PREPARE_MAGIC[..remaining.len().min(4)] {
                bail!("CORRUPT_MIDDLE_RECORD: unrecognized partial frame header at byte {offset} in {}", path.display());
            }
            if remaining.len() >= 12 {
                let length = u64::from_le_bytes(remaining[4..12].try_into().unwrap());
                anyhow::ensure!(
                    length <= MAX_EVENT_BYTES as u64,
                    "CORRUPT_COMMITTED_RECORD: invalid partial header length"
                );
            }
            if remaining.len() >= 20 {
                let sequence = u64::from_le_bytes(remaining[12..20].try_into().unwrap());
                anyhow::ensure!(
                    sequence == ledger.revision() + 1,
                    "CORRUPT_MIDDLE_RECORD: invalid partial header revision"
                );
            }
            incomplete_tail = true;
            break;
        }
        if bytes.get(offset..offset + 4) != Some(PREPARE_MAGIC.as_slice()) {
            bail!("CORRUPT_MIDDLE_RECORD: ledger {} segment {} byte {} expected revision {} has invalid frame magic", manifest.ledger_id, path.display(), offset, ledger.revision() + 1);
        }
        offset += 4;
        let payload_len = read_u64(&bytes, &mut offset).unwrap() as usize;
        let sequence = read_u64(&bytes, &mut offset).unwrap();
        if payload_len > MAX_EVENT_BYTES {
            bail!(
                "CORRUPT_COMMITTED_RECORD: ledger {} segment {} revision {} length exceeds bound",
                manifest.ledger_id,
                path.display(),
                sequence
            );
        }
        let digest = bytes.get(offset..offset + 64).unwrap();
        offset += 64;
        let header_digest = &bytes[offset..offset + 64];
        if header_digest != route_core::sha256_hex(&bytes[record_start..offset]).as_bytes() {
            bail!("CORRUPT_COMMITTED_RECORD: frame header checksum mismatch at revision {sequence} in {}", path.display());
        }
        offset += 64;
        let payload_end = match offset.checked_add(payload_len) {
            Some(end) if end <= bytes.len() => end,
            _ => {
                incomplete_tail = true;
                break;
            }
        };
        let payload = &bytes[offset..payload_end];
        offset = payload_end;
        if bytes.len() - offset < 4 + 8 + 64 {
            incomplete_tail = true;
            break;
        }
        if bytes.get(offset..offset + 4) != Some(COMMIT_MAGIC.as_slice()) {
            bail!("CORRUPT_COMMITTED_RECORD: ledger {} segment {} revision {} missing commit marker at byte {}", manifest.ledger_id, path.display(), sequence, offset);
        }
        offset += 4;
        let committed_sequence = read_u64(&bytes, &mut offset).unwrap();
        let committed_digest = &bytes[offset..offset + 64];
        offset += 64;
        if sequence != ledger.revision() + 1 || committed_sequence != sequence {
            bail!(
                "CORRUPT_MIDDLE_RECORD: ledger {} segment {} byte {} revision ordering mismatch",
                manifest.ledger_id,
                path.display(),
                record_start
            );
        }
        let actual = route_core::sha256_hex(payload);
        if digest != actual.as_bytes() || committed_digest != actual.as_bytes() {
            bail!(
                "CORRUPT_COMMITTED_RECORD: ledger {} segment {} revision {} frame digest mismatch",
                manifest.ledger_id,
                path.display(),
                sequence
            );
        }
        let event: DevelopmentEvent = serde_json::from_slice(payload).with_context(|| {
            format!("CORRUPT_COMMITTED_RECORD: invalid event JSON at revision {sequence}")
        })?;
        if event.sequence != sequence {
            bail!(
                "CORRUPT_COMMITTED_RECORD: envelope/event revision mismatch at revision {sequence}"
            );
        }
        ledger.head_hash = event.hash.clone();
        ledger.events.push(event);
        committed_len = offset as u64;
    }
    ledger.verify(&manifest.project_id)?;
    if ledger.revision() < manifest.acknowledged_revision
        || (manifest.acknowledged_revision > 0
            && ledger.events[(manifest.acknowledged_revision - 1) as usize].hash
                != manifest.acknowledged_hash)
    {
        bail!("CORRUPT_COMMITTED_RECORD: acknowledged revision {} missing or changed in {}; recovery must not truncate confirmed history", manifest.acknowledged_revision, path.display());
    }
    Ok(ParsedJournal {
        ledger,
        committed_len,
        incomplete_tail,
    })
}

fn new_manifest(project_id: &str, migrated_from: Option<String>, revision: u64) -> JournalManifest {
    JournalManifest {
        format: "route.development-journal".into(),
        version: 1,
        project_id: project_id.into(),
        ledger_id: format!("ldg_{}", route_core::new_id()),
        segment: SEGMENT_NAME.into(),
        created_at: route_core::now_millis(),
        migrated_from_sha256: migrated_from,
        migrated_revision: revision,
        acknowledged_revision: 0,
        acknowledged_hash: super::GENESIS_HASH.into(),
    }
}

fn write_complete_journal(
    path: &Path,
    manifest: &JournalManifest,
    ledger: &DevelopmentLedger,
) -> Result<()> {
    let tmp = path.with_extension(format!("rlog.staging-{}", route_core::new_id()));
    let mut file = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
    file.write_all(&encode_header(manifest)?)?;
    for event in &ledger.events {
        let (prepare, digest) = encode_prepare(event)?;
        file.write_all(&prepare)?;
        file.write_all(&encode_commit(event.sequence, &digest))?;
    }
    file.sync_all()?;
    if failpoint_enabled("rename_fails") {
        bail!("injected rename failure while publishing journal segment");
    }
    fs::rename(&tmp, path)
        .with_context(|| format!("publishing journal segment {}", path.display()))?;
    Ok(())
}

fn initialize_empty_journal(dir: &Path, project_id: &str) -> Result<JournalManifest> {
    fs::create_dir_all(dir)?;
    if manifest_path(dir).try_exists()?
        || legacy_path(dir).try_exists()?
        || dir.join(SEGMENT_NAME).try_exists()?
    {
        bail!("storage changed while initializing development journal");
    }
    let manifest = new_manifest(project_id, None, 0);
    crate::constitutive::write_atomic(
        &dir.join(PENDING_NAME),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    write_complete_journal(
        &dir.join(SEGMENT_NAME),
        &manifest,
        &DevelopmentLedger::empty(project_id.to_string()),
    )?;
    crate::constitutive::write_atomic(
        &legacy_path(dir),
        &migration_sentinel(
            &DevelopmentLedger::empty(project_id.to_string()),
            &manifest.ledger_id,
        )?,
    )?;
    failpoint("before_manifest_publish")?;
    crate::constitutive::write_atomic(&manifest_path(dir), &serde_json::to_vec_pretty(&manifest)?)?;
    failpoint("after_manifest_publish")?;
    Ok(manifest)
}

pub(super) fn commit_append(
    dir: &Path,
    project_id: &str,
    ledger: &DevelopmentLedger,
    event: &DevelopmentEvent,
) -> Result<()> {
    if manifest_path(dir).try_exists()? {
        let mut manifest = read_manifest(dir, project_id)?.expect("manifest existence checked");
        let path = segment_path(dir, &manifest)?;
        append_frame(&path, event).with_context(|| format!(
            "UNKNOWN_COMMIT_STATUS: ledger {} segment {} revision {} event {}; inspect storage and retry the same operation key",
            manifest.ledger_id, path.display(), event.sequence, event.event_id
        ))?;
        manifest.acknowledged_revision = event.sequence;
        manifest.acknowledged_hash = event.hash.clone();
        crate::constitutive::write_atomic(
            &manifest_path(dir),
            &serde_json::to_vec_pretty(&manifest)?,
        )
        .with_context(|| format!("UNKNOWN_COMMIT_STATUS: publishing acknowledged ledger {} revision {} at {}; retry the same operation key", manifest.ledger_id, event.sequence, manifest_path(dir).display()))
    } else if legacy_path(dir).try_exists()? {
        // Compatibility contract: legacy stores retain their original writer
        // until the operator explicitly migrates them.
        crate::constitutive::write_atomic(&legacy_path(dir), &serde_json::to_vec_pretty(ledger)?)
    } else {
        initialize_empty_journal(dir, project_id)?;
        commit_append(dir, project_id, ledger, event)
    }
}

fn append_frame(path: &Path, event: &DevelopmentEvent) -> Result<()> {
    failpoint("before_append")?;
    if failpoint_enabled("disk_full") {
        bail!("injected quota-style disk-full write failure");
    }
    if failpoint_enabled("permission_denied") {
        bail!("injected permission-denied write failure");
    }
    if failpoint_enabled("write_returns_error") {
        bail!("injected event write error");
    }
    let (prepare, digest) = encode_prepare(event)?;
    let mut file = OpenOptions::new().append(true).open(path)?;
    if failpoint_enabled("half_record") {
        file.write_all(&prepare[..prepare.len() / 2])?;
        file.flush()?;
        crash_or_error("half_record")?;
    }
    file.write_all(&prepare)?;
    file.sync_all()
        .context("UNKNOWN_COMMIT_STATUS: flushing prepared event bytes")?;
    failpoint("before_durable_commit")?;
    file.write_all(&encode_commit(event.sequence, &digest))?;
    if failpoint_enabled("flush_fails") {
        return Err(anyhow::anyhow!(
            "UNKNOWN_COMMIT_STATUS: injected commit flush failure"
        ));
    }
    file.sync_all()
        .context("UNKNOWN_COMMIT_STATUS: flushing development event commit")?;
    failpoint("after_durable_commit")?;
    Ok(())
}

#[cfg(feature = "test-utils")]
pub(super) fn seed_new_journal(
    dir: &Path,
    project_id: &str,
    ledger: &DevelopmentLedger,
) -> Result<()> {
    if manifest_path(dir).try_exists()?
        || legacy_path(dir).try_exists()?
        || dir.join(SEGMENT_NAME).try_exists()?
    {
        bail!("benchmark seed requires absent storage files");
    }
    let manifest = new_manifest(project_id, None, 0);
    write_complete_journal(&dir.join(SEGMENT_NAME), &manifest, ledger)?;
    crate::constitutive::write_atomic(
        &legacy_path(dir),
        &migration_sentinel(ledger, &manifest.ledger_id)?,
    )?;
    crate::constitutive::write_atomic(&manifest_path(dir), &serde_json::to_vec_pretty(&manifest)?)
}

pub fn inspect_development_storage(root: &Path) -> Result<DevelopmentStorageInspection> {
    let identity = crate::project_identity::load_identity(root)?
        .ok_or_else(|| anyhow::anyhow!("project identity is missing"))?;
    let owner = identity
        .attached_from
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.to_path_buf());
    let owner_identity = crate::project_identity::load_identity(&owner)?
        .ok_or_else(|| anyhow::anyhow!("ledger owner identity is missing"))?;
    anyhow::ensure!(
        owner_identity.project_id == identity.project_id,
        "ledger owner project mismatch"
    );
    let dir = super::development_dir(&owner);
    if !dir.try_exists()? {
        let empty = DevelopmentLedger::empty(identity.project_id.clone());
        return Ok(inspection(
            "UNINITIALIZED",
            0,
            None,
            vec![],
            &empty,
            false,
            false,
        ));
    }
    if let Some(manifest) = read_manifest(&dir, &identity.project_id)? {
        let parsed = parse_journal(&segment_path(&dir, &manifest)?, &manifest)?;
        return Ok(inspection(
            "FRAMED_JOURNAL",
            manifest.version,
            Some(manifest.ledger_id),
            vec![MANIFEST_NAME.into(), manifest.segment],
            &parsed.ledger,
            parsed.incomplete_tail,
            legacy_backup_path(&dir).try_exists()?,
        ));
    }
    let ledger = load_legacy(&legacy_path(&dir), &identity.project_id)?;
    Ok(inspection(
        "LEGACY_JSON",
        ledger.schema,
        None,
        vec![LEGACY_NAME.into()],
        &ledger,
        false,
        legacy_backup_path(&dir).try_exists()?,
    ))
}

fn inspection(
    format: &str,
    version: u8,
    ledger_id: Option<String>,
    files: Vec<String>,
    ledger: &DevelopmentLedger,
    tail: bool,
    backup: bool,
) -> DevelopmentStorageInspection {
    DevelopmentStorageInspection {
        format: format.into(),
        format_version: version,
        project_id: ledger.project_id.clone(),
        ledger_id,
        authoritative_files: files,
        revision: ledger.revision(),
        event_count: ledger.events.len(),
        head_hash: ledger.head_hash.clone(),
        incomplete_trailing_record: tail,
        recovery_backup_present: backup,
        read_scaling: "OPEN_LINEAR_FULL_REPLAY".into(),
    }
}

fn migration_sentinel(ledger: &DevelopmentLedger, ledger_id: &str) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(&serde_json::json!({
        "schema": 255,
        "project_id": ledger.project_id,
        "head_hash": ledger.head_hash,
        "events": [],
        "migrated_to": ledger_id,
        "message": "This legacy path is fenced. Use a Route version with framed journal support."
    }))?)
}

pub fn migrate_development_storage(root: &Path) -> Result<DevelopmentStorageInspection> {
    let (owner, identity) = super::development_state_owner(root)?;
    let dir = super::development_dir(&owner);
    fs::create_dir_all(&dir)?;
    let _lock = DevelopmentAppendLock::acquire(&dir)?;
    if read_manifest(&dir, &identity.project_id)?.is_some() {
        drop(_lock);
        return inspect_development_storage(root);
    }
    let backup_path = legacy_backup_path(&dir);
    let live_path = legacy_path(&dir);
    let live_bytes = fs::read(&live_path)?;
    let live_value: serde_json::Value = serde_json::from_slice(&live_bytes)?;
    let fenced = live_value.get("schema").and_then(serde_json::Value::as_u64) == Some(255);
    let source_path = if fenced { &backup_path } else { &live_path };
    if !source_path.try_exists()? {
        bail!("no legacy development ledger exists to migrate");
    }
    let ledger = load_legacy(source_path, &identity.project_id)?;
    let source_bytes = fs::read(source_path)?;
    let source_hash = route_core::sha256_hex(&source_bytes);
    if fenced {
        let pending: JournalManifest = serde_json::from_slice(&fs::read(dir.join(PENDING_NAME))?)?;
        validate_manifest(&pending, &identity.project_id)?;
        anyhow::ensure!(
            pending.migrated_from_sha256.as_deref() == Some(source_hash.as_str()),
            "MIGRATION_SOURCE_MISMATCH: recovery backup is not the recorded migration source"
        );
    }
    let mut manifest = new_manifest(&identity.project_id, Some(source_hash), ledger.revision());
    manifest.acknowledged_revision = ledger.revision();
    manifest.acknowledged_hash = ledger.head_hash.clone();
    let segment = dir.join(SEGMENT_NAME);
    if segment.try_exists()? {
        let pending: JournalManifest = serde_json::from_slice(&fs::read(dir.join(PENDING_NAME))
            .context("UNKNOWN_COMMIT_STATUS: segment has no owned migration metadata; preserve and inspect")?)?;
        validate_manifest(&pending, &identity.project_id)?;
        parse_journal(&segment, &pending)?;
        // Preserve the previous staging/rolled-back segment rather than deleting
        // an existing file. Its identity remains available for operator audit.
        fs::rename(
            &segment,
            dir.join(format!("ledger-preserved-{}.rlog", route_core::new_id())),
        )?;
    }
    crate::constitutive::write_atomic(
        &dir.join(PENDING_NAME),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    write_complete_journal(&segment, &manifest, &ledger)?;
    let staged = parse_journal(&segment, &manifest)?;
    anyhow::ensure!(
        !staged.incomplete_tail,
        "staged journal has an incomplete tail"
    );
    anyhow::ensure!(
        staged.ledger.events == ledger.events,
        "migration event equivalence failed"
    );
    anyhow::ensure!(
        staged.ledger.head_hash == ledger.head_hash,
        "migration head equivalence failed"
    );
    failpoint("migration_staged")?;
    if !fenced {
        crate::constitutive::write_atomic(&backup_path, &source_bytes)?;
    }
    crate::constitutive::write_atomic(
        &legacy_path(&dir),
        &migration_sentinel(&ledger, &manifest.ledger_id)?,
    )?;
    failpoint("migration_before_publish")?;
    crate::constitutive::write_atomic(
        &manifest_path(&dir),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    failpoint("migration_after_publish")?;
    drop(_lock);
    inspect_development_storage(root)
}

pub fn rollback_development_migration(root: &Path) -> Result<DevelopmentStorageInspection> {
    let (owner, identity) = super::development_state_owner(root)?;
    let dir = super::development_dir(&owner);
    let _lock = DevelopmentAppendLock::acquire(&dir)?;
    let manifest = match read_manifest(&dir, &identity.project_id)? {
        Some(manifest) => manifest,
        None => {
            let pending: JournalManifest =
                serde_json::from_slice(&fs::read(dir.join(PENDING_NAME))?)?;
            validate_manifest(&pending, &identity.project_id)?;
            validate_legacy_fence(&dir, &pending)?;
            pending
        }
    };
    let parsed = parse_journal(&segment_path(&dir, &manifest)?, &manifest)?;
    anyhow::ensure!(
        !parsed.incomplete_tail,
        "ROLLBACK_REFUSED_INCOMPLETE_TAIL: retry interrupted write first"
    );
    let current = parsed.ledger;
    let backup_path = legacy_backup_path(&dir);
    let legacy = load_legacy(&backup_path, &identity.project_id)?;
    if current.revision() != manifest.migrated_revision
        || current.revision() != legacy.revision()
        || current.head_hash != legacy.head_hash
        || current.events != legacy.events
    {
        bail!("ROLLBACK_REFUSED_NEW_EVENTS: migrated ledger changed after cutover; export/backup the journal and keep it authoritative");
    }
    // Fail-closed cutover: remove journal authority while legacy_path is still
    // the unsupported sentinel, then publish the verified legacy bytes.
    if manifest_path(&dir).try_exists()? {
        fs::remove_file(manifest_path(&dir))?;
    }
    failpoint("rollback_after_manifest_remove")?;
    crate::constitutive::write_atomic(&legacy_path(&dir), &fs::read(&backup_path)?)?;
    drop(_lock);
    inspect_development_storage(root)
}

pub fn backup_development_storage(root: &Path, target: &Path) -> Result<PathBuf> {
    let (owner, identity) = super::development_state_owner(root)?;
    let dir = super::development_dir(&owner);
    fs::create_dir_all(&dir)?;
    let _lock = DevelopmentAppendLock::acquire(&dir)?;
    let execution_dir = owner.join(".route/execution");
    let _evidence_lock = if execution_dir.try_exists()? {
        Some(crate::ownership_lock::OwnershipLock::acquire(
            &execution_dir.join(".evidence-write-lock"),
        )?)
    } else {
        None
    };
    crate::execution::EvidenceStore::load(&owner)?;
    let ledger = load(&dir, &identity.project_id, LoadMode::ReadOnly)?;
    if target.try_exists()? && fs::read_dir(target)?.next().is_some() {
        bail!(
            "backup target must be absent or empty: {}",
            target.display()
        );
    }
    fs::create_dir_all(target)?;
    let mut names = vec![];
    if let Some(manifest) = read_manifest(&dir, &identity.project_id)? {
        names.push(MANIFEST_NAME.to_string());
        names.push(manifest.segment);
        names.push(LEGACY_NAME.to_string());
    } else if legacy_path(&dir).try_exists()? {
        names.push(LEGACY_NAME.to_string());
    }
    let mut files = vec![];
    for name in names {
        let bytes = fs::read(dir.join(&name))?;
        fs::write(target.join(&name), &bytes)?;
        OpenOptions::new()
            .write(true)
            .open(target.join(&name))?
            .sync_all()?;
        files.push(BackupFile {
            name,
            bytes: bytes.len() as u64,
            sha256: route_core::sha256_hex(&bytes),
        });
    }
    // Evidence remains a separate store. Preserve its verified bytes so restored
    // event references resolve to the same IDs; this is not a cross-domain
    // transaction guarantee for Sessions, CheckRuns or external artifacts.
    let evidence_path = execution_dir.join("evidence.json");
    if evidence_path.try_exists()? {
        let bytes = fs::read(&evidence_path)?;
        fs::create_dir_all(target.join("execution"))?;
        crate::constitutive::write_atomic(&target.join("execution/evidence.json"), &bytes)?;
        files.push(BackupFile {
            name: "execution/evidence.json".into(),
            bytes: bytes.len() as u64,
            sha256: route_core::sha256_hex(&bytes),
        });
    }
    let manifest = BackupManifest {
        format: "route.development-backup".into(),
        version: 1,
        project_id: identity.project_id,
        storage_format: if manifest_path(&dir).try_exists()? {
            "FRAMED_JOURNAL"
        } else {
            "LEGACY_JSON"
        }
        .into(),
        revision: ledger.revision(),
        head_hash: ledger.head_hash,
        files,
    };
    crate::constitutive::write_atomic(
        &target.join("backup.manifest.json"),
        &serde_json::to_vec_pretty(&manifest)?,
    )?;
    Ok(target.to_path_buf())
}

pub fn restore_development_storage(
    backup: &Path,
    target_root: &Path,
) -> Result<DevelopmentStorageInspection> {
    let manifest_path_value = backup.join("backup.manifest.json");
    let manifest: BackupManifest = serde_json::from_slice(&fs::read(&manifest_path_value)?)?;
    if manifest.format != "route.development-backup" || manifest.version != 1 {
        bail!("unsupported development backup format");
    }
    if target_root.join(".route").try_exists()? {
        bail!(
            "restore target already contains Route state: {}",
            target_root.display()
        );
    }
    let mut expected: std::collections::BTreeSet<&str> = match manifest.storage_format.as_str() {
        "FRAMED_JOURNAL" => [MANIFEST_NAME, SEGMENT_NAME, LEGACY_NAME]
            .into_iter()
            .collect(),
        "LEGACY_JSON" => [LEGACY_NAME].into_iter().collect(),
        _ => bail!("unsupported backup storage format"),
    };
    if manifest
        .files
        .iter()
        .any(|f| f.name == "execution/evidence.json")
    {
        expected.insert("execution/evidence.json");
    }
    let actual: std::collections::BTreeSet<&str> =
        manifest.files.iter().map(|f| f.name.as_str()).collect();
    anyhow::ensure!(
        actual == expected && actual.len() == manifest.files.len(),
        "backup file set is invalid, incomplete or duplicated"
    );
    let mut verified = std::collections::BTreeMap::new();
    for entry in &manifest.files {
        let bytes = fs::read(backup.join(&entry.name))?;
        if bytes.len() as u64 != entry.bytes || route_core::sha256_hex(&bytes) != entry.sha256 {
            bail!("backup verification failed for {}", entry.name);
        }
        verified.insert(entry.name.clone(), bytes);
    }
    fs::create_dir_all(target_root)?;
    let staging = target_root.join(format!(".route-restore-{}", route_core::new_id()));
    fs::create_dir_all(staging.join(".route/development"))?;
    let canonical = fs::canonicalize(target_root).unwrap_or_else(|_| target_root.to_path_buf());
    let identity = ProjectIdentity {
        schema: 1,
        project_id: manifest.project_id.clone(),
        workspace_id: format!("ws_{}", route_core::new_id()),
        created_at: route_core::now_millis(),
        attached_from: None,
        bound_root: Some(canonical.to_string_lossy().replace('\\', "/")),
    };
    crate::constitutive::write_atomic(
        &identity_path(&staging),
        &serde_json::to_vec_pretty(&identity)?,
    )?;
    for entry in &manifest.files {
        let bytes = &verified[&entry.name];
        crate::constitutive::write_atomic(
            &if entry.name == "execution/evidence.json" {
                staging.join(".route").join(&entry.name)
            } else {
                staging.join(".route/development").join(&entry.name)
            },
            bytes,
        )?;
    }
    let inspected = inspect_development_storage(&staging)?;
    crate::execution::EvidenceStore::load(&staging)?;
    anyhow::ensure!(
        inspected.revision == manifest.revision,
        "restored revision mismatch"
    );
    anyhow::ensure!(
        inspected.head_hash == manifest.head_hash,
        "restored head mismatch"
    );
    fs::rename(staging.join(".route"), target_root.join(".route"))?;
    fs::remove_dir(staging)?;
    Ok(inspected)
}

#[cfg(any(test, feature = "test-utils"))]
fn failpoint_enabled(name: &str) -> bool {
    std::env::var("ROUTE_LEDGER_FAILPOINT").as_deref() == Ok(name)
}

#[cfg(not(any(test, feature = "test-utils")))]
fn failpoint_enabled(_name: &str) -> bool {
    false
}

#[cfg(any(test, feature = "test-utils"))]
fn crash_or_error(name: &str) -> Result<()> {
    if std::env::var_os("ROUTE_LEDGER_FAILPOINT_CRASH").is_some() {
        std::process::exit(86);
    }
    bail!("injected ledger failpoint: {name}")
}

#[cfg(not(any(test, feature = "test-utils")))]
fn crash_or_error(_name: &str) -> Result<()> {
    Ok(())
}

fn failpoint(name: &str) -> Result<()> {
    if failpoint_enabled(name) {
        crash_or_error(name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::development::{
        append_development_event, DevelopmentEventDraft, DevelopmentEventPayload,
    };
    use tempfile::tempdir;

    fn write_legacy_fixture(root: &Path, count: usize) -> DevelopmentLedger {
        let identity = crate::project_identity::ensure_identity(root).unwrap();
        let source = tempdir().unwrap();
        let mut ledger = DevelopmentLedger::empty(identity.project_id.clone());
        for index in 0..count {
            let mut event =
                append_development_event(source.path(), draft(&format!("legacy-{index}")))
                    .unwrap()
                    .event;
            event.project_id = identity.project_id.clone();
            event.sequence = index as u64 + 1;
            event.workspace_ref = None;
            event.previous_hash = ledger.head_hash.clone();
            event.hash = super::super::event_hash(&event).unwrap();
            ledger.head_hash = event.hash.clone();
            ledger.events.push(event);
        }
        let dir = super::super::development_dir(root);
        fs::create_dir_all(&dir).unwrap();
        crate::constitutive::write_atomic(
            &legacy_path(&dir),
            &serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();
        ledger
    }

    fn spawn_fixture(
        root: &Path,
        action: &str,
        failpoint: Option<&str>,
    ) -> std::process::ExitStatus {
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "development::storage::tests::ledger_process_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("ROUTE_LEDGER_FIXTURE_ROOT", root)
            .env("ROUTE_LEDGER_FIXTURE_ACTION", action);
        if let Some(point) = failpoint {
            command
                .env("ROUTE_LEDGER_FAILPOINT", point)
                .env("ROUTE_LEDGER_FAILPOINT_CRASH", "1");
        }
        command.status().unwrap()
    }

    fn draft(key: &str) -> DevelopmentEventDraft {
        DevelopmentEventDraft {
            event_id: Some(format!("event-{key}")),
            actor_worker_id: None,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: vec![],
            evidence_refs: vec![format!("evidence-{key}")],
            payload: DevelopmentEventPayload::Finding {
                summary: format!("finding {key}"),
                source_refs: vec![],
            },
            deduplication_key: Some(key.into()),
        }
    }

    #[test]
    fn new_projects_use_incremental_frames_and_replay_exactly() {
        let root = tempdir().unwrap();
        let first = append_development_event(root.path(), draft("one")).unwrap();
        let path = super::super::development_ledger_path(root.path()).unwrap();
        assert!(path.ends_with(SEGMENT_NAME));
        let before = fs::metadata(&path).unwrap().len();
        let second = append_development_event(root.path(), draft("two")).unwrap();
        let after = fs::metadata(&path).unwrap().len();
        assert_eq!(first.global_revision, 1);
        assert_eq!(second.global_revision, 2);
        assert!(after - before < 4096);
        let replay = append_development_event(root.path(), draft("two")).unwrap();
        assert!(replay.replay);
        assert_eq!(fs::metadata(path).unwrap().len(), after);
    }

    #[test]
    fn readonly_rejects_and_writer_recovers_only_incomplete_tail() {
        let root = tempdir().unwrap();
        append_development_event(root.path(), draft("one")).unwrap();
        let path = super::super::development_ledger_path(root.path()).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"EVT1partial")
            .unwrap();
        assert!(super::super::query_development_events(root.path(), 0, 10)
            .unwrap_err()
            .to_string()
            .contains("INCOMPLETE_TRAILING_RECORD"));
        let recovered = append_development_event(root.path(), draft("two")).unwrap();
        assert_eq!(recovered.global_revision, 2);
        assert_eq!(
            super::super::query_development_events(root.path(), 0, 10)
                .unwrap()
                .events
                .len(),
            2
        );
    }

    #[test]
    fn committed_corruption_is_never_truncated() {
        let root = tempdir().unwrap();
        append_development_event(root.path(), draft("one")).unwrap();
        append_development_event(root.path(), draft("two")).unwrap();
        let path = super::super::development_ledger_path(root.path()).unwrap();
        let mut bytes = fs::read(&path).unwrap();
        let position = bytes.windows(9).position(|w| w == b"finding o").unwrap();
        bytes[position] ^= 1;
        fs::write(&path, &bytes).unwrap();
        let before = fs::read(&path).unwrap();
        assert!(append_development_event(root.path(), draft("three"))
            .unwrap_err()
            .to_string()
            .contains("CORRUPT_COMMITTED_RECORD"));
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn legacy_migration_backup_restore_and_rollback_are_lossless() {
        let root = tempdir().unwrap();
        let identity = crate::project_identity::ensure_identity(root.path()).unwrap();
        let dir = super::super::development_dir(root.path());
        fs::create_dir_all(&dir).unwrap();
        let mut ledger = DevelopmentLedger::empty(identity.project_id.clone());
        // Produce canonical events in a temporary new journal, then serialize
        // those exact events into the legacy envelope fixture.
        let source = tempdir().unwrap();
        let a = append_development_event(source.path(), draft("one"))
            .unwrap()
            .event;
        let mut a = a;
        a.project_id = identity.project_id.clone();
        a.previous_hash = super::super::GENESIS_HASH.into();
        a.hash = super::super::event_hash(&a).unwrap();
        ledger.head_hash = a.hash.clone();
        ledger.events.push(a);
        let evidence_id = crate::execution::EvidenceStore::update(root.path(), |store| {
            Ok(store.record(
                "storage-fixture",
                crate::execution::EvidenceKind::Manual,
                crate::execution::EvidenceSource::User,
                route_core::sha256_hex(b"declared fixture note"),
                std::collections::HashMap::new(),
            ))
        })
        .unwrap();
        ledger.events[0].evidence_refs = vec![evidence_id];
        ledger.events[0].hash = super::super::event_hash(&ledger.events[0]).unwrap();
        ledger.head_hash = ledger.events[0].hash.clone();
        crate::constitutive::write_atomic(
            &legacy_path(&dir),
            &serde_json::to_vec_pretty(&ledger).unwrap(),
        )
        .unwrap();
        let migrated = migrate_development_storage(root.path()).unwrap();
        assert_eq!(migrated.format, "FRAMED_JOURNAL");
        let backup = tempdir().unwrap();
        let backup_dir = backup.path().join("ledger-backup");
        backup_development_storage(root.path(), &backup_dir).unwrap();
        let restored = tempdir().unwrap();
        let restored_state = restore_development_storage(&backup_dir, restored.path()).unwrap();
        assert_eq!(restored_state.revision, migrated.revision);
        assert_eq!(restored_state.head_hash, migrated.head_hash);
        assert_eq!(
            fs::read(root.path().join(".route/execution/evidence.json")).unwrap(),
            fs::read(restored.path().join(".route/execution/evidence.json")).unwrap()
        );
        assert_eq!(
            super::super::query_development_events(root.path(), 0, 100)
                .unwrap()
                .events,
            super::super::query_development_events(restored.path(), 0, 100)
                .unwrap()
                .events
        );
        let restored_identity = crate::project_identity::load_identity(restored.path())
            .unwrap()
            .unwrap();
        assert_eq!(restored_identity.project_id, migrated.project_id);
        assert_ne!(restored_identity.workspace_id, identity.workspace_id);
        let rolled = rollback_development_migration(root.path()).unwrap();
        assert_eq!(rolled.format, "LEGACY_JSON");
        migrate_development_storage(root.path()).unwrap();
        append_development_event(root.path(), draft("two")).unwrap();
        assert!(rollback_development_migration(root.path())
            .unwrap_err()
            .to_string()
            .contains("ROLLBACK_REFUSED_NEW_EVENTS"));
    }

    #[test]
    #[ignore = "subprocess fixture invoked by ledger crash and concurrency tests"]
    fn ledger_process_fixture() {
        let root = PathBuf::from(std::env::var_os("ROUTE_LEDGER_FIXTURE_ROOT").unwrap());
        let action = std::env::var("ROUTE_LEDGER_FIXTURE_ACTION").unwrap();
        if action == "migrate" {
            migrate_development_storage(&root).unwrap();
        } else if action == "read" {
            super::super::query_development_events(&root, 0, 1000).unwrap();
        } else {
            append_development_event(&root, draft(&action)).unwrap();
        }
    }

    #[test]
    fn real_process_crash_windows_a_through_h_recover_without_duplicates() {
        // A: before append. The empty initialized journal is valid and no event exists.
        let a = tempdir().unwrap();
        assert_eq!(
            spawn_fixture(a.path(), "a", Some("before_append")).code(),
            Some(86)
        );
        assert_eq!(
            super::super::global_development_revision(a.path()).unwrap(),
            0
        );

        // B/C: a partial prepare or fully durable PREPARE without COMMIT is not
        // an event. Read-only diagnoses it; the next writer truncates only that
        // recognized tail and commits exactly once.
        for (key, point) in [("b", "half_record"), ("c", "before_durable_commit")] {
            let root = tempdir().unwrap();
            assert_eq!(
                spawn_fixture(root.path(), key, Some(point)).code(),
                Some(86)
            );
            assert!(super::super::query_development_events(root.path(), 0, 10)
                .unwrap_err()
                .to_string()
                .contains("INCOMPLETE_TRAILING_RECORD"));
            let committed = append_development_event(root.path(), draft(key)).unwrap();
            assert!(!committed.replay);
            assert_eq!(committed.global_revision, 1);
        }

        // D/E: acknowledgement loss after durable COMMIT is recovered by the
        // operation key; retry returns the original event, never a duplicate.
        let d = tempdir().unwrap();
        assert_eq!(
            spawn_fixture(d.path(), "d", Some("after_durable_commit")).code(),
            Some(86)
        );
        let replay = append_development_event(d.path(), draft("d")).unwrap();
        assert!(replay.replay);
        assert_eq!(replay.global_revision, 1);

        // F/G: migration cutover fails closed before publication and is fully
        // authoritative after publication. Re-running resumes/idempotently reads.
        let f = tempdir().unwrap();
        write_legacy_fixture(f.path(), 2);
        assert_eq!(
            spawn_fixture(f.path(), "migrate", Some("migration_before_publish")).code(),
            Some(86)
        );
        assert!(super::super::query_development_events(f.path(), 0, 10).is_err());
        assert_eq!(migrate_development_storage(f.path()).unwrap().revision, 2);
        let g = tempdir().unwrap();
        write_legacy_fixture(g.path(), 2);
        assert_eq!(
            spawn_fixture(g.path(), "migrate", Some("migration_after_publish")).code(),
            Some(86)
        );
        assert_eq!(
            super::super::global_development_revision(g.path()).unwrap(),
            2
        );
        assert_eq!(migrate_development_storage(g.path()).unwrap().revision, 2);

        // H: recovery itself crashes after the safe truncation. The committed
        // prefix remains readable and a later retry commits once.
        let h = tempdir().unwrap();
        append_development_event(h.path(), draft("h-0")).unwrap();
        let path = super::super::development_ledger_path(h.path()).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"EVT1partial")
            .unwrap();
        assert_eq!(
            spawn_fixture(h.path(), "h-1", Some("recovery_after_truncate")).code(),
            Some(86)
        );
        assert_eq!(
            super::super::global_development_revision(h.path()).unwrap(),
            1
        );
        assert_eq!(
            append_development_event(h.path(), draft("h-1"))
                .unwrap()
                .global_revision,
            2
        );
    }

    #[test]
    fn real_process_writers_and_reader_preserve_one_total_order() {
        let root = tempdir().unwrap();
        crate::project_identity::ensure_identity(root.path()).unwrap();
        let mut children = vec![];
        for index in 0..8 {
            let root = root.path().to_path_buf();
            children.push(std::thread::spawn(move || {
                spawn_fixture(&root, &format!("different-{index}"), None)
            }));
        }
        for child in children {
            assert!(child.join().unwrap().success());
        }
        let page = super::super::query_development_events(root.path(), 0, 100).unwrap();
        assert_eq!(page.events.len(), 8);
        assert!(page
            .events
            .iter()
            .enumerate()
            .all(|(i, e)| e.sequence == i as u64 + 1));

        let left_root = root.path().to_path_buf();
        let right_root = left_root.clone();
        let left = std::thread::spawn(move || spawn_fixture(&left_root, "same", None));
        let right = std::thread::spawn(move || spawn_fixture(&right_root, "same", None));
        assert!(left.join().unwrap().success());
        assert!(right.join().unwrap().success());
        assert_eq!(
            super::super::global_development_revision(root.path()).unwrap(),
            9
        );

        let writer_root = root.path().to_path_buf();
        let reader_root = writer_root.clone();
        let writer = std::thread::spawn(move || spawn_fixture(&writer_root, "writer", None));
        let reader = std::thread::spawn(move || spawn_fixture(&reader_root, "read", None));
        assert!(writer.join().unwrap().success());
        assert!(reader.join().unwrap().success());
        let final_page = super::super::query_development_events(root.path(), 0, 100).unwrap();
        assert_eq!(final_page.global_revision, 10);
        assert_eq!(final_page.events.len(), 10);
    }

    #[test]
    fn malformed_storage_conditions_fail_closed_without_read_repair() {
        let missing = tempdir().unwrap();
        append_development_event(missing.path(), draft("one")).unwrap();
        let segment = super::super::development_ledger_path(missing.path()).unwrap();
        fs::remove_file(&segment).unwrap();
        assert!(
            super::super::query_development_events(missing.path(), 0, 10)
                .unwrap_err()
                .to_string()
                .contains("MISSING_SEGMENT")
        );

        let unknown = tempdir().unwrap();
        append_development_event(unknown.path(), draft("one")).unwrap();
        let dir = super::super::development_dir(unknown.path());
        let path = manifest_path(&dir);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        value["version"] = serde_json::json!(99);
        fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        let before = fs::read(&path).unwrap();
        assert!(
            super::super::query_development_events(unknown.path(), 0, 10)
                .unwrap_err()
                .to_string()
                .contains("UNKNOWN_FORMAT_VERSION")
        );
        assert_eq!(before, fs::read(path).unwrap());
    }

    #[test]
    fn injected_io_failures_never_report_false_success_or_erase_history() {
        for point in ["disk_full", "permission_denied", "write_returns_error"] {
            let root = tempdir().unwrap();
            let status = spawn_fixture(root.path(), point, Some(point));
            assert!(!status.success());
            assert_eq!(
                super::super::global_development_revision(root.path()).unwrap(),
                0
            );
        }

        // Commit bytes may have reached the OS before a flush error. The call
        // returns UNKNOWN, while retrying the same operation key discovers the
        // complete committed frame and cannot create another event.
        let flush = tempdir().unwrap();
        assert!(!spawn_fixture(flush.path(), "flush", Some("flush_fails")).success());
        let replay = append_development_event(flush.path(), draft("flush")).unwrap();
        assert!(replay.replay);
        assert_eq!(replay.global_revision, 1);

        let rename = tempdir().unwrap();
        write_legacy_fixture(rename.path(), 1);
        assert!(!spawn_fixture(rename.path(), "migrate", Some("rename_fails")).success());
        assert_eq!(
            super::super::global_development_revision(rename.path()).unwrap(),
            1
        );
    }

    #[test]
    fn middle_frame_damage_and_one_hundred_reads_are_fail_closed_and_zero_growth() {
        let root = tempdir().unwrap();
        append_development_event(root.path(), draft("one")).unwrap();
        append_development_event(root.path(), draft("two")).unwrap();
        let dir = super::super::development_dir(root.path());
        let path = super::super::development_ledger_path(root.path()).unwrap();
        let before_files = || {
            let mut files = fs::read_dir(&dir)
                .unwrap()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().unwrap().is_file())
                .map(|entry| {
                    let bytes = fs::read(entry.path()).unwrap();
                    (
                        entry.file_name(),
                        route_core::sha256_hex(&bytes),
                        bytes.len(),
                    )
                })
                .collect::<Vec<_>>();
            files.sort_by(|a, b| a.0.cmp(&b.0));
            files
        };
        let snapshot = before_files();
        for _ in 0..100 {
            assert_eq!(
                super::super::global_development_revision(root.path()).unwrap(),
                2
            );
        }
        assert_eq!(snapshot, before_files());

        let mut bytes = fs::read(&path).unwrap();
        let first = bytes.windows(4).position(|v| v == PREPARE_MAGIC).unwrap();
        bytes[first] ^= 1;
        fs::write(&path, &bytes).unwrap();
        assert!(super::super::query_development_events(root.path(), 0, 10)
            .unwrap_err()
            .to_string()
            .contains("CORRUPT_MIDDLE_RECORD"));
        assert_eq!(fs::read(path).unwrap(), bytes);
    }

    #[test]
    fn acknowledged_truncation_and_corrupt_length_cannot_be_recovered_as_tail() {
        for damage in ["truncate", "length"] {
            let root = tempdir().unwrap();
            append_development_event(root.path(), draft("one")).unwrap();
            append_development_event(root.path(), draft("two")).unwrap();
            let path = super::super::development_ledger_path(root.path()).unwrap();
            let mut bytes = fs::read(&path).unwrap();
            if damage == "truncate" {
                bytes.truncate(bytes.len() - 100);
            } else {
                let first = bytes.windows(4).position(|v| v == PREPARE_MAGIC).unwrap();
                bytes[first + 4..first + 12].copy_from_slice(&1_000_000u64.to_le_bytes());
            }
            fs::write(&path, &bytes).unwrap();
            assert!(append_development_event(root.path(), draft("three")).is_err());
            assert_eq!(bytes, fs::read(path).unwrap());
        }
    }

    #[test]
    fn remigration_uses_new_legacy_writes_and_initialization_crash_resumes() {
        let root = tempdir().unwrap();
        write_legacy_fixture(root.path(), 1);
        migrate_development_storage(root.path()).unwrap();
        rollback_development_migration(root.path()).unwrap();
        append_development_event(root.path(), draft("after-rollback")).unwrap();
        let before = super::super::query_development_events(root.path(), 0, 100).unwrap();
        migrate_development_storage(root.path()).unwrap();
        assert_eq!(
            before,
            super::super::query_development_events(root.path(), 0, 100).unwrap()
        );

        let init = tempdir().unwrap();
        assert_eq!(
            spawn_fixture(init.path(), "init", Some("before_manifest_publish")).code(),
            Some(86)
        );
        assert!(super::super::query_development_events(init.path(), 0, 10).is_err());
        assert_eq!(
            append_development_event(init.path(), draft("init"))
                .unwrap()
                .global_revision,
            1
        );
    }

    #[test]
    fn restore_rejects_traversal_and_false_receipts_before_publishing_state() {
        let root = tempdir().unwrap();
        append_development_event(root.path(), draft("one")).unwrap();
        let backup = tempdir().unwrap();
        backup_development_storage(root.path(), backup.path()).unwrap();
        let receipt_path = backup.path().join("backup.manifest.json");
        let original = fs::read(&receipt_path).unwrap();
        let mut receipt: BackupManifest = serde_json::from_slice(&original).unwrap();
        receipt.files[0].name = "../escape".into();
        fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        let target = tempdir().unwrap();
        assert!(restore_development_storage(backup.path(), target.path()).is_err());
        assert!(!target.path().join(".route").exists());
        let mut receipt: BackupManifest = serde_json::from_slice(&original).unwrap();
        receipt.revision += 1;
        fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
        assert!(restore_development_storage(backup.path(), target.path()).is_err());
        assert!(!target.path().join(".route").exists());
    }

    #[test]
    fn arbitrary_tail_bytes_are_not_a_recoverable_write() {
        let root = tempdir().unwrap();
        append_development_event(root.path(), draft("one")).unwrap();
        let path = super::super::development_ledger_path(root.path()).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"BAD")
            .unwrap();
        let before = fs::read(&path).unwrap();
        assert!(append_development_event(root.path(), draft("two")).is_err());
        assert_eq!(before, fs::read(path).unwrap());
    }

    #[test]
    #[ignore = "explicit ledger write/read benchmark; run and capture output for storage acceptance"]
    fn ledger_storage_performance() {
        use std::time::Instant;
        let mut rows = vec![];
        for count in [100usize, 400, 1000] {
            for format in ["LEGACY_JSON", "FRAMED_JOURNAL"] {
                let root = tempdir().unwrap();
                if format == "LEGACY_JSON" {
                    write_legacy_fixture(root.path(), 0);
                }
                let mut append_ms = Vec::with_capacity(count);
                let mut written = 0u64;
                let mut metadata_written = 0u64;
                let mut previous = 0u64;
                for index in 0..count {
                    let start = Instant::now();
                    append_development_event(root.path(), draft(&format!("perf-{index}"))).unwrap();
                    append_ms.push(start.elapsed().as_secs_f64() * 1000.0);
                    let path = super::super::development_ledger_path(root.path()).unwrap();
                    let length = fs::metadata(path).unwrap().len();
                    written += if format == "LEGACY_JSON" {
                        length
                    } else {
                        length - previous
                    };
                    previous = length;
                    if format == "FRAMED_JOURNAL" {
                        let dir = super::super::development_dir(root.path());
                        metadata_written += fs::metadata(manifest_path(&dir)).unwrap().len();
                        if index == 0 {
                            // Initialization writes pending metadata, an initial
                            // manifest and the compatibility fence once.
                            metadata_written +=
                                2 * fs::metadata(dir.join(PENDING_NAME)).unwrap().len();
                            metadata_written += fs::metadata(legacy_path(&dir)).unwrap().len();
                        }
                    }
                }
                append_ms.sort_by(f64::total_cmp);
                let cold_start = Instant::now();
                let cold_page =
                    super::super::query_development_events(root.path(), 0, 1000).unwrap();
                let reader_cold_ms = cold_start.elapsed().as_secs_f64() * 1000.0;
                assert_eq!(cold_page.global_revision, count as u64);
                let mut reads = vec![];
                for _ in 0..20 {
                    let start = Instant::now();
                    let page =
                        super::super::query_development_events(root.path(), 0, 1000).unwrap();
                    assert_eq!(page.global_revision, count as u64);
                    reads.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                reads.sort_by(f64::total_cmp);
                let logical =
                    fs::metadata(super::super::development_ledger_path(root.path()).unwrap())
                        .unwrap()
                        .len();
                rows.push(serde_json::json!({
                    "format": format,
                    "events": count,
                    "logical_ledger_bytes": logical,
                    "cumulative_writer_bytes": written,
                    "metadata_writer_bytes": metadata_written,
                    "total_storage_write_bytes": written + metadata_written,
                    "append_p50_ms": append_ms[count / 2],
                    "append_p95_ms": append_ms[(count * 95 / 100).min(count - 1)],
                    "reader_cold_ms": reader_cold_ms,
                    "reader_warm_p50_ms": reads[10],
                    "reader_warm_p95_ms": reads[18],
                    "full_replay_ms": reader_cold_ms,
                    "read_scaling": "OPEN_LINEAR_FULL_REPLAY"
                }));
            }
        }
        let report = serde_json::json!({"profile":"cargo-test-debug","rows":rows});
        println!("LEDGER_STORAGE_PERFORMANCE={report}");
        if let Ok(path) = std::env::var("ROUTE_LEDGER_PERF_REPORT") {
            fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
        }
    }
}
