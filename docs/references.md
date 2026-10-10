# References

The **Reference** registry describes external resources that can be injected
into agent context **on demand**. References are not inline copies of content;
they are resource descriptions that Route can load when needed.

A Reference is **information**, not authority. It may inform development but
does not automatically become a Constraint, Evidence, fact, or statement of
current reality. See the complete ownership and cooperation contract in
[Reference, Constraint, and Cooperation](reference-cooperation.md).

## Reference Types

| Type | Meaning |
|------|---------|
| `cli` | A command-line tool (e.g. `cargo`, `git`) |
| `document` | A documentation file |
| `repo` | An external repository |
| `executable` | A binary / executable capability |
| `workflow` | A reusable, high-level task workflow |
| `skill` | A skill definition |
| (other) | Classified by the source when imported |

## Fields

A reference entry carries metadata:

- `name`, `source`, `type`
- `enabled` — whether it is active
- `capabilities`, `constraints` — legacy descriptive strings, not authority
- `tags[]` — descriptive tags
- `project_scope?`, `trust?`, `entrypoint?` — optional context

The **native source format is preserved** — references are not forced into a
Route-private format.

The legacy descriptive `constraints` field records limitations reported by a
Reference. It does not create canonical authority. Authoritative Constraints
remain owned by their existing project sources and are exposed through a
read-only projection; attaching a Reference only documents that authority.

## CLI

```bash
route reference list                 # list entries (optionally by type)
route reference inspect <id>         # details of one entry
route reference add ...              # register an entry
route reference remove <id>          # remove an entry
route reference import <source>      # import from file / repo / CLI help
route reference enable <id>          # enable an entry
route reference disable <id>         # disable an entry
route reference review               # run the curator: analyze the registry
route reference apply <proposal_id>  # apply a curator proposal
```

## Import Sources (`route reference import`)

Supported sources in v0:

- **Markdown / text file** — path ending in `.md` / `.mdx` / `.txt`
- **Local Route skill file** — a `.rs` / `.toml` file, or a directory
  containing `SKILL.md` / `SKILL.toml`
- **Git repository URL** — `https://...git` or `git@...`
- **CLI `--help` output** — a saved help text file, or `cli:<cmd>` to invoke
  `<cmd> --help` and parse its output

## Compatibility Levels

References are bound by **ID reference only** (no content copying). Bindings
are supported between workflows → skills/references, profiles →
workflows/references, and tasks → profiles/workflows. Compatibility levels:

| Level | Meaning |
|-------|---------|
| **L1** | Referenceable (can be listed/shown) |
| **L2** | AI-understandable (can be injected into context) |
| **L3** | Executable (host-dependent) |

## On-Demand Context

References are injected on demand. Task context selects the references
relevant to the task, so the agent is not flooded with the whole registry.
This keeps context lean while still providing the right resources.

Logical availability never requires copying a complete external resource into
Route state or a prompt. Large resources and directories remain opaque
locators with bounded metadata until exact content is explicitly requested.
Registration does not recursively ingest, auto-stage, or take ownership of the
referenced content. Missing and unknown remain valid information states.

## Trust & Safety

- User-created references cannot bypass the proposal process.
- Removing a reference requires explicit confirmation (agents cannot delete
  references directly).
- References preserve their native source format.
- References and their metadata never become Evidence or authoritative
  Constraints automatically.
- Reference state accepts bounded operational metadata, never raw
  chain-of-thought.
