//! Sync modes and conflict resolution strategies.

use serde::{Deserialize, Serialize};

/// Sync mode — determines how files are copied from source to target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMode {
    /// Complete copy of latest state. Source overwrites target completely.
    /// Files in target that don't exist in source are deleted.
    Mirror,
    /// Append-only. Never deletes files. On conflict, use ConflictResolution.
    Backup,
    /// Strict versioning. Each sync creates a timestamped snapshot folder.
    /// Custom rules control retention and naming.
    Archive,
}

impl SyncMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncMode::Mirror => "mirror",
            SyncMode::Backup => "backup",
            SyncMode::Archive => "archive",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "mirror" => Some(SyncMode::Mirror),
            "backup" => Some(SyncMode::Backup),
            "archive" => Some(SyncMode::Archive),
            _ => None,
        }
    }
}

impl Default for SyncMode {
    fn default() -> Self {
        SyncMode::Backup
    }
}

/// How to handle conflicts in Backup mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Keep both files — rename the target file with a suffix.
    KeepBoth,
    /// Skip the conflicting file — keep the existing target.
    SkipExisting,
    /// Overwrite the target with the source.
    Overwrite,
}

impl Default for ConflictResolution {
    fn default() -> Self {
        ConflictResolution::KeepBoth
    }
}

impl ConflictResolution {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConflictResolution::KeepBoth => "keep_both",
            ConflictResolution::SkipExisting => "skip",
            ConflictResolution::Overwrite => "overwrite",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "keep_both" => Some(ConflictResolution::KeepBoth),
            "skip" => Some(ConflictResolution::SkipExisting),
            "overwrite" => Some(ConflictResolution::Overwrite),
            _ => None,
        }
    }
}

/// Rules for Archive mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveRule {
    /// Format for snapshot folder names (chrono format string).
    /// Default: "%Y%m%d_%H%M%S"
    pub folder_format: String,
    /// Maximum number of snapshots to keep. None = unlimited.
    pub max_snapshots: Option<usize>,
    /// Maximum age in days. None = unlimited.
    pub max_age_days: Option<u32>,
}

impl Default for ArchiveRule {
    fn default() -> Self {
        Self {
            folder_format: "%Y%m%d_%H%M%S".to_string(),
            max_snapshots: None,
            max_age_days: None,
        }
    }
}
