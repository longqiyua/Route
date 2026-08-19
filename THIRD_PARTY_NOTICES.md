# Third-Party Notices

Route uses the following open-source components. We are grateful to their authors and contributors.

## Runtime Dependencies (Rust workspace)

| Component | License | Purpose |
|-----------|---------|---------|
| [anyhow](https://github.com/dtolnay/anyhow) | MIT / Apache-2.0 | Error handling |
| [thiserror](https://github.com/dtolnay/thiserror) | MIT / Apache-2.0 | Error types |
| [serde / serde_json](https://serde.rs/) | Apache-2.0 / MIT | Serialization |
| [rusqlite](https://github.com/rusqlite/rusqlite) | MIT | SQLite bindings |
| [sha2](https://github.com/RustCrypto/hashes) | Apache-2.0 / MIT | SHA-256 hashing |
| [hex](https://github.com/KokaKiwi/rust-hex) | MIT / Apache-2.0 | Hex encoding |
| [ulid](https://github.com/dylanhart/ulid-rs) | MIT / Apache-2.0 | ULID identifiers |
| [chrono](https://github.com/chronotope/chrono) | Apache-2.0 / MIT | Date/time |
| [ignore](https://github.com/BurntSushi/ripgrep) | MIT | .gitignore-aware scanning |
| [zip](https://github.com/zip-rs/zip) | MIT / Apache-2.0 | ZIP archives |
| [notify](https://github.com/notify-rs/notify) | CC0-1.0 | File watching |
| [reqwest](https://github.com/seanmonstar/reqwest) | Apache-2.0 / MIT | HTTP client |
| [axum / tokio / tower](https://github.com/tokio-rs) | MIT | HTTP server / async runtime |
| [base64](https://github.com/marshallpierce/rust-base64) | MIT / Apache-2.0 | Base64 encoding |
| [percent-encoding](https://github.com/servo/rust-url) | MIT / Apache-2.0 | Percent encoding |
| [quick-xml](https://github.com/tafia/quick-xml) | MIT | XML parsing |
| [hmac](https://github.com/RustCrypto/MACs) | MIT / Apache-2.0 | HMAC |
| [tracing / tracing-subscriber](https://github.com/tokio-rs/tracing) | MIT | Structured logging |
| [clap](https://github.com/clap-rs/clap) | MIT / Apache-2.0 | CLI parsing |
| [rustyline](https://github.com/kkawakam/rustyline) | MIT | REPL line editing |
| [owo-colors](https://github.com/jam1garner/owo-colors) | MIT | Terminal colors |
| [comfy-table](https://github.com/Nukesor/comfy-table) | MIT | Terminal tables |
| [pyo3](https://github.com/PyO3/pyo3) | MIT / Apache-2.0 | Python bindings |
| [once_cell](https://github.com/matklad/once_cell) | MIT / Apache-2.0 | Lazy statics |
| [rstest](https://github.com/la10736/rstest) | MIT / Apache-2.0 | Test fixtures |
| [tempfile](https://github.com/Stebalien/tempfile) | MIT / Apache-2.0 | Temp files |

## Design Inspiration

Route's mental model is inspired by [Git](https://git-scm.com/) (GPL-2.0), but Route is an independent
implementation with a session/work-first version unit. Route does **not** embed or fork Git. Route's
`route git` subcommand drives the user's own system `git` binary when explicitly requested.

## Attribution Requirement

When distributing Route, include this file alongside the AGPL-3.0 `LICENSE`.