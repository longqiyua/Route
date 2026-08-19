# Sandbox — Route Development History

> **SANDBOX = HOW ROUTE WAS DEVELOPED.**

This branch is the **public development record** for Route. It is **not** product state,
**not** a raw debug dump, and **not** a runtime dependency.

```
MAIN    = WHAT ROUTE IS.
FRUIT   = WHAT ROUTE MAY BECOME.
SANDBOX = HOW ROUTE WAS DEVELOPED.
```

- **Route remembers project work at runtime** (`.route/` / `.route-basic/` in the target project).
- **Sandbox remembers how Route itself was developed, for humans.**

## What lives here

- `development/YYYY-MM-DD-<short-topic>.md` — dated development records.
- Records cover goals, triggers, evidence, tests, failures, rollbacks, outcomes, and what Route learned.
- **Failed experiments and rollbacks are preserved on purpose.** "Why is it designed this way?
  What was tried before? What failed?" is exactly the value of an open-source development history.

## What does NOT live here

- Hidden chain-of-thought / raw AI reasoning.
- Private user data, credentials, tokens, personal paths.
- Product source truth (that is `main`).
- Runtime state.

## Branch policy

- Experimental code → `fruit`.
- Development record → `sandbox`.
- Public confirmed Route → `main`.
- Promotion to `main` happens only with verified evidence, never by an AI Worker self-deciding.

## Records

See [development/](development/) for the dated history.