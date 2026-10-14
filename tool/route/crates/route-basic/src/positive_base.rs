//! Optional source-backed learning. Canonical state remains CooperationKnowledge
//! in DevelopmentEvent; projections/packets are read-only and disposable.
use crate::{
    cooperation::{self, CooperationKnowledgeRecord as Record, EpistemicStatus},
    development::{self, DevelopmentEvent, DevelopmentEventPayload as Payload},
    execution::{Evidence, EvidenceKind, EvidenceSource, EvidenceStore},
    passive_base::{KnowledgeBudget, LessonState},
};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionPacket {
    pub trigger: String,
    pub reason: String,
    pub goal: String,
    pub work: String,
    pub finding: String,
    pub source_worker: String,
    pub source_events: Vec<String>,
    pub source_revision: u64,
    pub failed_evidence: String,
    pub passed_evidence: String,
    pub component: String,
    pub environment: Vec<String>,
    pub finding_summary: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositiveLesson {
    pub packet: ExtractionPacket,
    /// FIXTURE or MANUAL. No real AI synthesis is asserted by this pipeline.
    pub synthesis: String,
    /// FIX_PATTERN or FAILED_APPROACH; failed approaches are conditional, not bans.
    pub kind: String,
    pub title: String,
    pub symptoms: Vec<String>,
    pub cause_if_established: Option<String>,
    pub non_applicability: Vec<String>,
    #[serde(default)]
    pub reuse_evidence: Vec<String>,
}

fn scoped(e: &Evidence, goal: &str) -> bool {
    e.source == EvidenceSource::System
        && (e.session_id == goal || e.metadata.get("goal_id").is_some_and(|g| g == goal))
}
fn pass(e: &Evidence) -> bool {
    matches!(e.kind, EvidenceKind::CheckPass | EvidenceKind::TestPass)
}
fn fail(e: &Evidence) -> bool {
    matches!(e.kind, EvidenceKind::CheckFail | EvidenceKind::TestFail)
}
fn reuse_work<'a>(
    events: &'a [DevelopmentEvent],
    evidence: &Evidence,
    packet: &ExtractionPacket,
) -> Option<&'a str> {
    let started = evidence
        .metadata
        .get("check_started_revision")?
        .parse::<u64>()
        .ok()?;
    events.iter().rev().find_map(|event| match &event.payload {
        Payload::Work {
            action: crate::work::WorkAction::ChildCreated { work },
        } if event.sequence > packet.source_revision
            && event.sequence <= started
            && work.intent_ref == packet.goal
            && work.work_id != packet.work
            && work.scope_paths.contains(&packet.component)
            && evidence
                .metadata
                .get("check_id")
                .is_some_and(|c| work.verification_requirements.contains(c)) =>
        {
            Some(work.work_id.as_str())
        }
        _ => None,
    })
}
fn goal_work<'a>(
    events: &'a [DevelopmentEvent],
    goal: &str,
    work: &str,
) -> Option<(&'a crate::work::ChildWork, &'a DevelopmentEvent)> {
    events.iter().find_map(|event| match &event.payload {
        Payload::Work {
            action: crate::work::WorkAction::ChildCreated { work: item },
        } if item.work_id == work && item.intent_ref == goal => Some((item, event)),
        _ => None,
    })
}

/// Deterministic high signal only. Does not synthesize a lesson or write a job.
/// Zero is a healthy outcome, including arbitrary ordinary PASS histories.
pub fn packets(root: &Path, goal: &str) -> Result<Vec<ExtractionPacket>> {
    let (ledger, _, _) = development::load_ledger_readonly(root)?;
    let store = EvidenceStore::load(root)?;
    if !store.evidence.iter().any(|e| scoped(e, goal) && fail(e)) {
        return Ok(vec![]);
    }
    let current = crate::compute_state_hash(root)?;
    let mut out = vec![];
    for finding in ledger
        .events
        .iter()
        .rev()
        .filter(|e| e.intent_ref.as_deref() == Some(goal))
    {
        let Payload::Finding { summary, .. } = &finding.payload else {
            continue;
        };
        // A failed check alone can be trivial CRUD/typo; require a structured
        // investigation observation as well as real failure/fix/pass proof.
        if !summary.starts_with("ROOT_CAUSE:") && !summary.starts_with("RECOVERY:") {
            continue;
        }
        let (Some(work), Some(worker)) = (
            finding.task_ref.as_deref(),
            finding.actor_worker_id.as_deref(),
        ) else {
            continue;
        };
        let Some((item, work_event)) = goal_work(&ledger.events, goal, work) else {
            continue;
        };
        let Some(component) = item
            .scope_paths
            .first()
            .filter(|p| summary.contains(p.as_str()))
        else {
            continue;
        };
        let environment = vec![if cfg!(windows) {
            "Windows"
        } else {
            std::env::consts::OS
        }
        .to_string()];
        if !environment.iter().all(|e| summary.contains(e)) {
            continue;
        }
        let Some(registration) = ledger.events.iter().find(|e| matches!(&e.payload, Payload::WorkerRegistered {descriptor} if descriptor.worker_id == worker)) else { continue };
        for (pass_index, verified) in store.evidence.iter().enumerate().rev() {
            if !scoped(verified, goal)
                || !pass(verified)
                || verified.created_at < finding.timestamp
                || store.is_pass_stale(verified, &current)
            {
                continue;
            }
            let Some(check) = verified
                .metadata
                .get("check_id")
                .filter(|check| item.verification_requirements.contains(check))
            else {
                continue;
            };
            let Some(failed) = store.evidence[..pass_index].iter().rev().find(|e| {
                scoped(e, goal)
                    && fail(e)
                    && e.created_at <= finding.timestamp
                    && e.metadata.get("check_id") == Some(check)
            }) else {
                continue;
            };
            let packet = ExtractionPacket {
                trigger: "FAIL_THEN_VERIFIED_FIX".into(),
                reason: "Same Work-required check failed, an attributed scoped Finding followed, and fresh System PASS verified the current source world".into(),
                goal: goal.into(), work: work.into(), finding: finding.event_id.clone(), source_worker: worker.into(),
                source_events: vec![work_event.event_id.clone(), finding.event_id.clone(), registration.event_id.clone()],
                source_revision: ledger.events.last().map_or(0, |e| e.sequence), failed_evidence: failed.id.clone(), passed_evidence: verified.id.clone(),
                component: component.clone(), environment, finding_summary: crate::sidecar::safe_text(summary),
            };
            ensure!(
                serde_json::to_vec(&packet)?.len() <= KnowledgeBudget::default().max_packet_bytes,
                "EXTRACTION_PACKET_TOO_LARGE"
            );
            out.push(packet);
            break;
        }
        if out.len() == KnowledgeBudget::default().max_jobs_per_goal {
            break;
        }
    }
    Ok(out)
}

fn shape(p: &PositiveLesson) -> Result<()> {
    ensure!(
        ["FIXTURE", "MANUAL"].contains(&p.synthesis.as_str()),
        "REAL_AI_EXTRACTION_NOT_RUN"
    );
    ensure!(
        ["FIX_PATTERN", "FAILED_APPROACH"].contains(&p.kind.as_str()),
        "INVALID_KNOWLEDGE_KIND"
    );
    ensure!(
        !p.symptoms.is_empty()
            && p.symptoms.len() <= 4
            && !p.non_applicability.is_empty()
            && p.non_applicability.len() <= 4
            && p.reuse_evidence.len() <= 8,
        "KNOWLEDGE_BOUNDS"
    );
    for text in std::iter::once(&p.title)
        .chain(p.symptoms.iter())
        .chain(p.non_applicability.iter())
        .chain(p.cause_if_established.iter())
    {
        ensure!(
            (12..=256).contains(&text.len()),
            "KNOWLEDGE_GATE: bounded specific text required"
        );
        ensure!(
            ![
                "testing is important",
                "understand requirements first",
                "review code carefully",
                "handle edge cases",
                "use good error handling",
                "be careful with concurrency",
                "use robust error handling",
                "always read documentation",
                "all rust filesystem"
            ]
            .iter()
            .any(|s| text.to_ascii_lowercase().contains(s)),
            "KNOWLEDGE_GATE: generic advice"
        );
    }
    ensure!(
        p.kind != "FAILED_APPROACH" || p.cause_if_established.is_some(),
        "FAILED_APPROACH_REQUIRES_REASON"
    );
    ensure!(
        p.packet.trigger == "FAIL_THEN_VERIFIED_FIX" && p.packet.source_events.len() <= 8,
        "UNKNOWN_EXTRACTION_TRIGGER"
    );
    ensure!(
        serde_json::to_vec(p)?.len() <= 4096,
        "KNOWLEDGE_PACKET_TOO_LARGE"
    );
    Ok(())
}

/// Called under the canonical development append lock. This accepts references
/// to existing System Evidence, never mints Evidence or edits Worker Findings.
pub(crate) fn validate(root: &Path, events: &[DevelopmentEvent], record: &Record) -> Result<()> {
    let lesson = record.lesson.as_ref().context("LESSON_REQUIRED")?;
    let positive = lesson
        .positive
        .as_ref()
        .context("POSITIVE_METADATA_REQUIRED")?;
    // Apply the existing shareable-metadata boundary to the nested extension,
    // not only CooperationKnowledge's top-level statement/provenance.
    cooperation::validate_public_metadata(&serde_json::to_string(lesson)?)?;
    shape(positive)?;
    let packet = &positive.packet;
    ensure!(
        packet.environment
            == vec![if cfg!(windows) {
                "Windows"
            } else {
                std::env::consts::OS
            }
            .to_string()],
        "UNOBSERVED_ENVIRONMENT"
    );
    if packet
        .finding_summary
        .to_ascii_lowercase()
        .contains("restricted sandbox")
    {
        ensure!(
            lesson
                .condition
                .to_ascii_lowercase()
                .contains("restricted sandbox"),
            "NARROW_SANDBOX_SCOPE_REQUIRED"
        );
    }
    ensure!(
        record.epistemic_status == EpistemicStatus::Inferred,
        "KNOWLEDGE_IS_NOT_OBSERVED_CAPABILITY_EVIDENCE"
    );
    ensure!(
        lesson.goal_id == packet.goal
            && lesson.work_id == packet.work
            && lesson.finding_event_id == packet.finding
            && lesson.source_event_ids == packet.source_events
            && lesson.component == packet.component
            && lesson.environment == packet.environment,
        "EXTRACTION_PACKET_MISMATCH"
    );
    ensure!(
        lesson.creator_worker_id != packet.source_worker,
        "SEPARATE_KNOWLEDGE_WORKER_REQUIRED"
    );
    ensure!(events.iter().any(|e| matches!(&e.payload, Payload::WorkerRegistered {descriptor} if descriptor.worker_id == lesson.creator_worker_id)), "MISSING_KNOWLEDGE_WORKER");
    let finding = events
        .iter()
        .find(|e| e.event_id == packet.finding)
        .context("MISSING_FINDING")?;
    let Payload::Finding { summary, .. } = &finding.payload else {
        anyhow::bail!("SOURCE_NOT_FINDING")
    };
    ensure!(
        finding.intent_ref.as_deref() == Some(&packet.goal)
            && finding.task_ref.as_deref() == Some(&packet.work)
            && finding.actor_worker_id.as_deref() == Some(&packet.source_worker),
        "FOREIGN_FINDING_OR_CREATOR"
    );
    ensure!(
        (summary.starts_with("ROOT_CAUSE:") || summary.starts_with("RECOVERY:"))
            && summary.contains(&packet.component)
            && packet.environment.iter().all(|s| summary.contains(s))
            && crate::sidecar::safe_text(summary) == packet.finding_summary,
        "SCOPE_EXCEEDS_OBSERVATION"
    );
    let (work, work_event) =
        goal_work(events, &packet.goal, &packet.work).context("MISSING_WORK")?;
    ensure!(
        work.scope_paths.contains(&packet.component)
            && lesson.condition.contains(&packet.component)
            && packet
                .environment
                .iter()
                .all(|e| lesson.condition.contains(e))
            && !lesson.action.to_ascii_lowercase().contains("all "),
        "NARROW_SCOPE_REQUIRED"
    );
    let registration = events.iter().find(|e| matches!(&e.payload, Payload::WorkerRegistered {descriptor} if descriptor.worker_id == packet.source_worker)).context("MISSING_SOURCE_WORKER")?;
    ensure!(
        packet.source_events
            == vec![
                work_event.event_id.clone(),
                finding.event_id.clone(),
                registration.event_id.clone()
            ],
        "SOURCE_EVENTS_MISMATCH"
    );
    ensure!(
        packet.source_revision >= finding.sequence
            && packet.source_revision <= events.last().map_or(0, |e| e.sequence)
            && packet.source_events.iter().all(|id| events
                .iter()
                .any(|e| &e.event_id == id && e.sequence <= packet.source_revision)),
        "SOURCE_REVISION_OR_EVENT_MISMATCH"
    );
    let store = EvidenceStore::load(root)?;
    let (failed_index, failed) = store
        .evidence
        .iter()
        .enumerate()
        .find(|(_, e)| e.id == packet.failed_evidence)
        .context("MISSING_FAILURE_EVIDENCE")?;
    let (passed_index, verified) = store
        .evidence
        .iter()
        .enumerate()
        .find(|(_, e)| e.id == packet.passed_evidence)
        .context("MISSING_PASS_EVIDENCE")?;
    ensure!(
        failed_index < passed_index
            && scoped(failed, &packet.goal)
            && scoped(verified, &packet.goal)
            && fail(failed)
            && pass(verified)
            && failed.metadata.get("check_id") == verified.metadata.get("check_id")
            && verified
                .metadata
                .get("check_id")
                .is_some_and(|c| work.verification_requirements.contains(c))
            && failed.created_at <= finding.timestamp
            && failed.created_at >= work_event.timestamp
            && verified.created_at >= finding.timestamp,
        "EVIDENCE_MISMATCH"
    );
    ensure!(
        record.evidence_refs
            == vec![
                packet.failed_evidence.clone(),
                packet.passed_evidence.clone()
            ],
        "EVIDENCE_REFS_MISMATCH"
    );
    let previous = record.supersedes.as_ref().and_then(|id| {
        events.iter().find_map(|e| match &e.payload {
            Payload::CooperationKnowledgeRecorded { record: r } if &r.knowledge_id == id => {
                Some((e, r))
            }
            _ => None,
        })
    });
    if let Some((_, old)) = previous {
        let old_lesson = old.lesson.as_ref().context("PREVIOUS_LESSON_REQUIRED")?;
        let old_positive = old_lesson
            .positive
            .as_ref()
            .context("PREVIOUS_POSITIVE_REQUIRED")?;
        let mut normalized = positive.clone();
        normalized.reuse_evidence = old_positive.reuse_evidence.clone();
        ensure!(
            normalized == *old_positive
                && lesson.condition == old_lesson.condition
                && lesson.action == old_lesson.action
                && lesson.falsification_check == old_lesson.falsification_check
                && lesson.creator_worker_id == old_lesson.creator_worker_id
                && record.cooperation_id == old.cooperation_id
                && record.resource_fingerprint == old.resource_fingerprint
                && old_lesson.state != LessonState::Retired,
            "IMMUTABLE_KNOWLEDGE_SCOPE_OR_HISTORY"
        );
        ensure!(
            positive
                .reuse_evidence
                .starts_with(&old_positive.reuse_evidence),
            "REUSE_HISTORY_REWRITE"
        );
        let allowed = match old_lesson.state {
            LessonState::Candidate => matches!(
                lesson.state,
                LessonState::ValidatedLocal
                    | LessonState::Disputed
                    | LessonState::Stale
                    | LessonState::Retired
            ),
            LessonState::ValidatedLocal | LessonState::ReusedConfirmed => matches!(
                lesson.state,
                LessonState::ReusedConfirmed
                    | LessonState::Disputed
                    | LessonState::Stale
                    | LessonState::Retired
            ),
            LessonState::Disputed | LessonState::Stale => lesson.state == LessonState::Retired,
            LessonState::Retired | LessonState::Superseded => false,
        };
        ensure!(allowed, "INVALID_KNOWLEDGE_TRANSITION");
        ensure!(
            lesson.state == LessonState::ReusedConfirmed
                || positive.reuse_evidence == old_positive.reuse_evidence,
            "UNCONFIRMED_REUSE_COUNTER"
        );
    } else {
        ensure!(
            record.supersedes.is_none()
                && lesson.state == LessonState::Candidate
                && positive.reuse_evidence.is_empty(),
            "CANDIDATE_FIRST_REQUIRED"
        );
        let same_goal = events.iter().filter(|e| matches!(&e.payload, Payload::CooperationKnowledgeRecorded {record:r} if r.supersedes.is_none() && r.lesson.as_ref().is_some_and(|l| l.goal_id == packet.goal))).count();
        ensure!(
            same_goal < KnowledgeBudget::default().max_candidates_per_goal,
            "KNOWLEDGE_BUDGET_DENIED"
        );
    }
    for event in events {
        if let Payload::CooperationKnowledgeRecorded { record: old } = &event.payload {
            if old.supersedes.is_none()
                && record.supersedes.is_none()
                && old.lesson.as_ref().is_some_and(|l| {
                    l.component == lesson.component
                        && l.environment == lesson.environment
                        && l.condition == lesson.condition
                        && l.action == lesson.action
                })
            {
                anyhow::bail!("EXACT_KNOWLEDGE_DUPLICATE: reuse existing unit; no new knowledge");
            }
        }
    }
    if matches!(
        lesson.state,
        LessonState::ValidatedLocal | LessonState::ReusedConfirmed
    ) {
        let (candidate_event, prior) = previous.context("REVIEW_REQUIRES_PUBLISHED_CANDIDATE")?;
        let review = events
            .iter()
            .find(|e| Some(&e.event_id) == lesson.review_source.as_ref())
            .context("INDEPENDENT_REVIEW_REQUIRED")?;
        ensure!(
            review.sequence > candidate_event.sequence
                && review.intent_ref.as_deref() == Some(&packet.goal)
                && review.task_ref.as_deref() == Some(&packet.work)
                && review
                    .actor_worker_id
                    .as_ref()
                    .is_some_and(|w| w != &lesson.creator_worker_id && w != &packet.source_worker)
                && matches!(&review.payload, Payload::Finding {summary,..} if summary.contains(&prior.knowledge_id)),
            "INDEPENDENT_REVIEW_MISMATCH"
        );
        ensure!(
            !store.is_pass_stale(verified, &crate::compute_state_hash(root)?),
            "CURRENT_EVIDENCE_WINS: source PASS stale"
        );
        let resources = cooperation::project_cooperation_resources(events);
        let resource = resources
            .get(&record.cooperation_id)
            .context("UNKNOWN_RESOURCE")?;
        let (_, _, current, _) =
            cooperation::inspect_locator(root, &resource.locator, &resource.kind)?;
        ensure!(
            record.resource_fingerprint.is_some()
                && current.as_ref().map(|f| f.value.as_str())
                    == record.resource_fingerprint.as_deref(),
            "RESOURCE_FINGERPRINT_MISMATCH"
        );
        ensure!(
            resource
                .locator
                .replace('\\', "/")
                .ends_with(&packet.component),
            "RESOURCE_COMPONENT_MISMATCH"
        );
        if lesson.state == LessonState::ReusedConfirmed {
            let old = prior.lesson.as_ref().unwrap().positive.as_ref().unwrap();
            ensure!(
                positive.reuse_evidence.len() == old.reuse_evidence.len() + 1,
                "FRESH_REUSE_EVIDENCE_REQUIRED"
            );
            let id = positive.reuse_evidence.last().unwrap();
            ensure!(
                !old.reuse_evidence.contains(id) && id != &packet.passed_evidence,
                "DUPLICATE_REUSE_PROOF"
            );
            let reused = store
                .evidence
                .iter()
                .find(|e| &e.id == id)
                .context("MISSING_REUSE_EVIDENCE")?;
            ensure!(
                pass(reused)
                    && scoped(reused, &packet.goal)
                    && reused.created_at >= verified.created_at
                    && reused.metadata.get("check_id") == verified.metadata.get("check_id")
                    && !store.is_pass_stale(reused, &crate::compute_state_hash(root)?),
                "REUSE_NOT_CURRENT_SYSTEM_PASS"
            );
            ensure!(
                reuse_work(events, reused, packet).is_some(),
                "REUSE_REQUIRES_LATER_SCOPED_WORK"
            );
        } else {
            ensure!(
                positive.reuse_evidence
                    == prior
                        .lesson
                        .as_ref()
                        .unwrap()
                        .positive
                        .as_ref()
                        .unwrap()
                        .reuse_evidence,
                "UNCONFIRMED_REUSE_COUNTER"
            );
        }
    }
    Ok(())
}

/// Explicit trusted-local Operator import, normally from a separate deferred
/// extraction job. Does not invoke an AI or wait on optional extraction.
pub fn submit(
    root: &Path,
    record: Record,
    key: &str,
) -> Result<development::AppendDevelopmentEventResult> {
    ensure!(!key.is_empty(), "OPERATION_KEY_REQUIRED");
    ensure!(
        record.lesson.as_ref().is_some_and(|l| l.positive.is_some()),
        "POSITIVE_METADATA_REQUIRED"
    );
    cooperation::record_cooperation_knowledge(root, record, None, Some(key.into()))
}

/// Explicit optional job; failure/defer never changes Goal completion or Work.
/// Packet is persisted in the existing Team event, not in a new queue database.
pub fn defer(root: &Path, goal: &str, job: &str, key: &str) -> Result<Value> {
    let Some(packet) = packets(root, goal)?.into_iter().next() else {
        return Ok(json!({"status":"NO_SIGNAL", "blocks_goal":false}));
    };
    let bytes = serde_json::to_vec(&packet)?.len();
    let action = crate::team::TeamAction::QueueExtraction {
        goal_id: goal.into(),
        job_id: job.into(),
        finding_event_id: packet.finding.clone(),
        positive_packet: Some(packet),
    };
    match crate::team::broker_action(
        root,
        &crate::principal::CallerContext::trusted_operator(),
        action,
        key,
    ) {
        Ok(event) => Ok(
            json!({"status":"PENDING", "event_id":event.event.event_id,"packet_bytes":bytes,"blocks_goal":false,"synthesis":"NOT_RUN"}),
        ),
        Err(error) => {
            Ok(json!({"status":"DEFERRED","reason":error.to_string(),"blocks_goal":false}))
        }
    }
}

/// Read-only derived lifecycle. Current resource/Evidence wins over history.
pub fn query(
    root: &Path,
    component: &str,
    environment: &[String],
    independent: bool,
    candidates: bool,
) -> Result<Vec<Value>> {
    if independent {
        return Ok(vec![]);
    }
    if crate::project_identity::load_identity(root)?.is_none() {
        return Ok(vec![]);
    }
    let (ledger, _, _) = development::load_ledger_readonly(root)?;
    query_events(root, &ledger.events, component, environment, candidates)
}

pub(crate) fn query_events(
    root: &Path,
    events: &[DevelopmentEvent],
    component: &str,
    environment: &[String],
    candidates: bool,
) -> Result<Vec<Value>> {
    let resources = cooperation::project_cooperation_resources(events);
    let mut units = cooperation::project_cooperation_knowledge(events, &resources);
    units.sort_by_key(|u| u.revision);
    let store = EvidenceStore::load(root)?;
    let mut out = vec![];
    let mut inspected = 0;
    for unit in units.iter().rev().filter(|u| u.superseded_by.is_none()) {
        let Some(lesson) = &unit.record.lesson else {
            continue;
        };
        let Some(p) = &lesson.positive else { continue };
        if !component.is_empty() && lesson.component != component {
            continue;
        }
        if !environment.is_empty() && lesson.environment != environment {
            continue;
        }
        // Bound expensive current-resource probes even when stale candidates
        // outnumber returnable units. This is a recent-window query, not an index.
        if inspected == 12 {
            break;
        }
        inspected += 1;
        let contradictions = units
            .iter()
            .filter(|other| {
                other.superseded_by.is_none()
                    && other.record.knowledge_id != unit.record.knowledge_id
                    && other.record.lesson.as_ref().is_some_and(|l| {
                        l.positive.is_some()
                            && l.component == lesson.component
                            && l.environment == lesson.environment
                            && l.condition == lesson.condition
                            && l.action != lesson.action
                    })
            })
            .count();
        let contradiction = contradictions > 0;
        let overlaps: Vec<_> = units
            .iter()
            .filter(|other| {
                other.superseded_by.is_none()
                    && other.record.knowledge_id != unit.record.knowledge_id
                    && other.record.lesson.as_ref().is_some_and(|l| {
                        l.positive.is_some()
                            && l.component == lesson.component
                            && l.environment == lesson.environment
                            && l.condition != lesson.condition
                            && l.action == lesson.action
                    })
            })
            .take(3)
            .map(|u| &u.record.knowledge_id)
            .collect();
        let newer_fail = store
            .evidence
            .iter()
            .rposition(|e| e.id == p.packet.passed_evidence)
            .is_some_and(|index| {
                store.evidence[index + 1..].iter().any(|e| {
                    scoped(e, &lesson.goal_id)
                        && fail(e)
                        && store.evidence[index].metadata.get("check_id")
                            == e.metadata.get("check_id")
                })
            });
        let resource_stale = if let Some(resource) = resources.get(&unit.record.cooperation_id) {
            let (_, _, current, _) =
                cooperation::inspect_locator(root, &resource.locator, &resource.kind)?;
            current.as_ref().map(|f| &f.value) != unit.record.resource_fingerprint.as_ref()
        } else {
            true
        };
        let missing_evidence = !store
            .evidence
            .iter()
            .any(|e| e.id == p.packet.passed_evidence && scoped(e, &lesson.goal_id) && pass(e))
            || !store
                .evidence
                .iter()
                .any(|e| e.id == p.packet.failed_evidence && scoped(e, &lesson.goal_id) && fail(e));
        let state = if lesson.state == LessonState::Retired {
            LessonState::Retired
        } else if contradiction || newer_fail {
            LessonState::Disputed
        } else if unit.stale || resource_stale || missing_evidence {
            LessonState::Stale
        } else {
            lesson.state.clone()
        };
        if !candidates
            && !matches!(
                state,
                LessonState::ValidatedLocal | LessonState::ReusedConfirmed
            )
        {
            continue;
        }
        let mut ancestor = Some(unit.record.knowledge_id.as_str());
        let mut last_validated_revision = None;
        while let Some(id) = ancestor {
            let Some(old) = units.iter().find(|u| u.record.knowledge_id == id) else {
                break;
            };
            if old.record.lesson.as_ref().is_some_and(|l| {
                matches!(
                    l.state,
                    LessonState::ValidatedLocal | LessonState::ReusedConfirmed
                )
            }) {
                last_validated_revision = Some(old.revision);
                break;
            }
            ancestor = old.record.supersedes.as_deref();
        }
        let value = json!({"knowledge_id":unit.record.knowledge_id,"kind":p.kind,"title":p.title,"problem_pattern":lesson.condition,"symptoms":p.symptoms,"cause_if_established":p.cause_if_established,"resolution":lesson.action,"verification_guidance":lesson.falsification_check,"applicability":{"component":lesson.component,"environment":lesson.environment},"non_applicability":p.non_applicability,"related_components":[lesson.component],"source_goal":lesson.goal_id,"source_work":lesson.work_id,"source_findings":[lesson.finding_event_id],"source_events":lesson.source_event_ids,"source_evidence":unit.record.evidence_refs,"source_revision":p.packet.source_revision,"created_by":lesson.creator_worker_id,"review_provenance":lesson.review_source,"status":state,"revision":unit.revision,"supersedes":unit.record.supersedes,"support_count":1,"successful_reuse_count":p.reuse_evidence.len(),"contradiction_count":contradictions+usize::from(newer_fail),"probable_overlap_ids":overlaps,"last_validated_revision":last_validated_revision,"synthesis":p.synthesis,"trust":"HISTORICAL_KNOWLEDGE_NOT_CURRENT_EVIDENCE"});
        let mut value = value;
        value["reuse_evidence"] = json!(p.reuse_evidence);
        value["reuse_work"] = json!(p
            .reuse_evidence
            .iter()
            .filter_map(|id| store
                .evidence
                .iter()
                .find(|e| &e.id == id)
                .and_then(|e| reuse_work(events, e, &p.packet)))
            .collect::<Vec<_>>());
        if serde_json::to_vec(&out)?.len() + serde_json::to_vec(&value)?.len() > 8192 {
            break;
        }
        out.push(value);
        if out.len() == 3 {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
