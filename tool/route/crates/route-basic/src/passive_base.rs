//! Optional lesson metadata inside CooperationKnowledge; no second knowledge store.
//! Conservative gates reject unsupported scope and unverifiable assertions.
use crate::{
    cooperation::{CooperationKnowledgeRecord, EpistemicStatus},
    development::{DevelopmentEvent, DevelopmentEventPayload as Payload},
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LessonState {
    Candidate,
    ValidatedLocal,
    Stale,
    Disputed,
    Superseded,
    Retired,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub state: LessonState,
    pub goal_id: String,
    pub work_id: String,
    pub creator_worker_id: String,
    pub finding_event_id: String,
    pub source_event_ids: Vec<String>,
    pub review_source: Option<String>,
    pub component: String,
    /// Exact environment labels backed by source Finding, never universal claims.
    pub environment: Vec<String>,
    pub condition: String,
    pub action: String,
    pub falsification_check: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeBudget {
    pub max_pending_jobs: usize,
    pub max_jobs_per_goal: usize,
    pub retry_limit: usize,
    pub max_packet_bytes: usize,
    pub max_candidates_per_goal: usize,
}
impl Default for KnowledgeBudget {
    fn default() -> Self {
        Self {
            max_pending_jobs: 2,
            max_jobs_per_goal: 1,
            retry_limit: 0,
            max_packet_bytes: 8192,
            max_candidates_per_goal: 2,
        }
    }
}
pub(crate) fn validate_shape(record: &CooperationKnowledgeRecord) -> Result<()> {
    let Some(l) = &record.lesson else {
        return Ok(());
    };
    ensure!(
        serde_json::to_vec(l)?.len() <= 4096,
        "KNOWLEDGE_PACKET_TOO_LARGE"
    );
    for s in [
        &l.condition,
        &l.action,
        &l.falsification_check,
        &l.component,
    ] {
        ensure!(
            s.trim().len() >= 12 && s.len() <= 512,
            "KNOWLEDGE_GATE: actionable specific falsifiable fields required"
        );
        let s = s.to_ascii_lowercase();
        ensure!(
            ![
                "testing is important",
                "understand requirements before coding",
                "be careful with edge cases",
                "review code for quality"
            ]
            .iter()
            .any(|g| s.contains(g)),
            "KNOWLEDGE_GATE: generic advice"
        );
    }
    ensure!(
        !l.environment.is_empty()
            && l.environment.len() <= 4
            && l.source_event_ids.len() <= 8
            && !l.source_event_ids.is_empty(),
        "KNOWLEDGE_GATE: narrow sourced environment required"
    );
    ensure!(
        !record.evidence_refs.is_empty(),
        "KNOWLEDGE_GATE: Evidence required"
    );
    let statement = record.statement.to_ascii_lowercase();
    ensure!(
        ![
            "testing is important",
            "understand requirements before coding",
            "be careful with edge cases",
            "review code for quality"
        ]
        .iter()
        .any(|g| statement.contains(g)),
        "KNOWLEDGE_GATE: generic advice"
    );
    ensure!(
        l.state != LessonState::ValidatedLocal
            || (record.epistemic_status == EpistemicStatus::Observed && l.review_source.is_some()),
        "KNOWLEDGE_GATE: validation requires observed Evidence and review"
    );
    Ok(())
}
pub(crate) fn validate_transition(
    root: &Path,
    events: &[DevelopmentEvent],
    record: &CooperationKnowledgeRecord,
) -> Result<()> {
    let Some(l) = &record.lesson else {
        return Ok(());
    };
    validate_shape(record)?;
    let finding = events
        .iter()
        .find(|e| e.event_id == l.finding_event_id)
        .ok_or_else(|| anyhow::anyhow!("KNOWLEDGE_GATE: missing Finding"))?;
    let Payload::Finding { summary, .. } = &finding.payload else {
        anyhow::bail!("KNOWLEDGE_GATE: source is not a Finding")
    };
    ensure!(
        finding.intent_ref.as_deref() == Some(&l.goal_id),
        "KNOWLEDGE_GATE: foreign Goal"
    );
    ensure!(
        l.environment
            .iter()
            .all(|s| s.len() >= 3 && s.len() <= 128 && summary.contains(s))
            && summary.contains(&l.component),
        "KNOWLEDGE_GATE: scope exceeds observed source"
    );
    ensure!(events.iter().any(|e|matches!(&e.payload,Payload::Work {action:crate::work::WorkAction::ChildCreated {work}} if work.work_id==l.work_id && work.intent_ref==l.goal_id)),"KNOWLEDGE_GATE: missing Work");
    ensure!(events.iter().any(|e|matches!(&e.payload,Payload::WorkerRegistered {descriptor} if descriptor.worker_id==l.creator_worker_id)),"KNOWLEDGE_GATE: missing creator");
    ensure!(
        l.source_event_ids.iter().all(|id| events
            .iter()
            .any(|e| &e.event_id == id && e.intent_ref.as_deref() == Some(&l.goal_id))),
        "KNOWLEDGE_GATE: missing or foreign Events"
    );
    let store = crate::execution::EvidenceStore::load(root)?;
    ensure!(
        record
            .evidence_refs
            .iter()
            .all(|id| finding.evidence_refs.contains(id)
                && store.evidence.iter().any(|e| &e.id == id
                    && e.source == crate::execution::EvidenceSource::System
                    && (e.session_id == l.goal_id
                        || e.metadata.get("goal_id") == Some(&l.goal_id)))),
        "KNOWLEDGE_GATE: missing Goal-scoped System Evidence"
    );
    if let Some(review) = &l.review_source {
        ensure!(
            events.iter().any(|e| &e.event_id == review
                && e.intent_ref.as_deref() == Some(l.goal_id.as_str())
                && e.actor_worker_id
                    .as_ref()
                    .is_some_and(|id| id != &l.creator_worker_id)
                && matches!(e.payload, Payload::Finding { .. })),
            "KNOWLEDGE_GATE: independent review source required"
        );
    }
    let count=events.iter().filter(|e|matches!(&e.payload,Payload::CooperationKnowledgeRecorded {record:r} if r.lesson.as_ref().is_some_and(|old|old.goal_id==l.goal_id) && r.supersedes.is_none())).count();
    ensure!(
        record.supersedes.is_some() || count < KnowledgeBudget::default().max_candidates_per_goal,
        "KNOWLEDGE_BUDGET_DENIED: candidates_per_goal"
    );
    for e in events {
        if let Payload::CooperationKnowledgeRecorded { record: old } = &e.payload {
            if let Some(o) = &old.lesson {
                if record.supersedes.as_ref() == Some(&old.knowledge_id) {
                    ensure!(
                        o.goal_id == l.goal_id
                            && o.work_id == l.work_id
                            && o.component == l.component
                            && o.environment == l.environment,
                        "KNOWLEDGE_GATE: scope widening requires new independent source unit"
                    );
                    ensure!(
                        o.state != LessonState::Retired,
                        "KNOWLEDGE_GATE: retired unit cannot be revived"
                    );
                }
                if o.component == l.component
                    && o.environment == l.environment
                    && o.condition == l.condition
                {
                    ensure!(record.supersedes.as_ref()==Some(&old.knowledge_id) || events.iter().any(|e|matches!(&e.payload,Payload::CooperationKnowledgeRecorded {record:r} if r.supersedes.as_ref()==Some(&old.knowledge_id))),"KNOWLEDGE_DUPLICATE: explicitly supersede current unit");
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> CooperationKnowledgeRecord {
        CooperationKnowledgeRecord {
            knowledge_id:"validator-fixture-only".into(),cooperation_id:"fixture-resource".into(),
            statement:"Under restricted Windows path access, retry with an explicitly approved local boundary".into(),
            epistemic_status:EpistemicStatus::Inferred,provenance:"synthetic validator fixture, not project knowledge".into(),
            capability_refs:vec![],source_refs:vec![],evidence_refs:vec!["missing-evidence".into()],observed_at:0,resource_fingerprint:None,supersedes:None,
            lesson:Some(Lesson {state:LessonState::Candidate,goal_id:"fixture-goal".into(),work_id:"fixture-work".into(),creator_worker_id:"fixture-creator".into(),finding_event_id:"missing-finding".into(),source_event_ids:vec!["missing-event".into()],review_source:None,component:"path canonicalization".into(),environment:vec!["Windows".into(),"restricted sandbox".into()],condition:"When canonicalization returns access denied in restricted sandbox".into(),action:"Request an explicit local authority boundary and retry canonicalization".into(),falsification_check:"Repeat canonicalization under the same restricted sandbox and compare exit code".into()})
        }
    }
    #[test]
    fn generic_missing_sources_and_fake_validation_rejected() {
        let mut f = fixture();
        assert!(validate_shape(&f).is_ok());
        f.statement = "Testing is important.".into();
        assert!(validate_shape(&f).is_err());
        f = fixture();
        f.evidence_refs.clear();
        assert!(validate_shape(&f).is_err());
        f = fixture();
        f.lesson.as_mut().unwrap().state = LessonState::ValidatedLocal;
        assert!(validate_shape(&f).is_err());
        let tmp = tempfile::tempdir().unwrap();
        assert!(validate_transition(tmp.path(), &[], &fixture()).is_err());
    }
    #[test]
    fn blind_retrieval_never_opens_state_or_returns_conclusions() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(retrieve(
            tmp.path(),
            "path canonicalization",
            &["Windows".into()],
            true
        )
        .unwrap()
        .is_empty());
        assert!(!tmp.path().join(".route").exists());
    }
}
/// Ordinary successful work does not trigger extraction. This V1 recognizes only
/// explicit reusable markings in published Findings; no free-text historical mining.
pub fn detect(root: &Path, goal: &str) -> Result<Vec<Value>> {
    let (ledger, _, _) = crate::development::load_ledger_readonly(root)?;
    let evidence = crate::execution::EvidenceStore::load(root)?;
    let packets:Vec<_>=ledger.events.iter().rev().filter(|e|e.intent_ref.as_deref()==Some(goal) && supported_source(&evidence,e)).filter_map(|e|match &e.payload {
        Payload::Finding {summary,source_refs} if summary.starts_with("REUSABLE:") && !e.evidence_refs.is_empty()=>
            Some(json!({"goal_id":goal,"work_id":e.task_ref,"finding_event_id":e.event_id,"source_event_ids":[e.event_id],"summary":crate::sidecar::safe_text(summary),"source_refs":source_refs.iter().take(8).collect::<Vec<_>>(),"evidence_refs":e.evidence_refs.iter().take(4).collect::<Vec<_>>(),"evidence":evidence.evidence.iter().filter(|proof|e.evidence_refs.contains(&proof.id)).take(4).map(|proof|json!({"evidence_id":proof.id,"kind":proof.kind,"source":proof.source,"check_id":proof.metadata.get("check_id").map(|s|crate::sidecar::safe_text(s))})).collect::<Vec<_>>(),"trigger":"EXPLICIT_REUSABLE_FINDING","verification":"UNVALIDATED_EXTRACTION_INPUT"})),
        _=>None }).take(KnowledgeBudget::default().max_jobs_per_goal).collect();
    ensure!(
        serde_json::to_vec(&packets)?.len() <= KnowledgeBudget::default().max_packet_bytes,
        "KNOWLEDGE_PACKET_TOO_LARGE"
    );
    Ok(packets)
}
pub(crate) fn supported_source(
    store: &crate::execution::EvidenceStore,
    event: &DevelopmentEvent,
) -> bool {
    !event.evidence_refs.is_empty()
        && event.evidence_refs.iter().all(|id| {
            store.evidence.iter().any(|e| {
                &e.id == id
                    && e.source == crate::EvidenceSource::System
                    && (Some(e.session_id.as_str()) == event.intent_ref.as_deref()
                        || e.metadata.get("goal_id") == event.intent_ref.as_ref())
            })
        })
}
pub fn retrieve(
    root: &Path,
    component: &str,
    environment: &[String],
    independent_first: bool,
) -> Result<Vec<Value>> {
    if independent_first {
        return Ok(vec![]);
    }
    let units = crate::cooperation::cooperation_knowledge(root, None)?;
    let mut out = vec![];
    for k in units
        .iter()
        .rev()
        .filter(|k| !k.stale && k.superseded_by.is_none())
    {
        let Some(l) = &k.record.lesson else { continue };
        if l.state != LessonState::ValidatedLocal
            || l.component != component
            || &l.environment != environment
        {
            continue;
        }
        let value = json!({"knowledge_id":k.record.knowledge_id,"revision":k.revision,"condition":l.condition,"action":l.action,"check":l.falsification_check,"state":l.state,"trust":"KNOWLEDGE_NOT_CURRENT_EVIDENCE"});
        if serde_json::to_vec(&out)?.len() + serde_json::to_vec(&value)?.len() > 4096 {
            break;
        }
        out.push(value);
        if out.len() == 3 {
            break;
        }
    }
    Ok(out)
}
