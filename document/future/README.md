# Future Architecture — Direction Notes

> **HISTORICAL / DESIGN DOCUMENT**
>
> This document is not a statement of currently implemented Route behavior.
> For current behavior see:
> - [/ROUTE.md](../ROUTE.md)
> - [/docs/status.md](../docs/status.md)
>
> **Status: Design direction only. Not implemented.**
>
> This file records the three core concepts that will shape Route's
> long-term evolution. It exists so future work has a stable reference
> point. **Do not implement these as large systems now** — record new
> thinking here, keep the core stable, and let these emerge from real
> usage rather than premature architecture.

---

## 1. Constitution（开发宪法）

The **Constitution** is the immutable contract that defines what Route
*is* and what it *will never do*. It is the highest-authority layer:
every Protocol, Reference, and agent policy must comply with it.

Design direction:

- **Immutable by default.** The Constitution changes rarely and only
  with explicit user consent. It is not a config file the AI may
  rewrite at will.
- **Defines invariants**, not features: data safety, locality,
  recoverability, single-user ownership, no silent data loss, no
  operations outside the project root.
- **Bounds all agents.** Any AI workflow / agent policy operates
  *under* the Constitution, never above it. A Constitution violation
  is a hard stop, not a warning.

The Constitution is **not** a plugin marketplace, a workflow engine, or
a permission UI. It is a small, durable statement of invariants.

---

## 2. Protocol（执行规范）

The **Protocol** layer defines *how* operations execute within the
Constitution's bounds. It is the execution contract: the steps,
ordering, commit points, and recovery rules for every destructive or
observable action.

Design direction:

- **Mutable and versioned.** Unlike the Constitution, Protocols evolve
  as Route learns which execution patterns are safest. A Protocol
  change must never weaken a Constitutional invariant.
- **Embodies the crash-safety model** Route already follows: prefer
  recovery over impossible global atomicity, make every destructive
  operation detectable and recoverable, define an explicit commit
  point.
- **Drives CLI / MCP / HTTP / PyO3 uniformly.** The same Protocol
  description is exposed across every control surface; no surface gets
  a private execution path that bypasses safety.

The Protocol is **not** a new database, a new runtime, or a new wire
format. It is the codified execution discipline Route already practices
informally.

---

## 3. Reference（可变参考）

The **Reference** layer is Route's mechanism for knowing about — and
delegating to — the outside world.

> **Reference should maximize compatibility, not reimplement external tools.**

Anything may become a reference:

- a tool,
- a CLI,
- an exe,
- a skill,
- a repository,
- a workflow,
- a documentation set,
- an MCP server,
- an API,
- or any external project.

Route should **describe and expose** capabilities, while **execution
remains dependent on the host environment**. Route does not bundle or
re-ship the external tool; it records how to find it, how to talk to
it, what it is good for, and when to trust it.

Design direction:

- A Reference is a **description, not a dependency**. If the host lacks
  the referenced tool, Route degrades gracefully and says so — it does
  not silently fail or vendor a copy.
- References are **first-class data**: indexable, dedup-able, and
  queryable by both humans and agents.
- Compatibility is the metric. The Reference layer succeeds when Route
  can orchestrate whatever the user already has installed, not when it
  reimplements those tools internally.

### Future Reference Curator

A future **Reference Curator** agent maintains reference quality:

- indexing,
- deduplication,
- availability (is the referenced tool actually present?),
- trust (is the source known?),
- usage evidence (how often, how recently, with what outcomes?),
- and relevance (does this reference still match the user's context?).

**It must not modify the Constitution.** The Curator operates strictly
within the Constitution's invariants and the Protocol's execution
rules. Its job is to keep the Reference layer accurate and useful —
never to redefine what Route is.

---

## Relationship

```
Constitution  (immutable invariants — what Route IS)
     ▲
     │  must comply
     │
Protocol       (versioned execution rules — HOW Route acts)
     ▲
     │  must comply
     │
Reference      (mutable descriptions — WHAT Route can reach)
     │
     │  maintained by
     ▼
Reference Curator  (never edits Constitution or Protocol)
```

Each layer may tighten what lies above it in *practice*, but may never
*weaken* what lies above it in *contract*.

---

## Out of Scope (recorded, not built)

The following are **explicitly deferred** and must not be pulled into
the core on inspiration:

- Vector DB / GraphRAG
- Agent Runtime
- Constitution Compiler
- Workflow marketplace / plugin marketplace
- GUI revival
- Cloud sync / collaboration
- Large embedded database (SQLite is enough)

When a new idea appears, add it here as a one-liner and move on. The
core stays small, safe, and recoverable.
