<!-- route-protocol:version=1 -->
<!-- route-protocol:revision=1 -->
<!-- route-protocol:updated_at=1786600000000 -->
# Protocol

Execution playbook for AI assistants working on this project.

## Workflow

1. Read the Constitution, this Protocol, and the latest Reference registry.
2. Understand the task. If the scope is ambiguous, ask the user before touching files.
3. Create a **snapshot** (route commit -m 'pre-<task>') before large changes.
4. Make the change. Keep commits small and well-described.
5. Run tests via `route task exec SESSION -- cargo test` (or `cargo test` directly).
6. Verify with `route task verify SESSION`.
7. If something breaks mid-flight, use `route rollback <snapshot_id>` rather than hand-rolling back.

## Review gates

- Schema / API changes: always flag to user for review.
- Destructive file moves or deletions: snapshot first.
- Anything marked in Constitution as user-only: escalate.