# Route

## Keep your tools. Keep your repo. Add Route.

Route is an optional, local-first AI work sidecar: a small shared work record
beside your existing project. Keep using your AI terminal, editor and Git.
Route helps the next session see what happened, what stopped, and what remains
verified — without copying the previous chat.

No migration. No central GUI. No account for local mode. No master agent.
Route does not replace Codex, Claude Code, Cursor, Git or your terminal.
Real terminal acceptance in this phase covers **Codex CLI**; other names describe
the tool-neutral design, not tested integrations.

## Get value before learning the architecture

The lightweight commands below are **fruit development features**, not commands
in the already-published v1.0.0 binary. Build this source first:

```sh
git clone --branch fruit https://github.com/longqiyua/Route.git
cd Route
cargo install --path tool/route/crates/route-cli --locked
```

Then, in the existing repository you actually want to use:

Windows also ships an unrelated network utility named route.exe. Verify
`route --version`, or call `$env:USERPROFILE\.cargo\bin\route.exe` explicitly;
do not change system networking settings to install Route.

```sh
route attach
route handoff --shared
```

That's the basic setup: one attach action, no configuration fields. Attach creates
local state without scanning or archiving your source. Existing files, Git
history, hooks and configuration stay intact. A short ROUTE.md is created only
if absent; your existing one is never replaced.

Leave a useful fact, not a transcript:

```sh
route sidecar note "Found missing boundary test; next add Unicode coverage"
```

A fresh AI session in that same project reads `route handoff --shared` and sees
the published finding, work/claim state, blockers and next actions. Nothing from
your old chat needs to be copied. If structured work already exists:

```sh
route sidecar whoami
route sidecar claim WORK_ID
route handoff --shared
route sidecar interrupt CLAIM_ID "Stopping here; boundary test remains"
```

The next trusted local host can explicitly `route attach --host reviewer`, then
use `route sidecar --host reviewer claim WORK_ID --resumes-claim-id CLAIM_ID`.
This records independent identity and predecessor-linked continuation without
creating an organization or a new execution framework.

See the [real acceptance record](docs/lightweight-interop-acceptance.md) for what
was actually exercised, timings and limitations. Model startup/inference time
is separate from local attach/read latency; this is not a claim that two AI
sessions finish their reasoning within 30 seconds.
Reusable [normal-terminal prompts](examples/lightweight-interop/README.md) need
no extension or transcript copying.

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
| Completion | Machine-derived obligations and honest missing verification |

`route handoff --shared` and `route/1 handoff.get` expose the same projection.
Up to 8 entries per list, 256 characters per text plus ellipsis, and 64 KiB total.
Older entries may be omitted; query the existing protocol for details.
This bounds context output, not the size or replay cost of the underlying ledger.

Host credentials stay outside the shared project. Explicit trusted-local
bootstrap uses existing Principal Binding; ordinary helpers have only narrow
coordination grants. Same-OS-user hostile isolation is not claimed.
See [interop/safety](docs/lightweight-interop.md).

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
