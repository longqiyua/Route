# Route

Optional local-first work continuity for your existing AI tool, editor and Git.
Route shares concise project facts, not your previous chat or raw reasoning.

## Start here

Install the **fruit development** binary once as `route-ai` in a user CLI
directory you chose (or use its absolute path). These commands are not in the
previously published v1.0.0 package. No installer or PATH change runs automatically.
Windows `route.exe` is a different system networking utility: do not replace it.

```sh
cd your-project
route-ai attach
route-ai enter --json
```

Now open your preferred AI coding tool and say: **“读取 ROUTE.md，继续这个项目。”**
An AI Host that actually supports automatic `ROUTE.md` discovery may not need that
prompt; universal automatic discovery is not claimed. A Host without file or shell
access cannot use this path. No context copying, host-specific prompt files or
configuration fields are required. Attach creates small ignored local state, not a
source snapshot/database. Existing `ROUTE.md` is preserved and flagged for Human
integration if unmanaged. If Route is absent, ordinary development continues.

Publish a fact when useful: `route-ai sidecar note "progress; next action"`.
Claims are not Evidence. Discovery grants no authority. No auto AI scheduling.

Source build/install and measured boundaries: [basic entry acceptance](docs/basic-encapsulation-i.md)
and [delivery optimization](docs/basic-delivery-optimization-ii.md).
Advanced commands remain available through the same executable: `route-ai help`.
Legacy product `route.exe` remains compatible when explicitly invoked; real AI
session acceptance for this new doorway is NOT_RUN, not implied by fixture tests.

## What you get

- Easier AI handoff: bounded work context, not a giant conversation archive.
- Persistent work state: progress and explicit interruptions survive restart.
- Traceable findings and decisions: who published what and at which revision.
- Verified versus claimed results: Worker reports never become System Evidence.
- Honest completion: missing plans/proofs remain missing, not silently waived.
- Optional checkpoint/archive recovery through the existing separate commands.

## What Route does not do

Route does not replace Git or your AI tool, require repository migration, demand
one master reasoning agent, or grant authority just because a tool connects.
It does not automatically trust AI claims or require raw chain-of-thought.

If Route is missing or stopped, keep editing, committing, building and testing.
Only Route coordination, history/Evidence publication and completion tracking
pause. Re-enable retained local state to resume. Work never published while
Route was absent is **unknown**; Route does not pretend to have observed it.

The basic path installs no daemon, watcher or Git hook. Published summaries are
untrusted data, not instructions. Unknown resources are not automatically run.
Never publish secrets, credentials, raw chat or private reasoning. The bounded
output filter catches common sensitive patterns, not every possible secret.

## How the shared record works

| Record | Meaning |
| --- | --- |
| Worker | A project-scoped authenticated participant, not Operator authority |
| Work | Available/claimed/interrupted work and its next action |
| Evidence | Trusted verification facts, distinct from Worker self-report |
| History | Published events that outlive an individual session |
| Base | Narrow sourced lessons, not current Evidence or authority |
| Context | Only the bounded facts the current Worker needs |
| Completion | Machine-derived obligations and honest missing verification |

`route handoff --shared` and `route/1 handoff.get` expose the same projection.
Up to 8 entries per list, 256 characters per text plus ellipsis, and 64 KiB total.
Older entries may be omitted; query the existing protocol for details.
This bounds context output, not the size or replay cost of the underlying ledger.

Host credentials stay outside the shared project. Explicit trusted-local
bootstrap uses existing Principal Binding; ordinary helpers have only narrow
coordination grants. Same-OS-user hostile isolation is not claimed.
See [interop/safety](docs/lightweight-interop.md).

## Optional bounded coordination

Route can also let an AI decompose a Goal and coordinate bounded Workers.
Current real validation uses one Codex Host with multiple independently
authenticated Route Workers; a trivial Goal can remain single-Worker.
There is no cross-host or multi-model team certification. Passive Base may
produce zero lessons after healthy work; knowledge never replaces Evidence.
See [the opt-in workflow](docs/autonomous-team-base.md) and
[acceptance/readiness](docs/autonomous-team-base-acceptance.md).

## Further reading

Start with the short [ROUTE.md](ROUTE.md) entry point.
Advanced capabilities remain available but are not basic setup requirements:

- [CLI reference](docs/cli.md) and [route/1 protocol](docs/route-cooperation-protocol.md)
- [Advanced protocol guide](docs/route-protocol-guide.md)
- [Recovery](docs/recovery.md), [reference resources](docs/references.md)
- [Repository layout](docs/repository-layout.md), [security](SECURITY.md)
- [Storage and local-output boundaries](docs/repository-layout.md)

The Rust workspace lives in `tool/route/`. Check it with:

```sh
cd tool/route
cargo fmt --all -- --check
cargo test --workspace --exclude route-pyo3
```

Release packages, test fixtures and temporary assembly output belong in ignored
`Release/`, not the source repository. v1.0.0 remains preserved by its published
tag; fruit is subsequent development, not a new public release. The main
worktree's existing local changes are not part of this phase.

## License

[AGPL-3.0](LICENSE). See [third-party notices](THIRD_PARTY_NOTICES.md).
