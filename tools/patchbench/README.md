# PatchBench

**A small standalone patch verification tool for AI-assisted development.**

A developer or an AI working group changes a repo, produces a `task.json` spec, then
PatchBench deterministically verifies the patch against it. PatchBench does **not** run
an AI and is **not** tied to Route, Yuich, or any model. Any host — Claude, Codex,
ChatGPT, Gemini, a human, Route, Yuich — can write the spec; PatchBench only executes it.

> **AI generates the evaluation spec. PatchBench deterministically executes it.**

PatchBench is a demo/experimental result produced on the `fruit` branch through Route
dogfood. It is not the Route product — for Route itself use `main`.

## What it does

```
patchbench run <task.json> [--repo DIR] [--files changed.json] [--save out.json] [--json]
patchbench compare <before.json> <after.json> [--json]
patchbench demo [--json]
patchbench --version
```

`run` validates:

| Concern | How |
|---------|-----|
| changed files | `--files` explicit JSON, or `task.files.changed`, or `git status` when available |
| allowed scope | every changed file must match an `allowed_paths` pattern (when non-empty) |
| forbidden paths | no changed file may match a `forbidden_paths` pattern |
| checks | each declared `command` is really executed (with timeout), not trusted |
| claims | AI claims are checked against real executed evidence, not the claim string |

Verdict: **VERIFIED / FAILED / INCOMPLETE**.

## Task schema

```json
{
  "goal": "describe the intended change",
  "repo": "path-or-empty (defaults to --repo or cwd)",
  "allowed_paths": ["src/**", "tests/**"],
  "forbidden_paths": [".github/**", "docs/private/**", "Cargo.lock"],
  "files": { "changed": ["src/app.py"] },          // explicit list (optional)
  "checks": [
    { "command": "python -m pytest tests/", "timeout": 60, "expected_exit_code": 0 }
  ],
  "claims": [
    { "text": "only source was touched", "kind": "scope", "condition": "in_scope" },
    { "text": "tests pass", "kind": "check_ref", "ref": 0 }
  ],
  "metrics": []                                     // optional declarative metrics
}
```

## Claim ≠ evidence

An AI claim like `"tests passed"` is **NOT_VERIFIED** until PatchBench actually runs the
test command. Only when the executed `exit_code` matches `expected_exit_code` does the
claim become **VERIFIED**; a mismatch yields **CONTRADICTED** (→ `FAILED`).

## Security boundary

PatchBench is **verification only**:
- no automatic `sudo`
- no installing dependencies, no network access
- no `git push` / `git commit`
- no deleting project files
- checks run with a timeout

## Exit codes

| code | meaning |
|------|---------|
| `0` | VERIFIED |
| `1` | FAILED (scope violation, failed check, or contradicted claim) |
| `2` | INVALID task / INCOMPLETE environment |

## Requirements

Python 3.10+ standard library only. No third-party packages, no install step:

```sh
python patchbench.py --help
python patchbench.py demo
python patchbench.py run examples/task.json --repo examples/fixture --json
```

External demo (a small isolated `src/` + `tests/` project — correct patch becomes
`VERIFIED`, a forbidden-path touch becomes `FAILED`):

```sh
python patchbench.py run examples/extern-demo/task_good.json --repo examples/extern-demo --json   # VERIFIED (exit 0)
python patchbench.py run examples/extern-demo/task_bad.json  --repo examples/extern-demo --json   # FAILED  (exit 1)
```

## Tests

```sh
python tests/run_tests.py
```

## License

See the `fruit` branch license context. This is an experimental dogfood result, not the
Route product release.