<!-- ROUTE:BEGIN -->
<!-- route-context-fingerprint: 416f9a556ca5a4d745b8c388bc9dcba0a21d930a8d0932c0375e6ab4d48dd0ba -->
<!-- route-constitution-hash: 14e4b712d8563e6968762b942a128806bd07d978974fb2b630536af8ed6ac434 -->
<!-- route-protocol-hash: 3ff994a69abb2949f753556429f8cd126eb3d13ba3b0113ccf5d9b09fd50616c -->
<!-- route-reference-hash: 4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945 -->

# Task-Scoped Effective Context

Task: `session2-complete-tag-notes-checkbox`

Project: C:/Users/longq/Desktop/route (1)/tests/fixtures/route-conformance/F6_abandoned

Profile: default

---

## Constitution

_version=1, created_at=1786856520112_

# Constitution

Stable development principles for this project.
AI assistants MUST read this document before making changes.

## Data safety

- Never delete user data without explicit confirmation.
- Rollback and undo are preferred over destructive rewrite.
- .route-basic/ and .route/ are internal data — treat as append-only.

## User control

- Conflicts between AI suggestions and existing logic go to the user.
- No silent behaviour change. Every setting change is explicit.

## Reversibility

- Every destructive change must have a corresponding restore path.
- Prefer checkpointing the project state before a large rewrite.

---

## Protocol

_version=1, revision=1, updated_at=1786856520115_

# Protocol

Execution playbook for AI assistants working on this project.

## Workflow

1. Read the Constitution, this Protocol, and the latest Reference registry.
2. Understand the task. If the scope is ambiguous, ask the user before touching files.
3. Create a **snapshot** (route commit -m 'pre-<task>') before large changes.
4. Make the change. Keep commits small and well-described.
5. Run tests via `route task exec SESSION -- cargo test` (or `cargo test` directly).
6. Verify with `route task verify SESSION`.
7. If something breaks mid-flight, use `route rollback <snapshot_id>` rather than hand-rolling back.

## Review gates

- Schema / API changes: always flag to user for review.
- Destructive file moves or deletions: snapshot first.
- Anything marked in Constitution as user-only: escalate.

---

## Relevant References

_(no relevant references found for this task)_

---

## Route Feedback Contract

You (the AI) may report structured observations about what worked or did not work during this session. Use the following format:

```
route_feedback_candidate:
  task: "<brief description of the task>"
  result: "<what happened — accept/reject/error/success>"
  observation: "<what you observed about the outcome>"
  evidence: "<supporting detail: file changed, test passed, etc.>"
  context_hash: "<the context fingerprint above>"
  selected_reference_ids: "<comma-separated ids of selected references>"
  agent_roles_used: "<roles used, if any>"
  verification_result: "<pass/fail/unknown>"
```

Constraints:
- You MUST NOT generate "user preference facts" or claim user intent.
- You MUST NOT self-apply LearningProposals.
- Each candidate is just a suggestion. Route Engine validates the
  source and records it as an ExperienceEvent after verification.
- Single observations never become rules. Only multi-evidence
  aggregation (via `route learn analyze`) can produce a proposal.
- You may also submit structured memory observations:

```
memory_candidate:
  kind: "convention" | "decision" | "risk" | "open_question"
  content: "<concise statement of the memory item>"
  source_ids: ["<commit-id>", "<session-id>"]
  confidence: 0.85
```

  Memory candidates are proposals only — never auto-applied. Route Engine validates and may promote them.

---

_End of Effective Development Context._
<!-- ROUTE:END -->
