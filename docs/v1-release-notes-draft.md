# Route AI v1 — local release notes draft

**Draft only. Not a tag or publication.** The public Route AI v1 product
version is `1.0.0`; the candidate source is the `fruit` branch and `main`
reconciliation remains pending. These notes describe bounded features already
implemented and exercised, not a claim of cross-model certification.

## What's in the candidate

- Persistent project/work history, checkpoints, evidence, verification, and
  recovery surfaces beside Git.
- `route/1` JSON RPC; shared multi-Worker work and authenticated Principal
  binding.
- Reference/Cooperation and Open Institution Runtime.
- Worker coordination, Execution Contract completion gates, General Work,
  versioned Planning, and a read-only bounded Assistant status/review loop.

## Validated scenarios

The prior [bounded assistant audit](route-ai-v1-closure-audit.md) records a
real non-code Goal with distinct same-host Workers, Plan v1/v2, denied
premature completion, final gate success, and restart continuity. This
release-candidate phase separately requires a clean remote clone, release
build, package/first-run/move smoke, and fresh regression before publication;
the phase report is the evidence for those gates, not these draft notes.

## Limitations and security model

Route is not an LLM runtime and does not guarantee semantic correctness. The
Operator is a trusted local host; Worker credentials must not be shared with
untrusted Workers. The binding does not isolate hostile processes under the
same OS user. Cross-model operation is untested. Cross-host authentication,
cloud security, arbitrary external actions, and future GUI/Parliament/Market
surfaces are not part of the Windows CLI candidate. Tauri and `route-pyo3`
are not shipped here.

## Upgrade and compatibility

Keep a backup of the project and local Route state before trying this
candidate on valuable work. Do not manually rewrite append-only history.
There is no published migration guarantee for this untagged candidate.
`route/1` remains the protocol identifier; it is not renamed to match a
future product version. See [candidate install and use](v1-release-candidate.md).
