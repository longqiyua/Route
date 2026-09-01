# Route Forge — Ownership Migration & Open Source Boundary

> **Snapshot date:** 2026-08-16. Records the canonical ownership split between
> Yuich (private) and Route (open source) after the Tool Genesis / Agent
> Genesis / Route Forge round. This is a PACKAGING BOUNDARY document, not a
> protocol spec.

## Product Strategy

```
Route  = open source.
Yuich  = private/closed (for now).
```

This boundary exists because Yuich and Route began as the same repository.
They must NOT accidentally share private assets on release.

## Ownership Migration

### Canonical Owner: YUICH (private)

These capabilities live in Yuich only. Route does NOT replicate them:

| Capability | Description |
|---|---|
| Subject | Persistent identity, continuity, encounter ownership |
| Prime | Sole approval authority |
| SelfModel | General self-modeling, capability usage tracking |
| Memory | Autobiographical/episodic/semantic/procedural memory |
| OpenFrontier | Open-world exploration |
| BoomMode | High-openness cognitive exploration |
| ISM | Internal cognitive atlas |
| Rhapsody | Emergent inner generation |
| Negativity | Friction/contradiction detection |
| HumanConstitution | Root invariants (H1-H8) |
| Constitute/Justify | Rule interpretation + human-value judgment |
| SelfLearning | General learning loop |
| CapabilityRegistry | General capability management |
| ToolRegistry | General tool management |
| ModelGateway | Model selection/routing |
| AgentGenesis | Temporary agent composition |
| ToolGenesis | Tool discovery/creation policy |
| Consciousness Semantics | Self-narrative candidates (not claimed) |

### Canonical Owner: ROUTE (open source)

These capabilities remain in Route. Moving them to Yuich would hollow out Route:

| Capability | Description |
|---|---|
| ProjectState | Codebase state, continuity |
| ProjectMemory | Architecture decisions, project history |
| ArchitectureMemory | Design rationale, constraints |
| Task/Intent | Development task tracking |
| KnownGood | Stable reference configurations |
| Evidence | Development evidence, benchmarks |
| Build/Test/Benchmark | Verification pipeline |
| Maintenance | Repair, refactor, restructure, migrate |
| Recovery | Save/rollback/checkpoint |
| Agent/Harness | Development-specific temporary agents |
| CLI | Route CLI, reference engine |
| Repository Continuity | Git integration, project history |

## Packaging Boundary

### Route Public Release MUST NOT Include

- Yuich private runtime (`yuich/mrs.py` subject engine)
- Yuich state files (`yuich/state/`)
- Yuich self-learning strategies
- Compliance update feed
- Relay data
- HumanConstitution enforcement logic
- Rhapsody generation
- Prime calibration data
- Accumulated real Experience
- ToolGenesis policies
- Proprietary dogfood/regression corpus

### Route Public API CAN Define

- Optional Subject Adapter contract (interface, not implementation)
- Tool specification format
- Agent specification format
- Harness specification format
- Development task/evidence protocol

Route MUST NOT require Yuich proprietary implementation to function.

## Yuich → Route Interaction

Yuich CAN call open-source Route as a tool.
Route CAN be used by non-Yuich AI / humans.
They are independent but complementary.

```
Yuich discovers need → Route builds tool → ToolRegistry → Yuich reuses → Learning
```

## Future Open Source Decision

Whether Yuich becomes open source in the future is an independent decision.
This round does NOT preset permanent closed-source status.

Route's open source status is final and non-negotiable.