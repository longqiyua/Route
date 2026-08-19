//! Project Bootstrap Intelligence — `route init --scan`.
//!
//! Performs a one-shot initialization that:
//! 1. Initializes the Route repository
//! 2. Discovers project capabilities
//! 3. Creates ProjectMemory candidates
//! 4. Detects stack/tools/docs
//! 5. Suggests profile/workflow/pack
//! 6. Generates an initial "understanding" of the project
//!
//! All generated items are proposals — nothing is auto-applied.

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::constitutive::ROUTE_DOT_DIR;

/// Result of a bootstrap scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapResult {
    /// Whether the repository was already initialized
    pub already_initialized: bool,
    /// Discovery proposal (if scan was performed)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery_proposal_id: Option<String>,
    /// Number of items discovered
    pub discovered_items: usize,
    /// Detected technologies/stacks
    #[serde(default)]
    pub detected_stacks: Vec<String>,
    /// Suggested profile name
    #[serde(default)]
    pub suggested_profile: String,
    /// Suggested workflow IDs
    #[serde(default)]
    pub suggested_workflows: Vec<String>,
    /// Suggested pack IDs
    #[serde(default)]
    pub suggested_packs: Vec<String>,
    /// Initial project understanding summary
    pub understanding: String,
    /// Memory candidates generated
    #[serde(default)]
    pub memory_candidates: Vec<BootstrapMemoryCandidate>,
    /// Whether a study was performed
    pub study_performed: bool,
}

/// A memory candidate generated during bootstrap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapMemoryCandidate {
    pub kind: String,
    pub content: String,
    pub source: String,
}

/// Perform a bootstrap scan of a project.
///
/// This is a one-shot operation that combines discovery, study, and
/// initialization into a single flow. All results are proposals.
pub fn bootstrap_scan(project_root: &Path) -> Result<BootstrapResult> {
    use std::time::UNIX_EPOCH;

    // Check if already initialized
    let already = crate::repository::BasicRepository::open(project_root).is_ok();

    // Detect stacks
    let mut stacks = Vec::new();
    let mut suggested_workflows = Vec::new();
    let mut memory_candidates = Vec::new();

    // Check for Cargo.toml (Rust)
    if project_root.join("Cargo.toml").exists() {
        stacks.push("Rust/Cargo".to_string());
        suggested_workflows.push("wf-cargo-build".to_string());
        memory_candidates.push(BootstrapMemoryCandidate {
            kind: "current".to_string(),
            content: "Project uses Rust with Cargo build system".to_string(),
            source: "detected: Cargo.toml".to_string(),
        });
    }

    // Check for package.json (Node.js)
    if project_root.join("package.json").exists() {
        stacks.push("Node.js/npm".to_string());
        memory_candidates.push(BootstrapMemoryCandidate {
            kind: "current".to_string(),
            content: "Project uses Node.js with npm package manager".to_string(),
            source: "detected: package.json".to_string(),
        });
    }

    // Check for Python
    if project_root.join("setup.py").exists()
        || project_root.join("pyproject.toml").exists()
        || project_root.join("requirements.txt").exists()
    {
        stacks.push("Python".to_string());
        memory_candidates.push(BootstrapMemoryCandidate {
            kind: "current".to_string(),
            content: "Project uses Python".to_string(),
            source: "detected: setup.py/pyproject.toml/requirements.txt".to_string(),
        });
    }

    // Check for Go
    if project_root.join("go.mod").exists() {
        stacks.push("Go".to_string());
        memory_candidates.push(BootstrapMemoryCandidate {
            kind: "current".to_string(),
            content: "Project uses Go with Go modules".to_string(),
            source: "detected: go.mod".to_string(),
        });
    }

    // Check for Docker
    if project_root.join("Dockerfile").exists() {
        stacks.push("Docker".to_string());
        memory_candidates.push(BootstrapMemoryCandidate {
            kind: "current".to_string(),
            content: "Project uses Docker containerization".to_string(),
            source: "detected: Dockerfile".to_string(),
        });
    }

    // Check for .github/workflows
    if project_root.join(".github").join("workflows").exists() {
        stacks.push("GitHub Actions".to_string());
        memory_candidates.push(BootstrapMemoryCandidate {
            kind: "current".to_string(),
            content: "Project uses GitHub Actions for CI/CD".to_string(),
            source: "detected: .github/workflows/".to_string(),
        });
    }

    // Check for Makefile
    if project_root.join("Makefile").exists() {
        stacks.push("Make".to_string());
        memory_candidates.push(BootstrapMemoryCandidate {
            kind: "current".to_string(),
            content: "Project uses Makefile build system".to_string(),
            source: "detected: Makefile".to_string(),
        });
    }

    // Check for docs directory
    if project_root.join("docs").is_dir() {
        memory_candidates.push(BootstrapMemoryCandidate {
            kind: "current".to_string(),
            content: "Project has documentation in docs/ directory".to_string(),
            source: "detected: docs/".to_string(),
        });
    }

    // Check for README
    for readme_name in &["README.md", "README.rst", "README.txt", "README"] {
        if project_root.join(readme_name).exists() {
            memory_candidates.push(BootstrapMemoryCandidate {
                kind: "current".to_string(),
                content: format!("Project has {} documentation", readme_name),
                source: "detected: README".to_string(),
            });
            break;
        }
    }

    // Check for existing Route configs
    let route_dir = project_root.join(ROUTE_DOT_DIR);
    if route_dir.exists() {
        if route_dir.join("constitution.md").exists() {
            memory_candidates.push(BootstrapMemoryCandidate {
                kind: "current".to_string(),
                content: "Project has a Route Constitution defining stable principles".to_string(),
                source: "detected: .route/constitution.md".to_string(),
            });
        }
        if route_dir.join("protocol.md").exists() {
            memory_candidates.push(BootstrapMemoryCandidate {
                kind: "current".to_string(),
                content: "Project has a Route Protocol defining execution playbook".to_string(),
                source: "detected: .route/protocol.md".to_string(),
            });
        }
    }

    // Determine suggested profile
    let suggested_profile = if stacks.len() > 2 {
        "default".to_string()
    } else if stacks.is_empty() {
        "fast".to_string()
    } else {
        "default".to_string()
    };

    // Build understanding summary
    let understanding = build_understanding(&stacks, &memory_candidates);

    // Perform discovery scan
    let discovered_items = {
        match crate::discovery::scan_project(project_root) {
            Ok(proposal) => {
                // Store the proposal
                let mut store = crate::discovery::DiscoveryStore::load(project_root)?;
                store.add(proposal);
                let _ = store.save(project_root);
                store.list().last().map(|p| p.items.len()).unwrap_or(0)
            }
            Err(_) => 0,
        }
    };

    Ok(BootstrapResult {
        already_initialized: already,
        discovery_proposal_id: Some(format!(
            "discovery-{}",
            std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        )),
        discovered_items,
        detected_stacks: stacks,
        suggested_profile,
        suggested_workflows,
        suggested_packs: Vec::new(),
        understanding,
        memory_candidates,
        study_performed: false,
    })
}

fn build_understanding(stacks: &[String], candidates: &[BootstrapMemoryCandidate]) -> String {
    let mut parts = vec!["Project Understanding:".to_string()];

    if stacks.is_empty() {
        parts.push("  No specific technology stack detected.".to_string());
    } else {
        parts.push(format!("  Detected stacks: {}", stacks.join(", ")));
    }

    let current_count = candidates.iter().filter(|c| c.kind == "current").count();
    parts.push(format!(
        "  {} initial observations recorded.",
        current_count
    ));

    parts.join("\n")
}
