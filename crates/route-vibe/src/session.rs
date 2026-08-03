//! 会话管理

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{VibeConfig, VibeSession};

/// 会话管理器
pub struct SessionManager {
    /// 所有会话（ID -> VibeSession）
    pub sessions: HashMap<String, VibeSession>,
    /// 当前活跃会话 ID
    pub active_id: Option<String>,
    /// 基础存储路径
    pub base_path: PathBuf,
}

impl SessionManager {
    /// 创建新的会话管理器
    pub fn new(base_path: &Path) -> Self {
        Self {
            sessions: HashMap::new(),
            active_id: None,
            base_path: base_path.to_path_buf(),
        }
    }

    /// 创建新的会话
    ///
    /// 返回新会话的 ID。
    pub fn create_session(&mut self, project_path: &Path) -> Result<String> {
        let config = VibeConfig {
            project_path: Some(project_path.to_path_buf()),
            ..Default::default()
        };

        #[allow(unused_mut)]
        let mut session = VibeSession::new(Some(project_path.to_path_buf()), config);

        // 尝试初始化各组件（如果对应 feature 启用）
        #[cfg(feature = "memory")]
        {
            let _ = session.init_memory();
        }

        #[cfg(feature = "vm")]
        {
            let _ = session.init_vm();
        }

        #[cfg(feature = "skill")]
        {
            let _ = session.init_skill();
        }

        #[cfg(feature = "engine")]
        {
            let _ = session.init_engine();
        }

        let id = session.id.clone();
        self.sessions.insert(id.clone(), session);
        self.active_id = Some(id.clone());

        tracing::info!("[SESSION] Created session: {}", id);
        Ok(id)
    }

    /// 获取会话引用
    pub fn get_session(&self, id: &str) -> Option<&VibeSession> {
        self.sessions.get(id)
    }

    /// 获取会话可变引用
    pub fn get_session_mut(&mut self, id: &str) -> Option<&mut VibeSession> {
        self.sessions.get_mut(id)
    }

    /// 获取当前活跃会话
    pub fn get_active(&self) -> Option<&VibeSession> {
        self.active_id
            .as_ref()
            .and_then(|id| self.sessions.get(id))
    }

    /// 获取当前活跃会话的可变引用
    pub fn get_active_mut(&mut self) -> Option<&mut VibeSession> {
        let active_id = self.active_id.clone()?;
        self.sessions.get_mut(&active_id)
    }

    /// 设置活跃会话
    pub fn set_active(&mut self, id: &str) -> Result<()> {
        if !self.sessions.contains_key(id) {
            anyhow::bail!("会话不存在: {}", id);
        }
        self.active_id = Some(id.to_string());
        tracing::info!("[SESSION] Active session set to: {}", id);
        Ok(())
    }

    /// 关闭会话
    pub fn close_session(&mut self, id: &str) -> Result<()> {
        self.sessions.remove(id).context(format!("会话不存在: {}", id))?;

        // 如果关闭的是活跃会话，清除 active_id
        if self.active_id.as_deref() == Some(id) {
            self.active_id = None;
        }

        tracing::info!("[SESSION] Closed session: {}", id);
        Ok(())
    }

    /// 列出所有会话
    pub fn list_sessions(&self) -> Vec<&VibeSession> {
        self.sessions.values().collect()
    }

    /// 获取会话数量
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_manager_new() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path());
        assert!(manager.sessions.is_empty());
        assert!(manager.active_id.is_none());
    }

    #[test]
    fn test_create_and_get_session() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());

        let id = manager.create_session(temp_dir.path()).unwrap();
        assert!(!id.is_empty());

        let session = manager.get_session(&id);
        assert!(session.is_some());
        assert_eq!(session.unwrap().id, id);
    }

    #[test]
    fn test_active_session() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());

        // 创建会话后自动设为活跃
        let id = manager.create_session(temp_dir.path()).unwrap();
        let active = manager.get_active();
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, id);
    }

    #[test]
    fn test_set_active() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());

        let id1 = manager.create_session(temp_dir.path()).unwrap();
        let id2 = manager.create_session(temp_dir.path()).unwrap();

        // 切换到 id1
        manager.set_active(&id1).unwrap();
        assert_eq!(manager.get_active().unwrap().id, id1);

        // 切换到 id2
        manager.set_active(&id2).unwrap();
        assert_eq!(manager.get_active().unwrap().id, id2);
    }

    #[test]
    fn test_set_active_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());
        let result = manager.set_active("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_close_session() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());

        let id = manager.create_session(temp_dir.path()).unwrap();
        assert_eq!(manager.session_count(), 1);

        manager.close_session(&id).unwrap();
        assert_eq!(manager.session_count(), 0);
        assert!(manager.active_id.is_none());
    }

    #[test]
    fn test_close_session_invalid() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());
        let result = manager.close_session("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());

        manager.create_session(temp_dir.path()).unwrap();
        manager.create_session(temp_dir.path()).unwrap();

        let sessions = manager.list_sessions();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_get_session_mut() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());

        let id = manager.create_session(temp_dir.path()).unwrap();
        let session = manager.get_session_mut(&id);
        assert!(session.is_some());
    }

    #[test]
    fn test_get_active_mut() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());

        manager.create_session(temp_dir.path()).unwrap();
        let active = manager.get_active_mut();
        assert!(active.is_some());
    }

    #[test]
    fn test_get_active_mut_no_session() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path());
        assert!(manager.get_active_mut().is_none());
    }
}