const fs = require("fs");
const path = "C:\\Users\\longq\\Desktop\\route (1)\\crates\\route-tauri\\web\\src\\App.tsx";

const src = `import { useCallback, useEffect, useState } from "react";
import { getInitialLocale, persistLocale, t as i18nT, type Locale } from "./i18n";
import { useTypewriter } from "./useTypewriter";
import type {
  BranchDto,
  CommitDto,
  PluginEntryDto,
  RepoStats,
  StatusDto,
  SyncTargetDto,
  TagDto,
  TreeNode,
  WorkingFileStatusDto,
} from "./api";
import {
  annotate,
  backupToDir,
  branchCreate,
  branchDelete,
  branchSwitch,
  commit,
  exportData,
  exportFile,
  historyTree,
  initRepo,
  logCommits,
  openRepo,
  pickFolder,
  pluginInstall,
  pluginList,
  pluginRemove,
  pluginSetEnabled,
  restoreFile,
  rollback,
  statsCollect,
  statsJson,
  statsMarkdown,
  status,
  syncAdd,
  syncList,
  syncRemove,
  syncRun,
  syncSetEnabled,
  syncStart,
  syncStatus,
  syncStop,
  tagCreate,
  tagDelete,
  tagList,
  workingDirStatus,
} from "./api";
import Titlebar from "./Titlebar";
import Logo from "./Logo";

// ---------------------------------------------------------------------------
// Typewriter
// ---------------------------------------------------------------------------

function Typewriter({
  text,
  speed,
  delay,
  loop,
}: {
  text: string;
  speed?: number;
  delay?: number;
  loop?: boolean;
}) {
  const { text: out, done } = useTypewriter(text, { speed, delay, loop });
  return (
    <span className="typewriter">
      <span className="typewriter-text">{out}</span>
      <span className={\`typewriter-caret \${done ? "done" : ""}\`} aria-hidden="true" />
    </span>
  );
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type Endpoint = "local" | "cloud";

interface Project {
  id: string;
  name: string;
  path: string;
  endpoint: Endpoint;
  routeA: boolean;
  backup?: { kind: "local" | "cloud"; target: string };
  addedAt: number;
}

type Page = "workspace" | "settings" | "history";

// ---------------------------------------------------------------------------
// Local storage keys
// ---------------------------------------------------------------------------

const PROJECTS_KEY = "route:projects:v1";
const ACTIVE_PROJECT_KEY = "route:active-project:v1";
const PAGE_KEY = "route:page:v1";

function loadProjects(): Project[] {
  if (typeof localStorage === "undefined") return [];
  try {
    const raw = localStorage.getItem(PROJECTS_KEY);
    return raw ? (JSON.parse(raw) as Project[]) : [];
  } catch {
    return [];
  }
}

function saveProjects(p: Project[]) {
  try { localStorage.setItem(PROJECTS_KEY, JSON.stringify(p)); } catch {}
}

function shortName(p: string): string {
  const cleaned = p.replace(/[\\\\/]+$/, "");
  const parts = cleaned.split(/[\\\\/]/).filter(Boolean);
  return parts[parts.length - 1] || cleaned;
}

function formatTs(ts: string | number): string {
  const d = new Date(ts);
  if (isNaN(d.getTime())) return "—";
  return d.toLocaleString();
}

function downloadText(name: string, text: string) {
  const blob = new Blob([text], { type: "text/plain;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// ---------------------------------------------------------------------------
// Main App
// ---------------------------------------------------------------------------

export default function App() {
  // Data
  const [info, setInfo] = useState<StatusDto | null>(null);
  const [tree, setTree] = useState<TreeNode | null>(null);
  const [commits, setCommits] = useState<CommitDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  // Project registry
  const [projects, setProjects] = useState<Project[]>(() => loadProjects());
  const [activeProjectId, setActiveProjectId] = useState<string | null>(
    () => (typeof localStorage === "undefined" ? null : localStorage.getItem(ACTIVE_PROJECT_KEY))
  );

  // Top-level page (workspace / settings / history)
  const [page, setPage] = useState<Page>(
    () => (typeof localStorage === "undefined" ? "workspace" : ((localStorage.getItem(PAGE_KEY) as Page) || "workspace"))
  );
  useEffect(() => { try { localStorage.setItem(PAGE_KEY, page); } catch {} }, [page]);

  // Derived
  const activeProject = projects.find((p) => p.id === activeProjectId) || null;
  const projectPath = activeProject?.path ?? null;
  const endpoint: Endpoint = activeProject?.endpoint ?? "local";
  const routeaEnabled: boolean = activeProject?.routeA ?? false;

  useEffect(() => { saveProjects(projects); }, [projects]);
  useEffect(() => {
    if (activeProjectId) try { localStorage.setItem(ACTIVE_PROJECT_KEY, activeProjectId); } catch {}
  }, [activeProjectId]);

  // Master switch
  const [appOn, setAppOn] = useState<boolean>(() => {
    if (typeof localStorage === "undefined") return false;
    return localStorage.getItem("route:app:on") === "1";
  });
  useEffect(() => {
    try { localStorage.setItem("route:app:on", appOn ? "1" : "0"); } catch {}
  }, [appOn]);

  // Locale + welcome state machine
  const [locale, setLocale] = useState<Locale>(getInitialLocale);
  const [languageStage, setLanguageStage] = useState<"language" | "folder">(() => {
    if (typeof localStorage === "undefined") return "language";
    return localStorage.getItem("route:lang:chosen:v4") === "1" ? "folder" : "language";
  });

  // Project mutators
  const addProject = useCallback(async (p: string) => {
    const id = \`p_\${Date.now().toString(36)}_\${Math.random().toString(36).slice(2, 8)}\`;
    const fresh: Project = {
      id, name: shortName(p), path: p, endpoint: "local", routeA: false, addedAt: Date.now(),
    };
    setProjects((prev) => {
      const existing = prev.find((x) => x.path === p);
      if (existing) { setActiveProjectId(existing.id); return prev; }
      setActiveProjectId(id);
      return [...prev, fresh];
    });
    try {
      const s = await openRepo(p).catch(() => initRepo(p));
      setInfo(s);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const removeProject = useCallback((id: string) => {
    setProjects((prev) => {
      const next = prev.filter((p) => p.id !== id);
      if (id === activeProjectId) {
        setActiveProjectId(next[0]?.id ?? null);
        setInfo(null);
      }
      return next;
    });
  }, [activeProjectId]);

  const setEndpoint = useCallback((e: Endpoint) => {
    if (!activeProjectId) return;
    setProjects((prev) => prev.map((p) => (p.id === activeProjectId ? { ...p, endpoint: e } : p)));
  }, [activeProjectId]);

  const toggleRoutea = useCallback(() => {
    if (!activeProjectId) return;
    setProjects((prev) => prev.map((p) => {
      if (p.id !== activeProjectId) return p;
      const next = !p.routeA;
      if (next) {
        const ok = window.confirm(
          "开启后，Route 将读取您所选文件中的注释内容，以识别 route 标记并自动归类修改。\\n\\n软件为本地计算与运行：所有解析仅在您本机进行，文件内容不会上传到任何远程服务器。\\n\\n但配置数据部分将会记录您所选项目的部分文件元数据，请注意保护隐私。\\n\\n确认开启？"
        );
        if (!ok) return p;
      }
      return { ...p, routeA: next };
    }));
  }, [activeProjectId]);

  const handleLanguageSelect = useCallback((loc: Locale) => {
    setLocale(loc);
    persistLocale(loc);
    try { localStorage.setItem("route:lang:chosen:v4", "1"); } catch {}
    setLanguageStage("folder");
  }, []);

  const refresh = useCallback(async () => {
    if (!projectPath) return;
    setLoading(true);
    try {
      const [s, t, c] = await Promise.all([status(), historyTree(), logCommits(100, null)]);
      setInfo(s);
      setTree(t);
      setCommits(c);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [projectPath]);

  // Restore last project on mount.
  useEffect(() => {
    if (typeof localStorage === "undefined") return;
    const saved = localStorage.getItem("route:lastProject");
    if (saved && !activeProjectId) addProject(saved);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handlePickFolder = useCallback(async () => {
    setError(null);
    const folder = await pickFolder();
    if (!folder) return;
    await addProject(folder);
    await refresh();
  }, [addProject, refresh]);

  // -----------------------------------------------------------------------
  // Empty state (welcome page)
  // -----------------------------------------------------------------------

  if (!projectPath || !info) {
    const tr = i18nT(locale);
    const languageChosen = languageStage === "folder";
    return (
      <div className="app-shell">
        <Titlebar
          appOn={appOn}
          onToggleApp={() => setAppOn((v) => !v)}
        />
        <div className="empty-state">
          <div className="welcome-aura" aria-hidden="true">
            <span className="welcome-aura-source" />
            <span className="welcome-aura-halo" aria-hidden="true" />
            <span className="welcome-aura-wave wave-1" />
            <span className="welcome-aura-glyph-wave glyph-wave-1">
              <Logo size={360} withWordmark={false} variant="outline" />
            </span>
            <span className="welcome-aura-glyph">
              <Logo size={360} withWordmark={false} variant="outline" />
            </span>
          </div>

          <div className="welcome-title-band">
            <h1 className="welcome-title">
              <span className="title-prefix">
                <Typewriter
                  text={tr.welcome}
                  speed={locale === "en" ? 70 : 95}
                  delay={300}
                  loop={false}
                />
              </span>
              <span className="title-space">&nbsp;</span>
              <span className="title-brand">
                <Typewriter
                  text={tr.brand}
                  speed={130}
                  delay={300 + (locale === "en" ? 11 * 70 : 4 * 95) + 220}
                  loop={false}
                />
              </span>
            </h1>
          </div>

          <div className="welcome-middle">
            <div className="welcome-line" aria-hidden="true">
              <span className="welcome-line-track">
                <span className="welcome-line-glow" />
              </span>
            </div>
            <span className="welcome-line-tagline">{tr.tagline}</span>
          </div>

          <div className="welcome-cta-band">
            <div className="cta-slot" data-stage={languageChosen ? "folder" : "language"}>
              <div
                className={\`cta-shape cta-shape-language \${!languageChosen ? "is-active" : ""}\`}
                aria-hidden={languageChosen}
              >
                <div className="language-picker" role="group" aria-label={tr.chooseLanguage}>
                  <button
                    className={\`language-pill \${locale === "zh" ? "active" : ""}\`}
                    onClick={() => handleLanguageSelect("zh")}
                    aria-pressed={locale === "zh"}
                    tabIndex={languageChosen ? -1 : 0}
                  >中文</button>
                  <span className="language-divider" aria-hidden="true" />
                  <button
                    className={\`language-pill \${locale === "en" ? "active" : ""}\`}
                    onClick={() => handleLanguageSelect("en")}
                    aria-pressed={locale === "en"}
                    tabIndex={languageChosen ? -1 : 0}
                  >English</button>
                </div>
              </div>

              <div
                className={\`cta-shape cta-shape-folder \${languageChosen ? "is-active" : ""}\`}
                aria-hidden={!languageChosen}
              >
                <button
                  className="welcome-cta"
                  onClick={handlePickFolder}
                  tabIndex={languageChosen ? 0 : -1}
                >
                  <span className="cta-label">{tr.cta}</span>
                  <span className="cta-arrow">→</span>
                </button>
              </div>
            </div>
            {error && <p className="welcome-error">{error}</p>}
          </div>
        </div>
      </div>
    );
  }

  // -----------------------------------------------------------------------
  // Main UI (multi-project + 3 pages)
  // -----------------------------------------------------------------------

  return (
    <div className="app-shell">
      <Titlebar
        appOn={appOn}
        onToggleApp={() => setAppOn((v) => !v)}
      />
      <div className="app">
        <Sidebar
          projects={projects}
          activeProjectId={activeProjectId}
          onSelectProject={setActiveProjectId}
          onAddProject={handlePickFolder}
          onRemoveProject={removeProject}
          appOn={appOn}
          onToggleApp={() => setAppOn((v) => !v)}
        />

        <main className="main">
          {error && <div className="error-banner">{error}</div>}

          <div className="page-rail">
            <button
              className={page === "workspace" ? "active" : ""}
              onClick={() => setPage("workspace")}
              title="工作台"
            >
              <span className="page-glyph" aria-hidden="true">
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="2" y="2" width="5" height="5" rx="1" />
                  <rect x="9" y="2" width="5" height="5" rx="1" />
                  <rect x="2" y="9" width="5" height="5" rx="1" />
                  <rect x="9" y="9" width="5" height="5" rx="1" />
                </svg>
              </span>
              <span>工作台</span>
            </button>
            <button
              className={page === "settings" ? "active" : ""}
              onClick={() => setPage("settings")}
              title="设置"
            >
              <span className="page-glyph" aria-hidden="true">
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="8" cy="8" r="2" />
                  <path d="M8 1.5v1.8M8 12.7v1.8M14.5 8h-1.8M3.3 8H1.5M12.6 3.4 11.3 4.7M4.7 11.3 3.4 12.6M12.6 12.6 11.3 11.3M4.7 4.7 3.4 3.4" />
                </svg>
              </span>
              <span>设置</span>
            </button>
            <button
              className={page === "history" ? "active" : ""}
              onClick={() => setPage("history")}
              title="历史"
            >
              <span className="page-glyph" aria-hidden="true">
                <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round">
                  <circle cx="8" cy="8" r="6" />
                  <path d="M8 4v4l2.5 1.5" />
                </svg>
              </span>
              <span>历史</span>
            </button>
          </div>

          {page === "workspace" && (
            <BasicMode
              endpoint={endpoint}
              onEndpointChange={setEndpoint}
              routeaEnabled={routeaEnabled}
              onToggleRoutea={toggleRoutea}
              appOn={appOn}
              onError={setError}
              loading={loading}
              info={info}
              tree={tree}
              commits={commits}
              onRefresh={refresh}
            />
          )}

          {page === "settings" && (
            <SettingsPage
              project={activeProject}
              endpoint={endpoint}
              onEndpointChange={setEndpoint}
              routeaEnabled={routeaEnabled}
              onToggleRoutea={toggleRoutea}
              locale={locale}
              onLocaleChange={setLocale}
            />
          )}

          {page === "history" && (
            <HistoryPage
              commits={commits}
              projectName={activeProject?.name}
            />
          )}
        </main>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Sidebar
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
          className={\`sidebar-master-switch \${appOn ? "is-on" : "is-off"}\`}
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
              className={\`project-card \${isActive ? "active" : ""}\`}
              onClick={() => onSelectProject(p.id)}
            >
              <div className="project-card-row1">
                <span className="project-card-name">{p.name}</span>
                <button
                  className="project-card-remove"
                  onClick={(e) => {
                    e.stopPropagation();
                    if (confirm(\`从列表中移除「\${p.name}」?（不会删除磁盘上的文件夹）\`)) {
                      onRemoveProject(p.id);
                    }
                  }}
                  title="移除项目"
                  aria-label="移除项目"
                >×</button>
              </div>
              <div className="project-card-row2">
                <span className={\`ep-pill ep-\${p.endpoint}\`}>
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
// SettingsPage
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
          <input
            className="settings-input"
            placeholder={endpoint === "local"
              ? "例如 D:\\\\route-backups\\\\this-project"
              : "s3://bucket/path 或 webdav://host/path"}
            defaultValue={project.backup?.target ?? ""}
            readOnly
          />
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
            <span className={\`status \${routeaEnabled ? "on" : "off"}\`}>
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
// HistoryPage
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

// ---------------------------------------------------------------------------
// BasicMode (placeholder — workspace page)
// ---------------------------------------------------------------------------

function BasicMode({
  endpoint,
  routeaEnabled,
  appOn,
  onError,
  loading,
  info,
  tree,
  commits,
  onRefresh,
}: {
  endpoint: Endpoint;
  onEndpointChange: (e: Endpoint) => void;
  routeaEnabled: boolean;
  onToggleRoutea: () => void;
  appOn: boolean;
  onError: (s: string | null) => void;
  loading: boolean;
  info: StatusDto | null;
  tree: TreeNode | null;
  commits: CommitDto[];
  onRefresh: () => void;
}) {
  return (
    <section className="card">
      <h2>工作台</h2>
      <div className="gate-banner">
        <span className="pill">基础模式</span>
        <span>端点 {endpoint === "local" ? "本地" : "云端"} · route A {routeaEnabled ? "已开启" : "未开启"} · 总控 {appOn ? "运行" : "关闭"}</span>
      </div>
      <div className="workspace-stats">
        <div className="stat-card">
          <span className="stat-label">当前分支</span>
          <span className="stat-value">{info?.current_branch ?? "—"}</span>
        </div>
        <div className="stat-card">
          <span className="stat-label">待处理</span>
          <span className="stat-value">{info?.pending ?? 0}</span>
        </div>
        <div className="stat-card">
          <span className="stat-label">提交数</span>
          <span className="stat-value">{commits.length}</span>
        </div>
        <div className="stat-card">
          <span className="stat-label">文件数</span>
          <span className="stat-value">{tree ? countFiles(tree) : 0}</span>
        </div>
      </div>
      <div className="workspace-actions">
        <button className="primary" onClick={onRefresh} disabled={loading}>
          {loading ? "刷新中…" : "刷新"}
        </button>
        <button
          onClick={async () => {
            const text = await exportData("markdown");
            downloadText("route-export.md", text);
          }}
        >导出 Markdown</button>
        <button
          onClick={() => {
            const target = prompt("全量备份到哪个目录？（绝对路径）");
            if (target) onRefresh();
          }}
        >全量备份到目录</button>
      </div>
    </section>
  );
}

function countFiles(node: TreeNode | null | undefined): number {
  if (!node) return 0;
  if (node.kind === "file" || node.kind === "snapshot") return 1;
  const children = (node as { children?: TreeNode[] }).children ?? [];
  return children.reduce((sum, c) => sum + countFiles(c), 0);
}
`;

fs.writeFileSync(path, src, "utf8");
console.log("Wrote App.tsx (" + src.length + " bytes)");
