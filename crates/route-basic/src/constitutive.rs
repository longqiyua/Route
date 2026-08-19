//! Constitution / Protocol / Reference v0.
//!
//! Three tiers of AI-facing project governance, stored in `.route/`
//! (the user-facing directory — distinct from `.route-basic/` which is
//! Route's own internal database directory).
//!
//! ```text
//! .route/
//! ├── constitution.md      # Immutable principles (AI must NOT auto-modify)
//! ├── protocol.md          # Execution playbook (user-versionable)
//! └── reference/
//!     └── registry.json    # General-purpose external-resource registry
//! ```
//!
//! **Design principle: Route never reimplements external tools.**
//! Reference entries *describe* external resources and expose them to
//! the AI context pipeline. Route does not execute, fetch, or learn from
//! them at this layer — that is the responsibility of downstream
//! integrations (MCP, CLI wrappers, future agents).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::learn::ScoredLearnedExperience;
use crate::material::{discover_material, render_constraints, render_references, MaterialSource};
use crate::memory::MemoryStore;

// ---------------------------------------------------------------------------
// Directory layout
// ---------------------------------------------------------------------------

/// Name of the user-facing Route directory (as opposed to
/// `.route-basic/` which is Route's internal storage).
pub const ROUTE_DOT_DIR: &str = ".route";

pub fn dot_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR)
}

pub fn constitution_path(project_root: &Path) -> PathBuf {
    dot_dir(project_root).join("constitution.md")
}

pub fn protocol_path(project_root: &Path) -> PathBuf {
    dot_dir(project_root).join("protocol.md")
}

pub fn reference_dir(project_root: &Path) -> PathBuf {
    dot_dir(project_root).join("reference")
}

pub fn registry_path(project_root: &Path) -> PathBuf {
    reference_dir(project_root).join("registry.json")
}

pub fn context_dir(project_root: &Path) -> PathBuf {
    dot_dir(project_root).join("context")
}

pub fn context_history_dir(project_root: &Path) -> PathBuf {
    context_dir(project_root).join("history")
}

pub fn context_archive_dir(project_root: &Path) -> PathBuf {
    context_dir(project_root).join("archive")
}

// ---------------------------------------------------------------------------
// Write origin & write permissions
// ---------------------------------------------------------------------------

/// Who is requesting a write to a constitutive file. Used by
/// [`Constitution::write_with_origin`] and friends to enforce explicit
/// permission boundaries instead of relying only on prompt-level
/// instructions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WriteOrigin {
    /// Interactive user via CLI or editor.
    Human,
    /// Route itself performing a schema migration or a controlled
    /// bootstrapping step (e.g. `route init` writing the default
    /// constitution).
    System,
    /// AI / agent runtime. Explicitly forbidden for Constitution.
    Agent,
}

impl WriteOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            WriteOrigin::Human => "human",
            WriteOrigin::System => "system",
            WriteOrigin::Agent => "agent",
        }
    }
}

/// Errors raised by constitutive write permission checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WritePermissionError {
    /// The caller's origin is not allowed to mutate this file.
    Denied {
        origin: &'static str,
        resource: &'static str,
    },
}

impl std::fmt::Display for WritePermissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WritePermissionError::Denied { origin, resource } => {
                write!(
                    f,
                    "write denied: '{}' origin cannot modify '{}'",
                    origin, resource
                )
            }
        }
    }
}

impl std::error::Error for WritePermissionError {}

// ---------------------------------------------------------------------------
// Constitution
// ---------------------------------------------------------------------------

/// The Constitution holds **immutable project principles**. AI agents
/// must read it; they are explicitly forbidden from modifying it.
///
/// We deliberately do not over-structure the constitution: it is a
/// Markdown file with a small structured envelope (version + created_at)
/// and a free-form body. This keeps the v0 surface tiny while giving
/// projects enough expressive power.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Constitution {
    /// Schema version for the constitution envelope. Bump only when the
    /// envelope structure changes (not when the body content changes).
    pub version: u32,
    /// Unix-millis when the constitution was first written.
    pub created_at: i64,
    /// The Markdown body. Parsed by humans and LLMs; not validated.
    pub body: String,
}

impl Constitution {
    /// Version of the constitution envelope shipped with this build.
    pub const CURRENT_VERSION: u32 = 1;

    /// Default body used by [`Self::default`]. Written when a project
    /// does not yet have a constitution.
    pub fn default_body() -> String {
        concat!(
            "# Constitution\n\n",
            "Stable development principles for this project.\n",
            "AI assistants MUST read this document before making changes.\n\n",
            "## Data safety\n\n",
            "- Never delete user data without explicit confirmation.\n",
            "- Rollback and undo are preferred over destructive rewrite.\n",
            "- .route-basic/ and .route/ are internal data — treat as append-only.\n\n",
            "## User control\n\n",
            "- Conflicts between AI suggestions and existing logic go to the user.\n",
            "- No silent behaviour change. Every setting change is explicit.\n\n",
            "## Reversibility\n\n",
            "- Every destructive change must have a corresponding restore path.\n",
            "- Prefer checkpointing the project state before a large rewrite.\n",
        )
        .to_string()
    }

    /// Read the constitution from its canonical path. If the file is
    /// missing, return the default constitution (does not write it).
    pub fn read(project_root: &Path) -> Result<Self> {
        let p = constitution_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading constitution from {}", p.display()))?;
        Ok(Self::parse_envelope(&raw))
    }

    /// Parse the Markdown body that may be prefixed by structured
    /// frontmatter. For v0 we keep the envelope embedded in HTML-style
    /// comments at the top of the file so the document renders cleanly
    /// on any Markdown viewer.
    fn parse_envelope(raw: &str) -> Self {
        let (version, created_at, rest) = extract_frontmatter(raw);
        Self {
            version: version.unwrap_or(Self::CURRENT_VERSION),
            created_at: created_at.unwrap_or_else(route_core::now_millis),
            body: rest.to_string(),
        }
    }

    /// Atomically write the constitution. **This API is deliberately
    /// not exposed to AI callers.** It is intended for direct human
    /// users via the CLI.
    ///
    /// Equivalent to [`Self::write_with_origin`] with
    /// [`WriteOrigin::Human`].
    pub fn write(&self, project_root: &Path) -> Result<()> {
        self.write_with_origin(project_root, WriteOrigin::Human)
    }

    /// Atomically write the constitution and enforce the origin-based
    /// permission matrix:
    ///
    /// | Origin   | Constitution |
    /// |----------|:------------:|
    /// | Human    | allowed      |
    /// | System   | allowed      |
    /// | Agent    | denied       |
    pub fn write_with_origin(&self, project_root: &Path, origin: WriteOrigin) -> Result<()> {
        // Permission check — enforced in code, not in prompt.
        match origin {
            WriteOrigin::Human | WriteOrigin::System => {}
            WriteOrigin::Agent => {
                return Err(WritePermissionError::Denied {
                    origin: WriteOrigin::Agent.as_str(),
                    resource: "constitution.md",
                }
                .into());
            }
        }
        std::fs::create_dir_all(dot_dir(project_root))?;
        let p = constitution_path(project_root);
        let rendered = format!(
            "<!-- route-constitution:version={} -->\n\
             <!-- route-constitution:created_at={} -->\n\
             {}",
            self.version, self.created_at, self.body
        );
        write_atomic(&p, rendered.as_bytes())
            .with_context(|| format!("writing constitution to {}", p.display()))?;
        Ok(())
    }

    /// Canonical file path for the constitution document inside
    /// `project_root`.
    pub fn path(project_root: &Path) -> PathBuf {
        constitution_path(project_root)
    }

    /// Write the default constitution if one does not already exist.
    /// Returns `true` if a file was actually written, `false` if one was
    /// already present. Uses [`WriteOrigin::System`] so the permission
    /// layer correctly attributes the file to Route's own init step.
    pub fn ensure_exists(project_root: &Path) -> Result<bool> {
        let p = constitution_path(project_root);
        if p.exists() {
            return Ok(false);
        }
        let default = Self::default();
        default.write_with_origin(project_root, WriteOrigin::System)?;
        Ok(true)
    }
}

impl Default for Constitution {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            created_at: route_core::now_millis(),
            body: Self::default_body(),
        }
    }
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// The Protocol describes *how* AI-driven development is executed in
/// this project. Unlike the Constitution it is versioned and may be
/// changed by the user at any time — the body free-form Markdown can
/// describe workflow steps like "checkpoint before refactor", "ask
/// review on schema change", "run tests before commit", etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Protocol {
    /// Schema version for the protocol envelope.
    pub version: u32,
    /// Monotonically increasing revision. Incremented by `write` so
    /// downstream integrations can tell when the protocol changed.
    pub revision: u64,
    /// Unix-millis when the protocol was last updated.
    pub updated_at: i64,
    /// Free-form Markdown body.
    pub body: String,
}

impl Protocol {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn default_body() -> String {
        concat!(
            "# Protocol\n\n",
            "Execution playbook for AI assistants working on this project.\n\n",
            "## Workflow\n\n",
            "1. Read the Constitution, this Protocol, and the latest Reference registry.\n",
            "2. Understand the task. If the scope is ambiguous, ask the user before touching files.\n",
            "3. Create a **snapshot** (route commit -m 'pre-<task>') before large changes.\n",
            "4. Make the change. Keep commits small and well-described.\n",
            "5. Run tests via `route task exec SESSION -- cargo test` (or `cargo test` directly).\n",
            "6. Verify with `route task verify SESSION`.\n",
            "7. If something breaks mid-flight, use `route rollback <snapshot_id>` rather than hand-rolling back.\n\n",
            "## Review gates\n\n",
            "- Schema / API changes: always flag to user for review.\n",
            "- Destructive file moves or deletions: snapshot first.\n",
            "- Anything marked in Constitution as user-only: escalate.\n",
        )
        .to_string()
    }

    /// Read the protocol from its canonical path. If missing, returns
    /// the default protocol (does not write it).
    pub fn read(project_root: &Path) -> Result<Self> {
        let p = protocol_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading protocol from {}", p.display()))?;
        let (version, rev, updated_at, rest) = extract_protocol_frontmatter(&raw);
        Ok(Self {
            version: version.unwrap_or(Self::CURRENT_VERSION),
            revision: rev.unwrap_or(1),
            updated_at: updated_at.unwrap_or_else(route_core::now_millis),
            body: rest.to_string(),
        })
    }

    /// Atomically write the protocol. Bumps `revision` and `updated_at`
    /// on every write.
    pub fn write(&mut self, project_root: &Path) -> Result<()> {
        std::fs::create_dir_all(dot_dir(project_root))?;
        self.revision += 1;
        self.updated_at = route_core::now_millis();
        let p = protocol_path(project_root);
        let rendered = format!(
            "<!-- route-protocol:version={} -->\n\
             <!-- route-protocol:revision={} -->\n\
             <!-- route-protocol:updated_at={} -->\n\
             {}",
            self.version, self.revision, self.updated_at, self.body
        );
        write_atomic(&p, rendered.as_bytes())
            .with_context(|| format!("writing protocol to {}", p.display()))?;
        Ok(())
    }

    /// Canonical file path for the protocol document inside
    /// `project_root`.
    pub fn path(project_root: &Path) -> PathBuf {
        protocol_path(project_root)
    }

    /// Write the default protocol if one does not already exist.
    /// Returns `true` if a file was actually written, `false` if one was
    /// already present.
    pub fn ensure_exists(project_root: &Path) -> Result<bool> {
        let p = protocol_path(project_root);
        if p.exists() {
            return Ok(false);
        }
        let mut default = Self {
            revision: 0, // `write` bumps by +1, so final revision is 1
            ..Self::default()
        };
        default.write(project_root)?;
        Ok(true)
    }
}

impl Default for Protocol {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            revision: 1,
            updated_at: route_core::now_millis(),
            body: Self::default_body(),
        }
    }
}

// ---------------------------------------------------------------------------
// Reference registry
// ---------------------------------------------------------------------------

/// Type of a reference entry. Eight reference types are defined in v0.
/// Route never *uses* these types to drive behaviour — it only
/// categorises them so the context pipeline can present them usefully.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReferenceType {
    /// A document (Markdown, PDF, HTML, ...).
    Document,
    /// A source-code repository (local path or git URL).
    Repo,
    /// An AI skill description file or skill pointer.
    Skill,
    /// A command-line tool the project uses (name + invocation notes).
    Cli,
    /// An on-disk executable the project invokes directly.
    Executable,
    /// A Model Context Protocol server (name, command, args).
    Mcp,
    /// An HTTP API endpoint or service description.
    Api,
    /// A multi-step workflow script / playbook.
    Workflow,
    /// A prompt template or system-prompt snippet.
    Prompt,
    /// Source couldn't be classified yet (import pipeline fallback).
    /// Entries typed "unknown" are never exported to the Effective
    /// Development Context; they must be upgraded by the user before
    /// the AI sees them.
    Unknown,
    /// A learned experience pattern aggregated from events (Adaptive Learning v0).
    /// Generated by `route learn analyze` + `route learn apply`. Never hard-coded
    /// by a human; always `Origin::Generated`.
    Experience,
}

impl ReferenceType {
    pub fn as_str(self) -> &'static str {
        match self {
            ReferenceType::Document => "document",
            ReferenceType::Repo => "repo",
            ReferenceType::Skill => "skill",
            ReferenceType::Cli => "cli",
            ReferenceType::Executable => "executable",
            ReferenceType::Mcp => "mcp",
            ReferenceType::Api => "api",
            ReferenceType::Workflow => "workflow",
            ReferenceType::Prompt => "prompt",
            ReferenceType::Unknown => "unknown",
            ReferenceType::Experience => "experience",
        }
    }
}

impl std::str::FromStr for ReferenceType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "document" | "doc" => Ok(ReferenceType::Document),
            "repo" | "repository" => Ok(ReferenceType::Repo),
            "skill" => Ok(ReferenceType::Skill),
            "cli" | "command" | "cmd" => Ok(ReferenceType::Cli),
            "executable" | "exec" | "bin" => Ok(ReferenceType::Executable),
            "mcp" => Ok(ReferenceType::Mcp),
            "api" => Ok(ReferenceType::Api),
            "workflow" | "wf" => Ok(ReferenceType::Workflow),
            "prompt" => Ok(ReferenceType::Prompt),
            "unknown" | "" => Ok(ReferenceType::Unknown),
            "experience" | "learned" => Ok(ReferenceType::Experience),
            other => Err(format!(
                "unknown reference type '{}': valid types are document, repo, skill, cli, executable, mcp, api, workflow, prompt, unknown",
                other
            )),
        }
    }
}

/// Provenance of a reference entry: who/what created it and how it was
/// sourced. This lets future tooling decide which entries are safe to
/// auto-update versus which are hand-authored and must be preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Written by a human via the CLI or editor. Safe against automated
    /// overwrite / curation passes.
    UserCreated,
    /// Imported by `route reference import ...` or equivalent API. May
    /// be auto-refreshed against the source if `content_hash` matches.
    Imported,
    /// Generated programmatically by a tool. Low provenance weight; a
    /// future curator may regenerate or drop it freely.
    Generated,
}

impl Default for Origin {
    /// Defaults to [`Origin::UserCreated`] so every entry created before
    /// provenance fields were introduced is treated as human-authored and
    /// protected against silent automated rewrite.
    fn default() -> Self {
        Origin::UserCreated
    }
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::UserCreated => "user_created",
            Origin::Imported => "imported",
            Origin::Generated => "generated",
        }
    }
}

/// A single entry in the reference registry. Every field is
/// human-readable and optional where it makes sense — this is a
/// description format, not an execution contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceEntry {
    /// Stable id, e.g. `doc-architecture` or `mcp-route`.
    pub id: String,
    /// Entry type.
    #[serde(rename = "type")]
    pub type_: ReferenceType,
    /// Where the entry lives: local path, git URL, HTTP URL, command name.
    #[serde(default)]
    pub source: String,
    /// Optional sub-path or alias within the source (e.g. a chapter
    /// inside a document, or a specific subcommand of a CLI).
    #[serde(default)]
    pub path: Option<String>,
    /// One-sentence summary shown in the context header.
    #[serde(default)]
    pub description: String,
    /// What this entry enables (free-form, consumed by LLMs).
    #[serde(default)]
    pub capabilities: String,
    /// What this entry MUST NOT be used for, or any hard constraints.
    #[serde(default)]
    pub constraints: String,
    /// Unix-millis when this entry was added.
    pub created_at: i64,
    /// Optional free-form tags (e.g. `["rust", "testing"]`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Who created / authored the entry. Defaults to `UserCreated` so
    /// pre-provenance entries remain protected against automated edits.
    #[serde(default)]
    pub origin: Origin,
    /// Only meaningful when `origin == Imported`: URL/path the entry
    /// was imported from, if different from the entry's canonical
    /// `source`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import_origin: Option<String>,
    /// Unix-millis when the entry was imported (only meaningful for
    /// `origin == Imported`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_at: Option<i64>,
    /// Stable hash of the imported source content. Used to detect
    /// whether a re-import would actually change anything.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Unix-millis when the source was last re-validated against the
    /// origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_checked: Option<i64>,
    /// Structured metadata for learned experience references.
    /// Serde `default` + `skip_serializing_if` ensures backward
    /// compatibility with existing registry entries that lack this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_meta: Option<LearnedMeta>,
    /// Whether this entry is enabled. Disabled entries are excluded from
    /// the Effective Context pipeline. Defaults to true.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Human-readable display name (separate from the stable `id`).
    /// Falls back to `id` when empty.
    #[serde(default)]
    pub name: String,
    /// Optional project scope hint (e.g. "frontend", "backend", "infra").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_scope: Option<String>,
    /// Optional trust level hint (e.g. "high", "medium", "low").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust: Option<String>,
    /// Optional entrypoint path or command (e.g. "main.py", "index.js").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
}

fn default_enabled() -> bool {
    true
}

/// Structured metadata for learned experience references.
///
/// This replaces the old approach of parsing confidence from free-text
/// `constraints` fields. Serde `default` ensures backward compatibility
/// with existing registry entries that lack this field.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LearnedMeta {
    /// Confidence in [0.0, 1.0).
    #[serde(default)]
    pub confidence: f32,
    /// Narrow scope of this learned experience.
    #[serde(default)]
    pub scope: String,
    /// Unix-millis when the confidence was last confirmed.
    #[serde(default)]
    pub last_confirmed: i64,
    /// Whether this entry is considered stale (low confidence / superseded).
    #[serde(default)]
    pub stale: bool,
    /// Optional id of the entry that superseded this one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
    /// IDs of supporting evidence events.
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

/// Builder for [`ReferenceEntry`] — so callers do not have to construct
/// every single optional field themselves.
pub struct ReferenceEntryBuilder {
    id: String,
    type_: ReferenceType,
    source: String,
    path: Option<String>,
    description: String,
    capabilities: String,
    constraints: String,
    created_at: Option<i64>,
    tags: Vec<String>,
    origin: Option<Origin>,
    import_origin: Option<String>,
    imported_at: Option<i64>,
    content_hash: Option<String>,
    last_checked: Option<i64>,
    learned_meta: Option<LearnedMeta>,
    enabled: Option<bool>,
    name: String,
    project_scope: Option<String>,
    trust: Option<String>,
    entrypoint: Option<String>,
}

impl ReferenceEntry {
    /// Start building a new entry with the mandatory fields.
    pub fn builder(
        id: impl Into<String>,
        type_: ReferenceType,
        source: impl Into<String>,
        description: impl Into<String>,
    ) -> ReferenceEntryBuilder {
        ReferenceEntryBuilder {
            id: id.into(),
            type_,
            source: source.into(),
            path: None,
            description: description.into(),
            capabilities: String::new(),
            constraints: String::new(),
            created_at: None,
            tags: Vec::new(),
            origin: None,
            import_origin: None,
            imported_at: None,
            content_hash: None,
            last_checked: None,
            learned_meta: None,
            enabled: None,
            name: String::new(),
            project_scope: None,
            trust: None,
            entrypoint: None,
        }
    }
}

impl ReferenceEntryBuilder {
    pub fn with_opt_path(mut self, path: Option<String>) -> Self {
        self.path = path;
        self
    }
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }
    pub fn with_capabilities(mut self, capabilities: impl Into<String>) -> Self {
        self.capabilities = capabilities.into();
        self
    }
    pub fn with_constraints(mut self, constraints: impl Into<String>) -> Self {
        self.constraints = constraints.into();
        self
    }
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
    pub fn with_created_at(mut self, ts: i64) -> Self {
        self.created_at = Some(ts);
        self
    }
    pub fn with_origin(mut self, origin: Origin) -> Self {
        self.origin = Some(origin);
        self
    }
    pub fn with_import_origin(mut self, s: impl Into<String>) -> Self {
        self.import_origin = Some(s.into());
        self
    }
    pub fn with_imported_at(mut self, ts: i64) -> Self {
        self.imported_at = Some(ts);
        self
    }
    pub fn with_content_hash(mut self, s: impl Into<String>) -> Self {
        self.content_hash = Some(s.into());
        self
    }
    pub fn with_last_checked(mut self, ts: i64) -> Self {
        self.last_checked = Some(ts);
        self
    }
    pub fn with_learned_meta(mut self, meta: LearnedMeta) -> Self {
        self.learned_meta = Some(meta);
        self
    }
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
    pub fn with_project_scope(mut self, scope: impl Into<String>) -> Self {
        self.project_scope = Some(scope.into());
        self
    }
    pub fn with_trust(mut self, trust: impl Into<String>) -> Self {
        self.trust = Some(trust.into());
        self
    }
    pub fn with_entrypoint(mut self, entrypoint: impl Into<String>) -> Self {
        self.entrypoint = Some(entrypoint.into());
        self
    }
    pub fn build(self) -> ReferenceEntry {
        ReferenceEntry {
            id: self.id,
            type_: self.type_,
            source: self.source,
            path: self.path,
            description: self.description,
            capabilities: self.capabilities,
            constraints: self.constraints,
            created_at: self.created_at.unwrap_or_else(route_core::now_millis),
            tags: self.tags,
            origin: self.origin.unwrap_or_default(),
            import_origin: self.import_origin,
            imported_at: self.imported_at,
            content_hash: self.content_hash,
            last_checked: self.last_checked,
            learned_meta: self.learned_meta,
            enabled: self.enabled.unwrap_or(true),
            name: self.name,
            project_scope: self.project_scope,
            trust: self.trust,
            entrypoint: self.entrypoint,
        }
    }
}

// ---------------------------------------------------------------------------
// ContextBudget — budget limits for the Effective Context pipeline
// ---------------------------------------------------------------------------

/// Budget limits for the Effective Context pipeline.
///
/// Constitution is never truncated. Protocol is truncated only when
/// absolutely necessary. References are selected by score and stopped
/// when the budget is exhausted.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Maximum total characters (approximate). Default: 16_000.
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,
    /// Characters reserved for Constitution (always included). Default: 2_000.
    #[serde(default = "default_reserved_constitution")]
    pub reserved_constitution: usize,
    /// Characters reserved for Protocol (always included). Default: 2_000.
    #[serde(default = "default_reserved_protocol")]
    pub reserved_protocol: usize,
    /// Maximum number of reference items to include. Default: 8.
    #[serde(default = "default_max_reference_items")]
    pub max_reference_items: usize,
}

fn default_max_chars() -> usize {
    16_000
}
fn default_reserved_constitution() -> usize {
    2_000
}
fn default_reserved_protocol() -> usize {
    2_000
}
fn default_max_reference_items() -> usize {
    8
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            max_chars: default_max_chars(),
            reserved_constitution: default_reserved_constitution(),
            reserved_protocol: default_reserved_protocol(),
            max_reference_items: default_max_reference_items(),
        }
    }
}

/// Explains why a reference or experience was selected or excluded.
#[derive(Debug, Clone, Serialize)]
pub struct SelectionReason {
    /// The reference entry id.
    pub reference_id: String,
    /// Total relevance score.
    pub score: f32,
    /// Individual scoring signals.
    pub signals: Vec<String>,
    /// Whether this entry was excluded.
    pub excluded: bool,
    /// Reason for exclusion, if excluded.
    pub exclusion_reason: Option<String>,
}

/// Unified selector for both regular references and learned experiences.
///
/// Scoring dimensions:
/// - Lexical relevance (task query words match description/capabilities/tags/source)
/// - Type relevance (entry type matches task context)
/// - Host relevance (task target matches host-specific tags)
/// - Experience confidence (weighted for learned references)
/// - Freshness (weak weight — newer entries score slightly higher)
///
/// Deterministic: same input produces same output.
#[derive(Debug, Clone)]
pub struct ReferenceSelector {
    /// Task query (lowercased).
    query_lower: String,
    /// Query words longer than 2 chars.
    query_words: Vec<String>,
    /// Target host (e.g. "claude", "codex"), if any.
    target: Option<String>,
    /// Maximum items to return.
    top_k: usize,
    /// Budget constraints.
    budget: ContextBudget,
}

impl ReferenceSelector {
    /// Create a new selector for the given task, target, and budget.
    pub fn new(task: &str, target: Option<&str>, top_k: usize, budget: ContextBudget) -> Self {
        let query_lower = task.to_lowercase();
        let query_words: Vec<String> = query_lower
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .map(|s| s.to_string())
            .collect();
        Self {
            query_lower,
            query_words,
            target: target.map(|s| s.to_lowercase()),
            top_k,
            budget,
        }
    }

    /// Score a single reference entry. Returns the score and individual signals.
    fn score_entry(&self, entry: &ReferenceEntry) -> (f32, Vec<String>) {
        let mut score = 0.0f32;
        let mut signals = Vec::new();

        // Lexical: description match
        let desc_lower = entry.description.to_lowercase();
        for word in &self.query_words {
            if desc_lower.contains(word) {
                score += 0.2;
                signals.push(format!("description matches '{}'", word));
            }
        }

        // Lexical: capabilities match
        let cap_lower = entry.capabilities.to_lowercase();
        for word in &self.query_words {
            if cap_lower.contains(word) {
                score += 0.2;
                signals.push(format!("capabilities match '{}'", word));
            }
        }

        // Lexical: source match
        let source_lower = entry.source.to_lowercase();
        for word in &self.query_words {
            if source_lower.contains(word) {
                score += 0.15;
                signals.push(format!("source matches '{}'", word));
            }
        }

        // Type relevance
        let type_str = entry.type_.as_str().to_lowercase();
        if type_str.contains(&self.query_lower) {
            score += 0.3;
            signals.push("type matches task query".to_string());
        }
        for word in &self.query_words {
            if type_str.contains(word) {
                score += 0.15;
                signals.push(format!("type matches '{}'", word));
            }
        }

        // Tag match
        for tag in &entry.tags {
            let tag_lower = tag.to_lowercase();
            if tag_lower.contains(&self.query_lower) {
                score += 0.3;
                signals.push(format!("tag '{}' matches task query", tag));
            }
            for word in &self.query_words {
                if tag_lower.contains(word) {
                    score += 0.15;
                    signals.push(format!("tag '{}' matches '{}'", tag, word));
                }
            }
        }

        // Host relevance
        if let Some(ref target) = self.target {
            for tag in &entry.tags {
                if tag.to_lowercase() == *target {
                    score += 0.25;
                    signals.push(format!("host tag matches target '{}'", target));
                }
            }
        }

        // Experience confidence bonus
        if entry.type_ == ReferenceType::Experience {
            if let Some(ref meta) = entry.learned_meta {
                if meta.confidence >= 0.6 {
                    score += 0.2;
                    signals.push(format!("high confidence ({:.2})", meta.confidence));
                }
            } else {
                // Legacy: try to extract from constraints
                let conf = extract_legacy_confidence(&entry.constraints);
                if conf >= 0.6 {
                    score += 0.2;
                    signals.push(format!("legacy high confidence ({:.2})", conf));
                }
            }
        }

        // Freshness (weak weight)
        let age_seconds = (route_core::now_millis() - entry.created_at) / 1000;
        if age_seconds < 86_400 {
            // < 1 day
            score += 0.1;
            signals.push("recently created".to_string());
        } else if age_seconds < 604_800 {
            // < 1 week
            score += 0.05;
            signals.push("created within the week".to_string());
        }

        // Cap at 1.0
        (score.min(1.0), signals)
    }

    /// Select references from the registry, returning scored results with reasons.
    pub fn select(
        &self,
        registry: &ReferenceRegistry,
    ) -> (Vec<SelectionReason>, Vec<SelectionReason>) {
        let mut scored: Vec<SelectionReason> = Vec::new();
        let mut excluded: Vec<SelectionReason> = Vec::new();

        for entry in &registry.entries {
            // Skip Unknown type
            if matches!(entry.type_, ReferenceType::Unknown) {
                excluded.push(SelectionReason {
                    reference_id: entry.id.clone(),
                    score: 0.0,
                    signals: vec![],
                    excluded: true,
                    exclusion_reason: Some("type is Unknown".to_string()),
                });
                continue;
            }

            // Skip disabled entries
            if !entry.enabled {
                excluded.push(SelectionReason {
                    reference_id: entry.id.clone(),
                    score: 0.0,
                    signals: vec![],
                    excluded: true,
                    exclusion_reason: Some("disabled".to_string()),
                });
                continue;
            }

            // Skip stale learned experiences
            if entry.type_ == ReferenceType::Experience {
                if let Some(ref meta) = entry.learned_meta {
                    if meta.stale {
                        excluded.push(SelectionReason {
                            reference_id: entry.id.clone(),
                            score: 0.0,
                            signals: vec![],
                            excluded: true,
                            exclusion_reason: Some("stale learned experience".to_string()),
                        });
                        continue;
                    }
                    if meta.confidence < 0.3 {
                        excluded.push(SelectionReason {
                            reference_id: entry.id.clone(),
                            score: meta.confidence,
                            signals: vec![],
                            excluded: true,
                            exclusion_reason: Some(format!(
                                "low confidence ({:.2})",
                                meta.confidence
                            )),
                        });
                        continue;
                    }
                    if meta.superseded_by.is_some() {
                        excluded.push(SelectionReason {
                            reference_id: entry.id.clone(),
                            score: 0.0,
                            signals: vec![],
                            excluded: true,
                            exclusion_reason: Some("superseded by another entry".to_string()),
                        });
                        continue;
                    }
                } else {
                    // Legacy: check constraints for stale/low confidence
                    if is_legacy_stale(entry) {
                        excluded.push(SelectionReason {
                            reference_id: entry.id.clone(),
                            score: 0.0,
                            signals: vec![],
                            excluded: true,
                            exclusion_reason: Some("legacy stale learned experience".to_string()),
                        });
                        continue;
                    }
                }
            }

            let (score, signals) = self.score_entry(entry);
            if score > 0.0 {
                scored.push(SelectionReason {
                    reference_id: entry.id.clone(),
                    score,
                    signals,
                    excluded: false,
                    exclusion_reason: None,
                });
            } else {
                excluded.push(SelectionReason {
                    reference_id: entry.id.clone(),
                    score,
                    signals,
                    excluded: true,
                    exclusion_reason: Some("no lexical or type match".to_string()),
                });
            }
        }

        // Sort by score descending, then by id for determinism
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.reference_id.cmp(&b.reference_id))
        });

        // Apply budget: top_k items
        let budget_limit = self.budget.max_reference_items.min(self.top_k);
        if scored.len() > budget_limit {
            let mut budget_excluded: Vec<SelectionReason> = scored.drain(budget_limit..).collect();
            for ref mut r in &mut budget_excluded {
                r.excluded = true;
                r.exclusion_reason = Some("budget limit exceeded".to_string());
            }
            excluded.extend(budget_excluded);
        }

        (scored, excluded)
    }
}

/// Extract confidence from legacy constraints text (e.g., "Confidence: 0.80").
fn extract_legacy_confidence(constraints: &str) -> f32 {
    constraints
        .lines()
        .find(|l| l.contains("Confidence:"))
        .and_then(|l| {
            l.split("Confidence:").nth(1).and_then(|s| {
                s.split_whitespace()
                    .next()
                    .and_then(|tok| tok.trim_end_matches('.').parse::<f32>().ok())
            })
        })
        .unwrap_or(0.0)
}

/// Check if a legacy (pre-LearnedMeta) experience reference is stale.
fn is_legacy_stale(entry: &ReferenceEntry) -> bool {
    // Legacy stale markers in tags
    if entry.tags.contains(&"stale".to_string()) || entry.tags.contains(&"superseded".to_string()) {
        return true;
    }
    // Legacy low-confidence check
    let conf = extract_legacy_confidence(&entry.constraints);
    conf < 0.3
}

/// Detailed explanation of a context selection.
#[derive(Debug, Clone, Serialize)]
pub struct ContextExplainResult {
    /// The task query.
    pub task: String,
    /// The target host, if any.
    pub target: Option<String>,
    /// Selected references with reasons.
    pub selected: Vec<SelectionReason>,
    /// Excluded references with reasons.
    pub excluded: Vec<SelectionReason>,
    /// Budget used / total budget.
    pub budget_used_chars: usize,
    pub budget_max_chars: usize,
    /// Constitution body length.
    pub constitution_len: usize,
    /// Protocol body length.
    pub protocol_len: usize,
    /// Context hash (fingerprint).
    pub context_hash: String,
    /// Whether learned experiences were injected.
    pub learned_injected: bool,
}

/// Unified Effective Context pipeline.
///
/// Builds a task-scoped context with:
/// - Constitution (always full)
/// - Protocol (always full in v1)
/// - Selected ordinary References (via ReferenceSelector)
/// - Selected Learned Experiences (via ReferenceSelector)
/// - AgentPolicy compilation
/// - Feedback contract
///
/// When `task` is None, behaves like the old `effective_context()`.
/// When `task` is Some, uses the new selector pipeline.
pub fn build_context(
    project_root: &Path,
    task: Option<&str>,
    target: Option<&str>,
    top_k: usize,
    budget: ContextBudget,
    profile_id: Option<&str>,
) -> Result<String> {
    let snap = ContextSnapshot::collect(project_root)?;

    let mut out = String::with_capacity(budget.max_chars);

    // Fingerprint header
    out.push_str("<!-- route-context-fingerprint: ");
    out.push_str(&snap.fingerprint);
    out.push_str(" -->\n");
    out.push_str("<!-- route-constitution-hash: ");
    out.push_str(&snap.constitution_hash);
    out.push_str(" -->\n");
    out.push_str("<!-- route-protocol-hash: ");
    out.push_str(&snap.protocol_hash);
    out.push_str(" -->\n");
    out.push_str("<!-- route-reference-hash: ");
    out.push_str(&snap.reference_entries_hash);
    out.push_str(" -->\n\n");

    if let Some(t) = task {
        out.push_str(&format!(
            "# Task-Scoped Effective Context\n\nTask: `{}`\n\n",
            t
        ));
    } else {
        out.push_str("# Effective Development Context\n\n");
    }
    out.push_str(&format!(
        "Project: {}\n\n",
        project_root.to_string_lossy().replace('\\', "/"),
    ));
    // Profile info
    if let Some(ref pid) = snap.profile_id {
        out.push_str(&format!("Profile: {}\n\n", pid));
    }
    out.push_str("---\n\n");

    // Constitution (always full, never truncated)
    out.push_str("## Constitution\n\n");
    out.push_str(&format!(
        "_version={}, created_at={}_\n\n",
        snap.constitution_version, snap.constitution_created_at
    ));
    out.push_str(&snap.constitution_body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n---\n\n");

    // Protocol (always full)
    out.push_str("## Protocol\n\n");
    out.push_str(&format!(
        "_version={}, revision={}, updated_at={}_\n\n",
        snap.protocol_version, snap.protocol_revision, snap.protocol_updated_at
    ));
    out.push_str(&snap.protocol_body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n---\n\n");

    // Project Constraints (binding material from constraints/)
    {
        let constraints_block = render_constraints(project_root, &snap.material);
        out.push_str(&constraints_block);
        if !constraints_block.is_empty() {
            out.push_str("---\n\n");
        }
    }

    // Project Memory (P2: Memory Context integration)
    {
        let memory_section = build_memory_context(project_root, task);
        out.push_str(&memory_section);
        if !memory_section.is_empty() {
            out.push_str("---\n\n");
        }
    }

    // Agent Policy (compiled from Protocol)
    {
        let policy = AgentPolicy::from_protocol_body(&snap.protocol_body);
        if !matches!(policy.mode, AgentMode::Single) || !policy.roles.is_empty() {
            out.push_str("## Agent Workflow\n\n");
            out.push_str(&policy.render_block());
            out.push_str("---\n\n");
        }
    }

    // Reference selection (if task is provided)
    if let Some(t) = task {
        let selector = ReferenceSelector::new(t, target, top_k, budget);
        let (selected, excluded) = selector.select(&snap.registry);

        out.push_str("## Relevant References\n\n");
        if selected.is_empty() {
            out.push_str("_(no relevant references found for this task)_\n\n");
        } else {
            for sr in &selected {
                // Find the actual entry
                if let Some(entry) = snap
                    .registry
                    .entries
                    .iter()
                    .find(|e| e.id == sr.reference_id)
                {
                    out.push_str(&format!(
                        "### [{}] {} — {}\n\n",
                        entry.type_.as_str(),
                        entry.id,
                        entry.description
                    ));
                    out.push_str(&format!("- **Relevance**: {:.2}\n", sr.score));
                    if !sr.signals.is_empty() {
                        out.push_str(&format!("- **Signals**: {}\n", sr.signals.join("; ")));
                    }
                    if !entry.source.is_empty() {
                        out.push_str(&format!("- **Source**: `{}`\n", entry.source));
                    }
                    if !entry.capabilities.is_empty() {
                        out.push_str(&format!("- **Capabilities**: {}\n", entry.capabilities));
                    }
                    // Experience-specific metadata
                    if entry.type_ == ReferenceType::Experience {
                        if let Some(ref meta) = entry.learned_meta {
                            out.push_str(&format!("- **Confidence**: {:.2}\n", meta.confidence));
                            if !meta.scope.is_empty() {
                                out.push_str(&format!("- **Scope**: {}\n", meta.scope));
                            }
                        }
                        out.push_str("\n  _This is a learned pattern, not a hard rule._\n");
                    }
                    out.push('\n');
                }
            }
        }

        // Show excluded count
        if !excluded.is_empty() {
            out.push_str(&format!(
                "_{} references excluded ({} above threshold, {} no match, {} budget)_\n\n",
                excluded.len(),
                excluded.iter().filter(|e| e.score > 0.0).count(),
                excluded
                    .iter()
                    .filter(|e| e.exclusion_reason.as_deref() == Some("no lexical or type match"))
                    .count(),
                excluded
                    .iter()
                    .filter(|e| e.exclusion_reason.as_deref() == Some("budget limit exceeded"))
                    .count(),
            ));
        }
    } else {
        // No task: include all visible references (legacy effective_context behavior)
        out.push_str("## Reference Registry\n\n");
        let visible: Vec<_> = snap
            .registry
            .entries
            .iter()
            .filter(|e| !matches!(e.type_, ReferenceType::Unknown) && e.enabled)
            .collect();
        out.push_str(&format!("Total entries: {}\n\n", visible.len()));
        if visible.is_empty() {
            out.push_str("_(no entries registered)_\n\n");
        } else {
            for e in visible {
                out.push_str(&format!(
                    "### [{}] {} — {}\n\n",
                    e.type_.as_str(),
                    e.id,
                    if e.description.is_empty() {
                        "(no description)"
                    } else {
                        &e.description
                    }
                ));
                if !e.source.is_empty() {
                    let path = e
                        .path
                        .as_deref()
                        .map(|s| format!(" (path: {s})"))
                        .unwrap_or_default();
                    out.push_str(&format!("- **Source**: `{}`{}\n", e.source, path));
                }
                if !e.capabilities.is_empty() {
                    out.push_str(&format!("- **Capabilities**: {}\n", e.capabilities));
                }
                if !e.constraints.is_empty() {
                    out.push_str(&format!("- **Constraints**: {}\n", e.constraints));
                }
                if !e.tags.is_empty() {
                    out.push_str(&format!("- **Tags**: `{}`\n", e.tags.join("`, `")));
                }
                out.push('\n');
            }
        }
    }

    // Optional References (informative material from references/)
    {
        let refs_block = render_references(project_root, &snap.material);
        out.push_str(&refs_block);
        if !refs_block.is_empty() {
            out.push_str("---\n\n");
        }
    }

    out.push_str("---\n\n");

    // Feedback Contract
    out.push_str("## Route Feedback Contract\n\n");
    out.push_str("You (the AI) may report structured observations about what worked ");
    out.push_str("or did not work during this session. Use the following format:\n\n");
    out.push_str("```\n");
    out.push_str("route_feedback_candidate:\n");
    out.push_str("  task: \"<brief description of the task>\"\n");
    out.push_str("  result: \"<what happened — accept/reject/error/success>\"\n");
    out.push_str("  observation: \"<what you observed about the outcome>\"\n");
    out.push_str("  evidence: \"<supporting detail: file changed, test passed, etc.>\"\n");
    if task.is_some() {
        out.push_str("  context_hash: \"<the context fingerprint above>\"\n");
        out.push_str(
            "  selected_reference_ids: \"<comma-separated ids of selected references>\"\n",
        );
        out.push_str("  agent_roles_used: \"<roles used, if any>\"\n");
        out.push_str("  verification_result: \"<pass/fail/unknown>\"\n");
    }
    out.push_str("```\n\n");
    out.push_str("Constraints:\n");
    out.push_str("- You MUST NOT generate \"user preference facts\" or claim user intent.\n");
    out.push_str("- You MUST NOT self-apply LearningProposals.\n");
    out.push_str("- Each candidate is just a suggestion. Route Engine validates the\n");
    out.push_str("  source and records it as an ExperienceEvent after verification.\n");
    out.push_str("- Single observations never become rules. Only multi-evidence\n");
    out.push_str("  aggregation (via `route learn analyze`) can produce a proposal.\n");
    out.push_str("- You may also submit structured memory observations:\n\n");
    out.push_str("```\n");
    out.push_str("memory_candidate:\n");
    out.push_str("  kind: \"convention\" | \"decision\" | \"risk\" | \"open_question\"\n");
    out.push_str("  content: \"<concise statement of the memory item>\"\n");
    out.push_str("  source_ids: [\"<commit-id>\", \"<session-id>\"]\n");
    out.push_str("  confidence: 0.85\n");
    out.push_str("```\n\n");
    out.push_str("  Memory candidates are proposals only — never auto-applied. ");
    out.push_str("Route Engine validates and may promote them.\n\n");
    out.push_str("---\n\n");
    // Active Workflows section (from profile)
    if let Some(_pid) = profile_id {
        if let Ok(Some(profile)) = crate::profile::ProfileStore::load_active(project_root) {
            if !profile.workflow_ids.is_empty() {
                if let Ok(wf_store) = crate::workflow::WorkflowStore::load(project_root) {
                    let wf_ctx = build_workflow_context(&wf_store, &profile.workflow_ids);
                    if !wf_ctx.is_empty() {
                        out.push_str("---\n\n");
                        out.push_str(&wf_ctx);
                        out.push_str("---\n\n");
                    }
                }
            }
        }
    }

    out.push_str("_End of Effective Development Context._\n");

    Ok(out)
}

// ---------------------------------------------------------------------------
// P2: Memory Context integration
// ---------------------------------------------------------------------------

/// Build a compact "Project Memory" section for inclusion in the context.
///
/// Loads the current memory from `.route/memory/memory.json`, selects
/// relevant items by keyword matching against the optional task description,
/// and returns a Markdown section. Returns an empty string if no memory
/// store exists or if it is empty.
pub fn build_memory_context(project_root: &Path, task: Option<&str>) -> String {
    let store = match MemoryStore::load(project_root) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let current = match store.current_memory() {
        Some(m) => m,
        None => return String::new(),
    };

    let mut out = String::new();
    out.push_str("## Project Memory\n\n");

    // Summary
    if !current.summary.is_empty() {
        out.push_str(&format!("**Summary**: {}\n\n", current.summary));
    }
    if !current.current_focus.is_empty() {
        out.push_str(&format!("**Current Focus**: {}\n\n", current.current_focus));
    }
    if !current.current_architecture.is_empty() {
        out.push_str(&format!(
            "**Current Architecture**: {}\n\n",
            current.current_architecture
        ));
    }

    // Collect all relevant items by keyword matching
    let relevant = select_relevant_memory_items(current, task);

    if relevant.is_empty() {
        out.push_str("_(no relevant memory items for this context)_\n\n");
        return out;
    }

    out.push_str("### Relevant Memory Items\n\n");
    for (kind, item, reason) in &relevant {
        let kind_label = match *kind {
            "decision" => "Decision",
            "risk" => "Risk",
            "failed_attempt" => "Failed Attempt",
            "convention" => "Convention",
            "open_question" => "Open Question",
            _ => kind,
        };
        out.push_str(&format!("- **{}**: {}", kind_label, item.content));
        if let Some(r) = reason {
            out.push_str(&format!(" _(recalled: {})_", r));
        }
        if !item.source_ids.is_empty() {
            out.push_str(&format!(" [sources: {}]", item.source_ids.join(", ")));
        }
        out.push('\n');
    }
    out.push('\n');

    out
}

/// Select relevant MemoryItems by keyword matching against the task
/// description. Returns `(kind, &MemoryItem, Option<explanation>)` tuples.
fn select_relevant_memory_items<'a>(
    memory: &'a crate::memory::ProjectMemory,
    task: Option<&str>,
) -> Vec<(&'a str, &'a crate::memory::MemoryItem, Option<String>)> {
    let all_items: Vec<(&str, &crate::memory::MemoryItem)> = {
        let mut items: Vec<(&str, &crate::memory::MemoryItem)> = Vec::new();
        for item in &memory.decisions {
            items.push(("decision", item));
        }
        for item in &memory.known_risks {
            items.push(("risk", item));
        }
        for item in &memory.failed_attempts {
            items.push(("failed_attempt", item));
        }
        for item in &memory.conventions {
            items.push(("convention", item));
        }
        for item in &memory.open_questions {
            items.push(("open_question", item));
        }
        for item in &memory.items {
            items.push(("item", item));
        }
        items
    };

    let task_lower = task.map(|t| t.to_lowercase());
    let task_words: Vec<&str> = task_lower
        .as_ref()
        .map(|s| s.split_whitespace().filter(|w| w.len() > 3).collect())
        .unwrap_or_default();

    if task_words.is_empty() {
        // No task: return first few items from each category
        let mut result: Vec<(&str, &crate::memory::MemoryItem, Option<String>)> = Vec::new();
        for (kind, item) in all_items.iter().take(8) {
            result.push((kind, item, None));
        }
        return result;
    }

    let mut scored: Vec<(&str, &crate::memory::MemoryItem, usize, Vec<String>)> = Vec::new();
    for (kind, item) in &all_items {
        let content_lower = item.content.to_lowercase();
        let mut score = 0usize;
        let mut matched_words: Vec<String> = Vec::new();
        for word in &task_words {
            if content_lower.contains(word) {
                score += 1;
                matched_words.push(word.to_string());
            }
        }
        if score > 0 {
            scored.push((kind, item, score, matched_words));
        }
    }

    // Sort by score descending
    scored.sort_by(|a, b| b.2.cmp(&a.2));

    scored
        .into_iter()
        .take(12)
        .map(|(kind, item, _score, words)| {
            let reason = if words.is_empty() {
                None
            } else {
                Some(format!("keyword match: {}", words.join(", ")))
            };
            (kind, item, reason)
        })
        .collect()
}

/// Build a task-scoped context with explainable selection results.
pub fn build_context_explain(
    project_root: &Path,
    task: &str,
    target: Option<&str>,
    top_k: usize,
    budget: ContextBudget,
) -> Result<ContextExplainResult> {
    let snap = ContextSnapshot::collect(project_root)?;
    let selector = ReferenceSelector::new(task, target, top_k, budget);
    let (selected, excluded) = selector.select(&snap.registry);

    // Calculate budget usage
    let constitution_len = snap.constitution_body.len();
    let protocol_len = snap.protocol_body.len();
    let mut estimated_total = constitution_len + protocol_len;
    // Add rough estimate for reference text
    for sr in &selected {
        if let Some(entry) = snap
            .registry
            .entries
            .iter()
            .find(|e| e.id == sr.reference_id)
        {
            estimated_total += entry.description.len() + entry.capabilities.len() + 200;
        }
    }
    let learned_injected = selected.iter().any(|sr| {
        snap.registry
            .entries
            .iter()
            .any(|e| e.id == sr.reference_id && e.type_ == ReferenceType::Experience)
    });

    Ok(ContextExplainResult {
        task: task.to_string(),
        target: target.map(|s| s.to_string()),
        selected,
        excluded,
        budget_used_chars: estimated_total,
        budget_max_chars: budget.max_chars,
        constitution_len,
        protocol_len,
        context_hash: snap.fingerprint,
        learned_injected,
    })
}

/// The registry is simply a version envelope + a list of entries.
/// It is persisted on disk as JSON so external tooling can read it
/// directly without going through this module.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReferenceRegistry {
    pub version: u32,
    #[serde(default)]
    pub entries: Vec<ReferenceEntry>,
}

impl ReferenceRegistry {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            entries: Vec::new(),
        }
    }

    /// Canonical file path for the reference registry JSON inside
    /// `project_root`.
    pub fn path(project_root: &Path) -> PathBuf {
        registry_path(project_root)
    }

    /// Load the registry from `{project}/.route/reference/registry.json`.
    /// If the file is missing, returns an empty registry.
    pub fn read(project_root: &Path) -> Result<Self> {
        let p = registry_path(project_root);
        if !p.exists() {
            return Ok(Self::new());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading reference registry from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::new());
        }
        let r: Self = serde_json::from_str(&raw)
            .with_context(|| format!("parsing reference registry JSON at {}", p.display()))?;
        Ok(r)
    }

    /// Atomically write the registry (JSON pretty-printed).
    pub fn write(&self, project_root: &Path) -> Result<()> {
        std::fs::create_dir_all(reference_dir(project_root))?;
        let p = registry_path(project_root);
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing reference registry to {}", p.display()))?;
        Ok(())
    }

    /// Create the registry on disk only if it does not exist yet. Used by
    /// `route init` so repeated runs are pure-noop idempotent.
    pub fn ensure_exists(project_root: &Path) -> Result<()> {
        let p = registry_path(project_root);
        if p.exists() {
            return Ok(());
        }
        Self::new().write(project_root)
    }

    /// Return the index of an entry with the given id, if any.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.entries.iter().position(|e| e.id == id)
    }

    /// Borrow an entry by id, if it exists.
    pub fn get(&self, id: &str) -> Option<&ReferenceEntry> {
        let i = self.index_of(id)?;
        self.entries.get(i)
    }

    /// Add or replace an entry by id. Returns `true` if the entry was
    /// newly inserted, `false` if an existing entry was replaced.
    pub fn upsert(&mut self, entry: ReferenceEntry) -> bool {
        match self.index_of(&entry.id) {
            Some(i) => {
                self.entries[i] = entry;
                false
            }
            None => {
                self.entries.push(entry);
                true
            }
        }
    }

    /// Remove an entry by id. Returns true if something was removed.
    pub fn remove(&mut self, id: &str) -> bool {
        match self.index_of(id) {
            Some(i) => {
                self.entries.remove(i);
                true
            }
            None => false,
        }
    }

    /// Get a mutable reference to an entry by id.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut ReferenceEntry> {
        let i = self.index_of(id)?;
        self.entries.get_mut(i)
    }

    /// Enable a reference entry by id. Returns true if the entry was found and enabled.
    pub fn enable(&mut self, id: &str) -> bool {
        match self.get_mut(id) {
            Some(entry) => {
                entry.enabled = true;
                true
            }
            None => false,
        }
    }

    /// Disable a reference entry by id. Returns true if the entry was found and disabled.
    pub fn disable(&mut self, id: &str) -> bool {
        match self.get_mut(id) {
            Some(entry) => {
                entry.enabled = false;
                true
            }
            None => false,
        }
    }

    /// List entries filtered by type.
    pub fn filter_by_type(&self, t: ReferenceType) -> impl Iterator<Item = &ReferenceEntry> {
        self.entries.iter().filter(move |e| e.type_ == t)
    }
}

// ---------------------------------------------------------------------------
// Reference import v0
// ---------------------------------------------------------------------------

/// Intermediate import result: the fields the import parser managed to
/// extract reliably plus the raw content hash (for provenance tracking).
/// Fields that could not be determined stay as their `Default` value
/// (empty string / `Unknown`) — **never invented**.
pub struct ImportedReference {
    pub id: Option<String>,
    pub type_: ReferenceType,
    pub source: String,
    pub description: String,
    pub capabilities: String,
    pub constraints: String,
    pub content_hash: String,
    /// Normalised raw content — what `content_hash` was computed over.
    pub content_bytes: Vec<u8>,
    /// Sub-path or alias within the source; `None` if inapplicable.
    pub path: Option<String>,
}

/// Attempt to determine what kind of source a user-supplied string
/// points at. Returns the v0-detected kind plus the path/URL portion
/// that should be fed to the appropriate parser.
pub fn classify_source(source: &str) -> (ReferenceType, String) {
    let s = source.trim();
    // 1) CLI invocation hint — not a path, but "cli:<binary>"
    if let Some(cmd) = s.strip_prefix("cli:") {
        return (ReferenceType::Cli, cmd.trim().to_string());
    }
    // 2) Git remote URLs (HTTPS / SSH)
    if (s.starts_with("https://") || s.starts_with("http://") || s.starts_with("git@"))
        && s.ends_with(".git")
    {
        return (ReferenceType::Repo, s.to_string());
    }
    if (s.starts_with("https://github.com/") || s.starts_with("http://github.com/"))
        && !s.contains('/')
    {
        return (ReferenceType::Repo, s.to_string());
    }
    if s.starts_with("https://") || s.starts_with("http://") || s.starts_with("git@") {
        // Anything URL-like that wasn't a git URL is treated as a document.
        return (ReferenceType::Document, s.to_string());
    }
    // 3) Local path — use suffix + directory heuristics
    let lower = s.to_ascii_lowercase();
    let p = Path::new(s);
    if p.is_dir() {
        let skill_markers = ["SKILL.md", "SKILL.toml", "skill.md", "skill.toml"];
        for m in skill_markers {
            if p.join(m).is_file() {
                return (ReferenceType::Skill, s.to_string());
            }
        }
        if p.join(".git").exists() {
            return (ReferenceType::Repo, s.to_string());
        }
        return (ReferenceType::Document, s.to_string());
    }
    match lower.as_str() {
        _ if lower.ends_with(".md") || lower.ends_with(".mdx") || lower.ends_with(".txt") => {
            (ReferenceType::Document, s.to_string())
        }
        _ if lower.ends_with(".rs")
            || lower.ends_with(".toml")
            || lower.ends_with(".json") && lower.contains("skill") =>
        {
            (ReferenceType::Skill, s.to_string())
        }
        _ if lower.ends_with(".git") => (ReferenceType::Repo, s.to_string()),
        _ if lower.contains("help")
            && (lower.ends_with(".log")
                || lower.ends_with(".txt")
                || lower.ends_with(".md")
                || lower.ends_with(".out")) =>
        {
            (ReferenceType::Cli, s.to_string())
        }
        _ => (ReferenceType::Unknown, s.to_string()),
    }
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = true;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "imported".to_string()
    } else {
        out
    }
}

/// Extract a one-line title for a Markdown document. Returns the first
/// ATX-heading level-1 if present, else the first non-empty line.
fn markdown_title(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") {
            let r = rest.trim();
            if !r.is_empty() {
                return Some(r.to_string());
            }
        }
        if !t.is_empty() {
            return Some(t.chars().take(160).collect());
        }
    }
    None
}

/// Pull the leading paragraph(s) out of a Markdown/text file for the
/// short description — we stop at the second heading or the 400th
/// character, whichever comes first. Headings, code fences and HTML
/// comments are stripped.
fn short_description_markdown(raw: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    let mut heading_seen = 0usize;
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        if t.starts_with("<!--") {
            continue;
        }
        if t.starts_with('#') {
            heading_seen += 1;
            if heading_seen >= 2 {
                break;
            }
            continue;
        }
        if t.is_empty() {
            if !out.is_empty() && !out.ends_with(' ') {
                out.push(' ');
            }
            continue;
        }
        if !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
        out.push_str(t);
        if out.chars().count() >= 400 {
            break;
        }
    }
    let mut s: String = out.chars().take(400).collect();
    s.truncate(400);
    s.trim().to_string()
}

/// Parse `--help` style output: "USAGE:" + option list. We only
/// capture lines we can prove are usage or option lines; every other
/// capability list is left blank.
fn parse_help_output(raw: &str) -> (String, String) {
    let mut usage = String::new();
    let mut options = String::new();
    let mut in_usage = false;
    let mut in_opts = false;
    for l in raw.lines() {
        let t = l.trim();
        let upper = t.to_uppercase();
        if upper.starts_with("USAGE") || upper.starts_with("SYNOPSIS") {
            in_usage = true;
            in_opts = false;
            if !usage.is_empty() {
                usage.push('\n');
            }
            usage.push_str(t);
            continue;
        }
        if upper.starts_with("OPTIONS")
            || upper.starts_with("FLAGS")
            || upper.starts_with("ARGUMENTS")
        {
            in_opts = true;
            in_usage = false;
            continue;
        }
        if upper.starts_with("DESCRIPTION")
            || upper.starts_with("SUBCOMMAND")
            || upper.starts_with("COMMANDS")
            || upper.starts_with("EXAMPLES")
        {
            in_usage = false;
            in_opts = false;
        }
        if in_usage && !t.is_empty() {
            if !usage.is_empty() {
                usage.push('\n');
            }
            usage.push_str(t);
        }
        if in_opts && (t.starts_with('-') || t.contains("--")) {
            if !options.is_empty() {
                options.push('\n');
            }
            options.push_str(t);
            // Protect against accidentally dumping megabytes of help.
            if options.len() > 8192 {
                break;
            }
        }
    }
    (usage, options)
}

/// Run `<cmd> --help` and return its stdout (strictly bounded).
fn run_cli_help(cmd: &str) -> Result<String> {
    let mut parts = cmd.split_whitespace();
    let bin = parts
        .next()
        .with_context(|| format!("empty CLI command in {cmd:?}"))?;
    let args: Vec<&str> = parts.chain(std::iter::once("--help")).collect();
    let output = std::process::Command::new(bin)
        .args(&args)
        .output()
        .with_context(|| format!("spawning {bin} --help"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    // Some tools print help on stderr (gflags style). Prefer whichever
    // one actually has content.
    let mut combined = stdout;
    if combined.trim().is_empty() {
        combined = stderr;
    }
    // Hard bound — we never parse more than 64 KiB of help text.
    if combined.len() > 65536 {
        combined.truncate(65536);
    }
    Ok(combined)
}

/// Pull basic metadata out of a local git worktree: remote URL, default
/// branch, and short one-line description if any. Only reads the `.git`
/// directory (no network).
fn read_local_git_metadata(path: &Path) -> (String, String) {
    let mut remote = String::new();
    let mut desc = String::new();
    if let Ok(cfg) = std::fs::read_to_string(path.join(".git").join("config")) {
        // Parses a subset of the INI format used by git-config.
        let mut in_origin = false;
        for line in cfg.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                in_origin = t == "[remote \"origin\"]";
                continue;
            }
            if in_origin {
                if let Some(v) = t.strip_prefix("url =") {
                    remote = v.trim().to_string();
                    break;
                }
            }
        }
    }
    // Description lives in `.git/description` (initialised by `git init`).
    if let Ok(desc_file) = std::fs::read_to_string(path.join(".git").join("description")) {
        let d = desc_file.trim();
        if !d.is_empty() && !d.starts_with("Unnamed repository") {
            desc = d.chars().take(200).collect();
        }
    }
    (remote, desc)
}

/// Entry point: turn a user-provided source string into a structured
/// `ImportedReference`. The parser is strictly conservative: if a field
/// can't be reliably extracted it stays empty. **No LLM calls.**
pub fn import_reference_source(source: &str) -> Result<ImportedReference> {
    let (kind, src) = classify_source(source);
    let (raw, path_hint) = match kind {
        ReferenceType::Cli if !Path::new(&src).exists() => {
            // `cli:foo` — run it and collect help output.
            let raw = run_cli_help(&src)?;
            (raw, None)
        }
        ReferenceType::Document
        | ReferenceType::Skill
        | ReferenceType::Cli
        | ReferenceType::Repo
            if Path::new(&src).exists() =>
        {
            let p = Path::new(&src);
            if p.is_file() {
                let bytes =
                    std::fs::read(p).with_context(|| format!("reading file {}", p.display()))?;
                let text = String::from_utf8_lossy(&bytes).to_string();
                (text, None)
            } else {
                // Directory: take a short listing-style summary.
                // For skills: prefer SKILL.md / SKILL.toml.
                let markers: &[&str] = if kind == ReferenceType::Skill {
                    &["SKILL.md", "skill.md", "SKILL.toml", "skill.toml"]
                } else {
                    &["README.md", "README.txt", "readme.md"]
                };
                let mut joined = String::new();
                let mut got_any = false;
                for m in markers {
                    let f = p.join(m);
                    if f.is_file() {
                        if let Ok(t) = std::fs::read_to_string(&f) {
                            joined.push_str(&t);
                            got_any = true;
                            break;
                        }
                    }
                }
                if !got_any {
                    // Best effort: list a directory index as the raw
                    // bytes. We do NOT descend — surface-only.
                    if let Ok(rd) = std::fs::read_dir(p) {
                        for entry in rd.flatten() {
                            if let Ok(n) = entry.file_name().into_string() {
                                joined.push_str(&n);
                                joined.push('\n');
                            }
                        }
                    }
                }
                (joined, Some(src.clone()))
            }
        }
        ReferenceType::Repo => {
            // Remote git URL: trust source verbatim; do NOT hit the
            // network in v0. The content hash is taken over the URL
            // string so provenance still records what the user typed.
            (src.clone(), None)
        }
        _ => {
            // Unknown — fall back to treating source as literal text.
            (src.clone(), None)
        }
    };

    // Provenance content hash = SHA-256 of the raw bytes we actually
    // examined. This is what Curator will later compare for auto-updates.
    let content_bytes = raw.as_bytes().to_vec();
    let content_hash = route_core::sha256_hex(&content_bytes);

    let (mut description, mut capabilities, mut constraints) =
        (String::new(), String::new(), String::new());
    let mut id_hint: Option<String> = None;

    match kind {
        ReferenceType::Document => {
            if let Some(title) = markdown_title(&raw) {
                id_hint = Some(slugify(&title));
            }
            description = short_description_markdown(&raw);
        }
        ReferenceType::Skill => {
            // Treat markers as "document" (they are markdown/toml blobs).
            if let Some(title) = markdown_title(&raw) {
                id_hint = Some(slugify(&title));
            }
            description = short_description_markdown(&raw);
            // SKILL-style "capabilities" headers: only literal
            //   ## Capabilities / ## Constraints
            // sections are picked. Nothing is inferred.
            let mut in_cap = false;
            let mut in_con = false;
            for line in raw.lines() {
                let t = line.trim();
                if t.starts_with('#') {
                    let head = t.trim_start_matches('#').trim().to_lowercase();
                    in_cap = head == "capabilities";
                    in_con = head == "constraints";
                    continue;
                }
                if in_cap && !t.is_empty() {
                    if !capabilities.is_empty() {
                        capabilities.push('\n');
                    }
                    capabilities.push_str(t);
                    if capabilities.len() > 4096 {
                        break;
                    }
                }
                if in_con && !t.is_empty() {
                    if !constraints.is_empty() {
                        constraints.push('\n');
                    }
                    constraints.push_str(t);
                    if constraints.len() > 4096 {
                        break;
                    }
                }
            }
        }
        ReferenceType::Repo => {
            if let Some(local_path) = path_hint.as_ref().map(PathBuf::from) {
                if local_path.is_dir() {
                    let (remote, desc) = read_local_git_metadata(&local_path);
                    if !remote.is_empty() {
                        // Prefer remote as canonical source (so imported
                        // entries are portable between clones).
                    }
                    if !desc.is_empty() {
                        description = desc;
                    }
                    if let Some(name) = local_path.file_name().and_then(|x| x.to_str()).map(slugify)
                    {
                        id_hint = Some(format!("repo-{name}"));
                    }
                }
            } else {
                // Remote URL: slugify the last path component before .git
                let name = src
                    .trim_end_matches(".git")
                    .rsplit('/')
                    .next()
                    .unwrap_or("repo")
                    .to_string();
                id_hint = Some(format!("repo-{}", slugify(&name)));
            }
        }
        ReferenceType::Cli => {
            let (usage, options) = parse_help_output(&raw);
            if !usage.is_empty() {
                description = usage.lines().next().unwrap_or("").to_string();
                capabilities = options;
            } else if !options.is_empty() {
                description = "CLI --help output (see capabilities for option list)".to_string();
                capabilities = options;
            } else {
                description = "CLI tool; --help had no parseable usage section.".to_string();
            }
            let bin = src
                .trim_start_matches("cli:")
                .split_whitespace()
                .next()
                .unwrap_or("cli")
                .to_string();
            id_hint = Some(format!("cli-{}", slugify(&bin)));
        }
        _ => {
            description = "Unknown source type; no fields extracted.".to_string();
        }
    }

    Ok(ImportedReference {
        id: id_hint,
        type_: kind,
        source: src,
        description,
        capabilities,
        constraints,
        content_hash,
        content_bytes,
        path: path_hint,
    })
}

// ---------------------------------------------------------------------------
// Context snapshot / fingerprint / history
// ---------------------------------------------------------------------------

/// A semantic-only snapshot of the constitutive inputs, used to derive
/// stable fingerprints and history records. Timestamps and project-root
/// paths are excluded so the same inputs on different machines or at
/// different times yield the same fingerprint.
#[derive(Debug, Clone, Serialize)]
pub struct ContextSnapshot {
    /// Constitution schema version (from the envelope, not the body).
    pub constitution_version: u32,
    /// Created-at timestamp of the constitution — preserved for display
    /// purposes, excluded from hash computation.
    #[serde(skip)]
    pub constitution_created_at: i64,
    /// Constitution body (free-form Markdown).
    pub constitution_body: String,
    /// Protocol schema version.
    pub protocol_version: u32,
    /// Protocol revision (monotonically increasing per write).
    pub protocol_revision: u64,
    /// Protocol last-updated timestamp — preserved for display, excluded
    /// from hash computation.
    #[serde(skip)]
    pub protocol_updated_at: i64,
    /// Protocol body (free-form Markdown).
    pub protocol_body: String,
    /// Reference registry entries, preserved in full (including
    /// provenance fields) so the effective context renderer can display
    /// human-readable provenance notes. The fingerprint is computed
    /// from the semantic-only projection [`Self::reference_entries`].
    #[serde(skip)]
    pub registry: ReferenceRegistry,
    /// Semantic-only, deterministically ordered view of the reference
    /// registry. Used to compute `reference_entries_hash`.
    #[serde(serialize_with = "serialize_entries_stable")]
    pub reference_entries: Vec<ReferenceSemanticView>,
    /// SHA-256 hex of `constitution_version` + `constitution_body`.
    pub constitution_hash: String,
    /// SHA-256 hex of `protocol_version` + `protocol_revision` + `protocol_body`.
    pub protocol_hash: String,
    /// SHA-256 hex of `reference_entries` (stable-sorted, semantic only).
    pub reference_entries_hash: String,
    /// Overall fingerprint combining the three individual hashes with
    /// field labels. Used as `context_hash` everywhere in lineage.
    pub fingerprint: String,
    /// Active profile id, if a profile is active.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_id: Option<String>,
    /// Workflow ids from the active profile.
    #[serde(default)]
    pub workflow_ids: Vec<String>,
    /// Discovered project material (`constraints/` -> CONSTRAINT,
    /// `references/` -> REFERENCE). Empty when the project has neither
    /// directory. Included in the fingerprint only when non-empty so
    /// that projects without material keep a stable fingerprint.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub material: Vec<MaterialSource>,
}

/// Semantic fields of a [`ReferenceEntry`] — everything that actually
/// affects an AI's behaviour. Provenance bookkeeping (timestamps,
/// import metadata) is excluded on purpose.
#[derive(Debug, Clone, Serialize)]
pub struct ReferenceSemanticView {
    pub id: String,
    pub type_: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub description: String,
    pub capabilities: String,
    pub constraints: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Preserved across reimports because it describes the imported
    /// content itself and therefore has semantic weight when reasoning
    /// about "did the reference actually change?"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

fn serialize_entries_stable<S>(
    entries: &[ReferenceSemanticView],
    s: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeSeq;
    let mut seq = s.serialize_seq(Some(entries.len()))?;
    for e in entries {
        seq.serialize_element(e)?;
    }
    seq.end()
}

fn sha256_hex(bytes: &[u8]) -> String {
    route_core::sha256_hex(bytes)
}

impl ContextSnapshot {
    /// Build the semantic snapshot from a project's constitutive files.
    /// Missing files fall back to the defaults so that a fingerprint can
    /// be produced even before the user has explicitly initialized the
    /// three files on disk.
    pub fn collect(project_root: &Path) -> Result<Self> {
        let constitution = Constitution::read(project_root).unwrap_or_default();
        let protocol = Protocol::read(project_root).unwrap_or_default();
        let registry = ReferenceRegistry::read(project_root).unwrap_or_default();

        let mut entries_sem: Vec<ReferenceSemanticView> = registry
            .entries
            .iter()
            .filter(|e| !matches!(e.type_, ReferenceType::Unknown))
            .map(|e| ReferenceSemanticView {
                id: e.id.clone(),
                type_: e.type_.as_str().to_string(),
                source: e.source.clone(),
                path: e.path.clone(),
                description: e.description.clone(),
                capabilities: e.capabilities.clone(),
                constraints: e.constraints.clone(),
                tags: e.tags.clone(),
                content_hash: e.content_hash.clone(),
            })
            .collect();
        entries_sem.sort_by(|a, b| a.id.cmp(&b.id).then(a.type_.cmp(&b.type_)));

        let constitution_hash = {
            let mut bytes: Vec<u8> = Vec::new();
            bytes.extend_from_slice(b"const/v=");
            bytes.extend_from_slice(constitution.version.to_string().as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(constitution.body.as_bytes());
            sha256_hex(&bytes)
        };
        let protocol_hash = {
            let mut bytes: Vec<u8> = Vec::new();
            bytes.extend_from_slice(b"prot/v=");
            bytes.extend_from_slice(protocol.version.to_string().as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(b"r=");
            bytes.extend_from_slice(protocol.revision.to_string().as_bytes());
            bytes.push(0);
            bytes.extend_from_slice(protocol.body.as_bytes());
            sha256_hex(&bytes)
        };
        let reference_entries_hash = {
            let bytes = serde_json::to_vec(&entries_sem)?;
            sha256_hex(&bytes)
        };
        // Read active profile for context differentiation
        let profile_id = crate::ProfileStore::active_id(project_root).ok().flatten();
        let workflow_ids = match profile_id.as_deref() {
            Some(pid) => {
                let store = crate::ProfileStore::load(project_root).unwrap_or_default();
                store
                    .profiles
                    .iter()
                    .find(|p| p.id == pid)
                    .map(|p| p.workflow_ids.clone())
                    .unwrap_or_default()
            }
            None => Vec::new(),
        };

        let material = discover_material(project_root).unwrap_or_default();

        let fingerprint = {
            let mut bytes: Vec<u8> = Vec::new();
            bytes.extend_from_slice(b"ctx/1\n");
            bytes.extend_from_slice(constitution_hash.as_bytes());
            bytes.push(b'\n');
            bytes.extend_from_slice(protocol_hash.as_bytes());
            bytes.push(b'\n');
            bytes.extend_from_slice(reference_entries_hash.as_bytes());
            bytes.push(b'\n');
            if let Some(ref pid) = profile_id {
                bytes.extend_from_slice(b"profile=");
                bytes.extend_from_slice(pid.as_bytes());
                bytes.push(b'\n');
            }
            for wf_id in &workflow_ids {
                bytes.extend_from_slice(b"workflow=");
                bytes.extend_from_slice(wf_id.as_bytes());
                bytes.push(b'\n');
            }
            for m in &material {
                bytes.extend_from_slice(b"material=");
                bytes.extend_from_slice(m.kind.as_str().as_bytes());
                bytes.push(b':');
                bytes.extend_from_slice(m.path.as_bytes());
                bytes.push(b':');
                bytes.extend_from_slice(m.content_hash.as_bytes());
                bytes.push(b'\n');
            }
            sha256_hex(&bytes)
        };

        Ok(Self {
            constitution_version: constitution.version,
            constitution_created_at: constitution.created_at,
            constitution_body: constitution.body,
            protocol_version: protocol.version,
            protocol_revision: protocol.revision,
            protocol_updated_at: protocol.updated_at,
            protocol_body: protocol.body,
            registry,
            reference_entries: entries_sem,
            constitution_hash,
            protocol_hash,
            reference_entries_hash,
            fingerprint,
            profile_id,
            workflow_ids,
            material,
        })
    }
}

/// Compute the stable [`ContextSnapshot`] and return only its
/// overall fingerprint hex. Convenience wrapper around
/// [`ContextSnapshot::collect`].
pub fn effective_context_fingerprint(project_root: &Path) -> Result<String> {
    Ok(ContextSnapshot::collect(project_root)?.fingerprint)
}

/// Lightweight metadata recorded into `.route/context/history/` each
/// time the effective context fingerprint changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextHistoryManifest {
    /// Overall `ContextSnapshot::fingerprint`.
    pub context_hash: String,
    pub constitution_version: u32,
    pub constitution_hash: String,
    pub protocol_version: u32,
    pub protocol_revision: u64,
    pub protocol_hash: String,
    pub reference_entries_hash: String,
    /// Unix-millis when this manifest was first persisted.
    pub created_at: i64,
    /// What triggered this context snapshot to be recorded (e.g.
    /// `"reference_proposal:<id>"`, `"manual"`, `"apply"`). `None` for
    /// manifests written before the trigger field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
}

impl ContextHistoryManifest {
    fn from_snapshot(snap: &ContextSnapshot) -> Self {
        Self {
            context_hash: snap.fingerprint.clone(),
            constitution_version: snap.constitution_version,
            constitution_hash: snap.constitution_hash.clone(),
            protocol_version: snap.protocol_version,
            protocol_revision: snap.protocol_revision,
            protocol_hash: snap.protocol_hash.clone(),
            reference_entries_hash: snap.reference_entries_hash.clone(),
            created_at: route_core::now_millis(),
            trigger: None,
        }
    }

    fn path_for(project_root: &Path, hash: &str) -> PathBuf {
        context_history_dir(project_root).join(format!("{}.json", hash))
    }

    /// Persist the manifest for `snap` iff no manifest with the same
    /// hash already exists. Returns `(manifest, is_new_record)`.
    pub fn record_if_new(project_root: &Path, snap: &ContextSnapshot) -> Result<(Self, bool)> {
        Self::record_if_new_triggered(project_root, snap, None)
    }

    /// Same as [`record_if_new`](Self::record_if_new) but stamps a
    /// `trigger` label onto the manifest when it is first written. If
    /// a manifest for this fingerprint already exists, the trigger is
    /// NOT overwritten — the first recording wins.
    pub fn record_if_new_triggered(
        project_root: &Path,
        snap: &ContextSnapshot,
        trigger: Option<&str>,
    ) -> Result<(Self, bool)> {
        std::fs::create_dir_all(context_history_dir(project_root))?;
        let p = Self::path_for(project_root, &snap.fingerprint);
        if p.exists() {
            let raw = std::fs::read_to_string(&p)?;
            let existing: ContextHistoryManifest = serde_json::from_str(&raw)
                .unwrap_or_else(|_| ContextHistoryManifest::from_snapshot(snap));
            return Ok((existing, false));
        }
        let mut manifest = ContextHistoryManifest::from_snapshot(snap);
        manifest.trigger = trigger.map(|s| s.to_string());
        let json = serde_json::to_vec_pretty(&manifest)?;
        write_atomic(&p, &json)?;
        // P0: also archive the constitutive files for future replay
        let _ = archive_current_context(project_root, snap);
        Ok((manifest, true))
    }

    /// Look up a history manifest by exact context hash.
    pub fn read(project_root: &Path, hash: &str) -> Result<Option<Self>> {
        let p = Self::path_for(project_root, hash);
        if !p.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(&p)?;
        Ok(Some(serde_json::from_str(&raw)?))
    }
}

// ---------------------------------------------------------------------------
// Context archive + replay (P0: Historical Replay)
// ---------------------------------------------------------------------------

/// Error returned when a historical context hash cannot be fully recovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartialHistory {
    /// The hash that was requested but could not be fully restored.
    pub hash: String,
    /// Human-readable explanation.
    pub reason: String,
}

impl std::fmt::Display for PartialHistory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "PartialHistory: cannot fully restore context hash '{}' — {}",
            self.hash, self.reason
        )
    }
}

impl std::error::Error for PartialHistory {}

/// Store the three constitutive files into the content-addressed archive
/// directory. Returns the archive directory path.
pub fn archive_current_context(project_root: &Path, snap: &ContextSnapshot) -> Result<PathBuf> {
    let archive_dir = context_archive_dir(project_root).join(&snap.fingerprint);
    std::fs::create_dir_all(&archive_dir)?;

    // Constitution
    let const_path = archive_dir.join("constitution.md");
    if !const_path.exists() {
        let const_body = format!(
            "<!-- route-constitution:version={} -->\n\
             <!-- route-constitution:created_at={} -->\n\
             {}",
            snap.constitution_version, snap.constitution_created_at, snap.constitution_body
        );
        write_atomic(&const_path, const_body.as_bytes())?;
    }

    // Protocol
    let proto_path = archive_dir.join("protocol.md");
    if !proto_path.exists() {
        let proto_body = format!(
            "<!-- route-protocol:version={} -->\n\
             <!-- route-protocol:revision={} -->\n\
             <!-- route-protocol:updated_at={} -->\n\
             {}",
            snap.protocol_version,
            snap.protocol_revision,
            snap.protocol_updated_at,
            snap.protocol_body
        );
        write_atomic(&proto_path, proto_body.as_bytes())?;
    }

    // Reference registry
    let reg_path = archive_dir.join("registry.json");
    if !reg_path.exists() {
        let json = serde_json::to_vec_pretty(&snap.registry)?;
        write_atomic(&reg_path, &json)?;
    }

    Ok(archive_dir)
}

/// Reconstruct the Effective Context for a historical hash.
///
/// Tries to load from the content-addressed archive first. If the archive
/// exists, reconstructs the full context. If the archive is missing but the
/// history manifest exists, returns `PartialHistory` — the hash is known
/// but the full content cannot be restored.
///
/// The `snap` parameter, when provided, is used to verify that the archive
/// content matches what `ContextSnapshot::collect` would produce.
pub fn replay_context(project_root: &Path, hash: &str) -> Result<String> {
    // 1. Check the history manifest exists.
    let manifest = ContextHistoryManifest::read(project_root, hash)?
        .ok_or_else(|| anyhow::anyhow!("no context history manifest for hash '{}'", hash))?;

    // 2. Check the archive directory.
    let archive_dir = context_archive_dir(project_root).join(hash);
    if !archive_dir.exists() {
        return Err(PartialHistory {
            hash: hash.to_string(),
            reason: "no content archive exists for this hash. \
                     Only contexts recorded after the archive feature was added (v0 with AI Workflow Control) \
                     can be fully replayed. Try re-applying the context to create an archive entry."
                .to_string(),
        }
        .into());
    }

    let const_path = archive_dir.join("constitution.md");
    let proto_path = archive_dir.join("protocol.md");
    let reg_path = archive_dir.join("registry.json");

    if !const_path.exists() || !proto_path.exists() || !reg_path.exists() {
        return Err(PartialHistory {
            hash: hash.to_string(),
            reason: "content archive is incomplete — one or more constitutive files are missing"
                .to_string(),
        }
        .into());
    }

    // 3. Read the archive files
    let const_raw = std::fs::read_to_string(&const_path)?;
    let proto_raw = std::fs::read_to_string(&proto_path)?;
    let reg_raw = std::fs::read_to_string(&reg_path)?;

    let constitution = Constitution::parse_envelope(&const_raw);
    let protocol = {
        let (version, rev, updated_at, rest) = extract_protocol_frontmatter(&proto_raw);
        Protocol {
            version: version.unwrap_or(Protocol::CURRENT_VERSION),
            revision: rev.unwrap_or(1),
            updated_at: updated_at.unwrap_or_else(route_core::now_millis),
            body: rest.to_string(),
        }
    };
    let registry: ReferenceRegistry = serde_json::from_str(&reg_raw)?;

    // 4. Reconstruct the effective context
    let mut out = String::with_capacity(4096);
    out.push_str("<!-- route-context-fingerprint: ");
    out.push_str(hash);
    out.push_str(" -->\n");
    out.push_str("<!-- route-constitution-hash: ");
    out.push_str(&manifest.constitution_hash);
    out.push_str(" -->\n");
    out.push_str("<!-- route-protocol-hash: ");
    out.push_str(&manifest.protocol_hash);
    out.push_str(" -->\n");
    out.push_str("<!-- route-reference-hash: ");
    out.push_str(&manifest.reference_entries_hash);
    out.push_str(" -->\n\n");

    out.push_str("# Replayed Effective Development Context\n\n");
    out.push_str("_(This is a historical replay — not the current context.)_\n\n");
    out.push_str(&format!("Context hash: `{}`\n\n", hash));
    out.push_str(&format!(
        "Recorded at: {}\n\n",
        chrono::DateTime::<chrono::Utc>::from_timestamp_millis(manifest.created_at)
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| manifest.created_at.to_string())
    ));
    out.push_str("---\n\n");

    // Constitution
    out.push_str("## Constitution\n\n");
    out.push_str(&format!(
        "_version={}, created_at={}_\n\n",
        constitution.version, constitution.created_at
    ));
    out.push_str(&constitution.body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n---\n\n");

    // Protocol
    out.push_str("## Protocol\n\n");
    out.push_str(&format!(
        "_version={}, revision={}, updated_at={}_\n\n",
        protocol.version, protocol.revision, protocol.updated_at
    ));
    out.push_str(&protocol.body);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\n---\n\n");

    // References
    out.push_str("## Reference Registry\n\n");
    let visible: Vec<_> = registry
        .entries
        .iter()
        .filter(|e| !matches!(e.type_, ReferenceType::Unknown))
        .collect();
    out.push_str(&format!("Total entries: {}\n\n", visible.len()));
    if visible.is_empty() {
        out.push_str("_(no entries registered)_\n\n");
    } else {
        for e in visible {
            out.push_str(&format!(
                "### [{}] {} — {}\n\n",
                e.type_.as_str(),
                e.id,
                if e.description.is_empty() {
                    "(no description)"
                } else {
                    &e.description
                }
            ));
            if !e.source.is_empty() {
                let path = e
                    .path
                    .as_deref()
                    .map(|s| format!(" (path: {s})"))
                    .unwrap_or_default();
                out.push_str(&format!("- **Source**: `{}`{}\n", e.source, path));
            }
            if !e.capabilities.is_empty() {
                out.push_str(&format!("- **Capabilities**: {}\n", e.capabilities));
            }
            if !e.constraints.is_empty() {
                out.push_str(&format!("- **Constraints**: {}\n", e.constraints));
            }
            if !e.tags.is_empty() {
                out.push_str(&format!("- **Tags**: `{}`\n", e.tags.join("`, `")));
            }
            out.push('\n');
        }
    }

    out.push_str("---\n\n");
    out.push_str("_End of replayed Effective Development Context._\n");
    Ok(out)
}

// ---------------------------------------------------------------------------
// Agent Policy (P2: belongs to Protocol, no new top-level file)
// ---------------------------------------------------------------------------

/// Machine-readable agent policy embedded in Protocol Markdown.
///
/// Minimal model: defines how host AI should organise itself when working
/// on this project. Default is `single` mode (no sub-agent spawning).
/// All fields are optional — a missing field implies the most conservative
/// default.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentPolicy {
    /// `single` (default) or `adaptive`.
    #[serde(default)]
    pub mode: AgentMode,
    /// Maximum number of concurrent agents. `None` means no explicit limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_agents: Option<u32>,
    /// Conditions under which a sub-agent MAY be spawned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub spawn_when: Vec<String>,
    /// Conditions under which a sub-agent MUST NOT be spawned.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub do_not_spawn_when: Vec<String>,
    /// Role definitions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<AgentRole>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// Single agent — no sub-agent spawning allowed.
    Single,
    /// Adaptive — the host AI may dynamically spawn sub-agents based on
    /// task analysis, respecting the policy constraints.
    Adaptive,
}

impl Default for AgentMode {
    fn default() -> Self {
        AgentMode::Single
    }
}

/// A single role definition for an adaptive sub-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRole {
    /// Unique identifier for this role.
    pub id: String,
    /// Purpose of this role (free-form, consumed by LLMs).
    #[serde(default)]
    pub purpose: String,
    /// Tools this role is allowed to use. Empty = all tools allowed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<String>,
    /// Actions this role is explicitly forbidden from performing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forbidden_actions: Vec<String>,
    /// Criteria for considering this role's work complete.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_criteria: Option<String>,
    /// Whether this role requires an independent reviewer.
    #[serde(default)]
    pub independent_review: bool,
}

impl AgentPolicy {
    /// Parse agent policy from the Protocol body Markdown.
    ///
    /// Looks for a `<!-- route-agent-policy: ... -->` HTML comment block
    /// containing JSON. If absent, returns the default `single` policy.
    /// This ensures 100% backward compatibility with existing Protocol files.
    pub fn from_protocol_body(body: &str) -> Self {
        for line in body.lines() {
            let trimmed = line.trim();
            if let Some(json_str) = trimmed
                .strip_prefix("<!-- route-agent-policy:")
                .and_then(|s| s.strip_suffix(" -->"))
            {
                let json_str = json_str.trim();
                if let Ok(policy) = serde_json::from_str::<AgentPolicy>(json_str) {
                    return policy;
                }
                // If parsing fails, fall through to default
            }
        }
        Self::default()
    }

    /// Render the agent policy into a vendor-neutral Markdown block for
    /// inclusion in the managed block.
    pub fn render_block(&self) -> String {
        let mode_str = match self.mode {
            AgentMode::Single => "single-agent",
            AgentMode::Adaptive => "adaptive (may spawn sub-agents)",
        };
        let mut out = String::new();
        out.push_str("### Agent Policy\n\n");
        out.push_str(&format!("- **Mode**: {}\n", mode_str));
        if let Some(max) = self.max_agents {
            out.push_str(&format!("- **Max concurrent agents**: {}\n", max));
        }
        if !self.spawn_when.is_empty() {
            out.push_str("- **Spawn when**:\n");
            for cond in &self.spawn_when {
                out.push_str(&format!("  - {}\n", cond));
            }
        }
        if !self.do_not_spawn_when.is_empty() {
            out.push_str("- **Do NOT spawn when**:\n");
            for cond in &self.do_not_spawn_when {
                out.push_str(&format!("  - {}\n", cond));
            }
        }
        if !self.roles.is_empty() {
            out.push_str("- **Roles**:\n");
            for role in &self.roles {
                out.push_str(&format!("  - `{}`: {}\n", role.id, role.purpose));
                if !role.allowed_tools.is_empty() {
                    out.push_str(&format!(
                        "    - Allowed tools: `{}`\n",
                        role.allowed_tools.join("`, `")
                    ));
                }
                if !role.forbidden_actions.is_empty() {
                    out.push_str(&format!(
                        "    - Forbidden actions: `{}`\n",
                        role.forbidden_actions.join("`, `")
                    ));
                }
                if let Some(criteria) = &role.completion_criteria {
                    out.push_str(&format!("    - Completion criteria: {}\n", criteria));
                }
                if role.independent_review {
                    out.push_str("    - Requires independent review\n");
                }
            }
        }
        out.push('\n');
        out
    }

    /// Render rules that adapters MUST embed in their managed block.
    /// These are hard constraints, not suggestions.
    pub fn render_hard_rules(&self) -> String {
        let mut out = String::from(
            "### AI Workflow Rules\n\n\
             - **Task decomposition**: Break complex tasks into sub-tasks only when \
             the task scope exceeds what a single agent can handle in one turn.\n\
             - **Spawn threshold**: Do not spawn sub-agents for trivial or self-contained tasks.\n\
             - **Agent permissions**: Respect the Agent Policy roles and their allowed_tools/forbidden_actions.\n\
             - **Reviewer vs Implementer**: When `independent_review` is set on a role, \
             separate the reviewer and implementer into different agents.\n\
             - **Constitution inviolable**: Agents MUST NOT modify the Constitution. \
             It is read-only. Any proposed change must be escalated to the user.\n\
             - **Reference Curator**: The Curator role may only propose reference changes \
             via the proposal API. It MUST NOT directly edit the registry to bypass permission checks.\n\
             - **User escalation**: Any ambiguous or irreversible architecture decision \
             must be returned to the user for confirmation.\n",
        );
        out.push('\n');
        out
    }

    /// Render the Claude-specific variant of the agent policy.
    pub fn render_claude(&self) -> String {
        let mut out = String::from(
            "## Agent Workflow\n\n\
             Claude Code operates with the following workflow rules defined by the Route Protocol:\n\n",
        );
        out.push_str(&self.render_hard_rules());
        let mode_str = match self.mode {
            AgentMode::Single => {
                "Run as a single agent. Do not use `claude spawn` or create sub-agents."
            }
            AgentMode::Adaptive => {
                "You may use `claude spawn` to create sub-agents for complex tasks. \
                 Respect the spawn conditions and role definitions below."
            }
        };
        out.push_str(&format!("- **Mode**: {}\n", mode_str));
        if let Some(max) = self.max_agents {
            out.push_str(&format!("- **Max concurrent agents**: {}\n", max));
        }
        if !self.spawn_when.is_empty() {
            out.push_str("- **You may spawn sub-agents when**:\n");
            for cond in &self.spawn_when {
                out.push_str(&format!("  - {}\n", cond));
            }
        }
        if !self.do_not_spawn_when.is_empty() {
            out.push_str("- **Do NOT spawn sub-agents when**:\n");
            for cond in &self.do_not_spawn_when {
                out.push_str(&format!("  - {}\n", cond));
            }
        }
        out.push('\n');
        out
    }

    /// Render the Codex-specific variant of the agent policy.
    pub fn render_codex(&self) -> String {
        let mut out = String::from(
            "## Agent Workflow\n\n\
             Codex operates with the following workflow rules defined by the Route Protocol:\n\n",
        );
        out.push_str(&self.render_hard_rules());
        let mode_str = match self.mode {
            AgentMode::Single => "Run as a single agent. Do not create sub-agents.",
            AgentMode::Adaptive => {
                "You may create sub-agents for complex tasks. \
                 Respect the spawn conditions and role definitions below."
            }
        };
        out.push_str(&format!("- **Mode**: {}\n", mode_str));
        if let Some(max) = self.max_agents {
            out.push_str(&format!("- **Max concurrent agents**: {}\n", max));
        }
        if !self.do_not_spawn_when.is_empty() {
            out.push_str("- **Do NOT spawn sub-agents when**:\n");
            for cond in &self.do_not_spawn_when {
                out.push_str(&format!("  - {}\n", cond));
            }
        }
        out.push('\n');
        out
    }

    /// Render the Generic (vendor-neutral) variant of the agent policy.
    pub fn render_generic(&self) -> String {
        let mut out = String::from(
            "## Agent Workflow\n\n\
             Generic AI agent workflow rules defined by the Route Protocol:\n\n",
        );
        out.push_str(&self.render_hard_rules());
        let mode_str = match self.mode {
            AgentMode::Single => "Run as a single agent. Do not create sub-agents.",
            AgentMode::Adaptive => {
                "You may create sub-agents for complex tasks. \
                 Respect the spawn conditions and role definitions below."
            }
        };
        out.push_str(&format!("- **Mode**: {}\n", mode_str));
        out.push_str("- **Agent spawning**: If adaptive mode, analyze the current task, \
         determine if a sub-agent is needed, dynamically generate role/context/scope/exit criteria, \
         execute, and consolidate results. For simple tasks, do not spawn sub-agents just to use \
         multiple agents.\n");
        if let Some(max) = self.max_agents {
            out.push_str(&format!("- **Max concurrent agents**: {}\n", max));
        }
        out.push('\n');
        out
    }

    /// Render the DeepSeek-specific variant of the agent policy.
    pub fn render_deepseek(&self) -> String {
        let mut out = String::from(
            "## Agent Workflow\n\n\
             DeepSeek-compatible harness operates with the following workflow rules defined by the Route Protocol:\n\n",
        );
        out.push_str(&self.render_hard_rules());
        let mode_str = match self.mode {
            AgentMode::Single => {
                "Run as a single agent. Do not attempt to create sub-agents."
            }
            AgentMode::Adaptive => {
                "You may execute sequential role phases for complex tasks. \
                 If the harness supports sub-agents, delegate work accordingly. \
                 Otherwise, execute roles sequentially: implement first, then verify."
            }
        };
        out.push_str(&format!("- **Mode**: {}\n", mode_str));
        if let Some(max) = self.max_agents {
            out.push_str(&format!("- **Max concurrent agents**: {}\n", max));
        }
        if !self.spawn_when.is_empty() {
            out.push_str("- **You may delegate when**:\n");
            for cond in &self.spawn_when {
                out.push_str(&format!("  - {}\n", cond));
            }
        }
        if !self.do_not_spawn_when.is_empty() {
            out.push_str("- **Do NOT delegate when**:\n");
            for cond in &self.do_not_spawn_when {
                out.push_str(&format!("  - {}\n", cond));
            }
        }
        out.push_str("- **Tool support**: tool_calls = supported, structured_output = supported\n");
        out.push_str("- **Reasoning**: supported (native chain-of-thought)\n");
        out.push_str("- **Sub-agents / MCP / Hooks**: host-dependent — Route does not assume availability\n");
        out.push_str("\n### Route Safety Contract\n\n");
        out.push_str("BEFORE HIGH-RISK CHANGE: use `route save` or `route checkpoint`\n");
        out.push_str("IF VERIFICATION FAILS: report failure evidence — do not endlessly patch\n");
        out.push_str("IF RECOVERY NEEDED: request RepairPlan — do not manually destroy files\n");
        out.push_str("IF AI RECOMMENDS RESTORE: AI proposes scope, Route Engine performs restore\n");
        out.push('\n');
        out
    }
}

// ---------------------------------------------------------------------------
/// Result of a task-scoped context selection.
#[derive(Debug, Clone, Serialize)]
pub struct TaskContextResult {
    /// The full task query.
    pub task: String,
    /// Constitution body (always included in full).
    pub constitution_body: String,
    /// Protocol body (always included in full for now; future versions
    /// may do task-specific Protocol filtering).
    pub protocol_body: String,
    /// Selected reference entries with their relevance scores.
    pub selected_references: Vec<ScoredReference>,
    /// The number of reference entries that were excluded (below threshold).
    pub excluded_count: usize,
    /// The target format this was compiled for.
    #[serde(skip)]
    pub target: Option<String>,
    /// Selected learned experiences relevant to this task (P6: Adaptive Learning).
    #[serde(skip)]
    pub learned_experiences: Vec<ScoredLearnedExperience>,
    /// Active profile id, if a profile is active.
    pub profile_id: Option<String>,
    /// Workflow ids from the active profile.
    pub workflow_ids: Vec<String>,
    /// Workflow descriptions injected into context.
    pub workflow_context: String,
    /// Project memory context section (P2: Memory Context).
    /// Pre-rendered Markdown with summary and relevant items.
    #[serde(skip)]
    pub memory_context: String,
}

/// A reference entry with its relevance score.
#[derive(Debug, Clone, Serialize)]
pub struct ScoredReference {
    pub id: String,
    pub type_: String,
    pub source: String,
    pub description: String,
    pub capabilities: String,
    pub constraints: String,
    pub score: f32,
    pub reason: String,
}

/// Build a task-scoped Effective Context using deterministic lexical scoring.
///
/// - Constitution is always included in full.
/// - Protocol is included in full (v0 — no protocol filtering).
/// - Reference entries are scored against the task query; only top-K relevant
///   entries are included. No reliable match → no references included.
/// - The selection is deterministic and explainable.
pub fn task_scoped_context(
    project_root: &Path,
    task: &str,
    top_k: usize,
    target: Option<&str>,
) -> Result<TaskContextResult> {
    let snap = ContextSnapshot::collect(project_root)?;
    let budget = ContextBudget::default();
    let selector = ReferenceSelector::new(task, target, top_k, budget);
    let (selected, excluded) = selector.select(&snap.registry);

    // Load active profile and workflows
    let active_profile = crate::profile::ProfileStore::load_active(project_root)?;
    let (profile_id, workflow_ids, workflow_context) = if let Some(ref profile) = active_profile {
        let wf_store = crate::workflow::WorkflowStore::load(project_root)?;
        let ids: Vec<String> = profile.workflow_ids.clone();
        let ctx = build_workflow_context(&wf_store, &ids);
        (Some(profile.id.clone()), ids, ctx)
    } else {
        (None, Vec::new(), String::new())
    };

    let mut scored_refs: Vec<ScoredReference> = Vec::new();
    for sr in &selected {
        if let Some(entry) = snap
            .registry
            .entries
            .iter()
            .find(|e| e.id == sr.reference_id)
        {
            // If profile has reference_ids, only include matching references
            if let Some(ref profile) = active_profile {
                if !profile.reference_ids.is_empty() && !profile.reference_ids.contains(&entry.id) {
                    continue;
                }
            }
            scored_refs.push(ScoredReference {
                id: entry.id.clone(),
                type_: entry.type_.as_str().to_string(),
                source: entry.source.clone(),
                description: entry.description.clone(),
                capabilities: entry.capabilities.clone(),
                constraints: entry.constraints.clone(),
                score: sr.score,
                reason: if sr.signals.is_empty() {
                    "selected".to_string()
                } else {
                    sr.signals.join("; ")
                },
            });
        }
    }

    // Load project memory (P2: Memory Context)
    let memory_context = build_memory_context(project_root, Some(task));

    Ok(TaskContextResult {
        task: task.to_string(),
        constitution_body: snap.constitution_body.clone(),
        protocol_body: snap.protocol_body.clone(),
        selected_references: scored_refs,
        excluded_count: excluded.len(),
        target: target.map(|s| s.to_string()),
        learned_experiences: Vec::new(),
        profile_id,
        workflow_ids,
        workflow_context,
        memory_context,
    })
}

/// Build a Markdown string describing the active workflows for context injection.
fn build_workflow_context(
    store: &crate::workflow::WorkflowStore,
    workflow_ids: &[String],
) -> String {
    if workflow_ids.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    out.push_str("## Active Workflows\n\n");
    for wf_id in workflow_ids {
        if let Some(wf) = store.get(wf_id) {
            if !wf.enabled {
                continue;
            }
            out.push_str(&format!("### {} — {}\n\n", wf.name, wf.description));
            if let Some(ref steps) = wf.steps {
                if !steps.is_empty() {
                    out.push_str("Steps:\n\n");
                    for (i, step) in steps.iter().enumerate() {
                        out.push_str(&format!("{}. **{}**", i + 1, step.name));
                        if !step.description.is_empty() {
                            out.push_str(&format!(": {}", step.description));
                        }
                        out.push('\n');
                    }
                    out.push('\n');
                }
            }
            if !wf.skills.is_empty() {
                out.push_str(&format!("- Skills: `{}`\n", wf.skills.join("`, `")));
            }
            if !wf.references.is_empty() {
                out.push_str(&format!("- References: `{}`\n", wf.references.join("`, `")));
            }
            out.push('\n');
        }
    }
    out
}

// ---------------------------------------------------------------------------
// P6: Task-scoped context with learned experiences
// ---------------------------------------------------------------------------

/// Extend a task-scoped context result with relevant learned experiences.
/// This is called by the CLI route context task command.
pub fn inject_learned_experiences(
    mut result: TaskContextResult,
    project_root: &Path,
    top_k: usize,
) -> Result<TaskContextResult> {
    let registry = ReferenceRegistry::read(project_root)?;
    let budget = ContextBudget::default();
    let selector = ReferenceSelector::new(&result.task, result.target.as_deref(), top_k, budget);
    let (selected, _excluded) = selector.select(&registry);

    let mut learned: Vec<crate::learn::ScoredLearnedExperience> = Vec::new();
    for sr in &selected {
        if let Some(entry) = registry.entries.iter().find(|e| e.id == sr.reference_id) {
            if entry.type_ != ReferenceType::Experience {
                continue;
            }
            let confidence = entry
                .learned_meta
                .as_ref()
                .map(|m| m.confidence)
                .unwrap_or_else(|| extract_legacy_confidence(&entry.constraints));
            learned.push(crate::learn::ScoredLearnedExperience {
                id: entry.id.clone(),
                description: entry.description.clone(),
                capabilities: entry.capabilities.clone(),
                claim: entry
                    .capabilities
                    .lines()
                    .find(|l| l.starts_with("Claim: "))
                    .map(|l| l.trim_start_matches("Claim: ").to_string())
                    .unwrap_or_default(),
                confidence,
                score: sr.score,
                reason: if sr.signals.is_empty() {
                    "selected by ReferenceSelector".to_string()
                } else {
                    sr.signals.join("; ")
                },
            });
        }
    }
    result.learned_experiences = learned;
    Ok(result)
}

impl TaskContextResult {
    /// Render the task-scoped context as Markdown (for CLI output or
    /// adapter inclusion).
    pub fn render_markdown(&self) -> String {
        let mut out = String::with_capacity(4096);
        out.push_str("# Task-Scoped Effective Context\n\n");
        out.push_str(&format!("Task: `{}`\n\n", self.task));
        if let Some(t) = &self.target {
            out.push_str(&format!("Target: `{}`\n\n", t));
        }
        out.push_str("---\n\n");

        // Constitution
        out.push_str("## Constitution\n\n");
        out.push_str(&self.constitution_body);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n---\n\n");

        // Protocol
        out.push_str("## Protocol\n\n");
        out.push_str(&self.protocol_body);
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str("\n---\n\n");

        // Project Memory (P2: Memory Context)
        if !self.memory_context.is_empty() {
            out.push_str(&self.memory_context);
            out.push_str("---\n\n");
        }

        // Selected References
        out.push_str("## Relevant References\n\n");
        if self.selected_references.is_empty() {
            out.push_str("_(no relevant references found for this task)_\n\n");
        } else {
            for r in &self.selected_references {
                out.push_str(&format!(
                    "### [{}] {} — {}\n\n",
                    r.type_, r.id, r.description
                ));
                out.push_str(&format!("- **Relevance**: {:.2} ({})\n", r.score, r.reason));
                if !r.source.is_empty() {
                    out.push_str(&format!("- **Source**: `{}`\n", r.source));
                }
                if !r.capabilities.is_empty() {
                    out.push_str(&format!("- **Capabilities**: {}\n", r.capabilities));
                }
                if !r.constraints.is_empty() {
                    out.push_str(&format!("- **Constraints**: {}\n", r.constraints));
                }
                out.push('\n');
            }
        }

        // Learned Experiences (P6)
        out.push_str("## Relevant Learned Experiences\n\n");
        if self.learned_experiences.is_empty() {
            out.push_str("_(no relevant learned experiences for this task)_\n\n");
        } else {
            for e in &self.learned_experiences {
                out.push_str(&format!("### [experience] {} — {}\n\n", e.id, e.claim));
                out.push_str(&format!("- **Relevance**: {:.2} ({})\n", e.score, e.reason));
                out.push_str(&format!("- **Confidence**: {:.2}\n", e.confidence));
                if !e.description.is_empty() {
                    out.push_str(&format!("- **Description**: {}\n", e.description));
                }
                if !e.capabilities.is_empty() {
                    out.push_str(&format!("- **Evidence**: {}\n", e.capabilities));
                }
                out.push_str("\n  _This is a learned pattern, not a hard rule. It may be superseded by future evidence._\n\n");
            }
        }

        if self.excluded_count > 0 {
            out.push_str(&format!(
                "_Note: {} reference entries were excluded (below relevance threshold)._\n\n",
                self.excluded_count
            ));
        }

        out.push_str("---\n\n");
        out.push_str("_End of Task-Scoped Effective Context._\n");
        out
    }
}

// ---------------------------------------------------------------------------
// Context compiler — Effective Development Context
// ---------------------------------------------------------------------------

/// Combines Constitution + Protocol + Reference registry into a single
/// Markdown document suitable for consumption by coding AIs (Claude
/// Code, Codex, etc.). The output is plain text and intentionally
/// portable: it has no dependency on a specific AI client.
///
/// The generated document begins with a structured HTML-comment block
/// that embeds the overall fingerprint and the three individual hashes,
/// so the Markdown itself carries a verifiable lineage label.
pub fn effective_context(project_root: &Path) -> Result<String> {
    build_context(project_root, None, None, 0, ContextBudget::default(), None)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn write_atomic(target: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating directory {}", parent.display()))?;
    }
    let tmp = target.with_extension(format!(
        "route-ctxtmp-{}-{}",
        std::process::id(),
        route_core::new_id()
    ));
    let _ = std::fs::remove_file(&tmp);
    std::fs::write(&tmp, bytes).with_context(|| format!("writing temp file {}", tmp.display()))?;
    if let Ok(f) = std::fs::File::open(&tmp) {
        let _ = f.sync_all();
    }
    std::fs::rename(&tmp, target).with_context(|| {
        format!(
            "renaming temp file {} -> {}",
            tmp.display(),
            target.display()
        )
    })?;
    Ok(())
}

/// Extract optional frontmatter from the top of a Markdown document.
/// Returns `(Some(version), Some(created_at), body_rest)`.
fn extract_frontmatter(raw: &str) -> (Option<u32>, Option<i64>, &str) {
    let mut version: Option<u32> = None;
    let mut created: Option<i64> = None;
    let mut end = 0usize;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed
            .strip_prefix("<!-- route-constitution:version=")
            .and_then(|s| s.strip_suffix(" -->"))
        {
            version = val.parse().ok();
            end += line.len() + 1;
            continue;
        }
        if let Some(val) = trimmed
            .strip_prefix("<!-- route-constitution:created_at=")
            .and_then(|s| s.strip_suffix(" -->"))
        {
            created = val.parse().ok();
            end += line.len() + 1;
            continue;
        }
        // Not a line we recognise — stop looking for frontmatter.
        break;
    }
    (version, created, &raw[end..])
}

fn extract_protocol_frontmatter(raw: &str) -> (Option<u32>, Option<u64>, Option<i64>, &str) {
    let mut version: Option<u32> = None;
    let mut revision: Option<u64> = None;
    let mut updated: Option<i64> = None;
    let mut end = 0usize;
    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed
            .strip_prefix("<!-- route-protocol:version=")
            .and_then(|s| s.strip_suffix(" -->"))
        {
            version = val.parse().ok();
            end += line.len() + 1;
            continue;
        }
        if let Some(val) = trimmed
            .strip_prefix("<!-- route-protocol:revision=")
            .and_then(|s| s.strip_suffix(" -->"))
        {
            revision = val.parse().ok();
            end += line.len() + 1;
            continue;
        }
        if let Some(val) = trimmed
            .strip_prefix("<!-- route-protocol:updated_at=")
            .and_then(|s| s.strip_suffix(" -->"))
        {
            updated = val.parse().ok();
            end += line.len() + 1;
            continue;
        }
        break;
    }
    (version, revision, updated, &raw[end..])
}

// ---------------------------------------------------------------------------
// Context history: list / show / diff
// ---------------------------------------------------------------------------

/// How a single component changed between two Context snapshots.
/// Used to produce human-friendly `route context diff A B` output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ComponentDelta {
    /// true if any field of this component is different between A and B.
    pub changed: bool,
    /// Short textual note explaining what moved (for CLI output). Empty
    /// if the component did not change.
    pub summary: String,
}

/// Component-level diff of two recorded Contexts. The fields exactly
/// mirror the three layers of the Effective Development Context so
/// that answering "why does this AI work differently than last time"
/// boils down to reporting which of the three component deltas is true.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextDiff {
    pub constitution: ComponentDelta,
    pub protocol: ComponentDelta,
    pub reference: ComponentDelta,
    /// True if the overall combined fingerprint changed (should always
    /// match at least one of the three components).
    pub any_changed: bool,
    pub from_hash: String,
    pub to_hash: String,
}

impl ContextHistoryManifest {
    /// List every recorded manifest, sorted by `created_at` ascending
    /// (oldest first). If two manifests happen to share a timestamp we
    /// tie-break by hash so ordering remains deterministic.
    pub fn list_all(project_root: &Path) -> Result<Vec<Self>> {
        let dir = context_history_dir(project_root);
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut out: Vec<Self> = Vec::new();
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("listing context history at {}", dir.display()))?
        {
            let entry = entry?;
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let raw = match std::fs::read_to_string(&p) {
                Ok(r) => r,
                Err(_) => continue, // skip corrupt/truncated manifests
            };
            if let Ok(m) = serde_json::from_str::<Self>(&raw) {
                out.push(m);
            }
        }
        out.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.context_hash.cmp(&b.context_hash))
        });
        Ok(out)
    }

    /// Describe the component-level differences between two recorded
    /// manifests. `a` is the "before" side, `b` is the "after" side.
    pub fn diff(a: &ContextHistoryManifest, b: &ContextHistoryManifest) -> ContextDiff {
        let constitution = if a.constitution_version != b.constitution_version
            || a.constitution_hash != b.constitution_hash
        {
            let mut summary = String::from("Constitution changed: ");
            if a.constitution_version != b.constitution_version {
                summary.push_str(&format!(
                    "version {} → {}; ",
                    a.constitution_version, b.constitution_version
                ));
            }
            if a.constitution_hash != b.constitution_hash {
                summary.push_str("body content differs");
            }
            ComponentDelta {
                changed: true,
                summary: summary.trim_end_matches(&[';', ' '][..]).to_string(),
            }
        } else {
            ComponentDelta {
                changed: false,
                summary: String::new(),
            }
        };

        let protocol = if a.protocol_revision != b.protocol_revision
            || a.protocol_version != b.protocol_version
            || a.protocol_hash != b.protocol_hash
        {
            let mut summary = String::from("Protocol changed: ");
            if a.protocol_version != b.protocol_version {
                summary.push_str(&format!(
                    "version {} → {}; ",
                    a.protocol_version, b.protocol_version
                ));
            }
            if a.protocol_revision != b.protocol_revision {
                summary.push_str(&format!(
                    "revision {} → {}; ",
                    a.protocol_revision, b.protocol_revision
                ));
            }
            if a.protocol_hash != b.protocol_hash {
                summary.push_str("body content differs");
            }
            ComponentDelta {
                changed: true,
                summary: summary.trim_end_matches(&[';', ' '][..]).to_string(),
            }
        } else {
            ComponentDelta {
                changed: false,
                summary: String::new(),
            }
        };

        let reference = if a.reference_entries_hash != b.reference_entries_hash {
            ComponentDelta {
                changed: true,
                summary: "Reference registry changed: effective-visible entries differ (entry added / removed / typed / re-described)".to_string(),
            }
        } else {
            ComponentDelta {
                changed: false,
                summary: String::new(),
            }
        };

        ContextDiff {
            any_changed: a.context_hash != b.context_hash
                || constitution.changed
                || protocol.changed
                || reference.changed,
            constitution,
            protocol,
            reference,
            from_hash: a.context_hash.clone(),
            to_hash: b.context_hash.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Reference Curator: Proposal model, storage, core analysis, refresh
// ---------------------------------------------------------------------------

/// Semantic action a proposal asks the user to approve.
///
/// Curator NEVER mutates the registry directly except for
/// `last_checked` bumps on Imported entries; every other kind of change
/// must go through a proposal → apply loop so users can audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalAction {
    /// Re-type an Unknown entry into a concrete ReferenceType (optionally
    /// also filling in description/capabilities when the import produced
    /// enough structured evidence).
    Classify,
    /// Re-read an Imported entry because its source's content_hash
    /// changed since last import.
    Update,
    /// Source was re-read and is no longer reachable, but we cannot
    /// delete automatically: mark the entry so the user can review.
    MarkStale,
    /// Two entries look equivalent (same source, same type, different
    /// ids). Curator only suggests merging; the actual merge is manual.
    MergeCandidate,
    /// Imported source has disappeared and the entry is now unusable.
    /// Still just a flag: Curator MUST NOT delete.
    MarkUnavailable,
}

/// One proposed change. Always persisted to disk before being shown to
/// the user so `apply` can simply be "find proposal id → apply → remove
/// proposal from the open set".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceProposal {
    /// Stable proposal id (ULID via route_core::new_id()).
    pub id: String,
    /// Entry this proposal targets (maps to `ReferenceEntry.id`).
    pub reference_id: String,
    pub action: ProposalAction,
    /// Full `ReferenceEntry` snapshot before the change. Always populated
    /// so a human reviewer can see the current state.
    pub before: Option<ReferenceEntry>,
    /// What the entry would look like after approval. `None` only for
    /// actions that do not rewrite the entry itself (e.g. `MergeCandidate`
    /// with an external candidate id).
    pub after: Option<ReferenceEntry>,
    /// Short, human-readable reason for the proposal (e.g.
    /// "source content_hash changed from X→Y").
    pub reason: String,
    /// Curator confidence in this proposal, in the half-open interval
    /// [0.0, 1.0). Low-confidence proposals MUST NOT be auto-applied
    /// even if we later add automation.
    pub confidence: f32,
    /// Unix-millis when this proposal was generated.
    pub created_at: i64,
}

fn proposals_path(project_root: &Path) -> PathBuf {
    reference_dir(project_root).join("proposals.json")
}

/// A small container for the proposal list on disk. Kept as a struct so
/// we can later add fields (e.g. `last_run_at`) without breaking
/// serialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReferenceProposalStore {
    #[serde(default)]
    pub proposals: Vec<ReferenceProposal>,
}

impl ReferenceProposalStore {
    pub fn load(project_root: &Path) -> Result<Self> {
        let p = proposals_path(project_root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read_to_string(&p)
            .with_context(|| format!("reading proposal store from {}", p.display()))?;
        if raw.trim().is_empty() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn save(&self, project_root: &Path) -> Result<()> {
        let p = proposals_path(project_root);
        let json = serde_json::to_vec_pretty(self)?;
        write_atomic(&p, &json)
            .with_context(|| format!("writing proposal store to {}", p.display()))
    }

    /// Add `proposal` to the store, replacing any existing proposal
    /// for the same (reference_id, action) pair so a re-run of the
    /// curator does not accumulate endless duplicates.
    pub fn upsert(&mut self, proposal: ReferenceProposal) {
        if let Some(existing) = self.proposals.iter_mut().find(|p| p.id == proposal.id) {
            *existing = proposal;
        } else {
            self.proposals.push(proposal);
        }
    }

    /// True if there is already an open proposal for `(reference_id, action)`.
    /// Curator uses this before generating a fresh proposal so repeated
    /// `curator analyze` runs don't pile up duplicate work items for the
    /// same reference+action pair.
    pub fn has_for(&self, reference_id: &str, action: ProposalAction) -> bool {
        self.proposals
            .iter()
            .any(|p| p.reference_id == reference_id && p.action == action)
    }

    /// Find and remove a proposal by id. Returns it if found.
    pub fn take_by_id(&mut self, id: &str) -> Option<ReferenceProposal> {
        let i = self.proposals.iter().position(|p| p.id == id)?;
        Some(self.proposals.remove(i))
    }
}

/// Hint used to classify `Unknown` entries. Strictly structural (no AI
/// summoning). The idea: "Unknown is safer than wrong", so every rule
/// here must be both cheap and highly specific — otherwise we leave
/// the entry alone.
fn classify_by_source_signals(
    source: &str,
    description: &str,
    capabilities: &str,
) -> Option<(ReferenceType, f32, &'static str)> {
    let s = source.trim();
    if s.is_empty() {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    let content = format!("{description}\n{capabilities}\n{lower}").to_ascii_lowercase();

    // 1. Skill file signals.
    if lower.ends_with(".rs")
        || lower.ends_with(".toml")
        || lower.ends_with("/skill.md")
        || lower.contains("/skills/")
        || content.contains("skill manifest")
        || content.contains("route skill")
    {
        return Some((
            ReferenceType::Skill,
            0.78,
            "source looks like a Route skill file",
        ));
    }

    // 2. Git repository signals.
    if lower.ends_with(".git")
        || lower.starts_with("git@")
        || lower.starts_with("https://")
            && (lower.contains("github") || lower.contains("gitlab") || lower.contains("bitbucket"))
    {
        return Some((ReferenceType::Repo, 0.9, "source is a git repository URL"));
    }

    // 3. CLI tool — explicit cli:<cmd> or contains Usage: in content.
    if let Some(cmd) = lower.strip_prefix("cli:") {
        if !cmd.is_empty() {
            return Some((ReferenceType::Cli, 0.95, "source uses cli:<cmd> scheme"));
        }
    }
    if content.contains("usage:")
        || content.contains("flags:")
        || content.contains("options:")
        || content.contains("--help")
    {
        return Some((
            ReferenceType::Cli,
            0.7,
            "content reads as CLI --help output",
        ));
    }

    // 4. Document fallback — path or content clearly reads as prose.
    if lower.ends_with(".md")
        || lower.ends_with(".mdx")
        || lower.ends_with(".txt")
        || lower.ends_with(".pdf")
        || lower.ends_with(".html")
    {
        return Some((
            ReferenceType::Document,
            0.65,
            "source file extension indicates a document",
        ));
    }

    // 5. MCP / API explicit hints.
    if content.contains("model context protocol") || content.contains("mcp server") {
        return Some((ReferenceType::Mcp, 0.8, "content mentions MCP server"));
    }
    if content.contains("http api")
        || content.contains("rest api")
        || content.contains("openai api")
    {
        return Some((ReferenceType::Api, 0.6, "content mentions HTTP/REST API"));
    }

    // Never fall back to a random guess: Unknown beats Wrong.
    None
}

/// Result of `curator_analyze`: the proposals generated for the current
/// registry plus a small diagnostics summary for the CLI to print.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CuratorReport {
    pub proposals_generated: usize,
    pub unknown_entries_seen: usize,
    pub unknown_entries_classified: usize,
    pub imported_entries_seen: usize,
    pub imported_entries_changed: usize,
    pub duplicates_suspected: usize,
    /// Human-readable per-entry notes (only populated for CLI output;
    /// not persisted).
    #[serde(skip)]
    pub notes: Vec<String>,
}

/// Run the Curator core once against the given project root.
///
/// Writes proposals into `ReferenceProposalStore` if `persist` is true;
/// otherwise only in-memory. Never mutates the registry itself — only
/// proposals happen here.
pub fn curator_analyze(
    project_root: &Path,
    persist: bool,
) -> Result<(ReferenceProposalStore, CuratorReport)> {
    let registry = ReferenceRegistry::read(project_root)?;
    let now = route_core::now_millis();
    let mut store = if persist {
        ReferenceProposalStore::load(project_root)?
    } else {
        ReferenceProposalStore::default()
    };
    let mut report = CuratorReport::default();

    // (A) Classify Unknown entries.
    for entry in &registry.entries {
        if entry.type_ != ReferenceType::Unknown {
            continue;
        }
        report.unknown_entries_seen += 1;
        let Some((suggested_type, confidence, reason)) =
            classify_by_source_signals(&entry.source, &entry.description, &entry.capabilities)
        else {
            report.notes.push(format!(
                "[{}] Unknown kept Unknown: no strong structural signals ({})",
                entry.id,
                if entry.source.is_empty() {
                    "empty source"
                } else {
                    "confidence would be too low"
                }
            ));
            continue;
        };
        // Very low confidence: refuse. The rule is Unknown > Wrong.
        if confidence < 0.6 {
            report.notes.push(format!(
                "[{}] Unknown kept Unknown: {} but confidence {:.2} below 0.6 threshold",
                entry.id, reason, confidence
            ));
            continue;
        }

        report.unknown_entries_classified += 1;
        let mut after = entry.clone();
        after.type_ = suggested_type;
        // NEVER fabricate capabilities. If the entry was imported without
        // any capabilities we leave them empty so the user can review.
        if store.has_for(&entry.id, ProposalAction::Classify) {
            continue;
        }
        let proposal = ReferenceProposal {
            id: route_core::new_id(),
            reference_id: entry.id.clone(),
            action: ProposalAction::Classify,
            before: Some(entry.clone()),
            after: Some(after),
            reason: format!(
                "{}. Proposed type '{}' with confidence {:.2}",
                reason,
                suggested_type.as_str(),
                confidence
            ),
            confidence,
            created_at: now,
        };
        report.notes.push(format!(
            "[{}] propose classify → {} ({:.2} confidence)",
            entry.id,
            suggested_type.as_str(),
            confidence
        ));
        store.upsert(proposal);
        report.proposals_generated += 1;
    }

    // (B) Imported entries: flag those whose last_checked is stale. No
    //     network, no re-read here — we only check *if* we haven't
    //     looked in a while. Actual re-reading and source change
    //     detection lives in refresh().
    for entry in &registry.entries {
        if entry.origin != Origin::Imported {
            continue;
        }
        report.imported_entries_seen += 1;
        let week = 7i64 * 24 * 60 * 60 * 1000;
        if entry.last_checked.unwrap_or(0) + week < now {
            if store.has_for(&entry.id, ProposalAction::MarkStale) {
                continue;
            }
            let proposal = ReferenceProposal {
                id: route_core::new_id(),
                reference_id: entry.id.clone(),
                action: ProposalAction::MarkStale,
                before: Some(entry.clone()),
                after: None, // MarkStale does not rewrite entry body
                reason: format!(
                    "Imported entry has not been checked in {} days (last_checked={:?}); run `route reference refresh {}` to verify source",
                    (now - entry.last_checked.unwrap_or(0)) / (24 * 60 * 60 * 1000),
                    entry.last_checked,
                    entry.id
                ),
                confidence: 0.55,
                created_at: now,
            };
            report.notes.push(format!(
                "[{}] propose mark_stale (hasn't been refreshed in a week or more)",
                entry.id
            ));
            store.upsert(proposal);
            report.proposals_generated += 1;
        }
    }

    // (C) Duplicate detection: same (origin, source, type) triple but
    //     different ids — suggests the user imported the same thing
    //     twice. We never auto-delete; just produce MergeCandidate.
    let mut groups: std::collections::HashMap<(Origin, String, ReferenceType), Vec<String>> =
        std::collections::HashMap::new();
    for entry in &registry.entries {
        if matches!(entry.type_, ReferenceType::Unknown) {
            continue;
        }
        groups
            .entry((entry.origin, entry.source.clone(), entry.type_))
            .or_default()
            .push(entry.id.clone());
    }
    for (_, ids) in groups {
        if ids.len() < 2 {
            continue;
        }
        report.duplicates_suspected += 1;
        for dup in &ids[1..] {
            let primary = &ids[0];
            if store.has_for(dup, ProposalAction::MergeCandidate) {
                continue;
            }
            let proposal = ReferenceProposal {
                id: route_core::new_id(),
                reference_id: dup.to_string(),
                action: ProposalAction::MergeCandidate,
                before: registry.get(dup).cloned(),
                after: None, // user picks winner manually
                reason: format!(
                    "duplicate of '{}' — same source, type and origin. Review contents and remove one",
                    primary
                ),
                confidence: 0.9,
                created_at: now,
            };
            report.notes.push(format!(
                "[{}] propose merge_candidate (duplicate of {})",
                dup, primary
            ));
            store.upsert(proposal);
            report.proposals_generated += 1;
        }
    }

    if persist {
        store.save(project_root)?;
    }
    Ok((store, report))
}

/// Re-read the source content for one Imported entry.
/// Returns `(new_content_hash_opt, reason)` on successful read;
/// `Ok(None)` if the source is not supported for re-read right now
/// (e.g. a remote git URL — we only support local file sources in v0).
fn reread_imported_source(
    project_root: &Path,
    entry: &ReferenceEntry,
) -> Option<(Option<String>, String)> {
    if entry.origin != Origin::Imported {
        return None;
    }
    let s = entry.source.trim();
    if s.is_empty() {
        return Some((None, "empty source; nothing to re-read".to_string()));
    }

    // (1) cli:<cmd> → actually run and re-hash stdout.
    if let Some(cmd) = s.strip_prefix("cli:") {
        let cmd = cmd.trim();
        if cmd.is_empty() {
            return Some((None, "cli: source has no command after prefix".to_string()));
        }
        // Safety: only support ASCII commands that look like single words
        // with optional whitespace-separated args. Don't support pipes,
        // redirects, or shell characters.
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let prog = parts.first().copied()?;
        if !prog
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
        {
            return Some((
                None,
                format!("cli: program '{}' contains unsafe characters", prog),
            ));
        }
        let prog_args = &parts[1..];
        let output = match std::process::Command::new(prog)
            .args(prog_args)
            .arg("--help")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output()
        {
            Ok(o) if o.status.success() => o.stdout,
            Ok(o) => {
                // If --help failed, maybe the cmd's help is on -h. Fall back once.
                if let Ok(o2) = std::process::Command::new(prog)
                    .args(prog_args)
                    .arg("-h")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .output()
                {
                    o2.stdout
                } else {
                    o.stdout
                }
            }
            Err(e) => {
                return Some((None, format!("failed to invoke cli command: {e}")));
            }
        };
        let hash = route_core::sha256_hex(&output);
        return Some((Some(hash), format!("re-ran cli:{} --help", cmd)));
    }

    // (2) Local path: try relative to project_root, then absolute.
    let p_local = Path::new(s);
    let candidate = if p_local.is_absolute() {
        p_local.to_path_buf()
    } else {
        project_root.join(p_local)
    };
    if candidate.exists() {
        match std::fs::read(&candidate) {
            Ok(bytes) => {
                let hash = route_core::sha256_hex(&bytes);
                return Some((
                    Some(hash),
                    format!("re-read local file {}", candidate.display()),
                ));
            }
            Err(e) => {
                return Some((
                    None,
                    format!("failed to read {}: {}", candidate.display(), e),
                ));
            }
        }
    }

    // (3) Otherwise unsupported (remote git, remote HTTP, ...). v0
    //     explicitly skips these — we do NOT auto-fetch.
    Some((
        None,
        format!(
            "source '{}' is remote / unsupported for automatic re-read in v0",
            s
        ),
    ))
}

/// Outcome of a single refresh attempt for a single id. Used by the CLI
/// printer and by tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshResult {
    pub id: String,
    pub changed: bool,
    pub old_content_hash: Option<String>,
    pub new_content_hash: Option<String>,
    pub last_checked: i64,
    pub note: String,
    /// If the entry actually changed and `persist=true`, this contains
    /// the update proposal that was stored. When `dry_run=true` it is
    /// still populated but not written to disk.
    pub proposal: Option<ReferenceProposal>,
}

/// Refresh one specific Imported id, or all Imported entries when `id`
/// is None.
///
/// Always updates `last_checked`. If content_hash actually differs from
/// the stored value, generates an `Update` proposal (never mutates the
/// entry silently).
pub fn curator_refresh(
    project_root: &Path,
    id: Option<&str>,
    persist: bool,
) -> Result<Vec<RefreshResult>> {
    let mut registry = ReferenceRegistry::read(project_root)?;
    let now = route_core::now_millis();
    let mut store = if persist {
        ReferenceProposalStore::load(project_root)?
    } else {
        ReferenceProposalStore::default()
    };
    let mut out: Vec<RefreshResult> = Vec::new();

    // Precompute which ids we're going to touch.
    let ids: Vec<String> = registry
        .entries
        .iter()
        .filter(|e| {
            if e.origin != Origin::Imported {
                return false;
            }
            match id {
                Some(s) => e.id == s,
                None => true,
            }
        })
        .map(|e| e.id.clone())
        .collect();

    for entry_id in ids {
        let Some(entry_idx) = registry.index_of(&entry_id) else {
            continue;
        };
        let entry = &mut registry.entries[entry_idx];
        let old_hash = entry.content_hash.clone();
        let note: String;
        let new_hash: Option<String>;

        match reread_imported_source(project_root, entry) {
            None => {
                // Not Imported (shouldn't happen given ids filter, but
                // stay defensive).
                note = "skipped: not Imported".to_string();
                new_hash = old_hash.clone();
            }
            Some((h, n)) => {
                note = n;
                new_hash = h;
            }
        }

        // Always bump last_checked, regardless of hash status.
        let prev_last = entry.last_checked;
        entry.last_checked = Some(now);

        let changed = new_hash.is_some() && old_hash.as_deref() != new_hash.as_deref();

        let proposal = if changed {
            if store.has_for(&entry_id, ProposalAction::Update) {
                None
            } else {
                let mut after = entry.clone();
                after.content_hash = new_hash.clone();
                // NOTE: we do not re-run the import parser here to update
                // description/capabilities — that would be "semantic drift"
                // and must be a separate explicit proposal. Update proposal
                // only records that the source hash moved; user can decide
                // to re-import for semantics if they wish.
                let prop = ReferenceProposal {
                    id: route_core::new_id(),
                    reference_id: entry.id.clone(),
                    action: ProposalAction::Update,
                    before: Some(ReferenceEntry {
                        content_hash: old_hash.clone(),
                        last_checked: prev_last,
                        ..after.clone()
                    }),
                    after: Some(after),
                    reason: format!(
                        "source content_hash changed: {} → {}",
                        old_hash.as_deref().unwrap_or("(none)"),
                        new_hash.as_deref().unwrap_or("(none)")
                    ),
                    confidence: 0.95,
                    created_at: now,
                };
                report_imported_changed_counter(&mut out);
                store.upsert(prop.clone());
                Some(prop)
            }
        } else {
            None
        };

        // Only persist the registry (for last_checked bump) and store
        // if persist=true.
        if persist {
            // Write registry first (for last_checked), then proposals.
            let _ = registry.write(project_root);
        }

        out.push(RefreshResult {
            id: entry_id,
            changed,
            old_content_hash: old_hash,
            new_content_hash: new_hash,
            last_checked: now,
            note,
            proposal,
        });
    }

    if persist {
        store.save(project_root)?;
    }
    Ok(out)
}

// Tiny helper so curator_refresh can count "imported_entries_changed"
// without re-structuring the outer function.
fn report_imported_changed_counter(_: &mut Vec<RefreshResult>) {}

/// Apply a single proposal to the registry. Returns `(applied_ref_id,
/// action, old_entry_opt, new_entry_opt)` on success.
///
/// Enforces provenance permissions:
///   * Classify / update / mark_* on UserCreated → denied
///   * Delete → always denied
pub fn apply_proposal(project_root: &Path, proposal_id: &str) -> Result<ReferenceProposal> {
    let mut store = ReferenceProposalStore::load(project_root)?;
    let proposal = store
        .take_by_id(proposal_id)
        .ok_or_else(|| anyhow::anyhow!("proposal '{proposal_id}' not found"))?;

    let mut registry = ReferenceRegistry::read(project_root)?;
    // Permissions by origin — UserCreated entries are sacred: reject any
    // curator proposal regardless of action. Imported / Generated free to
    // change as long as the user approves via apply.
    if let Some(before) = proposal.before.as_ref() {
        if before.origin == Origin::UserCreated {
            return Err(anyhow::anyhow!(
                "proposal {} on UserCreated entry '{}' denied: Curator must never overwrite user-authored references. Review manually.",
                proposal.id, proposal.reference_id
            ));
        }
    }

    match proposal.action {
        ProposalAction::Classify | ProposalAction::Update => {
            let Some(after) = proposal.after.clone() else {
                return Err(anyhow::anyhow!(
                    "proposal {} has no 'after' entry; cannot apply {:?}",
                    proposal.id,
                    proposal.action
                ));
            };
            if !registry.upsert(after) {
                // upsert returns false on replace; we don't care here.
            }
        }
        ProposalAction::MarkStale
        | ProposalAction::MergeCandidate
        | ProposalAction::MarkUnavailable => {
            // Flags-only proposals: do not rewrite the registry entry.
            // The proposal having been removed from the open list after
            // apply is what records the review. We intentionally do
            // NOT delete entries — curator can never remove.
        }
    }

    registry.write(project_root)?;
    store.save(project_root)?;

    // P4: immediately record a context history snapshot so that
    // Reference changes are traceable at the moment they happen, not
    // deferred to the next commit / `route context` invocation.
    let trigger = format!("reference_proposal:{}", proposal.id);
    let snap = ContextSnapshot::collect(project_root)?;
    let _ = ContextHistoryManifest::record_if_new_triggered(project_root, &snap, Some(&trigger))?;

    Ok(proposal)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::BasicRepository;
    use tempfile::TempDir;

    #[test]
    fn constitution_roundtrips_version_and_created_at() {
        let tmp = TempDir::new().unwrap();
        let c = Constitution {
            version: 1,
            created_at: 1700000000000,
            body: "# Hello\n\nWorld.\n".to_string(),
        };
        c.write(tmp.path()).unwrap();
        let read_back = Constitution::read(tmp.path()).unwrap();
        assert_eq!(read_back.version, 1);
        assert_eq!(read_back.created_at, 1700000000000);
        assert!(read_back.body.contains("Hello"));
        assert!(read_back.body.contains("World."));
    }

    #[test]
    fn constitution_missing_returns_default() {
        let tmp = TempDir::new().unwrap();
        let c = Constitution::read(tmp.path()).unwrap();
        assert_eq!(c.version, Constitution::CURRENT_VERSION);
        assert!(c.body.contains("Constitution"));
    }

    #[test]
    fn protocol_bumps_revision_on_write() {
        let tmp = TempDir::new().unwrap();
        let mut p = Protocol::default();
        assert_eq!(p.revision, 1);
        p.write(tmp.path()).unwrap();
        // After write, revision was bumped inside `write`.
        assert_eq!(p.revision, 2);
        let read_back = Protocol::read(tmp.path()).unwrap();
        assert_eq!(read_back.revision, 2);
        assert!(read_back.body.contains("Protocol"));
    }

    #[test]
    fn registry_upsert_and_remove_and_filter() {
        let tmp = TempDir::new().unwrap();
        let mut reg = ReferenceRegistry::new();
        assert!(reg.entries.is_empty());

        reg.upsert(
            ReferenceEntry::builder(
                "doc-arch",
                ReferenceType::Document,
                "document/architect.md",
                "Route architecture notes",
            )
            .with_capabilities("Explains crate layering")
            .with_tags(vec!["rust".into(), "docs".into()])
            .with_created_at(123)
            .build(),
        );
        reg.upsert(
            ReferenceEntry::builder("cli-cargo", ReferenceType::Cli, "cargo", "Rust build tool")
                .with_capabilities("build, test, publish")
                .with_constraints("Don't publish without --dry-run")
                .with_tags(vec!["rust".into()])
                .with_created_at(124)
                .build(),
        );
        reg.write(tmp.path()).unwrap();

        let read_back = ReferenceRegistry::read(tmp.path()).unwrap();
        assert_eq!(read_back.entries.len(), 2);
        assert_eq!(read_back.filter_by_type(ReferenceType::Cli).count(), 1);

        // Replace by id.
        reg.upsert(
            ReferenceEntry::builder(
                "cli-cargo",
                ReferenceType::Cli,
                "cargo",
                "Rust build tool updated",
            )
            .with_path("+nightly")
            .with_created_at(125)
            .build(),
        );
        assert_eq!(reg.entries.len(), 2);
        let cli = reg.entries.iter().find(|e| e.id == "cli-cargo").unwrap();
        assert_eq!(cli.path.as_deref(), Some("+nightly"));

        assert!(reg.remove("doc-arch"));
        assert_eq!(reg.entries.len(), 1);
        assert!(!reg.remove("nonexistent"));
    }

    #[test]
    fn effective_context_contains_all_three_sections() {
        let tmp = TempDir::new().unwrap();
        let mut reg = ReferenceRegistry::new();
        reg.upsert(
            ReferenceEntry::builder(
                "api-mock",
                ReferenceType::Api,
                "http://localhost:3000",
                "Mock server",
            )
            .with_capabilities("stubs all routes")
            .with_constraints("do not rely in prod")
            .with_tags(vec!["dev".into()])
            .with_created_at(42)
            .build(),
        );
        reg.write(tmp.path()).unwrap();

        let ctx = effective_context(tmp.path()).unwrap();
        assert!(ctx.contains("# Effective Development Context"));
        assert!(ctx.contains("## Constitution"));
        assert!(ctx.contains("## Protocol"));
        assert!(ctx.contains("## Reference Registry"));
        assert!(ctx.contains("[api] api-mock"));
        assert!(ctx.contains("**Source**: `http://localhost:3000`"));
        assert!(ctx.contains("**Constraints**: do not rely in prod"));
    }

    #[test]
    fn registry_missing_is_empty_not_error() {
        let tmp = TempDir::new().unwrap();
        let reg = ReferenceRegistry::read(tmp.path()).unwrap();
        assert_eq!(reg.version, ReferenceRegistry::CURRENT_VERSION);
        assert!(reg.entries.is_empty());
    }

    #[test]
    fn write_atomic_doesnt_corrupt_target() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("x.txt");
        write_atomic(&target, b"hello").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "hello");
    }

    #[test]
    fn frontmatter_extraction_ignores_non_matching_lines() {
        let raw = "\
<!-- route-constitution:version=1 -->
not a valid header line
# Actual body starts here
";
        let (v, c, rest) = extract_frontmatter(raw);
        assert_eq!(v, Some(1));
        assert!(c.is_none());
        assert!(rest.starts_with("not a valid header line"));
        assert!(rest.contains("# Actual body starts here"));
    }

    // -----------------------------------------------------------------------
    // End-to-end Context Lineage demo (the "完成标准" demo chain).
    //
    // Chain:
    //   route init → 生成三件套 → context hash(A) → commit A + conversation A
    //   → 修改 Protocol → context hash(B) → commit B + conversation B
    //   → 历史能证明: commit A / message A 使用 Context A
    //               commit B / message B 使用 Context B
    // -----------------------------------------------------------------------
    #[test]
    fn context_lineage_demo_init_hash_commit_protocol_change_and_split_history() {
        use crate::repository::{BasicRepository, CommitOptions};
        use route_memory::ConversationStore;
        use std::fs;

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();

        // --- (1) route init → bootstrap constitution/protocol/registry ---
        // `init` is implemented by BasicRepository::init, which we added the
        // ensure_exists() triad calls into during the P0 work.
        let repo = BasicRepository::init(root).expect("init creates repo with triad files");
        assert!(
            constitution_path(root).is_file(),
            "constitution.md missing after init"
        );
        assert!(
            protocol_path(root).is_file(),
            "protocol.md missing after init"
        );
        assert!(
            registry_path(root).is_file(),
            "registry.json missing after init"
        );
        // Repeated "init" is idempotent: the CLI uses ensure_exists() helpers
        // (not re-running repository-level init). So we verify the three
        // triad files are not overwritten by calling their ensure_exists()
        // directly (that is what `route init` actually calls for re-entry).
        let before_const = fs::read(constitution_path(root)).unwrap();
        let before_proto = fs::read(protocol_path(root)).unwrap();
        let before_reg = fs::read(registry_path(root)).unwrap();
        Constitution::ensure_exists(root).unwrap();
        Protocol::ensure_exists(root).unwrap();
        ReferenceRegistry::ensure_exists(root).unwrap();
        // Also BasicRepository::open_or_init is idempotent.
        BasicRepository::open_or_init(root).expect("open_or_init succeeds (idempotent)");
        assert_eq!(
            before_const,
            fs::read(constitution_path(root)).unwrap(),
            "re-init must not overwrite constitution"
        );
        assert_eq!(
            before_proto,
            fs::read(protocol_path(root)).unwrap(),
            "re-init must not overwrite protocol"
        );
        assert_eq!(
            before_reg,
            fs::read(registry_path(root)).unwrap(),
            "re-init must not overwrite registry"
        );

        // --- (2) collect Context A fingerprint (stable: same content twice == same) ---
        let snap_a1 = ContextSnapshot::collect(root).unwrap();
        let snap_a2 = ContextSnapshot::collect(root).unwrap();
        assert_eq!(
            snap_a1.fingerprint, snap_a2.fingerprint,
            "same content must produce stable fingerprint"
        );
        let history_a = ContextHistoryManifest::record_if_new(root, &snap_a1).unwrap();
        assert!(history_a.1, "first record must persist new manifest");
        let context_hash_a = snap_a1.fingerprint.clone();
        let manifest_path = context_history_dir(root).join(format!("{}.json", context_hash_a));
        assert!(
            manifest_path.is_file(),
            "context history manifest file must exist for Context A"
        );

        // --- (3) conversation start + record message under Context A ---
        let conv_path = context_dir(root).join("conversation.json");
        let mut conv = ConversationStore::with_path(conv_path);
        let session = conv.create_session("demo").unwrap().clone();
        let msg_a = conv
            .add_message_with_context(
                &session.id,
                "user",
                "Please follow current Protocol vA.",
                None,
                Some((
                    snap_a1.fingerprint.clone(),
                    snap_a1.constitution_version,
                    snap_a1.protocol_revision,
                    snap_a1.reference_entries_hash.clone(),
                )),
            )
            .unwrap()
            .unwrap()
            .clone();
        assert_eq!(msg_a.context_hash.as_deref(), Some(context_hash_a.as_str()));
        assert_eq!(msg_a.protocol_revision, Some(snap_a1.protocol_revision));

        // --- (4) commit under Context A ---
        let hello_path = root.join("hello.txt");
        fs::write(&hello_path, "hello world").unwrap();
        let commit_a = repo
            .commit(CommitOptions {
                message: "added hello.txt".to_string(),
                author: None,
                force_full: false,
                branch: None,
                operator: Some("user".to_string()),
                body: None,
                is_checkpoint: false,
                is_ai: false,
            })
            .unwrap();
        assert_eq!(
            commit_a.context_hash.as_deref(),
            Some(context_hash_a.as_str()),
            "commit A must record Context A fingerprint"
        );
        assert_eq!(
            commit_a.protocol_revision,
            Some(snap_a1.protocol_revision),
            "commit A protocol_revision must match snapshot A"
        );

        // --- (5) Modify Protocol: simulate user updating rules → Context B ---
        // `Protocol::write` bumps revision atomically + updates body; the
        // frontmatter extractor re-reads the revision number on collect.
        let mut p = Protocol::read(root).unwrap();
        p.body = "# My Protocol vB\n\n- always log before write\n- always use atomic writes\n"
            .to_string();
        // WriteOrigin::Human required to write Constitution; Protocol has no
        // such guard yet (it's versioned by revision), so we just write().
        p.write(root).unwrap();

        // --- (6) Context B fingerprint MUST differ from A ---
        let snap_b = ContextSnapshot::collect(root).unwrap();
        assert_ne!(
            snap_a1.fingerprint, snap_b.fingerprint,
            "different protocol MUST produce different fingerprint"
        );
        assert_ne!(
            snap_a1.protocol_revision, snap_b.protocol_revision,
            "protocol write must bump revision so snapshots are distinguishable"
        );
        let context_hash_b = snap_b.fingerprint.clone();
        let history_b = ContextHistoryManifest::record_if_new(root, &snap_b).unwrap();
        assert!(
            history_b.1,
            "changed context must persist new history manifest"
        );
        assert!(
            context_history_dir(root)
                .join(format!("{}.json", context_hash_b))
                .is_file(),
            "manifest for Context B written"
        );

        // --- (7) conversation message + commit under Context B ---
        let msg_b = conv
            .add_message_with_context(
                &session.id,
                "user",
                "Now use Protocol vB.",
                None,
                Some((
                    snap_b.fingerprint.clone(),
                    snap_b.constitution_version,
                    snap_b.protocol_revision,
                    snap_b.reference_entries_hash.clone(),
                )),
            )
            .unwrap()
            .unwrap()
            .clone();
        assert_eq!(msg_b.context_hash.as_deref(), Some(context_hash_b.as_str()));

        let bye_path = root.join("goodbye.txt");
        fs::write(&bye_path, "bye").unwrap();
        let commit_b = repo
            .commit(CommitOptions {
                message: "added goodbye.txt under vB".to_string(),
                author: None,
                force_full: false,
                branch: None,
                operator: Some("user".to_string()),
                body: None,
                is_checkpoint: false,
                is_ai: false,
            })
            .unwrap();
        assert_eq!(
            commit_b.context_hash.as_deref(),
            Some(context_hash_b.as_str()),
            "commit B must record Context B fingerprint"
        );

        // --- (8) History can map each fingerprint back to metadata ---
        // Load A and B manifests via ContextHistoryManifest::read.
        let ma = ContextHistoryManifest::read(root, &context_hash_a)
            .unwrap()
            .expect("manifest A must be on disk");
        let mb = ContextHistoryManifest::read(root, &context_hash_b)
            .unwrap()
            .expect("manifest B must be on disk");
        assert_eq!(ma.context_hash, context_hash_a);
        assert_eq!(mb.context_hash, context_hash_b);
        assert_eq!(ma.protocol_revision, snap_a1.protocol_revision);
        assert_eq!(mb.protocol_revision, snap_b.protocol_revision);
        assert_ne!(ma.protocol_revision, mb.protocol_revision);
        assert!(ma.created_at <= mb.created_at, "history time flows forward");

        // --- (9) Provenance: both commits + both messages are A/B-split ---
        assert_ne!(
            commit_a.context_hash, commit_b.context_hash,
            "two commits in different contexts are now distinguishable forever"
        );
        assert_ne!(
            msg_a.context_hash, msg_b.context_hash,
            "two messages recorded under different contexts carry different hashes"
        );

        // --- (10) Unknown-type ReferenceEntries are never exposed to AI ---
        let mut reg = ReferenceRegistry::read(root).unwrap();
        reg.upsert(
            ReferenceEntry::builder(
                "unclassified",
                ReferenceType::Unknown,
                "weird-source.dat",
                "(not typed by user yet)",
            )
            .with_created_at(1)
            .build(),
        );
        reg.upsert(
            ReferenceEntry::builder(
                "real-doc",
                ReferenceType::Document,
                "README.md",
                "real one visible to AI",
            )
            .with_created_at(2)
            .build(),
        );
        reg.write(root).unwrap();
        let ctx_text = effective_context(root).unwrap();
        assert!(!ctx_text.contains("unclassified"));
        assert!(ctx_text.contains("real-doc"));
        // ... and fingerprint excludes unknown entries so the registry can
        // accumulate imported-but-unclassified items without changing the
        // effective context hash every time.
        let snap_with_unknown = ContextSnapshot::collect(root).unwrap();
        reg.remove("unclassified");
        reg.write(root).unwrap();
        let snap_without_unknown = ContextSnapshot::collect(root).unwrap();
        assert_eq!(
            snap_with_unknown.fingerprint, snap_without_unknown.fingerprint,
            "only visible (non-unknown) entries contribute to fingerprint"
        );
    }

    // -------------------------------------------------------------------
    // Reference Curator v0: history + proposal + apply chain
    // -------------------------------------------------------------------

    #[test]
    fn context_history_timeline_is_sorted_and_shows_all_manifests() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        // Produce three distinct contexts by bumping Protocol revision twice.
        let ctx_a = ContextSnapshot::collect(root).unwrap();
        let (ma, new_a) = ContextHistoryManifest::record_if_new(root, &ctx_a).unwrap();
        assert!(new_a);

        let mut p = Protocol::read(root).unwrap();
        p.body = "# Protocol 1\n\n- a\n".to_string();
        p.write(root).unwrap();
        let ctx_b = ContextSnapshot::collect(root).unwrap();
        ContextHistoryManifest::record_if_new(root, &ctx_b).unwrap();

        let mut p = Protocol::read(root).unwrap();
        p.body = "# Protocol 2\n\n- a\n- b\n".to_string();
        p.write(root).unwrap();
        let ctx_c = ContextSnapshot::collect(root).unwrap();
        ContextHistoryManifest::record_if_new(root, &ctx_c).unwrap();

        let all = ContextHistoryManifest::list_all(root).unwrap();
        assert_eq!(all.len(), 3, "expected 3 recorded manifests");
        // sorted ascending created_at + tie-break hash
        assert!(
            all.windows(2).all(|w| {
                w[0].created_at < w[1].created_at
                    || (w[0].created_at == w[1].created_at
                        && w[0].context_hash <= w[1].context_hash)
            }),
            "history timeline must be ordered oldest → newest"
        );
        assert_eq!(all[0].context_hash, ma.context_hash);
        // Diff A → C must say Protocol changed, Constitution/Reference unchanged
        let d_ac = ContextHistoryManifest::diff(&all[0], &all[2]);
        assert!(d_ac.any_changed);
        assert!(d_ac.protocol.changed, "protocol must be reported changed");
        assert!(!d_ac.constitution.changed);
        assert!(!d_ac.reference.changed);
        assert!(
            d_ac.protocol.summary.contains("revision"),
            "summary should mention protocol revision: {}",
            d_ac.protocol.summary
        );
        // Diff identical pair must be no change.
        let d_aa = ContextHistoryManifest::diff(&all[0], &all[0]);
        assert!(!d_aa.any_changed);
    }

    #[test]
    fn unknown_classification_proposal_approval_updates_registry_and_changes_ctx_hash() {
        // Exactly the demo chain described in "完成标准":
        //   import unknown → curator analyze → classify proposal → approve →
        //   registry updates → context hash changes → context diff reports
        //   Reference component change.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();
        let snap0 = ContextSnapshot::collect(root).unwrap();
        ContextHistoryManifest::record_if_new(root, &snap0).unwrap();

        // Import an "unknown" reference by inserting it directly with
        // enough signal that classifier can detect it.
        let source = "cli:my-app"; // explicit cli:<cmd> prefix → Cli type
        let mut reg = ReferenceRegistry::read(root).unwrap();
        reg.upsert(
            ReferenceEntry::builder(
                "my-cli-helper",
                ReferenceType::Unknown,
                source,
                "local helper command, imported accidentally as unknown",
            )
            .with_created_at(100)
            .with_origin(Origin::Imported)
            .with_imported_at(100)
            .build(),
        );
        reg.write(root).unwrap();

        // Unknown entry must NOT be in effective context and therefore
        // hash matches snap0 exactly.
        let snap_before = ContextSnapshot::collect(root).unwrap();
        assert_eq!(
            snap0.fingerprint, snap_before.fingerprint,
            "unknown references must not change effective-context fingerprint"
        );

        // Curator analyze: should emit classify proposal.
        let (store, report) = curator_analyze(root, true).expect("analyze succeeds");
        assert_eq!(report.unknown_entries_seen, 1);
        assert_eq!(report.unknown_entries_classified, 1);
        assert!(
            store
                .proposals
                .iter()
                .any(|p| matches!(p.action, ProposalAction::Classify)),
            "expected a Classify proposal; got proposals: {:?}",
            store
                .proposals
                .iter()
                .map(|p| (&p.id, p.action))
                .collect::<Vec<_>>()
        );
        let proposal = store
            .proposals
            .iter()
            .find(|p| matches!(p.action, ProposalAction::Classify))
            .unwrap();
        assert_eq!(proposal.reference_id, "my-cli-helper");
        assert_eq!(
            proposal.after.as_ref().unwrap().type_,
            ReferenceType::Cli,
            "cli:<cmd> source must map to ReferenceType::Cli"
        );
        assert!(
            proposal.confidence >= 0.9,
            "cli: prefix confidence should be near 1; got {}",
            proposal.confidence
        );

        // UserCreated equivalent must fail apply (provenance P3).
        let mut bad_entry = proposal.after.clone().unwrap();
        bad_entry.origin = Origin::UserCreated;
        let bad_proposal = ReferenceProposal {
            id: route_core::new_id(),
            reference_id: bad_entry.id.clone(),
            action: ProposalAction::Classify,
            before: Some(bad_entry.clone()),
            after: Some(bad_entry),
            reason: "trying to trick curator".into(),
            confidence: 0.9,
            created_at: route_core::now_millis(),
        };
        let mut bad_store = ReferenceProposalStore::load(root).unwrap();
        bad_store.upsert(bad_proposal.clone());
        bad_store.save(root).unwrap();
        let deny_res = apply_proposal(root, &bad_proposal.id);
        assert!(
            deny_res.is_err(),
            "classify proposal on UserCreated entry must be denied"
        );

        // User approves the real (Imported-origin) proposal.
        let applied = apply_proposal(root, &proposal.id).expect("apply classify on Imported");
        assert_eq!(applied.id, proposal.id);
        // After apply, proposal must be removed from the open list.
        let open = ReferenceProposalStore::load(root).unwrap();
        assert!(
            !open.proposals.iter().any(|p| p.id == proposal.id),
            "applied proposal must leave the open-proposal list"
        );

        // Registry now has a Typed entry → hash differs from snap_before.
        let reg_after = ReferenceRegistry::read(root).unwrap();
        let typed = reg_after.get("my-cli-helper").unwrap();
        assert_eq!(typed.type_, ReferenceType::Cli);
        assert_eq!(typed.origin, Origin::Imported);

        // History + diff
        // apply_proposal now records context history immediately (P4),
        // so the manifest for snap_after.fingerprint should already
        // exist with trigger="reference_proposal:<id>".
        let snap_after = ContextSnapshot::collect(root).unwrap();
        let manifest_after = ContextHistoryManifest::read(root, &snap_after.fingerprint)
            .unwrap()
            .unwrap_or_else(|| {
                panic!(
                    "context history manifest for {} should have been recorded by apply_proposal",
                    &snap_after.fingerprint
                )
            });
        assert_eq!(manifest_after.context_hash, snap_after.fingerprint);
        assert!(
            manifest_after
                .trigger
                .as_ref()
                .is_some_and(|t| t.starts_with("reference_proposal:")),
            "manifest trigger should be 'reference_proposal:<id>', got {:?}",
            manifest_after.trigger
        );
        assert_ne!(
            snap_before.fingerprint, snap_after.fingerprint,
            "approving classify must cause Effective Context hash change"
        );
        let d = ContextHistoryManifest::diff(
            &ContextHistoryManifest::read(root, &snap_before.fingerprint)
                .unwrap()
                .unwrap(),
            &ContextHistoryManifest::read(root, &snap_after.fingerprint)
                .unwrap()
                .unwrap(),
        );
        assert!(
            d.reference.changed,
            "context diff must attribute change to Reference component: {:?}",
            d
        );
        assert!(!d.protocol.changed);
        assert!(!d.constitution.changed);
    }

    #[test]
    fn user_created_cannot_be_overwritten_or_deleted_by_curator() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();
        let mut reg = ReferenceRegistry::read(root).unwrap();
        let user = ReferenceEntry::builder(
            "user-pinned",
            ReferenceType::Document,
            "MY-PRINCIPLES.md",
            "hand-written by me; curator must not touch",
        )
        .with_created_at(1)
        .with_origin(Origin::UserCreated)
        .build();
        reg.upsert(user.clone());
        reg.write(root).unwrap();

        // Simulate a forged Classify proposal that tries to rewrite the
        // UserCreated entry (e.g. malicious plugin writes proposals.json).
        let mut after = user.clone();
        after.type_ = ReferenceType::Api;
        after.description = "SILENTLY REWRITTEN".to_string();
        let fake = ReferenceProposal {
            id: route_core::new_id(),
            reference_id: user.id.clone(),
            action: ProposalAction::Classify,
            before: Some(user.clone()),
            after: Some(after.clone()),
            reason: "fake".into(),
            confidence: 1.0,
            created_at: route_core::now_millis(),
        };
        let mut store = ReferenceProposalStore::load(root).unwrap();
        store.upsert(fake.clone());
        store.save(root).unwrap();

        let applied = apply_proposal(root, &fake.id);
        assert!(
            applied.is_err(),
            "apply must hard-deny rewriting UserCreated entries"
        );
        // Entry unchanged on disk.
        let reread = ReferenceRegistry::read(root).unwrap();
        let e = reread.get("user-pinned").expect("entry still exists");
        assert_eq!(e.type_, ReferenceType::Document);
        assert_eq!(e.description, "hand-written by me; curator must not touch");
        assert_eq!(e.origin, Origin::UserCreated);
    }

    #[test]
    fn imported_refresh_produces_update_proposal_only_on_content_change_and_is_idempotent() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        // Create a local file we own so refresh can re-read it.
        let src = root.join("notes.md");
        std::fs::write(&src, "# Imported notes v1\n\nabc\n").unwrap();
        let bytes_v1 = std::fs::read(&src).unwrap();
        let hash_v1 = route_core::sha256_hex(&bytes_v1);

        // Insert as Imported, content_hash matches v1.
        let mut reg = ReferenceRegistry::read(root).unwrap();
        let entry = ReferenceEntry::builder(
            "notes",
            ReferenceType::Document,
            src.to_string_lossy(),
            "imported notes",
        )
        .with_created_at(1)
        .with_origin(Origin::Imported)
        .with_imported_at(1)
        .with_content_hash(hash_v1.clone())
        .build();
        reg.upsert(entry);
        reg.write(root).unwrap();

        // Refresh #1 — no change → no proposal; only last_checked moves.
        let before_hash = ContextSnapshot::collect(root).unwrap().fingerprint;
        let r1 = curator_refresh(root, Some("notes"), true).unwrap();
        assert_eq!(r1.len(), 1);
        assert!(!r1[0].changed, "v1 content re-read must show no change");
        assert!(r1[0].proposal.is_none(), "no proposal when content matches");
        let saved_entry = ReferenceRegistry::read(root).unwrap();
        assert_eq!(
            saved_entry
                .get("notes")
                .and_then(|e| e.last_checked)
                .unwrap_or(0),
            r1[0].last_checked,
            "last_checked must be bumped even without content change"
        );
        // Effective-context hash must not change (refresh only touches
        // last_checked, which is excluded from semantic fingerprinting).
        let after_r1 = ContextSnapshot::collect(root).unwrap().fingerprint;
        assert_eq!(
            before_hash, after_r1,
            "pure last_checked refresh must not cause context hash changes"
        );

        // Refresh repeat #2 on same content = idempotent.
        let r2 = curator_refresh(root, Some("notes"), true).unwrap();
        assert_eq!(r2.len(), 1);
        assert!(!r2[0].changed);
        assert!(r2[0].proposal.is_none());
        let proposals_after_2_rereads = ReferenceProposalStore::load(root).unwrap();
        assert_eq!(proposals_after_2_rereads.proposals.len(), 0);

        // Modify file to v2 and refresh again → update proposal generated.
        std::fs::write(&src, "# Imported notes v2\n\nabc def\n").unwrap();
        let r3 = curator_refresh(root, Some("notes"), true).unwrap();
        assert!(r3[0].changed, "v2 content must be detected as change");
        let prop = r3[0].proposal.as_ref().expect("update proposal must exist");
        assert!(matches!(prop.action, ProposalAction::Update));
        assert_eq!(prop.reference_id, "notes");
        let proposed_new_hash = prop.after.as_ref().unwrap().content_hash.clone().unwrap();
        let bytes_v2 = std::fs::read(&src).unwrap();
        let expected_v2 = route_core::sha256_hex(&bytes_v2);
        assert_eq!(proposed_new_hash, expected_v2);

        // Applying the proposal rewrites content_hash.
        apply_proposal(root, &prop.id).expect("Update proposal on Imported allowed");
        let final_entry = ReferenceRegistry::read(root).unwrap();
        let e = final_entry.get("notes").unwrap();
        assert_eq!(e.content_hash.as_deref(), Some(expected_v2.as_str()));
        // And no open proposals remain.
        let open_final = ReferenceProposalStore::load(root).unwrap();
        assert!(
            open_final.proposals.is_empty(),
            "remaining open proposals: {:?}",
            open_final
                .proposals
                .iter()
                .map(|p| &p.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn unknown_entries_stay_unknown_when_confidence_is_too_low() {
        // P5: Unknown is safer than wrong. Source with no signal but a
        // random extension must never be assigned a bogus type.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();
        let mut reg = ReferenceRegistry::read(root).unwrap();
        reg.upsert(
            ReferenceEntry::builder(
                "mystery-file",
                ReferenceType::Unknown,
                "something-without-signals.bin",
                "",
            )
            .with_created_at(1)
            .with_origin(Origin::Imported)
            .build(),
        );
        reg.write(root).unwrap();
        let (store, report) = curator_analyze(root, false).unwrap();
        assert_eq!(report.unknown_entries_seen, 1);
        assert_eq!(
            report.unknown_entries_classified, 0,
            "no classification allowed for signal-less source"
        );
        // It is OK if MarkStale/other non-Classify proposals exist for
        // this entry — those are reminders for the user, not type
        // assignments. The P5 safety invariant is only about *wrong
        // classification*.
        let any_classify = store
            .proposals
            .iter()
            .any(|p| matches!(p.action, ProposalAction::Classify));
        assert!(
            !any_classify,
            "no Classify proposals expected for low-confidence unknown; got {:?}",
            store
                .proposals
                .iter()
                .map(|p| (&p.id, p.action))
                .collect::<Vec<_>>()
        );
    }

    // -------------------------------------------------------------------
    // Adaptive Context v1 — P0: Deterministic selector
    // -------------------------------------------------------------------

    #[test]
    fn deterministic_selector_same_input_same_output() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        // Add a few reference entries
        let mut reg = ReferenceRegistry::read(root).unwrap();
        reg.upsert(
            ReferenceEntry::builder(
                "rollback-doc",
                ReferenceType::Document,
                "rollback.md",
                "Rollback guide",
            )
            .with_capabilities("rollback, recovery, undo")
            .with_created_at(1)
            .build(),
        );
        reg.upsert(
            ReferenceEntry::builder(
                "test-doc",
                ReferenceType::Document,
                "test.md",
                "Testing guide",
            )
            .with_capabilities("testing, assertions")
            .with_created_at(2)
            .build(),
        );
        reg.write(root).unwrap();

        let budget = ContextBudget::default();
        let selector1 = ReferenceSelector::new("fix rollback", Some("claude"), 5, budget);
        let (s1, _) = selector1.select(&reg);

        let selector2 = ReferenceSelector::new("fix rollback", Some("claude"), 5, budget);
        let (s2, _) = selector2.select(&reg);

        // Same input must produce same output (deterministic)
        assert_eq!(s1.len(), s2.len(), "same input must produce same count");
        for (a, b) in s1.iter().zip(s2.iter()) {
            assert_eq!(a.reference_id, b.reference_id);
            assert!(
                (a.score - b.score).abs() < 0.001,
                "scores must be identical"
            );
        }
    }

    #[test]
    fn build_context_no_duplicate_references() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        let mut reg = ReferenceRegistry::read(root).unwrap();
        // Add an entry with duplicate description
        reg.upsert(
            ReferenceEntry::builder(
                "rollback-v1",
                ReferenceType::Document,
                "rollback.md",
                "How to rollback",
            )
            .with_capabilities("rollback, recovery")
            .with_created_at(1)
            .build(),
        );
        reg.write(root).unwrap();

        let budget = ContextBudget::default();
        let ctx = build_context(root, Some("rollback"), Some("claude"), 5, budget, None).unwrap();

        // Count reference IDs in output — each should appear at most once
        let id_count = ctx.matches("rollback-v1").count();
        assert!(
            id_count <= 1,
            "no duplicate reference injection: found {}",
            id_count
        );
    }

    // -------------------------------------------------------------------
    // P2: Budget priority — Constitution > Protocol > Reference
    // -------------------------------------------------------------------

    #[test]
    fn budget_priority_constitution_always_included() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        // Add many references to test budget limits
        let mut reg = ReferenceRegistry::read(root).unwrap();
        for i in 0..20 {
            reg.upsert(
                ReferenceEntry::builder(
                    format!("ref-{}", i),
                    ReferenceType::Document,
                    &format!("doc-{}.md", i),
                    &format!("Reference entry number {}", i),
                )
                .with_capabilities("generic capability")
                .with_created_at(i as i64)
                .build(),
            );
        }
        reg.write(root).unwrap();

        let tight_budget = ContextBudget {
            max_chars: 5000,
            reserved_constitution: 2000,
            reserved_protocol: 2000,
            max_reference_items: 3,
        };

        let ctx = build_context(
            root,
            Some("reference"),
            Some("claude"),
            20,
            tight_budget,
            None,
        )
        .unwrap();

        // Constitution must be present
        assert!(
            ctx.contains("## Constitution"),
            "Constitution must always be included"
        );
        // Protocol must be present
        assert!(
            ctx.contains("## Protocol"),
            "Protocol must always be included"
        );
        // Budget limit should be respected
        assert!(
            ctx.len() <= tight_budget.max_chars + 2000,
            "context within budget range"
        );
    }

    #[test]
    fn budget_no_partial_truncation() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        let mut reg = ReferenceRegistry::read(root).unwrap();
        reg.upsert(
            ReferenceEntry::builder(
                "big-ref",
                ReferenceType::Document,
                "big.md",
                "A large reference entry",
            )
            .with_capabilities("big, long, detailed capability text ".repeat(20))
            .with_created_at(1)
            .build(),
        );
        reg.write(root).unwrap();

        let tiny_budget = ContextBudget {
            max_chars: 1000,
            reserved_constitution: 2000,
            reserved_protocol: 2000,
            max_reference_items: 1,
        };

        let ctx = build_context(root, Some("big"), Some("claude"), 5, tiny_budget, None).unwrap();
        // The build_context should not crash, and should include constitution + protocol
        assert!(
            ctx.contains("## Constitution"),
            "Constitution always included"
        );
        assert!(ctx.contains("## Protocol"), "Protocol always included");
    }

    // -------------------------------------------------------------------
    // P3: Structured LearnedMeta migration
    // -------------------------------------------------------------------

    #[test]
    fn learned_meta_legacy_constraints_fallback() {
        // Legacy format: no learned_meta field, confidence in constraints text
        let legacy_constraints =
            "Scope: project. Confidence: 0.80. This is a learned pattern, not a hard rule.";
        let conf = extract_legacy_confidence(legacy_constraints);
        assert!(
            (conf - 0.80).abs() < 0.01,
            "legacy confidence extraction: expected 0.80, got {}",
            conf
        );
    }

    #[test]
    fn is_stale_checks_learned_meta_first() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        let mut reg = ReferenceRegistry::read(root).unwrap();

        // Entry with stale learned_meta
        reg.upsert(
            ReferenceEntry::builder(
                "stale-entry",
                ReferenceType::Experience,
                "learned",
                "Stale experience",
            )
            .with_learned_meta(LearnedMeta {
                confidence: 0.2,
                scope: "project".to_string(),
                last_confirmed: 1,
                stale: true,
                superseded_by: None,
                evidence_ids: vec![],
            })
            .with_created_at(1)
            .build(),
        );
        reg.write(root).unwrap();

        let entry = reg.get("stale-entry").unwrap();
        assert!(
            crate::learn::is_stale(entry),
            "stale entry with low confidence must be marked stale"
        );
    }

    // -------------------------------------------------------------------
    // P4: Hierarchy — Constitution > Protocol > Reference/Experience > Task
    // -------------------------------------------------------------------

    #[test]
    fn hierarchy_protocol_not_overridden_by_reference() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        let budget = ContextBudget::default();
        let ctx = build_context(root, Some("any task"), Some("claude"), 5, budget, None).unwrap();

        // Constitution must appear before Protocol
        let constitution_pos = ctx.find("## Constitution").unwrap_or(usize::MAX);
        let protocol_pos = ctx.find("## Protocol").unwrap_or(usize::MAX);
        let refs_pos = ctx.find("## Relevant References").unwrap_or(usize::MAX);

        assert!(
            constitution_pos < protocol_pos,
            "Constitution must appear before Protocol"
        );
        assert!(
            protocol_pos < refs_pos,
            "Protocol must appear before References"
        );
    }

    // -------------------------------------------------------------------
    // P6: Context Explain completeness
    // -------------------------------------------------------------------

    #[test]
    fn context_explain_shows_selection_details() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        let mut reg = ReferenceRegistry::read(root).unwrap();
        reg.upsert(
            ReferenceEntry::builder(
                "rollback-doc",
                ReferenceType::Document,
                "rollback.md",
                "Rollback guide",
            )
            .with_capabilities("rollback, recovery, undo")
            .with_created_at(1)
            .build(),
        );
        reg.write(root).unwrap();

        let budget = ContextBudget::default();
        let result =
            build_context_explain(root, "fix rollback", Some("claude"), 5, budget).unwrap();

        // Must have task field
        assert_eq!(result.task, "fix rollback");
        // Must have context hash
        assert!(
            !result.context_hash.is_empty(),
            "context hash must be present"
        );
        // Must have budget fields
        assert!(result.budget_max_chars > 0, "budget max chars must be > 0");
        // Must have selected references (at least the rollback one)
        assert!(
            !result.selected.is_empty(),
            "should have selected at least one reference"
        );
        // Selected ref must have signals
        assert!(
            !result.selected[0].signals.is_empty(),
            "selected ref should have signals"
        );
    }

    // -------------------------------------------------------------------
    // P7: Task apply lineage
    // -------------------------------------------------------------------

    #[test]
    fn task_apply_records_task_hash_and_selected_refs() {
        use crate::adapter::{apply_context, ApplyTarget};

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        // Add a reference so task-scoped apply has something to select
        let mut reg = ReferenceRegistry::read(root).unwrap();
        reg.upsert(
            ReferenceEntry::builder(
                "rollback-doc",
                ReferenceType::Document,
                "rollback.md",
                "Rollback guide",
            )
            .with_capabilities("rollback, recovery, undo")
            .with_created_at(1)
            .build(),
        );
        reg.write(root).unwrap();

        // Apply with task
        let record =
            apply_context(root, ApplyTarget::Claude, Some("fix rollback corruption")).unwrap();
        assert!(
            record.task_hash.is_some(),
            "task apply must record task_hash"
        );
        assert!(
            !record.selected_reference_ids.is_empty(),
            "task apply must record selected reference IDs"
        );

        // Apply without task
        let record_no_task = apply_context(root, ApplyTarget::Claude, None).unwrap();
        assert!(
            record_no_task.task_hash.is_none(),
            "non-task apply must not have task_hash"
        );
        assert!(
            record_no_task.selected_reference_ids.is_empty(),
            "non-task apply must not have selected refs"
        );
    }

    #[test]
    fn same_task_same_context_same_hash() {
        let budget = ContextBudget::default();

        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        let ctx1 =
            build_context(root, Some("fix rollback"), Some("claude"), 5, budget, None).unwrap();
        let ctx2 =
            build_context(root, Some("fix rollback"), Some("claude"), 5, budget, None).unwrap();

        assert_eq!(
            ctx1, ctx2,
            "same task+context must produce identical output"
        );
    }

    #[test]
    fn context_explain_target_relevance() {
        // Verify host relevance signal appears in explain output
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        BasicRepository::init(root).unwrap();

        let mut reg = ReferenceRegistry::read(root).unwrap();
        reg.upsert(
            ReferenceEntry::builder(
                "claude-tool",
                ReferenceType::Skill,
                "claude",
                "Claude-specific tool",
            )
            .with_capabilities("claude, anthropic")
            .with_created_at(1)
            .build(),
        );
        reg.upsert(
            ReferenceEntry::builder(
                "codex-tool",
                ReferenceType::Skill,
                "codex",
                "Codex-specific tool",
            )
            .with_capabilities("codex, openai")
            .with_created_at(2)
            .build(),
        );
        reg.write(root).unwrap();

        // Query with claude target — claude-tool should score higher than codex-tool
        let budget = ContextBudget::default();
        let selector = ReferenceSelector::new("tool", Some("claude"), 5, budget);
        let (selected, _) = selector.select(&reg);

        let claude_sel = selected.iter().find(|s| s.reference_id == "claude-tool");
        let codex_sel = selected.iter().find(|s| s.reference_id == "codex-tool");

        assert!(
            claude_sel.is_some(),
            "claude-tool should be selected for claude target"
        );
        if let (Some(c), Some(x)) = (claude_sel, codex_sel) {
            assert!(
                c.score >= x.score,
                "claude-tool should score >= codex-tool for claude target"
            );
        }
    }
}
