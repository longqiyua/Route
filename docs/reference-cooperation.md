# Reference, Constraint, and Cooperation

Status: architecture contract. The integrated domain, `route/1`, CLI, and
multi-worker conformance gates remain in progress; this document does not
claim that those surfaces are complete.

## Semantic ownership

These concepts are deliberately separate. Relationships between them do not
transfer truth, authority, capability, or evidence status.

| Concept | Meaning | Canonical owner | What it is not |
|---|---|---|---|
| **Reference** | Information that may inform development | The existing project Reference registry | Constraint, Evidence, fact, or current reality |
| **Constraint** | Authority that governs development | Existing authoritative project sources, exposed through one read-only constraints projection | A second writable registry or authority inferred from a Reference |
| **CooperationResource** | An affordance or resource that may assist development | Projection of project-scoped Cooperation events in the development ledger | Capability, tool execution, Evidence, or authority |
| **CooperationKnowledge** | Revisable shared learning about a CooperationResource | Projection of knowledge events and supersessions in that ledger | Evidence, raw reasoning, or an irreversible fact |
| **DevelopmentEvent** | One atomic development-history observation or state transition | The project Global Development Commons ledger | A replacement for Git, Route History, Evidence, or domain state |

In short:

```text
REFERENCE INFORMS.
CONSTRAINT GOVERNS.
COOPERATION ENABLES.
```

Attaching a Reference to a Constraint documents the Constraint; it does not
make the Reference authoritative. A Constraint may require a
CooperationResource, but that relationship does not make the resource
authoritative. Declaring a capability on a resource does not prove that the
capability works.

The intended constraints projection must read canonical authority already
owned by Route, including applicable Constitution, Protocol, and explicit
intent constraints. The current experimental `material.rs` only scans
`constraints/`; it does not yet implement that unified view.
It must not create a second Constraint truth store. Descriptive free text named
`constraints` on a legacy Reference entry remains Reference metadata and does
not become authority merely because of its field name.

## Project architecture

```text
                   Route Project
                        │
       ┌────────────────┼────────────────┐
       │                │                │
   References       Constraints      Cooperation
       │                │                │
       └────────────────┼────────────────┘
                        │
             Global Development Commons
                        │
              independent Workers
```

References, Constraints, and Cooperation remain project-scoped. Another
project may be registered as a CooperationResource, but project identity,
workspace identity, history, constraints, and write authority remain separate.
Registration never grants implicit writes into the cooperating project.

## References are bounded information

A Reference is a locator plus bounded metadata and provenance. Local and
external files, documents, directories, and URIs may be described without
copying their full contents into Route state. Missing and unknown are valid
states. Registration does not recursively ingest a directory, stage content in
Git, or promote information to Evidence or Constraint.

Global logical availability is not full prompt materialization. Lists and
shared-state projections contain IDs, counts, status, fingerprints, and recent
changes. Exact content is retrieved on demand and under a caller-controlled
budget.

## Cooperation is an affordance, not execution authority

A CooperationResource may describe a file, document, executable, CLI, script,
directory, project, service, dataset, model, protocol, or an unknown resource.
It may use a relative path, absolute local locator, PATH command, repository,
or external endpoint. Registration records that the resource might be useful;
it does not execute it, prove its capabilities, take ownership of it, or grant
an adapter, plugin, shell, network, or filesystem permission.

Discovery is bounded to metadata, basic structure, adjacent documentation,
known References and CooperationKnowledge, Development History, worker or
Human questions, and explicitly safe deterministic probes. When safe probing
cannot be guaranteed, the correct state is `REQUIRES_EXTERNAL_DISCOVERY`, not
an unrestricted command execution escape hatch.

## CooperationKnowledge is revisable shared learning

Knowledge keeps epistemic status explicit:

| Status | Meaning |
|---|---|
| `DECLARED` | A Human, resource, or document claims something |
| `OBSERVED` | A bounded observation, with real Evidence references where available, supports it |
| `INFERRED` | A Worker inferred it |
| `UNKNOWN` | Current information is insufficient |

`DECLARED` and `INFERRED` never silently become `OBSERVED`. A resource version
or fingerprint change can make prior knowledge stale; it does not rewrite the
old record. Corrections create revisions or superseding records so provenance
and prior claims remain inspectable.

CooperationKnowledge is not Evidence automatically. A WorkerMessage remains a
message, and a knowledge statement remains knowledge even when either refers
to Evidence. No domain accepts or persists raw chain-of-thought. Only bounded
operational statements, provenance, source references, Evidence references,
and revision metadata belong in shared state.

## Shared learning

Meaningful registrations, refreshes, availability changes, discovery updates,
knowledge records, and bounded capability observations produce typed
DevelopmentEvents and advance the project GlobalRevision under the existing
ledger rules.

```text
ONE WORKER LEARNS
→ durable CooperationKnowledge
→ GlobalRevision advances
→ ALL WORKERS CAN OBSERVE THE LEARNING
```

A worker may start from an older revision, read the incremental event delta,
and query the same durable knowledge without independently rediscovering it.
For Cooperation, events contain the domain transitions and current records are
derived projections; there is no separate Cooperation registry. Reference still
has a separate registry and its recoverable event bridge is unfinished.
Shared-state summaries limit their output, but this is not a guarantee of
bounded ledger-read cost. See [practical readiness](practical-readiness.md).

## Future boundary

```text
Route Core
  → development primitives

Open Institution Runtime
  → future programmable organizational runtime

Society SDK
  → future Human/AI-authored institutions
```

This substrate does **not** implement Parliament, voting, reputation, a task
market, agent economics, Institution Runtime, Society SDK, institution
self-evolution, worker self-modification, generic cooperation execution,
unrestricted shell access, or cross-project mutation. Parliament and markets
may someday be reference institutions above Route Core; neither is mandatory
Route architecture.
