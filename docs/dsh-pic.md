# DSH / PIC Driver — machine-to-machine example

This is a **conceptual example** of how an external harness (DSH/PIC-style) drives
Route through the CLI. It is **not** a new Route runtime, and Route does not run,
schedule, or spawn any harness. Route supplies policy, state, and verification;
PIC supplies orchestration; DSH supplies execution.

> Status: **example / conceptual**. No `route://` URI, no HTTP, no Cordis plugin,
> no agent runtime. Everything here is the existing `route` CLI invoked as a
> subprocess. The commands shown exist and are machine-readable with `--json`
> (see `docs/harness.md`).

## Roles

| Role | Responsibility |
| --- | --- |
| **Route** | Persistent state, policy, evidence, verification, recommendation. |
| **PIC** | Orchestration loop: ask Route for the next action, dispatch to a harness, collect results. |
| **DSH** | Actual execution: shell / filesystem / code work on the candidate. |

## The loop

```
┌────────────────────────────────────────────────────────────┐
│ PIC driver (TypeScript / Code Mode, conceptual)            │
│  1. campaign next --json            get next_action        │
│  2. context --task --json           inspect required ctx   │
│  3. checkpoint                      record a savepoint     │
│  4. DSH: shell/filesystem work      execute candidate      │
│  5. verify / benchmark              measure outcome        │
│  6. campaign report --outcome       report evidence        │
│  7. repeat from 1                   until STOP/NEEDS_HUMAN  │
└────────────────────────────────────────────────────────────┘
```

Route never says "spawn vendor X". The `next_action` contract expresses
**capabilities** (`filesystem`, `shell`, `web`, `subagents`, `code_mode`,
`skills`, `workflows`). The harness decides how to satisfy them.

## TypeScript-like driver (conceptual)

```ts
// PIC driver — orchestrates, does not execute the model itself.
// Route = policy/state, PIC = orchestration, DSH = execution.

import { spawnSync } from "node:child_process";

const ROUTE = "route"; // the route binary on PATH

function route(args: string[], json = false): any {
  const r = spawnSync(ROUTE, json ? [...args, "--json"] : args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (r.status !== 0) throw new Error(`route ${args[0]} failed: ${r.stderr}`);
  return json ? JSON.parse(r.stdout) : r.stdout;
}

async function main(): Promise<void> {
  const campaign = process.argv[2];
  if (!campaign) throw new Error("usage: pic-driver <campaign-id>");

  for (let i = 0; i < 20; i++) {
    // 1. obtain the next action contract (machine-readable).
    const next = route(["evolve", "campaign", "next", campaign], true);

    const action = next.next_action;
    if (action === "stop" || action === "needs_human") {
      console.log(`[pic] terminal action -> ${action}`);
      return;
    }
    if (!next.satisfiable) {
      console.log(
        `[pic] cannot satisfy ${next.required_capabilities.join(", ")} ` +
          `(missing ${next.missing_capabilities.join(", ")}) -> needs_human`,
      );
      return; // never silently pretend execution is possible
    }

    // 2. inspect the Route context the action requires.
    if (next.context_required) {
      const ctx = route(["context", "--task", next.goal], true);
      // ctx contains context_hash, selected refs, policy — use as input.
      console.log(`[pic] context_hash=${ctx.context_hash}`);
    }

    // 3. record a checkpoint before mutating anything.
    if (next.checkpoint_required) {
      route(["checkpoint", `pre-${action}-${i}`]);
    }

    // 4. DSH executes the candidate (shell/filesystem/code work).
    const candidate = await runCandidateWork(next); // DSH, not Route

    // 5. run verification / benchmark on the candidate.
    const tests = await runVerification(candidate);

    // 6. report evidence back to Route (UNTRUSTED input).
    const report = {
      campaign_id: next.campaign_id,
      experiment_id: next.experiment_id ?? `exp-${i}`,
      session_id: next.task_id ?? undefined,
      execution_status: tests.ok ? "success" : "failed",
      candidate: candidate.id,
      observed_changes: candidate.changedFiles,
      tests: tests.names,
      benchmarks: tests.names,
      evidence_refs: tests.evidenceRefs, // real Route evidence ids
      metrics: tests.metrics,
      failure: tests.failure,
      harness: "dsh-pic", // provenance, not a new campaign
    };
    route(["evolve", "campaign", "report", campaign, "--outcome",
      JSON.stringify(report)]);

    // 7. loop: ask Route for the next action again.
  }
  console.log("[pic] budget exhausted");
}
```

## Key contracts

- `next.next_action` ∈ `execute_experiment | diversify | replicate | ablate |
  stop | needs_human`.
- `next.satisfiable === false` means the harness lacks a required capability;
  the driver MUST stop/degrade, never pretend execution succeeded.
- `route evolve campaign report --outcome <json>` ingests **UNTRUSTED** input.
  `execution_status: "success"` from the harness never equals a trusted PASS.
  Route resolves trust through its own evidence hierarchy only.
- Re-submitting the same report is idempotent (`duplicate`); a disagreeing
  re-submission is recorded as `conflict` evidence.
- A harness switch (e.g. DSH → generic host) is **provenance**, not a new
  campaign: campaign id, task id, baseline, and lineage stay continuous.
- A campaign never silently marks its parent Task Succeeded. It only yields a
  recommendation; the Task must still pass its own verification.