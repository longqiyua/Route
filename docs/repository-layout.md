# Route repository layout

`C:\Users\longq\Desktop\Route` is the Route Git repository root and is bound
to `https://github.com/longqiyua/Route`.

## Source and maintained content

| Path | Ownership |
|---|---|
| `tool/route/` | Active Rust workspace and Route core implementation |
| `docs/` | Route architecture, protocols, operations, and conformance records |
| `tests/` | Repository-level tests and fixtures |
| `scripts/` | Maintained development and verification scripts |
| `examples/` | Route examples |
| `archive/` | Retained historical Route implementations; not the active core |
| `packages/`, `tools/` | Maintained supporting packages and development tools |
| `ROUTE.md` | Canonical Route specification and operational truth |

## Local-only content

| Path | Rule |
|---|---|
| `Release/` | Distribution artifacts, release staging, and legacy local backups; ignored by Git |
| `.route/`, `.route-basic/` | Local Route runtime/project state; ignored by Git |
| `target/`, `**/target/` | Build output; ignored by Git |

Release artifacts must be assembled under `Release/`, never committed to the
repository. A release package contains only the intended distributable output;
it must not vendor the Route source repository, Yuich, local state, or build
caches.

The repository root is the project boundary, while `tool/route/` is the Rust
workspace boundary. Commands that invoke Cargo should run from `tool/route/`
or pass `--manifest-path tool/route/Cargo.toml` explicitly.

Yuich is an independent repository at `C:\Users\longq\Desktop\Yuich` and is
not part of the Route repository tree.
