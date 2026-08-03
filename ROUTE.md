# V---

> **One Markdown, give it to your AI, and start using Route.**
> **一个 Markdown，交给你的 AI，然后开始使用 Route。**
>
> *From Route to Routine.*
>
> *Version 0.5 — 2026-08-04*

---

## 🤖 AI Instructions

If you are an AI assistant reading this file, follow these steps to take control of the user's project using Route:

### Step 1: Download the Route Repository

```bash
# Clone the repository
git clone https://github.com/longqiyua/route.git /path/to/route
cd /path/to/route

# Or download a release
curl -fsSL https://github.com/longqiyua/route/releases/latest/download/route-x86_64-pc-windows-msvc.zip -o route.zip
```

### Step 2: Build the Project

```bash
# Full build (all crates)
cargo build --release

# Specific components
cargo build --release -p route          # CLI binary
cargo build --release -p route-mcp      # MCP server binary
cargo build --release -p route-tui      # TUI binary
cargo build --release -p route-http     # HTTP REST API server
cargo build --release -p route-pyo3    # Python native module (.pyd)
maturin develop -m packages/route-py/pyproject.toml  # Install as pip package
```

### Step 3: Configure MCP Server

The MCP server is the primary way for AI to interact with Route. Configure it in your MCP client:

```json
{
  "mcpServers": {
    "route": {
      "command": "/path/to/route/target/release/route-mcp",
      "args": ["--project", "/path/to/user/project"],
      "env": {}
    }
  }
}
```

Once configured, you can call these MCP tools:
- `tracking_list` / `tracking_add` / `tracking_remove` — project tracking
- `tracking_sync` — sync tracked projects
- `tracking_history` — view tracking history
- `extension_skills` / `extension_references` — manage skills & references
- `project_context` — get full project context
- `ai_chat` — AI-assisted chat with project context
- `conversation_new` / `conversation_list` / `conversation_show` — conversation tracking
- `conversation_record` / `conversation_rollback` — record & rollback messages
- `conversation_archive` / `conversation_delete` — manage sessions

### Step 4: Initialize Route in the User's Project

```bash
# Initialize Route in the user's project directory
cd /path/to/user/project
route init

# This creates:
#   .route/skills/       — AI skills directory
#   .route/references/   — AI reference materials
#   .route/tracking.json — tracking configuration
#   .route/base-config.json — base configuration
```

### Step 5: Use CLI Commands

| Command | Description |
|---------|-------------|
| `route status` | Show project status |
| `route base status` | Show Root Base status (memory, engine, services) |
| `route base search <query>` | Search code with semantic fuzzy matching |
| `route base memory` | Show memory statistics |
| `route base self-manage introspect` | Route introspects its own codebase |
| `route permission status/set high|normal` | Manage permissions |
| `route mcp --config` | Show MCP configuration |
| `route project-context` | Get project context |
| `route ai chat <message>` | AI chat with project context |
| `route ai config` | Show AI configuration |
| `route conversation list` | List conversation sessions |
| `route conversation new <title>` | Create a new conversation |
| `route conversation show <id>` | Show messages in a session |
| `route conversation record <id> <role> <content>` | Record a message |
| `route conversation rollback <sid> <mid>` | Rollback to a specific message |
| `route conversation archive <id>` | Archive a session |
| `route conversation delete <id>` | Delete a session |

### Step 6: Use Python Bindings (Optional)

**Option A — Install via pip (recommended for daily use):**

```bash
# Install maturin (one-time)
pip install maturin

# Build and install the route Python package
maturin develop -m packages/route-py/pyproject.toml

# Now you can use it anywhere:
python -c "import route; print(route.__version__)"
```

**Option B — Direct import from build output:**

After building, the Python native module is at `target/release/route.dll` (Windows) / `route.so` (Linux/macOS). Copy it to your project and use it:

```python
import route

# Initialize or open a repository
route.init("/path/to/project")
# route.open("/path/to/project")  # open existing

# Basic operations
status = route.status()
print(status["current_branch"])  # "main"

commit = route.commit("feat: add login", author="AI")
print(commit["id"])

# History
commits = route.log(limit=5)
for c in commits:
    print(f"{c['id']}: {c['message']}")

# Branch management
route.branch_create("feature-x", kind="inherited")
route.branch_switch("feature-x")
branches = route.branch_list()

# Tags
route.tag_create("v1.0", message="first release")
tags = route.tag_list()

# Conversation tracking
session = route.conversation_new("chat about refactoring")
route.conversation_record(session, "user", "Hello, how do I refactor this?")
route.conversation_record(session, "ai", "Let me analyze the code...")
msgs = route.conversation_show(session)
```

### Step 7: Read Architecture Docs

The repository contains `document/architect.md` with detailed architecture documentation.

---

## Repository Structure

```
route/
├── ROUTE.md              ← This file — give this to any AI
├── document/architect.md ← Architecture documentation
├── LICENSE               ← AGPL-3.0
├── crates/
│   ├── route-core/       ← Core primitives (paths, hash, guard, schema)
│   ├── route-basic/      ← Core repository engine
│   ├── route-engine/     ← Search, fuzzy match, vector, RAG
│   ├── route-memory/     ← Memory, causal chain, conversation tracking
│   ├── route-cli/        ← CLI binary
│   ├── route-mcp/        ← MCP server binary
│   ├── route-tui/        ← TUI binary (Claude-Code-style REPL)
│   ├── route-http/       ← HTTP REST API server
│   ├── route-sync/       ← Backup & sync (WebDAV/S3/SSH)
│   ├── route-plugins/    ← Plugin system (webhook, logger)
│   ├── route-pyo3/       ← Python native bindings (PyO3)
│   └── route-stats/      ← Statistics collector
├── packages/
│   ├── cli/              ← TypeScript CLI wrapper
│   ├── core/             ← TypeScript core library
│   └── route-py/         ← Python package (pip install route-vc)
├── archive/
│   ├── crates/
│   │   └── route-tauri/  ← Archived GUI desktop app
│   └── packages/
│       ├── desktop/      ← Archived GUI frontend
│       └── desktop-bridge/ ← Archived GUI bridge
```

---

## Core Architecture

```
Root Base (orchestrator)
├── Root Engine (driver layer)
│   ├── Trie Index          — exact & prefix match
│   ├── Fuzzy Match         — Levenshtein-based fuzzy search
│   ├── BM25 Index          — semantic text retrieval
│   ├── Vector Index        — hybrid vectorizer (Fast rule + ML embed)
│   ├── Code Graph          — function call graph & dependency analysis
│   ├── GraphRAG            — graph-based retrieval augmented generation
│   ├── Semantic Index      — dual-mode (exact/semantic) fuzzy matching
│   ├── Keyword Index       — keyword-based search
│   ├── Hot/Cold Index      — adaptive memory management
│   └── Search Pipeline     — multi-stage search orchestrator
├── Root Memory (storage layer)
│   ├── Project Memory      — structured memory entries
│   ├── Causal Chain        — action → effect tracking
│   ├── Conversation        — AI chat tracking, snapshot rollback
│   ├── Tiered Memory       — hot/cold/archived memory tiers
│   └── Mermaid Output      — visual structure & chain diagrams
└── Interfaces
    ├── CLI                 — route command
    ├── MCP                 — model context protocol server
    ├── HTTP                — REST API server
    ├── TUI                 — interactive REPL (Claude-Code-style)
    └── PyO3                — Python native module (import route)
```

---

## AI Skill & Reference System

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

The HybridVectorizer supports three modes:
- **Fast** — rule-based token frequency vectorization (64-dim, O(n), zero state)
- **ML** — HashedNGramEmbedding (128-dim, n-gram feature hashing, incremental learning)
- **Auto** — automatically switches based on query length (default)

---

## Distribution

| Item | Link |
|------|------|
| **GitHub** | https://github.com/longqiyua/route |
| **Download** | https://github.com/longqiyua/route/releases |
| **License** | AGPL-3.0 |

**Zero-threshold distribution**: Just copy this `ROUTE.md` file and give it to any AI. The AI will:
1. Read this file to understand Route
2. Clone/download the repository
3. Build the project
4. Configure MCP for the user's project
5. Start managing the project with full version control + code intelligence

---

## License

AGPL-3.0 — See [LICENSE](LICENSE) for details.