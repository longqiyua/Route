//! Route self-evolution → SOP generator.
//!
//! Route studies **its own development process**: it reads the project's
//! decision memory plus its own standard files (constitution / protocol /
//! reference registry) and distills them into a standard operating procedure
//! (`sop.md`). This is how Route "constrains its own development and upgrades
//! it into an SOP" from real project evidence — not a hand-written demo.
//!
//! The output is a single, readable Markdown document written to
//! `.route/sop.md`. Generation is **deterministic and evidence-based**: every
//! section cites the concrete input it was derived from (memory topic, standard
//! file, reference entry). Nothing here executes code or mutates other state;
//! promotion/archival of the SOP is an explicit separate step.
//!
//! # Persistent capability
//!
//! The generated SOP is also **persisted as a versioned capability** in Route's
//! central (unified) archive — `Documents/Route/route/versions/`, append-only,
//! alongside Route's own standards. This is what makes `route` a *persistent*
//! capability: every SOP iteration is archived, historical, and cross-project
//! discoverable, surviving local `.route/` churn and any single project.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::constitutive::{constitution_path, protocol_path, ReferenceRegistry};
use crate::memory::MemoryStore;

/// Path to the generated SOP inside a project's `.route/`.
pub fn sop_path(project_root: &Path) -> PathBuf {
    project_root.join(".route").join("sop.md")
}

/// Read the current constitution as text (empty when missing).
fn read_standard(path: &Path, label: &str) -> String {
    std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {label}"))
        .unwrap_or_default()
}

/// Build a bullet list of a memory category (decisions/conventions/risks).
fn bullet_items(entries: &[crate::memory::MemoryItem], fallback: &str) -> std::string::String {
    if entries.is_empty() {
        return fallback.to_string();
    }
    let mut out = String::new();
    for item in entries {
        let line = item.content.lines().next().unwrap_or("").trim();
        if !line.is_empty() {
            out.push_str(&format!("- {line}\n"));
        }
    }
    if out.trim().is_empty() {
        fallback.to_string()
    } else {
        out
    }
}

/// Generate a Standard Operating Procedure Markdown document from a Route
/// project's own decision memory and standards. Pure, deterministic, read-only
/// over the project (it never writes; the caller decides where to put the
/// result).
pub fn generate_sop(project_root: &Path) -> Result<String> {
    let memory = MemoryStore::load(project_root).context("failed to load project memory")?;
    let current = memory.current_memory();

    let constitution = read_standard(&constitution_path(project_root), "constitution");
    let protocol = read_standard(&protocol_path(project_root), "protocol");
    let registry =
        ReferenceRegistry::read(project_root).context("failed to read reference registry")?;

    // ---- Distill development constraints from the constitution ----
    let mut constraints: Vec<String> = constitution
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("---"))
        .map(|l| l.trim_start_matches(['-', '*', '>']).trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    // ---- Distill procedure steps from the protocol ----
    let mut steps: Vec<String> = protocol
        .lines()
        .map(|l| l.trim())
        .filter(|l| {
            !l.is_empty() && !l.starts_with('#') && !l.starts_with("---") && !l.starts_with("_")
        })
        .map(|l| {
            l.trim_start_matches([
                '1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '.', ')', '-', '*', '>',
            ])
            .trim()
            .to_string()
        })
        .filter(|l| !l.is_empty())
        .collect();

    // ---- Reference inventory ----
    let mut reference_lines = String::new();
    for e in registry.entries.iter().filter(|e| e.enabled).take(80) {
        let name = if e.name.is_empty() {
            e.id.clone()
        } else {
            format!("{} (`{}`)", e.name, e.id)
        };
        let desc = if e.description.trim().is_empty() {
            String::new()
        } else {
            format!(" — {}", e.description.trim())
        };
        reference_lines.push_str(&format!("- {name}{desc}\n"));
    }
    if reference_lines.trim().is_empty() {
        reference_lines = "(none registered yet)\n".to_string();
    }

    // ---- Memory evidence ----
    let focus = current
        .map(|m| m.current_focus.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "_No current focus recorded._".to_string());
    let decisions = current
        .map(|m| bullet_items(&m.decisions, "_No decisions recorded._"))
        .unwrap_or_else(|| "_No project memory yet._".to_string());
    let conventions = current
        .map(|m| bullet_items(&m.conventions, "_No conventions recorded._"))
        .unwrap_or_else(|| "_No project memory yet._".to_string());
    let risks = current
        .map(|m| bullet_items(&m.known_risks, "_No known risks recorded._"))
        .unwrap_or_else(|| "_No project memory yet._".to_string());

    if constraints.is_empty() {
        constraints.push("_No explicit principles recorded yet._".to_string());
    }
    if steps.is_empty() {
        steps.push("_No explicit procedure steps recorded yet._".to_string());
    }

    // ---- Assemble ----
    let with_label = |items: &[String]| {
        items
            .iter()
            .map(|s| format!("- {s}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let doc = format!(
        "# {name} — Standard Operating Procedure (SOP)\n\
         \n\
         > Auto-generated by Route self-evolution from this project's decision \
         memory and its own standard files. Editable — treat as canonical once approved.\n\
         \n\
         ## 1. Purpose\n\
         \n\
         Constrains how this project is developed: what must hold, what the \
         standard operating procedure is, and which references are in scope.\n\
         \n\
         ## 2. Development Constraints (from constitution)\n\
         \n\
         {constraints}\n\
         \n\
         ## 3. Standard Operating Procedure (from protocol)\n\
         \n\
         {steps}\n\
         \n\
         ## 4. Current Focus\n\
         \n\
         {focus}\n\
         \n\
         ## 5. Project Memory — Decisions\n\
         \n\
         {decisions}\n\
         \n\
         ## 6. Project Memory — Conventions\n\
         \n\
         {conventions}\n\
         \n\
         ## 7. Project Memory — Known Risks\n\
         \n\
         {risks}\n\
         \n\
         ## 8. In-Scope References\n\
         \n\
         {reference_lines}\
         \n\
         ## 9. Evidence Sources\n\
         \n\
         - `.route/constitution.md`\n\
         - `.route/protocol.md`\n\
         - `.route/reference/registry.json`\n\
         - `.route/memory.json` (decision memory)\n",
        name = "Route",
        constraints = with_label(&constraints),
        steps = with_label(&steps),
        focus = focus,
        decisions = decisions,
        conventions = conventions,
        risks = risks,
        reference_lines = reference_lines,
    );

    Ok(doc)
}

/// Generate and write the SOP to `.route/sop.md`, returning the written path.
pub fn generate_and_write(project_root: &Path) -> Result<PathBuf> {
    let doc = generate_sop(project_root)?;
    let path = sop_path(project_root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    crate::constitutive::write_atomic(&path, doc.as_bytes())?;
    Ok(path)
}

/// Persist the generated SOP as a **versioned persistent capability** in the
/// central (unified) archive.
///
/// Reuses Route's append-only self-archive (`Documents/Route/route/versions/`):
/// the SOP plus the standard files it was derived from are archived as one
/// version under `capability: sop`. Nothing is ever overwritten; every call
/// appends a new version, so the SOP capability grows and stays rollback-able.
/// Returns the archived version sequence.
pub fn persist_sop_capability(project_root: &Path) -> Result<u64> {
    let mut files: HashMap<String, String> = HashMap::new();
    match std::fs::read_to_string(sop_path(project_root)) {
        Ok(sop) => {
            files.insert("sop.md".to_string(), sop);
        }
        Err(_) => {
            // No SOP written yet — generate it on the fly so the persisted
            // capability never lags the project's current standards.
            let doc = generate_sop(project_root)?;
            files.insert("sop.md".to_string(), doc);
        }
    }
    // Include the standard files the SOP was derived from (best effort).
    for (logical, path) in [
        ("constitution.md", constitution_path(project_root)),
        ("protocol.md", protocol_path(project_root)),
    ] {
        if let Ok(content) = std::fs::read_to_string(&path) {
            files.insert(logical.to_string(), content);
        }
    }

    let version =
        crate::self_archive::archive_current(&files, env!("CARGO_PKG_VERSION"), "capability: sop")?;
    Ok(version.meta.seq)
}

/// The most recent persisted SOP capability version, if any.
pub fn latest_persisted_sop_seq() -> Result<Option<u64>> {
    Ok(crate::self_archive::list()?
        .into_iter()
        .filter(|m| m.message == "capability: sop")
        .map(|m| m.seq)
        .last())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryItem, MemoryItemKind, ProjectMemory};
    use tempfile::tempdir;

    fn seed_route_project(root: &Path) {
        let dot = root.join(".route");
        std::fs::create_dir_all(&dot).unwrap();
        std::fs::write(
            dot.join("constitution.md"),
            "## Data safety\n- Never delete data.\n## User control\n- No silent behaviour change.\n",
        )
        .unwrap();
        std::fs::write(
            dot.join("protocol.md"),
            "1. Understand the task.\n2. Create a snapshot before large changes.\n3. Run tests.\n",
        )
        .unwrap();

        let now = 1_700_000_000_000_i64;
        let mut store = MemoryStore::load(root).unwrap();
        store.memories.push(ProjectMemory {
            summary: "route self-evolution".to_string(),
            current_architecture: String::new(),
            decisions: vec![],
            known_risks: vec![],
            failed_attempts: vec![],
            conventions: vec![MemoryItem {
                id: route_core::hash::new_id(),
                kind: MemoryItemKind::Invariant,
                content: "Keep self-evolution evidence-based".to_string(),
                source_ids: vec![],
                confidence: 0.9,
                updated_at: now,
                created_at: now,
                superseded_by: None,
                supersedes: None,
                tags: vec![],
            }],
            open_questions: vec![],
            current_focus: "self-evolution SOP".to_string(),
            updated_at: now,
            version: 1,
            items: vec![],
        });
        store.current = Some((now).to_string());
        store.save(root).unwrap();
    }

    #[test]
    fn generates_sop_from_memory_and_standards() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        seed_route_project(root);

        let doc = generate_sop(root).unwrap();
        assert!(doc.contains("Standard Operating Procedure"));
        assert!(doc.contains("Never delete data"));
        assert!(doc.contains("snapshot before large changes"));
        assert!(doc.contains("Current Focus"));
        assert!(doc.contains("self-evolution SOP"));
        // Evidence sources are listed.
        assert!(doc.contains(".route/constitution.md"));
        assert!(doc.contains(".route/memory.json"));
    }

    #[test]
    fn generate_and_write_produces_sop_file() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        seed_route_project(root);

        let path = generate_and_write(root).unwrap();
        assert_eq!(path, sop_path(root));
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.starts_with("# Route — Standard Operating Procedure"));
    }

    #[test]
    fn empty_project_still_yields_documented_placeholders() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".route")).unwrap();

        let doc = generate_sop(root).unwrap();
        assert!(doc.contains("_No explicit principles recorded yet._"));
        assert!(doc.contains("_No project memory yet._"));
    }

    #[test]
    fn persist_sop_capability_appends_versioned_archive_entry() {
        // Persistence writes to the central archive root, so isolate it.
        let _guard = crate::game_save::TestArchiveRootGuard::new();
        let dir = tempdir().unwrap();
        let root = dir.path();
        seed_route_project(root);

        // Generates + persists on the fly, as a versioned capability.
        let seq1 = persist_sop_capability(root).unwrap();
        assert_eq!(seq1, 1);
        assert_eq!(latest_persisted_sop_seq().unwrap(), Some(1));

        // A second run appends (never overwrites).
        let seq2 = persist_sop_capability(root).unwrap();
        assert_eq!(seq2, 2);
        assert!(crate::self_archive::list()
            .unwrap()
            .iter()
            .any(|m| { m.message == "capability: sop" && m.seq == 2 }));
        // The archived version stores the sop + the standard files it derived
        // from, so the capability is fully reconstructable (rollback-able).
        let v = crate::self_archive::get(seq2).unwrap().unwrap();
        assert!(v.file_contents.contains_key("sop.md"));
        assert!(v.file_contents.contains_key("constitution.md"));
        assert!(v.file_contents.contains_key("protocol.md"));
    }
}
