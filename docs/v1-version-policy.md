# Route AI v1 version boundary

The Human-selected **public product version is `1.0.0`**. This source is still
an unpublished release candidate: version selection does not authorize a
`main` merge, tag, package publication, or GitHub Release.

| Surface | Classification | Version decision |
| --- | --- | --- |
| Windows Route CLI and Cargo workspace | SHIPPED_V1 candidate | `1.0.0`; `route.exe --version` must agree. |
| `tool/route/TOOL.json` | PRODUCT_DISCOVERY_METADATA, not in CLI ZIP | `1.0.0`. |
| JSON RPC identifier | PROTOCOL | Remains `route/1`; not a product version. |
| `route-pyo3` / `packages/route-py` | EXPERIMENTAL, NOT_SHIPPED | Existing `1.0.0-beta` retained; do not imply v1 Python support. |
| Root `packages/cli`, `packages/core` | LEGACY, NOT_SHIPPED | Existing `0.1.0` retained; not a Windows CLI package dependency. |
| Archived Tauri/desktop metadata | LEGACY, NOT_SHIPPED | Historical beta metadata retained. |
| V0.6 Beta / V0.8 documentation | HISTORICAL | Preserve milestone history; do not present as current product version. |

The v1 release candidate is the Windows x64 Rust CLI. The release archive must
not include the experimental Python package, legacy npm packages, archived
desktop code, or local runtime/reference datasets. A final reconciled-source
build, package audit, and Human approval are still required before publication.
