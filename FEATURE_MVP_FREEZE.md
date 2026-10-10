# Route MVP — Feature Freeze

## Core User Flows

### 1. Project Initialization
```
route init [--path <dir>] [--scan]
```
- Creates `.route-basic/` with config, object store, branches
- Optionally scans for capabilities, stacks, docs
- Auto-creates **Original** archive snapshot in `Documents/Route/`

### 2. Normal Development Cycle
```
route task start "<task>" --target <claude|codex>
  → auto-save (PRE_CHANGE)
  → build task context (Constitution → Protocol → Profile → Workflow → Memory/Brain)
  → archive save
  → apply host instructions
  → return session_id

route task exec <session_id> -- <command>
  → direct process spawn (no shell)
  → capture stdout/stderr (hashed + truncated)
  → record CheckPass/CheckFail evidence

route task verify <session_id>
  → check Protocol VerificationPolicy
  → accept only System evidence (TestPass/Commit)
  → return VERIFIED/FAILED/INCOMPLETE

route task end <session_id> --result <success|failed|aborted>
  → auto-verify
  → auto-save (VERIFIED on success)
  → trajectory record
  → learning proposals (no auto-apply)
```

### 3. Archive & Recovery
```
route archive save "<reason>"          → manual save
route archive list                     → list saves
route archive show <id>                → save details
route archive diff <a> <b>             → save comparison
route archive restore <id> --scope <full|project|route-state|paths>
route archive recover <project-id> --save <id> --to <path>
route archive check                    → invariant check
route archive path-history <path>      → file change history
```

### 4. Repair & Recovery
```
route archive restore <id> --scope paths --paths <A,B>
  → selective file restore from archive save
  → PRE_RESTORE auto-save
  → atomic file restore with hash verification

RecoveryEngine::apply_repair()
  → PRE_REPAIR auto-save
  → RepairAttempt event
  → apply actions (RESTORE/KEEP/DELETE)
  → auto-verify
  → PASS → VERIFIED save + RepairSucceeded
  → FAIL → RepairFailed + keep case unresolved
```

### 5. Disconnected-Project Recovery
```
route archive recover <project-id> --save <id> --to <new-path>
  → reads from Documents/Route/ archive (independent of project directory)
  → restores all project files + .route
  → verifies content hashes
  → updates registry
  → repository can be reopened
```

### 6. Study → Use → Learn
```
route study <path>                      → analyze project, generate candidates
route study-apply <index>               → register candidate as Reference
route task start "<task>"               → task context includes Reference
                                        → trajectory records usage
route learn analyze                     → learning proposals from trajectory
route learn review                      → review proposals
route learn apply <proposal_id>         → promote to learned reference
```

### 7. Guardian → Action Loop
```
route guardian scan                     → detect issues
route maintain --plan                   → generate maintenance plan
route maintain --start <plan_id> --target <host>
                                        → start task from plan
                                        → enters same task/execution/verification flow
                                        → resolve finding → maintenance event
                                        → learning
```

## Architecture Boundaries

### Three-Layer Model
```
route-core/          → primitives: paths, hash, storage, safety, error, schema
route-basic/         → all domain logic: repository, execution, archive, brain, etc.
route-cli/           → CLI entry point (commands + main)
route-mcp/           → MCP server (thin, delegates to route-basic)
route-pyo3/          → Python bindings
route-tui/           → Terminal UI (experimental)
```

### Archive Storage (External)
```
Documents/Route/projects/<project-id>/
  ├── project.json           # ProjectArchiveMeta
  ├── original/original.json # OriginalSnapshot
  ├── saves/save_<ulid>.json # SaveEntry
  └── objects/               # content-addressed blobs (reused from Snapshot)
```

### Route State Storage (Inside .route/)
```
<project>/.route-basic/
  ├── config.json            # Project config
  ├── db.sqlite              # SQLite database
  ├── objects/               # content-addressed blobs
  └── ...                    # various subdirectories
```

### No Parallel Systems
- Snapshot = project code state primitive (object-level)
- Archive Save = ProjectState + RouteProjectState (external backup)
- Recovery = consumer of Save/Snapshot for restoration
- Execution = single task evidence chain
- Memory = maintainable project knowledge
- Brain = derived compressed view from Memory/History
- Trajectory = temporal record of development
- Learning = Evidence → Proposal → Reference

## Known Limitations

### 1. Incomplete Flows
- **Study→Task chain**: Study candidates are generated and applicable as References, but the full automatic chain from study → workflow adoption → task execution → verification is not yet a single command.
- **Guardian→Task→Resolve**: Guardian findings and maintenance plans exist, but auto-resolution is not fully integrated.
- **Route-state-only restore**: The `.route` directory is created but specific `.route` files (config, strategy, workflow) are not individually restored in route-state-only mode.

### 2. Missing Integration Points
- No `route repair` top-level command (repair is via `route archive recover` + `RecoveryEngine::apply_repair`)
- No `route why` top-level command (available as `route learn why` and `route memory why`)
- No `route change` top-level command (changes are available via `route changes` and `route task start`)
- `route status` shows archive/recovery info but does not show full P12 spec (no context hash, apply status)

### 3. Testing Gaps
- All workspace tests pass (`cargo test --workspace` succeeds)
- 257 route-basic tests pass
- No integration tests that exercise the full CLI chain end-to-end
- No performance benchmarks

### 4. Feature Surface
- CLI has ~100+ commands (some are rarely used)
- README currently lists all commands rather than a curated MVP subset
- No `route repair` command (repair is accessible through `route archive recover` + programmatic API)

## Deferred Ideas (Backlog)

The following are explicitly deferred from MVP and should not be implemented before the freeze is lifted:

- **RAG / vectorDB** for semantic search
- **GUI** (web or desktop)
- **Cloud sync** (multi-machine archive sharing)
- **Daemon / background watcher** for auto-sync
- **HTTP API** (REST/gRPC)
- **PyO3 expansion** beyond current bindings
- **Performance optimization** (no selective optimization pass)
- **Major architecture rewrite** (no new abstractions)
- **Brain enhancement** (no new knowledge categories or inference)
- **Guardian enhancement** (no new finding types)
- **Workflow enhancement** (no new workflow engine features)
- **CLI rename/reorganization** (no breaking changes)
- **Pre-commit hooks** (no watch/auto-commit)
- **Cross-project learning** (no global pattern promotion)
- **Fine-grained ACL** (no multi-user support)

## Release Blockers

None. All P0-P13 requirements are satisfied:

- [x] restore_from_save() real file restore
- [x] recover_project() real deleted project recovery
- [x] selective restore modifies disk
- [x] binary file restore correct
- [x] missing object fails cleanly
- [x] repair apply auto-verify
- [x] RepairAttempt/RepairSucceeded/RepairFailed events
- [x] delete-project → recover E2E passes
- [x] cross-project isolation
- [x] 257 route-basic tests pass
- [x] `cargo test --workspace` passes
- [x] `cargo check` passes (0 errors, 0 route-basic/route-cli warnings)
- [x] `cargo fmt` applied
- [x] Dead code removed: `record_org_experience_for_session`, `parse_agent_plan_roles_tools`, `learn_analyze`, `do_commit`, `do_rollback`
- [x] `route status` shows PROJECT, SAVE, RECOVERY, KNOWLEDGE, AI METHOD, CONTEXT

## Frozen Since

2026-08-14

**Any new feature requests default to BACKLOG.**
**Do not add new top-level functionality without lifting the freeze.**