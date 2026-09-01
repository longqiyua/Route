//! Route basic mode — graph + tree mindmap, edge-centric commits.
//!
//! Innovation: commit metadata lives on **edges** (between snapshot nodes),
//! not on nodes. Nodes are pure file states. This enables N-N relationships
//! and path-attached text annotations on edges.

pub mod adapter;
pub mod agent_compiler;
pub mod agent_org;
pub mod ai_conflict;
pub mod backup;
pub mod binding;
pub mod bootstrap;
pub mod brain;
pub mod brief;
pub mod campaign;
pub mod capability;
pub mod constitutive;
pub mod curator;
pub mod development;
pub mod discovery;
pub mod drift;
pub mod emergence;
pub mod evolution;
pub mod execution;
pub mod experiment;
pub mod export;
pub mod fail_inject;
pub mod failure;
pub mod game_save;
pub mod goal;
pub mod guardian;
pub mod health;
pub mod idea;
pub mod impact;
pub mod index;
pub mod knowledge_map;
pub mod learn;
pub mod loop_detector;
pub mod maintainer;
pub mod memory;
pub mod models;
pub mod next_action;
pub mod pack;
pub mod pattern;
pub mod plan;
pub mod principle;
pub mod profile;
pub mod project_identity;
pub mod repository;
pub mod roadmap;
pub mod role_template;
pub mod route_history;
pub mod savepoint;
pub mod self_archive;
pub mod sop;
pub mod strategy;
pub mod study;
pub mod trajectory;
pub mod transaction;
pub mod workflow;

pub use adapter::{
    apply_context, check_status, verify_target, ApplyLineage, ApplyRecord, ApplyStatus,
    ApplyTarget, VerifyInfo, ROUTE_BEGIN, ROUTE_END,
};
pub use agent_compiler::{
    compile_plan, compile_to_host, render_plan, AgentPlan, AgentSpec, CompilerInput,
    HostCapabilities,
};
pub use agent_org::{
    agent_org_dir, agent_org_path, classify_task_pattern, OrganizationExperience,
    OrganizationExperienceStore, RecallSignal,
};
pub use ai_conflict::{
    list_verdicts as list_conflict_verdicts, parse_body as parse_conflict_body,
    record_verdict as record_conflict_verdict, report_for_commit as conflict_report_for_commit,
    AiConflict, AiConflictReport, AiConflictVerdict,
};
pub use binding::{binding_dir, binding_path, BindingStore, TaskBinding};
pub use bootstrap::{bootstrap_scan, BootstrapMemoryCandidate, BootstrapResult};
pub use brain::{
    apply_brain_proposals, archived_items, brain_at_savepoint, brain_brief, brain_doctor,
    can_promote, compact_brain, compress_knowledge, current_items, detect_contradictions,
    diff_brain, expand_item, format_brain, format_brain_brief, format_brain_diff, format_conflicts,
    format_doctor_report, format_host_context, format_relevant_knowledge, historical_items,
    refresh_brain, task_brain, validate_project_boundary, BrainBrief, BrainDiff, BrainDoctorReport,
    BrainItem, BrainItemStatus, BrainKnowledge, BrainProposal, BrainStore,
    BrainStore as BrainKnowledgeStore, KnowledgeConflict, ProjectBrain, RelevantKnowledge,
};
pub use brief::{
    format_brief, format_handoff, generate_brief, generate_handoff, HandoffDocument, ProjectBrief,
};
pub use capability::{
    capability_dir, capability_registry_path, Capability, CapabilityKind, CapabilityLevel,
    CapabilityRegistry, IntegrateReport, PromoteSkillReport,
};
pub use constitutive::{
    apply_proposal, archive_current_context, build_context, build_context_explain,
    build_memory_context, classify_source, constitution_path, context_archive_dir, context_dir,
    context_history_dir, curator_analyze, curator_refresh, dot_dir, effective_context,
    effective_context_fingerprint, import_reference_source, inject_learned_experiences,
    protocol_path, reference_dir, registry_path, replay_context, task_scoped_context, AgentMode,
    AgentPolicy, AgentRole, ComponentDelta, Constitution, ContextBudget, ContextDiff,
    ContextExplainResult, ContextHistoryManifest, ContextSnapshot, CuratorReport,
    ImportedReference, LearnedMeta, Origin, PartialHistory, ProposalAction, Protocol,
    ReferenceEntry, ReferenceEntryBuilder, ReferenceProposal, ReferenceProposalStore,
    ReferenceRegistry, ReferenceSelector, ReferenceSemanticView, ReferenceType, RefreshResult,
    ScoredReference, SelectionReason, TaskContextResult, WriteOrigin, WritePermissionError,
    ROUTE_DOT_DIR,
};
pub use curator::{
    curator_capabilities, render_curator_context, AgentCompiler, CuratorCapabilities, CuratorRole,
    MemoryCurator, ReferenceCurator,
};
pub use development::{
    append_development_event, development_ledger_path, development_state_owner,
    development_view_is_stale, global_development_revision, query_development_events,
    register_worker, send_worker_message, shared_development_state, update_worker_presence,
    worker_descriptors, worker_presences, AppendDevelopmentEventResult, DevelopmentEvent,
    DevelopmentEventDraft, DevelopmentEventPage, DevelopmentEventPayload, DevelopmentEventType,
    GitWorkspaceSummary, KnownGoodSummary, SessionSummary, SharedDevelopmentState,
    WorkerDescriptor, WorkerMessage, WorkerMessageInput, WorkerMessageType, WorkerMetadata,
    WorkerPresence, WorkerPresenceInput, WorkerStatus,
};
pub use discovery::{
    discovery_dir, discovery_proposals_path, format_discovery_proposal, scan_project,
    DiscoveryItem, DiscoveryProposal, DiscoveryStore,
};
pub use drift::{format_drift, scan_drift, DriftItem, DriftScanResult};
pub use execution::{
    auto_analyze_after_session, begin_session, capabilities, check_execution_ledger,
    compute_state_hash, end_session, exec_command, force_end_session,
    generate_learning_from_session, ingest_host_report, record_commit_evidence,
    record_commit_evidence_dedup, record_experience_event_with_session, record_proposal_evidence,
    record_rollback_evidence, record_rollback_evidence_dedup, record_snapshot_evidence,
    record_test_evidence, render_host_contract, replay_session, resume_session, session_status,
    show_session, start_task_session, verify_session, AppResult, CausalEntry, CommandEvidence,
    ConsistencyFinding, DriftStatus, Evidence, EvidenceKind, EvidenceSource, EvidenceStore,
    ExecutionSession, HostAction, HostReport, ReplayResult, RequestIdStore, ResumeResult,
    RouteCapabilities, SessionAudit, SessionStatus, SessionStore, StartTaskResult,
    VerificationPolicy, VerifyResult,
};
pub use experiment::{
    experiment_dir, experiment_path, ExperimentRecord, ExperimentStore, StrategyRanking,
    StrategyStats,
};
pub use export::exporters::DefaultExporters;
pub use export::{ExportContext, ExportFormat, Exporter};
pub use failure::{failure_dir, failure_path, format_failure_case, FailureCase, FailureLibrary};
pub use game_save::{
    archive_root,
    auto_save_before_repair,
    auto_save_before_rollback,
    auto_save_manual,
    // P1: Auto-save triggers
    auto_save_task_start,
    auto_save_verification_pass,
    check_invariants,
    create_save,
    delete_project_archive,
    delete_save,
    diff_saves,
    format_invariant_check,
    format_recovery_case,
    format_recovery_options,
    format_repair_plan,
    format_save_diff,
    format_save_summary,
    generate_recovery_case,
    get_recovery_options,
    init_project_archive,
    list_recovery_cases,
    list_saves,
    load_original,
    load_recovery_case,
    load_save,
    objects_dir,
    original_dir,
    path_history,
    project_archive_dir,
    project_id_from_path,
    projects_dir,
    record_repair_to_trajectory,
    recover_project,
    restore_from_save,
    saves_dir,
    try_self_heal,
    verify_no_recursive_save,
    ArchiveRestoreScope,
    AutoSaveReason,
    InvariantCheck,
    InvariantCheckResult,
    // P5-P11: Recovery system
    KnownGoodState,
    KnownGoodTracker,
    OriginalSnapshot,
    ProjectArchiveMeta,
    ProjectRegistry,
    ProjectState,
    RecoveryCase,
    RecoveryEngine,
    RecoveryLevel,
    RecoveryOptions,
    RecoveryPolicy,
    RecoveryTrigger,
    RegistryEntry,
    RepairAction,
    RepairActionKind,
    RepairApplyResult,
    RepairJudgment,
    RepairJudgmentRequest,
    RepairPlan,
    RestoreResult,
    RestoreState,
    RouteProjectState,
    SaveDiff,
    SaveEntry,
    SavePointers,
    SaveSummary,
    SelfHealOperation,
};
pub use goal::{format_goal, goal_dir, goal_path, Goal, GoalStatus, GoalStore};
pub use guardian::{
    format_finding, guardian_dir, guardian_events_path, guardian_findings_path, guardian_scan,
    FindingSeverity, FindingStatus, GuardianFinding, GuardianFindingsStore, GuardianScanResult,
    MaintenanceEvent, MaintenanceEventStore,
};
pub use health::{
    format_health_snapshot, health_dir, health_path, HealthAction, HealthRisk, HealthSignal,
    HealthSignalItem, HealthStore, ProjectHealthSnapshot,
};
pub use idea::{format_idea, idea_dir, idea_path, Idea, IdeaStatus, IdeaStore};
pub use impact::{
    analyze_impact, format_impact_report, impact_dir, impact_path, ImpactFinding, ImpactReport,
    ImpactStore,
};
pub use index::{build_index, write_index, IndexBranch, IndexCommit, IndexFile, RouteIndex};
pub use knowledge_map::{
    format_node, knowledge_map_dir, knowledge_map_path, EdgeKind, KnowledgeEdge, KnowledgeMap,
    KnowledgeNode, NodeKind,
};
pub use learn::{
    analyze_events, apply_learning_proposal, build_audit, recompute_confidence,
    reject_learning_proposal, render_feedback_contract, AuditEntry, AuditKind, EventKind,
    ExperienceEvent, ExperienceStore, LearnScope, LearningAudit, LearningProposal,
    LearningProposalStore, ProposalStatus, ScoredLearnedExperience,
};
pub use loop_detector::{format_open_loops, scan_open_loops, OpenLoop, OpenLoopResult};
pub use maintainer::{
    format_maintainer_plan, generate_maintainer_plan, MaintainerPlan, MaintainerStore,
    MaintainerTask,
};
pub use memory::{
    history_path, memory_dir, memory_path, MemoryItem, MemoryItemKind, MemoryStore,
    MemoryWhyExplanation, MemoryWhyLink, ProjectMemory,
};
pub use models::{
    AiPrompt, Branch, BranchKind, Commit, CommitKind, CommitPathAnnotation, DiffSummary,
    ManifestEntry, RepoConfig, Snapshot, SnapshotWithBranch, Tag, TrackConfig, TrackOs,
};
pub use next_action::{
    format_action_proposal, generate_next_actions, ActionProposal, NextActionStore,
};
pub use pack::{format_pack, pack_dir, pack_index_path, CapabilityPack, PackIndex};
pub use pattern::{pattern_dir, pattern_path, PatternProposal, PatternStore, ReusablePattern};
pub use plan::{
    compare_plans, plan_task, render_comparison, render_plan as render_plan_preview,
    CounterfactualPlan, PlanComparison,
};
pub use principle::{
    format_principle, principle_dir, principle_path, PrincipleCandidate, PrincipleStore,
    PrincipleTarget,
};
pub use profile::{
    active_profile_path, builtin_profiles, init_profile_store, profile_path, ProfileStore,
    ProjectProfile,
};
pub use project_identity::{
    attach as attach_project_identity, discover_project_root, ensure_identity, identity_path,
    load_identity, ProjectIdentity,
};
pub use repository::{
    BasicRepository, CommitDiffEntry, CommitOptions, CreateBranchOptions, FileRevision,
    RollbackOptions, SnapshotDiffEntry, VerifyFinding, VerifyOptions, VerifyReport, VerifySeverity,
    WorkingFileStatus,
};
pub use roadmap::{Roadmap, RoadmapNode};
pub use role_template::{
    format_role_template, role_template_dir, role_template_path, RoleTemplate, RoleTemplateStore,
};
pub use route_history::{
    append_route_history, find_route_project_root, load_route_history, route_history_dir,
    route_history_head_path, route_history_path, verify_route_history, RouteHistoryEvent,
    RouteHistoryVerification,
};
pub use savepoint::{
    create_memory_snapshot, create_savepoint, delete_savepoint, diff_savepoints, execute_restore,
    format_diff, format_savepoint, preview_restore, savepoint_dir, savepoint_path,
    DevelopmentSavepoint, RestorePreview, RestoreScope, SavepointDiff, SavepointStore,
};
pub use strategy::{strategy_dir, strategy_path, StrategySnapshot, StrategyStore};
pub use study::{
    format_report, library_dir, library_path, load_report, save_report, study_library_add,
    study_library_compare, study_library_diff, study_library_get, study_library_load,
    study_library_remove, study_library_save, study_project, study_report_path, StudyCandidate,
    StudyDecision, StudyLibrary, StudyPattern, StudyRecord, StudyReport, StudySkill, StudyWorkflow,
};
pub use trajectory::{
    analyze_agent_plans, analyze_retention, analyze_reversal, complete_trajectory,
    consolidate_memory_candidates, create_trajectory, diff_trajectories, distill_patterns,
    explain_learning, format_dashboard, format_trajectory, format_trajectory_diff,
    generate_learning_dashboard, generate_strategy_learning_proposals,
    generate_workflow_evolution_proposals, promote_to_cross_project, record_decision,
    record_failure, record_rollback, AgentPlanInsight, AutoSaveState, CrossProjectPattern,
    DevelopmentTrajectory, DistilledPattern, LearningAction, LearningActionStore,
    LearningDashboard, LearningExplanation, MemoryCandidate, RetentionSignal, ReversalAnalysis,
    StrategyLearningProposal, TrajectoryDiff, TrajectoryStore, TrajectoryWorkflowProposal,
    TrajectoryWorkflowProposalStore, AUTO_SAVE_HIGH_RISK, AUTO_SAVE_MANUAL, AUTO_SAVE_ROLLBACK,
    AUTO_SAVE_STRATEGY_SWITCH, AUTO_SAVE_TASK_START, AUTO_SAVE_VERIFIED,
};
pub use transaction::{ConvIntent, TargetFiles, TransactionIntent, TransactionJournal, TxState};
pub use workflow::{
    generate_evolve_proposal, workflow_dir, workflow_from_candidate, workflow_path,
    workflow_proposal_dir, workflow_proposal_path, WorkflowChangeProposal, WorkflowDefinition,
    WorkflowProposalStore, WorkflowStep, WorkflowStore,
};
