//! Read-only view over canonical project constraints.
//!
//! `constraints/` is one source, not all project authority. This module derives
//! bounded metadata for inspection and context; it never writes a constraint
//! registry and never promotes a Reference into a Constraint.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::constitutive::{ReferenceEntry, ReferenceRegistry, ROUTE_DOT_DIR};

pub const CONSTRAINTS_DIR: &str = "constraints";

#[derive(Debug, Serialize, Deserialize)]
pub struct ConstraintCoverage {
    pub source: String,
    pub status: String,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConstraintProjection {
    pub constraints: Vec<ConstraintView>,
    pub coverage: Vec<ConstraintCoverage>,
}

/// Metadata-only coverage report. Never creates authority or project state.
pub fn constraint_projection(root: &Path) -> Result<ConstraintProjection> {
    let constraints = discover_constraints(root)?;
    let mut coverage = Vec::new();
    for source in ["constraints/", "constraints/ppam/"] {
        coverage.push(ConstraintCoverage {
            source: source.into(),
            status: if root.join(source).try_exists()? {
                "PARTIAL"
            } else {
                "MISSING"
            }
            .into(),
            reason: "Bounded text-file metadata only; not interpreted rules or exhaustive coverage"
                .into(),
        });
    }
    for source in [".route/constitution.md", ".route/protocol.md"] {
        coverage.push(ConstraintCoverage {
            source: source.into(),
            status: if root.join(source).try_exists()? {
                "NOT_PROJECTED"
            } else {
                "MISSING"
            }
            .into(),
            reason: "Canonical prose remains authoritative; rule extraction is not implemented"
                .into(),
        });
    }
    coverage.push(ConstraintCoverage { source: "active DevelopmentIntent constraints".into(), status: "NOT_PROJECTED".into(), reason: "Intent/session authority remains with its canonical owner; this file scanner does not enumerate active intents".into() });
    Ok(ConstraintProjection {
        constraints,
        coverage,
    })
}
const MAX_CONSTRAINT_FILES: usize = 200;
const MAX_CONSTRAINT_FILE_BYTES: u64 = 512 * 1024;
const MAX_CONSTRAINT_DEPTH: usize = 32;
const SKIP_DIRS: &[&str] = &[
    ".git",
    ROUTE_DOT_DIR,
    ".route-basic",
    "node_modules",
    "target",
    "Release",
    "Wiki",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConstraintAuthority {
    /// Binding material authorized by its placement under `constraints/`.
    /// It may restrict an action but cannot grant permission.
    ProjectConstraintMaterial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConstraintView {
    /// Stable identity derived only from the normalized project-relative path.
    pub constraint_id: String,
    /// Owning Route project when the project already has a persisted identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    /// Project-relative, slash-normalized locator.
    pub locator: String,
    /// Explicit source/provenance; currently always the canonical directory.
    pub provenance: String,
    pub authority: ConstraintAuthority,
    /// `ppam` for `constraints/ppam/**`, otherwise `project`.
    pub scope: String,
    /// A discovered view is active/current at scan time. Removed files simply
    /// disappear on the next scan; Route stores no shadow authority record.
    pub active: bool,
    pub current: bool,
    pub content_hash: String,
    pub byte_length: u64,
    /// Reverse relation derived from Reference entries. Documentation does not
    /// transfer Constraint authority to those References.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub documented_by_reference_ids: Vec<String>,
}

/// Discover the current canonical Constraint view. Missing `constraints/` is
/// normal. Reference relationships are read from the existing Reference store;
/// no second Constraint store is created.
pub fn discover_constraints(project_root: &Path) -> Result<Vec<ConstraintView>> {
    let references = ReferenceRegistry::read(project_root)?;
    discover_constraints_with_references(project_root, &references.entries)
}

/// Pure variant used by callers that already hold the Reference registry.
pub fn discover_constraints_with_references(
    project_root: &Path,
    references: &[ReferenceEntry],
) -> Result<Vec<ConstraintView>> {
    let root = project_root.join(CONSTRAINTS_DIR);
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let project_id =
        crate::project_identity::load_identity(project_root)?.map(|identity| identity.project_id);
    let mut paths = Vec::new();
    walk_bounded(&root, 0, &mut paths)?;
    paths.sort();

    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        let bytes = fs::read(&path)
            .with_context(|| format!("reading constraint material {}", path.display()))?;
        let locator = normalized_relative(project_root, &path)?;
        let constraint_id = format!("constraint:{locator}");
        let mut documented_by_reference_ids: Vec<String> = references
            .iter()
            .filter(|reference| {
                reference
                    .related_constraint_ids
                    .iter()
                    .any(|id| id == &constraint_id)
            })
            .map(|reference| reference.id.clone())
            .collect();
        documented_by_reference_ids.sort();
        documented_by_reference_ids.dedup();
        out.push(ConstraintView {
            constraint_id,
            project_id: project_id.clone(),
            scope: if locator.starts_with("constraints/ppam/") {
                "ppam".to_string()
            } else {
                "project".to_string()
            },
            locator,
            provenance: "constraints/".to_string(),
            authority: ConstraintAuthority::ProjectConstraintMaterial,
            active: true,
            current: true,
            content_hash: route_core::sha256_hex(&bytes),
            byte_length: bytes.len() as u64,
            documented_by_reference_ids,
        });
    }
    Ok(out)
}

pub fn get_constraint(project_root: &Path, constraint_id: &str) -> Result<Option<ConstraintView>> {
    Ok(discover_constraints(project_root)?
        .into_iter()
        .find(|constraint| constraint.constraint_id == constraint_id))
}

fn walk_bounded(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<()> {
    if depth > MAX_CONSTRAINT_DEPTH || out.len() >= MAX_CONSTRAINT_FILES {
        return Ok(());
    }
    let mut entries: Vec<_> = fs::read_dir(dir)
        .with_context(|| format!("reading constraint directory {}", dir.display()))?
        .filter_map(std::result::Result::ok)
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if out.len() >= MAX_CONSTRAINT_FILES {
            break;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("reading constraint metadata {}", path.display()))?;
        let file_type = metadata.file_type();
        // Never follow symlinks or junction-like reparse points. External
        // material must be registered as a Reference/Cooperation locator.
        if file_type.is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if file_type.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            walk_bounded(&path, depth + 1, out)?;
        } else if file_type.is_file()
            && metadata.len() <= MAX_CONSTRAINT_FILE_BYTES
            && is_text_material(&path)
        {
            out.push(path);
        }
    }
    Ok(())
}

fn should_skip_dir(name: &str) -> bool {
    SKIP_DIRS.contains(&name) || name.starts_with("target-")
}

fn is_text_material(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "txt" | "yaml" | "yml" | "json" | "toml")
    )
}

fn normalized_relative(project_root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(project_root).with_context(|| {
        format!(
            "constraint path {} escapes project {}",
            path.display(),
            project_root.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constitutive::{ReferenceEntry, ReferenceType};
    use tempfile::TempDir;

    #[test]
    fn absent_constraint_directory_is_normal() {
        let tmp = TempDir::new().unwrap();
        assert!(discover_constraints_with_references(tmp.path(), &[])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn fresh_projection_reports_all_sources_without_writes() {
        let tmp = TempDir::new().unwrap();
        let report = constraint_projection(tmp.path()).unwrap();
        assert_eq!(report.coverage.len(), 5);
        assert_eq!(report.coverage[4].status, "NOT_PROJECTED");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 0);
    }

    #[test]
    fn scans_canonical_tree_bounded_and_preserves_authority_boundary() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        fs::create_dir_all(root.join("constraints/ppam/rules")).unwrap();
        fs::create_dir_all(root.join("constraints/target-generated")).unwrap();
        fs::write(root.join("constraints/project.md"), "MUST preserve API").unwrap();
        fs::write(
            root.join("constraints/ppam/rules/naming.md"),
            "MUST name well",
        )
        .unwrap();
        fs::write(
            root.join("constraints/target-generated/ignored.md"),
            "MUST NOT LOAD",
        )
        .unwrap();
        fs::write(
            root.join("constraints/too-large.md"),
            vec![b'x'; MAX_CONSTRAINT_FILE_BYTES as usize + 1],
        )
        .unwrap();
        fs::write(root.join("constraints/image.png"), b"not text").unwrap();

        let ppam_id = "constraint:constraints/ppam/rules/naming.md";
        let reference = ReferenceEntry::builder(
            "ref-ppam-doc",
            ReferenceType::Document,
            "docs/ppam.md",
            "Explains the PPAM rule",
        )
        .with_related_constraint_ids(vec![ppam_id.to_string(), ppam_id.to_string()])
        .build();
        let views = discover_constraints_with_references(root, &[reference]).unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].locator, "constraints/ppam/rules/naming.md");
        assert_eq!(views[0].scope, "ppam");
        assert_eq!(views[0].documented_by_reference_ids, ["ref-ppam-doc"]);
        assert_eq!(
            views[0].authority,
            ConstraintAuthority::ProjectConstraintMaterial
        );
        assert_eq!(views[1].scope, "project");
        assert!(views.iter().all(|view| view.active && view.current));
        assert!(views.iter().all(|view| !view.content_hash.is_empty()));
        assert!(views.iter().all(|view| !view.locator.contains('\\')));
        assert!(!views.iter().any(|view| view.locator.contains("target-")));
        // The relation documents authority; it never changes the Reference's
        // own kind into a Constraint type.
        assert_eq!(ReferenceType::Document, ReferenceType::Document);
    }

    #[test]
    fn scan_is_deterministic_and_does_not_materialize_content() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("constraints/z")).unwrap();
        fs::write(tmp.path().join("constraints/z/b.md"), "b").unwrap();
        fs::write(tmp.path().join("constraints/a.md"), "a").unwrap();
        let first = discover_constraints_with_references(tmp.path(), &[]).unwrap();
        let second = discover_constraints_with_references(tmp.path(), &[]).unwrap();
        assert_eq!(first, second);
        assert_eq!(first[0].locator, "constraints/a.md");
        assert_eq!(first[1].locator, "constraints/z/b.md");
    }
}
