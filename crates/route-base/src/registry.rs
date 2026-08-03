//! 服务注册表 — Root Base 的可扩展服务注册与发现
//!
//! 支持后续扩展：storage, sync, plugin, network 等服务

use std::any::Any;
use std::collections::HashMap;

/// 服务注册表
///
/// 允许任意类型注册为服务，通过类型 ID 查询。
/// 设计为可扩展：后续新增 StorageService、SyncService 等只需注册即可。
#[derive(Default)]
pub struct ServiceRegistry {
    services: HashMap<String, Box<dyn Any + Send>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// 注册服务
    pub fn register<T: 'static + Send>(&mut self, name: &str, service: T) {
        self.services.insert(name.to_string(), Box::new(service));
        tracing::info!("[BASE] Service registered: {}", name);
    }

    /// 获取服务引用
    pub fn get<T: 'static>(&self, name: &str) -> Option<&T> {
        self.services
            .get(name)
            .and_then(|b| b.downcast_ref::<T>())
    }

    /// 获取服务可变引用
    pub fn get_mut<T: 'static>(&mut self, name: &str) -> Option<&mut T> {
        self.services
            .get_mut(name)
            .and_then(|b| b.downcast_mut::<T>())
    }

    /// 检查服务是否已注册
    pub fn has(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    /// 列出所有注册的服务名称
    pub fn list(&self) -> Vec<String> {
        self.services.keys().cloned().collect()
    }

    /// 移除服务
    pub fn remove(&mut self, name: &str) -> bool {
        self.services.remove(name).is_some()
    }

    /// 清空所有服务
    pub fn clear(&mut self) {
        self.services.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct MockService {
        value: i32,
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = ServiceRegistry::new();
        registry.register("mock", MockService { value: 42 });
        let svc = registry.get::<MockService>("mock");
        assert!(svc.is_some());
        assert_eq!(svc.unwrap().value, 42);
    }

    #[test]
    fn test_has_and_remove() {
        let mut registry = ServiceRegistry::new();
        registry.register("mock", MockService { value: 1 });
        assert!(registry.has("mock"));
        assert!(registry.remove("mock"));
        assert!(!registry.has("mock"));
    }

    #[test]
    fn test_list() {
        let mut registry = ServiceRegistry::new();
        registry.register("a", MockService { value: 1 });
        registry.register("b", MockService { value: 2 });
        let names = registry.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"a".to_string()));
        assert!(names.contains(&"b".to_string()));
    }
}