//! Project institutions: pure bounded evaluation, explicit grants, one existing ledger.
use crate::development::{
    self, AppendDevelopmentEventResult, DevelopmentEvent, DevelopmentEventDraft,
    DevelopmentEventPayload,
};
use anyhow::{anyhow, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};

pub const MAX_PACKAGE_BYTES: u64 = 65_536;
pub const MAX_ACTIVE: usize = 16;
pub const MAX_EFFECTS: usize = 32;
pub const MAX_REPLAY: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Hook {
    OnEvent,
    OnStateChange,
    OnWorkAvailable,
    OnWorkerStateChange,
    OnConflict,
    OnRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstitutionEffect {
    pub family: String,
    pub summary: String,
    #[serde(default)]
    pub target_ref: Option<String>,
    #[serde(default)]
    pub conflict_key: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub hook: Hook,
    #[serde(default)]
    pub event_type: Option<String>,
    #[serde(default)]
    pub require_available_work: bool,
    #[serde(default)]
    pub effects: Vec<InstitutionEffect>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstitutionPackage {
    pub institution_id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub version: String,
    pub provenance: String,
    pub implementation_kind: String,
    pub supported_hooks: Vec<Hook>,
    #[serde(default)]
    pub requested_capabilities: Vec<String>,
    pub compatibility: Vec<String>,
    #[serde(default)]
    pub configuration: BTreeMap<String, String>,
    #[serde(default)]
    pub configuration_schema: BTreeMap<String, String>,
    #[serde(default)]
    pub rules: Vec<Rule>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstitutionVersion {
    pub institution_id: String,
    pub version: String,
    pub name: String,
    pub description: Option<String>,
    pub fingerprint: String,
    pub provenance: String,
    pub source_locator: String,
    pub created_at: i64,
    pub supported_hooks: Vec<Hook>,
    pub requested_capabilities: Vec<String>,
    pub compatibility: Vec<String>,
    pub implementation_kind: String,
    pub configuration_fingerprint: String,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstitutionBinding {
    pub institution_id: String,
    pub version: String,
    pub fingerprint: String,
    pub project_id: String,
    pub active: bool,
    pub order: i32,
    pub grants: Vec<String>,
    pub authority_scope: String,
    pub configuration_ref: String,
    pub activated_by: String,
    pub activation_revision: u64,
    pub activated_at: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HookContext {
    pub project_id: String,
    pub institution_id: String,
    pub version: String,
    pub fingerprint: String,
    pub hook: Hook,
    pub triggering_event_ref: String,
    pub triggering_event_type: String,
    pub triggering_revision: u64,
    pub global_revision: u64,
    pub worker_refs: Vec<String>,
    pub worker_presence: Vec<development::WorkerPresence>,
    pub work_refs: Vec<String>,
    pub reference_refs: Vec<String>,
    pub cooperation_refs: Vec<String>,
    pub constraint_refs: Vec<String>,
    pub constraint_coverage: String,
    pub requesting_actor: Option<String>,
    pub correlation_id: String,
    pub causation_id: String,
    pub snapshot_semantics: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EffectRecord {
    pub effect_id: String,
    pub institution_id: String,
    pub version: String,
    pub fingerprint: String,
    pub effect: InstitutionEffect,
    pub lifecycle: Vec<String>,
    pub status: String,
    pub required_authority: String,
    pub available_authority: Vec<String>,
    pub reason: String,
    pub conflict_with: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HookResult {
    pub context: HookContext,
    pub status: String,
    pub reason: String,
    pub effects: Vec<EffectRecord>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InvocationReport {
    pub input_revision: u64,
    pub invocations: Vec<HookResult>,
    pub dry_run: bool,
}

/// Each transaction commits all its typed lifecycle records atomically.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum InstitutionChange {
    InstitutionRegistered {
        definition: InstitutionVersion,
    },
    InstitutionActivated {
        binding: InstitutionBinding,
        replaces_version: Option<String>,
    },
    InstitutionDeactivated {
        institution_id: String,
        version: String,
        fingerprint: String,
        actor: String,
    },
    InstitutionHookInvoked {
        report: InvocationReport,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct InstitutionTransaction {
    pub request_hash: String,
    pub expected_revision: u64,
    pub lifecycle: Vec<String>,
    pub change: InstitutionChange,
}

/// Commands are issued by the trusted local operator/host, never by package handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum InstitutionCommand {
    Register {
        #[serde(default)]
        source_locator: String,
        #[serde(default)]
        cooperation_ref: Option<String>,
    },
    Activate {
        institution_id: String,
        version: String,
        project_id: String,
        grants: Vec<String>,
        #[serde(default)]
        order: i32,
        activated_by: String,
        expected_binding_revision: u64,
    },
    Deactivate {
        institution_id: String,
        project_id: String,
        actor: String,
        expected_binding_revision: u64,
    },
    Invoke {
        hook: Hook,
        triggering_event_ref: String,
        expected_revision: u64,
        #[serde(default)]
        requesting_actor: Option<String>,
        correlation_id: String,
    },
}

fn safe_text(value: &str, max: usize) -> Result<()> {
    ensure!(
        !value.trim().is_empty() && value.len() <= max,
        "INVALID_METADATA: field length must be 1..{max}"
    );
    crate::cooperation::validate_public_metadata(value)?;
    Ok(())
}
fn identity(value: &str) -> Result<()> {
    safe_text(value, 128)?;
    ensure!(
        value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
        "INVALID_ID: use letters, numbers, dash, underscore or dot"
    );
    Ok(())
}
fn validate_package(package: &InstitutionPackage) -> Result<()> {
    identity(&package.institution_id)?;
    identity(&package.version)?;
    safe_text(&package.name, 256)?;
    safe_text(&package.provenance, 512)?;
    safe_text(&package.implementation_kind, 128)?;
    if let Some(value) = &package.description {
        safe_text(value, 1024)?;
    }
    ensure!(
        package.supported_hooks.len() <= 6
            && package.requested_capabilities.len() <= 16
            && package.compatibility.len() <= 16,
        "PACKAGE_LIMIT: too many hooks or capabilities"
    );
    ensure!(
        package.rules.len() <= 32
            && package.rules.iter().map(|r| r.effects.len()).sum::<usize>() <= MAX_EFFECTS,
        "PACKAGE_LIMIT: at most 32 total effects/rules"
    );
    for value in package
        .requested_capabilities
        .iter()
        .chain(&package.compatibility)
    {
        safe_text(value, 128)?;
    }
    for map in [&package.configuration, &package.configuration_schema] {
        ensure!(
            map.len() <= 16,
            "PACKAGE_LIMIT: configuration keys exceed 16"
        );
        for (key, value) in map {
            safe_text(key, 64)?;
            safe_text(value, 256)?;
            let key = key.to_ascii_lowercase();
            ensure!(
                !key.contains("chain_of_thought") && !key.contains("raw_reasoning") && key != "cot",
                "RAW_REASONING_REJECTED"
            );
        }
    }
    for rule in &package.rules {
        ensure!(
            package.supported_hooks.contains(&rule.hook),
            "UNSUPPORTED_HOOK: rule hook is not declared"
        );
        if let Some(value) = &rule.event_type {
            safe_text(value, 128)?;
        }
        for effect in &rule.effects {
            safe_text(&effect.family, 64)?;
            safe_text(&effect.summary, 512)?;
            for value in [&effect.target_ref, &effect.conflict_key, &effect.value]
                .into_iter()
                .flatten()
            {
                safe_text(value, 256)?;
            }
        }
    }
    Ok(())
}

pub fn inspect_package(
    root: &Path,
    source: &str,
) -> Result<(InstitutionVersion, InstitutionPackage)> {
    safe_text(source, 2048)?;
    let input = Path::new(source);
    let path = if input.is_absolute() {
        input.to_path_buf()
    } else {
        root.join(input)
    };
    // Reject ancestor links before checking whether a locator denotes a directory.
    let (_, availability, _, _) = crate::cooperation::inspect_locator(
        root,
        source,
        &crate::cooperation::CooperationKind("DIRECTORY".into()),
    )?;
    ensure!(
        availability == crate::cooperation::CooperationAvailability::Available,
        "INSTITUTION_UNAVAILABLE: use an available local manifest/directory without ancestor links"
    );
    let path = if path.is_dir() {
        path.join("institution.json")
    } else {
        path
    };
    let (_, availability, _, _) = crate::cooperation::inspect_locator(
        root,
        &path.to_string_lossy(),
        &crate::cooperation::CooperationKind("DOCUMENT".into()),
    )?;
    ensure!(
        availability == crate::cooperation::CooperationAvailability::Available,
        "INSTITUTION_UNAVAILABLE: manifest missing or linked"
    );
    ensure!(
        fs::metadata(&path)?.len() <= MAX_PACKAGE_BYTES,
        "PACKAGE_LIMIT: manifest exceeds 64 KiB"
    );
    let mut bytes = vec![];
    fs::File::open(&path)?
        .take(MAX_PACKAGE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_PACKAGE_BYTES,
        "PACKAGE_LIMIT: manifest grew during read"
    );
    let package: InstitutionPackage = serde_json::from_slice(&bytes)
        .context("INVALID_PACKAGE: strict manifest JSON; unknown fields are not accepted")?;
    validate_package(&package)?;
    let definition = InstitutionVersion {
        institution_id: package.institution_id.clone(),
        version: package.version.clone(),
        name: package.name.clone(),
        description: package.description.clone(),
        fingerprint: route_core::sha256_hex(&bytes),
        provenance: package.provenance.clone(),
        source_locator: path.to_string_lossy().into(),
        created_at: route_core::now_millis(),
        supported_hooks: package.supported_hooks.clone(),
        requested_capabilities: package.requested_capabilities.clone(),
        compatibility: package.compatibility.clone(),
        implementation_kind: package.implementation_kind.clone(),
        configuration_fingerprint: route_core::sha256_hex(&serde_json::to_vec(
            &package.configuration,
        )?),
        status: if package.implementation_kind == "DECLARATIVE_V1"
            && package.compatibility.contains(&"route/1".into())
        {
            "AVAILABLE"
        } else {
            "UNSUPPORTED_IMPLEMENTATION"
        }
        .into(),
    };
    Ok((definition, package))
}

#[derive(Default)]
struct World {
    versions: BTreeMap<(String, String), InstitutionVersion>,
    bindings: BTreeMap<String, InstitutionBinding>,
}
fn project(events: &[DevelopmentEvent]) -> World {
    let mut world = World::default();
    for event in events {
        let DevelopmentEventPayload::Institution { transaction } = &event.payload else {
            continue;
        };
        match &transaction.change {
            InstitutionChange::InstitutionRegistered { definition } => {
                world.versions.insert(
                    (
                        definition.institution_id.clone(),
                        definition.version.clone(),
                    ),
                    definition.clone(),
                );
            }
            InstitutionChange::InstitutionActivated { binding, .. } => {
                world
                    .bindings
                    .insert(binding.institution_id.clone(), binding.clone());
            }
            InstitutionChange::InstitutionDeactivated { institution_id, .. } => {
                if let Some(binding) = world.bindings.get_mut(institution_id) {
                    binding.active = false;
                    binding.activation_revision = event.sequence;
                }
            }
            _ => {}
        }
    }
    world
}
fn read(root: &Path) -> Result<(Vec<DevelopmentEvent>, String)> {
    let (ledger, identity, _) = development::load_ledger_readonly(root)?;
    Ok((ledger.events, identity.project_id))
}
pub fn versions(root: &Path) -> Result<Vec<InstitutionVersion>> {
    if crate::load_identity(root)?.is_none() {
        return Ok(vec![]);
    }
    Ok(project(&read(root)?.0).versions.into_values().collect())
}
pub fn bindings(root: &Path) -> Result<Vec<InstitutionBinding>> {
    if crate::load_identity(root)?.is_none() {
        return Ok(vec![]);
    }
    let mut values: Vec<_> = project(&read(root)?.0).bindings.into_values().collect();
    values.sort_by(|a, b| (a.order, &a.institution_id).cmp(&(b.order, &b.institution_id)));
    Ok(values)
}
pub fn get(root: &Path, id: &str, version: &str) -> Result<InstitutionVersion> {
    versions(root)?
        .into_iter()
        .find(|v| v.institution_id == id && v.version == version)
        .ok_or_else(|| {
            anyhow!("INSTITUTION_UNAVAILABLE: register this exact institution/version first")
        })
}
fn verified_source(root: &Path, version: &InstitutionVersion) -> Result<InstitutionPackage> {
    let (current, package) = inspect_package(root, &version.source_locator)?;
    ensure!(current.institution_id==version.institution_id && current.version==version.version && current.fingerprint==version.fingerprint,"VERSION_MISMATCH: source changed; register a new immutable version, do not overwrite the old source");
    ensure!(
        current.status == "AVAILABLE",
        "INSTITUTION_UNAVAILABLE: implementation/compatibility has no safe adapter"
    );
    Ok(package)
}

/// The SDK exposes values and stable references only: no project path, shell or storage handle.
pub trait InstitutionRuntime {
    fn evaluate(
        &self,
        package: &InstitutionPackage,
        context: &HookContext,
    ) -> Result<Vec<InstitutionEffect>>;
}
pub struct DeclarativeRuntime;
impl InstitutionRuntime for DeclarativeRuntime {
    fn evaluate(
        &self,
        package: &InstitutionPackage,
        context: &HookContext,
    ) -> Result<Vec<InstitutionEffect>> {
        ensure!(
            package.implementation_kind == "DECLARATIVE_V1",
            "INSTITUTION_UNAVAILABLE: unsupported adapter"
        );
        let effects: Vec<_> = package
            .rules
            .iter()
            .filter(|rule| {
                rule.hook == context.hook
                    && rule
                        .event_type
                        .as_ref()
                        .is_none_or(|kind| kind == &context.triggering_event_type)
                    && (!rule.require_available_work || !context.work_refs.is_empty())
            })
            .flat_map(|rule| rule.effects.clone())
            .collect();
        ensure!(
            effects.len() <= MAX_EFFECTS,
            "INSTITUTION_ERROR: effect budget exceeded"
        );
        Ok(effects)
    }
}
fn required(effect: &InstitutionEffect) -> Option<&'static str> {
    Some(match effect.family.as_str() {
        "NO_OP" => "OBSERVE",
        "MESSAGE" => "COMMUNICATE",
        "RECOMMEND" | "PROPOSE" | "REQUEST_WORK" | "OFFER_WORK" | "SUGGEST_ALLOCATION"
        | "REQUEST_REVIEW" | "REQUEST_ESCALATION" => "PROPOSE",
        "REQUEST_ACTION" => "REQUEST_DOMAIN_ACTION",
        _ => return None,
    })
}
fn allowed_grant(grant: &str) -> bool {
    matches!(
        grant,
        "OBSERVE" | "COMMUNICATE" | "PROPOSE" | "REQUEST_DOMAIN_ACTION"
    )
}
fn validate_effect(effect: &InstitutionEffect, grants: &[String]) -> (String, String, String) {
    let Some(authority) = required(effect) else {
        return (
            "INVALID_EFFECT".into(),
            "UNKNOWN".into(),
            "Unknown effect family; no action or Evidence was produced".into(),
        );
    };
    if effect.target_ref.as_ref().is_some_and(|target| {
        target.starts_with("evidence:")
            || target.starts_with("worker:")
            || target.starts_with("binding:")
            || target.starts_with("shell:")
            || target.starts_with("git:")
            || target.starts_with("file:")
    }) {
        return (
            "DENIED".into(),
            authority.into(),
            "Privileged mutation/impersonation target is not available; effect remains rejected"
                .into(),
        );
    }
    if !grants.iter().any(|grant| grant == authority) {
        return ("DENIED".into(),authority.into(),"Required authority not granted by project operator; package declarations are not grants".into());
    }
    ("ACCEPTED".into(),authority.into(),"Accepted only as shared communication/proposal/request; no domain action executed and no Evidence manufactured".into())
}

fn context(
    root: &Path,
    events: &[DevelopmentEvent],
    project_id: &str,
    event: &DevelopmentEvent,
    hook: Hook,
    actor: Option<String>,
    correlation: &str,
) -> Result<HookContext> {
    if let Some(actor) = &actor {
        ensure!(events.iter().any(|e|matches!(&e.payload,DevelopmentEventPayload::WorkerRegistered{descriptor} if &descriptor.worker_id==actor)),"UNKNOWN_WORKER: requesting_actor must reference an existing Worker");
    }
    let mut workers = BTreeSet::new();
    let mut resources = BTreeSet::new();
    for event in events {
        match &event.payload {
            DevelopmentEventPayload::WorkerRegistered { descriptor } => {
                if workers.len() < 16 {
                    workers.insert(descriptor.worker_id.clone());
                }
            }
            DevelopmentEventPayload::CooperationResourceRegistered { registration } => {
                if resources.len() < 16 {
                    resources.insert(registration.cooperation_id.clone());
                }
            }
            _ => {}
        }
    }
    let work_refs = crate::execution::SessionStore::load(root)?
        .sessions
        .into_iter()
        .filter(|s| s.status == crate::execution::SessionStatus::Active)
        .take(16)
        .map(|s| s.id)
        .collect();
    let reference_refs = crate::ReferenceRegistry::read(root)?
        .entries
        .into_iter()
        .take(16)
        .map(|e| e.id)
        .collect();
    Ok(HookContext {
        project_id: project_id.into(),
        institution_id: String::new(),
        version: String::new(),
        fingerprint: String::new(),
        hook,
        triggering_event_ref: event.event_id.clone(),
        triggering_event_type: serde_json::to_value(&event.event_type)?
            .as_str()
            .unwrap_or_default()
            .into(),
        triggering_revision: event.sequence,
        global_revision: events.last().map(|e| e.sequence).unwrap_or(0),
        worker_refs: workers.into_iter().collect(),
        worker_presence: development::project_worker_presences(events)
            .into_iter()
            .take(16)
            .collect(),
        work_refs,
        reference_refs,
        cooperation_refs: resources.into_iter().collect(),
        constraint_refs: [".route/constitution.md", ".route/protocol.md"]
            .iter()
            .filter(|p| root.join(p).exists())
            .map(|p| (*p).to_string())
            .collect(),
        constraint_coverage: "PARTIAL: canonical prose is not interpreted as grants".into(),
        requesting_actor: actor,
        correlation_id: correlation.into(),
        causation_id: event.event_id.clone(),
        snapshot_semantics: "CURRENT_READ_ONLY_PROJECT_SNAPSHOT_WITH_HISTORICAL_TRIGGER".into(),
    })
}
fn evaluate_one(
    root: &Path,
    version: &InstitutionVersion,
    binding: &InstitutionBinding,
    mut context: HookContext,
) -> HookResult {
    context.institution_id = version.institution_id.clone();
    context.version = version.version.clone();
    context.fingerprint = version.fingerprint.clone();
    let mut result = HookResult {
        context,
        status: "NO_EFFECT".into(),
        reason: "No matching rule".into(),
        effects: vec![],
    };
    if !binding.grants.contains(&"OBSERVE".into()) {
        result.status = "DENIED".into();
        result.reason =
            "Hook requires OBSERVE; package requested capabilities confer no grant".into();
        return result;
    }
    let package = match verified_source(root, version) {
        Ok(value) => value,
        Err(error) => {
            let text = error.to_string();
            result.status = if text.contains("VERSION_MISMATCH") {
                "VERSION_MISMATCH"
            } else if text.contains("INVALID_PACKAGE") {
                "INSTITUTION_ERROR"
            } else {
                "INSTITUTION_UNAVAILABLE"
            }
            .into();
            result.reason = text;
            return result;
        }
    };
    if !package.supported_hooks.contains(&result.context.hook) {
        return result;
    }
    let effects = match DeclarativeRuntime.evaluate(&package, &result.context) {
        Ok(value) => value,
        Err(_) => {
            result.status = "INSTITUTION_ERROR".into();
            result.reason = "Adapter failed; no project action executed".into();
            return result;
        }
    };
    for (index, effect) in effects.into_iter().enumerate() {
        let (status, authority, reason) = validate_effect(&effect, &binding.grants);
        result.effects.push(EffectRecord {
            effect_id: format!(
                "{}:{}:{}:{}:{index}",
                version.institution_id,
                version.version,
                result.context.triggering_event_ref,
                result.context.global_revision
            ),
            institution_id: version.institution_id.clone(),
            version: version.version.clone(),
            fingerprint: version.fingerprint.clone(),
            effect,
            lifecycle: vec![
                "INSTITUTION_EFFECT_PRODUCED".into(),
                if status == "ACCEPTED" {
                    "INSTITUTION_EFFECT_ACCEPTED"
                } else {
                    "INSTITUTION_EFFECT_REJECTED"
                }
                .into(),
            ],
            status,
            required_authority: authority,
            available_authority: binding.grants.clone(),
            reason,
            conflict_with: vec![],
        });
    }
    if !result.effects.is_empty() {
        result.status = if result.effects.iter().all(|e| e.status == "ACCEPTED") {
            "SUCCESS"
        } else if result.effects.iter().any(|e| e.status == "INVALID_EFFECT") {
            "INVALID_EFFECT"
        } else {
            "DENIED"
        }
        .into();
        result.reason = "Typed outcomes recorded; requests do not execute domain actions".into();
    }
    result
}
fn conflicts(results: &mut [HookResult]) {
    let mut candidates = vec![];
    for (i, result) in results.iter().enumerate() {
        for (j, effect) in result.effects.iter().enumerate() {
            if let Some(key) = &effect.effect.conflict_key {
                candidates.push((
                    i,
                    j,
                    key.clone(),
                    effect.effect.value.clone(),
                    effect.effect_id.clone(),
                ));
            }
        }
    }
    for (i, j, key, value, _) in &candidates {
        results[*i].effects[*j].conflict_with = candidates
            .iter()
            .filter(|(x, y, k, v, _)| (x, y) != (i, j) && k == key && v != value)
            .map(|v| v.4.clone())
            .collect();
    }
}

pub fn execute(
    root: &Path,
    command: InstitutionCommand,
    key: &str,
) -> Result<AppendDevelopmentEventResult> {
    safe_text(key, 256)?;
    let request_hash = route_core::sha256_hex(&serde_json::to_vec(&command)?);
    let (events, project_id) = read(root)?;
    if let Some(event) = events
        .iter()
        .find(|e| e.deduplication_key.as_deref() == Some(key))
    {
        ensure!(
            matches!(&event.payload,DevelopmentEventPayload::Institution{transaction} if transaction.request_hash==request_hash),
            "IDEMPOTENCY_CONFLICT: same operation key with different institution request"
        );
        return Ok(AppendDevelopmentEventResult {
            event: event.clone(),
            replay: true,
            global_revision: events.last().map(|e| e.sequence).unwrap_or(0),
        });
    }
    let world = project(&events);
    let revision = events.last().map(|e| e.sequence).unwrap_or(0);
    let change = match command {
        InstitutionCommand::Register {
            source_locator,
            cooperation_ref,
        } => {
            let source = if let Some(id) = cooperation_ref {
                ensure!(
                    source_locator.is_empty(),
                    "INVALID_PACKAGE: supply source_locator OR cooperation_ref, not both"
                );
                crate::cooperation_resource(root, &id)?
                    .ok_or_else(|| {
                        anyhow!("INSTITUTION_UNAVAILABLE: CooperationResource not registered")
                    })?
                    .locator
            } else {
                source_locator
            };
            let (definition, _) = inspect_package(root, &source)?;
            ensure!(!world.versions.contains_key(&(definition.institution_id.clone(),definition.version.clone())),"VERSION_CONFLICT: immutable version already registered; retry original key or use a new version");
            InstitutionChange::InstitutionRegistered { definition }
        }
        InstitutionCommand::Activate {
            institution_id,
            version,
            project_id: requested,
            grants,
            order,
            activated_by,
            expected_binding_revision,
        } => {
            ensure!(
                requested == project_id,
                "PROJECT_IDENTITY_CONFLICT: binding belongs to a different Project"
            );
            safe_text(&activated_by, 128)?;
            ensure!(grants.len()<=4 && grants.iter().all(|g|allowed_grant(g)),"AUTHORITY_DENIED: unknown/future privileged grant; CHANGE_INSTITUTION_BINDING belongs to the operator, not packages");
            let prior = world.bindings.get(&institution_id);
            ensure!(
                prior.map(|b| b.activation_revision).unwrap_or(0) == expected_binding_revision,
                "BINDING_CONFLICT: inspect binding revision and retry explicitly"
            );
            ensure!(
                prior.is_some_and(|b| b.active)
                    || world.bindings.values().filter(|b| b.active).count() < MAX_ACTIVE,
                "BINDING_LIMIT: 16 active institutions maximum"
            );
            let definition = world
                .versions
                .get(&(institution_id.clone(), version.clone()))
                .ok_or_else(|| anyhow!("INSTITUTION_UNAVAILABLE: register exact version first"))?;
            verified_source(root, definition)?;
            InstitutionChange::InstitutionActivated {
                binding: InstitutionBinding {
                    institution_id,
                    version,
                    fingerprint: definition.fingerprint.clone(),
                    project_id: project_id.clone(),
                    active: true,
                    order,
                    grants,
                    authority_scope: "PROJECT_SAFE_EFFECTS_ONLY".into(),
                    configuration_ref: definition.configuration_fingerprint.clone(),
                    activated_by,
                    activation_revision: revision + 1,
                    activated_at: route_core::now_millis(),
                },
                replaces_version: prior.map(|b| b.version.clone()),
            }
        }
        InstitutionCommand::Deactivate {
            institution_id,
            project_id: requested,
            actor,
            expected_binding_revision,
        } => {
            ensure!(requested == project_id, "PROJECT_IDENTITY_CONFLICT");
            safe_text(&actor, 128)?;
            let prior = world
                .bindings
                .get(&institution_id)
                .ok_or_else(|| anyhow!("INSTITUTION_UNAVAILABLE: no binding exists"))?;
            ensure!(
                prior.activation_revision == expected_binding_revision,
                "BINDING_CONFLICT: refresh binding revision"
            );
            InstitutionChange::InstitutionDeactivated {
                institution_id,
                version: prior.version.clone(),
                fingerprint: prior.fingerprint.clone(),
                actor,
            }
        }
        InstitutionCommand::Invoke {
            hook,
            triggering_event_ref,
            expected_revision,
            requesting_actor,
            correlation_id,
        } => {
            ensure!(
                expected_revision == revision,
                "STALE_CONTEXT: query current GlobalRevision and retry with a new operation key"
            );
            safe_text(&correlation_id, 128)?;
            let event = events
                .iter()
                .find(|e| e.event_id == triggering_event_ref)
                .ok_or_else(|| {
                    anyhow!("UNKNOWN_EVENT: use an existing same-project event reference")
                })?;
            let context = context(
                root,
                &events,
                &project_id,
                event,
                hook,
                requesting_actor,
                &correlation_id,
            )?;
            let mut active: Vec<_> = world.bindings.values().filter(|b| b.active).collect();
            active.sort_by(|a, b| (a.order, &a.institution_id).cmp(&(b.order, &b.institution_id)));
            let mut invocations = vec![];
            for binding in active {
                let version = world
                    .versions
                    .get(&(binding.institution_id.clone(), binding.version.clone()))
                    .ok_or_else(|| {
                        anyhow!("CORRUPTED_STATE: binding references missing version")
                    })?;
                invocations.push(evaluate_one(root, version, binding, context.clone()));
            }
            conflicts(&mut invocations);
            InstitutionChange::InstitutionHookInvoked {
                report: InvocationReport {
                    input_revision: revision,
                    invocations,
                    dry_run: false,
                },
            }
        }
    };
    let lifecycle = match &change {
        InstitutionChange::InstitutionRegistered { .. } => vec!["INSTITUTION_REGISTERED".into()],
        InstitutionChange::InstitutionActivated {
            binding,
            replaces_version,
        } => {
            let mut labels = vec!["INSTITUTION_ACTIVATED".into()];
            if replaces_version
                .as_ref()
                .is_some_and(|old| old != &binding.version)
            {
                labels.push("INSTITUTION_VERSION_CHANGED".into());
            }
            labels
        }
        InstitutionChange::InstitutionDeactivated { .. } => vec!["INSTITUTION_DEACTIVATED".into()],
        InstitutionChange::InstitutionHookInvoked { .. } => vec!["INSTITUTION_HOOK_INVOKED".into()],
    };
    let transaction = InstitutionTransaction {
        request_hash,
        expected_revision: revision,
        lifecycle,
        change,
    };
    let draft: DevelopmentEventDraft = serde_json::from_value(
        json!({"payload":DevelopmentEventPayload::Institution{transaction},"deduplication_key":key}),
    )?;
    development::append_institution_event(root, draft)
}

/// Called inside the existing ledger lock, after keyed replay resolution.
pub(crate) fn validate_transition(
    root: &Path,
    project_id: &str,
    events: &[DevelopmentEvent],
    transaction: &InstitutionTransaction,
) -> Result<()> {
    ensure!(
        transaction.expected_revision == events.last().map(|e| e.sequence).unwrap_or(0),
        "STALE_CONTEXT: concurrent world change; retry same request after reconciliation"
    );
    let world = project(events);
    match &transaction.change {
        InstitutionChange::InstitutionRegistered { definition } => {
            ensure!(
                !world.versions.contains_key(&(
                    definition.institution_id.clone(),
                    definition.version.clone()
                )),
                "VERSION_CONFLICT"
            );
            let (actual, _) = inspect_package(root, &definition.source_locator)?;
            ensure!(
                actual.fingerprint == definition.fingerprint,
                "VERSION_MISMATCH: package changed during registration"
            );
        }
        InstitutionChange::InstitutionActivated { binding, .. } => {
            ensure!(
                binding.project_id == project_id,
                "PROJECT_IDENTITY_CONFLICT"
            );
            let version = world
                .versions
                .get(&(binding.institution_id.clone(), binding.version.clone()))
                .ok_or_else(|| anyhow!("VERSION_MISMATCH"))?;
            verified_source(root, version)?;
        }
        InstitutionChange::InstitutionHookInvoked { report } => {
            for result in &report.invocations {
                if matches!(
                    result.status.as_str(),
                    "SUCCESS" | "NO_EFFECT" | "DENIED" | "INVALID_EFFECT"
                ) {
                    let version = world
                        .versions
                        .get(&(
                            result.context.institution_id.clone(),
                            result.context.version.clone(),
                        ))
                        .ok_or_else(|| anyhow!("VERSION_MISMATCH"))?;
                    // A missing grant requires no source access; never turn denial into an optional-adapter failure.
                    if result.reason
                        != "Hook requires OBSERVE; package requested capabilities confer no grant"
                    {
                        verified_source(root, version)?;
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn replay(
    root: &Path,
    id: &str,
    version: &str,
    after: u64,
    limit: usize,
) -> Result<InvocationReport> {
    ensure!(
        limit > 0 && limit <= MAX_REPLAY,
        "REPLAY_LIMIT: select 1..1000 events"
    );
    let (events, project_id) = read(root)?;
    let world = project(&events);
    let definition = world
        .versions
        .get(&(id.into(), version.into()))
        .ok_or_else(|| anyhow!("INSTITUTION_UNAVAILABLE: register exact replay version"))?;
    verified_source(root, definition)?;
    let grants = world
        .bindings
        .get(id)
        .map(|b| b.grants.clone())
        .unwrap_or_default();
    let binding = InstitutionBinding {
        institution_id: id.into(),
        version: version.into(),
        fingerprint: definition.fingerprint.clone(),
        project_id: project_id.clone(),
        active: false,
        order: 0,
        grants,
        authority_scope: "DRY_RUN_ONLY".into(),
        configuration_ref: definition.configuration_fingerprint.clone(),
        activated_by: "replay".into(),
        activation_revision: 0,
        activated_at: 0,
    };
    let selected: Vec<_> = events
        .iter()
        .filter(|e| e.sequence > after)
        .take(limit)
        .collect();
    let mut invocations = vec![];
    if let Some(first) = selected.first() {
        let base = context(
            root,
            &events,
            &project_id,
            first,
            Hook::OnEvent,
            None,
            "replay",
        )?;
        // Read and validate the immutable source once per bounded replay, not once per event.
        let package = verified_source(root, definition)?;
        for event in selected {
            let mut context = base.clone();
            context.triggering_event_ref = event.event_id.clone();
            context.causation_id = event.event_id.clone();
            context.triggering_revision = event.sequence;
            context.triggering_event_type = serde_json::to_value(&event.event_type)?
                .as_str()
                .unwrap_or_default()
                .into();
            context.institution_id = id.into();
            context.version = version.into();
            context.fingerprint = definition.fingerprint.clone();
            let effects = DeclarativeRuntime.evaluate(&package, &context)?;
            let mut result=HookResult{context,status:if effects.is_empty(){"NO_EFFECT"}else{"SUCCESS"}.into(),reason:"Read-only evaluation; no effects applied; grants are current binding grants, not historical authorization".into(),effects:vec![]};
            for (index, effect) in effects.into_iter().enumerate() {
                let (status, required_authority, reason) =
                    validate_effect(&effect, &binding.grants);
                result.effects.push(EffectRecord {
                    effect_id: format!("replay:{id}:{version}:{}:{index}", event.sequence),
                    institution_id: id.into(),
                    version: version.into(),
                    fingerprint: definition.fingerprint.clone(),
                    effect,
                    lifecycle: vec!["DRY_RUN_EFFECT".into()],
                    status,
                    required_authority,
                    available_authority: binding.grants.clone(),
                    reason,
                    conflict_with: vec![],
                });
            }
            if !binding.grants.contains(&"OBSERVE".into()) {
                result.status = "DENIED".into();
                result.reason =
                    "Replay has no OBSERVE grant; effects are hypothetical and never applied"
                        .into();
                for effect in &mut result.effects {
                    effect.status = "DENIED".into();
                    effect.reason = "OBSERVE was not granted".into();
                }
            } else if result
                .effects
                .iter()
                .any(|effect| effect.status == "INVALID_EFFECT")
            {
                result.status = "INVALID_EFFECT".into();
            } else if result
                .effects
                .iter()
                .any(|effect| effect.status == "DENIED")
            {
                result.status = "DENIED".into();
            }
            invocations.push(result);
        }
    }
    Ok(InvocationReport {
        input_revision: events.last().map(|e| e.sequence).unwrap_or(0),
        invocations,
        dry_run: true,
    })
}
