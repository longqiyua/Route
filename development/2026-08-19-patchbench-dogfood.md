# PatchBench — Route Dogfood Development Record

> Evidence labels: `REAL_OBSERVED` = real remote push; `LOCAL_OBSERVED` = executed
> CLI/tests/filesystem on this machine; `DETERMINISTIC_FIXTURE` = controlled demo/fixture.
> No hidden reasoning or AI chain-of-thought is stored here, by policy.

- **Date:** 2026-08-19
- **Workspace:** sandbox branch (this branch) / fruit branch (result artifact)

## Goal

Let `fruit` truly hold a runnable, standalone tool produced through Route dogfood:
**PatchBench**, a small patch-verification utility. `sandbox` records the real
development process. `main` (Route v1.0 beta) was **not** changed.

## Why

Repeatedly, a development working group makes a repo change and needs bounded,
deterministic verification (changed-file scope, executed checks, and
claims-vs-evidence). That recurring need is a good candidate to externalize as a
small independent tool, instead of growing Route Core.

## Yuich involvement

Yuich directed the intent (`cap:develop-software`, `handle:route`) and identified
the externalization opportunity, but did not execute the implementation. The
implementation and verification were performed by Host AI as the AI Worker under
a Route-style TaskSpec. Yuich's real runtime was not invoked end-to-end this run;
where the true Yuich runtime did not run, it is marked `NOT_RUN`.

## DevelopmentIntent summary

Externalize bounded deterministic patch verification as a standalone developer
tool; keep it tiny, stdlib-only, host-neutral; do not extend Route Core.

## Route plan

- P2/P17: Worker TaskSpec scopes edits to `tools/patchbench/**`; forbid touching
  `yuich/**`, Route main product source, or sandbox production content.
- P13: smallness budget (< ~1000 LOC), no database / LLM / MCP / web / daemon.
- P19: fruit commit first → capture SHA → sandbox records that SHA.
- P22: `MAIN_CHANGED=NO`.

## TaskSpec (Route → AI Worker)

```json
{
  "goal": "Build tools/patchbench/ as a standalone patch-verification CLI.",
  "allowed_paths": ["tools/patchbench/**"],
  "forbidden_paths": ["yuich/**", ".route/**", ".route-basic/**", "sandbox/**"],
  "checks": ["python tools/patchbench/tests/run_tests.py"],
  "claims": ["scope=in_scope", "self-test suite passes"]
}
```

## AI Worker role

Host AI implemented PatchBench in Python (standard library only). It is the
Worker; it is **not** claimed as Yuich, and not conflated with Route's own engine.

## Implementation

- Language: **Python 3.11** (stdlib only — no third-party packages, no build step).
- Single entry file: `tools/patchbench/patchbench.py` — **736 LOC** (under the
  ~1000 budget; no extension needed).
- Supports `run` / `compare` / `demo`, `--json`, `--version`, exit codes 0/1/2.

## Files

- `tools/patchbench/patchbench.py`
- `tools/patchbench/README.md`
- `tools/patchbench/examples/task.json`, `examples/self-task.json`
- `tools/patchbench/examples/fixture/{app.py, tests/test_app.py}`
- `tools/patchbench/examples/extern-demo/**` (isolated `src/` + `tests/` demo)
- `tools/patchbench/tests/run_tests.py`

## Tests

- `python tools/patchbench/tests/run_tests.py` → **29 passed, 0 failed** (`LOCAL_OBSERVED`).
- Standalone copy (`--help`/`demo`/`run`/`compare`/`--json`) ran from a temp dir
  with no Route/Yuich available → all `rc=0` (`LOCAL_OBSERVED`).

## Failures (first attempt)

- Demo `changed-files` detection: git folds fully-untracked dirs (`?? src/`); the
  directory was not expanded into files. Fixed by walking the collapsed dir.
- Example command relied on `&&` under `shell=False`. Refactored to a single-line
  `python -c` step (compile + run the test).
- Self-test fixture asserted `docs/**` should be `OUT_OF_SCOPE`; implemented
  semantics give forbidden patterns priority (`FORBIDDEN`), the stricter and
  correct behavior. Test fixture corrected; it now asserts the file is reported.
- `verify_claims()` did an unsafe `c['exit_code']` index on synthesized pure tests;
  switched to `c.get('exit_code')` for robustness.
- Example-run test passed `examples/task.json` with the subprocess inheriting the
  repo root as cwd; the path resolves relative to `tools/patchbench`. The test
  helper now accepts an explicit `cwd`.

Rejected attempts: none destructive. No snapshot/rollback was needed this run;
verified worktree stays append-only.

## Standalone result

Copied `tools/patchbench/` to an isolated temp dir (no Route, Yuich, sandbox, or
main). Verified `--help` (rc=0), `demo` (rc=0, `DETERMINISTIC_FIXTURE`),
`run examples/task.json --json` → `VERIFIED` (rc=0), and `compare` → `UNCHANGED`
(rc=0). `LOCAL_OBSERVED`.

## Benchmark result

- Self dogfood: `run examples/self-task.json --repo tools/patchbench --json` →
  `VERIFIED`, self-test check `PASS`, 2 claims `VERIFIED` (`LOCAL_OBSERVED`).
- External demo (isolated `src/`+`tests/`): `task_good.json` → `VERIFIED` (rc=0);
  `task_bad.json` (touches forbidden `secret.txt`) → `FAILED` (rc=1), violation
  `FORBIDDEN`, scope claim `CONTRADICTED` (`LOCAL_OBSERVED`, `DETERMINISTIC_FIXTURE`).

## Evidence labels

- `REAL_OBSERVED`: fruit push `e499ce1..dd4a036` `git push origin fruit`.
- `LOCAL_OBSERVED`: CLI/tests executed above.
- `DETERMINISTIC_FIXTURE`: `demo` and `extern-demo` scenarios.
- `NOT_RUN`: true Yuich runtime end-to-end; autonomous check-network install.

## Route learned

- Externalizing a verification utility is low-risk when scope is tiny, the Worker
  TaskSpec is explicit, and verification covers the real acceptance criteria.
- Check-execution semantics (never trust the AI claim string) carry over cleanly
  from Route to a standalone tool.

## Yuich learned

- Bounded deterministic verification can be externalized as a reusable development tool.

## Fruit commit

`dd4a0367b2e96e36a57bd488d6ccfe6fc4942fbd` — "fruit: add PatchBench - standalone
patch verification tool (Route dogfood)". Pushed to `origin/fruit` (`REAL_OBSERVED`).
Path: `tools/patchbench/`. 12 files changed.

## Outcome

`FRUIT_HAS_REAL_RESULT=YES`, `PATCHBENCH_STANDALONE=PASS`,
`SANDBOX_HAS_MATCHING_PROCESS=YES`, `MAIN_CHANGED=NO`.