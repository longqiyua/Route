# Route

> **From Route to Routine.**
> **One Markdown, give it to your AI, and start using Route.**
> **一个 Markdown，交给你的 AI，然后开始使用 Route。**

---

## 🤖 AI Instructions

If you are an AI assistant reading this file, follow these steps:

### Step 1: Clone & Build

```bash
git clone https://github.com/longqiyua/route.git
cd route
cargo build --release
```

### Step 2: Configure MCP

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

### Step 3: Initialize & Use

```bash
cd /path/to/user/project
route init
route base status
route base search "query"
route ai chat "your message"
```

### Step 4: Dive Deeper

For full architecture, API reference, and extended usage, read:

- [`document/architect.md`](document/architect.md) — Full architecture, design, feature toggles, all interfaces
- [`document/`](document/) — Additional documentation
- `.route/skills/` — AI skills directory (auto-created)
- `.route/references/` — Reference materials for AI context injection

---

**Distribution**: https://github.com/longqiyua/route/releases

**License**: AGPL-3.0