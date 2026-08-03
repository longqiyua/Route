//! Agent 核心循环
//!
//! 实现 plan→act→observe 循环，面向 vibecoding 场景。

/// Agent 执行结果
#[derive(Debug, Clone)]
pub struct VmResult {
    pub success: bool,
    pub status: String,
    pub output: String,
    pub git_changes: Vec<String>,
    pub memory_updates: Vec<String>,
    pub side_effects: Vec<String>,
    pub duration_ms: u128,
}

impl Default for VmResult {
    fn default() -> Self {
        Self {
            success: true,
            status: "completed".to_string(),
            output: String::new(),
            git_changes: Vec::new(),
            memory_updates: Vec::new(),
            side_effects: Vec::new(),
            duration_ms: 0,
        }
    }
}

impl VmResult {
    /// 创建新的 VmResult
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否成功
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// 获取摘要信息
    pub fn summary(&self) -> String {
        let mut s = format!("Status: {} ({}ms)\n", self.status, self.duration_ms);
        if !self.git_changes.is_empty() {
            s.push_str(&format!("Git changes: {}\n", self.git_changes.join(", ")));
        }
        if !self.memory_updates.is_empty() {
            s.push_str(&format!("Memory updates: {}\n", self.memory_updates.join(", ")));
        }
        if !self.side_effects.is_empty() {
            s.push_str(&format!("Side effects: {}\n", self.side_effects.join(", ")));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vm_result_default() {
        let result = VmResult::default();
        assert!(result.success);
        assert_eq!(result.status, "completed");
        assert_eq!(result.duration_ms, 0);
    }

    #[test]
    fn test_vm_result_new() {
        let result = VmResult::new();
        assert!(result.is_success());
    }

    #[test]
    fn test_vm_result_summary() {
        let result = VmResult {
            success: true,
            status: "completed".to_string(),
            output: "test output".to_string(),
            git_changes: vec!["feat: add test".to_string()],
            memory_updates: vec!["updated memory".to_string()],
            side_effects: vec![],
            duration_ms: 42,
        };
        let summary = result.summary();
        assert!(summary.contains("completed"));
        assert!(summary.contains("42ms"));
        assert!(summary.contains("feat: add test"));
        assert!(summary.contains("updated memory"));
    }
}