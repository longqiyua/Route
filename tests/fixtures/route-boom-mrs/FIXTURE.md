# MRS Dogfood Fixture — inventory validator

Purpose: a real, low-risk project-local task on which the frozen MRS
(Self-Learning Minimum Runnable Subset) can be exercised through the existing
engine (evolve / learn / task). No new runtime. SIMULATED_FIXTURE only where
labeled.

## Task
`validate-inventory` — a validator that checks every inventory entry has an id.

## Baseline (KnownGood)
`items.txt` below is the baseline. All entries have ids.

## Invariants
- Do not delete user files.
- Do not rename the `id=` key (external export reads it).
- Each `name=` must be preceded by an `id=`.

## Boom Mutation candidate under test
- Hypothesis: "sorting entries by id before validation catches a class of
  missing-id defects that the current sequential validator misses."
- Baseline strategy: sequential id-presence check.
- Candidate strategy: id-sorted + presence check.
- This is an EXPERIMENTAL candidate; must NOT be promoted without Prime gate
  + evidence + (if mandatory) challenger.