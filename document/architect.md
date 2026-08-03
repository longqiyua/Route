# Route Architecture

> **From Route to Routine.**
> **One Markdown, give it to your AI, and start using Route.**

---

## Table of Contents

- [Core Architecture](#core-architecture)
- [Crate Map](#crate-map)
- [Interfaces](#interfaces)
- [Feature Toggles](#feature-toggles)
- [CLI Reference](#cli-reference)
- [MCP Reference](#mcp-reference)
- [Enhanced RAG Cycle](#enhanced-rag-cycle)
- [Skill & Reference System](#skill--reference-system)
- [Development](#development)

---

## Core Architecture

```
Root Base (orchestrator)
├── Root Engine (driver layer)
│   ├── Trie Index            — exact & prefix match
│   ├── Fuzzy Match           — Levenshtein-based fuzzy search
│   ├── BM25 Index            — semantic text retrieval
│   ├── Vector Index          — hybrid vectorizer (Fast rule + ML embed)
│   ├── Code Graph            — function call graph & dependency analysis
│   ├── GraphRAG              — graph-based retrieval augmented generation
│   ├── Semantic Index        — dual-mode (exact/semantic) fuzzy matching
│   ├── Keyword Index         — keyword-based search
│   ├── Hot/Cold Index        — adaptive memory management
│   └── Search Pipeline       — multi-stage search orchestrator
├── Root Memory (storage layer)
│   ├── Project Memory        — structured memory entries
│   ├── Causal Chain          — action → effect tracking
│   ├── Tiered Memory         — hot/cold/archived memory tiers
│   └── Mermaid Output        — visual structure & chain diagrams
└── Interfaces
    ├── CLI                   — route command
    ├── MCP                   — model context protocol server
    ├── HTTP                  — REST API (in development)
    └── PyO3                  — Python native module
```

### Root Base

Root Base is the top-level orchestrator. It initializes Engine and Memory, provides unified API across CLI/MCP/HTTP/GUI, and manages configuration, permissions, and lifecycle.

### Root Engine

The driver layer — all search, match, vector, and RAG capabilities.

| Component | Description |
|-----------|-------------|
| Trie Index | Exact and prefix matching via trie data structure |
| Fuzzy Match | Levenshtein distance-based fuzzy string matching |
| BM25 Index | Probabilistic text retrieval for semantic search |
| Vector Index | Hybrid vectorizer combining rule-based (Fast) and ML embedding |
| Code Graph | Function call graph extraction and cross-file dependency analysis |
| GraphRAG | Graph-based retrieval augmented generation |
| Semantic Index | Dual-mode fuzzy matching (exact / semantic / hybrid) |
| Keyword Index | Keyword-based exact search |
| Hot/Cold Index | Adaptive in-memory index management |
| Search Pipeline | Multi-stage orchestrator: exact → prefix → fuzzy → BM25, deduplicated |

### Root Memory

The storage layer — project memory, causal tracking, and tiered storage.

| Component | Description |
|-----------|-------------|
| Project Memory | Structured memory entries with CRUD, query, and search |
| Causal Chain | Action → effect tracking for auditing and debugging |
| Tiered Memory | Hot/cold/archived tiers with automatic promotion/demotion |
| Mermaid Output | Visual structure diagrams and causal chain graphs |

---

## Crate Map

```
route/
├── route-base/       — Root Base: orchestrator, config, permissions
├── route-engine/     — Root Engine: search, fuzzy, vector, code graph, RAG
├── route-memory/     — Root Memory: project memory, causal chain, tiered storage
├── route-cli/        — CLI binary
├── route-mcp/        — MCP server binary
├── route-tui/        — Terminal UI binary
├── route-pyo3/       — Python bindings (PyO3)
├── route-sync/       — Backup & sync (WebDAV / S3 / SSH)
├── route-skill/      — Skill & reference system, RAG engine
├── route-vibe/       — Vibe session & model proxy
├── route-vm/         — VM agent, causal control, git operations
├── route-plugins/    — Plugin system (webhook, logger)
├── route-stats/      — Statistics collector
└── route-tauri/      — Desktop app (Tauri, feature-gated)
```

Each crate has a single responsibility and can be independently removed. `route-base` depends only on `route-engine` and `route-memory`.

---

## Interfaces

### CLI

Primary interface. All core features exposed via `route` command.

```bash
route init                    # Initialize project
route status                  # Show project status
route base status             # Show Root Base status
route base search <query>     # Semantic fuzzy search
route base memory             # Memory statistics
route base self-manage introspect  # Self-introspection
route base gui enable/disable/status  # Toggle GUI
route ai chat <message>       # AI chat with project context
route ai config               # Show AI configuration
route permission status/set high|normal  # Manage permissions
route mcp --config            # Show MCP configuration
route project-context         # Get project context
route log                     # Show commit history
route bench run default       # Run benchmarks
```

### MCP

Model Context Protocol server. Primary AI interaction channel.

**Tools:**

| Tool | Description |
|------|-------------|
| `tracking_list` | List tracked projects |
| `tracking_add` | Add project to tracking |
| `tracking_remove` | Remove project from tracking |
| `tracking_sync` | Sync tracked projects |
| `tracking_history` | View tracking history |
| `extension_skills` | Manage AI skills |
| `extension_references` | Manage reference materials |
| `project_context` | Get full project context |
| `ai_chat` | AI-assisted chat with project context |

**Configuration:**

```json
{
  "mcpServers": {
    "route": {
      "command": "/path/to/route/target/release/route-mcp",
      "args": ["--project", "/path/to/user/project"]
    }
  }
}
```

### HTTP

REST API (in development). Same capabilities as CLI/MCP, exposed over HTTP.

### PyO3

Python native module. Import Route directly in Python:

```python
import route_pyo3
engine = route_pyo3.Route()
results = engine.search("query")
```

---

## Feature Toggles

Route uses feature flags. Core features are always enabled; GUI is hidden by default.

### Always Enabled

| Feature | CLI | Description |
|---------|-----|-------------|
| CLI | `route` | Command-line interface |
| TUI | `route-tui` | Terminal UI |
| AI Assistant | `route ai chat` | AI chat (requires API key) |
| MCP | `route mcp` | MCP protocol support |
| Semantic Engine | `route base search` | Fuzzy match + semantic index |

### Hidden (Default Off)

| Feature | Enable | Description |
|---------|--------|-------------|
| GUI Desktop | `route base gui enable` | Tauri desktop app (requires build) |

```bash
route base gui enable     # Enable GUI
route base gui disable    # Disable GUI
route base gui status     # Check GUI status
```

### Build-Time Enable

```bash
cd packages/desktop
npm install
npm run build
cargo tauri build
```

---

## Enhanced RAG Cycle

Route implements a six-step RAG cycle for code intelligence:

```
① User writes code
    ↓
② AI associates (semantic matching via VectorIndex)
    ↓
③ AI modular refactors (group by function, not file)
    ↓
④ RAG reranking (3rd-party chunking + built-in rerank)
    ↓
⑤ RAG vectorization (HybridVectorizer: Fast rule + ML embed)
    ↓
⑥ Back to ①
```

### HybridVectorizer

Three modes:

| Mode | Method | Dimensions | Characteristics |
|------|--------|-----------|-----------------|
| Fast | Token frequency (rule-based) | 64 | O(n), zero state, maximum speed |
| ML | HashedNGramEmbedding | 128 | N-gram feature hashing, incremental learning |
| Auto | Automatic selection | — | Fast for short queries, ML for long queries (default) |

The ML embedder uses feature hashing to map n-gram features to fixed-dimension vectors, with online SGD for weight updates. No external model files needed.

---

## Skill & Reference System

Route uses two directories inside the user's project for AI context injection:

### `.route/skills/`

Place markdown skill files here. Each skill defines a capability the AI can use.

### `.route/references/`

Place reference materials here. The AI reads these files for project-specific context:
- Architecture decisions
- Coding conventions
- API documentation
- Business logic descriptions

The RAG engine automatically indexes these files and injects them into AI context.

---

## Development

### Build

```bash
# Full build
cargo build --release

# Specific crates
cargo build --release -p route-cli
cargo build --release -p route-mcp
cargo build --release -p route-pyo3

# Desktop GUI (requires GUI toggle)
cd packages/desktop
npm install
npm run build
```

### Test

```bash
# All tests
cargo test

# Specific crate
cargo test -p route-engine

# Doc tests
cargo test --doc
```

### Architecture Constraints

- `route-base` depends only on `route-engine` + `route-memory`
- Each crate must compile independently
- No circular dependencies
- GUI code never imported by non-GUI crates

---

## License

AGPL-3.0 — See [LICENSE](../LICENSE) for details.