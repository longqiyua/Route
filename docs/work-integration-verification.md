# Work integration verification review

This is a bounded review of the existing Work/Evidence boundary, not an additional
authority or a claim that concurrent filesystem writes are isolated.

## Baseline observed before the causal-revision candidate

`work::validate_transition` requires completed selected claims covering every
child, no active claim for those children, the current ledger revision, and the
current nonempty working-state hash. Each supplied proof must be a System
CheckPass/TestPass for the same Intent, match that hash, and have a timestamp
strictly later than the latest selected completion. Verification requirements
must match evidence `check_id` metadata.

`execution::compute_state_hash` includes snapshot identity, changed-path content
hashes and effective context. `execution::exec_command` computes its state hash
after the command exits; its evidence deduplication includes the development
revision. Thus finishing a claim changes the deduplication key even when file
contents do not change. Evidence is deduplicated by session, kind and key.

## Alternatives and boundary cases

| Case | Strict timestamp plus content | Content alone |
|---|---|---|
| Check predates claim completion, identical contents | Rejects | Can accept |
| Check follows completion within the same millisecond | Rejects despite ordering | Can accept |
| Clock moves backward between completion and check | Rejects despite ordering | Can accept |
| File bytes change after evidence is recorded | Rejects by hash | Rejects by hash |
| Command reads old bytes, concurrent writer changes them before command exits | Post-command hash can describe untested bytes | Same limitation |
| Another claim remains active | Rejects independently of evidence | Must retain this separate gate |
| Ledger revision changes before integration | Rejects stale request | Must retain this separate gate |

The equal-timestamp case cannot be repaired safely merely by changing `>` to
`>=`: a check actually preceding completion can have the same timestamp too.
Timestamp ordering is a conservative policy check, not proof of a stable test
input. Conversely, a content hash is necessary for stale-content rejection but
does not by itself establish when or against which transient state a test ran.

The equal-timestamp false negative also affects retries: a later check with the
same command, state, check ID and development revision is deduplicated to the
original evidence. Its timestamp remains equal to completion, so repeated real
runs alone do not guarantee recovery. This follows from `record_dedup`; it is a
source-derived boundary case, not a claim that a timed production run reproduced
the collision.

For deterministic regression coverage, construct explicit evidence timestamps
relative to the completion event; do not rely on sleeps to distinguish an equal
millisecond boundary. Test unchanged-content evidence from before completion
separately from stale-content evidence after completion. For concurrent mutation,
use a command/barrier that reads a fixture, signals the host, then waits while the
fixture changes before exiting; compare the recorded hash with the bytes read.

## Review position and limits

Initial recommendation: retain both current checks until the desired
post-completion policy is explicitly resolved by the bounded reliability task.
The strict wall-clock comparison has a demonstrated logical false-negative
boundary; removing it changes policy and should not be described as a mere retry
fix. An existing development revision could express causal ordering more
directly, but any such implementation must define which revision is captured
before command execution and checked at integration.

A start/end content check would detect many changing-input runs, but does not
provide atomic filesystem isolation or catch every change-and-revert sequence.
Do not claim either timestamp freshness or a post-run hash solves those races.
No new runtime, lock ownership, evidence authority or institutional authority is
proposed by this review.

The active `free-autonomy-minimal` version `2.0.0` binding has only OBSERVE,
COMMUNICATE and PROPOSE grants. Its messages can guide voluntary review; they
cannot choose the accepted implementation or substitute for System verification.

## Verification of this review

Peer proposal `evt_01M480HN4FWD1253GWK88TJ85B` replaces wall-clock ordering for
new checks with a check-start development revision, retaining current content
binding and a legacy timestamp fallback. This addresses the review's ordering
concern if the start revision also participates in deduplication. Otherwise a
check spanning completion could suppress its later valid rerun. Present but
malformed revision metadata should fail closed; a revision above the integration
ledger revision should also fail. Legacy fallback preserves old behavior and does
not establish command-start ordering. These are review criteria pending the
candidate and its tests, not a claim of approval.

Review disagreement `evt_01M480NK09BDY50X6646ER75Y5` records why a candidate
combining start-revision validation with end-revision deduplication is insufficient:
the check spanning completion can suppress a valid later rerun. The narrower
alternative is to use start revision in both places; introducing a new run
identity is a broader alternative, not required by this review. Follow-up finding
`evt_01M480P0A8TS16V846Z2KSXZEZ` also requests preserving the maximum completion
timestamp across all selected claims for legacy fallback, independently of the
maximum completion sequence, so clock regression does not weaken legacy policy.

The existing test
`work::tests::integration_requires_post_completion_system_evidence_and_current_state`
passed locally (one test, 409 filtered out). It checks pre-completion and Agent
evidence rejection and fresh System evidence acceptance; it does not test the
equal-timestamp retry or changing-during-command cases above. Repository
`scripts/check-docs.ps1` also passed. The additional boundary cases remain
source-derived findings pending dedicated test evidence from the shared task.

## Resolution in this continuation

The accepted candidate captures command-start development revision for causal
freshness and deduplication. Integration rejects malformed/future markers and
retains the conservative timestamp fallback for legacy Evidence. The dedicated
route/1 subprocess test covers a check spanning completion and its valid rerun,
equal-millisecond evidence, concurrent claims, changed bytes, and keyed retry.
An additional deterministic unit test changes project bytes while a verification
command is held: its exit code 0 yields CheckFail rather than CheckPass, and a
stable rerun yields CheckPass. A failing command likewise cannot create passing
Evidence. Start/end fingerprints reduce false positives, but do not guarantee
atomic filesystem isolation or detect every ABA edit-and-revert sequence.
