# Third-Party Notices

Route uses the following open-source components. We are grateful to their authors and contributors.

## Runtime Dependencies

| Component | License | Purpose | Homepage |
|-----------|---------|---------|----------|
| [fast-glob](https://github.com/mrmlnc/fast-glob) | MIT | Fast file globbing for project scans | https://github.com/mrmlnc/fast-glob |
| [commander](https://github.com/tj/commander.js) | MIT | CLI argument parsing | https://github.com/tj/commander.js |
| [Tauri](https://github.com/tauri-apps/tauri) | MIT / Apache-2.0 | Desktop app shell | https://tauri.app |
| [React](https://github.com/facebook/react) | MIT | Desktop UI | https://react.dev |
| [Vite](https://github.com/vitejs/vite) | MIT | Frontend build tool | https://vite.dev |

## Design Inspiration

Route's mental model is inspired by [Git](https://git-scm.com/) (GPL-2.0), but Route is an independent implementation with a conversation-first version unit. Route does **not** embed or fork Git.

## Git Mode (Planned — Mode 2)

The planned Git integration mode will invoke the user's system `git` binary. Users will be shown explicit risk warnings before enabling that mode. Route itself remains MIT-licensed; Git is subject to its own license when invoked externally.

## Attribution Requirement

When distributing Route, include this file alongside the MIT `LICENSE`.
