//! Project-scoped cooperation resources and revisable shared knowledge.
//!
//! The development ledger is the only durable store for these objects.  This
//! module projects typed events into current state; it never copies resource
//! content, executes commands, contacts URIs, or recursively scans directories.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::development::{
    append_development_event, load_ledger_readonly as load_ledger, AppendDevelopmentEventResult,
    DevelopmentEventDraft, DevelopmentEventPayload,
};

const MAX_TEXT: usize = 2_000;
const MAX_LOCATOR: usize = 4_096;
const MAX_REFS: usize = 128;
const FULL_HASH_LIMIT: u64 = 1_048_576;
const HASH_SAMPLE: usize = 65_536;

/// Forward-extensible cooperation kind. Known values are constants, while an
/// unknown future string remains readable and queryable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct CooperationKind(pub String);

impl CooperationKind {
    pub const FILE: &'static str = "FILE";
    pub const DOCUMENT: &'static str = "DOCUMENT";
    pub const EXECUTABLE: &'static str = "EXECUTABLE";
    pub const CLI: &'static str = "CLI";
    pub const SCRIPT: &'static str = "SCRIPT";
    pub const DIRECTORY: &'static str = "DIRECTORY";
    pub const PROJECT: &'static str = "PROJECT";
    pub const SERVICE: &'static str = "SERVICE";
    pub const DATASET: &'static str = "DATASET";
    pub const MODEL: &'static str = "MODEL";
    pub const PROTOCOL: &'static str = "PROTOCOL";
    pub const UNKNOWN: &'static str = "UNKNOWN";

    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() || value.len() > 128 {
            bail!("cooperation kind must contain 1..128 characters");
        }
        if !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        {
            bail!("cooperation kind contains unsupported characters");
        }
        Ok(Self(value.to_ascii_uppercase()))
    }

    pub fn is(&self, known: &str) -> bool {
        self.0.eq_ignore_ascii_case(known)
    }
}

impl Default for CooperationKind {
    fn default() -> Self {
        Self(Self::UNKNOWN.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CooperationAvailability {
    Available,
    Missing,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpistemicStatus {
    Declared,
    Observed,
    Inferred,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CooperationDiscoveryStatus {
    Started,
    Updated,
    Completed,
    RequiresExternalDiscovery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CooperationFingerprint {
    pub algorithm: String,
    pub value: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_at_millis: Option<i64>,
}

/// Immutable registration data stored in a typed DevelopmentEvent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CooperationResourceRegistration {
    pub cooperation_id: String,
    pub locator: String,
    pub kind: CooperationKind,
    pub provenance: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub availability: CooperationAvailability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<CooperationFingerprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovered_project_id: Option<String>,
    #[serde(default)]
    pub declared_capabilities: Vec<String>,
    #[serde(default)]
    pub related_reference_ids: Vec<String>,
    #[serde(default)]
    pub related_constraint_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CooperationRefreshObservation {
    pub cooperation_id: String,
    pub availability: CooperationAvailability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<CooperationFingerprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovered_project_id: Option<String>,
    pub observed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CooperationResource {
    pub cooperation_id: String,
    pub project_id: String,
    pub locator: String,
    pub kind: CooperationKind,
    pub provenance: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: u64,
    pub availability: CooperationAvailability,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<CooperationFingerprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovered_project_id: Option<String>,
    pub declared_capabilities: Vec<String>,
    pub observed_capabilities: Vec<String>,
    pub inferred_capabilities: Vec<String>,
    pub related_reference_ids: Vec<String>,
    pub related_constraint_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CooperationKnowledgeRecord {
    pub knowledge_id: String,
    pub cooperation_id: String,
    pub statement: String,
    #[serde(default)]
    pub capability_refs: Vec<String>,
    pub epistemic_status: EpistemicStatus,
    pub provenance: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub observed_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CooperationKnowledge {
    #[serde(flatten)]
    pub record: CooperationKnowledgeRecord,
    pub project_id: String,
    pub revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CooperationDiscoveryRecord {
    pub discovery_id: String,
    pub cooperation_id: String,
    pub status: CooperationDiscoveryStatus,
    pub safe_step: String,
    pub summary: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CooperationDiscovery {
    #[serde(flatten)]
    pub record: CooperationDiscoveryRecord,
    pub project_id: String,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CooperationResourceSummary {
    pub cooperation_id: String,
    pub kind: CooperationKind,
    pub availability: CooperationAvailability,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CooperationKnowledgeSummary {
    pub knowledge_id: String,
    pub cooperation_id: String,
    pub epistemic_status: EpistemicStatus,
    pub revision: u64,
    pub stale: bool,
}

/// Bounded reference metadata suitable for the global development ledger.
/// It intentionally contains no reference content or locator text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReferenceCommonsObservation {
    pub reference_id: String,
    pub locator_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<CooperationFingerprint>,
    pub status: CooperationAvailability,
    pub observed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HumanCooperationGuidance {
    pub guidance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cooperation_id: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub created_at: i64,
}

fn validate_id(label: &str, value: &str) -> Result<()> {
    validate_public_metadata(value)?;
    if value.trim().is_empty() || value.len() > 256 {
        bail!("{label} must contain 1..256 characters");
    }
    Ok(())
}

fn validate_text(label: &str, value: &str, max: usize) -> Result<()> {
    validate_public_metadata(value)?;
    if value.trim().is_empty() || value.len() > max {
        bail!("{label} must contain 1..{max} characters");
    }
    Ok(())
}

/// Shared boundary for locators/names persisted into public project metadata.
pub fn validate_public_metadata(value: &str) -> Result<()> {
    let lower = value.to_ascii_lowercase();
    if [
        "password=",
        "token=",
        "api_key=",
        "apikey=",
        "secret=",
        "authorization:",
        "-----begin private key",
        "sk-proj-",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
        || value
            .split_once("://")
            .is_some_and(|(_, rest)| rest.split('/').next().unwrap_or_default().contains('@'))
    {
        bail!("SECRET_METADATA_REJECTED: remove credentials from the locator/name; no secret was stored");
    }
    Ok(())
}

fn validate_refs(label: &str, refs: &[String]) -> Result<()> {
    if refs.len() > MAX_REFS {
        bail!("{label} exceeds {MAX_REFS} entries");
    }
    for value in refs {
        validate_id(label, value)?;
    }
    Ok(())
}

pub(crate) fn validate_registration(value: &CooperationResourceRegistration) -> Result<()> {
    validate_id("cooperation_id", &value.cooperation_id)?;
    validate_text("locator", &value.locator, MAX_LOCATOR)?;
    CooperationKind::new(&value.kind.0)?;
    validate_text("provenance", &value.provenance, MAX_TEXT)?;
    if let Some(description) = &value.description {
        validate_text("description", description, MAX_TEXT)?;
    }
    validate_refs("capability", &value.declared_capabilities)?;
    validate_refs("reference id", &value.related_reference_ids)?;
    validate_refs("constraint id", &value.related_constraint_ids)
}

pub(crate) fn validate_refresh(value: &CooperationRefreshObservation) -> Result<()> {
    validate_id("cooperation_id", &value.cooperation_id)
}

pub(crate) fn validate_knowledge(value: &CooperationKnowledgeRecord) -> Result<()> {
    validate_id("knowledge_id", &value.knowledge_id)?;
    validate_id("cooperation_id", &value.cooperation_id)?;
    validate_text("knowledge statement", &value.statement, MAX_TEXT)?;
    validate_text("knowledge provenance", &value.provenance, MAX_TEXT)?;
    validate_refs("capability", &value.capability_refs)?;
    validate_refs("source ref", &value.source_refs)?;
    validate_refs("evidence ref", &value.evidence_refs)?;
    if let Some(supersedes) = &value.supersedes {
        validate_id("supersedes", supersedes)?;
        if supersedes == &value.knowledge_id {
            bail!("knowledge cannot supersede itself");
        }
    }
    Ok(())
}

pub(crate) fn validate_discovery(value: &CooperationDiscoveryRecord) -> Result<()> {
    validate_id("discovery_id", &value.discovery_id)?;
    validate_id("cooperation_id", &value.cooperation_id)?;
    validate_text("safe discovery step", &value.safe_step, 256)?;
    validate_text("discovery summary", &value.summary, MAX_TEXT)?;
    validate_refs("source ref", &value.source_refs)
}

pub(crate) fn validate_reference_observation(value: &ReferenceCommonsObservation) -> Result<()> {
    validate_id("reference_id", &value.reference_id)?;
    if value.locator_hash.len() != 64
        || !value.locator_hash.chars().all(|ch| ch.is_ascii_hexdigit())
    {
        bail!("reference locator_hash must be a SHA-256 hex digest");
    }
    Ok(())
}

pub(crate) fn validate_capability_observation(value: &CooperationKnowledgeRecord) -> Result<()> {
    validate_knowledge(value)?;
    if value.epistemic_status != EpistemicStatus::Observed {
        bail!("a capability observation must have OBSERVED epistemic status");
    }
    if value.capability_refs.is_empty() {
        bail!("a capability observation requires at least one capability ref");
    }
    if value.evidence_refs.is_empty() {
        bail!("a capability observation requires existing evidence refs");
    }
    Ok(())
}

pub(crate) fn validate_human_guidance(value: &HumanCooperationGuidance) -> Result<()> {
    validate_id("guidance_id", &value.guidance_id)?;
    if let Some(id) = &value.cooperation_id {
        validate_id("cooperation_id", id)?;
    }
    validate_text("human cooperation guidance", &value.summary, MAX_TEXT)?;
    validate_refs("source ref", &value.source_refs)
}

fn is_uri(locator: &str) -> bool {
    locator.split_once(':').is_some_and(|(scheme, rest)| {
        !rest.is_empty()
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

fn normalize_relative(locator: &str) -> Result<PathBuf> {
    let path = Path::new(locator);
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        bail!("relative cooperation locator cannot escape the project root");
    }
    Ok(path
        .components()
        .filter(|part| !matches!(part, Component::CurDir))
        .collect())
}

fn modified_millis(metadata: &fs::Metadata) -> Option<i64> {
    metadata
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|value| value.as_millis().min(i64::MAX as u128) as i64)
}

fn file_fingerprint(path: &Path, metadata: &fs::Metadata) -> Result<CooperationFingerprint> {
    let size = metadata.len();
    let modified = modified_millis(metadata);
    if size <= FULL_HASH_LIMIT {
        let mut bytes = Vec::new();
        fs::File::open(path)?
            .take(FULL_HASH_LIMIT + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > FULL_HASH_LIMIT {
            bail!("RESOURCE_CHANGED: resource grew during bounded inspection; retry refresh");
        }
        return Ok(CooperationFingerprint {
            algorithm: "SHA256_FULL_V1".into(),
            value: route_core::sha256_hex(&bytes),
            size_bytes: Some(size),
            modified_at_millis: modified,
        });
    }
    let mut file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut sample = vec![0_u8; HASH_SAMPLE];
    file.read_exact(&mut sample)?;
    file.seek(SeekFrom::End(-(HASH_SAMPLE as i64)))?;
    let mut tail = vec![0_u8; HASH_SAMPLE];
    file.read_exact(&mut tail)?;
    let mut material = format!("{size}\n{:?}\n", metadata.modified().ok()).into_bytes();
    material.extend_from_slice(&sample);
    material.extend_from_slice(&tail);
    Ok(CooperationFingerprint {
        algorithm: "SHA256_BOUNDED_HEAD_TAIL_MTIME_V2".into(),
        value: route_core::sha256_hex(&material),
        size_bytes: Some(size),
        modified_at_millis: modified,
    })
}

fn metadata_fingerprint(metadata: &fs::Metadata, object_type: &str) -> CooperationFingerprint {
    let modified = modified_millis(metadata);
    let material = format!("{object_type}\n{}\n{:?}", metadata.len(), modified);
    CooperationFingerprint {
        algorithm: "SHA256_METADATA_V1".into(),
        value: route_core::sha256_hex(material.as_bytes()),
        size_bytes: Some(metadata.len()),
        modified_at_millis: modified,
    }
}

fn resolve_path_command(command: &str) -> Option<PathBuf> {
    if command.contains('/') || command.contains('\\') {
        return None;
    }
    let path = std::env::var_os("PATH")?;
    let extensions: Vec<String> = if cfg!(windows) {
        std::env::var("PATHEXT")
            .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .map(str::to_string)
            .collect()
    } else {
        vec![String::new()]
    };
    for dir in std::env::split_paths(&path) {
        for extension in &extensions {
            let candidate = if Path::new(command).extension().is_some() {
                dir.join(command)
            } else {
                dir.join(format!("{command}{extension}"))
            };
            if fs::symlink_metadata(&candidate)
                .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
            {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn inspect_locator(
    project_root: &Path,
    locator: &str,
    kind: &CooperationKind,
) -> Result<(
    String,
    CooperationAvailability,
    Option<CooperationFingerprint>,
    Option<String>,
)> {
    let locator = locator.trim();
    validate_public_metadata(locator)?;
    let input = Path::new(locator);
    if !input.is_absolute() && is_uri(locator) {
        return Ok((
            locator.to_string(),
            CooperationAvailability::Unknown,
            None,
            None,
        ));
    }
    if (kind.is(CooperationKind::CLI) || kind.is(CooperationKind::EXECUTABLE))
        && !Path::new(locator).is_absolute()
        && !locator.contains('/')
        && !locator.contains('\\')
    {
        let Some(path) = resolve_path_command(locator) else {
            return Ok((
                locator.to_string(),
                CooperationAvailability::Missing,
                None,
                None,
            ));
        };
        let metadata = fs::symlink_metadata(&path)?;
        return Ok((
            locator.to_string(),
            CooperationAvailability::Available,
            Some(file_fingerprint(&path, &metadata)?),
            None,
        ));
    }
    let resolved = if input.is_absolute() {
        input.to_path_buf()
    } else {
        project_root.join(normalize_relative(locator)?)
    };
    let normalized = if input.is_absolute() {
        resolved.to_string_lossy().to_string()
    } else {
        normalize_relative(locator)?
            .to_string_lossy()
            .replace('\\', "/")
    };
    let mut ancestor = PathBuf::new();
    for part in resolved.components() {
        ancestor.push(part.as_os_str());
        if let Ok(meta) = fs::symlink_metadata(&ancestor) {
            #[cfg(windows)]
            let linked = {
                use std::os::windows::fs::MetadataExt;
                meta.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let linked = meta.file_type().is_symlink();
            if linked {
                return Ok((normalized, CooperationAvailability::Unavailable, None, None));
            }
        }
    }
    let metadata = match fs::symlink_metadata(&resolved) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((normalized, CooperationAvailability::Missing, None, None));
        }
        Err(_) => return Ok((normalized, CooperationAvailability::Unavailable, None, None)),
    };
    if metadata.file_type().is_symlink() {
        return Ok((
            normalized,
            CooperationAvailability::Available,
            Some(metadata_fingerprint(&metadata, "symlink")),
            None,
        ));
    }
    let fingerprint = if metadata.is_file() {
        Some(file_fingerprint(&resolved, &metadata)?)
    } else if metadata.is_dir() {
        Some(metadata_fingerprint(&metadata, "directory"))
    } else {
        Some(metadata_fingerprint(&metadata, "other"))
    };
    let discovered_project_id = if kind.is(CooperationKind::PROJECT) && metadata.is_dir() {
        // Strictly read-only: never call ensure_identity on a cooperating project.
        crate::project_identity::load_identity(&resolved)?.map(|identity| identity.project_id)
    } else {
        None
    };
    Ok((
        normalized,
        CooperationAvailability::Available,
        fingerprint,
        discovered_project_id,
    ))
}

pub fn register_cooperation_resource(
    root: &Path,
    cooperation_id: String,
    locator: String,
    kind: CooperationKind,
    provenance: String,
    description: Option<String>,
    declared_capabilities: Vec<String>,
    related_reference_ids: Vec<String>,
    related_constraint_ids: Vec<String>,
    actor_worker_id: Option<String>,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    let (locator, availability, fingerprint, discovered_project_id) =
        inspect_locator(root, &locator, &kind)?;
    let registration = CooperationResourceRegistration {
        cooperation_id,
        locator,
        kind,
        provenance,
        description,
        availability,
        fingerprint,
        discovered_project_id,
        declared_capabilities,
        related_reference_ids,
        related_constraint_ids,
    };
    validate_registration(&registration)?;
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: registration.related_reference_ids.clone(),
            evidence_refs: vec![],
            payload: DevelopmentEventPayload::CooperationResourceRegistered { registration },
            deduplication_key,
        },
    )
}

pub fn refresh_cooperation_resource(
    root: &Path,
    cooperation_id: &str,
    actor_worker_id: Option<String>,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    let resource = cooperation_resource(root, cooperation_id)?
        .ok_or_else(|| anyhow::anyhow!("cooperation resource does not exist"))?;
    let (_, availability, fingerprint, discovered_project_id) =
        inspect_locator(root, &resource.locator, &resource.kind)?;
    let observation = CooperationRefreshObservation {
        cooperation_id: cooperation_id.to_string(),
        availability,
        fingerprint,
        discovered_project_id,
        observed_at: route_core::now_millis(),
    };
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: vec![],
            evidence_refs: vec![],
            payload: DevelopmentEventPayload::CooperationResourceRefreshed { observation },
            deduplication_key,
        },
    )
}

pub fn record_cooperation_knowledge(
    root: &Path,
    record: CooperationKnowledgeRecord,
    actor_worker_id: Option<String>,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    validate_knowledge(&record)?;
    if cooperation_resource(root, &record.cooperation_id)?.is_none() {
        bail!("cooperation resource does not exist");
    }
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: record.source_refs.clone(),
            evidence_refs: record.evidence_refs.clone(),
            payload: DevelopmentEventPayload::CooperationKnowledgeRecorded { record },
            deduplication_key,
        },
    )
}

/// Record a capability as OBSERVED. Unlike declarations and inferences, this
/// path requires every cited Evidence id to exist in the canonical
/// EvidenceStore. The knowledge remains knowledge; no Evidence is created.
pub fn record_cooperation_capability(
    root: &Path,
    record: CooperationKnowledgeRecord,
    actor_worker_id: Option<String>,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    validate_capability_observation(&record)?;
    if cooperation_resource(root, &record.cooperation_id)?.is_none() {
        bail!("cooperation resource does not exist");
    }
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: record.source_refs.clone(),
            evidence_refs: record.evidence_refs.clone(),
            payload: DevelopmentEventPayload::CooperationCapabilityObserved { record },
            deduplication_key,
        },
    )
}

pub fn record_human_cooperation_guidance(
    root: &Path,
    guidance: HumanCooperationGuidance,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    validate_human_guidance(&guidance)?;
    if let Some(id) = &guidance.cooperation_id {
        if cooperation_resource(root, id)?.is_none() {
            bail!("cooperation resource does not exist");
        }
    }
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id: None,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: guidance.source_refs.clone(),
            evidence_refs: vec![],
            payload: DevelopmentEventPayload::HumanCooperationGuidance { guidance },
            deduplication_key,
        },
    )
}

/// The sole stateful validator. Called by append_development_event after
/// idempotent replay resolution, with the ledger writer lock held.
pub(crate) fn validate_transition(
    root: &Path,
    project_id: &str,
    events: &[crate::development::DevelopmentEvent],
    payload: &DevelopmentEventPayload,
) -> Result<()> {
    let resources = project_cooperation_resources(events);
    let known = project_cooperation_knowledge(events, &resources);
    match payload {
        DevelopmentEventPayload::ReferenceRegistryCommitted {
            operation_id,
            registry_hash,
        } => {
            crate::reference_operation::validate_event(root, operation_id, registry_hash)?;
        }
        DevelopmentEventPayload::ReferenceRegistered { .. }
        | DevelopmentEventPayload::ReferenceChanged { .. }
        | DevelopmentEventPayload::ReferenceRefreshed { .. }
        | DevelopmentEventPayload::ReferenceMissing { .. } => {
            bail!("Reference transitions must use the recoverable registry operation API");
        }
        DevelopmentEventPayload::CooperationResourceRegistered { registration } => {
            if resources.contains_key(&registration.cooperation_id) {
                bail!("cooperation resource ID conflict; use refresh or replay the original operation");
            }
            let (_, availability, fingerprint, discovered) =
                inspect_locator(root, &registration.locator, &registration.kind)?;
            if availability != registration.availability
                || fingerprint != registration.fingerprint
                || discovered != registration.discovered_project_id
            {
                bail!("resource observation changed or was fabricated; retry discovery");
            }
        }
        DevelopmentEventPayload::CooperationResourceRefreshed { observation } => {
            let resource = resources
                .get(&observation.cooperation_id)
                .ok_or_else(|| anyhow::anyhow!("unknown cooperation resource"))?;
            let (_, availability, fingerprint, discovered) =
                inspect_locator(root, &resource.locator, &resource.kind)?;
            if availability != observation.availability
                || fingerprint != observation.fingerprint
                || discovered != observation.discovered_project_id
            {
                bail!("resource refresh observation is not current");
            }
        }
        DevelopmentEventPayload::CooperationKnowledgeRecorded { record }
        | DevelopmentEventPayload::CooperationCapabilityObserved { record } => {
            let resource = resources
                .get(&record.cooperation_id)
                .ok_or_else(|| anyhow::anyhow!("unknown cooperation resource"))?;
            if known
                .iter()
                .any(|old| old.record.knowledge_id == record.knowledge_id)
            {
                bail!("knowledge ID conflict; replay the original operation");
            }
            if let Some(id) = &record.supersedes {
                let old = known
                    .iter()
                    .find(|old| old.record.knowledge_id == *id)
                    .ok_or_else(|| {
                        anyhow::anyhow!("superseded knowledge does not exist in this project")
                    })?;
                if old.project_id != project_id
                    || old.record.cooperation_id != record.cooperation_id
                    || old.superseded_by.is_some()
                {
                    bail!("invalid or concurrently superseded knowledge target");
                }
            }
            if record.epistemic_status == EpistemicStatus::Observed {
                if known.iter().any(|old| {
                    !old.stale
                        && old.record.cooperation_id == record.cooperation_id
                        && old.record.epistemic_status == EpistemicStatus::Observed
                        && old
                            .record
                            .capability_refs
                            .iter()
                            .any(|capability| record.capability_refs.contains(capability))
                        && record.supersedes.as_deref() != Some(old.record.knowledge_id.as_str())
                }) {
                    bail!("observed capability conflict: explicitly supersede its current observation");
                }
                validate_observed(root, project_id, resource, record)?;
            }
        }
        DevelopmentEventPayload::CooperationDiscoveryUpdated { record } => {
            if !resources.contains_key(&record.cooperation_id) {
                bail!("unknown discovery resource");
            }
            for event in events {
                if let DevelopmentEventPayload::CooperationDiscoveryUpdated { record: old } =
                    &event.payload
                {
                    if old.discovery_id == record.discovery_id
                        && old.cooperation_id != record.cooperation_id
                    {
                        bail!("discovery identity conflict");
                    }
                }
            }
        }
        DevelopmentEventPayload::HumanCooperationGuidance { guidance } => {
            if guidance
                .cooperation_id
                .as_ref()
                .is_some_and(|id| !resources.contains_key(id))
            {
                bail!("unknown guidance resource");
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_observed(
    root: &Path,
    project_id: &str,
    resource: &CooperationResource,
    record: &CooperationKnowledgeRecord,
) -> Result<()> {
    use crate::execution::{EvidenceKind, EvidenceSource, EvidenceStore};
    let fingerprint = resource
        .fingerprint
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("OBSERVED requires a current resource fingerprint"))?;
    if resource.availability != CooperationAvailability::Available
        || record.resource_fingerprint.as_deref() != Some(fingerprint.value.as_str())
    {
        bail!("OBSERVED requires an available, fingerprint-bound resource");
    }
    let (_, availability, current, _) = inspect_locator(root, &resource.locator, &resource.kind)?;
    if availability != CooperationAvailability::Available || current.as_ref() != Some(fingerprint) {
        bail!("resource is stale; refresh before recording observation");
    }
    if record.evidence_refs.is_empty() {
        bail!("OBSERVED requires existing Evidence");
    }
    let evidence = EvidenceStore::load(root)?;
    for id in &record.evidence_refs {
        let item = evidence
            .evidence
            .iter()
            .find(|item| item.id == *id)
            .ok_or_else(|| anyhow::anyhow!("unknown Evidence ID"))?;
        if item.source != EvidenceSource::System
            || !matches!(item.kind, EvidenceKind::TestPass | EvidenceKind::CheckPass)
            || item.metadata.get("project_id").map(String::as_str) != Some(project_id)
            || item.metadata.get("cooperation_id").map(String::as_str)
                != Some(record.cooperation_id.as_str())
            || item
                .metadata
                .get("resource_fingerprint")
                .map(String::as_str)
                != Some(fingerprint.value.as_str())
        {
            bail!("Evidence must be successful System evidence bound to this project and resource version");
        }
    }
    Ok(())
}

pub fn update_cooperation_discovery(
    root: &Path,
    record: CooperationDiscoveryRecord,
    actor_worker_id: Option<String>,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    validate_discovery(&record)?;
    if cooperation_resource(root, &record.cooperation_id)?.is_none() {
        bail!("cooperation resource does not exist");
    }
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: record.source_refs.clone(),
            evidence_refs: vec![],
            payload: DevelopmentEventPayload::CooperationDiscoveryUpdated { record },
            deduplication_key,
        },
    )
}

fn project_cooperation_resources(
    events: &[crate::development::DevelopmentEvent],
) -> BTreeMap<String, CooperationResource> {
    let mut resources = BTreeMap::<String, CooperationResource>::new();
    for event in events {
        match &event.payload {
            DevelopmentEventPayload::CooperationResourceRegistered { registration } => {
                resources.insert(
                    registration.cooperation_id.clone(),
                    CooperationResource {
                        cooperation_id: registration.cooperation_id.clone(),
                        project_id: event.project_id.clone(),
                        locator: registration.locator.clone(),
                        kind: registration.kind.clone(),
                        provenance: registration.provenance.clone(),
                        description: registration.description.clone(),
                        created_at: event.timestamp,
                        updated_at: event.timestamp,
                        revision: event.sequence,
                        availability: registration.availability.clone(),
                        fingerprint: registration.fingerprint.clone(),
                        discovered_project_id: registration.discovered_project_id.clone(),
                        declared_capabilities: registration.declared_capabilities.clone(),
                        observed_capabilities: vec![],
                        inferred_capabilities: vec![],
                        related_reference_ids: registration.related_reference_ids.clone(),
                        related_constraint_ids: registration.related_constraint_ids.clone(),
                    },
                );
            }
            DevelopmentEventPayload::CooperationResourceRefreshed { observation } => {
                if let Some(resource) = resources.get_mut(&observation.cooperation_id) {
                    resource.availability = observation.availability.clone();
                    resource.fingerprint = observation.fingerprint.clone();
                    resource.discovered_project_id = observation.discovered_project_id.clone();
                    resource.updated_at = observation.observed_at;
                    resource.revision = event.sequence;
                }
            }
            _ => {}
        }
    }
    resources
}

pub fn cooperation_resources(root: &Path) -> Result<Vec<CooperationResource>> {
    if crate::project_identity::load_identity(root)?.is_none() {
        return Ok(Vec::new());
    }
    let events = load_ledger(root)?.0.events;
    let mut resources = project_cooperation_resources(&events);
    for knowledge in project_cooperation_knowledge(&events, &resources) {
        if knowledge.stale {
            continue;
        }
        let Some(resource) = resources.get_mut(&knowledge.record.cooperation_id) else {
            continue;
        };
        let target = match knowledge.record.epistemic_status {
            EpistemicStatus::Declared => &mut resource.declared_capabilities,
            EpistemicStatus::Observed => &mut resource.observed_capabilities,
            EpistemicStatus::Inferred => &mut resource.inferred_capabilities,
            EpistemicStatus::Unknown => continue,
        };
        for capability in knowledge.record.capability_refs {
            if !target.contains(&capability) {
                target.push(capability);
            }
        }
    }
    Ok(resources.into_values().collect())
}

pub fn cooperation_resource(root: &Path, id: &str) -> Result<Option<CooperationResource>> {
    Ok(cooperation_resources(root)?
        .into_iter()
        .find(|resource| resource.cooperation_id == id))
}

fn project_cooperation_knowledge(
    events: &[crate::development::DevelopmentEvent],
    resources: &BTreeMap<String, CooperationResource>,
) -> Vec<CooperationKnowledge> {
    let mut knowledge = BTreeMap::<String, CooperationKnowledge>::new();
    for event in events {
        let record = match &event.payload {
            DevelopmentEventPayload::CooperationKnowledgeRecorded { record }
            | DevelopmentEventPayload::CooperationCapabilityObserved { record } => record,
            _ => continue,
        };
        if let Some(old) = record
            .supersedes
            .as_ref()
            .and_then(|id| knowledge.get_mut(id))
        {
            old.superseded_by = Some(record.knowledge_id.clone());
            old.stale = true;
        }
        let current_fingerprint = resources
            .get(&record.cooperation_id)
            .and_then(|resource| resource.fingerprint.as_ref())
            .map(|fingerprint| fingerprint.value.as_str());
        let stale = record
            .resource_fingerprint
            .as_deref()
            .is_some_and(|recorded| Some(recorded) != current_fingerprint);
        knowledge.insert(
            record.knowledge_id.clone(),
            CooperationKnowledge {
                record: record.clone(),
                project_id: event.project_id.clone(),
                revision: event.sequence,
                superseded_by: None,
                stale,
            },
        );
    }
    knowledge.into_values().collect()
}

pub fn cooperation_knowledge(
    root: &Path,
    cooperation_id: Option<&str>,
) -> Result<Vec<CooperationKnowledge>> {
    if crate::project_identity::load_identity(root)?.is_none() {
        return Ok(Vec::new());
    }
    let events = load_ledger(root)?.0.events;
    let resources = project_cooperation_resources(&events);
    Ok(project_cooperation_knowledge(&events, &resources)
        .into_iter()
        .filter(|item| cooperation_id.is_none_or(|id| item.record.cooperation_id == id))
        .collect())
}

pub fn cooperation_discoveries(
    root: &Path,
    cooperation_id: Option<&str>,
) -> Result<Vec<CooperationDiscovery>> {
    if crate::project_identity::load_identity(root)?.is_none() {
        return Ok(Vec::new());
    }
    let events = load_ledger(root)?.0.events;
    let mut discoveries = BTreeMap::<String, CooperationDiscovery>::new();
    for event in events {
        if let DevelopmentEventPayload::CooperationDiscoveryUpdated { record } = event.payload {
            if cooperation_id.is_none_or(|id| record.cooperation_id == id) {
                discoveries.insert(
                    record.discovery_id.clone(),
                    CooperationDiscovery {
                        record,
                        project_id: event.project_id,
                        revision: event.sequence,
                    },
                );
            }
        }
    }
    Ok(discoveries.into_values().collect())
}

pub(crate) fn cooperation_summaries(
    events: &[crate::development::DevelopmentEvent],
    limit: usize,
) -> Result<(
    Vec<CooperationResourceSummary>,
    Vec<CooperationKnowledgeSummary>,
)> {
    let resources = project_cooperation_resources(events);
    let mut knowledge = project_cooperation_knowledge(events, &resources);
    let resources: Vec<_> = resources.into_values().collect();
    knowledge.sort_by_key(|item| item.revision);
    let knowledge = knowledge
        .into_iter()
        .rev()
        .take(limit.min(100))
        .map(|item| CooperationKnowledgeSummary {
            knowledge_id: item.record.knowledge_id,
            cooperation_id: item.record.cooperation_id,
            epistemic_status: item.record.epistemic_status,
            revision: item.revision,
            stale: item.stale,
        })
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let resources = resources
        .into_iter()
        .take(100)
        .map(|resource| CooperationResourceSummary {
            cooperation_id: resource.cooperation_id,
            kind: resource.kind,
            availability: resource.availability,
            revision: resource.revision,
        })
        .collect();
    Ok((resources, knowledge))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::development::{
        query_development_events, register_worker, send_worker_message, WorkerMessageInput,
        WorkerMessageType, WorkerMetadata,
    };
    use crate::execution::EvidenceStore;
    use tempfile::tempdir;

    fn kind(value: &str) -> CooperationKind {
        CooperationKind::new(value).unwrap()
    }

    fn draft(payload: DevelopmentEventPayload) -> DevelopmentEventDraft {
        serde_json::from_value(serde_json::json!({"payload": payload})).unwrap()
    }

    #[test]
    fn observed_policy_rejects_every_ingress_and_unrelated_evidence() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("tool"), b"fixture").unwrap();
        register(root.path(), "tool", "tool", "DOCUMENT", "reg");
        let valid = knowledge(
            root.path(),
            "valid",
            "tool",
            EpistemicStatus::Observed,
            None,
            None,
        );
        let original_store = crate::execution::EvidenceStore::load(root.path()).unwrap();
        let before = load_ledger(root.path()).unwrap().0.events.len();
        for case in [
            "missing",
            "message",
            "wrong-project",
            "wrong-resource",
            "agent",
            "failed",
            "stale",
        ] {
            let mut record = valid.clone();
            let mut store = original_store.clone();
            match case {
                "missing" => record.evidence_refs.clear(),
                "message" => record.evidence_refs = vec!["worker-message-not-evidence".into()],
                "wrong-project" => {
                    store.evidence[0]
                        .metadata
                        .insert("project_id".into(), "other".into());
                }
                "wrong-resource" => {
                    store.evidence[0]
                        .metadata
                        .insert("cooperation_id".into(), "other".into());
                }
                "agent" => store.evidence[0].source = crate::execution::EvidenceSource::Agent,
                "failed" => store.evidence[0].kind = crate::execution::EvidenceKind::CheckFail,
                "stale" => record.resource_fingerprint = Some("old".into()),
                _ => unreachable!(),
            }
            store.save(root.path()).unwrap();
            assert!(
                record_cooperation_knowledge(root.path(), record.clone(), None, None).is_err(),
                "{case}"
            );
            assert!(
                record_cooperation_capability(root.path(), record.clone(), None, None).is_err(),
                "{case}"
            );
            for payload in [
                DevelopmentEventPayload::CooperationKnowledgeRecorded {
                    record: record.clone(),
                },
                DevelopmentEventPayload::CooperationCapabilityObserved { record },
            ] {
                assert!(
                    append_development_event(root.path(), draft(payload)).is_err(),
                    "{case}"
                );
            }
        }
        assert_eq!(load_ledger(root.path()).unwrap().0.events.len(), before);
        original_store.save(root.path()).unwrap();
        record_cooperation_capability(root.path(), valid, None, Some("valid".into())).unwrap();
        assert_eq!(
            cooperation_resource(root.path(), "tool")
                .unwrap()
                .unwrap()
                .observed_capabilities,
            ["test.read"]
        );
    }

    #[test]
    fn conflicts_and_concurrent_supersession_are_serialized() {
        let root = tempdir().unwrap();
        register(root.path(), "one", "opaque:one", "UNKNOWN", "r1");
        register(root.path(), "two", "opaque:two", "UNKNOWN", "r2");
        let registration = load_ledger(root.path()).unwrap().0.events[0]
            .payload
            .clone();
        assert!(append_development_event(root.path(), draft(registration)).is_err());
        let old = knowledge(
            root.path(),
            "old",
            "one",
            EpistemicStatus::Declared,
            None,
            None,
        );
        record_cooperation_knowledge(root.path(), old.clone(), None, Some("old".into())).unwrap();
        assert!(record_cooperation_knowledge(root.path(), old, None, None).is_err());
        for (resource, target) in [("one", "missing"), ("two", "old")] {
            let record = knowledge(
                root.path(),
                "bad",
                resource,
                EpistemicStatus::Declared,
                None,
                Some(target.into()),
            );
            assert!(record_cooperation_knowledge(root.path(), record, None, None).is_err());
        }
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|i| {
                let path = root.path().to_owned();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let record = knowledge(
                        &path,
                        &format!("next-{i}"),
                        "one",
                        EpistemicStatus::Declared,
                        None,
                        Some("old".into()),
                    );
                    barrier.wait();
                    record_cooperation_knowledge(&path, record, None, Some(format!("next-{i}")))
                        .is_ok()
                })
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .filter(|h| h.thread().id() != std::thread::current().id())
                .map(|h| usize::from(h.join().unwrap()))
                .sum::<usize>(),
            1
        );
    }

    #[test]
    fn inspection_of_fresh_project_does_not_write() {
        let root = tempdir().unwrap();
        assert!(cooperation_resources(root.path()).unwrap().is_empty());
        assert!(cooperation_knowledge(root.path(), None).unwrap().is_empty());
        assert!(cooperation_discoveries(root.path(), None)
            .unwrap()
            .is_empty());
        assert!(crate::constitutive::ReferenceRegistry::read(root.path())
            .unwrap()
            .entries
            .is_empty());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }

    #[test]
    fn inspection_never_rebinds_an_existing_copied_identity() {
        let root = tempdir().unwrap();
        let mut identity = crate::project_identity::ensure_identity(root.path()).unwrap();
        identity.bound_root = Some("old-location".into());
        let path = crate::project_identity::identity_path(root.path());
        let bytes = serde_json::to_vec(&identity).unwrap();
        fs::write(&path, &bytes).unwrap();
        cooperation_resources(root.path()).unwrap();
        cooperation_knowledge(root.path(), None).unwrap();
        cooperation_discoveries(root.path(), None).unwrap();
        crate::constitutive::ReferenceRegistry::read(root.path()).unwrap();
        assert_eq!(fs::read(path).unwrap(), bytes);
        assert_eq!(fs::read_dir(root.path().join(".route")).unwrap().count(), 1);
    }

    #[test]
    fn corrupt_evidence_is_not_reinitialized_or_accepted() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("tool"), b"fixture").unwrap();
        register(root.path(), "tool", "tool", "DOCUMENT", "reg");
        let record = knowledge(
            root.path(),
            "k",
            "tool",
            EpistemicStatus::Observed,
            None,
            None,
        );
        let path = root.path().join(".route/execution/evidence.json");
        assert!(path.exists());
        for bytes in ["", "{", "not-json"] {
            fs::write(&path, bytes).unwrap();
            assert!(crate::execution::EvidenceStore::load(root.path()).is_err());
            assert!(record_cooperation_knowledge(root.path(), record.clone(), None, None).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn refresh_retry_is_one_operation_even_when_sample_changes() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("tool"), b"first").unwrap();
        register(root.path(), "tool", "tool", "DOCUMENT", "reg");
        let first = refresh_cooperation_resource(root.path(), "tool", None, Some("refresh".into()))
            .unwrap();
        fs::write(root.path().join("tool"), b"changed").unwrap();
        let replay =
            refresh_cooperation_resource(root.path(), "tool", None, Some("refresh".into()))
                .unwrap();
        assert!(replay.replay);
        assert_eq!(first.event.event_id, replay.event.event_id);
        let next =
            refresh_cooperation_resource(root.path(), "tool", None, Some("new-refresh".into()))
                .unwrap();
        assert_eq!(next.global_revision, first.global_revision + 1);
    }

    fn register(root: &Path, id: &str, locator: &str, resource_kind: &str, key: &str) {
        register_cooperation_resource(
            root,
            id.into(),
            locator.into(),
            kind(resource_kind),
            "LOCAL_OBSERVED".into(),
            Some("bounded test resource".into()),
            vec![],
            vec![],
            vec![],
            None,
            Some(key.into()),
        )
        .unwrap();
    }

    fn knowledge(
        root: &Path,
        id: &str,
        cooperation_id: &str,
        status: EpistemicStatus,
        fingerprint: Option<String>,
        supersedes: Option<String>,
    ) -> CooperationKnowledgeRecord {
        let fingerprint = if status == EpistemicStatus::Observed {
            fingerprint.or_else(|| {
                cooperation_resource(root, cooperation_id)
                    .unwrap()
                    .unwrap()
                    .fingerprint
                    .map(|f| f.value)
            })
        } else {
            fingerprint
        };
        let evidence_refs = if status == EpistemicStatus::Observed {
            let project_id = crate::project_identity::load_identity(root)
                .unwrap()
                .unwrap()
                .project_id;
            let mut store = crate::execution::EvidenceStore::load(root).unwrap();
            let id = store.record(
                "fixture",
                crate::execution::EvidenceKind::CheckPass,
                crate::execution::EvidenceSource::System,
                route_core::sha256_hex(b"fixture observation"),
                std::collections::HashMap::from([
                    ("project_id".into(), project_id),
                    ("cooperation_id".into(), cooperation_id.into()),
                    ("resource_fingerprint".into(), fingerprint.clone().unwrap()),
                ]),
            );
            store.save(root).unwrap();
            vec![id]
        } else {
            vec![]
        };
        CooperationKnowledgeRecord {
            knowledge_id: id.into(),
            cooperation_id: cooperation_id.into(),
            statement: "This resource has a bounded test capability.".into(),
            capability_refs: vec!["test.read".into()],
            epistemic_status: status,
            provenance: "worker observation".into(),
            source_refs: vec![],
            evidence_refs,
            observed_at: route_core::now_millis(),
            resource_fingerprint: fingerprint,
            supersedes,
        }
    }

    #[test]
    fn unknown_and_future_kinds_are_valid_and_uri_is_not_contacted() {
        let root = tempdir().unwrap();
        register(
            root.path(),
            "unknown",
            "https://invalid.invalid/resource",
            "UNKNOWN",
            "u",
        );
        register(
            root.path(),
            "future",
            "opaque:locator",
            "FUTURE_KIND_V9",
            "f",
        );
        let resources = cooperation_resources(root.path()).unwrap();
        assert_eq!(resources.len(), 2);
        assert!(resources
            .iter()
            .all(|resource| resource.availability == CooperationAvailability::Unknown));
        assert_eq!(resources[0].kind.0, "FUTURE_KIND_V9");
    }

    #[test]
    fn traversal_is_rejected_and_directory_fingerprint_is_non_recursive() {
        let root = tempdir().unwrap();
        assert!(register_cooperation_resource(
            root.path(),
            "escape".into(),
            "../outside".into(),
            kind("FILE"),
            "test".into(),
            None,
            vec![],
            vec![],
            vec![],
            None,
            Some("escape".into()),
        )
        .is_err());

        fs::create_dir(root.path().join("data")).unwrap();
        fs::write(root.path().join("data/large-child"), vec![1_u8; 2_000_000]).unwrap();
        register(root.path(), "dir", "data", "DIRECTORY", "dir-register");
        let before = cooperation_resource(root.path(), "dir")
            .unwrap()
            .unwrap()
            .fingerprint;
        fs::write(root.path().join("data/large-child"), vec![2_u8; 2_000_000]).unwrap();
        refresh_cooperation_resource(root.path(), "dir", None, Some("dir-refresh".into())).unwrap();
        let after = cooperation_resource(root.path(), "dir")
            .unwrap()
            .unwrap()
            .fingerprint;
        assert_eq!(before, after, "directory contents must not be traversed");
    }

    #[test]
    fn missing_resource_stales_fingerprint_bound_knowledge() {
        let root = tempdir().unwrap();
        let file = root.path().join("resource.txt");
        fs::write(&file, b"safe fixture").unwrap();
        register(
            root.path(),
            "resource",
            "resource.txt",
            "DOCUMENT",
            "register",
        );
        let fingerprint = cooperation_resource(root.path(), "resource")
            .unwrap()
            .unwrap()
            .fingerprint
            .unwrap()
            .value;
        record_cooperation_knowledge(
            root.path(),
            knowledge(
                root.path(),
                "k",
                "resource",
                EpistemicStatus::Declared,
                Some(fingerprint),
                None,
            ),
            None,
            Some("knowledge".into()),
        )
        .unwrap();
        assert!(!cooperation_knowledge(root.path(), Some("resource")).unwrap()[0].stale);
        fs::remove_file(&file).unwrap();
        refresh_cooperation_resource(root.path(), "resource", None, Some("missing".into()))
            .unwrap();
        assert!(cooperation_knowledge(root.path(), Some("resource")).unwrap()[0].stale);
        let resource = cooperation_resource(root.path(), "resource")
            .unwrap()
            .unwrap();
        assert_eq!(resource.availability, CooperationAvailability::Missing);
        assert!(resource.declared_capabilities.is_empty());
    }

    #[test]
    fn large_file_uses_bounded_fingerprint_and_change_stales_old_knowledge() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("large.bin"), vec![7_u8; 2_000_000]).unwrap();
        register(
            root.path(),
            "large",
            "large.bin",
            "DATASET",
            "large-register",
        );
        let resource = cooperation_resource(root.path(), "large").unwrap().unwrap();
        let fingerprint = resource.fingerprint.unwrap();
        assert_eq!(fingerprint.algorithm, "SHA256_BOUNDED_HEAD_TAIL_MTIME_V2");
        record_cooperation_knowledge(
            root.path(),
            knowledge(
                root.path(),
                "k1",
                "large",
                EpistemicStatus::Observed,
                Some(fingerprint.value),
                None,
            ),
            Some("worker-a".into()),
            Some("knowledge-1".into()),
        )
        .unwrap();
        fs::write(root.path().join("large.bin"), vec![8_u8; 2_000_000]).unwrap();
        refresh_cooperation_resource(root.path(), "large", None, Some("large-refresh".into()))
            .unwrap();
        assert!(cooperation_knowledge(root.path(), Some("large")).unwrap()[0].stale);
    }

    #[test]
    fn supersession_and_epistemic_classes_remain_distinct() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("tool.txt"), b"safe").unwrap();
        register(root.path(), "tool", "tool.txt", "DOCUMENT", "tool-register");
        for (index, status) in [
            EpistemicStatus::Declared,
            EpistemicStatus::Observed,
            EpistemicStatus::Inferred,
            EpistemicStatus::Unknown,
        ]
        .into_iter()
        .enumerate()
        {
            record_cooperation_knowledge(
                root.path(),
                knowledge(
                    root.path(),
                    &format!("k{index}"),
                    "tool",
                    status,
                    None,
                    None,
                ),
                None,
                Some(format!("knowledge-{index}")),
            )
            .unwrap();
        }
        record_cooperation_knowledge(
            root.path(),
            knowledge(
                root.path(),
                "replacement",
                "tool",
                EpistemicStatus::Declared,
                None,
                Some("k0".into()),
            ),
            None,
            Some("replacement".into()),
        )
        .unwrap();
        let all = cooperation_knowledge(root.path(), Some("tool")).unwrap();
        assert_eq!(all.len(), 5);
        let old = all
            .iter()
            .find(|item| item.record.knowledge_id == "k0")
            .unwrap();
        assert!(old.stale);
        assert_eq!(old.superseded_by.as_deref(), Some("replacement"));
    }

    #[test]
    fn worker_learning_advances_global_revision_without_ontology_collapse() {
        let root = tempdir().unwrap();
        register_worker(
            root.path(),
            Some("worker-a".into()),
            WorkerMetadata::default(),
            Some("worker-a".into()),
        )
        .unwrap();
        register_worker(
            root.path(),
            Some("worker-b".into()),
            WorkerMetadata::default(),
            Some("worker-b".into()),
        )
        .unwrap();
        register(root.path(), "x", "opaque:x", "UNKNOWN", "register-x");
        let seen_by_b = 3;
        send_worker_message(
            root.path(),
            "worker-a",
            WorkerMessageInput {
                message_id: Some("question".into()),
                target_worker: Some("worker-b".into()),
                intent_ref: None,
                task_ref: None,
                thread_ref: None,
                message_type: WorkerMessageType::HelpRequest,
                content: "What is x?".into(),
                source_refs: vec![],
                requires_response: true,
            },
            Some("question".into()),
        )
        .unwrap();
        let evidence_before = EvidenceStore::load(root.path()).unwrap().evidence.len();
        record_cooperation_knowledge(
            root.path(),
            knowledge(
                root.path(),
                "shared",
                "x",
                EpistemicStatus::Declared,
                None,
                None,
            ),
            Some("worker-a".into()),
            Some("shared".into()),
        )
        .unwrap();
        let delta = query_development_events(root.path(), seen_by_b, 20).unwrap();
        assert_eq!(delta.global_revision, 5);
        assert!(delta.events.iter().any(|event| matches!(
            event.payload,
            DevelopmentEventPayload::CooperationKnowledgeRecorded { .. }
        )));
        assert!(delta
            .events
            .iter()
            .any(|event| matches!(event.payload, DevelopmentEventPayload::WorkerMessage { .. })));
        assert_eq!(
            EvidenceStore::load(root.path()).unwrap().evidence.len(),
            evidence_before
        );
        assert_eq!(
            cooperation_knowledge(root.path(), Some("x")).unwrap().len(),
            1
        );
    }

    #[test]
    fn project_registration_is_read_only_and_projects_remain_isolated() {
        let host = tempdir().unwrap();
        let peer = tempdir().unwrap();
        register(
            host.path(),
            "generic-peer",
            peer.path().to_str().unwrap(),
            "PROJECT",
            "peer-generic",
        );
        assert!(!peer.path().join(".route").exists());
        assert!(cooperation_resource(host.path(), "generic-peer")
            .unwrap()
            .unwrap()
            .discovered_project_id
            .is_none());

        let peer_identity = crate::project_identity::ensure_identity(peer.path()).unwrap();
        let identity_path = peer.path().join(".route/project-identity.json");
        let before = fs::read(&identity_path).unwrap();
        register(
            host.path(),
            "route-peer",
            peer.path().to_str().unwrap(),
            "PROJECT",
            "peer-route",
        );
        assert_eq!(fs::read(&identity_path).unwrap(), before);
        assert_eq!(
            cooperation_resource(host.path(), "route-peer")
                .unwrap()
                .unwrap()
                .discovered_project_id
                .as_deref(),
            Some(peer_identity.project_id.as_str())
        );
        assert!(cooperation_resources(peer.path()).unwrap().is_empty());
    }

    #[test]
    fn concurrent_resources_are_totally_ordered_and_corruption_fails_closed() {
        let root = tempdir().unwrap();
        let root = std::sync::Arc::new(root.path().to_path_buf());
        let workers = (0..8)
            .map(|index| {
                let root = root.clone();
                std::thread::spawn(move || {
                    register(
                        &root,
                        &format!("resource-{index}"),
                        &format!("opaque:{index}"),
                        "UNKNOWN",
                        &format!("resource-{index}"),
                    );
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().unwrap();
        }
        let events = query_development_events(&root, 0, 20).unwrap();
        assert_eq!(events.global_revision, 8);
        assert!(events
            .events
            .iter()
            .enumerate()
            .all(|(index, event)| event.sequence == index as u64 + 1));
        fs::write(
            crate::development::development_ledger_path(&root).unwrap(),
            b"{broken",
        )
        .unwrap();
        assert!(cooperation_resources(&root).is_err());
    }
}
