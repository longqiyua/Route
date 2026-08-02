$path = "C:\Users\longq\Desktop\route (1)\crates\route-tauri\web\src\App.tsx"
$content = Get-Content $path -Raw

# Find insertion point — just before the very last function's closing brace.
# Strategy: find the last `}\n` (with newline) at the end, insert before it.
# Simpler: find end of last known function ("kindLabel" or near bottom),
# append new components after the file's last top-level statement.

# Look for the very last "}" followed by optional newline + EOF.
# We will find the final closing brace of the file by locating the last
# occurrence of "\n}\n" near the end.

# Use [System.IO.File]::ReadAllText for byte-exact read.
$bytes = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8)
$lines = $bytes -split "`n"

# The file currently ends with the classifyChange return-statement block.
# We'll locate the very last function definition (the one that ends the
# file) and append our new components after it.

# Strategy: just append at end. Find EOF (no trailing newline perhaps).
$lastNonEmpty = -1
for ($i = $lines.Count - 1; $i -ge 0; $i--) {
    if ($lines[$i].Trim() -ne "") { $lastNonEmpty = $i; break }
}
Write-Output "Last non-empty line: $lastNonEmpty : $($lines[$lastNonEmpty])"
$insertAtLine = $lastNonEmpty + 1
# Trim to last "}" boundary
$head = ($lines[0..($lastNonEmpty)]) -join "`n"

$append = @'

// ---------------------------------------------------------------------------
// Sidebar — multi-project + master switch + add-project bar
// ---------------------------------------------------------------------------

function Sidebar({
  projects,
  activeProjectId,
  onSelectProject,
  onAddProject,
  onRemoveProject,
  appOn,
  onToggleApp,
}: {
  projects: Project[];
  activeProjectId: string | null;
  onSelectProject: (id: string) => void;
  onAddProject: () => void;
  onRemoveProject: (id: string) => void;
  appOn: boolean;
  onToggleApp: () => void;
}) {
  return (
    <aside className="sidebar">
      <div className="brand-row">
        <Logo size={18} />
      </div>

      <div className="sidebar-master">
        <span className="sidebar-master-label">运行</span>
        <button
          className={`sidebar-master-switch ${appOn ? "is-on" : "is-off"}`}
          onClick={onToggleApp}
          aria-pressed={appOn}
          title={appOn ? "点击关闭总控" : "点击开启总控"}
        >
          <span className="master-dot" />
          <span className="master-text">{appOn ? "运行" : "关闭"}</span>
        </button>
      </div>

      <button className="sidebar-add" onClick={onAddProject} title="添加项目文件夹">
        <span className="sidebar-add-icon" aria-hidden="true">
          <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round">
            <line x1="3" y1="8" x2="13" y2="8" />
            <line x1="8" y1="3" x2="8" y2="13" />
          </svg>
        </span>
        <span className="sidebar-add-text">添加项目</span>
      </button>

      <div className="project-cards" role="list">
        {projects.length === 0 && (
          <div className="empty-projects">尚未添加项目</div>
        )}
        {projects.map((p) => {
          const isActive = p.id === activeProjectId;
          return (
            <div
              key={p.id}
              role="listitem"
              className={`project-card ${isActive ? "active" : ""}`}
              onClick={() => onSelectProject(p.id)}
            >
              <div className="project-card-row1">
                <span className="project-card-name">{p.name}</span>
                <button
                  className="project-card-remove"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (confirm(`从列表中移除「${p.name}」?（不会删除磁盘上的文件夹）`)) {
                      onRemoveProject(p.id);
                    }
                  }}
                  title="移除项目"
                  aria-label="移除项目"
                >
                  ×
                </button>
              </div>
              <div className="project-card-row2">
                <span className={`ep-pill ep-${p.endpoint}`}>
                  {p.endpoint === "local" ? "本地" : "云端"}
                </span>
                {p.routeA && <span className="routea-pill">route A</span>}
              </div>
              <div className="project-card-path" title={p.path}>{p.path}</div>
            </div>
          );
        })}
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// SettingsPage — per-project configuration + global theme + locale.
// ---------------------------------------------------------------------------

function SettingsPage({
  project,
  endpoint,
  onEndpointChange,
  routeaEnabled,
  onToggleRoutea,
  locale,
  onLocaleChange,
}: {
  project: Project | null;
  endpoint: Endpoint;
  onEndpointChange: (e: Endpoint) => void;
  routeaEnabled: boolean;
  onToggleRoutea: () => void;
  locale: Locale;
  onLocaleChange: (l: Locale) => void;
}) {
  if (!project) {
    return (
      <section className="card">
        <div className="gate-banner">
          <span>请先在左侧选择一个项目。</span>
        </div>
      </section>
    );
  }

  return (
    <section className="card settings-page">
      <h2>设置 · {project.name}</h2>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>语言</span>
          <span className="settings-row-hint">立即生效，刷新时也保留</span>
        </div>
        <div className="settings-row-control">
          <div className="seg-control">
            <button
              className={locale === "zh" ? "active" : ""}
              onClick={() => onLocaleChange("zh")}
              aria-pressed={locale === "zh"}
            >中文</button>
            <button
              className={locale === "en" ? "active" : ""}
              onClick={() => onLocaleChange("en")}
              aria-pressed={locale === "en"}
            >English</button>
          </div>
        </div>
      </div>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>端点</span>
          <span className="settings-row-hint">选择本地或云端备份</span>
        </div>
        <div className="settings-row-control">
          <div className="seg-control">
            <button
              className={endpoint === "local" ? "active" : ""}
              onClick={() => onEndpointChange("local")}
              aria-pressed={endpoint === "local"}
            >本地</button>
            <button
              className={endpoint === "cloud" ? "active" : ""}
              onClick={() => onEndpointChange("cloud")}
              aria-pressed={endpoint === "cloud"}
            >云端</button>
          </div>
        </div>
      </div>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>备份方式</span>
          <span className="settings-row-hint">
            {endpoint === "local" ? "本地路径（绝对路径）" : "云端目标 + 凭据"}
          </span>
        </div>
        <div className="settings-row-control">
          {endpoint === "local" ? (
            <input
              className="settings-input"
              placeholder="例如 D:\\route-backups\\this-project"
              defaultValue={project.backup?.target ?? ""}
              readOnly
            />
          ) : (
            <input
              className="settings-input"
              placeholder="s3://bucket/path 或 webdav://host/path"
              defaultValue={project.backup?.target ?? ""}
              readOnly
            />
          )}
        </div>
      </div>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>route 标记模式</span>
          <span className="settings-row-hint">
            开启后，软件将读取文档与代码内容以识别 route 注释。
          </span>
        </div>
        <div className="settings-row-control">
          <label className="switch">
            <input
              type="checkbox"
              checked={routeaEnabled}
              onChange={onToggleRoutea}
            />
            <span className="track">
              <span className="thumb" />
            </span>
            <span className={`status ${routeaEnabled ? "on" : "off"}`}>
              {routeaEnabled ? "已开启" : "未开启"}
            </span>
          </label>
        </div>
      </div>

      <div className="settings-cheatsheet">
        <h3>route 标记语法</h3>
        <table className="cheat-table">
          <thead>
            <tr>
              <th>标记</th>
              <th>含义</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td><code>// route X</code></td>
              <td>此文件所有修改归属 X 分类</td>
            </tr>
            <tr>
              <td><code>// route start X</code> ... <code>// route over X</code></td>
              <td>两行之间的修改归属 X 分类</td>
            </tr>
            <tr>
              <td><code>// route no start</code> ... <code>// route no</code></td>
              <td>这段记录不会被追踪</td>
            </tr>
          </tbody>
        </table>
        <p className="cheat-note">
          支持 <code>//</code>、<code>#</code>、<code>--</code>、<code>/* */</code> 等常见注释风格。
        </p>
      </div>

      <div className="settings-privacy">
        <h3>隐私与本地计算</h3>
        <p>
          <strong>route 标记模式</strong>开启后，Route 将会读取您所选文件中的注释内容，
          以识别标记并归类修改。
        </p>
        <p>
          软件为<strong>本地计算与运行</strong>：所有解析仅在您本机进行，
          文件内容不会上传到任何远程服务器。
        </p>
        <p>
          但配置数据部分将会记录您所选项目的部分文件元数据（例如路径、修改计数、文件类型），
          请注意保护隐私。
        </p>
      </div>
    </section>
  );
}

// ---------------------------------------------------------------------------
// HistoryPage — chronological commit timeline for the active project.
// ---------------------------------------------------------------------------

function HistoryPage({
  commits,
  projectName,
}: {
  commits: CommitDto[];
  projectName?: string;
}) {
  const [sort, setSort] = useState<"asc" | "desc">("desc");
  const [filter, setFilter] = useState("");

  const filtered = commits.filter((c) =>
    !filter || (c.message || "").toLowerCase().includes(filter.toLowerCase())
  );
  const sorted = [...filtered].sort((a, b) => {
    const ta = new Date(a.timestamp).getTime();
    const tb = new Date(b.timestamp).getTime();
    return sort === "asc" ? ta - tb : tb - ta;
  });

  return (
    <section className="card history-page">
      <header className="history-header">
        <h2>历史 · {projectName ?? "—"}</h2>
        <div className="history-controls">
          <input
            className="settings-input history-search"
            placeholder="搜索提交信息"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
          <div className="seg-control">
            <button
              className={sort === "desc" ? "active" : ""}
              onClick={() => setSort("desc")}
              title="最新在前"
              aria-pressed={sort === "desc"}
            >倒序</button>
            <button
              className={sort === "asc" ? "active" : ""}
              onClick={() => setSort("asc")}
              title="最旧在前"
              aria-pressed={sort === "asc"}
            >顺序</button>
          </div>
        </div>
      </header>

      {sorted.length === 0 ? (
        <div className="empty-projects">暂无修改记录</div>
      ) : (
        <ol className="timeline">
          {sorted.map((c) => (
            <li key={c.id} className="timeline-item">
              <span className="timeline-dot" aria-hidden="true" />
              <div className="timeline-card">
                <div className="timeline-row1">
                  <span className="timeline-msg">{c.message || "(无消息)"}</span>
                  <span className="timeline-time">{formatTs(c.timestamp)}</span>
                </div>
                <div className="timeline-row2">
                  <span className="timeline-author">{c.author || "anonymous"}</span>
                  <span className="timeline-id mono">{c.id.slice(0, 8)}</span>
                </div>
              </div>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
'@

$new = $head + "`n" + $append
[System.IO.File]::WriteAllText($path, $new, [System.Text.UTF8Encoding]::new($false))
Write-Output "Appended. New length: $($new.Length)"
