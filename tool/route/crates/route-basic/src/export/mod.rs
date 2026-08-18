//! Export framework + built-in exporters.
//!
//! Text formats (JSON / Markdown / Mermaid / Emacs org) implement the `Exporter` trait
//! and write to any `dyn Write`. File-based formats (ZIP / Folder) are side-effecting
//! operations handled by `BasicRepository::export_zip` and `BasicRepository::export_folder`.

pub mod exporters;

use std::io::Write;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::models::{Branch, Commit, CommitPathAnnotation, Snapshot};

/// Supported export formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Markdown,
    Mermaid,
    EmacsOrg,
    /// ZIP archive containing all metadata + blob files.
    Zip,
    /// Folder copy containing all metadata + blob files.
    Folder,
}

impl ExportFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Markdown => "markdown",
            ExportFormat::Mermaid => "mermaid",
            ExportFormat::EmacsOrg => "emacs",
            ExportFormat::Zip => "zip",
            ExportFormat::Folder => "folder",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "json" => Some(ExportFormat::Json),
            "markdown" | "md" => Some(ExportFormat::Markdown),
            "mermaid" | "mmd" => Some(ExportFormat::Mermaid),
            "emacs" | "org" => Some(ExportFormat::EmacsOrg),
            "zip" => Some(ExportFormat::Zip),
            "folder" => Some(ExportFormat::Folder),
            _ => None,
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            ExportFormat::Json => "json",
            ExportFormat::Markdown => "md",
            ExportFormat::Mermaid => "mmd",
            ExportFormat::EmacsOrg => "org",
            ExportFormat::Zip => "zip",
            ExportFormat::Folder => "", // directory, no extension
        }
    }

    /// Whether this format produces a file/directory on disk (rather than text to stdout).
    pub fn is_file_based(&self) -> bool {
        matches!(self, ExportFormat::Zip | ExportFormat::Folder)
    }
}

/// Context passed to exporters — contains everything they need.
#[derive(Debug, Clone, Serialize)]
pub struct ExportContext {
    pub project_name: String,
    pub project_path: String,
    pub branches: Vec<Branch>,
    pub snapshots: Vec<Snapshot>,
    pub commits: Vec<Commit>,
    /// commit_id → annotations
    pub annotations: std::collections::HashMap<String, Vec<CommitPathAnnotation>>,
}

/// Exporter trait — produces a single text document for a given format.
/// File-based formats (ZIP, Folder) are handled separately by `BasicRepository`.
pub trait Exporter {
    fn format(&self) -> ExportFormat;
    fn export(&self, ctx: &ExportContext, out: &mut dyn Write) -> Result<()>;
}
