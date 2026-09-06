//! Open Development Substrate: project-scoped events, revisions and workers.
//!
//! This module adds atomic development observations without replacing Route's
//! existing ExecutionSession, Evidence, Git or locked command-history stores.
//! The ledger is append-only through this API and is atomically replaced under
//! a cross-process lock so readers observe a complete revision.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::cooperation;
use crate::cooperation::*;
use crate::execution::{Evidence, EvidenceStore, ExecutionSession, SessionStatus, SessionStore};
use crate::game_save::KnownGoodTracker;
use crate::project_identity::{ensure_identity, load_identity, ProjectIdentity};

const LEGACY_LEDGER_SCHEMA: u8 = 1;
const LEDGER_SCHEMA: u8 = 2;
const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_SUMMARY: usize = 2_000;
const MAX_MESSAGE: usize = 4_000;
const MAX_ACTIVITY: usize = 512;
const MAX_EVENTS_PER_QUERY: usize = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DevelopmentEventType {
    WorkerLifecycle,
    WorkerMetadata,
    WorkerPresence,
    WorkerMessage,
    TaskActivity,
    Finding,
    DevelopmentAction,
    EvidenceReference,
    ProjectStateObservation,
    HumanIntervention,
    CooperationResource,
    CooperationKnowledge,
    CooperationDiscovery,
    Reference,
    CooperationCapability,
    HumanCooperationGuidance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerStatus {
    Active,
    Idle,
    Away,
    Disconnected,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkerMessageType {
    Question,
    Answer,
    Notice,
    HelpRequest,
    HelpOffer,
    Warning,
    Proposal,
    Disagreement,
    ReviewRequest,
    ReviewFinding,
    Handoff,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default)]
    pub capabilities: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerDescriptor {
    pub worker_id: String,
    pub created_at: i64,
    #[serde(flatten)]
    pub metadata: WorkerMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerPresence {
    pub worker_id: String,
    pub status: WorkerStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_intent_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_task_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_session_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_workspace_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_activity_summary: Option<String>,
    pub observed_global_revision: u64,
    pub last_seen: i64,
    pub last_event_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerPresenceInput {
    pub status: WorkerStatus,
    #[serde(default)]
    pub current_intent_ref: Option<String>,
    #[serde(default)]
    pub current_task_ref: Option<String>,
    #[serde(default)]
    pub current_session_ref: Option<String>,
    #[serde(default)]
    pub current_workspace_ref: Option<String>,
    #[serde(default)]
    pub current_activity_summary: Option<String>,
    pub observed_global_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerMessage {
    pub message_id: String,
    pub from_worker: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_worker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_ref: Option<String>,
    pub message_type: WorkerMessageType,
    pub content: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub requires_response: bool,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkerMessageInput {
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub target_worker: Option<String>,
    #[serde(default)]
    pub intent_ref: Option<String>,
    #[serde(default)]
    pub task_ref: Option<String>,
    #[serde(default)]
    pub thread_ref: Option<String>,
    pub message_type: WorkerMessageType,
    pub content: String,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub requires_response: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    deny_unknown_fields,
    tag = "kind",
    content = "data",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum DevelopmentEventPayload {
    WorkerRegistered {
        descriptor: WorkerDescriptor,
    },
    WorkerMetadataUpdated {
        worker_id: String,
        metadata: WorkerMetadata,
    },
    WorkerPresenceUpdated {
        presence: WorkerPresenceInput,
    },
    WorkerMessage {
        message: WorkerMessage,
    },
    TaskActivity {
        status: String,
        summary: String,
    },
    Finding {
        summary: String,
        #[serde(default)]
        source_refs: Vec<String>,
    },
    DevelopmentAction {
        action: String,
        summary: String,
    },
    EvidenceReferenced {
        summary: String,
        evidence_refs: Vec<String>,
    },
    ProjectStateObserved {
        summary: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        git_head: Option<String>,
    },
    HumanIntervention {
        summary: String,
    },
    CooperationResourceRegistered {
        registration: CooperationResourceRegistration,
    },
    CooperationResourceRefreshed {
        observation: CooperationRefreshObservation,
    },
    CooperationKnowledgeRecorded {
        record: CooperationKnowledgeRecord,
    },
    CooperationDiscoveryUpdated {
        record: CooperationDiscoveryRecord,
    },
    ReferenceRegistered {
        observation: ReferenceCommonsObservation,
    },
    ReferenceRegistryCommitted {
        operation_id: String,
        registry_hash: String,
    },
    ReferenceRefreshed {
        observation: ReferenceCommonsObservation,
    },
    ReferenceChanged {
        observation: ReferenceCommonsObservation,
    },
    ReferenceMissing {
        observation: ReferenceCommonsObservation,
    },
    CooperationCapabilityObserved {
        record: CooperationKnowledgeRecord,
    },
    HumanCooperationGuidance {
        guidance: HumanCooperationGuidance,
    },
}

impl DevelopmentEventPayload {
    pub fn event_type(&self) -> DevelopmentEventType {
        match self {
            Self::WorkerRegistered { .. } => DevelopmentEventType::WorkerLifecycle,
            Self::WorkerMetadataUpdated { .. } => DevelopmentEventType::WorkerMetadata,
            Self::WorkerPresenceUpdated { .. } => DevelopmentEventType::WorkerPresence,
            Self::WorkerMessage { .. } => DevelopmentEventType::WorkerMessage,
            Self::TaskActivity { .. } => DevelopmentEventType::TaskActivity,
            Self::Finding { .. } => DevelopmentEventType::Finding,
            Self::DevelopmentAction { .. } => DevelopmentEventType::DevelopmentAction,
            Self::EvidenceReferenced { .. } => DevelopmentEventType::EvidenceReference,
            Self::ProjectStateObserved { .. } => DevelopmentEventType::ProjectStateObservation,
            Self::HumanIntervention { .. } => DevelopmentEventType::HumanIntervention,
            Self::CooperationResourceRegistered { .. }
            | Self::CooperationResourceRefreshed { .. } => {
                DevelopmentEventType::CooperationResource
            }
            Self::CooperationKnowledgeRecorded { .. } => DevelopmentEventType::CooperationKnowledge,
            Self::CooperationDiscoveryUpdated { .. } => DevelopmentEventType::CooperationDiscovery,
            Self::ReferenceRegistered { .. }
            | Self::ReferenceRegistryCommitted { .. }
            | Self::ReferenceRefreshed { .. }
            | Self::ReferenceChanged { .. }
            | Self::ReferenceMissing { .. } => DevelopmentEventType::Reference,
            Self::CooperationCapabilityObserved { .. } => {
                DevelopmentEventType::CooperationCapability
            }
            Self::HumanCooperationGuidance { .. } => DevelopmentEventType::HumanCooperationGuidance,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentEvent {
    pub event_id: String,
    pub project_id: String,
    pub sequence: u64,
    pub timestamp: i64,
    pub event_type: DevelopmentEventType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor_worker_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution_session_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub payload: DevelopmentEventPayload,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deduplication_key: Option<String>,
    pub previous_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DevelopmentEventDraft {
    #[serde(default)]
    pub event_id: Option<String>,
    #[serde(default)]
    pub actor_worker_id: Option<String>,
    #[serde(default)]
    pub execution_session_ref: Option<String>,
    #[serde(default)]
    pub intent_ref: Option<String>,
    #[serde(default)]
    pub task_ref: Option<String>,
    #[serde(default)]
    pub workspace_ref: Option<String>,
    #[serde(default)]
    pub causation_id: Option<String>,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    pub payload: DevelopmentEventPayload,
    #[serde(default)]
    pub deduplication_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppendDevelopmentEventResult {
    pub event: DevelopmentEvent,
    pub replay: bool,
    pub global_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevelopmentEventPage {
    pub project_id: String,
    pub after_revision: u64,
    pub global_revision: u64,
    pub events: Vec<DevelopmentEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct DevelopmentLedger {
    schema: u8,
    project_id: String,
    head_hash: String,
    pub(crate) events: Vec<DevelopmentEvent>,
}

impl DevelopmentLedger {
    fn empty(project_id: String) -> Self {
        Self {
            schema: LEDGER_SCHEMA,
            project_id,
            head_hash: GENESIS_HASH.to_string(),
            events: Vec::new(),
        }
    }

    fn revision(&self) -> u64 {
        self.events.last().map(|event| event.sequence).unwrap_or(0)
    }

    fn verify(&self, expected_project_id: &str) -> Result<()> {
        if !matches!(self.schema, LEGACY_LEDGER_SCHEMA | LEDGER_SCHEMA) {
            bail!("unsupported development ledger schema {}", self.schema);
        }
        if self.project_id != expected_project_id {
            bail!("development ledger project identity mismatch");
        }
        let mut previous_hash = GENESIS_HASH.to_string();
        let mut event_ids = BTreeSet::new();
        let mut deduplication_keys = BTreeSet::new();
        for (index, event) in self.events.iter().enumerate() {
            let sequence = index as u64 + 1;
            if event.project_id != expected_project_id || event.sequence != sequence {
                bail!("development ledger sequence/project mismatch at {sequence}");
            }
            if event.previous_hash != previous_hash {
                bail!("development ledger hash chain is broken at {sequence}");
            }
            if !event_ids.insert(event.event_id.clone()) {
                bail!("duplicate development event id {}", event.event_id);
            }
            if let Some(key) = &event.deduplication_key {
                if !deduplication_keys.insert(key.clone()) {
                    bail!("duplicate development event deduplication key");
                }
            }
            let expected = event_hash(event)?;
            if event.hash != expected {
                bail!("development event content hash mismatch at {sequence}");
            }
            previous_hash = event.hash.clone();
        }
        if self.head_hash != previous_hash {
            bail!("development ledger head does not match the event chain");
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct EventHashPayload<'a> {
    event_id: &'a str,
    project_id: &'a str,
    sequence: u64,
    timestamp: i64,
    event_type: &'a DevelopmentEventType,
    actor_worker_id: &'a Option<String>,
    execution_session_ref: &'a Option<String>,
    intent_ref: &'a Option<String>,
    task_ref: &'a Option<String>,
    workspace_ref: &'a Option<String>,
    causation_id: &'a Option<String>,
    correlation_id: &'a Option<String>,
    source_refs: &'a [String],
    evidence_refs: &'a [String],
    payload: &'a DevelopmentEventPayload,
    deduplication_key: &'a Option<String>,
    previous_hash: &'a str,
}

fn event_hash(event: &DevelopmentEvent) -> Result<String> {
    let payload = EventHashPayload {
        event_id: &event.event_id,
        project_id: &event.project_id,
        sequence: event.sequence,
        timestamp: event.timestamp,
        event_type: &event.event_type,
        actor_worker_id: &event.actor_worker_id,
        execution_session_ref: &event.execution_session_ref,
        intent_ref: &event.intent_ref,
        task_ref: &event.task_ref,
        workspace_ref: &event.workspace_ref,
        causation_id: &event.causation_id,
        correlation_id: &event.correlation_id,
        source_refs: &event.source_refs,
        evidence_refs: &event.evidence_refs,
        payload: &event.payload,
        deduplication_key: &event.deduplication_key,
        previous_hash: &event.previous_hash,
    };
    Ok(route_core::sha256_hex(&serde_json::to_vec(&payload)?))
}

pub fn development_state_owner(root: &Path) -> Result<(PathBuf, ProjectIdentity)> {
    let identity = if load_identity(root)?.is_some() {
        ensure_identity(root)?
    } else {
        let initialization_dir = root.join(".route").join("identity-initialization");
        fs::create_dir_all(&initialization_dir)?;
        let _lock = DevelopmentAppendLock::acquire(&initialization_dir)?;
        ensure_identity(root)?
    };
    let Some(attached_from) = identity.attached_from.as_deref() else {
        return Ok((root.to_path_buf(), identity));
    };
    let source = PathBuf::from(attached_from);
    let source_identity = load_identity(&source)?
        .ok_or_else(|| anyhow::anyhow!("attached development-state owner has no identity"))?;
    if source_identity.project_id != identity.project_id {
        bail!("attached development-state owner project identity mismatch");
    }
    Ok((source, identity))
}

fn development_dir(owner: &Path) -> PathBuf {
    owner.join(".route").join("development")
}

pub fn development_ledger_path(root: &Path) -> Result<PathBuf> {
    let (owner, _) = development_state_owner(root)?;
    Ok(development_dir(&owner).join("ledger.json"))
}

pub(crate) fn load_ledger(root: &Path) -> Result<(DevelopmentLedger, ProjectIdentity, PathBuf)> {
    let (owner, identity) = development_state_owner(root)?;
    load_ledger_at(owner, identity)
}

pub(crate) fn load_ledger_readonly(
    root: &Path,
) -> Result<(DevelopmentLedger, ProjectIdentity, PathBuf)> {
    let identity =
        load_identity(root)?.ok_or_else(|| anyhow::anyhow!("project identity is missing"))?;
    let owner = identity
        .attached_from
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.to_owned());
    let owner_identity = load_identity(&owner)?
        .ok_or_else(|| anyhow::anyhow!("ledger owner identity is missing"))?;
    if owner_identity.project_id != identity.project_id {
        bail!("ledger owner project mismatch");
    }
    load_ledger_at(owner, identity)
}

fn load_ledger_at(
    owner: PathBuf,
    identity: ProjectIdentity,
) -> Result<(DevelopmentLedger, ProjectIdentity, PathBuf)> {
    let path = development_dir(&owner).join("ledger.json");
    if !path.exists() {
        return Ok((
            DevelopmentLedger::empty(identity.project_id.clone()),
            identity,
            owner,
        ));
    }
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let ledger: DevelopmentLedger = serde_json::from_slice(&bytes)
        .with_context(|| format!("parsing development ledger at {}", path.display()))?;
    ledger.verify(&identity.project_id)?;
    Ok((ledger, identity, owner))
}

fn validate_identifier(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() || value.len() > 256 {
        bail!("{label} must contain 1..256 characters");
    }
    Ok(())
}

fn validate_text(label: &str, value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > max {
        bail!("{label} must contain 1..{max} characters");
    }
    Ok(())
}

fn validate_metadata(metadata: &WorkerMetadata) -> Result<()> {
    for value in [
        metadata.display_name.as_deref(),
        metadata.provider.as_deref(),
        metadata.model.as_deref(),
        metadata.host.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_text("worker metadata", value, 256)?;
    }
    if metadata.capabilities.len() > 128 {
        bail!("worker capabilities exceed 128 entries");
    }
    for (key, value) in &metadata.capabilities {
        validate_identifier("capability key", key)?;
        validate_text("capability value", value, 512)?;
        let normalized = key.to_ascii_lowercase().replace('-', "_");
        if normalized.contains("chain_of_thought")
            || normalized == "cot"
            || normalized.contains("raw_reasoning")
        {
            bail!("raw reasoning metadata is not permitted");
        }
    }
    Ok(())
}

fn validate_payload(payload: &DevelopmentEventPayload) -> Result<()> {
    match payload {
        DevelopmentEventPayload::WorkerRegistered { descriptor } => {
            validate_identifier("worker_id", &descriptor.worker_id)?;
            validate_metadata(&descriptor.metadata)?;
        }
        DevelopmentEventPayload::WorkerMetadataUpdated {
            worker_id,
            metadata,
        } => {
            validate_identifier("worker_id", worker_id)?;
            validate_metadata(metadata)?;
        }
        DevelopmentEventPayload::WorkerPresenceUpdated { presence } => {
            if let Some(summary) = &presence.current_activity_summary {
                validate_text("activity summary", summary, MAX_ACTIVITY)?;
            }
        }
        DevelopmentEventPayload::WorkerMessage { message } => {
            validate_identifier("message_id", &message.message_id)?;
            validate_identifier("from_worker", &message.from_worker)?;
            validate_text("message content", &message.content, MAX_MESSAGE)?;
        }
        DevelopmentEventPayload::TaskActivity { status, summary } => {
            validate_text("task status", status, 128)?;
            validate_text("task summary", summary, MAX_SUMMARY)?;
        }
        DevelopmentEventPayload::Finding { summary, .. }
        | DevelopmentEventPayload::EvidenceReferenced { summary, .. }
        | DevelopmentEventPayload::ProjectStateObserved { summary, .. }
        | DevelopmentEventPayload::HumanIntervention { summary } => {
            validate_text("event summary", summary, MAX_SUMMARY)?;
        }
        DevelopmentEventPayload::DevelopmentAction { action, summary } => {
            validate_text("development action", action, 128)?;
            validate_text("development summary", summary, MAX_SUMMARY)?;
        }
        DevelopmentEventPayload::CooperationResourceRegistered { registration } => {
            cooperation::validate_registration(registration)?;
        }
        DevelopmentEventPayload::CooperationResourceRefreshed { observation } => {
            cooperation::validate_refresh(observation)?;
        }
        DevelopmentEventPayload::CooperationKnowledgeRecorded { record } => {
            cooperation::validate_knowledge(record)?;
        }
        DevelopmentEventPayload::CooperationDiscoveryUpdated { record } => {
            cooperation::validate_discovery(record)?;
        }
        DevelopmentEventPayload::ReferenceRegistered { observation }
        | DevelopmentEventPayload::ReferenceRefreshed { observation }
        | DevelopmentEventPayload::ReferenceChanged { observation }
        | DevelopmentEventPayload::ReferenceMissing { observation } => {
            cooperation::validate_reference_observation(observation)?;
        }
        DevelopmentEventPayload::ReferenceRegistryCommitted {
            operation_id,
            registry_hash,
        } => {
            validate_identifier("Reference operation", operation_id)?;
            if registry_hash.len() != 64 {
                bail!("invalid Reference registry hash");
            }
        }
        DevelopmentEventPayload::CooperationCapabilityObserved { record } => {
            cooperation::validate_capability_observation(record)?;
        }
        DevelopmentEventPayload::HumanCooperationGuidance { guidance } => {
            cooperation::validate_human_guidance(guidance)?;
        }
    }
    Ok(())
}

fn same_semantics(event: &DevelopmentEvent, draft: &DevelopmentEventDraft) -> bool {
    let same_payload = match (&event.payload, &draft.payload) {
        (
            DevelopmentEventPayload::WorkerRegistered { descriptor: left },
            DevelopmentEventPayload::WorkerRegistered { descriptor: right },
        ) => left.worker_id == right.worker_id && left.metadata == right.metadata,
        (
            DevelopmentEventPayload::CooperationResourceRegistered { registration: left },
            DevelopmentEventPayload::CooperationResourceRegistered {
                registration: right,
            },
        ) => {
            // These three fields are sampled outcomes, not caller intent.
            let mut right = right.clone();
            right.availability = left.availability.clone();
            right.fingerprint = left.fingerprint.clone();
            right.discovered_project_id = left.discovered_project_id.clone();
            left == &right
        }
        (
            DevelopmentEventPayload::CooperationResourceRefreshed { observation: left },
            DevelopmentEventPayload::CooperationResourceRefreshed { observation: right },
        ) => left.cooperation_id == right.cooperation_id,
        (left, right) => left == right,
    };
    draft
        .event_id
        .as_ref()
        .is_none_or(|event_id| event_id == &event.event_id)
        && event.actor_worker_id == draft.actor_worker_id
        && event.execution_session_ref == draft.execution_session_ref
        && event.intent_ref == draft.intent_ref
        && event.task_ref == draft.task_ref
        && event.workspace_ref == draft.workspace_ref
        && event.causation_id == draft.causation_id
        && event.correlation_id == draft.correlation_id
        && event.source_refs == draft.source_refs
        && event.evidence_refs == draft.evidence_refs
        && same_payload
}

pub fn append_development_event(
    root: &Path,
    mut draft: DevelopmentEventDraft,
) -> Result<AppendDevelopmentEventResult> {
    validate_payload(&draft.payload)?;

    for value in [
        draft.event_id.as_deref(),
        draft.actor_worker_id.as_deref(),
        draft.execution_session_ref.as_deref(),
        draft.intent_ref.as_deref(),
        draft.task_ref.as_deref(),
        draft.workspace_ref.as_deref(),
        draft.causation_id.as_deref(),
        draft.correlation_id.as_deref(),
        draft.deduplication_key.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        validate_identifier("event reference", value)?;
    }

    let (owner, identity) = development_state_owner(root)?;
    let dir = development_dir(&owner);
    fs::create_dir_all(&dir)?;
    let _lock = DevelopmentAppendLock::acquire(&dir)?;
    let (mut ledger, current_identity, _) = load_ledger(root)?;
    if current_identity.project_id != identity.project_id {
        bail!("project identity changed while acquiring development ledger lock");
    }

    if draft.workspace_ref.is_none() {
        draft.workspace_ref = Some(identity.workspace_id.clone());
    }

    if let Some(key) = draft.deduplication_key.as_deref() {
        if let Some(existing) = ledger
            .events
            .iter()
            .find(|event| event.deduplication_key.as_deref() == Some(key))
        {
            if !same_semantics(existing, &draft) {
                bail!("development event idempotency conflict");
            }
            return Ok(AppendDevelopmentEventResult {
                event: existing.clone(),
                replay: true,
                global_revision: ledger.revision(),
            });
        }
    }
    if let Some(event_id) = draft.event_id.as_deref() {
        if let Some(existing) = ledger
            .events
            .iter()
            .find(|event| event.event_id == event_id)
        {
            if !same_semantics(existing, &draft) {
                bail!("development event id conflict");
            }
            return Ok(AppendDevelopmentEventResult {
                event: existing.clone(),
                replay: true,
                global_revision: ledger.revision(),
            });
        }
    }

    cooperation::validate_transition(&owner, &identity.project_id, &ledger.events, &draft.payload)?;
    if let DevelopmentEventPayload::WorkerRegistered { descriptor } = &draft.payload {
        let already_registered = ledger.events.iter().any(|event| {
            matches!(
                &event.payload,
                DevelopmentEventPayload::WorkerRegistered { descriptor: existing }
                    if existing.worker_id == descriptor.worker_id
            )
        });
        if already_registered {
            draft.payload = DevelopmentEventPayload::WorkerMetadataUpdated {
                worker_id: descriptor.worker_id.clone(),
                metadata: descriptor.metadata.clone(),
            };
        }
    }
    if let DevelopmentEventPayload::WorkerPresenceUpdated { presence } = &draft.payload {
        if presence.observed_global_revision > ledger.revision() {
            bail!("observed_global_revision cannot exceed the current global revision");
        }
    }
    let sequence = ledger.revision() + 1;
    let mut event = DevelopmentEvent {
        event_id: draft
            .event_id
            .unwrap_or_else(|| format!("evt_{}", route_core::new_id())),
        project_id: identity.project_id.clone(),
        sequence,
        timestamp: route_core::now_millis(),
        event_type: draft.payload.event_type(),
        actor_worker_id: draft.actor_worker_id,
        execution_session_ref: draft.execution_session_ref,
        intent_ref: draft.intent_ref,
        task_ref: draft.task_ref,
        workspace_ref: draft.workspace_ref,
        causation_id: draft.causation_id,
        correlation_id: draft.correlation_id,
        source_refs: draft.source_refs,
        evidence_refs: draft.evidence_refs,
        payload: draft.payload,
        deduplication_key: draft.deduplication_key,
        previous_hash: ledger.head_hash.clone(),
        hash: String::new(),
    };
    event.hash = event_hash(&event)?;
    ledger.head_hash = event.hash.clone();
    ledger.events.push(event.clone());
    // Schema 2 adds typed cooperation payloads. Existing schema-1 events keep
    // their exact bytes and hashes; the envelope upgrades only when a new
    // event is successfully committed.
    ledger.schema = LEDGER_SCHEMA;
    ledger.verify(&identity.project_id)?;
    crate::constitutive::write_atomic(
        &dir.join("ledger.json"),
        &serde_json::to_vec_pretty(&ledger)?,
    )?;
    Ok(AppendDevelopmentEventResult {
        event,
        replay: false,
        global_revision: sequence,
    })
}

/// Deterministic benchmark fixture, unavailable in ordinary production builds.
/// Uses the canonical event hash and verifier; refuses any nonempty ledger.
#[cfg(feature = "test-utils")]
pub fn seed_benchmark_events(root: &Path, count: usize) -> Result<()> {
    anyhow::ensure!(count <= 10_000, "benchmark fixture limit exceeded");
    let (mut ledger, identity, owner) = load_ledger(root)?;
    anyhow::ensure!(
        ledger.events.is_empty(),
        "benchmark requires an empty ledger"
    );
    for index in 0..count {
        let payload = DevelopmentEventPayload::Finding {
            summary: format!("bounded benchmark finding {index}"),
            source_refs: vec![],
        };
        let mut event = DevelopmentEvent {
            event_id: format!("benchmark-{index}"),
            project_id: identity.project_id.clone(),
            sequence: index as u64 + 1,
            timestamp: 0,
            event_type: payload.event_type(),
            actor_worker_id: None,
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: vec![],
            evidence_refs: vec![],
            payload,
            deduplication_key: None,
            previous_hash: ledger.head_hash.clone(),
            hash: String::new(),
        };
        event.hash = event_hash(&event)?;
        ledger.head_hash = event.hash.clone();
        ledger.events.push(event);
    }
    ledger.verify(&identity.project_id)?;
    let dir = development_dir(&owner);
    fs::create_dir_all(&dir)?;
    crate::constitutive::write_atomic(
        &dir.join("ledger.json"),
        &serde_json::to_vec_pretty(&ledger)?,
    )
}

pub fn query_development_events(
    root: &Path,
    after_revision: u64,
    limit: usize,
) -> Result<DevelopmentEventPage> {
    if limit == 0 || limit > MAX_EVENTS_PER_QUERY {
        bail!("event query limit must be between 1 and {MAX_EVENTS_PER_QUERY}");
    }
    let (ledger, identity, _) = load_ledger_readonly(root)?;
    let global_revision = ledger.revision();
    let events = ledger
        .events
        .into_iter()
        .filter(|event| event.sequence > after_revision)
        .take(limit)
        .collect();
    Ok(DevelopmentEventPage {
        project_id: identity.project_id,
        after_revision,
        global_revision,
        events,
    })
}

pub fn global_development_revision(root: &Path) -> Result<u64> {
    Ok(load_ledger_readonly(root)?.0.revision())
}

pub fn development_view_is_stale(seen_revision: u64, global_revision: u64) -> bool {
    seen_revision < global_revision
}

pub fn worker_descriptors(root: &Path) -> Result<Vec<WorkerDescriptor>> {
    Ok(project_worker_descriptors(
        &load_ledger_readonly(root)?.0.events,
    ))
}
fn project_worker_descriptors(events: &[DevelopmentEvent]) -> Vec<WorkerDescriptor> {
    let mut workers = BTreeMap::<String, WorkerDescriptor>::new();
    for event in events {
        match &event.payload {
            DevelopmentEventPayload::WorkerRegistered { descriptor } => {
                workers
                    .entry(descriptor.worker_id.clone())
                    .or_insert_with(|| descriptor.clone());
            }
            DevelopmentEventPayload::WorkerMetadataUpdated {
                worker_id,
                metadata,
            } => {
                if let Some(worker) = workers.get_mut(worker_id) {
                    worker.metadata = metadata.clone();
                }
            }
            _ => {}
        }
    }
    workers.into_values().collect()
}

pub fn worker_presences(root: &Path) -> Result<Vec<WorkerPresence>> {
    Ok(project_worker_presences(
        &load_ledger_readonly(root)?.0.events,
    ))
}
fn project_worker_presences(events: &[DevelopmentEvent]) -> Vec<WorkerPresence> {
    let mut presences = BTreeMap::<String, WorkerPresence>::new();
    for event in events {
        if let DevelopmentEventPayload::WorkerPresenceUpdated { presence } = &event.payload {
            if let Some(worker_id) = &event.actor_worker_id {
                presences.insert(
                    worker_id.clone(),
                    WorkerPresence {
                        worker_id: worker_id.clone(),
                        status: presence.status.clone(),
                        current_intent_ref: presence.current_intent_ref.clone(),
                        current_task_ref: presence.current_task_ref.clone(),
                        current_session_ref: presence.current_session_ref.clone(),
                        current_workspace_ref: presence.current_workspace_ref.clone(),
                        current_activity_summary: presence.current_activity_summary.clone(),
                        observed_global_revision: presence.observed_global_revision,
                        last_seen: event.timestamp,
                        last_event_id: event.event_id.clone(),
                    },
                );
            }
        }
    }
    presences.into_values().collect()
}

pub fn register_worker(
    root: &Path,
    worker_id: Option<String>,
    metadata: WorkerMetadata,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    validate_metadata(&metadata)?;
    let worker_id = worker_id.unwrap_or_else(|| format!("wrk_{}", route_core::new_id()));
    validate_identifier("worker_id", &worker_id)?;
    let payload = DevelopmentEventPayload::WorkerRegistered {
        descriptor: WorkerDescriptor {
            worker_id: worker_id.clone(),
            created_at: route_core::now_millis(),
            metadata,
        },
    };
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id: Some(worker_id),
            execution_session_ref: None,
            intent_ref: None,
            task_ref: None,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: vec![],
            evidence_refs: vec![],
            payload,
            deduplication_key,
        },
    )
}

pub fn update_worker_presence(
    root: &Path,
    worker_id: &str,
    presence: WorkerPresenceInput,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    if !worker_descriptors(root)?
        .iter()
        .any(|worker| worker.worker_id == worker_id)
    {
        bail!("worker does not exist");
    }
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id: Some(worker_id.to_string()),
            execution_session_ref: presence.current_session_ref.clone(),
            intent_ref: presence.current_intent_ref.clone(),
            task_ref: presence.current_task_ref.clone(),
            workspace_ref: presence.current_workspace_ref.clone(),
            causation_id: None,
            correlation_id: None,
            source_refs: vec![],
            evidence_refs: vec![],
            payload: DevelopmentEventPayload::WorkerPresenceUpdated { presence },
            deduplication_key,
        },
    )
}

pub fn send_worker_message(
    root: &Path,
    from_worker: &str,
    input: WorkerMessageInput,
    deduplication_key: Option<String>,
) -> Result<AppendDevelopmentEventResult> {
    let workers = worker_descriptors(root)?;
    if !workers.iter().any(|worker| worker.worker_id == from_worker) {
        bail!("from_worker does not exist");
    }
    if let Some(target) = input.target_worker.as_deref() {
        if !workers.iter().any(|worker| worker.worker_id == target) {
            bail!("target_worker does not exist");
        }
    }
    let message = WorkerMessage {
        message_id: input
            .message_id
            .unwrap_or_else(|| format!("msg_{}", route_core::new_id())),
        from_worker: from_worker.to_string(),
        target_worker: input.target_worker,
        intent_ref: input.intent_ref.clone(),
        task_ref: input.task_ref.clone(),
        thread_ref: input.thread_ref,
        message_type: input.message_type,
        content: input.content,
        source_refs: input.source_refs,
        requires_response: input.requires_response,
        timestamp: route_core::now_millis(),
    };
    append_development_event(
        root,
        DevelopmentEventDraft {
            event_id: None,
            actor_worker_id: Some(from_worker.to_string()),
            execution_session_ref: None,
            intent_ref: input.intent_ref,
            task_ref: input.task_ref,
            workspace_ref: None,
            causation_id: None,
            correlation_id: None,
            source_refs: message.source_refs.clone(),
            evidence_refs: vec![],
            payload: DevelopmentEventPayload::WorkerMessage { message },
            deduplication_key,
        },
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_ref: String,
    pub intent_ref: String,
    pub task: String,
    pub status: SessionStatus,
    pub started_at: i64,
}

impl From<&ExecutionSession> for SessionSummary {
    fn from(session: &ExecutionSession) -> Self {
        Self {
            session_ref: session.id.clone(),
            intent_ref: session.id.clone(),
            task: session.task.clone(),
            status: session.status.clone(),
            started_at: session.started_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitWorkspaceSummary {
    pub workspace_id: String,
    pub root: String,
    pub git_present: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct KnownGoodSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_verified: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_failed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedDevelopmentState {
    pub project_id: String,
    pub workspace_id: String,
    pub global_revision: u64,
    pub active_development_intents: Vec<SessionSummary>,
    pub active_execution_sessions: Vec<SessionSummary>,
    pub active_tasks: Vec<SessionSummary>,
    pub workers: Vec<WorkerDescriptor>,
    pub worker_presence: Vec<WorkerPresence>,
    pub latest_evidence: Vec<Evidence>,
    pub git_workspace: GitWorkspaceSummary,
    pub known_good: KnownGoodSummary,
    pub recent_events: Vec<DevelopmentEvent>,
    pub cooperation_resources: Vec<CooperationResourceSummary>,
    pub recent_cooperation_knowledge: Vec<CooperationKnowledgeSummary>,
}

fn git_value(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub fn shared_development_state(
    root: &Path,
    recent_limit: usize,
) -> Result<SharedDevelopmentState> {
    let identity = load_identity(root)?
        .ok_or_else(|| anyhow::anyhow!("project identity is missing; run route init first"))?;
    let ledger = load_ledger_readonly(root)?.0;
    let global_revision = ledger.revision();
    let sessions = SessionStore::load(root)?.sessions;
    let active: Vec<SessionSummary> = sessions
        .iter()
        .filter(|session| session.status == SessionStatus::Active)
        .map(SessionSummary::from)
        .collect();
    let mut evidence = EvidenceStore::load(root)?.evidence;
    evidence.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    evidence.truncate(20);
    let known_good = KnownGoodTracker::load(&identity.project_id)
        .map(|tracker| KnownGoodSummary {
            latest_verified: tracker.latest_verified,
            latest_failed: tracker.latest_failed,
        })
        .unwrap_or_default();
    let branch = git_value(root, &["branch", "--show-current"]).filter(|value| !value.is_empty());
    let head = git_value(root, &["rev-parse", "HEAD"]).filter(|value| !value.is_empty());
    let dirty = git_value(root, &["status", "--porcelain"]).map(|value| !value.is_empty());
    let recent_limit = recent_limit.min(100);
    let recent_events = ledger
        .events
        .iter()
        .rev()
        .take(recent_limit)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let (cooperation_resources, recent_cooperation_knowledge) =
        cooperation::cooperation_summaries(&ledger.events, recent_limit)?;
    Ok(SharedDevelopmentState {
        project_id: identity.project_id,
        workspace_id: identity.workspace_id.clone(),
        global_revision,
        active_development_intents: active.clone(),
        active_execution_sessions: active.clone(),
        active_tasks: active,
        workers: project_worker_descriptors(&ledger.events),
        worker_presence: project_worker_presences(&ledger.events),
        latest_evidence: evidence,
        git_workspace: GitWorkspaceSummary {
            workspace_id: identity.workspace_id,
            root: root.to_string_lossy().to_string(),
            git_present: root.join(".git").exists(),
            branch,
            head,
            dirty,
        },
        known_good,
        recent_events,
        cooperation_resources,
        recent_cooperation_knowledge,
    })
}

struct DevelopmentAppendLock {
    _guard: crate::ownership_lock::OwnershipLock,
}
impl DevelopmentAppendLock {
    fn acquire(dir: &Path) -> Result<Self> {
        Ok(Self {
            _guard: crate::ownership_lock::OwnershipLock::acquire(&dir.join(".append-lock"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_identity::attach;
    use tempfile::tempdir;

    fn metadata(provider: &str, model: &str) -> WorkerMetadata {
        WorkerMetadata {
            provider: Some(provider.to_string()),
            model: Some(model.to_string()),
            ..WorkerMetadata::default()
        }
    }

    #[test]
    fn event_persistence_revision_query_restart_and_isolation() {
        let first = tempdir().unwrap();
        let second = tempdir().unwrap();
        register_worker(
            first.path(),
            Some("worker-a".into()),
            metadata("p", "m"),
            None,
        )
        .unwrap();
        let page = query_development_events(first.path(), 0, 10).unwrap();
        assert_eq!(page.global_revision, 1);
        assert_eq!(page.events[0].sequence, 1);
        assert!(query_development_events(second.path(), 0, 10).is_err());
        assert_eq!(fs::read_dir(second.path()).unwrap().count(), 0);
        ensure_identity(second.path()).unwrap();
        assert!(query_development_events(second.path(), 0, 10)
            .unwrap()
            .events
            .is_empty());
        drop(page);
        assert_eq!(global_development_revision(first.path()).unwrap(), 1);
    }

    #[test]
    fn worker_identity_survives_provider_and_model_change() {
        let root = tempdir().unwrap();
        register_worker(
            root.path(),
            Some("stable-worker".into()),
            metadata("a", "one"),
            None,
        )
        .unwrap();
        register_worker(
            root.path(),
            Some("stable-worker".into()),
            metadata("b", "two"),
            None,
        )
        .unwrap();
        let workers = worker_descriptors(root.path()).unwrap();
        assert_eq!(workers.len(), 1);
        assert_eq!(workers[0].worker_id, "stable-worker");
        assert_eq!(workers[0].metadata.provider.as_deref(), Some("b"));
        assert_eq!(workers[0].metadata.model.as_deref(), Some("two"));
        assert_eq!(global_development_revision(root.path()).unwrap(), 2);
    }

    #[test]
    fn presence_and_staleness_are_project_metadata() {
        let root = tempdir().unwrap();
        register_worker(root.path(), Some("w".into()), metadata("p", "m"), None).unwrap();
        update_worker_presence(
            root.path(),
            "w",
            WorkerPresenceInput {
                status: WorkerStatus::Active,
                current_intent_ref: None,
                current_task_ref: None,
                current_session_ref: None,
                current_workspace_ref: None,
                current_activity_summary: Some("reviewing changes".into()),
                observed_global_revision: 1,
            },
            None,
        )
        .unwrap();
        let presence = worker_presences(root.path()).unwrap().pop().unwrap();
        assert_eq!(presence.status, WorkerStatus::Active);
        assert!(development_view_is_stale(
            presence.observed_global_revision,
            global_development_revision(root.path()).unwrap()
        ));
    }

    #[test]
    fn worker_message_is_globally_visible_and_never_evidence() {
        let root = tempdir().unwrap();
        register_worker(root.path(), Some("a".into()), metadata("p", "m"), None).unwrap();
        register_worker(root.path(), Some("b".into()), metadata("p", "m"), None).unwrap();
        let before = EvidenceStore::load(root.path()).unwrap().evidence.len();
        send_worker_message(
            root.path(),
            "a",
            WorkerMessageInput {
                message_id: Some("message-1".into()),
                target_worker: Some("b".into()),
                intent_ref: None,
                task_ref: None,
                thread_ref: None,
                message_type: WorkerMessageType::Question,
                content: "Can you review this?".into(),
                source_refs: vec![],
                requires_response: true,
            },
            None,
        )
        .unwrap();
        let page = query_development_events(root.path(), 2, 10).unwrap();
        assert_eq!(page.events.len(), 1);
        assert!(matches!(
            page.events[0].payload,
            DevelopmentEventPayload::WorkerMessage { .. }
        ));
        assert_eq!(
            EvidenceStore::load(root.path()).unwrap().evidence.len(),
            before
        );
    }

    #[test]
    fn concurrent_writers_are_ordered_and_duplicate_retry_is_idempotent() {
        let root = tempdir().unwrap();
        let root = std::sync::Arc::new(root.path().to_path_buf());
        let handles: Vec<_> = (0..16)
            .map(|index| {
                let root = root.clone();
                std::thread::spawn(move || {
                    register_worker(
                        &root,
                        Some(format!("w-{index}")),
                        metadata("p", "m"),
                        Some(format!("register-{index}")),
                    )
                    .unwrap()
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
        let page = query_development_events(&root, 0, 100).unwrap();
        assert_eq!(page.events.len(), 16);
        assert!(page
            .events
            .iter()
            .enumerate()
            .all(|(index, event)| event.sequence == index as u64 + 1));
        let replay = register_worker(
            &root,
            Some("w-0".into()),
            metadata("p", "m"),
            Some("register-0".into()),
        )
        .unwrap();
        assert!(replay.replay);
        assert_eq!(global_development_revision(&root).unwrap(), 16);
    }

    #[test]
    fn attached_workspaces_share_project_events_but_keep_workspace_identity() {
        let source = tempdir().unwrap();
        let attached = tempdir().unwrap();
        let source_identity = ensure_identity(source.path()).unwrap();
        let attached_identity = attach(attached.path(), source.path()).unwrap();
        assert_eq!(source_identity.project_id, attached_identity.project_id);
        assert_ne!(source_identity.workspace_id, attached_identity.workspace_id);
        register_worker(
            attached.path(),
            Some("attached-worker".into()),
            metadata("p", "m"),
            None,
        )
        .unwrap();
        assert_eq!(global_development_revision(source.path()).unwrap(), 1);
        assert_eq!(global_development_revision(attached.path()).unwrap(), 1);
    }

    #[test]
    fn corrupt_ledger_fails_closed_and_raw_reasoning_has_no_schema_path() {
        let root = tempdir().unwrap();
        register_worker(root.path(), Some("w".into()), metadata("p", "m"), None).unwrap();
        fs::write(development_ledger_path(root.path()).unwrap(), b"{broken").unwrap();
        assert!(query_development_events(root.path(), 0, 10).is_err());

        let serialized = serde_json::to_string(&WorkerPresenceInput {
            status: WorkerStatus::Idle,
            current_intent_ref: None,
            current_task_ref: None,
            current_session_ref: None,
            current_workspace_ref: None,
            current_activity_summary: Some("bounded summary".into()),
            observed_global_revision: 0,
        })
        .unwrap();
        assert!(!serialized.contains("reasoning"));
        assert!(!serialized.contains("chain_of_thought"));
        let unknown = r#"{"status":"IDLE","observed_global_revision":0,"raw_reasoning":"x"}"#;
        assert!(serde_json::from_str::<WorkerPresenceInput>(unknown).is_err());
    }

    #[test]
    fn schema_one_ledger_is_readable_and_upgrades_without_rewriting_events() {
        let root = tempdir().unwrap();
        register_worker(root.path(), Some("w".into()), metadata("p", "m"), None).unwrap();
        let path = development_ledger_path(root.path()).unwrap();
        let mut document: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let original_event = document["events"][0].clone();
        document["schema"] = serde_json::json!(LEGACY_LEDGER_SCHEMA);
        crate::constitutive::write_atomic(&path, &serde_json::to_vec_pretty(&document).unwrap())
            .unwrap();
        assert_eq!(
            query_development_events(root.path(), 0, 10)
                .unwrap()
                .events
                .len(),
            1
        );
        register_worker(root.path(), Some("w2".into()), metadata("p", "m"), None).unwrap();
        let upgraded: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(upgraded["schema"], LEDGER_SCHEMA);
        assert_eq!(upgraded["events"][0], original_event);
    }
}
