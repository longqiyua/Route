# Basic Example

This directory shows the minimal, real on-disk formats Route reads and writes.
Every file here is a literal match for what the current V0.6 Beta implementation
produces. These are **not** "pretty fake examples" — Route can parse them.

## Files

| File | What it shows | Format |
|------|---------------|--------|
| `constitution.md` | Immutable development principles | HTML-comment envelope + Markdown body |
| `protocol.md` | Versioned execution playbook | HTML-comment envelope (version/revision/updated_at) + Markdown body |
| `reference-entry.json` | A single Reference registry entry | `ReferenceEntry` JSON (`route-basic` `constitutive.rs`) |

## How to use

Initialize a project and let Route write real files, then compare:

```bash
route init
# .route/constitution.md and .route/protocol.md are created with this shape.
```

To add the reference entry from this example:

```bash
route reference add --source ./examples/basic/reference-entry.json
```

To see the Effective Context that includes the Constitution, Protocol, and
enabled References:

```bash
route context --metadata
route apply --target generic
```

## Format notes

- The `type` field is one of: `document`, `repo`, `skill`, `cli`, `executable`,
  `mcp`, `api`, `workflow`, `prompt`, `unknown`, `experience`.
- `origin` is one of `UserCreated` (default, protected against automated
  edits), `Imported`, or `Generated`. `Experience` entries are always
  `Generated` by `route learn apply` — never hand-written.
- Disabled entries (`enabled: false`) are excluded from the Effective Context.
- `unknown`-typed entries are never exported until upgraded by the user.