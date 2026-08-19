//! Auto Discovery — scan a project for capabilities and propose imports.
//!
//! Scans filesystem metadata only (no code analysis). Each discovered item
//! is a candidate that the user can approve or reject.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Discovery directory under `.route/`.
pub fn discovery_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("discovery")
}

/// Path to the discovery proposals file.
pub fn discovery_proposals_path(project_root: &Path) -> PathBuf {
    discovery_dir(project_root).join("proposals.json")
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// A single discovered item — a candidate for import.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryItem {
    /// What kind of item was discovered
    pub kind: String,
    /// Name/path of the discovered item
    pub name: String,
    /// Absolute or relative path to the item
    pub path: String,
    /// Brief description
    pub description: String,
    /// Evidence — why we think this is a capability
    pub evidence: String,
    /// Confidence (0.0 - 1.0)
    pub confidence: f64,
    /// Suggested capability kind (if applicable)
    #[serde(default)]
    pub suggested_kind: String,
}

/// A discovery proposal — a set of discovered items the user can review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryProposal {
    /// Unique proposal ID
    pub id: String,
    /// The path that was scanned
    pub scanned_path: String,
    /// When the scan was performed (unix timestamp)
    pub created_at: i64,
    /// Discovered items
    pub items: Vec<DiscoveryItem>,
    /// Which items the user has accepted
    #[serde(default)]
    pub accepted: Vec<String>,
    /// Which items the user has rejected
    #[serde(default)]
    pub rejected: Vec<String>,
    /// Status: pending | applied | declined
    #[serde(default)]
    pub status: String,
}

/// The discovery store — tracks proposals over time.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoveryStore {
    pub proposals: Vec<DiscoveryProposal>,
}

impl DiscoveryStore {
    /// Load discovery store from disk.
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = discovery_proposals_path(project_root);
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Save discovery store to disk.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let dir = discovery_dir(project_root);
        std::fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(discovery_proposals_path(project_root), json)?;
        Ok(())
    }

    /// Add a proposal.
    pub fn add(&mut self, proposal: DiscoveryProposal) {
        self.proposals.push(proposal);
    }

    /// Get a proposal by ID.
    pub fn get(&self, id: &str) -> Option<&DiscoveryProposal> {
        self.proposals.iter().find(|p| p.id == id)
    }

    /// List all proposals.
    pub fn list(&self) -> &[DiscoveryProposal] {
        &self.proposals
    }

    /// Accept specific items in a proposal.
    pub fn accept_items(&mut self, proposal_id: &str, item_names: &[String]) -> Result<()> {
        let proposal = self
            .proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| anyhow::anyhow!("Proposal {} not found", proposal_id))?;

        for name in item_names {
            if !proposal.accepted.contains(name) {
                proposal.accepted.push(name.clone());
            }
        }
        Ok(())
    }

    /// Reject specific items in a proposal.
    pub fn reject_items(&mut self, proposal_id: &str, item_names: &[String]) -> Result<()> {
        let proposal = self
            .proposals
            .iter_mut()
            .find(|p| p.id == proposal_id)
            .ok_or_else(|| anyhow::anyhow!("Proposal {} not found", proposal_id))?;

        for name in item_names {
            if !proposal.rejected.contains(name) {
                proposal.rejected.push(name.clone());
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Scanning logic
// ---------------------------------------------------------------------------

/// Scan a project path for discoverable capabilities.
///
/// Returns a `DiscoveryProposal` with all items found. Does NOT
/// auto-apply anything — the user must accept items.
pub fn scan_project(path: &Path) -> Result<DiscoveryProposal> {
    use std::time::UNIX_EPOCH;

    let mut items: Vec<DiscoveryItem> = Vec::new();
    let canonical_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = canonical_path.display().to_string();

    // Helper to check if a file exists and add an item
    let check_file = |items: &mut Vec<DiscoveryItem>,
                      kind: &str,
                      name: &str,
                      rel_path: &str,
                      desc: &str,
                      evidence: &str,
                      confidence: f64,
                      suggested_kind: &str| {
        let full_path = if Path::new(rel_path).is_absolute() {
            PathBuf::from(rel_path)
        } else {
            canonical_path.join(rel_path)
        };
        if full_path.exists() {
            items.push(DiscoveryItem {
                kind: kind.to_string(),
                name: name.to_string(),
                path: full_path.display().to_string(),
                description: desc.to_string(),
                evidence: evidence.to_string(),
                confidence,
                suggested_kind: suggested_kind.to_string(),
            });
        }
    };

    // Scan for AGENTS.md / CLAUDE.md
    check_file(
        &mut items,
        "config",
        "AGENTS.md",
        "AGENTS.md",
        "AI agent configuration and rules",
        "Found at project root — standard AI convention",
        0.9,
        "knowledge",
    );

    check_file(
        &mut items,
        "config",
        "CLAUDE.md",
        "CLAUDE.md",
        "Claude-specific AI instructions",
        "Found at project root — standard Claude convention",
        0.9,
        "knowledge",
    );

    // Scan for Route configs
    check_file(
        &mut items,
        "config",
        "Constitution",
        ".route/constitution.md",
        "Route project constitution — stable principles",
        "Route project detected",
        1.0,
        "knowledge",
    );

    check_file(
        &mut items,
        "config",
        "Protocol",
        ".route/protocol.md",
        "Route project protocol — execution playbook",
        "Route project detected",
        1.0,
        "instruction",
    );

    // Scan for skills directory
    let skills_dir = canonical_path.join("skills");
    if skills_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".md") || fname.ends_with(".yaml") || fname.ends_with(".yml") {
                    items.push(DiscoveryItem {
                        kind: "skill".to_string(),
                        name: format!(
                            "skill-{}",
                            fname.trim_end_matches(&['.', 'm', 'd', 'y', 'a', 'l'][..])
                        ),
                        path: entry.path().display().to_string(),
                        description: format!("Skill definition: {}", fname),
                        evidence: "Found in skills/ directory".to_string(),
                        confidence: 0.8,
                        suggested_kind: "skill".to_string(),
                    });
                }
            }
        }
    }

    // Scan for MCP configs
    let mcp_paths = [
        ".mcp.json",
        "mcp.json",
        ".mcp/config.json",
        ".cursor/mcp.json",
        ".vscode/mcp.json",
    ];
    for mcp_rel in &mcp_paths {
        check_file(
            &mut items,
            "mcp",
            &format!("mcp-{}", mcp_rel.replace('/', "-").replace('.', "")),
            mcp_rel,
            "MCP server configuration",
            "Found MCP configuration file",
            0.85,
            "service",
        );
    }

    // Scan for package.json scripts
    let pkg_path = canonical_path.join("package.json");
    if pkg_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&pkg_path) {
            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(scripts) = pkg.get("scripts").and_then(|s| s.as_object()) {
                    for (name, _cmd) in scripts {
                        let cmd_str = _cmd.as_str().unwrap_or("");
                        items.push(DiscoveryItem {
                            kind: "script".to_string(),
                            name: format!("npm-{}", name),
                            path: pkg_path.display().to_string(),
                            description: format!("npm script: {} — {}", name, cmd_str),
                            evidence: "Found in package.json scripts".to_string(),
                            confidence: 0.7,
                            suggested_kind: "tool".to_string(),
                        });
                    }
                }
            }
        }
    }

    // Scan for Makefile
    check_file(
        &mut items,
        "build",
        "Makefile",
        "Makefile",
        "Makefile build system",
        "Found at project root",
        0.8,
        "tool",
    );

    // Scan for Taskfile
    check_file(
        &mut items,
        "build",
        "Taskfile",
        "Taskfile.yml",
        "Taskfile build system",
        "Found at project root",
        0.8,
        "tool",
    );

    check_file(
        &mut items,
        "build",
        "Taskfile",
        "Taskfile.yaml",
        "Taskfile build system",
        "Found at project root",
        0.8,
        "tool",
    );

    // Scan for Cargo.toml (Rust)
    check_file(
        &mut items,
        "build",
        "Cargo.toml",
        "Cargo.toml",
        "Rust/Cargo build system",
        "Found at project root",
        0.9,
        "runtime",
    );

    // Scan for docs/ directory
    let docs_dir = canonical_path.join("docs");
    if docs_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&docs_dir) {
            let doc_count = entries.flatten().count();
            if doc_count > 0 {
                items.push(DiscoveryItem {
                    kind: "documentation".to_string(),
                    name: "docs".to_string(),
                    path: docs_dir.display().to_string(),
                    description: format!("Documentation directory with {} files", doc_count),
                    evidence: "Found docs/ directory at project root".to_string(),
                    confidence: 0.7,
                    suggested_kind: "knowledge".to_string(),
                });
            }
        }
    }

    // Scan for .github/workflows
    let gh_workflows = canonical_path.join(".github").join("workflows");
    if gh_workflows.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&gh_workflows) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".yml") || fname.ends_with(".yaml") {
                    items.push(DiscoveryItem {
                        kind: "workflow".to_string(),
                        name: format!(
                            "gh-{}",
                            fname.trim_end_matches(&['.', 'y', 'a', 'm', 'l'][..])
                        ),
                        path: entry.path().display().to_string(),
                        description: format!("GitHub Actions workflow: {}", fname),
                        evidence: "Found in .github/workflows/".to_string(),
                        confidence: 0.85,
                        suggested_kind: "workflow".to_string(),
                    });
                }
            }
        }
    }

    // Scan for .workflow/ directory
    let workflow_dir = canonical_path.join(".workflow");
    if workflow_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&workflow_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".md")
                    || fname.ends_with(".json")
                    || fname.ends_with(".yaml")
                    || fname.ends_with(".yml")
                {
                    items.push(DiscoveryItem {
                        kind: "workflow".to_string(),
                        name: format!("wf-{}", fname),
                        path: entry.path().display().to_string(),
                        description: format!("Workflow definition: {}", fname),
                        evidence: "Found in .workflow/ directory".to_string(),
                        confidence: 0.8,
                        suggested_kind: "workflow".to_string(),
                    });
                }
            }
        }
    }

    // Scan for CLI binaries in common locations
    let bin_dir = canonical_path.join("bin");
    if bin_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&bin_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                items.push(DiscoveryItem {
                    kind: "binary".to_string(),
                    name: format!("bin-{}", fname),
                    path: entry.path().display().to_string(),
                    description: format!("CLI binary: {}", fname),
                    evidence: "Found in bin/ directory".to_string(),
                    confidence: 0.6,
                    suggested_kind: "tool".to_string(),
                });
            }
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    Ok(DiscoveryProposal {
        id: format!("discovery-{}", now),
        scanned_path: path_str,
        created_at: now,
        items,
        accepted: Vec::new(),
        rejected: Vec::new(),
        status: "pending".to_string(),
    })
}

/// Format a discovery proposal for display.
pub fn format_discovery_proposal(proposal: &DiscoveryProposal) -> String {
    let mut out = format!(
        "Discovery Proposal: {}\nScanned: {}\nItems: {}\n\n",
        proposal.id,
        proposal.scanned_path,
        proposal.items.len()
    );

    for (i, item) in proposal.items.iter().enumerate() {
        out.push_str(&format!(
            "  [{i}] {name} ({kind})\n       {desc}\n       Evidence: {ev}\n       Confidence: {conf:.0}%\n\n",
            i = i,
            name = item.name,
            kind = item.kind,
            desc = item.description,
            ev = item.evidence,
            conf = item.confidence * 100.0,
        ));
    }

    if !proposal.accepted.is_empty() {
        out.push_str(&format!("Accepted: {}\n", proposal.accepted.join(", ")));
    }
    if !proposal.rejected.is_empty() {
        out.push_str(&format!("Rejected: {}\n", proposal.rejected.join(", ")));
    }

    out
}
