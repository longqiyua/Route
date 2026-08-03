//! 因果控制/副作用检测
//!
//! 提供因果事件追踪、模式匹配、分支决策和迭代追踪能力。

use std::fmt;

/// 因果事件
#[derive(Debug, Clone)]
pub struct CausalEvent {
    pub id: String,
    pub action: String,
    pub file: String,
    pub timestamp: String,
    pub side_effects: Vec<String>,
}

/// 因果规则
pub enum CausalRule {
    /// 匹配文件模式
    FilePattern(String),
    /// 匹配目录
    Directory(String),
    /// 匹配操作类型
    Action(String),
    /// 自定义规则
    Custom(Box<dyn Fn(&CausalEvent) -> bool + Send>),
}

impl fmt::Debug for CausalRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CausalRule::FilePattern(p) => f.debug_tuple("FilePattern").field(p).finish(),
            CausalRule::Directory(d) => f.debug_tuple("Directory").field(d).finish(),
            CausalRule::Action(a) => f.debug_tuple("Action").field(a).finish(),
            CausalRule::Custom(_) => f.debug_tuple("Custom").field(&"<closure>").finish(),
        }
    }
}

/// 分支决策
#[derive(Debug, Clone, PartialEq)]
pub enum BranchDecision {
    Continue,
    Rollback(String),
    Review(String),
}

/// 因果控制器
#[derive(Debug)]
pub struct CausalController {
    pub history: Vec<CausalEvent>,
    pub rules: Vec<CausalRule>,
}

impl CausalController {
    /// 创建新的因果控制器
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
            rules: Vec::new(),
        }
    }

    /// 注册因果规则
    pub fn register_rule(&mut self, rule: CausalRule) {
        self.rules.push(rule);
    }

    /// 记录因果事件
    pub fn record_event(&mut self, event: CausalEvent) {
        // 如果存在相同 ID 的事件，替换之
        if let Some(pos) = self.history.iter().position(|e| e.id == event.id) {
            self.history[pos] = event;
        } else {
            self.history.push(event);
        }
    }

    /// 检测副作用：找出与指定文件或操作相关的所有事件
    pub fn detect_side_effects(&self, file_or_action: &str) -> Vec<String> {
        let mut effects = Vec::new();

        for event in &self.history {
            // 检查文件是否匹配
            if event.file.contains(file_or_action) || file_or_action.contains(&event.file) {
                for effect in &event.side_effects {
                    if !effects.contains(effect) {
                        effects.push(effect.clone());
                    }
                }
            }

            // 检查操作是否匹配
            if event.action.contains(file_or_action) {
                for effect in &event.side_effects {
                    if !effects.contains(effect) {
                        effects.push(effect.clone());
                    }
                }
            }

            // 检查自定义规则
            for rule in &self.rules {
                match rule {
                    CausalRule::FilePattern(pattern) => {
                        if event.file.contains(pattern) {
                            for effect in &event.side_effects {
                                if !effects.contains(effect) {
                                    effects.push(effect.clone());
                                }
                            }
                        }
                    }
                    CausalRule::Directory(dir) => {
                        if event.file.starts_with(dir) {
                            for effect in &event.side_effects {
                                if !effects.contains(effect) {
                                    effects.push(effect.clone());
                                }
                            }
                        }
                    }
                    CausalRule::Action(action) => {
                        if event.action.contains(action) {
                            for effect in &event.side_effects {
                                if !effects.contains(effect) {
                                    effects.push(effect.clone());
                                }
                            }
                        }
                    }
                    CausalRule::Custom(f) => {
                        if f(event) {
                            for effect in &event.side_effects {
                                if !effects.contains(effect) {
                                    effects.push(effect.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        effects
    }

    /// 模式匹配：检查变更是否符合已有模式
    pub fn pattern_match(&self, action: &str) -> Vec<&CausalEvent> {
        self.history
            .iter()
            .filter(|e| e.action.contains(action) || action.contains(&e.action))
            .collect()
    }

    /// 分支选择：推荐回滚还是继续
    pub fn branch_decision(&self, event: &CausalEvent) -> BranchDecision {
        // 如果有严重的副作用，建议回滚
        let has_critical_side_effects = event
            .side_effects
            .iter()
            .any(|e| e.contains("error") || e.contains("conflict") || e.contains("break"));

        if has_critical_side_effects {
            return BranchDecision::Rollback(format!(
                "Critical side effects detected: {:?}",
                event.side_effects
            ));
        }

        // 检查是否有类似的历史事件导致问题
        let similar = self.pattern_match(&event.action);
        let problematic_history = similar.iter().any(|e| {
            e.side_effects
                .iter()
                .any(|s| s.contains("error") || s.contains("conflict"))
        });

        if problematic_history {
            BranchDecision::Review(format!(
                "Similar past actions had side effects: {:?}",
                event.side_effects
            ))
        } else {
            BranchDecision::Continue
        }
    }

    /// 迭代追踪：同一文件的连续变更序列
    pub fn iteration_trace(&self, file: &str) -> Vec<&CausalEvent> {
        self.history
            .iter()
            .filter(|e| e.file == file || e.file.contains(file))
            .collect()
    }
}

impl Default for CausalController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_causal_controller_new() {
        let controller = CausalController::new();
        assert!(controller.history.is_empty());
        assert!(controller.rules.is_empty());
    }

    #[test]
    fn test_record_and_detect_side_effects() {
        let mut controller = CausalController::new();
        controller.record_event(CausalEvent {
            id: "1".to_string(),
            action: "edit".to_string(),
            file: "src/main.rs".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            side_effects: vec!["src/lib.rs".to_string()],
        });

        let effects = controller.detect_side_effects("src/main.rs");
        assert_eq!(effects, vec!["src/lib.rs"]);
    }

    #[test]
    fn test_pattern_match() {
        let mut controller = CausalController::new();
        controller.record_event(CausalEvent {
            id: "1".to_string(),
            action: "refactor: extract function".to_string(),
            file: "src/main.rs".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            side_effects: vec![],
        });

        let matches = controller.pattern_match("refactor");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_branch_decision_continue() {
        let controller = CausalController::new();
        let event = CausalEvent {
            id: "1".to_string(),
            action: "edit".to_string(),
            file: "src/main.rs".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            side_effects: vec![],
        };
        let decision = controller.branch_decision(&event);
        assert_eq!(decision, BranchDecision::Continue);
    }

    #[test]
    fn test_branch_decision_rollback() {
        let controller = CausalController::new();
        let event = CausalEvent {
            id: "1".to_string(),
            action: "edit".to_string(),
            file: "src/main.rs".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            side_effects: vec!["error: compilation failed".to_string()],
        };
        let decision = controller.branch_decision(&event);
        assert!(matches!(decision, BranchDecision::Rollback(_)));
    }

    #[test]
    fn test_iteration_trace() {
        let mut controller = CausalController::new();
        controller.record_event(CausalEvent {
            id: "1".to_string(),
            action: "edit".to_string(),
            file: "src/main.rs".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
            side_effects: vec![],
        });
        controller.record_event(CausalEvent {
            id: "2".to_string(),
            action: "edit".to_string(),
            file: "src/main.rs".to_string(),
            timestamp: "2024-01-02T00:00:00Z".to_string(),
            side_effects: vec![],
        });

        let trace = controller.iteration_trace("src/main.rs");
        assert_eq!(trace.len(), 2);
    }

    #[test]
    fn test_register_rule() {
        let mut controller = CausalController::new();
        controller.register_rule(CausalRule::FilePattern("*.rs".to_string()));
        assert_eq!(controller.rules.len(), 1);
    }
}