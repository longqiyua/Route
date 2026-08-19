# 2026-08-19 — AI Worker Enforcement

- **Date**: 2026-08-19
- **Route version**: 1.0.0-beta
- **Goal**: Keep AI Worker a bounded executor — the AI may not decide promotion or disguise claims as evidence.
- **Trigger**: Release closure; enforce the "AI says 'tests passed' ≠ evidence" rule by actually running Route gates.
- **Source branch/ref**: `fruit` `bbd0618` (enforcement tests, discover_bundled_tools), Route gates in `route-basic` (BranchGate/PathGate/CommandGate/DiffGate/TestGate/ClaimEvidenceGate).

## What Route understood

- Claims are not evidence. Route must actually execute tests; AI-declared evidence cannot be disguised as system evidence (TestPass/Commit are system-produced).
- AI Worker must not self-promote; promotion requires Host/Route-confirmed evidence.

## What changed

Nothing structural this round (gates already exist); this record documents that the release-closure AI work was performed inside those boundaries.

## Evidence

- `cargo test --workspace` exit code 0 = system evidence for the self-dogfood change.
- `cargo test -p route-cli` green after the dead-import cleanup.
- Existing enforcement tests in `fruit` `bbd0618`.

## Tests

Workspace regression; route-cli unit tests.

## Failures

None.

## Outcome

AI_WORKER_BOUNDARIES_ENFORCED YES; CLAIM_EQUALS_EVIDENCE NO.

## Status

CONFIRMED.

## Route learned

Running the actual test command is the difference between "claimed" and "evidence".

## Related commits

- fruit: `bbd0618`, `e30ce43`; sandbox: this record.
