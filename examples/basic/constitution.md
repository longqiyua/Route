<!-- route-constitution:version=1 -->
<!-- route-constitution:created_at=1786600000000 -->
# Constitution

Stable development principles for this project.
AI assistants MUST read this document before making changes.

## Data safety

- Never delete user data without explicit confirmation.
- Rollback and undo are preferred over destructive rewrite.
- .route-basic/ and .route/ are internal data — treat as append-only.

## User control

- Conflicts between AI suggestions and existing logic go to the user.
- No silent behaviour change. Every setting change is explicit.

## Reversibility

- Every destructive change must have a corresponding restore path.
- Prefer checkpointing the project state before a large rewrite.