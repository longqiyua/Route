//! Built-in exporters: JSON, Markdown, Mermaid mindmap, Emacs org-mode.

use std::io::Write;

use anyhow::Result;
use chrono::{DateTime, Utc};

use super::{ExportContext, ExportFormat, Exporter};
use crate::models::{BranchKind, CommitKind};

/// Container for all default exporters.
pub struct DefaultExporters;

impl DefaultExporters {
    pub fn all() -> Vec<Box<dyn Exporter>> {
        vec![
            Box::new(JsonExporter),
            Box::new(MarkdownExporter),
            Box::new(MermaidExporter),
            Box::new(EmacsOrgExporter),
        ]
    }

    pub fn for_format(fmt: ExportFormat) -> Option<Box<dyn Exporter>> {
        match fmt {
            ExportFormat::Json => Some(Box::new(JsonExporter)),
            ExportFormat::Markdown => Some(Box::new(MarkdownExporter)),
            ExportFormat::Mermaid => Some(Box::new(MermaidExporter)),
            ExportFormat::EmacsOrg => Some(Box::new(EmacsOrgExporter)),
            // ZIP and Folder are file-based — handled by BasicRepository methods, not the Exporter trait.
            ExportFormat::Zip | ExportFormat::Folder => None,
        }
    }
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

pub struct JsonExporter;

impl Exporter for JsonExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::Json
    }

    fn export(&self, ctx: &ExportContext, out: &mut dyn Write) -> Result<()> {
        let json = serde_json::to_string_pretty(ctx)?;
        writeln!(out, "{json}")?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

pub struct MarkdownExporter;

impl Exporter for MarkdownExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::Markdown
    }

    fn export(&self, ctx: &ExportContext, out: &mut dyn Write) -> Result<()> {
        writeln!(out, "# Route 报告 — {}", ctx.project_name)?;
        writeln!(out)?;
        writeln!(out, "项目路径：`{}`", ctx.project_path)?;
        writeln!(out)?;

        writeln!(out, "## 分支")?;
        for b in &ctx.branches {
            let head = b
                .head_snapshot
                .as_deref()
                .map(|s| &s[..8])
                .unwrap_or("(空)");
            writeln!(
                out,
                "- **{}** ({}) — HEAD: `{}`",
                b.name,
                b.kind.as_str(),
                head
            )?;
            if let Some(parent) = &b.parent_branch {
                let parent_branch = ctx
                    .branches
                    .iter()
                    .find(|x| x.id == *parent)
                    .map(|x| x.name.as_str())
                    .unwrap_or("?");
                writeln!(out, "  - 父分支：`{}`", parent_branch)?;
            }
            if let Some(base) = &b.baseline_snapshot {
                writeln!(out, "  - Baseline: `{}`", &base[..8.min(base.len())])?;
            }
        }
        writeln!(out)?;

        writeln!(out, "## 历史（最新优先）")?;
        writeln!(
            out,
            "| Commit | Snapshot | 分支 | 类型 | 时间 | Message | Diff |"
        )?;
        writeln!(out, "|---|---|---|---|---|---|---|")?;
        for c in &ctx.commits {
            let branch_name = ctx
                .branches
                .iter()
                .find(|b| b.id == c.branch_id)
                .map(|b| b.name.as_str())
                .unwrap_or("?");
            let time = format_ts(c.created_at);
            let diff_short = c
                .diff_summary
                .as_deref()
                .map(|s| {
                    serde_json::from_str::<crate::models::DiffSummary>(s)
                        .map(|d| d.short())
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            writeln!(
                out,
                "| `{}` | `{}` | {} | {} | {} | {} | {} |",
                &c.id[..8.min(c.id.len())],
                &c.to_snapshot[..8.min(c.to_snapshot.len())],
                branch_name,
                c.kind.as_str(),
                time,
                escape_md(&c.message),
                diff_short,
            )?;
        }
        writeln!(out)?;

        // Path annotations
        let has_annotations = ctx.annotations.values().any(|v| !v.is_empty());
        if has_annotations {
            writeln!(out, "## 路径注释（边附带信息）")?;
            for (commit_id, anns) in &ctx.annotations {
                if anns.is_empty() {
                    continue;
                }
                writeln!(out, "### Commit `{}`", &commit_id[..8.min(commit_id.len())])?;
                for a in anns {
                    writeln!(
                        out,
                        "- {} — {}",
                        format_ts(a.created_at),
                        escape_md(&a.text)
                    )?;
                }
                writeln!(out)?;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Mermaid mindmap — innovation point: commit message lives ON the edge
// (as `::commit <text>` annotation), not as node metadata.
// ---------------------------------------------------------------------------

pub struct MermaidExporter;

impl Exporter for MermaidExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::Mermaid
    }

    fn export(&self, ctx: &ExportContext, out: &mut dyn Write) -> Result<()> {
        writeln!(out, "```mermaid")?;
        writeln!(out, "mindmap")?;
        writeln!(out, "  root(({}))", escape_mermaid(&ctx.project_name))?;

        // Group commits by branch
        let mut by_branch: std::collections::HashMap<String, Vec<&crate::models::Commit>> =
            std::collections::HashMap::new();
        for c in &ctx.commits {
            by_branch.entry(c.branch_id.clone()).or_default().push(c);
        }

        for branch in &ctx.branches {
            let branch_commits = by_branch.get(&branch.id).cloned().unwrap_or_default();
            let indent = "    ";
            let kind_label = match branch.kind {
                BranchKind::Main => "main".to_string(),
                BranchKind::Inherited => "inherited".to_string(),
                BranchKind::Sandbox => "sandbox".to_string(),
            };
            writeln!(out, "{indent}{}[{}]", branch.name, kind_label)?;

            if let Some(parent) = &branch.parent_branch {
                let parent_name = ctx
                    .branches
                    .iter()
                    .find(|b| b.id == *parent)
                    .map(|b| b.name.as_str())
                    .unwrap_or("?");
                match branch.kind {
                    BranchKind::Inherited => {
                        if let Some(base) = &branch.baseline_snapshot {
                            writeln!(
                                out,
                                "{indent}{indent}::baseline {}",
                                &base[..8.min(base.len())]
                            )?;
                        }
                    }
                    BranchKind::Sandbox => {
                        writeln!(out, "{indent}{indent}::copied from {}", parent_name)?;
                    }
                    _ => {}
                }
            }

            // Build commit chain: walk from HEAD backwards via from_snapshot
            let mut chain: Vec<&crate::models::Commit> = branch_commits
                .iter()
                .filter(|c| c.kind != CommitKind::Full)
                .copied()
                .collect();
            chain.sort_by(|a, b| a.created_at.cmp(&b.created_at));

            let mut depth = 2;
            for c in chain {
                let pad = "  ".repeat(depth + 2);
                let to_id = &c.to_snapshot;
                let short = &to_id[..8.min(to_id.len())];
                let msg = escape_mermaid(&c.message);
                let kind_tag = match c.kind {
                    CommitKind::Incremental => "".to_string(),
                    CommitKind::Full => "【全量备份】".to_string(),
                    CommitKind::Merge => "【合并】".to_string(),
                    CommitKind::Rollback => "【回退】".to_string(),
                };

                writeln!(out, "{pad}{}", short)?;
                writeln!(out, "{pad}  ::commit {}{}", kind_tag, msg)?;

                // Path annotations on this edge
                if let Some(anns) = ctx.annotations.get(&c.id) {
                    for a in anns {
                        writeln!(out, "{pad}  ::note {}", escape_mermaid(&a.text))?;
                    }
                }

                depth += 1;
            }
        }

        writeln!(out, "```")?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Emacs org-mode
// ---------------------------------------------------------------------------

pub struct EmacsOrgExporter;

impl Exporter for EmacsOrgExporter {
    fn format(&self) -> ExportFormat {
        ExportFormat::EmacsOrg
    }

    fn export(&self, ctx: &ExportContext, out: &mut dyn Write) -> Result<()> {
        writeln!(out, "* Route 项目 — {}", ctx.project_name)?;
        writeln!(out, ":PROPERTIES:")?;
        writeln!(out, ":ROUTE_MODE: basic")?;
        writeln!(out, ":ROUTE_PROJECT_PATH: {}", ctx.project_path)?;
        writeln!(
            out,
            ":ROUTE_CREATED: {}",
            format_ts(ctx.snapshots.first().map(|s| s.created_at).unwrap_or(0))
        )?;
        writeln!(out, ":END:")?;
        writeln!(out)?;

        for branch in &ctx.branches {
            writeln!(out, "** {} 分支", branch.name)?;
            writeln!(out, ":PROPERTIES:")?;
            writeln!(out, ":ROUTE_BRANCH_KIND: {}", branch.kind.as_str())?;
            if let Some(h) = &branch.head_snapshot {
                writeln!(out, ":ROUTE_HEAD: {}", h)?;
            }
            if let Some(p) = &branch.parent_branch {
                let parent_name = ctx
                    .branches
                    .iter()
                    .find(|b| b.id == *p)
                    .map(|b| b.name.as_str())
                    .unwrap_or("?");
                writeln!(out, ":ROUTE_PARENT: {}", parent_name)?;
            }
            if let Some(b) = &branch.baseline_snapshot {
                writeln!(out, ":ROUTE_BASELINE: {}", b)?;
            }
            writeln!(out, ":END:")?;
            writeln!(out)?;

            // List commits on this branch as sub-headings
            let mut branch_commits: Vec<&crate::models::Commit> = ctx
                .commits
                .iter()
                .filter(|c| c.branch_id == branch.id)
                .collect();
            branch_commits.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            for c in branch_commits {
                let short_to = &c.to_snapshot[..8.min(c.to_snapshot.len())];
                let short_from = &c.from_snapshot[..8.min(c.from_snapshot.len())];
                writeln!(out, "*** Commit {}", escape_org(&c.message))?;
                writeln!(out, ":PROPERTIES:")?;
                writeln!(out, ":ROUTE_KIND: {}", c.kind.as_str())?;
                writeln!(out, ":ROUTE_FROM: {}", short_from)?;
                writeln!(out, ":ROUTE_TO: {}", short_to)?;
                writeln!(out, ":ROUTE_TIME: {}", format_ts(c.created_at))?;
                if let Some(diff) = &c.diff_summary {
                    if let Ok(d) = serde_json::from_str::<crate::models::DiffSummary>(diff) {
                        writeln!(out, ":ROUTE_DIFF: {}", d.short())?;
                    }
                }
                writeln!(out, ":END:")?;

                // Path annotations as list items
                if let Some(anns) = ctx.annotations.get(&c.id) {
                    for a in anns {
                        writeln!(out, "- 路径注释 :: {}", escape_org(&a.text))?;
                    }
                }
                writeln!(out)?;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_ts(millis: i64) -> String {
    let dt = DateTime::<Utc>::from_timestamp_millis(millis).unwrap_or_default();
    dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
}

fn escape_md(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn escape_mermaid(s: &str) -> String {
    // Mermaid mindmap is sensitive to parentheses and brackets
    s.replace(['(', ')', '[', ']', '{', '}', '\n'], " ")
        .replace('"', "'")
}

fn escape_org(s: &str) -> String {
    // Org-mode headlines can't contain certain chars
    s.replace('\n', " ")
}
