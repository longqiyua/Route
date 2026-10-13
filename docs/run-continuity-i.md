# Route Run Continuity I — implementation and acceptance record

Date: 2026-10-12–13. Platform: Microsoft Windows NT 10.0.26200.0;
Rust 1.95.0. Starting branch `fruit`, HEAD
`eb9751ba3d563e116ffcf0bf36a102c840cc0fca`; initial remote divergence 0/0.
The mixed worktree and previous Execution Trust I implementation are preserved.
This is OS process continuity, not AI session continuity.

## Architecture and authority boundary

The logical check obligations, durable CheckRun/Team Run and OS identity are
separate. `RunIdentity` contains run_id, operation_id, owner and attempt.
Its optional explicit team_goal distinguishes Team and CheckRun domains;
operation-key text is never used to infer the domain.
`ProcessIdentity` contains PID, Windows creation FILETIME, canonical executable
path and SHA-256. A random launch nonce binds an independently named Job Object
to the canonical checkout and exact Run identity.

Authoritative process receipts and cancellation/recovery journals live outside
the shared project, under the local user's
`AppData/Local/Route/processes/<canonical-checkout-digest>/`. No raw prompt,
credential, inherited environment values or transcript is stored there.
Project files, handoff text, executable names and arbitrary PID metadata do not
grant process-control authority. Copying a receipt to a different checkout does
not create a valid local binding. This storage is local runtime state, not a
Git-tracked product file or user authentication change.

This is NOT isolation against a malicious, unrestricted program running as the
same OS user, which could already control that user's processes/private files.
Worker RPC cannot call this Operator-only CLI surface. Existing sandbox and
credential boundaries remain necessary; this phase does not expand them.

## Windows primitive: investigated, then corrected by real tests

Creation uses documented `CreateProcessW`, `STARTUPINFOEXW`,
`PROC_THREAD_ATTRIBUTE_JOB_LIST` and `CREATE_SUSPENDED`. Job association happens
atomically at process creation, not in a post-spawn PID assignment race.
Only an explicit `.exe` and CRT-escaped argv are accepted; scripts need an
explicit approved interpreter. An explicit handle list restricts inheritance
to standard streams and a query-only Job lifetime lease. No inherited Job
terminate/set-limit capability is given to the root child.

The initial experiment without an inherited lease failed: after the supervisor
crashed, reopening the named Job returned no object. A surviving kernel Job
reference must not be confused with an open named handle. That design was
replaced, not accepted as a pass.

Final policy: `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, no breakaway flags, plus an
inherited **query-only lifetime handle in the root child**. While that handle
survives, a new Route process can reopen and inspect the same named Job.
If an executable deliberately closes inherited nonstandard handles, continuity
may end when Route exits; the last-handle policy kills the contained tree rather
than leaving an unopenable orphan. No daemon, service, startup task or extra
supervisor process is installed. Parent exit alone does not release capacity
while Route still observes live Job members.

Primary documentation:

- [Windows Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)
- [Atomic assignment with a Job list](https://devblogs.microsoft.com/oldnewthing/20230209-00/?p=107812)
- [OpenJobObjectW and inherited handles](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-openjobobjectw)
- [Nested Jobs](https://learn.microsoft.com/en-us/windows/win32/procthread/nested-jobs)

Containment covers the documented CreateProcess descendant hierarchy, not
arbitrary work delegated through WMI, an external service/broker or another
machine. In particular, no real Codex/mxc containment claim is inferred from
these fixtures. Outer/nested Job restrictions can reject creation; Route fails
closed without an uncontained fallback. Windows may add a console-host process
to the Job, so fixture membership can exceed the three named fixture processes.

## Spawn, result and cancellation truth

Durable stages are BEFORE_SPAWN, SPAWN_ATTEMPTED_BUT_IDENTITY_NOT_DURABLE,
OWNED_PROCESS_IDENTIFIED, RUNNING, PROCESS_EXIT_CAPTURED and
TERMINATED_WITHOUT_RESULT. CheckRun's existing RESULT_PERSISTED /
EVIDENCE_COMMITTED remain the authorities for command result and System proof.

The attempted stage is saved before CreateProcess. The child starts suspended,
already contained. Actual identity and executable hash are persisted before
ResumeThread. Crashes before identity persistence leave a discoverable owned
Job or explicit uncertainty, never permission to launch the same Run again.
Neither inspect nor recover automatically resumes a suspended thread.

An OS exit, a durable exit-code receipt, complete command output, committed
Evidence and verified Work are different facts. `PROCESS_EXIT_CAPTURED` does
not mean CheckRun result or Evidence exists. After supervisor loss, stdout/stderr
continuity is UNKNOWN; no daemon/spool is introduced to fabricate complete logs.
Disappearance without a durable CheckRun result is LOST_RESULT, not PASS/FAIL.
A missing named Job is not proof that asynchronous last-handle termination has
finished. Unless a zero-member observation was durably recorded beforehand,
`active_processes` remains null, cancellation remains unconfirmed, capacity
stays quarantined and acknowledgement is rejected. Inspect never writes this
proof; only explicit cancellation, result capture or acknowledged recovery does.

Cancellation persists CANCEL_REQUESTED before signalling the proven Job,
CANCEL_SIGNALLED after the OS call, then TERMINATION_CONFIRMED only after an
observed empty Job. A bounded five-second confirmation failure remains
CANCEL_UNCONFIRMED. A restart retries the same operation key. Conflicting keys
are rejected. Already-completed results are not rewritten as cancellation.
No cancellation uses taskkill, process names, command substrings or PID-only
termination. Identity mismatch fails closed; unrelated processes are untouched.

## CLI and recovery

```text
route run inspect RUN_ID
route run cancel RUN_ID --operation-key UNIQUE_KEY
route run recover RUN_ID --operation-key UNIQUE_KEY
route run recover RUN_ID --operation-key UNIQUE_KEY --ack-lost-result
```

Inspect is read-only, including the local runtime store. It reports what is
known, active managed members, uncertainty, output limitations and a safe next
action. Plain recover is an explicit keyed observation journal. Its sole safe
retry exception is a proven BEFORE_SPAWN CheckRun with no created process:
explicit recover restores the original reservation, and an explicit subsequent
execute retakes owner/attempt and archives the old launch receipt before spawn.
It never silently launches a process. All other unknown/possibly-running Runs
retain or quarantine capacity.
Default `team status` keeps its previous JSON contract. Opt-in
`team status GOAL_ID --budget` adds a conservative active-reservation quarantine
indicator and process-inspection hint; per-Run `run inspect` provides stronger
ownership knowledge. These views do not release slots.

`--ack-lost-result` is a deliberate Operator action, accepted only for a proven
terminated tree without a durable result. An audit record is written before
canonical capacity release. CheckRun/Team gains additive `LOST_RESULT` terminal
state; this is not a successful or failed execution. Original Run, contract,
owner and attempt remain intact. Same-key CheckRun execution is still forbidden;
any new observation needs a separately approved new operation key. No Work or
Goal is finalized by acknowledgement. Crash/replay of acknowledgement is safe.
These additive CLI and serialized state changes require compatibility review by
consumers that exhaustively match terminal states.

## Real evidence (not AI Host certification)

`process_supervision::tests` uses real Windows root/child/grandchild processes,
including a separate supervisor that exits with code 23 without cleanup.
`run_continuity` launches the actual Route binary with an approved fixture check,
kills only that exact owned Route Child, then invokes a fresh Route binary for
inspect, cancel and keyed release. Observed duplicate_process_launch_count = 0.

Focused security cases include a real unrelated process substituted for the
recorded PID while preserving the old creation identity, forged shared-project
PID records, changed executable path, foreign checkout binding, malformed
receipt and stale owner/attempt writes. The unrelated process is verified alive
after denied control. The test cleans up only its original exact Child handle.

Crash extension:

| Case | External reality and restart action | Forbidden action |
| --- | --- | --- |
| G | Identified live tree survives supervisor exit; reopen same Job | Duplicate launch |
| H | Tree exits after supervisor loss; LOST_RESULT, no invented code; missing Job without durable empty-tree proof retains quarantine | Success/Evidence or capacity release from disappearance |
| I | Simulated reused PID points to a different real live process; identity mismatch | Attach/kill unrelated process |
| J | Supervisor exits after durable CANCEL_REQUESTED; same-key retry confirms empty tree | Pretend request was confirmation |
| K | Parent exits with descendants alive; owned wait stays pending until Job empty | Free slot on parent exit |
| L | Stale owner/attempt cannot write result or cancellation; live-process fixture included | Late result as canonical success |

Additional exact seams exercise BEFORE_SPAWN, immediately after CreateProcess
before durable identity, and after identity before ResumeThread. Test subprocesses
self-expire as a safety guard; no age-based production takeover is introduced.

Initial full-suite execution exposed fixture readiness deadlines under parallel
heavy executable hashing. New OS fixture suites are serialized and isolated from
legacy process-global environment mutation. Production deadlines, hashes and
ownership checks are not relaxed. Final regression and readiness results are
recorded below; pending and ignored checks are not acceptance evidence.

## AI Host and portability

Codex read-only availability check: `Not logged in`, exit 1.
RealAIHostRestart = NOT_RUN (external validation dependency, not code blocker).
Authentication was not modified. No browser login, credentials, account switch
or semantic reconnect was attempted.

Windows is the implemented containment platform. Other platforms keep existing
direct-child behavior and return explicit UNSUPPORTED for restart/tree control.
Linux/macOS continuity is NOT_RUN, not inferred from compilation.

## Final regression and performance

ROUTE_RUN_CONTINUITY_I = **PASS**, limited to the Windows OS-process substrate
and conservative recovery contract documented here, not AI session continuity.
Raw local logs: `Release/local-tools/run-continuity-20261012/` (ignored, not a
public Release). Final commands used `--offline` for Cargo tests.

| Gate | Actual exit | Result |
| --- | --- | --- |
| cargo fmt --manifest-path tool/route/Cargo.toml --all -- --check | 0 | PASS |
| cargo test --manifest-path tool/route/Cargo.toml -p route-core | 0 | 25 unit + 1 doc passed |
| cargo test --manifest-path tool/route/Cargo.toml -p route-basic | 0 | 476 passed, 14 ignored |
| cargo test --manifest-path tool/route/Cargo.toml -p route-cli | 0 | 94 passed, 5 ignored across targets |
| cargo test --manifest-path tool/route/Cargo.toml --workspace --exclude route-pyo3 | 0 | All selected targets passed; 19 ignored |
| Focused Windows process-supervision suite | 0 | 9 passed, 1 helper ignored |
| Actual CLI restart/security suite, repeated with metrics | 0 | 2 passed, 1 helper ignored |
| scripts/check-docs.ps1 | 0 | check-docs: OK |
| PowerShell parser | 0 | 11 scripts, zero parse errors |
| git diff --check | 0 | PASS |

Do not sum nested subprocess test summaries into parent-suite counts. Ignored
helper entries are exercised by active parent tests, but remain ignored as
standalone tests. Manual resource/performance probes, including the 2 GiB
dual-pipe resource baseline, were not rerun and are not claimed PASS here.
`route-pyo3` was excluded, not validated.
Native integration tests required permission to write their authoritative local
runtime receipts outside the checkout. A final sandbox-only fmt recheck returned
exit 1 because canonicalizing checkout ancestors was denied; rerunning the same
check with the approved validation permissions returned exit 0. That environment
error is not counted as a successful check. Final remote fetch also returned 0.

Regression history is retained: an earlier full basic run found a missing
CheckRun lock directory in Team-only recovery; the mutation now initializes
that directory without changing inspect. An earlier CLI run found default
`team status` JSON incompatibility; the old output was restored and additional
fields moved to opt-in `--budget`. The original compatibility assertion remains.
An earlier Open Society concurrent append test failed before diagnostic context
was printed. Only assertion diagnostics were improved; no concurrency assertion
was weakened or unrelated production logic changed. Subsequent complete CLI and
workspace runs both passed that test; its original transient cause is not proven.

Measured debug-build sample from `restart-metrics.log`, separate from the
fixture's intentional lifetime and Cargo compilation:

| Operation | Observed milliseconds | Measurement boundary |
| --- | --- | --- |
| Supervision setup | 392.136 | Executable hash, native setup and pre-spawn receipt work |
| Process identity persistence | 14.270 | Durable identity receipt write |
| Live inspect | 407.495 | Fresh CLI invocation while original Route remains alive |
| Restart inspect | 420.512 | Fresh CLI invocation after original Route is killed |
| Cancel request persistence | 5.021 | Durable request write, not whole cancellation |
| Termination confirmation | 0.006 | Post-signal zero-member confirmation window |
| Restart/cancel/release combined | 937.335 | CLI calls including keyed release/replay |

Separate direct-supervisor fixture measured restart inspect 596.338 ms and
cancel plus whole-tree confirmation 619.648 ms (`focused-acceptance.log`),
including rehashing its larger debug executable. These are single-run debug
observations, not thresholds, medians or release-binary latency claims.
Known overhead includes executable hashing and existing O(n) ledger/dedup scans.

## Readiness and final structured reconciliation

DAILY_USE_VERIFIED below means the explicit Windows OS fixture/CLI contract,
not certification of arbitrary AI Hosts, external launch brokers or platforms.

| Capability | Readiness |
| --- | --- |
| ProcessIdentity | DAILY_USE_VERIFIED (Windows) |
| ProcessOwnership | DAILY_USE_VERIFIED (Windows, trusted local Operator boundary) |
| ProcessTreeContainment | DAILY_USE_VERIFIED (Windows CreateProcess hierarchy) |
| RestartProcessInspection | DAILY_USE_VERIFIED (Windows, inherited lease retained) |
| CancellationSemantics | DAILY_USE_VERIFIED (Windows, unconfirmed remains unconfirmed) |
| RunRecovery | DAILY_USE_VERIFIED (Windows, explicit keyed transitions only) |
| RunFencing | DAILY_USE_VERIFIED (Windows real-process + prior CheckRun tests) |
| RestartContinuity | DAILY_USE_VERIFIED (OS process recognition, not semantic reconnect) |
| RealAIHostRestart | NOT_RUN (Codex not logged in; authentication unchanged) |

```text
ROUTE_RUN_CONTINUITY_I: PASS
BASE:
  branch=fruit
  starting_HEAD=eb9751ba3d563e116ffcf0bf36a102c840cc0fca
  mixed_worktree_preserved=YES
PREVIOUS_EXECUTION_TRUST:
  Q17_still_green=YES (one physical execution, two logical obligations)
  Evidence_recovery=PASS
  finalization_recovery=PASS
  crash_A-F=PASS
PROCESS_IDENTITY:
  fields=run_id, operation_id, owner, attempt, team_goal, pid, creation_FILETIME,
         canonical_executable, executable_SHA256, launch_nonce, named_Job
  PID-only=NO
  PID_reuse_protected=YES (simulated reuse, real unrelated target)
PROCESS_OWNERSHIP:
  platform=Windows 10.0.26200.0
  mechanism=atomic Job-list assignment, suspended creation, query-only inherited lease
  ownership_proof=private canonical-project receipt plus exact Job/process identity
  forged_metadata_protection=shared PID/handoff text grants no authority
PROCESS_TREE:
  fixture_tree=real parent -> child -> grandchild (plus possible console host)
  containment=Windows Job, no breakaway fallback
  termination_confirmed=observed zero members, persisted for later inspection
  limitations=external brokers/WMI/services/remote work not covered
SPAWN_HANDSHAKE:
  before_spawn=proven absent; explicit reservation recovery and retry with history
  after_spawn_before_identity=contained suspended Job or explicit uncertainty
  after_identity=durable identity before ResumeThread
  duplicate_launch_possible=NO in tested crash/replay paths; UNKNOWN never relaunches
RESTART:
  G=PASS same owned live tree recognized
  H=PASS LOST_RESULT, no invented exit; missing Job without proof keeps quarantine
  I=PASS simulated PID reuse rejects unrelated real process
  J=PASS persisted cancel request retried and confirmed
  K=PASS parent exit does not release live descendants
  L=PASS late old callback fenced after safe release/new live Run
RESTART_DOGFOOD:
  Route_process_1=actual route.exe team verify, terminated by its exact Child handle
  owned_process_survived=YES
  Route_process_2=fresh route.exe run inspect/cancel/recover
  same_process_recognized=YES (same run_id and parent PID)
  duplicate_process_launch_count=0
  final_result=LOST_RESULT, tree terminated, audited release; no fabricated Evidence
CANCELLATION:
  request_persisted=YES
  signal=TerminateJobObject for proven owned Job only
  tree_behavior=whole contained tree
  confirmation=zero active members or explicit CANCEL_UNCONFIRMED
  unknown_processes_killed=NO
PID_REUSE:
  test=real unrelated process substituted for recorded PID with old creation identity
  unrelated_process_touched=NO (no control; read-only identity query permitted)
FENCING:
  late_old_attempt_result_accepted=NO
  late_old_attempt_Evidence_accepted=NO
  qualification=old tree first proven terminated; stale callback arrives after new Run
CAPACITY:
  uncertain_Run_consumes_or_quarantines_slot=YES
  silent_capacity_release=NO
OUTPUT_CONTINUITY:
  survives=durable Run identity, receipts and already captured canonical results
  UNKNOWN=stdout/stderr after supervisor death; missing durable result/termination proof
RECOVERY_UX:
  states=RUNNING, BEFORE_SPAWN, RESULT_PERSISTED, LOST_RESULT, RECOVERY_REQUIRED,
         CANCEL_REQUESTED, CANCEL_SIGNALLED, TERMINATION_CONFIRMED, CANCEL_UNCONFIRMED
  safe_next_actions=inspect; keyed owned cancellation; explicit keyed recovery;
                    acknowledge lost result only after tree termination proof
REAL_AI_HOST:
  Codex_status=Not logged in (read-only check, exit 1, performed once)
  authentication_modified=NO
  RealAIHostRestart=NOT_RUN
WINDOWS:
  real_OS_tests=PASS
  readiness=WINDOWS_DAILY_USE_VERIFIED within documented OS substrate scope
OTHER_PLATFORMS:
  Linux_macOS_continuity=NOT_RUN / UNSUPPORTED
  existing_direct_child_behavior=retained, not upgraded to tree guarantees
GIT:
  commits=NONE (mixed provenance/overlapping previous phase; no unrelated staging)
  push=NO
  local_SHA=eb9751ba3d563e116ffcf0bf36a102c840cc0fca
  remote_SHA=eb9751ba3d563e116ffcf0bf36a102c840cc0fca
  final_fetch_exit=0
  ahead_behind=0/0
MAIN_MODIFIED:NO
TAG_CREATED:NO
RELEASE_CREATED:NO
FORCE_PUSH:NO
YUICH_CHANGED:NO (no changes by this task; pre-existing mixed-tree deletions preserved)
```

Known limitations: no hostile same-OS-user isolation; inherited lease may be
closed by a Host; a lost Job name without durable termination proof cannot be
released by this recovery surface; no uninterrupted pipe capture or AI semantic
reconnect; no cross-platform certification; hash/scan costs remain; existing
unrelated compiler warnings were not bulk-fixed. New serialized fields default
conservatively for older receipts. Additive LOST_RESULT consumers need review.

## Git and scope

No reset, checkout-away, stash, rebase, force push, tag, public Release, main
worktree mutation or Yuich edit. Do not stage the mixed historical changes as
though they belonged solely to this phase. The previous phase report remains
historical; this supplement supersedes only its OS process-continuity limits
once the new acceptance gates are verified.
