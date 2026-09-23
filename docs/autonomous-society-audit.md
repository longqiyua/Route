# Autonomous Society Dogfood I — pre-implementation audit

Status: autonomous phase PARTIAL, not autonomous-session certification.
Identity-spoofing blocker: **CLOSED** by Worker Principal Binding I (2026-09-23).
Autonomous A/B/C remain **NOT_RUN**, ready for the next authorized batch.

## Identity-boundary closure

[Worker Principal Binding I](worker-principal-binding.md) now separates explicit
Operator administration from host-bound Worker requests. Project-scoped random
credentials are verified against durable/revocable bindings; Worker actor fields
are derived, not selected by payload. Real process attacks A–J, JSONL revocation,
restart, credential rotation, concurrent issue and keyed recovery pass. Full
workspace regression: 617 passed, 2 existing subprocess fixture ignores, exit 0.
No same-OS-user hostile-process isolation or remote authentication is claimed.
No remaining autonomous work/claim/integration scenarios were implemented here.

## Historical pre-implementation audit

The following finding and proposed path describe the original baseline, not the
post-fix authority surface. Its historical observation has not been rewritten.

Baseline: `6d7697089077cf224f15e72bab63605aeab9b959`, fruit; fetched
origin/fruit divergence 0/0. Three preexisting Yuich deletions remain untouched.
No runtime, Institution, Worker or authority changes were made in that initial audit.

## Existing work model: ten answers from code

| Question | Current implementation and missing boundary |
|---|---|
| What represents available work? | `ExecutionSession` in execution.rs; RPC intent refs alias session IDs. Institution context exposes at most 16 active session IDs, not dependency-aware child work. |
| Can a Worker claim work? | Presence may reference a task/session; it is an observation, not a validated work claim. `TaskBinding` binds resources to a session, not Workers to exclusive work. |
| Is a claim durable? | Presence/events are durable; no canonical claim lifecycle exists. A presence reference must not be relabeled a claim. |
| Simultaneous claims? | The ledger provides locked, revisioned atomic transitions and deduplication; there is no incompatible-work-claim validator yet. |
| Crash/release/reassignment? | Worker last-observed presence and session/history persist. No durable release/reassignable claim transition exists; wall-clock absence is not proof of death. |
| Child work under Intent? | ExecutionSession has no parent Intent/child/dependency/scope authorization fields. `campaign_id` references an evolution campaign, not a general parent Intent. Extend existing session semantics only after defining a single canonical transition owner. |
| Request help? | Existing `WorkerMessageType::HelpRequest`, Question, ReviewRequest and Handoff; use shared event history, no dispatcher required. |
| Volunteer help? | Existing HelpOffer/Answer/ReviewFinding permit communications. They do not claim work or authorize changes. |
| Detect overlap? | Git working-file summaries and Worker task references provide observations, but no claim scope conflict classification. Counterfactual PlanComparison is analytical, not durable ownership or candidate integration. |
| Before canonical Git promotion? | Existing session verification, EvidenceStore, repository/checkpoint/KnownGood and Git history are reusable. Per-session success is not a multi-contribution integration gate. No automatic combined-result verification currently follows Worker messages. |

Code owners inspected: execution.rs (ExecutionSession/SessionStore/verification/
Evidence), development.rs (Workers/presence/messages/events/shared state),
binding.rs (resource binding), plan.rs (analytical plans), institution.rs
(pure effects and explicit operator grants), rpc.rs (intent aliases, Worker
surfaces and transport context), and free-autonomy-minimal 1.0.0.

## Security finding requiring an explicit trust decision

`worker.message.send` takes a caller-supplied `params.worker_id`.
`send_worker_message` verifies that the Worker exists, then copies that ID into
message provenance and event actor. Neither the Request envelope nor this
domain entry binds a caller credential/principal to that Worker. The local
stdio boundary is a trusted operator/host, as already documented by Institution I.

Real-process reproduction in an isolated temporary project:

1. Run the existing Route binary's init.
2. Register audit-a and audit-b using one client.
3. From the same client send a NOTICE with worker_id audit-b, without any
   caller credential or bound Worker identity.
4. Actual result: accepted=true, event actor_worker_id=audit-b,
   message from_worker=audit-b, caller_credentials_supplied=false.

This is a test of missing caller-to-Worker binding, not a demonstrated bypass
of a credential check: there is currently no such check. It does not falsify
the previous phase's narrower institution-payload anti-forgery guarantees.
The disposable project's operational event history is retained under the
system temporary directory; no user project state was edited by the probe.

Consequently, an adversarial Worker using the unrestricted operator interface
can choose another existing Worker ID. Adding work.claim with another freely
supplied worker_id cannot meet the requested anti-impersonation criterion.
The existing trusted interface also exposes operator-owned institutional
activation; independent Workers must not inherit that authority implicitly.

## Proposed boundary at the historical baseline

Separate a host-bound Worker interface from the existing local operator
interface. The trusted host establishes the Worker principal; ordinary Worker
requests cannot select another principal or call operator-only grant/binding
operations. Claims/messages/events derive their actor from that binding.
The operator interface remains explicitly privileged for compatibility.

This must state its threat model: host-enforced API identity is not OS-level
isolation against a process that can directly modify the same project files
or invoke the privileged operator interface. Protecting against that actor
requires host/sandbox/filesystem separation, not just adding a token to the
shared ledger. No secrets should be put in the globally visible history.

Choosing this trust/deployment boundary is the batch's L4 authorization step,
not routine Worker assignment. Do not claim either weaker trusted-client
coordination or stronger hostile-process isolation on the user's behalf.

## Smallest implementation path after that decision

1. Reuse ExecutionSession IDs for work; retain the current Intent parent and
   bounded child scope explicitly. Do not create AutonomousTask/SocialTask.
2. Use the existing DevelopmentEvent ledger/lock/revision/idempotency path for
   claim, release, explicit interruption acknowledgement and reassignment.
   No lease expiry by age; no second work database.
3. Reuse WorkerMessage and event deltas for help/disagreement/reconsideration;
   an institution only suggests. Preserve v1 bytes when adding the v2 example.
4. Keep separate contribution provenance and verification Evidence; require
   explicit integration checks before parent success or canonical promotion.
5. Run three independent clients on one disposable objective, with no Human
   assignments; record A/B/C scenarios, interruption and metrics. Report actual
   client/model identity. Scripted clients are not independent AI models.

## Evidence disposition

- Security probe: executed, exit 0; missing caller binding reproduced.
- Autonomous A/B/C dogfood, micro-management metrics, performance and new
  runtime regression: NOT_RUN. Do not reuse Institution I counts as this phase's tests.
- CROSS_MODEL: NOT_RUN.
- The initial autonomous audit did not satisfy its PASS-only commit/push gate.
  The separately authorized Worker Principal Binding phase has its own gate.
- No main/Yuich changes, tag, release, force or history rewrite.
