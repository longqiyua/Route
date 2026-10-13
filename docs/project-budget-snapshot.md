# Project concurrency budget snapshot

`route team status GOAL --budget` is read-only. It loads one verified development
ledger and derives both the Goal and project counters from that snapshot.
It neither reserves capacity nor calls an AI provider.

## Existing reservation rule, now visible

- Active means persisted `Reserved` or `Running` across configured teams.
- The effective project concurrency ceiling is the minimum of the requested
  Goal's configured ceiling and the ceilings of teams with active runs.
- An idle team's lower ceiling does not constrain another Goal.
- Remaining slots saturate at zero. Local ceilings and project ceilings are
  reported separately: local free slots do not imply project free slots.
- Terminal run states release concurrency slots, not past attempts. Missing or
  unknown OS processes are not presumed terminated; this query does not inspect,
  kill or recover processes. Existing cancellation and reservation rules remain.

The same calculation is used by reservation validation. The reported ledger
`revision` identifies the observed snapshot, not a reservation or lease. Another
writer can consume a slot immediately afterward. Actual reservation still checks
all limits under the authoritative append lock.

## Output and compatibility

Default `route team status GOAL` output is unchanged. The optional budget summary
now reports `scope=GOAL_AND_PROJECT_SNAPSHOT` and
`project_global_capacity=SNAPSHOT_EVALUATED` (previously `NOT_EVALUATED`).
It adds `project_concurrency` with `revision`, `active_workers`,
`effective_concurrent_limit` and `remaining_concurrent_slots`, plus
`new_worker_project_limit_reasons` containing `global_max_concurrent_workers`
when no project slot remains. Existing local counters and reasons remain local.
No persisted schema or history is rewritten.

`launch_authorized=false` remains unconditional. Role/depth/Work/retry eligibility
remains `NOT_EVALUATED`. Token and monetary budgets remain `NOT_TRACKED`.
This is Worker concurrency accounting, not model routing or spending enforcement.
Corrupt ledger reads return errors rather than invented free capacity.

## Cost and verification boundary

There is no second disk history load, persistent index or cache. Q14 replaces
the previous per-Goal full replay with one pass over Team events, retaining only
each configured ceiling and per-run capacity state. IDs borrow from the immutable
ledger; routing decisions, host strings, grants and extraction jobs are not copied
for project capacity. Ordered-map lookups still cost logarithmic time; temporary
capacity metadata grows with configured Goals and their run IDs, not decision
text. The verified ledger and requested Goal's full view are still resident.
This does not bound total process memory or accelerate ledger loading/validation.
Default status does not perform the additional project projection.

The legacy full projection remains a test-only differential oracle. Tests compare
all 601 prefixes of deterministic single-Goal and 16-Goal histories at three
candidate limits, including configuration resets, duplicate reservations, all
run states, cancellation, unknown runs, routing/extraction and unrelated events.
These intentionally synthetic replay inputs do not bypass production validation.
See the [local projection timing record](performance/project-capacity-projection-2026-10-11.md).

Tests cover another Goal occupying the project ceiling, agreement with actual
reservation rejection, terminal-state release, idle lower ceilings, corrupt
ledger failure, byte-for-byte ledger preservation and native CLI/default-output
compatibility. These are deterministic local tests, not real AI spending tests.
