# Recovery

Route's recovery model keeps a persistent, project-independent archive so you
can restore a single file, a partial set, or an entire deleted project.

## The Archive

- **Location:** `Documents/Route/projects/<project-id>/`
- **Registry:** `Documents/Route/registry.json`
- The archive is independent of the project directory. Deleting the project
  does not delete the archive.

## Original

`route init` creates the **Original** archive save — an immutable baseline of
the project at initialization time. It is never overwritten and is the source
of truth for disaster recovery.

## Save

A **save** is a snapshot of project state stored in the archive.

```bash
route archive init                     # (re)initialize archive + Original
route archive save "reason"            # manual save
route archive list                     # list saves
route archive show <id>                # save details
route archive diff <a> <b>             # compare two saves
route archive path-history <path>      # change history of one file
```

Saves are also created automatically: on init (Original), on task start
(PRE_CHANGE), and on successful verification (VERIFIED).

## KnownGood

**KnownGood** is the last verified, promoted configuration. It is the safe
fallback used by recovery and rollback. It is a trust root that candidates
cannot modify.

## Selective Restore

You can restore a subset of files instead of everything.

```bash
route archive restore <id> --scope paths --paths "src/broken.rs,src/other.rs"
```

Restore scopes:

| Scope | Behavior |
|-------|----------|
| `full` | Restore the whole project |
| `project` | Restore project files |
| `route-state` | Restore Route state (`.route/`) |
| `paths=<...>` | Restore specific paths (use `--paths`) |

Restore **defaults to preview first** and **auto-saves before** applying
(PRE_RESTORE). File writes are atomic and hash-verified.

## Deleted-Project Recovery (Beta)

> **Beta.** This flow restores a deleted project from the archive. It is not a
> general disaster-recovery guarantee — it restores what was saved, and only
> works if the archive itself is intact.

If the project directory is deleted, the archive survives.

```bash
route archive recover <project-id> --save <id> --to /new/path
```

Or inspect recovery options first:

```bash
route archive recover-list <project-id>
```

This restores all saved project **files**, verifies content hashes, and updates
the registry.

**Known limitation (Beta):** `recover` does **not** restore the `.route` project
state (config, constitution, task/history). The recovered directory is a plain
file tree — run `route init` there to re-establish a Route project. Because the
original project identity is derived at init, the recovered project receives a
**new** project ID and its prior task history is not carried over. Treat the
recovered tree as the user's files restored, not as the original project resumed.

## Abandoned-Project Takeover (protocol)

Recovering files is only half of continuity. A fresh AI taking over an
abandoned project must reconstruct `CurrentState / Unknowns / BrokenAreas /
RecoverableAssets / RecommendedNextAction` from **project files + Route
state + Evidence + History + Architecture Memory + References + user
intent** — never from old chat history. Engine surface today is the
deleted-project file recovery above (`.route` state is not carried — known
Beta limitation); the full takeover protocol is defined in
[maintenance.md §14](maintenance.md#14-abandoned-project-takeover).
Component deprecation/retirement states (`ACTIVE → DEPRECATED → REPLACED →
ARCHIVED → RETIRED`) with migration semantics are defined in
[maintenance.md §13](maintenance.md#13-deprecation--retirement).

## Candidate Isolation

In the evolution loop, candidates are evaluated in isolation and never touch
stable state directly. A candidate carries a `reversible_save_id` so it can be
rolled back. Promotion is gated. See [evolution.md](evolution.md).

## Real Limitations

- **Selective recovery is at file granularity.** You cannot restore a single
  line or hunk through Route.
- **Archive storage grows with each save.** There is no automatic
  retention/GC.
- **Cross-platform is uneven.** Windows is the primary development platform;
  other platforms are experimental.
- **Recovery restores what was saved.** Files changed after the last save are
  not recoverable from the archive.
- **`route archive delete` / `delete-project` require confirmation.** These are
  destructive by design.

## Safety Invariants

- Restore and repair default to **preview first**.
- The **Original** save is immutable.
- Auto-save happens before destructive operations.
- `.route/` is protected by the Route Guard.
- Local-first: no cloud upload.