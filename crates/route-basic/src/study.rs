//! Project analysis and structured StudyReport generation.
//!
//! Analyzes a project directory (Cargo.toml, package.json, README, directory
//! structure, CI/CD configs) and produces a structured report with
//! Architecture, capabilities, workflows, skills, patterns, interesting
//! decisions, and import candidates.

use std::path::{Path, PathBuf};

use crate::constitutive::ROUTE_DOT_DIR;
use anyhow::{Context, Result};

/// Study directory under `.route/`.
pub fn study_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("study")
}

/// Path to the last study report file.
pub fn study_report_path(project_root: &Path) -> PathBuf {
    study_dir(project_root).join("last_report.json")
}

/// Save a study report to disk.
pub fn save_report(project_root: &Path, report: &StudyReport) -> Result<()> {
    let dir = study_dir(project_root);
    std::fs::create_dir_all(&dir)?;
    let path = study_report_path(project_root);
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Load a study report from disk.
pub fn load_report(project_root: &Path) -> Result<StudyReport> {
    let path = study_report_path(project_root);
    let json = std::fs::read_to_string(&path)?;
    let report: StudyReport = serde_json::from_str(&json)?;
    Ok(report)
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Structured analysis report for a project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyReport {
    pub summary: String,
    pub architecture: String,
    pub capabilities: Vec<String>,
    pub workflows: Vec<StudyWorkflow>,
    pub skills: Vec<StudySkill>,
    pub patterns: Vec<StudyPattern>,
    pub interesting_decisions: Vec<StudyDecision>,
    pub applicability: String,
    pub candidates: Vec<StudyCandidate>,
}

/// A workflow pattern detected in the project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyWorkflow {
    pub name: String,
    pub description: String,
    pub source: String,
    pub evidence: String,
}

/// A skill detected in the project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudySkill {
    pub name: String,
    pub description: String,
    pub source: String,
    pub evidence: String,
}

/// A code/design pattern detected in the project.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyPattern {
    pub name: String,
    pub description: String,
    pub source: String,
    pub evidence: String,
}

/// An interesting design or architectural decision.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyDecision {
    pub title: String,
    pub description: String,
    pub source: String,
    pub evidence: String,
}

/// A candidate for importing into the Reference registry, workflow store,
/// skill store, or pattern library.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyCandidate {
    pub kind: String, // "reference" | "workflow" | "skill" | "pattern"
    pub name: String,
    pub source: String,
    pub evidence: String,
    pub reason: String,
    pub confidence: f64, // 0.0 - 1.0
}

// ---------------------------------------------------------------------------
// Helpers: reading files
// ---------------------------------------------------------------------------

/// Try to read a file to a string. Returns None if the file doesn't exist.
fn try_read_to_string(path: &Path) -> Option<String> {
    if path.is_file() {
        std::fs::read_to_string(path).ok()
    } else {
        None
    }
}

/// Format a file path relative to the project root.
fn relative<'a>(root: &'a Path, path: &'a Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

// ---------------------------------------------------------------------------
// Analysis helpers
// ---------------------------------------------------------------------------

/// Parse a Cargo.toml and extract metadata, dependencies, features.
fn analyze_cargo_toml(root: &Path, cargo_path: &Path) -> Result<CargoAnalysis> {
    let mut analysis = CargoAnalysis::default();

    let content =
        try_read_to_string(cargo_path).context("Cargo.toml exists but could not be read")?;

    // Parse package name and description from [package] section
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("name = \"") {
            if let Some(end) = val.find('"') {
                analysis.package_name = val[..end].to_string();
            }
        } else if let Some(val) = trimmed.strip_prefix("name = '") {
            if let Some(end) = val.find('\'') {
                analysis.package_name = val[..end].to_string();
            }
        } else if let Some(val) = trimmed.strip_prefix("description = \"") {
            if let Some(end) = val.find('"') {
                analysis.description = val[..end].to_string();
            }
        } else if let Some(val) = trimmed.strip_prefix("description = '") {
            if let Some(end) = val.find('\'') {
                analysis.description = val[..end].to_string();
            }
        }
    }

    // Parse [dependencies] and [dev-dependencies]
    let mut in_deps = false;
    let mut in_dev_deps = false;
    let mut in_features = false;

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed.starts_with("[dependencies]");
            in_dev_deps = trimmed.starts_with("[dev-dependencies]");
            in_features = trimmed.starts_with("[features]");
            continue;
        }
        if in_deps || in_dev_deps {
            if let Some(eq_pos) = trimmed.find('=') {
                let dep_name = trimmed[..eq_pos].trim().to_string();
                if !dep_name.is_empty() && !dep_name.starts_with('#') {
                    let entry = DepEntry {
                        name: dep_name,
                        dev_only: in_dev_deps,
                        line: line_no + 1,
                    };
                    analysis.dependencies.push(entry);
                }
            }
        }
        if in_features {
            if let Some(eq_pos) = trimmed.find('=') {
                let feat_name = trimmed[..eq_pos].trim().to_string();
                if !feat_name.is_empty() && !feat_name.starts_with('#') {
                    analysis.features.push(feat_name);
                }
            }
        }
    }

    analysis.cargo_toml_path = relative(root, cargo_path);
    analysis.cargo_toml_lines = content.lines().count();
    Ok(analysis)
}

#[derive(Default)]
struct CargoAnalysis {
    package_name: String,
    description: String,
    dependencies: Vec<DepEntry>,
    features: Vec<String>,
    cargo_toml_path: String,
    cargo_toml_lines: usize,
}

struct DepEntry {
    name: String,
    dev_only: bool,
    line: usize,
}

/// Parse a package.json if it exists.
fn analyze_package_json(root: &Path, pkg_path: &Path) -> Option<PkgAnalysis> {
    let content = try_read_to_string(pkg_path)?;
    let mut analysis = PkgAnalysis::default();

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("\"name\": \"") {
            if let Some(end) = val.find('"') {
                analysis.name = val[..end].to_string();
            }
        } else if let Some(val) = trimmed.strip_prefix("\"description\": \"") {
            if let Some(end) = val.find('"') {
                analysis.description = val[..end].to_string();
            }
        }
    }
    analysis.path = relative(root, pkg_path);
    Some(analysis)
}

#[derive(Default)]
struct PkgAnalysis {
    name: String,
    description: String,
    path: String,
}

/// Collect directory structure relevant for analysis.
fn collect_directory_structure(root: &Path) -> DirStructure {
    let mut dirs = Vec::new();
    let mut key_files = Vec::new();
    let skip_dirs = [
        "target",
        ".git",
        "node_modules",
        ".svn",
        "__pycache__",
        ".venv",
        "venv",
    ];

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = relative(root, &path);
            if path.is_dir() {
                // Skip large build / dependency directories
                let should_skip = path
                    .file_name()
                    .map(|n| {
                        let name = n.to_string_lossy();
                        skip_dirs.contains(&name.as_ref()) || name.starts_with('.')
                    })
                    .unwrap_or(false);
                if !should_skip {
                    dirs.push(rel);
                }
            } else if path.is_file() {
                key_files.push(rel);
            }
        }
    }
    DirStructure { dirs, key_files }
}

struct DirStructure {
    dirs: Vec<String>,
    #[allow(dead_code)]
    key_files: Vec<String>,
}

/// Look for CI/CD configuration files.
fn find_ci_configs(root: &Path) -> Vec<CiConfig> {
    let mut configs = Vec::new();

    // GitHub Actions
    let gh_path = root.join(".github").join("workflows");
    if gh_path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&gh_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .map_or(false, |e| e == "yml" || e == "yaml")
                {
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    configs.push(CiConfig {
                        name,
                        source: relative(root, &path),
                        kind: "github_actions".to_string(),
                    });
                }
            }
        }
    }

    // Jenkinsfile
    let jenkins = root.join("Jenkinsfile");
    if jenkins.is_file() {
        configs.push(CiConfig {
            name: "Jenkinsfile".to_string(),
            source: relative(root, &jenkins),
            kind: "jenkins".to_string(),
        });
    }

    // .gitlab-ci.yml
    let gitlab = root.join(".gitlab-ci.yml");
    if gitlab.is_file() {
        configs.push(CiConfig {
            name: ".gitlab-ci.yml".to_string(),
            source: relative(root, &gitlab),
            kind: "gitlab_ci".to_string(),
        });
    }

    configs
}

struct CiConfig {
    name: String,
    source: String,
    kind: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Analyze a project at the given path and return a structured StudyReport.
///
/// The path can be a repository root, file, or directory. Analysis is
/// file-based — it reads Cargo.toml, package.json, README.md, directory
/// structure, and CI/CD configs to extract capabilities, workflows, skills,
/// patterns, decisions, and import candidates.
pub fn study_project(path: &str) -> Result<StudyReport> {
    let root = Path::new(path);
    if !root.exists() {
        anyhow::bail!("path does not exist: {}", path);
    }

    // Determine the project root: if path is a file, use its parent.
    let project_root = if root.is_file() {
        root.parent().unwrap_or(root)
    } else {
        root
    };

    // ---- Summary ----
    let mut summary_parts = Vec::new();

    // Cargo.toml analysis
    let cargo_path = project_root.join("Cargo.toml");
    let cargo_analysis = if cargo_path.is_file() {
        match analyze_cargo_toml(project_root, &cargo_path) {
            Ok(a) => {
                if !a.package_name.is_empty() {
                    summary_parts.push(format!("Rust crate: {}", a.package_name));
                }
                if !a.description.is_empty() {
                    summary_parts.push(format!("Description: {}", a.description));
                }
                summary_parts.push(format!(
                    "Dependencies: {} ({} dev-only)",
                    a.dependencies.iter().filter(|d| !d.dev_only).count(),
                    a.dependencies.iter().filter(|d| d.dev_only).count()
                ));
                if !a.features.is_empty() {
                    summary_parts.push(format!("Features: {}", a.features.join(", ")));
                }
                Some(a)
            }
            Err(e) => {
                summary_parts.push(format!("Cargo.toml: parse error — {}", e));
                None
            }
        }
    } else {
        None
    };

    // package.json analysis
    let pkg_json_path = project_root.join("package.json");
    let pkg_analysis = if pkg_json_path.is_file() {
        analyze_package_json(project_root, &pkg_json_path)
    } else {
        None
    };
    if let Some(ref pkg) = pkg_analysis {
        if !pkg.name.is_empty() {
            summary_parts.push(format!("npm package: {}", pkg.name));
        }
        if !pkg.description.is_empty() {
            summary_parts.push(format!("Description: {}", pkg.description));
        }
    }

    // README.md
    let readme_path = find_readme(project_root);
    let readme_content = readme_path.as_ref().and_then(|p| try_read_to_string(p));
    if let Some(ref readme) = readme_content {
        let first_line = readme.lines().next().unwrap_or("").trim();
        if !first_line.is_empty() {
            summary_parts.push(format!("README: {}", first_line));
        }
    }

    // Directory structure
    let dir_structure = collect_directory_structure(project_root);
    let src_dirs: Vec<_> = dir_structure
        .dirs
        .iter()
        .filter(|d| d.starts_with("src") || d.starts_with("crates"))
        .cloned()
        .collect();
    if !src_dirs.is_empty() {
        summary_parts.push(format!("Source dirs: {}", src_dirs.join(", ")));
    }

    // CI/CD configs
    let ci_configs = find_ci_configs(project_root);
    if !ci_configs.is_empty() {
        let ci_names: Vec<_> = ci_configs.iter().map(|c| c.name.as_str()).collect();
        summary_parts.push(format!("CI/CD: {}", ci_names.join(", ")));
    }

    let summary = summary_parts.join("\n  ");

    // ---- Architecture ----
    let mut arch_parts = Vec::new();

    // Detect workspace vs single crate
    let workspace_cargo = project_root.join("Cargo.toml");
    if workspace_cargo.is_file() {
        if let Some(content) = try_read_to_string(&workspace_cargo) {
            if content.contains("[workspace]") {
                arch_parts.push("Cargo workspace project".to_string());
                // Find member crates
                let mut in_members = false;
                for (line_no, line) in content.lines().enumerate() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("[workspace]") {
                        in_members = true;
                        continue;
                    }
                    if in_members && trimmed.starts_with('[') {
                        break;
                    }
                    if in_members && trimmed.starts_with("members") {
                        arch_parts.push(format!(
                            "  Workspace members defined at {}:{}",
                            relative(project_root, &workspace_cargo),
                            line_no + 1
                        ));
                    }
                }
            } else {
                arch_parts.push("Single Rust crate".to_string());
            }
        }
    }

    if let Some(ref pkg) = pkg_analysis {
        arch_parts.push(format!("Node.js package at {}", pkg.path));
    }

    // Source directory structure
    let src_path = project_root.join("src");
    if src_path.is_dir() {
        let modules = collect_modules(&src_path, "");
        if !modules.is_empty() {
            arch_parts.push("Source modules:".to_string());
            for m in &modules {
                arch_parts.push(format!("  - {}", m));
            }
        }
    }

    // Crates directory
    let crates_path = project_root.join("crates");
    if crates_path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&crates_path) {
            let sub_crates: Vec<_> = entries
                .flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    // Check if it has a Cargo.toml
                    if e.path().join("Cargo.toml").is_file() {
                        if let Some(content) = try_read_to_string(&e.path().join("Cargo.toml")) {
                            for line in content.lines() {
                                let t = line.trim();
                                if let Some(val) = t.strip_prefix("name = \"") {
                                    if let Some(end) = val.find('"') {
                                        return format!("{} (crate: {})", name, &val[..end]);
                                    }
                                }
                            }
                        }
                    }
                    name
                })
                .collect();
            if !sub_crates.is_empty() {
                arch_parts.push("Sub-crates:".to_string());
                for c in &sub_crates {
                    arch_parts.push(format!("  - {}", c));
                }
            }
        }
    }

    let architecture = arch_parts.join("\n");

    // ---- Capabilities ----
    let mut capabilities = Vec::new();

    // From Cargo.toml dependencies
    if let Some(ref cargo) = cargo_analysis {
        for dep in &cargo.dependencies {
            let label = match dep.name.as_str() {
                "serde" | "serde_json" => "Serialization (serde)",
                "tokio" => "Async runtime (tokio)",
                "clap" => "CLI argument parsing (clap)",
                "anyhow" | "thiserror" => "Error handling",
                "tracing" | "log" => "Structured logging",
                "rusqlite" | "sqlite" | "diesel" => "SQL database",
                "reqwest" => "HTTP client",
                "actix-web" | "axum" | "warp" | "rocket" => "Web framework",
                "zip" => "Archive handling (zip)",
                "chrono" => "Date/time handling",
                "ulid" | "uuid" => "Unique ID generation",
                "tempfile" => "Temporary file handling",
                "rstest" => "Test framework",
                _ => continue,
            };
            let evidence = format!("{}:{}", cargo.cargo_toml_path, dep.line);
            let cap = format!("{} ({})", label, evidence);
            if !capabilities.contains(&cap) {
                capabilities.push(cap);
            }
        }
    }

    // From package.json
    if let Some(ref pkg) = pkg_analysis {
        capabilities.push(format!("Node.js package management ({})", pkg.path));
    }

    // From CI/CD
    for ci in &ci_configs {
        capabilities.push(format!("CI/CD pipeline: {} ({})", ci.name, ci.source));
    }

    // ---- Workflows ----
    let mut workflows = Vec::new();

    // CI/CD workflows
    for ci in &ci_configs {
        if ci.kind == "github_actions" {
            let content = try_read_to_string(&project_root.join(&ci.source));
            if let Some(ref c) = content {
                for (line_no, line) in c.lines().enumerate() {
                    let trimmed = line.trim();
                    if let Some(name) = trimmed.strip_prefix("name: ") {
                        workflows.push(StudyWorkflow {
                            name: name.to_string(),
                            description: format!("GitHub Actions workflow: {}", name),
                            source: ci.source.clone(),
                            evidence: format!("{}:{}", ci.source, line_no + 1),
                        });
                    }
                }
            }
        }
    }

    // Detect test workflow
    if let Some(ref cargo) = cargo_analysis {
        if cargo
            .dependencies
            .iter()
            .any(|d| d.name == "rstest" || d.name == "tempfile")
        {
            workflows.push(StudyWorkflow {
                name: "test".to_string(),
                description: "Run project tests with cargo test".to_string(),
                source: cargo.cargo_toml_path.clone(),
                evidence: format!(
                    "{}: (dev-dependencies with test tooling)",
                    cargo.cargo_toml_path
                ),
            });
        }
    }

    // ---- Skills ----
    let mut skills = Vec::new();

    // Detect if project has a skills directory
    let skills_dir = project_root.join(".route").join("skills");
    if skills_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let rel = relative(project_root, &path);
                if path.is_file() {
                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    // Read first few lines for description
                    let desc = try_read_to_string(&path)
                        .and_then(|c| {
                            c.lines()
                                .find(|l| l.contains("description"))
                                .map(|l| l.trim().to_string())
                        })
                        .unwrap_or_else(|| "Embedded skill".to_string());
                    skills.push(StudySkill {
                        name,
                        description: desc,
                        source: rel.clone(),
                        evidence: rel,
                    });
                }
            }
        }
    }

    // Detect SKILL.md files
    let skill_files = find_files(project_root, "SKILL.md");
    for sf in &skill_files {
        let rel = relative(project_root, sf);
        let name = sf
            .parent()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "skill".to_string());
        let desc = try_read_to_string(sf)
            .and_then(|c| {
                c.lines()
                    .find(|l| l.contains('#'))
                    .map(|l| l.trim_matches('#').trim().to_string())
            })
            .unwrap_or_else(|| "Skill definition".to_string());
        skills.push(StudySkill {
            name,
            description: desc,
            source: rel.clone(),
            evidence: rel,
        });
    }

    // ---- Patterns ----
    let mut patterns = Vec::new();

    // Detect common patterns from the project structure
    if project_root.join("tests").is_dir() {
        patterns.push(StudyPattern {
            name: "integration-tests".to_string(),
            description: "Integration tests in tests/ directory".to_string(),
            source: relative(project_root, &project_root.join("tests")),
            evidence: "tests/ directory exists".to_string(),
        });
    }

    if project_root.join("examples").is_dir() {
        patterns.push(StudyPattern {
            name: "examples".to_string(),
            description: "Usage examples in examples/ directory".to_string(),
            source: relative(project_root, &project_root.join("examples")),
            evidence: "examples/ directory exists".to_string(),
        });
    }

    if project_root.join("benches").is_dir() {
        patterns.push(StudyPattern {
            name: "benchmarks".to_string(),
            description: "Benchmarks in benches/ directory".to_string(),
            source: relative(project_root, &project_root.join("benches")),
            evidence: "benches/ directory exists".to_string(),
        });
    }

    if project_root.join("scripts").is_dir() {
        patterns.push(StudyPattern {
            name: "scripts".to_string(),
            description: "Utility scripts in scripts/ directory".to_string(),
            source: relative(project_root, &project_root.join("scripts")),
            evidence: "scripts/ directory exists".to_string(),
        });
    }

    // Detect multi-crate layout
    if crates_path.is_dir() {
        patterns.push(StudyPattern {
            name: "multi-crate".to_string(),
            description: "Workspace with multiple sub-crates".to_string(),
            source: relative(project_root, &crates_path),
            evidence: "crates/ directory exists".to_string(),
        });
    }

    // ---- Interesting decisions ----
    let mut decisions = Vec::new();

    if let Some(ref cargo) = cargo_analysis {
        // Check for interesting dependency choices
        for dep in &cargo.dependencies {
            let decision = match dep.name.as_str() {
                "rusqlite" => Some((
                    "Database choice: SQLite".to_string(),
                    format!(
                        "Uses rusqlite for embedded SQL database ({}:{})",
                        cargo.cargo_toml_path, dep.line
                    ),
                )),
                "zip" => Some((
                    "Archive format: zip".to_string(),
                    format!(
                        "Uses zip crate for archive handling ({}:{})",
                        cargo.cargo_toml_path, dep.line
                    ),
                )),
                _ => None,
            };
            if let Some((title, desc)) = decision {
                decisions.push(StudyDecision {
                    title,
                    description: desc.clone(),
                    source: cargo.cargo_toml_path.clone(),
                    evidence: desc,
                });
            }
        }
    }

    // Detect CLI pattern
    if let Some(ref cargo) = cargo_analysis {
        if cargo.dependencies.iter().any(|d| d.name == "clap") {
            decisions.push(StudyDecision {
                title: "CLI-first design".to_string(),
                description: format!(
                    "Uses clap for CLI argument parsing ({}: dependencies)",
                    cargo.cargo_toml_path
                ),
                source: cargo.cargo_toml_path.clone(),
                evidence: "clap dependency present".to_string(),
            });
        }
    }

    // ---- Applicability ----
    let mut applicability_lines = Vec::new();
    if let Some(ref cargo) = cargo_analysis {
        applicability_lines.push(format!(
            "Rust crate '{}' — suitable for project analysis and tooling integration",
            cargo.package_name
        ));
    }
    if let Some(ref pkg) = pkg_analysis {
        applicability_lines.push(format!(
            "Node package '{}' — suitable for web/frontend analysis",
            pkg.name
        ));
    }
    if ci_configs.is_empty() {
        applicability_lines
            .push("No CI/CD detected — consider adding GitHub Actions or similar".to_string());
    }
    if applicability_lines.is_empty() {
        applicability_lines
            .push("General project — analysis based on directory structure".to_string());
    }
    let applicability = applicability_lines.join("\n");

    // ---- Candidates ----
    let mut candidates = Vec::new();

    // Candidate: each CI/CD workflow as a reference candidate
    for ci in &ci_configs {
        let confidence = if ci.kind == "github_actions" {
            0.9
        } else {
            0.7
        };
        candidates.push(StudyCandidate {
            kind: "reference".to_string(),
            name: format!("ci-{}", ci.name),
            source: ci.source.clone(),
            evidence: format!("Detected CI config: {}", ci.kind),
            reason: format!(
                "CI/CD pipeline '{}' can be imported as a reference",
                ci.name
            ),
            confidence,
        });
    }

    // Candidate: each Cargo dependency as a reference candidate
    if let Some(ref cargo) = cargo_analysis {
        for dep in &cargo.dependencies {
            if dep.name == "rusqlite"
                || dep.name == "serde"
                || dep.name == "tokio"
                || dep.name == "clap"
            {
                candidates.push(StudyCandidate {
                    kind: "reference".to_string(),
                    name: format!("dep-{}", dep.name),
                    source: cargo.cargo_toml_path.clone(),
                    evidence: format!("{}:{}", cargo.cargo_toml_path, dep.line),
                    reason: format!(
                        "Key dependency '{}' should be tracked as a reference",
                        dep.name
                    ),
                    confidence: 0.8,
                });
            }
        }
    }

    // Candidate: workspace / multi-crate pattern
    if crates_path.is_dir() {
        candidates.push(StudyCandidate {
            kind: "pattern".to_string(),
            name: "multi-crate-workspace".to_string(),
            source: relative(project_root, &crates_path),
            evidence: "crates/ directory with sub-crates".to_string(),
            reason: "Multi-crate workspace pattern can be extracted for reuse".to_string(),
            confidence: 0.85,
        });
    }

    // Candidate: README as a document reference
    if let Some(rp) = readme_path.as_ref() {
        candidates.push(StudyCandidate {
            kind: "reference".to_string(),
            name: "doc-readme".to_string(),
            source: relative(project_root, rp),
            evidence: "Primary project documentation".to_string(),
            reason: "README.md should be registered as a document reference".to_string(),
            confidence: 0.95,
        });
    }

    // Candidate: each detected skill
    for skill in &skills {
        candidates.push(StudyCandidate {
            kind: "skill".to_string(),
            name: skill.name.clone(),
            source: skill.source.clone(),
            evidence: skill.evidence.clone(),
            reason: format!("Detected skill '{}' can be imported", skill.name),
            confidence: 0.75,
        });
    }

    // ---- Route self-dogfood detection ----
    // When studying Route itself, enrich the report with Route-specific
    // patterns, workflows, decisions, and self-improvement candidates.
    let is_route_self = cargo_analysis
        .as_ref()
        .map(|c| {
            c.package_name == "route"
                || c.package_name.starts_with("route-")
                || c.dependencies.iter().any(|d| d.name.starts_with("route-"))
        })
        .unwrap_or(false)
        || project_root
            .join("crates")
            .join("route-cli")
            .join("Cargo.toml")
            .is_file();

    if is_route_self {
        // --- Route architecture ---
        if !architecture.contains("Route") {
            // Already captured via workspace detection, but add more detail
        }

        // --- Route-specific patterns ---
        let route_patterns = [
            ("journal", "Transaction journal for crash-safe snapshot operations", "Journal-based commit ensures atomicity and crash recovery"),
            ("snapshot-chain", "Content-addressed snapshot chain with edge-centric metadata", "Snapshots are pure file states; metadata lives on edges between nodes"),
            ("context-pipeline", "Constitution → Protocol → Profile → Reference → Context pipeline", "Context is assembled from layered components: Constitution (stable), Protocol (versioned), Profile (active), References (imported)"),
            ("study-candidate", "Study → Candidate → Proposal → Apply pipeline", "Project analysis produces candidates that can be proposed and applied as references, workflows, skills, or patterns"),
            ("evidence-ledger", "Evidence-ledger for session audit and verification", "Every session action (commit, rollback, test) is recorded as evidence for audit"),
            ("memory-refresh", "ProjectMemory.refresh() scans sessions, evidence, registry, commits", "Memory is a semantic layer generated by scanning raw data — never hand-written"),
        ];
        for (name, desc, evidence) in &route_patterns {
            let already = patterns.iter().any(|p| p.name == *name);
            if !already {
                patterns.push(StudyPattern {
                    name: name.to_string(),
                    description: desc.to_string(),
                    source: "crates/route-basic/src/".to_string(),
                    evidence: evidence.to_string(),
                });
            }
        }

        // --- Route-specific workflows (from .route/workflows/) ---
        let route_wf_dir = project_root.join(ROUTE_DOT_DIR).join("workflows");
        if route_wf_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&route_wf_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(true, |e| e != "json") {
                        continue;
                    }
                    let rel = relative(project_root, &path);
                    let wf_name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let already = workflows.iter().any(|w| w.name == wf_name);
                    if !already {
                        let content = try_read_to_string(&path).unwrap_or_default();
                        let desc = if content.contains("\"description\"") {
                            // Try to extract description from JSON
                            content
                                .lines()
                                .find(|l| l.trim().starts_with("\"description\""))
                                .and_then(|l| {
                                    l.split(':').nth(1).map(|s| {
                                        s.trim().trim_matches(',').trim_matches('"').to_string()
                                    })
                                })
                                .unwrap_or_else(|| format!("Route workflow: {}", wf_name))
                        } else {
                            format!("Route workflow: {}", wf_name)
                        };
                        workflows.push(StudyWorkflow {
                            name: wf_name,
                            description: desc,
                            source: rel.clone(),
                            evidence: rel,
                        });
                    }
                }
            }
        }

        // --- Route-specific decisions ---
        let route_decisions = [
            ("Multi-crate workspace architecture",
             "Route is organized as a workspace with separate crates (route-core, route-basic, route-cli, route-engine, etc.) for clean separation of concerns",
             "crates/ directory and workspace Cargo.toml"),
            ("Edge-centric commit model",
             "Commit metadata lives on edges (between snapshot nodes), not on nodes. Nodes are pure file states. This enables N-N relationships and path-attached text annotations",
             "crates/route-basic/src/lib.rs:1"),
            ("Context pipeline over configuration files",
             "Route builds the Effective Context by layering Constitution + Protocol + Profile + References, rather than using a single monolithic config file",
             "crates/route-basic/src/constitutive.rs"),
            ("Study + Library model for project analysis",
             "Route studies projects into structured reports, stores them in a library, and can diff/compare multiple studies to extract cross-project patterns",
             "crates/route-basic/src/study.rs"),
            ("Evidence-based audit trail",
             "Session actions (commit, rollback, test, verify) all produce evidence records that form an immutable audit trail",
             "crates/route-basic/src/execution.rs"),
        ];
        for (title, desc, evidence) in &route_decisions {
            let already = decisions.iter().any(|d| d.title == *title);
            if !already {
                decisions.push(StudyDecision {
                    title: title.to_string(),
                    description: desc.to_string(),
                    source: "crates/route-basic/src/".to_string(),
                    evidence: evidence.to_string(),
                });
            }
        }

        // --- Self-improvement candidates ---
        let self_candidates = [
            ("pattern", "route-context-pipeline",
             "Route's context pipeline (Constitution → Protocol → Profile → Reference) is a reusable pattern that can be extracted and formalized",
             "crates/route-basic/src/constitutive.rs",
             "Route's own context assembly pattern should be registered for reuse across projects", 0.85),
            ("pattern", "route-study-library",
             "Route's study library (study → record → library → diff/compare) is a reusable pattern for project analysis",
             "crates/route-basic/src/study.rs",
             "The study library pattern can be applied to other projects for structured analysis", 0.80),
            ("workflow", "wf-self-improve",
             "Route can study itself and generate improvement proposals — this is a meta-workflow for self-dogfooding",
             "crates/route-cli/src/commands.rs",
             "Self-improvement workflow should be formalized as a reusable workflow", 0.75),
            ("reference", "ref-memory-module",
             "Route's memory module (ProjectMemory + MemoryStore) is a key reference for understanding project semantics",
             "crates/route-basic/src/memory.rs",
             "The memory module should be registered as a reference for context injection", 0.90),
        ];
        for (kind, name, reason, source, evidence, confidence) in &self_candidates {
            let already = candidates.iter().any(|c| c.name == *name);
            if !already {
                candidates.push(StudyCandidate {
                    kind: kind.to_string(),
                    name: name.to_string(),
                    source: source.to_string(),
                    evidence: evidence.to_string(),
                    reason: reason.to_string(),
                    confidence: *confidence,
                });
            }
        }
    }

    // ---- Build the report ----
    Ok(StudyReport {
        summary,
        architecture,
        capabilities,
        workflows,
        skills,
        patterns,
        interesting_decisions: decisions,
        applicability,
        candidates,
    })
}

/// Find a README file (README.md, README.rst, README, etc.)
fn find_readme(root: &Path) -> Option<PathBuf> {
    let candidates = ["README.md", "README.rst", "README.txt", "README"];
    for name in &candidates {
        let p = root.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Collect Rust module names from a directory by finding .rs files.
fn collect_modules(dir: &Path, prefix: &str) -> Vec<String> {
    let mut modules = Vec::new();
    let skip_dirs = [
        "target",
        ".git",
        "node_modules",
        ".svn",
        "__pycache__",
        ".venv",
        "venv",
    ];

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip large build / dependency directories
                let should_skip = path
                    .file_name()
                    .map(|n| {
                        let name = n.to_string_lossy();
                        skip_dirs.contains(&name.as_ref()) || name.starts_with('.')
                    })
                    .unwrap_or(false);
                if should_skip {
                    continue;
                }
                let sub = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let sub_prefix = if prefix.is_empty() {
                    sub.clone()
                } else {
                    format!("{}::{}", prefix, sub)
                };
                modules.push(sub_prefix.clone());
                modules.extend(collect_modules(&path, &sub_prefix));
            } else if path.extension().map_or(false, |e| e == "rs") {
                if let Some(name) = path.file_stem() {
                    let name = name.to_string_lossy().to_string();
                    if name != "lib" && name != "main" && name != "mod" {
                        let full = if prefix.is_empty() {
                            name
                        } else {
                            format!("{}::{}", prefix, name)
                        };
                        modules.push(full);
                    }
                }
            }
        }
    }
    modules
}

/// Find all files with a given name under a root directory.
fn find_files(root: &Path, target: &str) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let skip_dirs = [
        "target",
        ".git",
        "node_modules",
        ".route",
        ".svn",
        "__pycache__",
        ".venv",
        "venv",
    ];

    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Skip large build / dependency directories
                if let Some(name) = path.file_name() {
                    let name = name.to_string_lossy();
                    if skip_dirs.contains(&name.as_ref()) {
                        continue;
                    }
                    if name.starts_with('.') && name != ".route" {
                        continue;
                    }
                }
                results.extend(find_files(&path, target));
            } else if path.is_file() {
                if let Some(name) = path.file_name() {
                    if name == target {
                        results.push(path);
                    }
                }
            }
        }
    }
    results
}

// ---------------------------------------------------------------------------
// Study Library — persistent storage of study records
// ---------------------------------------------------------------------------

/// A stored study record with metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyRecord {
    pub id: String,
    pub source: String,
    pub summary: String,
    pub architecture: String,
    pub patterns: Vec<String>,
    pub workflows: Vec<String>,
    pub skills: Vec<String>,
    pub decisions: Vec<String>,
    pub evidence: Vec<String>,
    pub applicability: String,
    pub created_at: i64,
    pub tags: Vec<String>,
}

/// A persistent library of studies.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StudyLibrary {
    pub studies: Vec<StudyRecord>,
}

impl StudyLibrary {
    /// Create an empty library.
    pub fn new() -> Self {
        Self {
            studies: Vec::new(),
        }
    }
}

impl Default for StudyLibrary {
    fn default() -> Self {
        Self::new()
    }
}

/// Directory for study library data under `.route/`.
pub fn library_dir(project_root: &Path) -> PathBuf {
    project_root.join(ROUTE_DOT_DIR).join("studies")
}

/// Path to the study library JSON file.
pub fn library_path(project_root: &Path) -> PathBuf {
    library_dir(project_root).join("library.json")
}

/// Load the study library from disk. Returns an empty library if the file
/// does not exist.
pub fn study_library_load(project_root: &Path) -> Result<StudyLibrary> {
    let path = library_path(project_root);
    if path.is_file() {
        let json = std::fs::read_to_string(&path)?;
        let lib: StudyLibrary = serde_json::from_str(&json)?;
        Ok(lib)
    } else {
        Ok(StudyLibrary::new())
    }
}

/// Save the study library to disk.
pub fn study_library_save(project_root: &Path, lib: &StudyLibrary) -> Result<()> {
    let dir = library_dir(project_root);
    std::fs::create_dir_all(&dir)?;
    let path = library_path(project_root);
    let json = serde_json::to_string_pretty(lib)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Add a study record to the library (inserts or replaces by id).
pub fn study_library_add(project_root: &Path, record: StudyRecord) -> Result<()> {
    let mut lib = study_library_load(project_root)?;
    if let Some(pos) = lib.studies.iter().position(|s| s.id == record.id) {
        lib.studies[pos] = record;
    } else {
        lib.studies.push(record);
    }
    study_library_save(project_root, &lib)
}

/// Remove a study record by id.
pub fn study_library_remove(project_root: &Path, id: &str) -> Result<()> {
    let mut lib = study_library_load(project_root)?;
    lib.studies.retain(|s| s.id != id);
    study_library_save(project_root, &lib)
}

/// Get a study record by id.
pub fn study_library_get(project_root: &Path, id: &str) -> Result<Option<StudyRecord>> {
    let lib = study_library_load(project_root)?;
    Ok(lib.studies.into_iter().find(|s| s.id == id))
}

/// Diff two studies by id, returning a human-readable description of
/// differences in summary, architecture, patterns, workflows, skills,
/// decisions, and applicability.
pub fn study_library_diff(project_root: &Path, a_id: &str, b_id: &str) -> Result<String> {
    let lib = study_library_load(project_root)?;
    let a = lib
        .studies
        .iter()
        .find(|s| s.id == a_id)
        .ok_or_else(|| anyhow::anyhow!("study not found: {}", a_id))?;
    let b = lib
        .studies
        .iter()
        .find(|s| s.id == b_id)
        .ok_or_else(|| anyhow::anyhow!("study not found: {}", b_id))?;

    let mut out = String::new();
    out.push_str(&format!("--- {}\n+++ {}\n\n", a_id, b_id));

    // Summary
    if a.summary != b.summary {
        out.push_str("## Summary\n");
        out.push_str(&format!("- A: {}\n", a.summary));
        out.push_str(&format!("- B: {}\n", b.summary));
        out.push('\n');
    }

    // Architecture
    if a.architecture != b.architecture {
        out.push_str("## Architecture\n");
        out.push_str(&format!("- A: {}\n", a.architecture));
        out.push_str(&format!("- B: {}\n", b.architecture));
        out.push('\n');
    }

    // Patterns
    diff_string_vec(&mut out, "Patterns", &a.patterns, &b.patterns);

    // Workflows
    diff_string_vec(&mut out, "Workflows", &a.workflows, &b.workflows);

    // Skills
    diff_string_vec(&mut out, "Skills", &a.skills, &b.skills);

    // Decisions
    diff_string_vec(&mut out, "Decisions", &a.decisions, &b.decisions);

    // Applicability
    if a.applicability != b.applicability {
        out.push_str("## Applicability\n");
        out.push_str(&format!("- A: {}\n", a.applicability));
        out.push_str(&format!("- B: {}\n", b.applicability));
        out.push('\n');
    }

    // Evidence
    diff_string_vec(&mut out, "Evidence", &a.evidence, &b.evidence);

    if out.is_empty() {
        out.push_str("(no differences found)\n");
    }

    Ok(out)
}

/// Helper: compare two `Vec<String>` fields and emit diff lines.
fn diff_string_vec(out: &mut String, label: &str, a: &[String], b: &[String]) {
    let only_in_a: Vec<_> = a.iter().filter(|x| !b.contains(x)).collect();
    let only_in_b: Vec<_> = b.iter().filter(|x| !a.contains(x)).collect();
    if only_in_a.is_empty() && only_in_b.is_empty() {
        return;
    }
    out.push_str(&format!("## {}\n", label));
    if !only_in_a.is_empty() {
        out.push_str("  Only in A:\n");
        for item in &only_in_a {
            out.push_str(&format!("    - {}\n", item));
        }
    }
    if !only_in_b.is_empty() {
        out.push_str("  Only in B:\n");
        for item in &only_in_b {
            out.push_str(&format!("    - {}\n", item));
        }
    }
    out.push('\n');
}

/// Compare multiple studies, highlighting common/conflicting/unique patterns
/// and what Route can learn from the combination.
///
/// Output format:
/// - What patterns appear together (common across all)
/// - What conflicts (same pattern name, different description)
/// - What is unique to each
/// - What Route can learn
pub fn study_library_compare(project_root: &Path, ids: &[String]) -> Result<String> {
    let lib = study_library_load(project_root)?;

    let records: Vec<&StudyRecord> = ids
        .iter()
        .map(|id| {
            lib.studies
                .iter()
                .find(|s| s.id == id.as_str())
                .ok_or_else(|| anyhow::anyhow!("study not found: {}", id))
        })
        .collect::<Result<Vec<_>>>()?;

    if records.len() < 2 {
        anyhow::bail!("need at least 2 studies to compare");
    }

    let mut out = String::new();
    out.push_str("## Study Comparison\n\n");

    // Extract all pattern names from each study
    let pattern_sets: Vec<Vec<String>> = records
        .iter()
        .map(|r| {
            r.patterns
                .iter()
                .map(|p| {
                    // Patterns are stored as "name: description"
                    p.split(':').next().unwrap_or(p).trim().to_string()
                })
                .collect()
        })
        .collect();

    // --- Common patterns (appear in all) ---
    let first_set = &pattern_sets[0];
    let common: Vec<&String> = first_set
        .iter()
        .filter(|p| pattern_sets[1..].iter().all(|s| s.contains(p)))
        .collect();

    out.push_str("### Patterns appearing together (common across all)\n");
    if common.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for p in &common {
            // Find the description from the first record that has this pattern
            let desc = records
                .iter()
                .find_map(|r| {
                    r.patterns
                        .iter()
                        .find(|rp| rp.starts_with(p.as_str()))
                        .map(|rp| rp.trim())
                })
                .unwrap_or(p);
            out.push_str(&format!("  - {}\n", desc));
        }
    }
    out.push('\n');

    // --- Unique patterns per study ---
    out.push_str("### What is unique to each\n");
    for (i, (record, pset)) in records.iter().zip(&pattern_sets).enumerate() {
        let unique: Vec<&String> = pset
            .iter()
            .filter(|p| {
                pattern_sets
                    .iter()
                    .enumerate()
                    .all(|(j, s)| j == i || !s.contains(p))
            })
            .collect();
        if unique.is_empty() {
            out.push_str(&format!("  Study {} ({}): (none unique)\n", i, record.id));
        } else {
            out.push_str(&format!("  Study {} ({}):\n", i, record.id));
            for p in unique {
                out.push_str(&format!("    - {}\n", p));
            }
        }
    }
    out.push('\n');

    // --- Conflicting patterns ---
    out.push_str("### What conflicts\n");
    let mut found_conflict = false;
    // Collect all pattern names
    let all_pattern_names: Vec<String> = {
        let mut names: Vec<String> = Vec::new();
        for pset in &pattern_sets {
            for p in pset {
                let name = p.clone();
                if !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        names
    };
    for pname in &all_pattern_names {
        let mut descriptions: Vec<(&StudyRecord, &str)> = Vec::new();
        for r in &records {
            if let Some(p) = r.patterns.iter().find(|rp| rp.starts_with(pname.as_str())) {
                descriptions.push((r, p.as_str()));
            }
        }
        if descriptions.len() > 1 {
            let first_desc = descriptions[0].1;
            if descriptions[1..].iter().any(|(_, d)| *d != first_desc) {
                found_conflict = true;
                out.push_str(&format!(
                    "  Pattern '{}' has conflicting descriptions:\n",
                    pname
                ));
                for (r, d) in &descriptions {
                    out.push_str(&format!("    - {}: {}\n", r.id, d));
                }
            }
        }
    }
    if !found_conflict {
        out.push_str("  (no conflicts detected)\n");
    }
    out.push('\n');

    // --- What Route can learn ---
    out.push_str("### What Route can learn\n");
    let mut learnings = Vec::new();

    // Learning from common patterns
    if !common.is_empty() {
        learnings.push(format!(
            "Common patterns ({}): These patterns are consistent across {} studies, suggesting stable practices worth formalizing.",
            common.len(),
            records.len()
        ));
    }

    // Learning from unique patterns
    for (i, (record, pset)) in records.iter().zip(&pattern_sets).enumerate() {
        let unique: Vec<&String> = pset
            .iter()
            .filter(|p| {
                pattern_sets
                    .iter()
                    .enumerate()
                    .all(|(j, s)| j == i || !s.contains(p))
            })
            .collect();
        if !unique.is_empty() {
            learnings.push(format!(
                "Study {} ({}) has {} unique pattern(s) — consider integrating: {}",
                i,
                record.id,
                unique.len(),
                unique
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    // Learning from conflicts
    if found_conflict {
        learnings.push(
            "Conflicting patterns detected — Route should reconcile these before adopting either variant.".to_string(),
        );
    }

    if learnings.is_empty() {
        out.push_str("  (no actionable learnings)\n");
    } else {
        for l in &learnings {
            out.push_str(&format!("  - {}\n", l));
        }
    }
    out.push('\n');

    // --- Source evidence links ---
    out.push_str("### Source evidence\n");
    for r in &records {
        out.push_str(&format!(
            "  - {}: created_at={}, source={}\n",
            r.id, r.created_at, r.source
        ));
        for e in &r.evidence {
            out.push_str(&format!("      evidence: {}\n", e));
        }
    }
    out.push('\n');

    Ok(out)
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Format a StudyReport as human-readable YAML-like text.
pub fn format_report(report: &StudyReport) -> String {
    let mut out = String::new();

    // Summary
    out.push_str("## Study Report\n\n");
    out.push_str("### Summary\n");
    for line in report.summary.lines() {
        out.push_str(&format!("  {}\n", line));
    }
    out.push('\n');

    // Architecture
    out.push_str("### Architecture\n");
    for line in report.architecture.lines() {
        out.push_str(&format!("  {}\n", line));
    }
    out.push('\n');

    // Capabilities
    out.push_str("### Capabilities\n");
    if report.capabilities.is_empty() {
        out.push_str("  (none detected)\n");
    } else {
        for cap in &report.capabilities {
            out.push_str(&format!("  - {}\n", cap));
        }
    }
    out.push('\n');

    // Workflows
    out.push_str("### Workflows\n");
    if report.workflows.is_empty() {
        out.push_str("  (none detected)\n");
    } else {
        for wf in &report.workflows {
            out.push_str(&format!("  - {} ({})\n", wf.name, wf.description));
            out.push_str(&format!("      source:   {}\n", wf.source));
            out.push_str(&format!("      evidence: {}\n", wf.evidence));
        }
    }
    out.push('\n');

    // Skills
    out.push_str("### Skills\n");
    if report.skills.is_empty() {
        out.push_str("  (none detected)\n");
    } else {
        for sk in &report.skills {
            out.push_str(&format!("  - {}: {}\n", sk.name, sk.description));
            out.push_str(&format!("      source:   {}\n", sk.source));
            out.push_str(&format!("      evidence: {}\n", sk.evidence));
        }
    }
    out.push('\n');

    // Patterns
    out.push_str("### Patterns\n");
    if report.patterns.is_empty() {
        out.push_str("  (none detected)\n");
    } else {
        for pt in &report.patterns {
            out.push_str(&format!("  - {}: {}\n", pt.name, pt.description));
            out.push_str(&format!("      source:   {}\n", pt.source));
            out.push_str(&format!("      evidence: {}\n", pt.evidence));
        }
    }
    out.push('\n');

    // Interesting decisions
    out.push_str("### Interesting Decisions\n");
    if report.interesting_decisions.is_empty() {
        out.push_str("  (none detected)\n");
    } else {
        for dec in &report.interesting_decisions {
            out.push_str(&format!("  - {}: {}\n", dec.title, dec.description));
            out.push_str(&format!("      source:   {}\n", dec.source));
            out.push_str(&format!("      evidence: {}\n", dec.evidence));
        }
    }
    out.push('\n');

    // Applicability
    out.push_str("### Applicability\n");
    for line in report.applicability.lines() {
        out.push_str(&format!("  {}\n", line));
    }
    out.push('\n');

    // Candidates
    out.push_str("### Candidates\n");
    if report.candidates.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for (i, cand) in report.candidates.iter().enumerate() {
            out.push_str(&format!(
                "  [{:03}] {} ({}) — confidence: {:.2}\n",
                i, cand.name, cand.kind, cand.confidence
            ));
            out.push_str(&format!("          source:   {}\n", cand.source));
            out.push_str(&format!("          evidence: {}\n", cand.evidence));
            out.push_str(&format!("          reason:   {}\n", cand.reason));
        }
    }
    out.push('\n');

    out
}
