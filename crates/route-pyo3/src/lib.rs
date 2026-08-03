//! Route Python bindings (PyO3)
//!
//! Exposes Route's core functionality (Root Base, Root Engine, Root Memory)
//! as a Python native module. Usage:
//!
//! ```python
//! import route
//! import json
//!
//! # Initialize
//! r = route.Route("/path/to/project")
//!
//! # Status
//! status = json.loads(r.status())
//! print(status)
//!
//! # Search (semantic fuzzy matching)
//! results = json.loads(r.search("a + b", top_k=5))
//! for r in results:
//!     print(f"[{r['score']:.3f}] {r.get('symbol_name', r.get('text', ''))}")
//!
//! # Memory
//! stats = json.loads(r.memory_stats())
//! print(stats)
//!
//! # Index code
//! r.index("fn my_add(a: i32, b: i32) -> i32 { a + b }", "lib.rs", "rust")
//! ```

use std::path::Path;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use route_base::RootBase;
use route_engine::symbol::{CodeSymbol, SymbolInfo};
use route_engine::{
    ArchitectureAssistant, MatchMode, RagPipeline, SemanticIndex, ThirdPartyChunking,
};

/// Route — Python native module
#[pyclass]
struct Route {
    base: RootBase,
}

#[pymethods]
impl Route {
    /// Create a new Route instance for the given project path.
    #[new]
    fn new(project_path: &str) -> PyResult<Self> {
        let base = RootBase::new(Path::new(project_path))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to open Route: {e}")))?;
        Ok(Route { base })
    }

    /// Get the Root Base status including memory mode, causal control, hot/cold index stats.
    fn status(&self) -> PyResult<String> {
        let s = self.base.status();
        let value = serde_json::json!({
            "project": s.project,
            "memory_mode": s.memory_mode,
            "causal_control": s.causal_control,
            "auto_git": s.auto_git,
            "adaptive_debounce": s.adaptive_debounce,
            "memory_entries": s.memory_entries,
            "memory_chains": s.memory_chains,
            "hot_blocks": s.hot_blocks,
            "cold_blocks": s.cold_blocks,
            "estimated_bytes": s.estimated_bytes,
            "services": s.services,
        });
        serde_json::to_string(&value)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize status: {e}")))
    }

    /// Search the project code using the three-mechanism engine (Vector + Graph + Keyword).
    #[pyo3(signature = (query, top_k=None))]
    fn search(&self, query: &str, top_k: Option<usize>) -> PyResult<String> {
        let k = top_k.unwrap_or(10);
        let results = self.base.search(query, k);
        let items: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "text": r.text,
                    "score": r.score,
                    "file_path": r.file_path,
                    "line": r.line,
                    "source": r.source,
                })
            })
            .collect();
        serde_json::to_string(&items)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize search results: {e}")))
    }

    /// Semantic search with the dual-mode fuzzy matching engine.
    #[pyo3(signature = (query, mode=None, top_k=None))]
    fn semantic_search(&self, query: &str, mode: Option<&str>, top_k: Option<usize>) -> PyResult<String> {
        let k = top_k.unwrap_or(10);
        let match_mode = MatchMode::from_str(mode.unwrap_or("hybrid"));
        let index = self.base.engine.symbol_index.clone();
        let mut sem = SemanticIndex::new();
        sem.set_mode(match_mode);
        for sym in &index.symbols {
            sem.add_entry(sym.clone(), "");
        }
        let results = sem.search(query, k);
        let items: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "symbol_name": r.symbol_name,
                    "operation": r.operation,
                    "core_expr": r.core_expr,
                    "file_path": r.file_path,
                    "line": r.line,
                    "score": r.score,
                    "annotation": r.annotation,
                })
            })
            .collect();
        serde_json::to_string(&items)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize semantic results: {e}")))
    }

    /// Get memory statistics.
    fn memory_stats(&self) -> PyResult<String> {
        let stats = self.base.memory.stats();
        let value = serde_json::json!({
            "total_entries": stats.total_entries,
            "by_kind": stats.by_kind,
            "oldest_entry": stats.oldest_entry,
            "newest_entry": stats.newest_entry,
        });
        serde_json::to_string(&value)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize memory stats: {e}")))
    }

    /// Get the project structure in Mermaid format.
    fn project_structure(&self) -> PyResult<Option<String>> {
        Ok(self.base.project_structure())
    }

    /// Get the project meta information.
    fn project_meta(&self) -> PyResult<Option<String>> {
        Ok(self.base.project_meta())
    }

    /// Index a code line into the engine.
    fn index(&mut self, content: &str, file_path: &str, language: &str) -> PyResult<()> {
        self.base
            .engine
            .add_line(content, file_path, 1, language);
        Ok(())
    }

    /// Record a causal link.
    fn record_causal(&mut self, action: &str, file: &str, reason: &str, effect: &str) -> PyResult<()> {
        self.base
            .record_causal_link(action, file, reason, effect)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to record causal link: {e}")))?;
        Ok(())
    }

    /// Get the causal chain.
    fn causal_chain(&self) -> PyResult<String> {
        let links = &self.base.memory.chain.links;
        let items: Vec<serde_json::Value> = links
            .iter()
            .map(|l| {
                serde_json::json!({
                    "id": l.id,
                    "action": l.action,
                    "reason": l.reason,
                    "effect": l.effect,
                    "file_path": l.file_path,
                    "timestamp": l.timestamp,
                    "actor": l.actor,
                    "status": format!("{:?}", l.status),
                })
            })
            .collect();
        serde_json::to_string(&items)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize causal chain: {e}")))
    }

    /// Run the six-step RAG cycle: user writes code → AI associates → modular refactor → RAG rerank → vectorize → loop.
    #[pyo3(signature = (code, symbol_name, file_path, top_k=None))]
    fn run_rag_cycle(&mut self, code: &str, symbol_name: &str, file_path: &str, top_k: Option<usize>) -> PyResult<String> {
        let k = top_k.unwrap_or(5);
        let mut pipeline = RagPipeline::new();
        let mut sem_idx = SemanticIndex::new();
        sem_idx.set_mode(MatchMode::Semantic);

        // Build symbol
        let symbol = SymbolInfo {
            kind: CodeSymbol::Function,
            name: symbol_name.to_string(),
            file_path: file_path.to_string(),
            line_start: 1,
            line_end: 3,
            doc_comment: None,
        };

        // Run cycle
        let modules = pipeline.run_cycle(&mut sem_idx, code, symbol, code, k);

        let mut result = serde_json::Map::new();
        let mut mods = serde_json::Map::new();
        for (op, items) in &modules {
            let mod_items: Vec<serde_json::Value> = items
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "symbol_name": m.symbol_name,
                        "operation": m.operation,
                        "score": m.score,
                    })
                })
                .collect();
            mods.insert(op.clone(), serde_json::Value::Array(mod_items));
        }
        result.insert("modules".to_string(), serde_json::Value::Object(mods));
        result.insert("total_entries".to_string(), serde_json::Value::Number(serde_json::Number::from(sem_idx.count())));
        result.insert("stage".to_string(), serde_json::Value::String(pipeline.stage.as_str().to_string()));

        serde_json::to_string(&serde_json::Value::Object(result))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize RAG cycle result: {e}")))
    }

    /// Run the enhanced RAG cycle with 3rd-party AI chunking + architecture design.
    ///
    /// Implements the full (a) → (b) → (c) flow:
    /// (a) Third-party AI data chunking & reranking
    /// (b) Built-in small AI vectorization
    /// (c) Architecture design assistance
    /// Then runs the six-step cycle (②→⑥).
    #[pyo3(signature = (code, symbol_name, file_path, top_k=None))]
    fn run_rag_cycle_enhanced(&mut self, code: &str, symbol_name: &str, file_path: &str, top_k: Option<usize>) -> PyResult<String> {
        let k = top_k.unwrap_or(5);
        let mut pipeline = RagPipeline::new();
        let mut chunking = ThirdPartyChunking::new();
        let mut sem_idx = SemanticIndex::new();
        sem_idx.set_mode(MatchMode::Semantic);
        let mut architect = ArchitectureAssistant::new();

        let symbol = SymbolInfo {
            kind: CodeSymbol::Function,
            name: symbol_name.to_string(),
            file_path: file_path.to_string(),
            line_start: 1,
            line_end: 3,
            doc_comment: None,
        };

        let result = pipeline.run_cycle_with_chunking(
            &mut chunking, &mut sem_idx, &mut architect,
            code, symbol, code, k,
        );

        let mut py_result = serde_json::Map::new();

        // Modules
        let mut mods = serde_json::Map::new();
        for (op, items) in &result.modules {
            let mod_items: Vec<serde_json::Value> = items
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "symbol_name": m.symbol_name,
                        "operation": m.operation,
                        "score": m.score,
                    })
                })
                .collect();
            mods.insert(op.clone(), serde_json::Value::Array(mod_items));
        }
        py_result.insert("modules".to_string(), serde_json::Value::Object(mods));

        // Architecture suggestions
        let archs: Vec<serde_json::Value> = result.architectures
            .iter()
            .map(|a| {
                serde_json::json!({
                    "kind": format!("{:?}", a.kind),
                    "module_name": a.module_name,
                    "description": a.description,
                    "related_operations": a.related_operations,
                    "suggested_files": a.suggested_files,
                    "confidence": a.confidence,
                })
            })
            .collect();
        py_result.insert("architectures".to_string(), serde_json::Value::Array(archs));

        // Stats
        py_result.insert("total_chunks".to_string(), serde_json::json!(result.total_chunks));
        py_result.insert("vectorized_chunks".to_string(), serde_json::json!(result.vectorized_chunks));
        py_result.insert("high_confidence_chunks".to_string(), serde_json::json!(result.high_confidence_chunks));
        py_result.insert("total_entries".to_string(), serde_json::json!(result.total_entries));

        serde_json::to_string(&serde_json::Value::Object(py_result))
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize enhanced RAG cycle result: {e}")))
    }

    /// Get architecture design suggestions from the current semantic index.
    fn architecture_design(&self) -> PyResult<String> {
        let mut index = SemanticIndex::new();
        index.set_mode(MatchMode::Hybrid);
        // Convert symbol index entries to semantic entries
        for sym in &self.base.engine.symbol_index.symbols {
            index.add_entry(sym.clone(), "");
        }

        let mut architect = ArchitectureAssistant::new();
        let suggestions = architect.analyze(&index);

        let items: Vec<serde_json::Value> = suggestions
            .iter()
            .map(|s| {
                serde_json::json!({
                    "kind": format!("{:?}", s.kind),
                    "module_name": s.module_name,
                    "description": s.description,
                    "related_operations": s.related_operations,
                    "suggested_files": s.suggested_files,
                    "confidence": s.confidence,
                })
            })
            .collect();

        let value = serde_json::json!({
            "suggestions": items,
            "graph": architect.to_architecture_graph(),
        });
        serde_json::to_string(&value)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to serialize architecture design: {e}")))
    }
}

/// Python module definition
#[pymodule]
fn route(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Route>()?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}