//! Capability Model — unified view over Reference entries.
//!
//! Each Capability wraps a Reference entry with a standardized kind,
//! maturity level, and usage metadata. Route does not reimplement tools;
//! capabilities describe what the host AI can discover and use.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::{ReferenceEntry, ReferenceRegistry, ROUTE_DOT_DIR};
use crate::discovery::DiscoveryProposal;

/// Capability directory under `.route/`.
pub fn capability_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("capability")
}

/// Path to the capability registry file.
pub fn capability_registry_path(project_root: &Path) -> PathBuf {
    capability_dir(project_root).join("registry.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// The kind of capability a Reference entry provides.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    /// Declarative knowledge about a domain (docs, specs, decisions)
    Knowledge,
    /// How-to instructions for a specific task
    Instruction,
    /// A concrete tool/command/script
    Tool,
    /// A reusable AI skill (like a prompt template or agent capability)
    Skill,
    /// A development workflow (CI/CD, review, release)
    Workflow,
    /// A network service / API endpoint
    Service,
    /// A runtime environment (language, framework, platform)
    Runtime,
}

impl Default for CapabilityKind {
    fn default() -> Self {
        Self::Knowledge
    }
}

/// Maturity level of a capability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum CapabilityLevel {
    /// Referenceable — the entry exists and can be cited
    L1,
    /// Understandable — AI knows how to use it
    L2,
    /// Executable — the host environment can actually invoke it
    L3,
}

impl Default for CapabilityLevel {
    fn default() -> Self {
        Self::L1
    }
}

/// A unified capability view wrapping a Reference entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Unique capability ID (usually matches the reference entry id)
    pub id: String,
    /// The reference entry this capability wraps
    pub reference_id: String,
    /// Human-readable name
    pub name: String,
    /// Kind of capability
    pub kind: CapabilityKind,
    /// Maturity level
    pub level: CapabilityLevel,
    /// Optional entrypoint — how to invoke/use this (command, path, URL)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    /// Usage instructions / how to call this capability
    #[serde(default)]
    pub usage: String,
    /// Expected inputs (file paths, arguments, environment)
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Expected outputs (results, files, side effects)
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Required permissions to use this capability
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Constraints on usage
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Whether this capability is currently available
    #[serde(default = "default_availability")]
    pub availability: bool,
}

fn default_availability() -> bool {
    true
}

/// The capability registry — a collection of capabilities mapped from references.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CapabilityRegistry {
    pub capabilities: Vec<Capability>,
}

impl CapabilityRegistry {
    /// Load the capability registry from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = capability_registry_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save the capability registry to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = capability_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(capability_registry_path(project_root), json)?;
        Ok(())
    }

    /// List all capabilities, optionally filtered by kind.
    pub fn list(&self, kind: Option<&str>) -> Vec<&Capability> {
        self.capabilities
            .iter()
            .filter(|c| {
                kind.map_or(true, |k| {
                    format!("{:?}", c.kind).to_lowercase() == k.to_lowercase()
                })
            })
            .collect()
    }

    /// Get a capability by ID.
    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.capabilities.iter().find(|c| c.id == id)
    }

    /// Add or update a capability.
    pub fn set(&mut self, cap: Capability) {
        if let Some(existing) = self.capabilities.iter_mut().find(|c| c.id == cap.id) {
            *existing = cap;
        } else {
            self.capabilities.push(cap);
        }
    }

    /// Remove a capability by ID.
    pub fn remove(&mut self, id: &str) {
        self.capabilities.retain(|c| c.id != id);
    }

    /// In (scan / auto-discover) capabilities from a `ReferenceRegistry`.
    ///
    /// Maps each entry to a capability based on its type:
    /// - `document` → Knowledge
    /// - `skill` → Skill
    /// - `cli` / `executable` → Tool (L3 if available, L2 otherwise)
    /// - `mcp` → Service
    /// - `workflow` → Workflow
    /// - `repo` / `api` → Knowledge
    ///
    /// Existing capabilities are not overwritten unless `overwrite` is true.
    pub fn discover_from_registry(
        &mut self,
        registry: &ReferenceRegistry,
        overwrite: bool,
    ) -> Result<Vec<String>> {
        let mut discovered = Vec::new();

        for entry in &registry.entries {
            if !overwrite && self.capabilities.iter().any(|c| c.reference_id == entry.id) {
                continue;
            }

            let (kind, level) = Self::infer_from_entry(entry);
            let cap = Capability {
                id: format!("cap-{}", entry.id),
                reference_id: entry.id.clone(),
                name: entry.name.clone(),
                kind,
                level,
                entrypoint: entry.entrypoint.clone(),
                usage: entry.description.clone(),
                inputs: Vec::new(),
                outputs: Vec::new(),
                permissions: Vec::new(),
                constraints: if entry.constraints.is_empty() {
                    Vec::new()
                } else {
                    vec![entry.constraints.clone()]
                },
                availability: entry.enabled,
            };

            self.set(cap);
            discovered.push(entry.id.clone());
        }

        Ok(discovered)
    }

    fn infer_from_entry(entry: &ReferenceEntry) -> (CapabilityKind, CapabilityLevel) {
        use crate::constitutive::ReferenceType;
        match entry.type_ {
            ReferenceType::Document => (CapabilityKind::Knowledge, CapabilityLevel::L1),
            ReferenceType::Skill => (CapabilityKind::Skill, CapabilityLevel::L2),
            ReferenceType::Cli | ReferenceType::Executable => {
                (CapabilityKind::Tool, CapabilityLevel::L3)
            }
            ReferenceType::Mcp => (CapabilityKind::Service, CapabilityLevel::L3),
            ReferenceType::Workflow => (CapabilityKind::Workflow, CapabilityLevel::L2),
            ReferenceType::Repo => (CapabilityKind::Knowledge, CapabilityLevel::L1),
            ReferenceType::Api => (CapabilityKind::Service, CapabilityLevel::L2),
            _ => (CapabilityKind::Knowledge, CapabilityLevel::L1),
        }
    }

    /// The `.route/skills/` directory where Skill-kind capabilities are promoted.
    pub fn skills_dir(project_root: &Path) -> PathBuf {
        project_root.join(ROUTE_DOT_DIR).join("skills")
    }

    /// Promote Skill-kind capabilities to reusable `.route/skills/*.md` files.
    ///
    /// This bridges the L2 (understandable) capability view into the L3
    /// (executable, host-injectable) skill surface. It is idempotent: existing
    /// files are left untouched unless `force` is true. Only capabilities whose
    /// [`CapabilityKind`] is `Skill` are promoted; everything else is skipped
    /// without error. A `None` id means "all Skill capabilities".
    pub fn promote_skills(
        &self,
        project_root: &Path,
        id: Option<&str>,
        force: bool,
    ) -> Result<PromoteSkillReport> {
        let dir = Self::skills_dir(project_root);
        let mut caps: Vec<&Capability> = self
            .capabilities
            .iter()
            .filter(|c| c.kind == CapabilityKind::Skill)
            .filter(|c| id.map_or(true, |want| c.id == want))
            .collect();
        caps.sort_by(|a, b| a.name.cmp(&b.name));
        let mut report = PromoteSkillReport::default();
        for cap in caps {
            let fname = safe_skill_filename(&cap.name);
            let path = dir.join(format!("{fname}.md"));
            if path.exists() && !force {
                report.skipped_existing.push(fname);
                continue;
            }
            std::fs::create_dir_all(&dir)?;
            std::fs::write(&path, render_skill_markdown(cap))?;
            report.promoted.push(fname);
        }
        Ok(report)
    }

    /// Integrate a [`DiscoveryProposal`] into the capability registry, then
    /// promote the resulting Skill capabilities to `.route/skills/`.
    ///
    /// Only items whose `suggested_kind` maps to a known [`CapabilityKind`]
    /// are registered; items already present in the registry are left
    /// untouched (no silent overwrite). Skill files written by
    /// [`Self::promote_skills`] never clobber existing ones.
    pub fn integrate_discovery(
        &mut self,
        project_root: &Path,
        proposal: &DiscoveryProposal,
    ) -> Result<IntegrateReport> {
        let mut report = IntegrateReport::default();
        let mut changed = false;
        for item in &proposal.items {
            let Some(kind) = capability_kind_from_str(&item.suggested_kind) else {
                continue;
            };
            let cap_id = format!("cap-{}", item.name);
            if self.capabilities.iter().any(|c| c.id == cap_id) {
                continue;
            }
            self.capabilities.push(Capability {
                id: cap_id,
                reference_id: item.name.clone(),
                name: item.name.clone(),
                kind: kind.clone(),
                level: level_for_kind(&kind),
                entrypoint: Some(item.path.clone()),
                usage: item.description.clone(),
                inputs: Vec::new(),
                outputs: Vec::new(),
                permissions: Vec::new(),
                constraints: vec![item.evidence.clone()],
                availability: true,
            });
            report.registered.push(item.name.clone());
            changed = true;
        }
        if changed {
            self.save(project_root)?;
        }
        let pr = self.promote_skills(project_root, None, false)?;
        report.promoted = pr.promoted;
        report.skipped_existing = pr.skipped_existing;
        Ok(report)
    }

    /// Inspect a capability — return a detailed text description.
    pub fn inspect(&self, id: &str) -> Result<String> {
        let cap = self
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Capability '{}' not found", id))?;

        Ok(format!(
            r#"Capability: {name} ({id})
  Reference:  {ref_id}
  Kind:       {kind:?}
  Level:      {level:?}
  Entrypoint: {entry}
  Usage:      {usage}
  Inputs:     {inputs}
  Outputs:    {outputs}
  Permissions: {perms}
  Constraints: {constraints}
  Available:  {avail}"#,
            name = cap.name,
            id = cap.id,
            ref_id = cap.reference_id,
            kind = cap.kind,
            level = cap.level,
            entry = cap.entrypoint.as_deref().unwrap_or("(none)"),
            usage = cap.usage,
            inputs = cap.inputs.join(", "),
            outputs = cap.outputs.join(", "),
            perms = cap.permissions.join(", "),
            constraints = cap.constraints.join(", "),
            avail = if cap.availability { "yes" } else { "no" },
        ))
    }
}

/// Outcome of a promote-skill run: which skills were written, which were kept.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PromoteSkillReport {
    /// Skill filenames written (or force-overwritten) to `.route/skills/`.
    pub promoted: Vec<String>,
    /// Skill filenames that already existed and were left untouched.
    pub skipped_existing: Vec<String>,
}

/// Outcome of an [`CapabilityRegistry::integrate_discovery`] run.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IntegrateReport {
    /// Capability names added to `.route/capability/registry.json`.
    pub registered: Vec<String>,
    /// Skill filenames written to `.route/skills/`.
    pub promoted: Vec<String>,
    /// Skill filenames already present and left untouched.
    pub skipped_existing: Vec<String>,
}

/// Map a discovery `suggested_kind` string onto a capability kind, if known.
fn capability_kind_from_str(s: &str) -> Option<CapabilityKind> {
    Some(match &*s.to_lowercase() {
        "knowledge" | "document" => CapabilityKind::Knowledge,
        "instruction" => CapabilityKind::Instruction,
        "skill" => CapabilityKind::Skill,
        "tool" | "binary" => CapabilityKind::Tool,
        "service" | "mcp" | "api" => CapabilityKind::Service,
        "workflow" => CapabilityKind::Workflow,
        "runtime" => CapabilityKind::Runtime,
        _ => return None,
    })
}

/// Default maturity level assigned to a freshly integrated capability kind.
pub(crate) fn level_for_kind(kind: &CapabilityKind) -> CapabilityLevel {
    match kind {
        CapabilityKind::Tool | CapabilityKind::Service => CapabilityLevel::L3,
        CapabilityKind::Instruction | CapabilityKind::Skill | CapabilityKind::Workflow => {
            CapabilityLevel::L2
        }
        CapabilityKind::Knowledge | CapabilityKind::Runtime => CapabilityLevel::L1,
    }
}

/// Derive a safe lowercase filename stem from a capability name.
fn safe_skill_filename(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_ascii_whitespace() {
            out.push('-');
        }
        // Drop any other character (incl. leading dots) to keep the path safe.
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out = "skill".to_string();
    }
    out
}

/// Render a capability as a reusable Markdown skill file for `.route/skills/`.
fn render_skill_markdown(cap: &Capability) -> String {
    let entrypoint = cap.entrypoint.as_deref().unwrap_or("(none)");
    format!(
        "# {name}\n\n> capability: `{id}`\n> reference: `{ref}`\n> kind: `{kind:?}`\n> level: `{level:?}`\n\n{usage}\n\n## Entrypoint\n\n```\n{entry}\n```\n\n## Inputs\n\n{inputs}\n\n## Outputs\n\n{outputs}\n\n## Permissions\n\n{perms}\n\n## Constraints\n\n{constraints}\n",
        name = cap.name,
        id = cap.id,
        ref = cap.reference_id,
        kind = cap.kind,
        level = cap.level,
        usage = if cap.usage.is_empty() { "_No usage text._" } else { &cap.usage },
        entry = entrypoint,
        inputs = bullet_or_placeholder(&cap.inputs),
        outputs = bullet_or_placeholder(&cap.outputs),
        perms = bullet_or_placeholder(&cap.permissions),
        constraints = bullet_or_placeholder(&cap.constraints),
    )
}

/// Render a vector as a Markdown bullet list, or a placeholder when empty.
fn bullet_or_placeholder(items: &[String]) -> String {
    if items.is_empty() {
        "_none_".to_string()
    } else {
        items
            .iter()
            .map(|s| format!("- {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn skill_capability(id: &str, name: &str) -> Capability {
        Capability {
            id: id.to_string(),
            reference_id: format!("ref-{id}"),
            name: name.to_string(),
            kind: CapabilityKind::Skill,
            level: CapabilityLevel::L2,
            entrypoint: Some("use".to_string()),
            usage: "How to use this skill.".to_string(),
            inputs: vec!["prompt".to_string()],
            outputs: vec!["answer".to_string()],
            permissions: vec![],
            constraints: vec![],
            availability: true,
        }
    }

    #[test]
    fn promote_skill_is_idempotent_and_force_overwrites() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        let mut registry = CapabilityRegistry::default();
        registry.set(skill_capability("cap-a", "Skill A"));
        registry.set(skill_capability("cap-b", "Skill B"));

        // First run writes both.
        let r1 = registry.promote_skills(root, None, false).unwrap();
        assert_eq!(r1.promoted.len(), 2, "both skills promoted first run");
        assert!(r1.skipped_existing.is_empty());
        assert!(root.join(".route/skills/skill-a.md").exists());

        // Second run: existing files are skipped, not overwritten.
        let r2 = registry.promote_skills(root, None, false).unwrap();
        assert!(r2.promoted.is_empty());
        assert_eq!(r2.skipped_existing.len(), 2, "existing files skipped");

        // Non-skill capabilities are ignored.
        let mut other = registry.clone();
        other.set(Capability {
            id: "cap-tool".into(),
            reference_id: "ref-tool".into(),
            name: "A Tool".into(),
            kind: CapabilityKind::Tool,
            level: CapabilityLevel::L3,
            entrypoint: None,
            usage: String::new(),
            inputs: vec![],
            outputs: vec![],
            permissions: vec![],
            constraints: vec![],
            availability: true,
        });
        let rt = other.promote_skills(root, None, true).unwrap();
        assert!(
            !root.join(".route/skills/a-tool.md").exists(),
            "Tool capability must not be promoted"
        );
        assert!(
            !rt.promoted.iter().any(|f| f == "a-tool"),
            "Tool capability must not appear in promoted list"
        );

        // force overwrites existing.
        let r3 = registry.promote_skills(root, None, true).unwrap();
        assert_eq!(r3.promoted.len(), 2, "--force overwrites");

        // Filtering by a single id only promotes that one.
        let r4 = registry.promote_skills(root, Some("cap-a"), true).unwrap();
        assert_eq!(r4.promoted, vec!["skill-a".to_string()]);
    }

    #[test]
    fn safe_filename_sanitizes_and_never_escapes() {
        assert_eq!(safe_skill_filename("My Skill"), "my-skill");
        assert_eq!(safe_skill_filename("a/b\\c"), "abc");
        assert_eq!(safe_skill_filename("..secret.."), "secret");
        assert_eq!(safe_skill_filename("!!!"), "skill");
    }

    #[test]
    fn integrate_discovery_registers_skills_and_promotes_them() {
        use crate::discovery::{DiscoveryItem, DiscoveryProposal};

        let dir = tempdir().unwrap();
        let root = dir.path();
        let mut registry = CapabilityRegistry::default();

        let proposal = DiscoveryProposal {
            id: "discovery-1".to_string(),
            scanned_path: "/tmp/x".to_string(),
            created_at: 0,
            items: vec![
                DiscoveryItem {
                    kind: "skill".to_string(),
                    name: "my-skill".to_string(),
                    path: "/tmp/x/skills/my.md".to_string(),
                    description: "My reusable skill".to_string(),
                    evidence: "Found in skills/ directory".to_string(),
                    confidence: 0.8,
                    suggested_kind: "skill".to_string(),
                },
                DiscoveryItem {
                    kind: "build".to_string(),
                    name: "Makefile".to_string(),
                    path: "/tmp/x/Makefile".to_string(),
                    description: "Makefile build system".to_string(),
                    evidence: "Found at project root".to_string(),
                    confidence: 0.8,
                    suggested_kind: "tool".to_string(),
                },
                DiscoveryItem {
                    kind: "unknown".to_string(),
                    name: "weird".to_string(),
                    path: "/tmp/x/weird".to_string(),
                    description: "Unmapped".to_string(),
                    evidence: "n/a".to_string(),
                    confidence: 0.5,
                    suggested_kind: "nonsense".to_string(),
                },
            ],
            accepted: vec![],
            rejected: vec![],
            status: "pending".to_string(),
        };

        let report = registry.integrate_discovery(root, &proposal).unwrap();
        // skill + tool registered, unknown kind skipped.
        assert_eq!(report.registered.len(), 2);
        assert!(report.registered.contains(&"my-skill".to_string()));
        assert!(report.registered.contains(&"Makefile".to_string()));
        // Only the skill is promoted to .route/skills/.
        assert_eq!(report.promoted, vec!["my-skill".to_string()]);
        assert!(root.join(".route/skills/my-skill.md").exists());
        assert!(!root.join(".route/skills/makefile.md").exists());

        // Registry persisted and the tool capability has L3, skill L2.
        let reloaded = CapabilityRegistry::load(root).unwrap();
        assert!(reloaded.get("cap-my-skill").is_some());
        assert_eq!(
            reloaded.get("cap-my-skill").unwrap().kind,
            CapabilityKind::Skill
        );
        assert_eq!(
            reloaded.get("cap-Makefile").unwrap().kind,
            CapabilityKind::Tool
        );
        assert_eq!(
            reloaded.get("cap-Makefile").unwrap().level,
            CapabilityLevel::L3
        );
        assert!(reloaded.get("cap-weird").is_none());

        // Idempotent: a second run registers nothing new and skips the file.
        let report2 = registry.integrate_discovery(root, &proposal).unwrap();
        assert!(report2.registered.is_empty());
        assert!(report2.promoted.is_empty());
        assert_eq!(report2.skipped_existing, vec!["my-skill".to_string()]);
    }

    #[test]
    fn kind_map_covers_known_kinds_only() {
        assert_eq!(
            capability_kind_from_str("skill"),
            Some(CapabilityKind::Skill)
        );
        assert_eq!(capability_kind_from_str("Tool"), Some(CapabilityKind::Tool));
        assert_eq!(
            capability_kind_from_str("MCP"),
            Some(CapabilityKind::Service)
        );
        assert_eq!(capability_kind_from_str("mystery"), None);
    }
}
