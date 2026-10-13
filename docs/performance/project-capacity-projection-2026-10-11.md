# Project capacity projection: local debug timing

This measures only the pure capacity calculation, not CLI startup, disk reads,
ledger verification, real AI calls or peak RSS. It is not a production SLO.

Command (from `tool/route`):

```powershell
cargo test -p route-basic capacity_projection_timing -- --ignored --nocapture
```

Actual exit code: 0. Both implementations run in the same test binary and consume
the same already-resident synthetic 10,000-event, 16-Goal history. Routing decision
reason strings are 16 KiB each to exercise irrelevant payload copies. Configure,
Reserve, Transition, CancelAll, Decide, extraction and unrelated events are mixed;
reconfiguration resets occur, so this is not a steady-growing active-run workload.
The fixture is not an authenticated ledger or a real AI-host workload.

Thirty samples per implementation, full replay first, narrow replay second;
sorted sample indices 14 / 28 report nearest-rank p50 / p95. No cold/warm separation,
machine isolation, order randomization or release-mode run was performed.

| Implementation | p50 ms | p95 ms |
| --- | ---: | ---: |
| Previous full TeamView replay per Goal (test oracle) | 6.801 | 7.968 |
| Single-pass narrow capacity replay | 1.330 | 1.456 |

Raw emitted lines:

```text
CAPACITY_TIMING full_replay events=10000 goals=16 samples=30 p50_ms=6.801 p95_ms=7.968
CAPACITY_TIMING narrow_replay events=10000 goals=16 samples=30 p50_ms=1.330 p95_ms=1.456
```

The source change removes repeated full-history projection for each configured
Goal and avoids copying decisions, knowledge jobs and complete reservations just
to count concurrency. It retains borrowed Goal/run IDs and small run-state values.
Ledger loading, verification and the requested Goal's full projection are unchanged.
Peak RSS, allocated-byte totals, end-to-end latency, 100K events and release binary
behavior are NOT_MEASURED. No claim is made about the original 418 MiB process
observation, its root cause or the memory usage of a currently running executable.

The old implementation remains test-only to check equivalence; it is not a second
production store or a runtime fallback. Normal full regression excludes this
explicit timing test, so measurements do not become flaky timing assertions.
