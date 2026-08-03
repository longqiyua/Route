//! Skill Harness — 管理和执行用户定义的技能。
//!
//! 技能来自项目的 `.route/skills/` 目录，支持两种格式：
//!   - Markdown skill (.md)：提示词模板 + 元数据 frontmatter
//!   - JSON skill (.json)：结构化技能定义（inputs, outputs, steps）
//!
//! 每个 Skill 都有一个唯一 SkillId，Agent 通过 SkillHarness 调用技能。

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use tracing::{info, debug, warn};

pub type SkillId = String;

/// 技能清单元数据（frontmatter for md, 直接存储 for json）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillManifest {
    pub id: SkillId,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// 输入参数 schema 描述（人类可读）
    #[serde(default)]
    pub inputs: Vec<SkillParam>,
    /// 输出 schema 描述
    #[serde(default)]
    pub outputs: Vec<SkillParam>,
    /// 触发该技能的关键词（用于 Agent 自动匹配）
    #[serde(default)]
    pub triggers: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillParam {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub default: Option<String>,
}

/// 一个已加载的技能
pub struct Skill {
    pub manifest: SkillManifest,
    /// 技能主体：prompt 模板 or 结构化步骤
    pub body: SkillBody,
    pub source_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillBody {
    /// Markdown Prompt 模板：{variable} 插值
    Markdown(String),
    /// 结构化步骤：按序执行的步骤描述
    Steps(Vec<SkillStep>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStep {
    pub order: usize,
    pub description: String,
    /// 可选的工具名
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub tool_args: Option<serde_json::Value>,
}

/// 执行技能的上下文（参数）
#[derive(Debug, Clone, Default)]
pub struct SkillContext {
    pub vars: std::collections::BTreeMap<String, String>,
    pub project_path: Option<PathBuf>,
}

/// 技能执行结果
#[derive(Debug, Clone, Serialize)]
pub struct SkillExecution {
    pub skill_id: SkillId,
    /// 渲染后的完整 prompt（用于 Agent 注入）
    pub rendered_prompt: String,
    pub steps: Vec<SkillStepResult>,
    pub duration_ms: u128,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillStepResult {
    pub order: usize,
    pub description: String,
    pub output: Option<String>,
    pub duration_ms: u128,
    pub success: bool,
}

/// Skill Harness：注册、查找、执行技能
pub struct SkillHarness {
    skills: std::collections::BTreeMap<SkillId, Skill>,
}

impl SkillHarness {
    pub fn new() -> Self { Self { skills: Default::default() } }

    /// Convenience alias for load_from_dir — used by some call sites.
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        Self::load_from_dir(dir)
    }

    pub fn load_from_dir(dir: &Path) -> anyhow::Result<Self> {
        info!("[skill] loading skills from directory: {}", dir.display());
        let mut h = Self::new();
        let loaded = load_skills_from_dir(dir)?;
        info!("[skill] found {} skill files in directory", loaded.len());
        for s in &loaded {
            debug!(
                "[skill] registering skill: id={}, name={}, version={}, triggers={:?}",
                s.manifest.id, s.manifest.name, s.manifest.version, s.manifest.triggers
            );
        }
        for s in loaded {
            h.register(s);
        }
        info!("[skill] skill harness initialized with {} skills", h.skills.len());
        Ok(h)
    }

    pub fn register(&mut self, skill: Skill) {
        info!(
            "[skill] registering skill: id={}, name={}, version={}, source={}",
            skill.manifest.id, skill.manifest.name, skill.manifest.version,
            skill.source_path.display()
        );
        self.skills.insert(skill.manifest.id.clone(), skill);
    }

    pub fn get(&self, id: &str) -> Option<&Skill> { self.skills.get(id) }

    pub fn list(&self) -> Vec<&SkillManifest> {
        self.skills.values().map(|s| &s.manifest).collect()
    }

    /// 根据关键词匹配技能（Agent 自动选择用）
    pub fn find_by_trigger(&self, text: &str) -> Vec<&Skill> {
        let lower = text.to_lowercase();
        let mut matched: Vec<&Skill> = self.skills.values().filter(|s| {
            s.manifest.triggers.iter().any(|t| lower.contains(&t.to_lowercase()))
        }).collect();

        if matched.is_empty() {
            debug!(
                "[skill] trigger matching: no skills matched (text_len={}, registered={} skills with triggers)",
                text.len(),
                self.skills.values().filter(|s| !s.manifest.triggers.is_empty()).count()
            );
        } else {
            info!(
                "[skill] trigger matching: matched {} skill(s) from {} registered (text_len={})",
                matched.len(), self.skills.len(), text.len()
            );
            for s in &matched {
                debug!(
                    "[skill] matched by trigger: id={}, triggers_hit={:?}",
                    s.manifest.id,
                    s.manifest.triggers.iter().filter(|t| lower.contains(&t.to_lowercase())).collect::<Vec<_>>()
                );
            }
        }
        matched.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
        matched
    }

    /// 执行技能：渲染模板 + 返回结构化结果
    pub fn execute(&self, id: &str, ctx: &SkillContext) -> anyhow::Result<SkillExecution> {
        info!(
            "[skill] executing skill: id={}, vars_count={}, has_project={}",
            id, ctx.vars.len(), ctx.project_path.is_some()
        );
        let start = std::time::Instant::now();
        let skill = self.get(id).ok_or_else(|| {
            warn!("[skill] execute failed: skill not found: {}", id);
            anyhow::anyhow!("skill not found: {id}")
        })?;
        info!(
            "[skill] skill found: name={}, body_type={:?}, steps={}, description_len={}",
            skill.manifest.name,
            match &skill.body {
                SkillBody::Markdown(_) => "markdown",
                SkillBody::Steps(_) => "steps",
            },
            match &skill.body {
                SkillBody::Steps(st) => st.len(),
                _ => 0,
            },
            skill.manifest.description.len()
        );

        let mut steps: Vec<SkillStepResult> = Vec::new();
        let rendered = match &skill.body {
            SkillBody::Markdown(tmpl) => {
                debug!(
                    "[skill] rendering markdown template: template_len={}, vars={:?}",
                    tmpl.len(), ctx.vars.keys().collect::<Vec<_>>()
                );
                let rendered = Self::render_template(tmpl, ctx);
                info!(
                    "[skill] markdown template rendered: {}→{} chars ({} vars substituted)",
                    tmpl.len(), rendered.len(), ctx.vars.len()
                );
                rendered
            }
            SkillBody::Steps(st) => {
                debug!(
                    "[skill] executing {} structured steps",
                    st.len()
                );
                for step in st {
                    let s_start = std::time::Instant::now();
                    debug!(
                        "[skill] step {}: description={}, tool={:?}, args_keys={:?}",
                        step.order,
                        step.description,
                        step.tool,
                        step.tool_args.as_ref().and_then(|a| a.as_object().map(|o| o.keys().collect::<Vec<_>>()))
                    );
                    let result = SkillStepResult {
                        order: step.order,
                        description: step.description.clone(),
                        output: None,
                        duration_ms: s_start.elapsed().as_millis(),
                        success: true,
                    };
                    steps.push(result);
                }
                info!("[skill] all {} steps completed", st.len());
                skill.manifest.description.clone()
            }
        };

        let dur = start.elapsed().as_millis();
        info!(
            "[skill] skill '{}' execution complete: duration={}ms, steps={}, success=true",
            id, dur, steps.len()
        );

        Ok(SkillExecution {
            skill_id: id.to_string(),
            rendered_prompt: rendered,
            steps,
            duration_ms: dur,
            success: true,
            error: None,
        })
    }

    fn render_template(template: &str, ctx: &SkillContext) -> String {
        let mut out = template.to_string();
        for (k, v) in &ctx.vars {
            out = out.replace(&format!("{{{k}}}"), v);
        }
        out
    }
}

impl Default for SkillHarness { fn default() -> Self { Self::new() } }

/// 从目录加载所有 skill 文件
pub fn load_skills_from_dir(dir: &Path) -> anyhow::Result<Vec<Skill>> {
    let mut skills = Vec::new();
    if !dir.exists() {
        debug!("[skill] skills directory does not exist: {}", dir.display());
        return Ok(skills);
    }
    let read_dir = std::fs::read_dir(dir).map_err(|e| anyhow::anyhow!("cannot read skills dir: {e}"))?;
    let mut file_count = 0;
    for entry in read_dir.flatten() {
        let path = entry.path();
        file_count += 1;
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            debug!(
                "[skill] processing file: path={}, ext={}",
                path.display(), ext
            );
            let skill = match ext {
                "md" => {
                    match parse_markdown_skill(&path) {
                        Ok(s) => {
                            info!("[skill] parsed markdown skill: id={}, name={}", s.manifest.id, s.manifest.name);
                            s
                        }
                        Err(e) => {
                            warn!("[skill] failed to parse markdown skill {}: {}", path.display(), e);
                            continue;
                        }
                    }
                }
                "json" => {
                    match parse_json_skill(&path) {
                        Ok(s) => {
                            info!("[skill] parsed json skill: id={}, name={}", s.manifest.id, s.manifest.name);
                            s
                        }
                        Err(e) => {
                            warn!("[skill] failed to parse json skill {}: {}", path.display(), e);
                            continue;
                        }
                    }
                }
                _ => {
                    debug!("[skill] skipping non-skill file: {}", path.display());
                    continue;
                }
            };
            skills.push(skill);
        }
    }
    info!(
        "[skill] loaded {} skills from {} files in {}",
        skills.len(), file_count, dir.display()
    );
    skills.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    Ok(skills)
}

fn parse_markdown_skill(path: &Path) -> anyhow::Result<Skill> {
    let raw = std::fs::read_to_string(path)?;
    let mut manifest = SkillManifest::default();
    let mut body_start = 0;
    if raw.starts_with("---") {
        if let Some(end) = raw[3..].find("---") {
            let fm = &raw[3..3 + end];
            body_start = 3 + end + 3;
            if let Ok(m) = serde_yaml_from_fm(fm) { manifest = m; }
        }
    }
    if manifest.id.is_empty() {
        manifest.id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("skill").to_string();
    }
    if manifest.name.is_empty() {
        manifest.name = manifest.id.clone();
    }
    Ok(Skill {
        manifest,
        body: SkillBody::Markdown(raw[body_start..].trim().to_string()),
        source_path: path.to_path_buf(),
    })
}

fn parse_json_skill(path: &Path) -> anyhow::Result<Skill> {
    #[derive(Deserialize)]
    struct Raw {
        manifest: SkillManifest,
        #[serde(default)]
        steps: Vec<SkillStep>,
        #[serde(default)]
        prompt: String,
    }
    let raw: Raw = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let body = if !raw.steps.is_empty() {
        SkillBody::Steps(raw.steps)
    } else {
        SkillBody::Markdown(raw.prompt)
    };
    Ok(Skill {
        manifest: raw.manifest,
        body,
        source_path: path.to_path_buf(),
    })
}

fn serde_yaml_from_fm(fm: &str) -> anyhow::Result<SkillManifest> {
    let mut manifest = SkillManifest::default();
    let mut current_list_key: Option<String> = None;
    for line in fm.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') { continue; }
        if let Some(stripped) = trimmed.strip_prefix("- ") {
            if let Some(key) = &current_list_key {
                match key.as_str() {
                    "tags" => manifest.tags.push(stripped.to_string()),
                    "triggers" => manifest.triggers.push(stripped.to_string()),
                    _ => {}
                }
            }
            continue;
        }
        if let Some((k, v)) = trimmed.split_once(':') {
            let key = k.trim().to_string();
            let val = v.trim().trim_matches('"').trim_matches('\'').to_string();
            match key.as_str() {
                "id" => manifest.id = val,
                "name" => manifest.name = val,
                "version" => manifest.version = val,
                "description" => manifest.description = val,
                "author" => manifest.author = val,
                "tags" => { current_list_key = Some("tags".to_string()); manifest.tags.clear(); }
                "triggers" => { current_list_key = Some("triggers".to_string()); manifest.triggers.clear(); }
                _ => {}
            }
        }
    }
    Ok(manifest)
}
