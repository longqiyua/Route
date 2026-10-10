# Evolution & Learning

> **EXPERIMENTAL.** Evolution and emergence are experimental capabilities.
> They are **off by default**, gated, and never silently change your project.
> Route does **not** claim to "automatically improve" your code — it records,
> evaluates, gates, and rolls back. Any candidate promotion requires the
> Engine gate; a candidate can never promote itself.

The protocol-level organization behind evolution — the tripartite
EXPLORE / EXECUTE / EVALUATE planes, deliberate divergence, and organization
learning — is defined in [agents.md](agents.md#tripartite-organization). This
page describes the Reference Engine surfaces that implement the loop.

## Tripartite Planes (protocol)

| Plane | Purpose | Cannot |
|-------|---------|--------|
| **EXPLORE** | N independent divergent thinkers produce Hypotheses, adversarial cases, BenchmarkCandidates. | mutate Stable; Promote; self-declare trust. |
| **EXECUTE** | implement the selected hypothesis into a Candidate, strictly (obey Intent + plan, minimal scope, evidence-producing). | self-Promote; substitute its own method for a stated `method_hint`. |
| **EVALUATE** | independent review via Benchmark / Reference / Evidence. | secretly mutate the Candidate; be sole judge of its own implementation. |

**EXPLORE FREELY. EXECUTE STRICTLY.** Route owns the final persisted
Promote/Reject state.

## Learning

Learning turns recorded experience into proposals.

```bash
route learn record                      # record an experience event
route learn analyze                     # generate learning proposals
route learn review                      # review open proposals
route learn apply <proposal_id>         # promote a proposal to a reference
route learn why <topic>                 # trace the evidence chain
```

**Proposals are never auto-applied.** Only a human or an explicit, gated
`apply` promotes them.

## Evolution

Evolution closes a **candidate-first** loop:

```
Observe → Propose → Candidate → Benchmark → Compare → Promote/Reject → Learn
```

```bash
route evolve propose --hypothesis "..." ...   # propose a candidate
route evolve show <id> --explain               # inspect a candidate
route evolve evaluate <id>                     # evaluate against a benchmark
route evolve promote <id>                      # promote to KnownGood (gated)
route evolve reject <id>                       # reject a candidate
route evolve history                           # experiments + KnownGood chain
route evolve explain <id>                      # emergence lineage
```

Key properties:

- **Candidate-first.** Stable/original state is only touched by an explicit,
  gated promotion.
- **Trust roots.** ORIGINAL archive, KnownGood, Evidence/Audit, benchmark
  baseline, promotion policy, and recovery executor cannot be modified by a
  candidate through ordinary tools.
- **No self-promotion.** A candidate can never promote itself.
- **Local-first.** The whole loop runs offline against local files.

## Emergence

`route emerge` is a hardening layer on top of evolution. It is **EXPERIMENTAL
and off by default**; enable it explicitly.

```bash
route emerge enable
route emerge status
route emerge run
route emerge event --actor <actor> --source <source>
route emerge court
route emerge novelty
```

## Campaigns

`route evolve campaign` governs a set of experiments with a budget and
selection state. It is **EXPERIMENTAL**.

```bash
route evolve campaign create --goal "..." --scope ... --strategy ...
route evolve campaign status <id> --json
route evolve campaign next <id> --json     # machine-readable next_action
route evolve campaign report <id> --json   # explain/report
```

Route manages state and budget only; a harness executes the `next_action`.

## Trust Roots

These are protected from candidate modification:

- **OriginalArchive** — source of truth for disaster recovery
- **KnownGood** — last verified, promoted configuration
- **EvidenceAudit** — append-only ledger
- **BenchmarkBaseline** — fixed + holdout cases
- **PromotionPolicy** — who may promote, and what gate is required
- **RecoveryExecutor** — the mechanism that restores on disaster

## Route Self-Evolution

Route itself follows the same candidate-first discipline. Route keeps:
**Route Original, Route KnownGood, Route Candidates, Route History** — the
canonical Route never has only one writable copy (self-save semantics; see
[ROUTE.md §11](../ROUTE.md#11-save-self-save-and-self-evolution)).

Sources of a Route Candidate: repeated failures, user corrections, bootstrap
benchmark results, cross-project experience, harness behavior,
agent-organization results, references.

```
Current Route → detect limitation → Candidate ROUTE.md/policy
→ benchmark: fixed invariants, historical tasks, regressions,
  known-good cases, trusted references, bootstrap tests,
  independent evaluator
→ compare
→ promote / reject
→ retain previous KnownGood
```

Binding rules:

- A Route Candidate **cannot declare itself better using benchmarks it just
  wrote for itself**.
- **Route must not be its own only judge** — an independent evaluator plane is
  required.
- Promotion retains the previous KnownGood; rejection destroys nothing.
- Self-update preserves: Protocol First, Atomic Capability, Shared State,
  Dynamic Agency, Co-Learning, recoverability.

**Status: protocol defined; autonomous self-modification is NOT enabled by
default.** No part of the engine rewrites ROUTE.md or Route policy on its own.

## What Route Does NOT Do

- It does not run a model, sandbox, or subagent runtime.
- It does not automatically apply learning or evolution outcomes.
- It does not claim unverifiable results such as "automatic self-improvement."
  Any claim of improvement must be backed by benchmark evidence and pass the
  promotion gate.