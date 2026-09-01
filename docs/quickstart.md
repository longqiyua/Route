# Quickstart

This is the shortest real flow, from an empty project directory, using only
commands that exist in V0.6 Beta. It assumes:

- You have built `route` and added it to your PATH (see
  [README.md](../README.md)).
- You are on a machine with a supported host (Claude Code, Codex, or a
  DeepSeek-compatible harness) for the `--target` step.

## 1. Start from an empty directory

```bash
mkdir /tmp/route-demo && cd /tmp/route-demo
```

## 2. Initialize

```bash
route init
```

This creates `.route/` (config, profiles) and the immutable **Original**
archive save in `Documents/Route/projects/<project-id>/`.

Check it:

```bash
route status
```

## 3. Start a task session

```bash
route task start "add a hello function" --target generic
```

This creates a session, auto-saves (PRE_CHANGE), builds task-scoped context,
writes `.route/generated/context.md`, and returns a `session_id`.

> If you have no harness, use `--target generic` — it only writes a portable
> Markdown context file and does not require a specific host.

## 4. Do the work

Create a file:

```bash
echo 'fn hello() { println!("hi"); }' > src/lib.rs
```

## 5. Verify

```bash
route task verify <session-id>
```

Verification checks the Protocol verification policy. Only system evidence
(trusted test pass, commit) is accepted. If you have no test/commit evidence,
the session will be `INCOMPLETE` rather than `VERIFIED` — that is expected.

## 6. End the session

```bash
route task end <session-id> --result success
```

On success this auto-verifies, auto-saves (VERIFIED), records a trajectory,
and generates learning proposals (not applied).

## 7. Save to the external archive

```bash
route archive save "first milestone"
route archive list
```

## (Optional) Recover a deleted project

```bash
# After deleting the project directory:
route archive recover <project-id> --save <id> --to /tmp/route-restored
```

The archive is independent of the project directory, so it survives deletion.

## What just happened

- You created a persistent Route project (`.route/`).
- You ran one task session with a recorded evidence chain.
- You saved recoverable state to the external archive.

That is the core Route loop. For deeper concepts, see
[concepts.md](concepts.md); for the full command reference, see
[cli.md](cli.md).