# Open Development Substrate

Status: Route Open Society Substrate I architecture boundary.

Route is a development system, not an AI subject. This substrate gives
independent workers a shared, project-scoped development world without
defining an institution, authority model, reputation system, or social
simulation.

## Ownership boundary

| Concept | Canonical owner | Substrate relationship |
|---|---|---|
| Project | `ProjectIdentity.project_id` | Scope of one development commons |
| Workspace | `ProjectIdentity.workspace_id` | One attached checkout; never the Project itself |
| DevelopmentIntent | Existing task/session lifecycle | Referenced by events; not duplicated |
| ExecutionSession | Existing `ExecutionSession` / `SessionStore` | Referenced and projected; not duplicated |
| Evidence | Existing `EvidenceStore` and trust policy | Events carry evidence references only; messages and claims never become Evidence |
| Git history | Git | Source/version history |
| Route History | Existing locked Route command history | Sanitized command outcomes; retained unchanged |
| DevelopmentEvent | New append-only project ledger | Atomic development observation or state transition |
| Worker | New persistent, host/model-neutral identity | Participating actor; not a model, process, session, workspace, role, or authority |
| WorkerPresence | Projection from worker lifecycle/presence events | Bounded development metadata, never filesystem/runtime truth |
| SharedDevelopmentState | Read-only projection | Combines existing Route truth with DevelopmentEvents; owns no competing state |

## Persistence impact

The new ledger is stored under the explicitly shared project's Route state.
An ordinary workspace owns its local ledger. A workspace attached with the
existing project-identity attachment mechanism resolves the ledger through its
declared source project, so attached workspaces share events while retaining
distinct `workspace_id` values. If that explicit source is unavailable or its
`project_id` does not match, access fails closed instead of creating a second
truth.

Each committed event advances one monotonically increasing project-local
revision. The durable ledger is replaced atomically under a cross-process
lock, so readers see either the old complete revision or the new complete
revision. Event hashes form an integrity chain. Duplicate mutation retries are
blocked by Route's existing `route/1` idempotency receipt and by ledger-level
event/deduplication identity.

## Information and reasoning boundary

All committed events are logically visible to workers in the Project, subject
to future configurable visibility policy. Logical visibility does not mean
injecting the full ledger into every prompt: clients query a bounded sequence
range or `events after revision N`.

No schema field accepts raw chain-of-thought. Activity, finding, action, and
message content is a bounded operational summary. A message, agreement,
proposal, disagreement, or vote is not Evidence and cannot promote itself.
Current filesystem, Git, session, and Evidence stores override stale event
summaries when the shared-state projection is built.

## Logical architecture

```text
Route Core
  Project / State / DevelopmentEvent / Evidence / Git / History primitives
        |
        +-- future Open Institution Runtime (not implemented here)
                |
                +-- parliament / market / swarm / hierarchy / custom

Workers
  Claude / Codex / DeepSeek / Gemini / Human / scripts / other hosts
```

The future Society SDK and institution evolution belong above Route Core. This
phase provides only the neutral substrate.

## History integration

- Git History is canonical source/version history.
- Route Development History explains why and how development happened through
  existing sessions, evidence, checkpoints, and locked Route command history.
- `DevelopmentEvent` is one atomic historical observation or state transition;
  it supplements those stores and never replaces them.
- Any future AI-generated narrative is a derived interpretation, not canonical
  event truth and not Evidence.
