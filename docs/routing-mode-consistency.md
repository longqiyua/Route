# Routing mode consistency and actual capability boundary

## Reproduced defect

An authenticated running Planner could publish `split=false, role=IMPLEMENTER`.
The broker selected routed Work by role and Work ID without checking `split`, so
the record could delegate despite explicitly saying not to split. The regression
test first failed with actual exit code 1 because the contradictory record was
accepted. This is a routing-intent consistency defect, not a budget bypass:
other reservation, permission and Work checks still applied.

## Contract and compatibility

| split | role | Result |
| --- | --- | --- |
| false | PLANNER | Planner self-work, no delegated Planner |
| true | IMPLEMENTER | Delegation subject to existing gates |
| true | INDEPENDENT_REVIEWER | Delegation subject to existing gates |
| any other combination | any role | INVALID_ROUTING_MODE for new decisions |

Knowledge extraction retains its dedicated queue and job checks. It cannot be
requested by disguising a normal routing decision as `KNOWLEDGE`.

The additive pure `team::validate_routing_mode` helper is shared by new-decision
validation and the CLI scheduler. It evaluates intent only, not authority or
capacity. The authoritative reservation gate also requires a valid split decision
for non-Knowledge child runs; the separate Knowledge path is unchanged.
An unfinished self-work decision still produces the existing
`PLANNER_SELF_WORK_NOT_COMPLETED` error instead of spawning another Planner.

JSON fields, persisted schema and historical hashes are unchanged. Previously
accepted contradictory modes are now rejected on new publication; this is an
intentional validation tightening. Old records remain readable, not silently
rewritten or migrated. A contradictory old record cannot authorize a child;
the scheduler errors rather than converting it into a different instruction.
This does not add an edit/recovery mechanism for old decisions.

## Current routing reality (source audit, not new Host certification)

- `host_adapter.rs`: the executable is Operator-approved and SHA-256 pinned.
  The implemented adapter invokes Codex with an argv vector and existing local
  configuration. No model-selection or provider-selection algorithm exists here.
- `team_commands.rs`: the Planner publishes role/Work decisions; the broker
  schedules serially and passes the configured Worker budget in the prompt.
  A role declaration is not proof of a model's actual capability or review quality.
- `team.rs`: active bound Planner identity, Goal Work membership, role/grants,
  depth, retry/attempt/distinct-Worker and project concurrency checks remain.
  A routing decision never reserves slots by itself.
- Model remains `UNKNOWN`; token and monetary budgets remain `NOT_TRACKED`.
  Host configuration can choose a model outside Route, but Route does not verify
  or enforce its cost. Generic rendering elsewhere is not an implemented generic
  runtime adapter, fallback provider or tested second Host.

No real AI/provider invocation, spending test, new Host compatibility certification,
model-quality benchmark or model-fallback test was performed in this repair.

## Tests

Authenticated domain/storage regression rejects all five contradictory role/mode
combinations and verifies byte-for-byte ledger preservation on rejection. The
matrix covers all eight combinations. A test-only old-record projection verifies
that a formerly accepted non-split delegation remains readable but fails the
child reservation gate; it does not alter an authenticated ledger on disk.
Existing self-work, implementer/reviewer delegation, budget, authority, cancellation
and CLI regression tests remain the positive/negative workflow coverage.
