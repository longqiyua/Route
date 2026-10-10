# Route AI v1.0.0 — release notes

Route AI v1.0.0 is the Windows x64 CLI release. The product version is
`1.0.0`; the RPC protocol remains `route/1`. These notes describe bounded
implemented behavior, not cross-model certification.

## What's included

- Durable Project and work history, checkpoints, evidence, verification, and
  recovery surfaces beside Git, plus Global Development Commons.
- `route/1` JSON RPC, multi-Worker cooperation, and Worker Principal Binding.
- Reference/Cooperation and Open Institution Runtime.
- Autonomous multi-Worker development, Execution Contract completion gates,
  General Work, versioned Planning, and a bounded persistent Assistant
  status/review loop.

## Validated scenarios

The [bounded assistant audit](route-ai-v1-closure-audit.md) records a
non-code Goal with distinct same-host Workers, Plan v1/v2, denied premature
completion, final gate success, and restart continuity. Route does not
guarantee the semantic quality of the resulting work.

## Limitations and security model

Route is not an LLM runtime and does not guarantee semantic correctness. The
Operator is a trusted local host; Worker credentials must not be shared with
untrusted Workers. Principal Binding does not isolate malicious processes
under the same OS user. Cross-model certification is **NOT_RUN**. Cross-host
authentication and arbitrary external action automation are not certified.
Filesystem verification is scoped to the implemented checks; it does not
promise atomic snapshots or impossible ABA detection. Tauri, experimental
`route-pyo3`, and the legacy npm packages are not shipped in this CLI release.
Route does not claim AGI or an autonomous company.

## Upgrade and compatibility

Keep a backup of the project and local Route state before upgrading valuable
work. Do not manually rewrite append-only history. `route/1` remains the
protocol identifier; it is not renamed to match the product version. See
[installation and use](v1-release-candidate.md).
