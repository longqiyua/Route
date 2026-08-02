import { useEffect, useMemo, useRef, useState } from "react";
import {
  AiOperatorDto,
  BranchDto,
  CommitDto,
  StatusDto,
  TrackConfigDto,
  TreeNode,
  WatchStatusDto,
  aiConflictReport,
  aiConflictResolve,
  aiSummaryPrompt,
  aiChat,
  workingDirStatus,
  backupToDir,
  branchCreate,
  branchMerge,
  branchSwitch,
  checkpointCreate,
  clearAiOperator,
  commit,
  exportData,
  getAiOperator,
  gitBranchSwitch,
  gitBranchCreate,
  gitDetect,
  gitInit,
  gitCommit,
  gitLog,
  gitMerge,
  gitModeSet,
  gitStatus,
  gitRollback,
  gitDiff,
  gitRestore,
  gitStashList,
  gitStashPush,
  gitStashPop,
  gitTagCreate,
  gitTagDelete,
  gitTagList,
  historyTree,
  initRepo,
  logCommits,
  mcpGetConfig,
  type McpConfigDto,
  openRepo,
  pickFolder,
  rollback,
  routeIndexPath,
  routeIndexRefresh,
  setAiOperator as setAiOperatorApi,
  status,
  trackGet,
  trackSet,
  trackSetAll,
  trackSetMemoryBufferMs,
  trackSetOn,
  trackSetVerifySha256,
  watchStart,
  watchStatus as watchStatusApi,
  watchStop,
  autostartGet,
  autostartSet,
  type AutostartConfig,
  type GitDetectDto,
  type GitLogEntryDto,
  type AiProvider,
  type ChatMessage,
} from "./api";
import { isTauriBridgeAvailable } from "./ipc";
import {
  t as i18nT,
  getInitialLocale,
  persistLocale,
  type Locale,
  type Dict,
} from "./i18n";
import Titlebar from "./Titlebar";
import Logo from "./Logo";
import Welcome from "./Welcome";
import { useTypewriter } from "./useTypewriter";

// ---------------------------------------------------------------------------
// DEBUG instrumentation — settings-black-screen
// Report key events to the local debug server so we can see what happens
// immediately before the screen goes black. Wrapped in collapsible regions so
// it is easy to remove later.
// ---------------------------------------------------------------------------

// #region debug-point logger
const DEBUG_URL = "http://127.0.0.1:7777/event";
const DEBUG_SESSION = "settings-black-screen";

let debugRunId = "pre";
function setDebugRunId(id: "pre" | "post") {
  debugRunId = id;
}

function dbg(event: string, payload?: Record<string, unknown>) {
  try {
    const body = JSON.stringify({
      session: DEBUG_SESSION,
      run: debugRunId,
      ts: Date.now(),
      event,
      payload: payload || {},
      url: typeof window !== "undefined" ? window.location.href : "",
    });
    // Fire-and-forget; in a crashing webview this may not complete, but it
    // often does if the crash happens on the next paint frame.
    if (typeof fetch !== "undefined") {
      fetch(DEBUG_URL, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body,
      }).catch(() => {});
    }
  } catch {
    // never throw because of instrumentation
  }
}
// #endregion debug-point logger

// ---------------------------------------------------------------------------
// Typewriter — small welcome-page helper. Kept as a separate component so the
// main render path can stay declarative. Renders an inline-flex span with a
// blinking caret; once the string is fully typed the caret disappears.
// ---------------------------------------------------------------------------

function Typewriter({ text, speed = 90, delay = 0 }: { text: string; speed?: number; delay?: number }) {
  const { text: out, done } = useTypewriter(text, { speed, delay });
  return (
    <span className="typewriter">
      <span className="typewriter-text">{out}</span>
      <span className={`typewriter-caret${done ? " done" : ""}`} />
    </span>
  );
}

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

type ProjectMode = "standard" | "ai";
type TrackingState = "off" | "starting" | "running";

interface Project {
  id: string;
  name: string;
  path: string;
  routeA: boolean;
  backup?: {
    kind: "local" | "cloud";
    target: string;
    method?: "mirror" | "incremental" | "standard";
    mirrorDelay?: number;
  };
  addedAt: number;
}

type Page = "workspace" | "settings";
// Sort modes for the project list. "alpha" / "time" / "reverse" form the
// click-cycle on the square sort button; "custom" is only entered by
// dragging a card (preserved manual order) and is not part of the cycle.
type ProjectSort = "alpha" | "time" | "reverse" | "custom";

// ---------------------------------------------------------------------------
// Local storage keys + small pure helpers
// ---------------------------------------------------------------------------

const PROJECTS_KEY = "route:projects";
const PAGE_KEY = "route:page";
const APP_ON_KEY = "route:appOn";
const TRACKING_STATES_KEY = "route:trackingStates";
const MODES_KEY = "route:projectModes";
const INIT_KEY = "route:projectInitialized";
const SORT_KEY = "route:projectSortMode";
const CLI_MCP_KEY = "route:cliMcpEnabled";
const AI_ASSISTANT_MODE_KEY = "route:aiAssistantMode";
const AI_API_KEY_STR = "route:aiApiKey";
const AI_API_ENDPOINT_KEY = "route:aiApiEndpoint";
const AI_PROVIDER_KEY = "route:aiProvider";
const AI_MODEL_KEY = "route:aiModel";
const GIT_MODE_KEY = "route:gitModeEnabled";

/// Local alias for the git_detect result so the SettingsPage signature
/// reads cleanly. Mirrors `GitDetectDto` from api.ts.
type GitDetectResult = GitDetectDto;

function safeParse<T>(raw: string | null): T | null {
  if (!raw) return null;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

function loadProjects(): Project[] {
  if (typeof localStorage === "undefined") return [];
  const parsed = safeParse<Project[]>(localStorage.getItem(PROJECTS_KEY));
  if (!Array.isArray(parsed)) return [];
  return parsed.filter(
    (p) => p && typeof p.id === "string" && typeof p.name === "string" && typeof p.path === "string",
  );
}

function saveProjects(projects: Project[]) {
  try {
    localStorage.setItem(PROJECTS_KEY, JSON.stringify(projects));
  } catch {
    // ignore
  }
}

function loadPage(): Page {
  if (typeof localStorage === "undefined") return "workspace";
  const v = localStorage.getItem(PAGE_KEY);
  // "timeline" was removed when the timeline view merged into the
  // workbench; old localStorage values silently fall back to workspace.
  if (v === "workspace" || v === "settings") return v;
  return "workspace";
}

function persistPage(page: Page) {
  try {
    localStorage.setItem(PAGE_KEY, page);
  } catch {
    // ignore
  }
}

function loadAppOn(): boolean {
  if (typeof localStorage === "undefined") return false;
  const v = localStorage.getItem(APP_ON_KEY);
  if (v === "true") return true;
  return false;
}

function saveAppOn(on: boolean) {
  try {
    localStorage.setItem(APP_ON_KEY, on ? "true" : "false");
  } catch {
    // ignore
  }
}

function loadTrackingStates(): Record<string, TrackingState> {
  if (typeof localStorage === "undefined") return {};
  const parsed = safeParse<Record<string, TrackingState>>(localStorage.getItem(TRACKING_STATES_KEY));
  return parsed && typeof parsed === "object" ? parsed : {};
}

function saveTrackingStates(states: Record<string, TrackingState>) {
  try {
    localStorage.setItem(TRACKING_STATES_KEY, JSON.stringify(states));
  } catch {
    // ignore
  }
}

function loadProjectModes(): Record<string, ProjectMode> {
  if (typeof localStorage === "undefined") return {};
  const parsed = safeParse<Record<string, ProjectMode>>(localStorage.getItem(MODES_KEY));
  return parsed && typeof parsed === "object" ? parsed : {};
}

function persistProjectModes(modes: Record<string, ProjectMode>) {
  try {
    localStorage.setItem(MODES_KEY, JSON.stringify(modes));
  } catch {
    // ignore
  }
}

function loadProjectInitialized(): Record<string, boolean> {
  if (typeof localStorage === "undefined") return {};
  const parsed = safeParse<Record<string, boolean>>(localStorage.getItem(INIT_KEY));
  return parsed && typeof parsed === "object" ? parsed : {};
}

function persistProjectInitialized(flags: Record<string, boolean>) {
  try {
    localStorage.setItem(INIT_KEY, JSON.stringify(flags));
  } catch {
    // ignore
  }
}

function loadProjectSortMode(): ProjectSort {
  if (typeof localStorage === "undefined") return "alpha";
  const v = localStorage.getItem(SORT_KEY);
  if (v === "alpha" || v === "time" || v === "reverse" || v === "custom") return v;
  return "alpha";
}

function persistProjectSortMode(mode: ProjectSort) {
  try {
    localStorage.setItem(SORT_KEY, mode);
  } catch {
    // ignore
  }
}

const THEME_KEY = "route:theme";

function loadTheme(): "dark" | "light" {
  if (typeof localStorage === "undefined") return "dark";
  const v = localStorage.getItem(THEME_KEY);
  return v === "light" ? "light" : "dark";
}

function persistTheme(theme: "dark" | "light") {
  try {
    localStorage.setItem(THEME_KEY, theme);
  } catch {
    // ignore
  }
}

function loadCliMcpEnabled(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem(CLI_MCP_KEY) === "true";
}

function persistCliMcpEnabled(on: boolean) {
  try {
    localStorage.setItem(CLI_MCP_KEY, on ? "true" : "false");
  } catch {
    // ignore
  }
}

function loadGitModeEnabled(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem(GIT_MODE_KEY) === "true";
}

function persistGitModeEnabled(on: boolean) {
  try {
    localStorage.setItem(GIT_MODE_KEY, on ? "true" : "false");
  } catch {
    // ignore
  }
}

type AiAssistantMode = "follow" | "active";

function loadAiAssistantMode(): AiAssistantMode {
  if (typeof localStorage === "undefined") return "follow";
  return localStorage.getItem(AI_ASSISTANT_MODE_KEY) === "active" ? "active" : "follow";
}

function persistAiAssistantMode(mode: AiAssistantMode) {
  try {
    localStorage.setItem(AI_ASSISTANT_MODE_KEY, mode);
  } catch {
    // ignore
  }
}

function loadAiApiKey(): string {
  if (typeof localStorage === "undefined") return "";
  return localStorage.getItem(AI_API_KEY_STR) || "";
}

function persistAiApiKey(key: string) {
  try {
    localStorage.setItem(AI_API_KEY_STR, key);
  } catch {
    // ignore
  }
}

function loadAiApiEndpoint(): string {
  if (typeof localStorage === "undefined") return "";
  return localStorage.getItem(AI_API_ENDPOINT_KEY) || "";
}

function persistAiApiEndpoint(endpoint: string) {
  try {
    localStorage.setItem(AI_API_ENDPOINT_KEY, endpoint);
  } catch {
    // ignore
  }
}

const AI_PROVIDER_DEFAULT: AiProvider = "openai";

// Canonical endpoint default per provider. Used to auto-fill the endpoint
// field when the user switches providers and the current value is empty or
// still holds a previous provider's default — so they don't have to retype.
const PROVIDER_DEFAULT_ENDPOINT: Record<AiProvider, string> = {
  openai: "https://api.openai.com/v1",
  anthropic: "https://api.anthropic.com",
  ollama: "http://localhost:11434",
};

function loadAiProvider(): AiProvider {
  if (typeof localStorage === "undefined") return AI_PROVIDER_DEFAULT;
  const v = localStorage.getItem(AI_PROVIDER_KEY);
  if (v === "openai" || v === "anthropic" || v === "ollama") return v;
  return AI_PROVIDER_DEFAULT;
}

function persistAiProvider(p: AiProvider) {
  try {
    localStorage.setItem(AI_PROVIDER_KEY, p);
  } catch {
    // ignore
  }
}

function loadAiModel(): string {
  if (typeof localStorage === "undefined") return "";
  return localStorage.getItem(AI_MODEL_KEY) || "";
}

function persistAiModel(m: string) {
  try {
    localStorage.setItem(AI_MODEL_KEY, m);
  } catch {
    // ignore
  }
}

function shortName(path: string): string {
  if (!path) return "";
  const parts = path.replace(/[/\\]+$/, "").split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

function formatTs(ms: number): string {
  if (!ms || ms <= 0) return "";
  try {
    const d = new Date(ms);
    return d.toLocaleString();
  } catch {
    return "";
  }
}

function localizeBridgeError(e: unknown): string {
  const msg = (e as Error)?.message || String(e);
  if (msg && /tauri bridge unavailable/i.test(msg)) {
    return "bridge";
  }
  return msg || "Action failed";
}

/// Map a git log entry to the shape the timeline already renders. Git mode
/// reuses the existing CommitDto so TimelinePage doesn't need a parallel
/// code path — only the fields the timeline actually reads are populated;
/// the rest default to null/empty so nothing else breaks.
function gitLogEntryToCommitDto(g: GitLogEntryDto): CommitDto {
  return {
    id: g.sha,
    short_id: g.sha.slice(0, 7),
    from_snapshot: "",
    to_snapshot: g.sha,
    message: g.message,
    author: null,
    created_at: g.timestamp_ms,
    branch_id: "",
    branch_name: "",
    kind: "incremental",
    diff_summary: null,
    operator: g.is_ai ? "ai" : "user",
    body: null,
    is_checkpoint: g.is_checkpoint,
    is_ai: g.is_ai,
  };
}

// ---------------------------------------------------------------------------
// Sidebar — project list, master switch, page switcher, drag-to-reorder
// ---------------------------------------------------------------------------

function PlusIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
      <path d="M6 2v8M2 6h8" />
    </svg>
  );
}

function WorkspaceIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinejoin="round">
      <rect x="2" y="3" width="8" height="6" rx="1" />
      <path d="M2 6h8" />
    </svg>
  );
}

function TimelineIcon() {
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round">
      <circle cx="3" cy="3" r="1" />
      <circle cx="3" cy="9" r="1" />
      <circle cx="9" cy="6" r="1" />
      <path d="M3 4v4" />
      <path d="M4 6h4" />
    </svg>
  );
}

function SettingsIcon() {
  // Sliders / equalizer glyph — three horizontal tracks, each broken by a
  // knob at a different position. Reads as "controls / settings" without
  // being another gear. Stroke-only, so it inherits currentColor cleanly.
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round">
      <path d="M2 3.5h2.5M7.5 3.5h2.5" />
      <path d="M2 6h2.5M7.5 6h2.5" />
      <path d="M2 8.5h2.5M7.5 8.5h2.5" />
      <circle cx="6" cy="3.5" r="1.4" />
      <circle cx="4.5" cy="6" r="1.4" />
      <circle cx="7.5" cy="8.5" r="1.4" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// Sort glyphs for the square cycle button. One per mode so the icon itself
// tells the user which ordering is active; the title attribute carries the
// localized label as a fallback.
// ---------------------------------------------------------------------------

function SortAlphaIcon() {
  // Down arrow + three decreasing bars → A→Z (alphabetical, ascending)
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round">
      <path d="M2 2v6" />
      <path d="M0.6 6.8L2 8.2L3.4 6.8" />
      <path d="M5.5 3h4.5M5.5 6h3.5M5.5 9h2.5" />
    </svg>
  );
}

function SortTimeIcon() {
  // Clock face → by time (newest first)
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round">
      <circle cx="6" cy="6" r="4" />
      <path d="M6 3.6V6l1.8 1.2" />
    </svg>
  );
}

function SortReverseIcon() {
  // Up arrow + three increasing bars → Z→A (reverse alphabetical)
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round">
      <path d="M2 10V4" />
      <path d="M0.6 5.2L2 3.8L3.4 5.2" />
      <path d="M5.5 3h2.5M5.5 6h3.5M5.5 9h4.5" />
    </svg>
  );
}

function SortCustomIcon() {
  // Drag handle → manual / custom order (only reached by dragging a card)
  return (
    <svg viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round">
      <path d="M3 4.5h6M3 7.5h6" />
    </svg>
  );
}

function Sidebar({
  projects,
  activeProjectId,
  onSelectProject,
  onAddProject,
  onRemoveProject,
  onReorderProjects,
  sortMode,
  onSortModeChange,
  page,
  onPageChange,
  activeTrackingState,
  onToggleTracking,
  watchStatus,
  tr,
}: {
  projects: Project[];
  activeProjectId: string | null;
  onSelectProject: (id: string) => void;
  onAddProject: () => void;
  onRemoveProject: (id: string) => void;
  onReorderProjects: (fromId: string, toId: string) => void;
  sortMode: ProjectSort;
  onSortModeChange: (m: ProjectSort) => void;
  page: Page;
  onPageChange: (p: Page) => void;
  activeTrackingState: TrackingState;
  onToggleTracking: () => void;
  watchStatus: WatchStatusDto | null;
  tr: Dict;
}) {
  const [draggingId, setDraggingId] = useState<string | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);

  const sortedProjects = useMemo(() => {
    const arr = [...projects];
    if (sortMode === "alpha") {
      arr.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
    } else if (sortMode === "time") {
      arr.sort((a, b) => b.addedAt - a.addedAt);
    } else if (sortMode === "reverse") {
      arr.sort((a, b) => b.name.localeCompare(a.name, undefined, { sensitivity: "base" }));
    }
    // "custom" — use the array as-is (user's drag order)
    return arr;
  }, [projects, sortMode]);

  const handleDragStart = (e: React.DragEvent, id: string) => {
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", id);
    setDraggingId(id);
  };
  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
  };
  const handleDragEnterCard = (id: string) => setDragOverId(id);
  const handleDragLeaveCard = () => setDragOverId(null);
  const handleDropOnCard = (e: React.DragEvent, targetId: string) => {
    e.preventDefault();
    const sourceId = e.dataTransfer.getData("text/plain");
    if (sourceId && sourceId !== targetId) {
      onReorderProjects(sourceId, targetId);
    }
    setDraggingId(null);
    setDragOverId(null);
  };

  // When the user starts dragging, switch to "custom" mode so the
  // manual order is respected instead of being overwritten by the
  // alpha / time sort on the next render.
  const handleDragStartCustom = (e: React.DragEvent, id: string) => {
    if (sortMode !== "custom") {
      onSortModeChange("custom");
    }
    handleDragStart(e, id);
  };
  const handleDragEnd = () => {
    setDraggingId(null);
    setDragOverId(null);
  };

  const isActive = (p: Project) => p.id === activeProjectId;
  const hasBackup = (p: Project) => !!p.backup && !!p.backup.target;
  const isTracking = (p: Project) =>
    isActive(p) && activeTrackingState === "running";
  const isStarting = (p: Project) =>
    isActive(p) && activeTrackingState === "starting";
  const isBackupButIdle = (p: Project) =>
    isActive(p) && hasBackup(p) && !isTracking(p) && !isStarting(p);

  // Square sort button cycles alpha → time → reverse → alpha …
  // "custom" is only reached by dragging a card; from there a click
  // restarts the cycle at "alpha".
  const cycleSortMode = () => {
    const order: ProjectSort[] = ["alpha", "time", "reverse"];
    const idx = order.indexOf(sortMode);
    onSortModeChange(idx === -1 ? "alpha" : order[(idx + 1) % order.length]);
  };
  const sortLabel =
    sortMode === "alpha"
      ? tr.sortAlpha
      : sortMode === "time"
        ? tr.sortTime
        : sortMode === "reverse"
          ? tr.sortReverse
          : tr.sortCustom;

  return (
    <aside className="sidebar">
      <button
        type="button"
        className={`sidebar-power state-${activeTrackingState}`}
        onClick={onToggleTracking}
        aria-pressed={activeTrackingState !== "off"}
      >
        <span className="sidebar-power-dot" />
        <span className="sidebar-power-text">
          {activeTrackingState === "off"
            ? tr.masterOff
            : activeTrackingState === "starting"
              ? tr.trackingStarting
              : tr.masterOn}
        </span>
      </button>

      <div className="sidebar-add-row">
        <button type="button" className="sidebar-add" onClick={onAddProject}>
          <span className="sidebar-add-icon">
            <PlusIcon />
          </span>
          {tr.addProject}
        </button>
        <button
          type="button"
          className="sidebar-sort"
          onClick={cycleSortMode}
          title={sortLabel}
          aria-label={sortLabel}
        >
          {sortMode === "alpha" ? (
            <SortAlphaIcon />
          ) : sortMode === "time" ? (
            <SortTimeIcon />
          ) : sortMode === "reverse" ? (
            <SortReverseIcon />
          ) : (
            <SortCustomIcon />
          )}
        </button>
      </div>

      <div className="project-cards">
        {sortedProjects.length === 0 ? (
          <div className="empty-projects">{tr.noProjects}</div>
        ) : (
          sortedProjects.map((p) => {
            const active = isActive(p);
            const tracking = isTracking(p);
            const starting = isStarting(p);
            const idle = isBackupButIdle(p);
            const nameStyle: React.CSSProperties = {};
            if (tracking) nameStyle.color = "var(--accent-bright)";
            else if (starting) nameStyle.color = "var(--accent-deep)";
            else if (idle) nameStyle.color = "var(--accent-deep)";
            const isDragging = draggingId === p.id;
            const isDropTarget = dragOverId === p.id && !isDragging;
            return (
              <div
                key={p.id}
                className={`project-card${active ? " active" : ""}`}
                draggable
                onDragStart={(e) => handleDragStartCustom(e, p.id)}
                onDragOver={handleDragOver}
                onDragEnter={() => handleDragEnterCard(p.id)}
                onDragLeave={handleDragLeaveCard}
                onDrop={(e) => handleDropOnCard(e, p.id)}
                onDragEnd={handleDragEnd}
                onClick={() => onSelectProject(p.id)}
                style={{
                  opacity: isDragging ? 0.4 : 1,
                  borderColor: isDropTarget ? "var(--accent-line)" : undefined,
                  borderStyle: isDropTarget ? "dashed" : undefined,
                }}
                title={p.path}
              >
                <div className="project-card-row1">
                  <span className="project-card-name" style={nameStyle}>
                    {p.name}
                  </span>
                  <button
                    type="button"
                    className="project-card-remove"
                    onClick={(e) => {
                      e.stopPropagation();
                      onRemoveProject(p.id);
                    }}
                    title={tr.remove}
                    aria-label={tr.remove}
                  >
                    ×
                  </button>
                </div>
                <div
                  className="project-card-path"
                  onClick={(e) => {
                    e.stopPropagation();
                    try {
                      navigator.clipboard?.writeText(p.path);
                    } catch {
                      // ignore
                    }
                  }}
                  title={tr.clickToCopy}
                >
                  {p.path}
                </div>
              </div>
            );
          })
        )}
      </div>

      <div className="page-rail" style={{ marginTop: "auto" }}>
        <button
          type="button"
          className={page === "workspace" ? "active" : ""}
          onClick={() => onPageChange("workspace")}
        >
          <span className="page-glyph">
            <WorkspaceIcon />
          </span>
          {tr.pageWorkspace}
        </button>
        <button
          type="button"
          className={page === "settings" ? "active" : ""}
          onClick={() => onPageChange("settings")}
        >
          <span className="page-glyph">
            <SettingsIcon />
          </span>
          {tr.pageSettings}
        </button>
      </div>
    </aside>
  );
}

// ---------------------------------------------------------------------------
// HoldButton — must be held for `holdMs` before firing. Used for the danger
// zone actions in the settings page so a stray click cannot reset / clear
// user data.
// ---------------------------------------------------------------------------

function HoldButton({
  holdMs,
  label,
  doneLabel,
  onComplete,
  destructive = false,
}: {
  holdMs: number;
  label: string;
  doneLabel: string;
  onComplete: () => void;
  destructive?: boolean;
}) {
  const [progress, setProgress] = useState(0);
  const [fired, setFired] = useState(false);
  const startedAtRef = useRef<number | null>(null);
  const rafRef = useRef<number | null>(null);

  const stop = () => {
    if (rafRef.current !== null) {
      cancelAnimationFrame(rafRef.current);
      rafRef.current = null;
    }
    startedAtRef.current = null;
    if (!fired) setProgress(0);
  };

  const start = () => {
    if (fired) return;
    startedAtRef.current = performance.now();
    const tick = () => {
      const start = startedAtRef.current;
      if (start === null) return;
      const elapsed = performance.now() - start;
      const pct = Math.min(1, elapsed / holdMs);
      setProgress(pct);
      if (pct >= 1) {
        setFired(true);
        onComplete();
        return;
      }
      rafRef.current = requestAnimationFrame(tick);
    };
    rafRef.current = requestAnimationFrame(tick);
  };

  useEffect(() => {
    if (!fired) return;
    const id = window.setTimeout(() => {
      setFired(false);
      setProgress(0);
    }, 1800);
    return () => window.clearTimeout(id);
  }, [fired]);

  const r = 7;
  const c = 2 * Math.PI * r;
  const offset = c * (1 - progress);

  return (
    <button
      type="button"
      className={`hold-button${destructive ? " hold-destructive" : ""}${fired ? " is-fired" : ""}`}
      onPointerDown={start}
      onPointerUp={stop}
      onPointerLeave={stop}
      onPointerCancel={stop}
    >
      <span className="hold-ring" aria-hidden>
        <svg viewBox="0 0 18 18">
          <circle className="hold-ring-track" cx="9" cy="9" r={r} />
          <circle
            className="hold-ring-fill"
            cx="9"
            cy="9"
            r={r}
            strokeDasharray={c}
            strokeDashoffset={offset}
            transform="rotate(-90 9 9)"
          />
        </svg>
      </span>
      <span className="hold-label">
        {fired ? doneLabel : label}
        <span className="hold-pct">{Math.round(progress * 100)}%</span>
      </span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// TrackConfigPanel — surfaced inside the settings page; bound to `track`
// state. The Rust side stores a single TrackConfigDto; we mirror it.
// ---------------------------------------------------------------------------

function TrackConfigPanel({
  track,
  onSetAll,
  onSetSuffixes,
  onSetPrefixes,
  onSetVerifySha256,
  onSetMemoryBufferMs,
  onSetOn,
  tr,
}: {
  track: TrackConfigDto;
  onSetAll: (on: boolean) => void;
  onSetSuffixes: (s: string[]) => void;
  onSetPrefixes: (p: string[]) => void;
  onSetVerifySha256: (on: boolean) => void;
  onSetMemoryBufferMs: (ms: number) => void;
  onSetOn: (on: { windows: boolean; macos: boolean; linux: boolean }) => void;
  tr: Dict;
}) {
  return (
    <div className="settings-subcard">
      <h4 className="settings-subcard-title">{tr.trackSection}</h4>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>{tr.trackAllLabel}</span>
          <span className="settings-row-hint">{tr.trackAllHint}</span>
        </div>
        <div className="settings-row-control">
          <Switch
            checked={track.track_all}
            onChange={onSetAll}
            statusText={
              <span className={track.track_all ? "on" : ""}>
                {track.track_all ? tr.masterOn : tr.masterOff}
              </span>
            }
          />
        </div>
      </div>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>{tr.trackSuffixesLabel}</span>
          <span className="settings-row-hint">{tr.trackSuffixesHint}</span>
        </div>
        <div className="settings-row-control settings-row-control-input">
          <input
            className="settings-input"
            value={track.track_suffixes.join(",")}
            onChange={(e) =>
              onSetSuffixes(
                e.target.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean),
              )
            }
            placeholder=".py,.js,.tsx"
          />
        </div>
      </div>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>{tr.trackPrefixesLabel}</span>
          <span className="settings-row-hint">{tr.trackPrefixesHint}</span>
        </div>
        <div className="settings-row-control settings-row-control-input">
          <input
            className="settings-input"
            value={track.track_prefixes.join(",")}
            onChange={(e) =>
              onSetPrefixes(
                e.target.value
                  .split(",")
                  .map((s) => s.trim())
                  .filter(Boolean),
              )
            }
            placeholder="src/,docs/"
          />
        </div>
      </div>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>{tr.trackVerifySha256Label}</span>
          <span className="settings-row-hint">{tr.trackVerifySha256Hint}</span>
        </div>
        <div className="settings-row-control">
          <Switch
            checked={track.verify_sha256}
            onChange={onSetVerifySha256}
            statusText={
              <span className={track.verify_sha256 ? "on" : ""}>
                {track.verify_sha256 ? tr.masterOn : tr.masterOff}
              </span>
            }
          />
        </div>
      </div>

      <div className="settings-row">
        <div className="settings-row-label">
          <span>{tr.trackMemoryBufferLabel}</span>
          <span className="settings-row-hint">{tr.trackMemoryBufferHint}</span>
        </div>
        <div className="settings-row-control">
          <input
            className="settings-input"
            type="number"
            min={0}
            step={50}
            value={track.memory_buffer_ms}
            onChange={(e) => {
              const v = Number(e.target.value);
              onSetMemoryBufferMs(Number.isFinite(v) && v >= 0 ? v : 0);
            }}
            style={{ width: 120 }}
          />
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// DangerZone — two HoldButtons: reset settings + clear all data
// ---------------------------------------------------------------------------

function DangerZone({
  onResetSettings,
  onClearAllData,
  tr,
}: {
  onResetSettings: () => void;
  onClearAllData: () => void;
  tr: Dict;
}) {
  return (
    <div className="settings-dangerzone">
      <div className="dangerzone-title">
        <span className="dangerzone-dot" />
        {tr.dangerZone}
      </div>
      <div className="dangerzone-grid">
        <div className="dangerzone-cell">
          <div className="dangerzone-cell-label">{tr.resetSettings}</div>
          <div className="dangerzone-cell-hint">{tr.resetSettingsHint}</div>
          <HoldButton
            holdMs={1000}
            label={tr.resetSettingsHold}
            doneLabel={tr.resetSettingsDone}
            onComplete={onResetSettings}
          />
        </div>
        <div className="dangerzone-cell dangerzone-cell-destructive">
          <div className="dangerzone-cell-label">{tr.clearData}</div>
          <div className="dangerzone-cell-hint">
            {tr.clearDataHint}
            <br />
            <span className="dangerzone-irreversible">
              {tr.clearDataIrreversible}
            </span>
          </div>
          <HoldButton
            holdMs={10000}
            label={tr.clearDataHold}
            doneLabel={tr.clearDataDone}
            onComplete={onClearAllData}
            destructive
          />
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// AiConflictDialog — modal that lets the user resolve one or more AI
// conflicts surfaced in the most recent AI commit's body
// ---------------------------------------------------------------------------

type AiConflictReportLike = {
  commit_id: string;
  body: string;
  conflicts: { path: string; reason: string; recommendation: string }[];
};

function AiConflictDialog({
  open,
  onClose,
  tr,
}: {
  open: boolean;
  onClose: () => void;
  tr: Dict;
}) {
  const [loading, setLoading] = useState(false);
  const [report, setReport] = useState<AiConflictReportLike | null>(null);
  const [verdicts, setVerdicts] = useState<Record<string, "keep_old" | "keep_ai" | "keep_both">>({});
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setLoading(true);
    setVerdicts({});
    setNotes({});
    (async () => {
      try {
        const r = await aiConflictReport();
        if (cancelled) return;
        setReport({
          commit_id: r.commit_id,
          body: r.body,
          conflicts: r.conflicts.map((c) => ({
            path: c.path,
            reason: c.reason,
            recommendation: c.recommendation,
          })),
        });
      } catch {
        if (!cancelled) setReport({ commit_id: "", body: "", conflicts: [] });
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open]);

  if (!open) return null;

  const handleSave = async () => {
    if (!report) return;
    setSaving(true);
    try {
      for (const c of report.conflicts) {
        const v = verdicts[c.path];
        if (!v) continue;
        await aiConflictResolve({
          commit_id: report.commit_id,
          path: c.path,
          verdict: v,
          note: notes[c.path] || null,
        });
      }
    } catch {
      // ignore
    } finally {
      setSaving(false);
      onClose();
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal-card ai-conflict-dialog"
        onClick={(e) => e.stopPropagation()}
      >
        <h3>{tr.aiConflictTitle}</h3>
        <p className="modal-hint">{tr.aiConflictHint}</p>
        {loading ? (
          <div className="ai-conflict-loading">…</div>
        ) : !report || report.conflicts.length === 0 ? (
          <div className="ai-conflict-empty">{tr.aiConflictEmpty}</div>
        ) : (
          <ul className="ai-conflict-list">
            {report.conflicts.map((c) => (
              <li key={c.path} className="ai-conflict-item">
                <div className="ai-conflict-item-head">
                  <span className="ai-conflict-path" title={c.path}>
                    {c.path}
                  </span>
                  <span className="ai-conflict-reco">
                    {c.recommendation}
                  </span>
                </div>
                <div className="ai-conflict-reason">{c.reason}</div>
                <div className="ai-conflict-actions">
                  <button
                    type="button"
                    className={`modal-btn${verdicts[c.path] === "keep_old" ? " primary" : ""}`}
                    onClick={() => setVerdicts((p) => ({ ...p, [c.path]: "keep_old" }))}
                  >
                    {tr.aiConflictKeepOld}
                  </button>
                  <button
                    type="button"
                    className={`modal-btn${verdicts[c.path] === "keep_ai" ? " primary" : ""}`}
                    onClick={() => setVerdicts((p) => ({ ...p, [c.path]: "keep_ai" }))}
                  >
                    {tr.aiConflictKeepAi}
                  </button>
                  <button
                    type="button"
                    className={`modal-btn${verdicts[c.path] === "keep_both" ? " primary" : ""}`}
                    onClick={() => setVerdicts((p) => ({ ...p, [c.path]: "keep_both" }))}
                  >
                    {tr.aiConflictKeepBoth}
                  </button>
                </div>
                <div className="modal-label">
                  <span>{tr.aiConflictNote}</span>
                  <input
                    className="settings-input"
                    value={notes[c.path] || ""}
                    onChange={(e) =>
                      setNotes((p) => ({ ...p, [c.path]: e.target.value }))
                    }
                  />
                </div>
              </li>
            ))}
          </ul>
        )}
        <div className="modal-actions">
          <button type="button" className="modal-btn" onClick={onClose} disabled={saving}>
            {tr.aiConflictSkip}
          </button>
          <button
            type="button"
            className="modal-btn primary"
            onClick={handleSave}
            disabled={saving || !report || report.conflicts.length === 0}
          >
            {tr.aiConflictSave}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// AutostartSection — self-contained settings card for boot launch,
// silent start, and startup priority. Manages its own state by calling
// the autostart API directly, so it doesn't need to thread props
// through SettingsPage.
// ---------------------------------------------------------------------------

function AutostartSection({ tr }: { tr: Dict }) {
  const [cfg, setCfg] = useState<AutostartConfig | null>(null);

  useEffect(() => {
    let cancelled = false;
    autostartGet()
      .then((c) => {
        if (!cancelled) setCfg(c);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  const update = (patch: Partial<AutostartConfig>) => {
    autostartSet(patch)
      .then((c) => setCfg(c))
      .catch(() => {});
  };

  if (!cfg) return null;

  // Renders TWO sub-cards so each lands as its own function card inside
  // the "基本" category: ① autostart (enable + silent + priority) and
  // ② close behavior (hide to tray / quit). They share the same cfg state.
  return (
    <>
      <div className="settings-subcard">
        <h4 className="settings-subcard-title">{tr.autostartSection}</h4>
        <div className="settings-row">
          <div className="settings-row-label">
            <span>{tr.autostartEnable}</span>
            <span className="settings-row-hint">{tr.autostartEnableHint}</span>
          </div>
          <div className="settings-row-control">
            <Switch
              checked={cfg.enabled}
              onChange={(checked) => update({ enabled: checked })}
              statusText={
                <span className={cfg.enabled ? "on" : ""}>
                  {cfg.enabled ? tr.cliMcpOn : tr.cliMcpOff}
                </span>
              }
            />
          </div>
        </div>
        <div className={`autostart-reveal${cfg.enabled ? " active" : ""}`}>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.autostartSilent}</span>
                <span className="settings-row-hint">{tr.autostartSilentHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={cfg.silent}
                  onChange={(checked) => update({ silent: checked })}
                  statusText={
                    <span className={cfg.silent ? "on" : ""}>
                      {cfg.silent ? tr.cliMcpOn : tr.cliMcpOff}
                    </span>
                  }
                />
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.autostartPriority}</span>
              </div>
              <div className="settings-row-control">
                <div className="seg-control">
                  <button
                    type="button"
                    className={cfg.priority === "low" ? "active" : ""}
                    onClick={() => update({ priority: "low" })}
                  >
                    {tr.autostartPriorityLow}
                  </button>
                  <button
                    type="button"
                    className={cfg.priority === "normal" ? "active" : ""}
                    onClick={() => update({ priority: "normal" })}
                  >
                    {tr.autostartPriorityNormal}
                  </button>
                  <button
                    type="button"
                    className={cfg.priority === "high" ? "active" : ""}
                    onClick={() => update({ priority: "high" })}
                  >
                    {tr.autostartPriorityHigh}
                  </button>
                </div>
              </div>
            </div>
        </div>
      </div>

      {/* Close behavior — always visible, independent of autostart.
          Own sub-card so it is a peer of autostart, not nested inside it.
          Switch: ON = hide to tray, OFF = quit the app. */}
      <div className="settings-subcard">
        <h4 className="settings-subcard-title">{tr.closeBehaviorLabel}</h4>
        <div className="settings-row">
          <div className="settings-row-label">
            <span>{tr.closeBehaviorLabel}</span>
            <span className="settings-row-hint">{tr.closeBehaviorHint}</span>
          </div>
          <div className="settings-row-control">
            <Switch
              checked={cfg.close_behavior === "hide"}
              onChange={(checked) => update({ close_behavior: checked ? "hide" : "quit" })}
              statusText={
                <span className={cfg.close_behavior === "hide" ? "on" : ""}>
                  {cfg.close_behavior === "hide" ? tr.closeBehaviorHide : tr.closeBehaviorQuit}
                </span>
              }
            />
          </div>
        </div>
      </div>
    </>
  );
}

// ---------------------------------------------------------------------------
// Switch — accessible toggle button replacing the hidden-checkbox pattern.
// WebView2's software renderer has shown instability with opacity:0 absolute
// checkbox inputs inside labels; using a real button avoids that path while
// keeping keyboard focus and aria support.
// ---------------------------------------------------------------------------

interface SwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  statusText?: React.ReactNode;
  id?: string;
}

function Switch({ checked, onChange, statusText, id }: SwitchProps) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      className={`switch switch-btn${checked ? " on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span className="track">
        <span className="thumb" />
      </span>
      {statusText && <span className="status">{statusText}</span>}
    </button>
  );
}

// ---------------------------------------------------------------------------
// SettingsPage — the entire settings surface, one card per section
// ---------------------------------------------------------------------------

function SettingsPage({
  project,
  projectMode,
  locale,
  theme,
  cliMcpEnabled,
  gitModeEnabled,
  gitDetecting,
  gitDetectResult,
  aiAssistantMode,
  aiApiKey,
  aiApiEndpoint,
  aiProvider,
  aiModel,
  aiTesting,
  aiTestResult,
  track,
  aiOperator,
  aiPrompt,
  aiIndexPath,
  aiPromptLoading,
  onSetLocale,
  onSetTheme,
  onSetCliMcpEnabled,
  onSetGitModeEnabled,
  onGitDetect,
  onSetAiAssistantMode,
  onSetAiApiKey,
  onSetAiApiEndpoint,
  onSetAiProvider,
  onSetAiModel,
  onTestAiConnection,
  onSetProjectRouteA,
  onSetProjectMode,
  onTrackSetAll,
  onTrackSetVerifySha256,
  onTrackSetMemoryBufferMs,
  onTrackSetSuffixes,
  onTrackSetPrefixes,
  onTrackSetOn,
  onClaimAiControl,
  onReleaseAiControl,
  onOpenAiConflict,
  onAiIndexRefresh,
  onCopyAiIndex,
  onResetSettings,
  onClearAllData,
  tr,
}: {
  project: Project;
  projectMode: ProjectMode;
  locale: Locale;
  theme: "dark" | "light";
  cliMcpEnabled: boolean;
  gitModeEnabled: boolean;
  gitDetecting: boolean;
  gitDetectResult: GitDetectResult | null;
  aiAssistantMode: AiAssistantMode;
  aiApiKey: string;
  aiApiEndpoint: string;
  aiProvider: AiProvider;
  aiModel: string;
  aiTesting: boolean;
  aiTestResult: { ok: boolean; msg: string } | null;
  track: TrackConfigDto | null;
  aiOperator: AiOperatorDto | null;
  aiPrompt: string;
  aiIndexPath: string | null;
  aiPromptLoading: boolean;
  onSetLocale: (l: Locale) => void;
  onSetTheme: (t: "dark" | "light") => void;
  onSetCliMcpEnabled: (on: boolean) => void;
  onSetGitModeEnabled: (on: boolean) => void;
  onGitDetect: () => void;
  onSetAiAssistantMode: (m: AiAssistantMode) => void;
  onSetAiApiKey: (k: string) => void;
  onSetAiApiEndpoint: (e: string) => void;
  onSetAiProvider: (p: AiProvider) => void;
  onSetAiModel: (m: string) => void;
  onTestAiConnection: () => void;
  onSetProjectRouteA: (on: boolean) => void;
  onSetProjectMode: (m: ProjectMode) => void;
  onTrackSetAll: (on: boolean) => void;
  onTrackSetVerifySha256: (on: boolean) => void;
  onTrackSetMemoryBufferMs: (ms: number) => void;
  onTrackSetSuffixes: (s: string[]) => void;
  onTrackSetPrefixes: (p: string[]) => void;
  onTrackSetOn: (on: { windows: boolean; macos: boolean; linux: boolean }) => void;
  onClaimAiControl: () => void;
  onReleaseAiControl: () => void;
  onOpenAiConflict: () => void;
  onAiIndexRefresh: () => void;
  onCopyAiIndex: () => void;
  onResetSettings: () => void;
  onClearAllData: () => void;
  tr: Dict;
}) {
  // #region debug-point settings-handlers
  // Wrap every settings handler so we can see which interaction immediately
  // precedes a black-screen crash. These are thin pass-throughs; business
  // logic is untouched.
  const wrap = <T extends unknown[]>(name: string, fn: (...args: T) => void) => {
    return (...args: T) => {
      dbg(`settings:${name}:before`, { args: args as unknown });
      try {
        fn(...args);
        dbg(`settings:${name}:after`, { args: args as unknown });
      } catch (e) {
        dbg(`settings:${name}:error`, { error: String(e) });
        throw e;
      }
    };
  };
  const setLocale = wrap("locale", onSetLocale);
  const setTheme = wrap("theme", onSetTheme);
  const setProjectRouteA = wrap("routeA", (on: boolean) => {
    dbg("settings:bridge-check", { available: isTauriBridgeAvailable() });
    onSetProjectRouteA(on);
  });
  const setProjectMode = wrap("projectMode", onSetProjectMode);
  const setCliMcpEnabled = wrap("cliMcp", onSetCliMcpEnabled);
  const setGitModeEnabled = wrap("gitMode", onSetGitModeEnabled);
  const gitDetect = wrap("gitDetect", onGitDetect);
  const setAiAssistantMode = wrap("aiAssistantMode", onSetAiAssistantMode);
  const setAiProvider = wrap("aiProvider", onSetAiProvider);
  const setAiApiKey = wrap("aiApiKey", onSetAiApiKey);
  const setAiApiEndpoint = wrap("aiApiEndpoint", onSetAiApiEndpoint);
  const setAiModel = wrap("aiModel", onSetAiModel);
  const testAiConnection = wrap("testAi", onTestAiConnection);
  const claimAiControl = wrap("claimAi", onClaimAiControl);
  const releaseAiControl = wrap("releaseAi", onReleaseAiControl);
  const openAiConflict = wrap("openAiConflict", onOpenAiConflict);
  const aiIndexRefresh = wrap("aiIndexRefresh", onAiIndexRefresh);
  const copyAiIndex = wrap("copyAiIndex", onCopyAiIndex);
  const resetSettings = wrap("resetSettings", onResetSettings);
  const clearAllData = wrap("clearAllData", onClearAllData);
  // #endregion debug-point settings-handlers

  // Export the timeline (commit history) as a JSON file download. Uses the
  // backend `export_data` command with the JSON exporter, then triggers a
  // browser-side Blob download so no extra filesystem permission is needed.
  const [exportError, setExportError] = useState<string | null>(null);
  const handleExportTimeline = async () => {
    setExportError(null);
    try {
      const json = await exportData("json");
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `route-timeline-${shortName(project.path) || "project"}.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (e) {
      setExportError(localizeBridgeError(e));
    }
  };

  // Copy the project's local data folder path (.route) to the clipboard.
  const dataFolderPath = project.path ? `${project.path}/.route` : "";
  const [pathCopied, setPathCopied] = useState(false);
  const handleCopyPath = async () => {
    if (!dataFolderPath) return;
    try {
      await navigator.clipboard.writeText(dataFolderPath);
      setPathCopied(true);
      setTimeout(() => setPathCopied(false), 1400);
    } catch {
      // ignore — clipboard may be unavailable
    }
  };

  // MCP config — fetched from the backend so the user gets a ready-to-
  // paste JSON snippet with the correct binary path and --project arg.
  // Re-fetched whenever the toggle flips on or the project changes.
  const [mcpConfig, setMcpConfig] = useState<McpConfigDto | null>(null);
  const [mcpCopied, setMcpCopied] = useState(false);

  useEffect(() => {
    if (!cliMcpEnabled) {
      setMcpConfig(null);
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const cfg = await mcpGetConfig();
        if (!cancelled) setMcpConfig(cfg);
      } catch {
        // Bridge error — leave config null, the card just won't show
        // the snippet. The toggle still works as a UI preference.
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [cliMcpEnabled, project.path]);

  const handleCopyMcpConfig = async () => {
    if (!mcpConfig) return;
    try {
      await navigator.clipboard.writeText(mcpConfig.config_snippet);
      setMcpCopied(true);
      setTimeout(() => setMcpCopied(false), 1400);
    } catch {
      // ignore — clipboard may be unavailable
    }
  };

  return (
    <div className="card settings-page">
      <h2>{tr.settings}</h2>

      {/* ══ 基本 / Basic ════════════════════════════════════════════════
          Foundational, always-relevant settings: the project mode and
          the OS-level launch/close behavior. Autostart used to live at
          the bottom under "System" — pulled up here so it is not buried. */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.basicSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* Mode — standard is the only usable mode right now; AI
              Collaboration is disabled and marked "in development". */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.modeSection}</h4>
            <p className="settings-section-hint">{tr.modeSectionHint}</p>
            <div className="seg-control">
              <button
                type="button"
                className={projectMode === "standard" ? "active" : ""}
                onClick={() => setProjectMode("standard")}
              >
                {tr.modeStandard}
              </button>
              <button
                type="button"
                className="mode-disabled"
                disabled
                title={tr.modeAiDev}
              >
                {tr.modeAi}
                <span className="mode-dev-pill">{tr.modeAiDev}</span>
              </button>
            </div>
            <p className="settings-section-aside">
              {projectMode === "standard" ? tr.modeStandardHint : tr.modeAiHint}
            </p>
          </div>

          {/* Autostart (enable + silent + priority) + close behavior.
              AutostartSection renders TWO sub-cards. */}
          <AutostartSection tr={tr} />
        </div>
      </section>

      {/* ══ 外观 / Appearance ═══════════════════════════════════════════
          Visual preferences only. Theme and language are split into
          one sub-card each — single function, single card. */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.appearanceSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* Theme — a single light/dark switch. On = light, off = dark.
              No slider, no multi-stop adjustment; a binary toggle. */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.themeSection}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.themeSection}</span>
                <span className="settings-row-hint">{tr.themeSectionHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={theme === "light"}
                  onChange={(checked) => setTheme(checked ? "light" : "dark")}
                  statusText={
                    <span className={theme === "light" ? "on" : ""}>
                      {theme === "light" ? tr.themeLight : tr.themeDark}
                    </span>
                  }
                />
              </div>
            </div>
          </div>

          {/* Language */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.languageLabel}</h4>
            <p className="settings-section-hint">{tr.languageHint}</p>
            <div className="seg-control">
              <button
                type="button"
                className={locale === "zh" ? "active" : ""}
                onClick={() => setLocale("zh")}
              >
                中文
              </button>
              <button
                type="button"
                className={locale === "en" ? "active" : ""}
                onClick={() => setLocale("en")}
              >
                English
              </button>
            </div>
          </div>
        </div>
      </section>

      {/* ══ 文件追踪 / Tracking ═════════════════════════════════════════
          What gets tracked: route 标记 mode + granular track config. */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.trackSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* route 标记 mode + cheatsheet + privacy */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.routeaLabel}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.routeaLabel}</span>
                <span className="settings-row-hint">{tr.routeaSettingHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={project.routeA}
                  onChange={setProjectRouteA}
                  statusText={
                    <span className={project.routeA ? "on" : ""}>
                      {project.routeA ? tr.routeaOn : tr.routeaOff}
                    </span>
                  }
                />
              </div>
            </div>

            <div className="settings-cheatsheet">
              <h4>{tr.routeaCheatsheetTitle}</h4>
              <table className="cheat-table">
                <thead>
                  <tr>
                    <th>{tr.routeaColMarker}</th>
                    <th>{tr.routeaColMeaning}</th>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                    <td><code>{tr.routeaRow1Marker}</code></td>
                    <td>{tr.routeaRow1Meaning}</td>
                  </tr>
                  <tr>
                    <td><code>{tr.routeaRow2Marker}</code></td>
                    <td>{tr.routeaRow2Meaning}</td>
                  </tr>
                  <tr>
                    <td><code>{tr.routeaRow3Marker}</code></td>
                    <td>{tr.routeaRow3Meaning}</td>
                  </tr>
                </tbody>
              </table>
              <p className="cheat-note">{tr.routeaCommentNote}</p>
            </div>

            <div className="settings-privacy">
              <h4>{tr.privacyTitle}</h4>
              <p>{tr.privacyP1}</p>
              <p>{tr.privacyP2}</p>
              <p>{tr.privacyP3}</p>
            </div>
          </div>

          {/* Granular track configuration — one sub-card */}
          {track && (
            <TrackConfigPanel
              track={track}
              onSetAll={onTrackSetAll}
              onSetSuffixes={onTrackSetSuffixes}
              onSetPrefixes={onTrackSetPrefixes}
              onSetVerifySha256={onTrackSetVerifySha256}
              onSetMemoryBufferMs={onTrackSetMemoryBufferMs}
              onSetOn={onTrackSetOn}
              tr={tr}
            />
          )}
        </div>
      </section>

      {/* ══ AI ═════════════════════════════════════════════════════════
          AI assistance, control handoff, and the index file external
          agents read. Grouped so all AI concerns live in one place. */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.aiSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* AI index file — moved from Data so all AI surfaces are
              co-located. */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.aiIndexSection}</h4>
            <p className="settings-section-hint">{tr.aiIndexSectionHint}</p>
            <div className="ai-index-row">
              <div className="ai-index-label">
                <span>{tr.aiIndexPathLabel}</span>
                <span className="settings-row-hint">{tr.aiIndexPathHint}</span>
              </div>
              <div className="ai-index-control">
                <span className="ai-index-path" title={aiIndexPath || ""}>
                  {aiIndexPath || "—"}
                </span>
                <button type="button" className="ghost-btn" onClick={copyAiIndex}>
                  {tr.aiIndexCopy}
                </button>
                <button type="button" className="ghost-btn" onClick={aiIndexRefresh}>
                  {tr.aiIndexRefresh}
                </button>
              </div>
            </div>
          </div>

          {/* AI 辅助模式 — two sub-modes: follow the existing Web Coding AI,
              or connect a separate API for a dedicated AI to operate Route. */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.aiAssistantSection}</h4>
            <p className="settings-section-hint">{tr.aiAssistantHint}</p>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.aiAssistantSection}</span>
                <span className="settings-row-hint">{tr.aiAssistantHint}</span>
              </div>
              <div className="settings-row-control">
                <div className="seg-control">
                  <button
                    type="button"
                    className={aiAssistantMode === "follow" ? "active" : ""}
                    onClick={() => setAiAssistantMode("follow")}
                  >
                    {tr.aiFollowExisting}
                  </button>
                  <button
                    type="button"
                    className={aiAssistantMode === "active" ? "active" : ""}
                    onClick={() => setAiAssistantMode("active")}
                  >
                    {tr.aiActiveMode}
                    <span className="beta-badge">{tr.betaBadge}</span>
                  </button>
                </div>
              </div>
            </div>
            <p className="settings-section-aside">
              {aiAssistantMode === "follow" ? tr.aiFollowExistingHint : tr.aiActiveModeHint}
            </p>
            <div className={`ai-api-config${aiAssistantMode === "active" ? " active" : ""}`}>
                {/* Provider — OpenAI (covers OpenAI-compatible servers too),
                    Anthropic, or local Ollama. Switching auto-fills the
                    endpoint default so the user rarely edits the URL. */}
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span>{tr.aiProviderLabel}</span>
                    <span className="settings-row-hint">
                      {aiProvider === "openai"
                        ? tr.aiProviderOpenaiHint
                        : aiProvider === "anthropic"
                          ? tr.aiProviderAnthropicHint
                          : tr.aiProviderOllamaHint}
                    </span>
                  </div>
                  <div className="settings-row-control">
                    <div className="seg-control">
                      <button
                        type="button"
                        className={aiProvider === "openai" ? "active" : ""}
                        onClick={() => setAiProvider("openai")}
                      >
                        {tr.aiProviderOpenai}
                      </button>
                      <button
                        type="button"
                        className={aiProvider === "anthropic" ? "active" : ""}
                        onClick={() => setAiProvider("anthropic")}
                      >
                        {tr.aiProviderAnthropic}
                      </button>
                      <button
                        type="button"
                        className={aiProvider === "ollama" ? "active" : ""}
                        onClick={() => setAiProvider("ollama")}
                      >
                        {tr.aiProviderOllama}
                      </button>
                    </div>
                  </div>
                </div>
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span>{tr.aiApiEndpointLabel}</span>
                  </div>
                  <div className="settings-row-control settings-row-control-input">
                    <input
                      className="settings-input"
                      type="text"
                      value={aiApiEndpoint}
                      onChange={(e) => setAiApiEndpoint(e.target.value)}
                      placeholder={tr.aiApiEndpointPlaceholder}
                    />
                  </div>
                </div>
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span>{tr.aiApiKeyLabel}</span>
                    <span className="settings-row-hint">
                      {aiProvider === "ollama" ? tr.aiProviderOllamaHint : tr.aiApiKeyHint}
                    </span>
                  </div>
                  <div className="settings-row-control settings-row-control-input">
                    <input
                      className="settings-input"
                      type="password"
                      value={aiApiKey}
                      onChange={(e) => setAiApiKey(e.target.value)}
                      placeholder={tr.aiApiKeyPlaceholder}
                    />
                  </div>
                </div>
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span>{tr.aiModelLabel}</span>
                  </div>
                  <div className="settings-row-control settings-row-control-input">
                    <input
                      className="settings-input"
                      type="text"
                      value={aiModel}
                      onChange={(e) => setAiModel(e.target.value)}
                      placeholder={tr.aiModelPlaceholder}
                    />
                  </div>
                </div>
                <div className="ai-test-row">
                  <button
                    type="button"
                    className="ghost-btn"
                    onClick={testAiConnection}
                    disabled={aiTesting}
                  >
                    {aiTesting ? tr.aiTesting : tr.aiTestConnection}
                  </button>
                  {aiTestResult && (
                    <p
                      className={`settings-section-aside ${aiTestResult.ok ? "ok" : "err"}`}
                    >
                      {aiTestResult.msg}
                    </p>
                  )}
                </div>
              </div>
          </div>

          {/* AI control — live operator handoff */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.aiControlLabel}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.aiControlLabel}</span>
                <span className="settings-row-hint">{tr.aiPromptHint}</span>
              </div>
              <div className="settings-row-control">
                {aiOperator ? (
                  <div style={{ display: "inline-flex", gap: 6, flexWrap: "wrap" }}>
                    <span className="ai-pill">
                      <span className="ai-pill-mark" />
                      {tr.aiControlActive}
                    </span>
                    <button type="button" className="ghost-btn" onClick={openAiConflict}>
                      {tr.aiConflictTitle}
                    </button>
                    <button type="button" className="ghost-btn" onClick={releaseAiControl}>
                      {tr.masterOff}
                    </button>
                  </div>
                ) : (
                  <button type="button" className="ghost-btn" onClick={claimAiControl}>
                    {tr.aiControlActive}
                  </button>
                )}
              </div>
            </div>
            <pre className="ai-prompt-pre">
              {aiPromptLoading ? "…" : aiPrompt || "(empty)"}
            </pre>
          </div>
        </div>
      </section>

      {/* ══ 版本控制 / Version Control ══════════════════════════════════
          Git mode — a power-user opt-in that changes how version history
          is recorded. Push and remote operations are intentionally blocked. */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.versionControlSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* Git 模式 (Beta) — software calls the user's git for add+commit.
              Disabled by default; checkpoints become git commits. Push and
              remote operations are intentionally blocked — the user does
              those themselves. */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">
              {tr.gitModeLabel}
              <span className="beta-badge">{tr.betaBadge}</span>
            </h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.gitModeLabel}</span>
                <span className="settings-row-hint">{tr.gitModeHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={gitModeEnabled}
                  onChange={(checked) => setGitModeEnabled(checked)}
                  statusText={
                    <span className={gitModeEnabled ? "on" : ""}>
                      {gitModeEnabled ? tr.gitModeOn : tr.gitModeOff}
                    </span>
                  }
                />
              </div>
            </div>
            <p className="settings-section-aside">{tr.gitModeAside}</p>
            <div className={`git-mode-status${gitModeEnabled ? " active" : ""}`}>
                <div className="settings-row">
                  <div className="settings-row-label">
                    <span>{tr.gitModeDetectLabel}</span>
                  </div>
                  <div className="settings-row-control">
                    <button
                      type="button"
                      className="ghost-btn"
                      onClick={gitDetect}
                      disabled={gitDetecting}
                    >
                      {gitDetecting ? "…" : tr.gitModeDetect}
                    </button>
                  </div>
                </div>
                {gitDetectResult && (
                  <p className={`settings-section-aside${gitDetectResult.available ? " ok" : " err"}`}>
                    {gitDetectResult.available
                      ? tr.gitModeAvailable.replace("{ver}", gitDetectResult.version || "git")
                      : tr.gitModeUnavailable}
                  </p>
                )}
                <p className="settings-section-hint">{tr.gitModeNoPushHint}</p>
              </div>
          </div>
        </div>
      </section>

      {/* ══ 外部接入 / External integrations ═══════════════════════════════
          External automation surfaces — CLI/MCP interface. Split into its
          own main card so the Advanced card stays focused on tracking/AI
          concerns and each integration reads as an independent function. */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.integrationsSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* CLI / MCP interface */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.cliMcpEnable}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.cliMcpEnable}</span>
                <span className="settings-row-hint">{tr.cliMcpEnableHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={cliMcpEnabled}
                  onChange={(checked) => setCliMcpEnabled(checked)}
                  statusText={
                    <span className={cliMcpEnabled ? "on" : ""}>
                      {cliMcpEnabled ? tr.cliMcpOn : tr.cliMcpOff}
                    </span>
                  }
                />
              </div>
            </div>
            <p className="settings-section-aside">{tr.cliMcpStartHint}</p>

            {/* Config snippet — shown only when MCP is enabled and the
                backend has returned the config. Gives the user a
                ready-to-paste JSON block for their AI client. */}
            <div className={`mcp-config-block${cliMcpEnabled && mcpConfig ? " active" : ""}`}>
              {cliMcpEnabled && mcpConfig && (
                <>
                <div className="mcp-config-header">
                  <span className="mcp-config-label">{tr.cliMcpConfigSnippet}</span>
                  <button
                    type="button"
                    className="mcp-copy-btn"
                    onClick={handleCopyMcpConfig}
                  >
                    {mcpCopied ? tr.cliMcpConfigCopied : tr.cliMcpCopyConfig}
                  </button>
                </div>
                {!mcpConfig.binary_exists && (
                  <p className="mcp-config-warn">{tr.cliMcpBuildHint}</p>
                )}
                {!mcpConfig.project_path && (
                  <p className="mcp-config-warn">{tr.cliMcpNoProject}</p>
                )}
                <pre className="mcp-config-snippet">
                  <code>{mcpConfig.config_snippet}</code>
                </pre>
                {mcpConfig.binary_exists && (
                  <div className="mcp-config-path">
                    <span className="mcp-config-path-label">{tr.cliMcpBinaryPath}:</span>
                    <span className="mono mcp-config-path-value">{mcpConfig.binary_path}</span>
                  </div>
                )}
                </>
              )}
            </div>
          </div>
        </div>
      </section>

      {/* ══ 数据 / Data ═════════════════════════════════════════════════
          Data-facing surfaces: the AI index file that external agents
          read, and the destructive reset/clear actions. */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.dataSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* Timeline data — export as JSON + show local data folder. The
              timeline records themselves live in <project>/.route, so the
              folder path is the canonical data location for backup/restore. */}
          <div className="settings-subcard">
            <h4 className="settings-subcard-title">{tr.dataImportExport}</h4>
            <p className="settings-section-hint">{tr.dataImportExportHint}</p>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.dataExportLabel}</span>
                <span className="settings-row-hint">{tr.dataExportHint}</span>
              </div>
              <div className="settings-row-control">
                <button
                  type="button"
                  className="ghost-btn"
                  onClick={handleExportTimeline}
                >
                  {tr.dataExportLabel}
                </button>
                {exportError && (
                  <span className="workspace-feedback err">{exportError}</span>
                )}
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.dataLocationLabel}</span>
                <span className="settings-row-hint">{tr.dataLocationHint}</span>
              </div>
              <div className="settings-row-control data-location-control">
                <span className="mono data-location-path" title={dataFolderPath}>
                  {dataFolderPath || "—"}
                </span>
                <button type="button" className="ghost-btn" onClick={handleCopyPath}>
                  {pathCopied ? tr.pathCopied : tr.copyPath}
                </button>
              </div>
            </div>
          </div>

          {/* Danger zone */}
          <DangerZone
            onResetSettings={onResetSettings}
            onClearAllData={onClearAllData}
            tr={tr}
          />
        </div>
      </section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Timeline node icons — one per commit kind. Rendered inside .timeline-node
// so the glyph itself communicates the commit type at a glance: a marker
// flag for checkpoints, a sparkle for AI-driven commits, a plain commit
// dot for regular user commits. Stroke-only, inherit currentColor.
// ---------------------------------------------------------------------------

function TimelineNodeCommit() {
  // Solid filled dot — a regular user commit.
  return (
    <svg viewBox="0 0 14 14" fill="currentColor" stroke="none">
      <circle cx="7" cy="7" r="2.6" />
    </svg>
  );
}

function TimelineNodeCheckpoint() {
  // Marker flag — a user-marked checkpoint.
  return (
    <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3.5 2.5v9" />
      <path d="M3.5 3l6.5 1.8-6.5 1.8" />
    </svg>
  );
}

function TimelineNodeAi() {
  // Four-point sparkle — an AI-driven commit.
  return (
    <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round">
      <path d="M7 2v4M7 8v4M2 7h4M8 7h4" />
      <path d="M3.8 3.8l2 2M8.2 8.2l2 2M10.2 3.8l-2 2M5.8 8.2l-2 2" opacity="0.55" />
    </svg>
  );
}

// ---------------------------------------------------------------------------
// BranchTree — vertical stack of branch lanes that absorbs the old timeline.
//
// Layout: branches are rendered top-to-bottom, newest first, with `main`
// pinned to the bottom — so the reading direction is "from the leaves down
// to the trunk", matching the user's mental model of a git tree. Each lane
// is a horizontal row with a header (name + kind badge + actions). The
// current branch's lane expands to show its commit timeline inline; other
// lanes stay collapsed so the user can scan the branch structure at a
// glance and click "切换" to focus a different lane.
//
// Branch kinds (route_basic, non-Git mode):
//   - main       : the trunk. Cannot be deleted. Lanes fork from here.
//   - inherited  : a normal branch. 3-way mergeable into any non-sandbox.
//   - sandbox    : a scratch space. Cannot be a merge target, but its
//                  contents can be copied into a new inherited branch so
//                  the user's experiments are never lost.
//
// In Git mode the kind distinction disappears (git branches are all the
// same); the lane actions reduce to switch + merge, and the new-branch
// form drops the kind selector.
// ---------------------------------------------------------------------------

function BranchTree({
  branches,
  currentBranch,
  commits,
  gitModeEnabled,
  onSwitch,
  onCreate,
  onMerge,
  onRollback,
  tr,
}: {
  branches: BranchDto[];
  currentBranch: string;
  commits: CommitDto[];
  gitModeEnabled: boolean;
  onSwitch: (name: string) => void;
  onCreate: (name: string, kind: "inherited" | "sandbox", from: string | null) => void;
  onMerge: (source: string) => void;
  onRollback: (snapshotId: string) => void;
  tr: Dict;
}) {
  const [showForm, setShowForm] = useState(false);
  const [formName, setFormName] = useState("");
  const [formKind, setFormKind] = useState<"inherited" | "sandbox">("inherited");
  // When set, the new-branch form was opened via "复制到新分支" — the
  // source branch is pre-filled as the parent so the sandbox's content
  // is carried into a safe, mergeable inherited branch.
  const [formFrom, setFormFrom] = useState<string | null>(null);

  // Sort: main pinned to the bottom, the rest newest-first on top. This
  // gives the "from bottom up" tree reading the user asked for without
  // needing a real graph layout.
  const sorted = useMemo(() => {
    const list = [...branches];
    list.sort((a, b) => {
      if (a.kind === "main" && b.kind !== "main") return 1;
      if (b.kind === "main" && a.kind !== "main") return -1;
      return b.created_at - a.created_at;
    });
    return list;
  }, [branches]);

  // Current branch's commits, newest first — reuses the exact timeline
  // item rendering the old TimelinePage used, so the visual language stays
  // consistent and the diff-details / rollback controls keep working.
  const currentCommits = useMemo(
    () => [...commits].sort((a, b) => b.created_at - a.created_at),
    [commits],
  );

  const handleSubmit = () => {
    const n = formName.trim();
    if (!n) return;
    onCreate(n, formKind, formFrom);
    setFormName("");
    setFormKind("inherited");
    setFormFrom(null);
    setShowForm(false);
  };

  const openCopyForm = (source: string) => {
    setFormFrom(source);
    setFormKind("inherited");
    setFormName(`${source}-copy`);
    setShowForm(true);
  };

  const kindLabel = (k: string) =>
    k === "main"
      ? tr.branchKindMain
      : k === "sandbox"
        ? tr.branchKindSandbox
        : tr.branchKindInherited;

  return (
    <section className="branch-tree">
      <header className="branch-tree-header">
        <h3>{tr.branchTreeTitle}</h3>
        <button
          type="button"
          className="branch-new-btn"
          onClick={() => {
            // Reset form defaults when opening via the top button — a
            // fresh inherited branch off the current branch.
            setFormFrom(currentBranch || null);
            setFormKind("inherited");
            setShowForm((v) => !v);
          }}
        >
          + {tr.newBranch}
        </button>
      </header>

      {showForm && (
        <div className="branch-form">
          <input
            className="settings-input branch-form-name"
            value={formName}
            onChange={(e) => setFormName(e.target.value)}
            placeholder={tr.branchName}
          />
          {!gitModeEnabled && (
            <>
              <select
                className="settings-input branch-form-kind"
                value={formKind}
                onChange={(e) => setFormKind(e.target.value as "inherited" | "sandbox")}
              >
                <option value="inherited">{tr.branchKindInherited}</option>
                <option value="sandbox">{tr.branchKindSandbox}</option>
              </select>
              {formFrom && (
                <span className="branch-form-from">
                  ← {formFrom}
                </span>
              )}
            </>
          )}
          <button
            type="button"
            className="primary"
            onClick={handleSubmit}
            disabled={!formName.trim()}
          >
            {tr.createBranch}
          </button>
          <button
            type="button"
            className="branch-form-cancel"
            onClick={() => setShowForm(false)}
          >
            {tr.cancelBranch}
          </button>
        </div>
      )}

      {sorted.length === 0 ? (
        <div className="empty-projects">{tr.branchEmpty}</div>
      ) : (
        <ul className="branch-lanes">
          {sorted.map((b) => {
            const isCurrent = b.name === currentBranch;
            const isSandbox = b.kind === "sandbox";
            return (
              <li
                key={b.id || b.name}
                className={`branch-lane${isCurrent ? " current" : ""}${isSandbox ? " sandbox" : ""}`}
              >
                <div className="branch-lane-header">
                  <div className="branch-lane-title">
                    <span
                      className="branch-lane-name"
                      onClick={() => !isCurrent && onSwitch(b.name)}
                      title={isCurrent ? tr.currentBranchLabel : tr.switchBranch}
                    >
                      {b.name}
                    </span>
                    <span className={`branch-kind-badge ${b.kind}`}>{kindLabel(b.kind)}</span>
                  </div>
                  <div className="branch-lane-actions">
                    {isCurrent ? (
                      <span className="branch-current-tag">{tr.currentBranchLabel}</span>
                    ) : (
                      <>
                        <button
                          type="button"
                          className="branch-action"
                          onClick={() => onSwitch(b.name)}
                        >
                          {tr.switchBranch}
                        </button>
                        {isSandbox ? (
                          <button
                            type="button"
                            className="branch-action"
                            onClick={() => openCopyForm(b.name)}
                          >
                            {tr.copyToNewBranch}
                          </button>
                        ) : (
                          <button
                            type="button"
                            className="branch-action"
                            onClick={() => onMerge(b.name)}
                          >
                            {tr.mergeBranch}
                          </button>
                        )}
                      </>
                    )}
                  </div>
                </div>

                {isCurrent && currentCommits.length > 0 && (
                  <ul className="timeline branch-timeline">
                    {currentCommits.map((c) => {
                      const diff = c.diff_summary;
                      const fileCount =
                        (diff?.added?.length || 0) +
                        (diff?.modified?.length || 0) +
                        (diff?.removed?.length || 0);
                      return (
                        <li
                          key={c.id}
                          className={`timeline-item${c.is_ai ? " op-ai" : ""}${c.is_checkpoint ? " op-checkpoint" : ""}`}
                        >
                          <div className="timeline-node" aria-hidden="true">
                            {c.is_checkpoint ? (
                              <TimelineNodeCheckpoint />
                            ) : c.is_ai ? (
                              <TimelineNodeAi />
                            ) : (
                              <TimelineNodeCommit />
                            )}
                          </div>
                          <div className="timeline-body">
                            <div className="timeline-row1">
                              <div className="timeline-msg">{c.message || tr.noMessage}</div>
                              <div className="timeline-time">{formatTs(c.created_at)}</div>
                            </div>
                            {fileCount > 0 && (
                              <details className="timeline-diff">
                                <summary>
                                  {tr.diffShowFiles}
                                  <span className="timeline-diff-count">
                                    {diff!.added.length > 0 && (
                                      <span className="diff-tag add">+{diff!.added.length}</span>
                                    )}
                                    {diff!.modified.length > 0 && (
                                      <span className="diff-tag mod">~{diff!.modified.length}</span>
                                    )}
                                    {diff!.removed.length > 0 && (
                                      <span className="diff-tag del">-{diff!.removed.length}</span>
                                    )}
                                  </span>
                                </summary>
                                <div className="timeline-diff-body">
                                  {diff!.added.length > 0 && (
                                    <div className="diff-group">
                                      <span className="diff-group-label add">{tr.filesAdded}</span>
                                      <ul className="diff-file-list">
                                        {diff!.added.map((f) => (
                                          <li key={f} className="diff-file add">{f}</li>
                                        ))}
                                      </ul>
                                    </div>
                                  )}
                                  {diff!.modified.length > 0 && (
                                    <div className="diff-group">
                                      <span className="diff-group-label mod">{tr.filesModified}</span>
                                      <ul className="diff-file-list">
                                        {diff!.modified.map((f) => (
                                          <li key={f} className="diff-file mod">{f}</li>
                                        ))}
                                      </ul>
                                    </div>
                                  )}
                                  {diff!.removed.length > 0 && (
                                    <div className="diff-group">
                                      <span className="diff-group-label del">{tr.filesRemoved}</span>
                                      <ul className="diff-file-list">
                                        {diff!.removed.map((f) => (
                                          <li key={f} className="diff-file del">{f}</li>
                                        ))}
                                      </ul>
                                    </div>
                                  )}
                                </div>
                              </details>
                            )}
                            {c.to_snapshot && !c.is_checkpoint && (
                              <div className="timeline-row2">
                                <div />
                                <button
                                  type="button"
                                  className="timeline-rollback"
                                  onClick={() => onRollback(c.to_snapshot)}
                                >
                                  {tr.rollbackOne}
                                </button>
                              </div>
                            )}
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                )}
                {isCurrent && currentCommits.length === 0 && (
                  <div className="branch-lane-empty">{tr.timelineEmpty}</div>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// TimelinePage — list of commits, search, sort, filter
// ---------------------------------------------------------------------------

function TimelinePage({
  commits,
  loading,
  error,
  onRollback,
  tr,
}: {
  commits: CommitDto[];
  loading: boolean;
  error: string | null;
  onRollback: (snapshotId: string) => void;
  tr: Dict;
}) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<"all" | "user" | "ai" | "marker">("all");

  const filtered = useMemo(() => {
    let list = commits;
    if (filter === "user") list = list.filter((c) => !c.is_ai);
    else if (filter === "ai") list = list.filter((c) => c.is_ai);
    else if (filter === "marker") list = list.filter((c) => c.is_checkpoint);
    if (query.trim()) {
      const q = query.toLowerCase();
      list = list.filter(
        (c) =>
          (c.message || "").toLowerCase().includes(q) ||
          (c.short_id || "").toLowerCase().includes(q),
      );
    }
    // Default order: newest on top, oldest at the bottom. No sort toggle —
    // the timeline is a single chronological feed, newest-first is the
    // only sensible reading order for a history view.
    list = [...list].sort((a, b) => b.created_at - a.created_at);
    return list;
  }, [commits, filter, query]);

  return (
    <div className="history-page">
      <div className="timeline-top">
        <h2>{tr.timeline}</h2>
        <div className="timeline-controls">
          <input
            className="settings-input history-search"
            placeholder={tr.searchPlaceholder}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <select
            className="settings-input timeline-filter"
            value={filter}
            onChange={(e) => setFilter(e.target.value as "all" | "user" | "ai" | "marker")}
          >
            <option value="all">{tr.filterAll}</option>
            <option value="user">{tr.filterUser}</option>
            <option value="ai">{tr.filterAi}</option>
            <option value="marker">{tr.filterMarker}</option>
          </select>
        </div>
      </div>

        {error && <div className="error-banner">{error}</div>}

        {filtered.length === 0 ? (
          <div className="empty-projects">{tr.timelineEmpty}</div>
        ) : (
          <ul className="timeline">
              {filtered.map((c) => {
                const diff = c.diff_summary;
                const fileCount =
                  (diff?.added?.length || 0) +
                  (diff?.modified?.length || 0) +
                  (diff?.removed?.length || 0);
                return (
                <li
                  key={c.id}
                  className={`timeline-item${c.is_ai ? " op-ai" : ""}${c.is_checkpoint ? " op-checkpoint" : ""}`}
                >
                  <div className="timeline-node" aria-hidden="true">
                    {c.is_checkpoint ? <TimelineNodeCheckpoint /> : c.is_ai ? <TimelineNodeAi /> : <TimelineNodeCommit />}
                  </div>
                  <div className="timeline-body">
                    <div className="timeline-row1">
                      <div className="timeline-msg">{c.message || tr.noMessage}</div>
                      <div className="timeline-time">{formatTs(c.created_at)}</div>
                    </div>

                    {/* File-level change summary — shows which files were
                        added / modified / removed in this change. The
                        commit-type is already conveyed by the left icon
                        node, so no branch / author / id metadata is
                        shown: there is a single main line and no
                        "commit" concept to surface. */}
                    {fileCount > 0 && (
                      <details className="timeline-diff">
                        <summary>
                          {tr.diffShowFiles}
                          <span className="timeline-diff-count">
                            {diff!.added.length > 0 && (
                              <span className="diff-tag add">+{diff!.added.length}</span>
                            )}
                            {diff!.modified.length > 0 && (
                              <span className="diff-tag mod">~{diff!.modified.length}</span>
                            )}
                            {diff!.removed.length > 0 && (
                              <span className="diff-tag del">-{diff!.removed.length}</span>
                            )}
                          </span>
                        </summary>
                        <div className="timeline-diff-body">
                          {diff!.added.length > 0 && (
                            <div className="diff-group">
                              <span className="diff-group-label add">{tr.filesAdded}</span>
                              <ul className="diff-file-list">
                                {diff!.added.map((f) => (
                                  <li key={f} className="diff-file add">{f}</li>
                                ))}
                              </ul>
                            </div>
                          )}
                          {diff!.modified.length > 0 && (
                            <div className="diff-group">
                              <span className="diff-group-label mod">{tr.filesModified}</span>
                              <ul className="diff-file-list">
                                {diff!.modified.map((f) => (
                                  <li key={f} className="diff-file mod">{f}</li>
                                ))}
                              </ul>
                            </div>
                          )}
                          {diff!.removed.length > 0 && (
                            <div className="diff-group">
                              <span className="diff-group-label del">{tr.filesRemoved}</span>
                              <ul className="diff-file-list">
                                {diff!.removed.map((f) => (
                                  <li key={f} className="diff-file del">{f}</li>
                                ))}
                              </ul>
                            </div>
                          )}
                        </div>
                      </details>
                    )}

                    {c.to_snapshot && !c.is_checkpoint && (
                      <div className="timeline-row2">
                        <div />
                        <button
                          type="button"
                          className="timeline-rollback"
                          onClick={() => onRollback(c.to_snapshot)}
                        >
                          {tr.rollbackOne}
                        </button>
                      </div>
                    )}
                  </div>
                </li>
                );
              })}
            </ul>
          )}
      {loading && <div className="gate-banner">…</div>}
    </div>
  );
}

// ---------------------------------------------------------------------------
// WorkbenchPage — unified, card-free workspace. Backup configuration lives in
// a collapsible region at the bottom; once confirmed, the top region reveals
// the inline checkpoint (title + body) and the manual full-backup button.
// No stats, no modal. Source path is the project folder (read-only); target
// path is seeded from the last-used location so the second visit is one click.
// ---------------------------------------------------------------------------

function WorkbenchPage({
  project,
  initialMode,
  error,
  onSave,
  onMark,
  onBackup,
  backupStatus,
  branches,
  currentBranch,
  commits,
  gitModeEnabled,
  trackingState,
  onSwitchBranch,
  onCreateBranch,
  onMergeBranch,
  onRollback,
  aiActive,
  onAiGenerateMark,
  tr,
}: {
  project: Project;
  initialMode: ProjectMode;
  error: string | null;
  onSave: (patch: { mode: ProjectMode; backup?: Project["backup"] }) => void;
  onMark: (title: string, body: string | null) => void;
  onBackup: () => void;
  backupStatus: "idle" | "running" | "done" | "fail";
  branches: BranchDto[];
  currentBranch: string;
  commits: CommitDto[];
  gitModeEnabled: boolean;
  trackingState: TrackingState;
  onSwitchBranch: (name: string) => void;
  onCreateBranch: (name: string, kind: "inherited" | "sandbox", from: string | null) => void;
  onMergeBranch: (source: string) => void;
  onRollback: (snapshotId: string) => void;
  aiActive: boolean;
  onAiGenerateMark: () => Promise<string>;
  tr: Dict;
}) {
  const configured = !!(project.backup && project.backup.target);
  const [editing, setEditing] = useState<boolean>(!configured);

  // Config form — seeded from the existing project backup so a re-edit keeps
  // the last-used values ("second time defaults to last position").
  const [backupKind, setBackupKind] = useState<"local" | "cloud">(
    project.backup?.kind === "cloud" ? "cloud" : "local",
  );
  const [backupTarget, setBackupTarget] = useState<string>(project.backup?.target || "");
  const [backupMethod, setBackupMethod] = useState<"mirror" | "incremental" | "standard">(
    project.backup?.method || "standard",
  );
  const [mirrorDelay, setMirrorDelay] = useState<number>(project.backup?.mirrorDelay || 0);

  // Inline checkpoint editor (replaces the old modal).
  const [markTitle, setMarkTitle] = useState("");
  const [markBody, setMarkBody] = useState("");
  // AI-generate state — loading flag + inline error for the "AI 生成" button.
  const [aiGenerating, setAiGenerating] = useState(false);
  const [aiGenError, setAiGenError] = useState<string | null>(null);

  // Git stash — local shelf for uncommitted changes. The count is fetched on
  // mount and after every push/pop so the pop button only appears when
  // there's something to restore. Pop conflicts throw (git keeps the stash);
  // we surface git's output so the user can resolve manually.
  const [stashCount, setStashCount] = useState(0);
  const [stashLoading, setStashLoading] = useState(false);
  const [stashError, setStashError] = useState<string | null>(null);
  const [stashInfo, setStashInfo] = useState<string | null>(null);

  // Git tag — mark the current HEAD as a named version milestone (e.g.
  // v1.0). Tags are created at HEAD only; the list refreshes after every
  // create/delete. Reuses createBranch/cancelBranch for the form verbs
  // since they're generic ("创建"/"取消").
  const [tags, setTags] = useState<string[]>([]);
  const [tagLoading, setTagLoading] = useState(false);
  const [tagError, setTagError] = useState<string | null>(null);
  const [tagName, setTagName] = useState("");
  const [tagFormOpen, setTagFormOpen] = useState(false);

  // Uncommitted changes — a normalized {change, path} list fetched from
  // gitStatus (Git mode) or workingDirStatus (standard mode). Shown as
  // quiet diff-tag counts above the checkpoint editor so the user knows
  // what they're about to commit. Refreshed on mount, after every
  // checkpoint (commits prop changes), and after stash/restore operations.
  const [uncommittedChanges, setUncommittedChanges] = useState<
    { change: string; path: string }[]
  >([]);
  // changesTick — manual refresh trigger bumped after stash/restore ops
  // that change the working tree without touching the commits prop.
  const [changesTick, setChangesTick] = useState(0);

  // Git restore — discard uncommitted modifications to tracked files.
  // Uses a two-step inline confirmation (first click arms, second click
  // executes) because the operation is destructive and can't be undone.
  const [restoreConfirming, setRestoreConfirming] = useState(false);
  const [restoreLoading, setRestoreLoading] = useState(false);
  const [restoreError, setRestoreError] = useState<string | null>(null);
  const [restoreInfo, setRestoreInfo] = useState<string | null>(null);

  useEffect(() => {
    if (!gitModeEnabled) {
      setStashCount(0);
      setTags([]);
      return;
    }
    let cancelled = false;
    gitStashList()
      .then((list) => {
        if (!cancelled) setStashCount(list.length);
      })
      .catch(() => {
        if (!cancelled) setStashCount(0);
      });
    gitTagList()
      .then((list) => {
        if (!cancelled) setTags(list);
      })
      .catch(() => {
        if (!cancelled) setTags([]);
      });
    return () => {
      cancelled = true;
    };
  }, [gitModeEnabled, project.path]);

  // Refresh uncommitted changes whenever the project, git mode, commit
  // history, or a manual tick changes. A new checkpoint (commits.length
  // increases) means the working tree is clean until the user edits again;
  // stash/restore bump changesTick because they mutate the working tree
  // without touching commits. Best-effort: errors are swallowed so a
  // missing git repo or a transient IPC failure doesn't block the
  // workbench. Both backends are normalized into the same {change, path}
  // tuples so the UI is identical regardless of mode.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      const list: { change: string; path: string }[] = [];
      try {
        if (gitModeEnabled) {
          const s = await gitStatus();
          for (const p of s.added) list.push({ change: "added", path: p });
          for (const p of s.untracked) list.push({ change: "added", path: p });
          for (const p of s.modified) list.push({ change: "modified", path: p });
          for (const p of s.removed) list.push({ change: "removed", path: p });
        } else {
          const changes = await workingDirStatus();
          for (const c of changes) list.push({ change: c.change, path: c.path });
        }
      } catch {
        // best-effort — leave list empty
      }
      if (!cancelled) setUncommittedChanges(list);
    })();
    return () => {
      cancelled = true;
    };
  }, [gitModeEnabled, project.path, commits.length, changesTick]);

  const handlePick = async () => {
    try {
      const path = await pickFolder();
      if (path) setBackupTarget(path);
    } catch {
      // ignore
    }
  };

  // "确认生效" — save the backup config, collapse the form, and trigger the
  // first full backup (via the parent's handleInitSave → runBackup).
  const handleConfirm = () => {
    const t = backupTarget.trim();
    if (!t) return;
    const patch: { mode: ProjectMode; backup?: Project["backup"] } = { mode: initialMode };
    patch.backup = {
      kind: backupKind,
      target: t,
      method: backupKind === "local" ? backupMethod : undefined,
      mirrorDelay:
        backupKind === "local" && backupMethod === "mirror" ? mirrorDelay : undefined,
    };
    onSave(patch);
    setEditing(false);
  };

  const handleSaveMark = () => {
    if (!markTitle.trim()) return;
    onMark(markTitle.trim(), markBody.trim() || null);
    setMarkTitle("");
    setMarkBody("");
  };

  // Ask the configured AI to draft a checkpoint title from the current
  // uncommitted changes, then drop it into the title field. Errors surface
  // inline beneath the button rather than as a global banner so the user
  // can fix the config and retry without losing context.
  const handleAiGenerate = async () => {
    if (aiGenerating) return;
    setAiGenerating(true);
    setAiGenError(null);
    try {
      const title = await onAiGenerateMark();
      setMarkTitle(title);
    } catch (e) {
      setAiGenError(String(e));
    } finally {
      setAiGenerating(false);
    }
  };

  // Stash the current uncommitted changes. An empty result means there was
  // nothing to stash (git's "No local changes to save") — we tell the user
  // instead of silently doing nothing.
  const handleStashPush = async () => {
    if (stashLoading) return;
    setStashLoading(true);
    setStashError(null);
    setStashInfo(null);
    try {
      const out = await gitStashPush(null);
      const list = await gitStashList();
      setStashCount(list.length);
      setStashInfo(out.trim() === "" ? tr.gitStashNothing : tr.gitStashStashed);
      // Stashing mutates the working tree — refresh the change indicator.
      setChangesTick((t) => t + 1);
    } catch (e) {
      setStashError(String(e));
    } finally {
      setStashLoading(false);
    }
  };

  // Pop the top stash back into the working tree. On conflict git exits
  // non-zero and keeps the stash — we show git's output and re-fetch the
  // count in case the pop partially applied.
  const handleStashPop = async () => {
    if (stashLoading) return;
    setStashLoading(true);
    setStashError(null);
    setStashInfo(null);
    try {
      await gitStashPop();
      const list = await gitStashList();
      setStashCount(list.length);
      setStashInfo(tr.gitStashPopped);
      // Popping mutates the working tree — refresh the change indicator.
      setChangesTick((t) => t + 1);
    } catch (e) {
      setStashError(String(e));
      try {
        const list = await gitStashList();
        setStashCount(list.length);
      } catch {
        // ignore — the error is already surfaced
      }
      // A conflicted pop may have partially applied — refresh regardless.
      setChangesTick((t) => t + 1);
    } finally {
      setStashLoading(false);
    }
  };

  // Discard uncommitted modifications to tracked files via `git restore`.
  // Only tracked files (modified/removed) can be restored — untracked files
  // aren't in the index so `git restore` ignores them. Destructive: the
  // changes are gone for good, which is why we use a two-step confirmation.
  const handleRestore = async () => {
    const restorePaths = uncommittedChanges
      .filter((c) => c.change === "modified" || c.change === "removed")
      .map((c) => c.path);
    if (!restorePaths.length) {
      setRestoreInfo(tr.gitRestoreNothing);
      setRestoreConfirming(false);
      return;
    }
    setRestoreLoading(true);
    setRestoreError(null);
    setRestoreInfo(null);
    try {
      await gitRestore(restorePaths);
      setRestoreInfo(tr.gitRestored);
      setRestoreConfirming(false);
      // Restore mutates the working tree — refresh the change indicator.
      setChangesTick((t) => t + 1);
    } catch (e) {
      setRestoreError(String(e));
    } finally {
      setRestoreLoading(false);
    }
  };

  // Create a tag at the current HEAD. The form collapses on success so the
  // user sees the new chip appear in the list immediately.
  const handleTagCreate = async () => {
    const name = tagName.trim();
    if (!name || tagLoading) return;
    setTagLoading(true);
    setTagError(null);
    try {
      await gitTagCreate(name, null);
      setTagName("");
      setTagFormOpen(false);
      const list = await gitTagList();
      setTags(list);
    } catch (e) {
      setTagError(String(e));
    } finally {
      setTagLoading(false);
    }
  };

  // Delete a tag by name. Re-fetches the list on success.
  const handleTagDelete = async (name: string) => {
    if (tagLoading) return;
    setTagLoading(true);
    setTagError(null);
    try {
      await gitTagDelete(name);
      const list = await gitTagList();
      setTags(list);
    } catch (e) {
      setTagError(String(e));
    } finally {
      setTagLoading(false);
    }
  };

  const kindLabel = (k: "local" | "cloud") =>
    k === "cloud" ? tr.initBackupCloud : tr.initBackupLocal;
  const methodLabel = (m: "mirror" | "incremental" | "standard") =>
    m === "mirror" ? tr.initMirror : m === "incremental" ? tr.initIncremental : tr.initStandard;
  const summaryMethod = project.backup?.method || "standard";

  // Uncommitted change counts — derived from the normalized list so both
  // Git mode and standard mode produce the same +/~/- tallies.
  const addedCount = uncommittedChanges.filter((c) => c.change === "added").length;
  const modifiedCount = uncommittedChanges.filter((c) => c.change === "modified").length;
  const removedCount = uncommittedChanges.filter((c) => c.change === "removed").length;
  // Whether there are restorable (tracked, modified/removed) files — gates
  // the "discard changes" button in the stash row.
  const hasRestorable = uncommittedChanges.some(
    (c) => c.change === "modified" || c.change === "removed",
  );

  return (
    <div className="workbench">
      {/* Top region — only after the backup target is configured. Holds the
          inline checkpoint editor and the manual full-backup trigger. */}
      {configured && !editing && (
        <div className="workbench-ops">
          <div className="workbench-head">
            <h2>{project.name}</h2>
            <span className="mono workbench-head-path">{project.path}</span>
          </div>

          {/* Uncommitted changes — quiet diff-tag counts so the user sees
              what they're about to commit before typing a title. Shown in
              both Git and standard modes; only appears when there are
              pending changes. */}
          {uncommittedChanges.length > 0 && (
            <div className="uncommitted-summary">
              {addedCount > 0 && <span className="diff-tag add">+{addedCount}</span>}
              {modifiedCount > 0 && <span className="diff-tag mod">~{modifiedCount}</span>}
              {removedCount > 0 && <span className="diff-tag del">-{removedCount}</span>}
              <span className="uncommitted-label">{tr.uncommittedChanges}</span>
            </div>
          )}

          <div className="checkpoint-inline">
            <input
              className="settings-input checkpoint-inline-title"
              value={markTitle}
              onChange={(e) => setMarkTitle(e.target.value)}
              placeholder={tr.markTitle}
            />
            <textarea
              className="settings-input checkpoint-inline-body"
              value={markBody}
              onChange={(e) => setMarkBody(e.target.value)}
              placeholder={tr.markBody}
              rows={2}
            />
            <div className="checkpoint-inline-actions">
              {aiActive && (
                <button
                  type="button"
                  className="ghost-btn"
                  onClick={handleAiGenerate}
                  disabled={aiGenerating}
                  title={tr.aiGenerateHint}
                >
                  {aiGenerating ? tr.aiGeneratingMark : tr.aiGenerateMark}
                </button>
              )}
              <button
                type="button"
                className="checkpoint-inline-mark"
                onClick={handleSaveMark}
                disabled={!markTitle.trim()}
              >
                {tr.mark}
              </button>
            </div>
            {aiGenError && <p className="settings-section-aside err">{aiGenError}</p>}
          </div>

          {gitModeEnabled && (
            <div className="stash-row">
              <button
                type="button"
                className="ghost-btn"
                onClick={handleStashPush}
                disabled={stashLoading}
              >
                {tr.gitStashPush}
              </button>
              {stashCount > 0 && (
                <button
                  type="button"
                  className="ghost-btn"
                  onClick={handleStashPop}
                  disabled={stashLoading}
                >
                  {tr.gitStashPop}
                </button>
              )}
              {stashCount > 0 && (
                <span className="mono stash-count">{stashCount}</span>
              )}
              {stashInfo && <span className="workspace-feedback ok">{stashInfo}</span>}
              {stashError && <span className="workspace-feedback err">{stashError}</span>}

              {/* Discard uncommitted modifications to tracked files. Only
                  appears when there are restorable (modified/removed)
                  files. Two-step inline confirmation because it's
                  destructive — the first click arms, the second executes. */}
              {hasRestorable && <span className="stash-sep" aria-hidden="true" />}
              {hasRestorable && !restoreConfirming && (
                <button
                  type="button"
                  className="ghost-btn danger"
                  onClick={() => {
                    setRestoreConfirming(true);
                    setRestoreError(null);
                    setRestoreInfo(null);
                  }}
                  disabled={restoreLoading || stashLoading}
                >
                  {tr.gitRestore}
                </button>
              )}
              {hasRestorable && restoreConfirming && (
                <>
                  <button
                    type="button"
                    className="ghost-btn danger"
                    onClick={handleRestore}
                    disabled={restoreLoading}
                  >
                    {restoreLoading ? "…" : tr.gitRestoreConfirm}
                  </button>
                  <button
                    type="button"
                    className="ghost-btn"
                    onClick={() => {
                      setRestoreConfirming(false);
                      setRestoreError(null);
                      setRestoreInfo(null);
                    }}
                    disabled={restoreLoading}
                  >
                    {tr.cancelBranch}
                  </button>
                </>
              )}
              {restoreInfo && <span className="workspace-feedback ok">{restoreInfo}</span>}
              {restoreError && <span className="workspace-feedback err">{restoreError}</span>}
            </div>
          )}

          {gitModeEnabled && (
            <div className="tag-row">
              {!tagFormOpen ? (
                <button
                  type="button"
                  className="ghost-btn"
                  onClick={() => setTagFormOpen(true)}
                  disabled={tagLoading}
                >
                  + {tr.gitTagAdd}
                </button>
              ) : (
                <div className="tag-form">
                  <input
                    className="settings-input tag-form-name"
                    value={tagName}
                    onChange={(e) => setTagName(e.target.value)}
                    placeholder={tr.gitTagPlaceholder}
                    autoFocus
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleTagCreate();
                      if (e.key === "Escape") {
                        setTagFormOpen(false);
                        setTagName("");
                        setTagError(null);
                      }
                    }}
                  />
                  <button
                    type="button"
                    className="ghost-btn"
                    onClick={handleTagCreate}
                    disabled={!tagName.trim() || tagLoading}
                  >
                    {tr.createBranch}
                  </button>
                  <button
                    type="button"
                    className="ghost-btn"
                    onClick={() => {
                      setTagFormOpen(false);
                      setTagName("");
                      setTagError(null);
                    }}
                    disabled={tagLoading}
                  >
                    {tr.cancelBranch}
                  </button>
                </div>
              )}
              {tags.length > 0 && (
                <div className="tag-list">
                  {tags.map((t) => (
                    <span key={t} className="tag-chip">
                      <span className="tag-chip-name">{t}</span>
                      <button
                        type="button"
                        className="tag-chip-del"
                        onClick={() => handleTagDelete(t)}
                        disabled={tagLoading}
                        aria-label={tr.gitTagDelete}
                        title={tr.gitTagDelete}
                      >
                        ×
                      </button>
                    </span>
                  ))}
                </div>
              )}
              {tagError && <span className="workspace-feedback err">{tagError}</span>}
            </div>
          )}

          <div className="workbench-backup-row">
            <button
              type="button"
              className="primary"
              onClick={onBackup}
              disabled={backupStatus === "running"}
            >
              {backupStatus === "running" ? "…" : tr.manualBackup}
            </button>
            {backupStatus === "done" && (
              <span className="workspace-feedback ok">{tr.backupDone}</span>
            )}
            {backupStatus === "fail" && (
              <span className="workspace-feedback err">{tr.backupFail}</span>
            )}
          </div>

          {error && <div className="error-banner">{error}</div>}
        </div>
      )}

      {/* Branch tree — absorbs the old timeline page. Shows all branches
          as lanes (main pinned to the bottom) with the current branch's
          commit history expanded inline. Only rendered once the repo is
          open so an unconfigured workspace doesn't show an empty tree. */}
      {branches.length > 0 && (
        <BranchTree
          branches={branches}
          currentBranch={currentBranch}
          commits={commits}
          gitModeEnabled={gitModeEnabled}
          onSwitch={onSwitchBranch}
          onCreate={onCreateBranch}
          onMerge={onMergeBranch}
          onRollback={onRollback}
          tr={tr}
        />
      )}

      {/* Tracking status strip — a single hairline-separated row that tells
          the user whether the watcher is running and which branch they're
          on. Sits between the ops region and the config card so it's always
          visible without scrolling past the branch tree. */}
      {configured && !editing && (
        <div className="workbench-status-strip">
          <span className={`workbench-status-dot ${trackingState}`} aria-hidden="true" />
          <span className="workbench-status-text">
            {trackingState === "running"
              ? tr.trackingRunning
              : trackingState === "starting"
                ? tr.trackingStarting
                : tr.trackingOff}
          </span>
          <span className="workbench-status-sep" aria-hidden="true" />
          <span className="workbench-status-branch">
            {tr.currentBranchLabel}: {currentBranch || "—"}
          </span>
        </div>
      )}

      {/* Bottom region — backup configuration as a big "配置" card with
          one sub-card per function. Controls sit full-width below each
          sub-card title so widths stay aligned (no label-left/control-right
          raggedness). Collapses to a single-line summary once confirmed so
          it stops eating vertical space after the first save. */}
      <section
        className={`settings-category workbench-config-card${configured && !editing ? " collapsed" : ""}`}
      >
        <header className="settings-category-header">
          <h3>{tr.workbenchConfigTitle}</h3>
          {configured && !editing && (
            <>
              <span className="mono workbench-config-summary">
                {project.backup?.target}
                <span className="workbench-config-summary-method">
                  {" · "}
                  {project.backup?.kind === "cloud"
                    ? kindLabel("cloud")
                    : methodLabel(summaryMethod)}
                </span>
              </span>
              <button
                type="button"
                className="workbench-edit"
                onClick={() => setEditing(true)}
              >
                {tr.workbenchEdit}
              </button>
            </>
          )}
        </header>
        {(!configured || editing) && (
          <div className="settings-category-body">
            <>
              {/* 源 — read-only project path. */}
              <div className="settings-subcard">
                <h4 className="settings-subcard-title">{tr.workbenchSourceLabel}</h4>
                <span className="mono workbench-source-readonly">{project.path}</span>
              </div>

              {/* 备份目标 — local / cloud. */}
              <div className="settings-subcard">
                <h4 className="settings-subcard-title">{tr.workbenchBackupTarget}</h4>
                <div className="seg-control workbench-seg">
                  <button
                    type="button"
                    className={backupKind === "local" ? "active" : ""}
                    onClick={() => setBackupKind("local")}
                  >
                    {tr.initBackupLocal}
                  </button>
                  <button
                    type="button"
                    className={backupKind === "cloud" ? "active" : ""}
                    onClick={() => setBackupKind("cloud")}
                  >
                    {tr.initBackupCloud}
                  </button>
                </div>
              </div>

              {/* 目标路径 — input + picker. */}
              <div className="settings-subcard">
                <h4 className="settings-subcard-title">{tr.workbenchTargetPath}</h4>
                <div className="workbench-target-row">
                  <input
                    className="settings-input"
                    value={backupTarget}
                    onChange={(e) => setBackupTarget(e.target.value)}
                    placeholder={
                      backupKind === "cloud"
                        ? "s3://bucket/path  ·  webdav://host/path  ·  ssh://server/path"
                        : "C:\\Users\\me\\backups"
                    }
                  />
                  {backupKind === "local" && (
                    <button type="button" className="settings-input-pick" onClick={handlePick}>
                      {tr.pickTarget}
                    </button>
                  )}
                </div>
              </div>

              {/* 备份方式 — local only. */}
              {backupKind === "local" && (
                <div className="settings-subcard">
                  <h4 className="settings-subcard-title">{tr.workbenchBackupMethod}</h4>
                  <div className="seg-control workbench-seg">
                    <button
                      type="button"
                      className={backupMethod === "mirror" ? "active" : ""}
                      onClick={() => setBackupMethod("mirror")}
                    >
                      {tr.initMirror}
                    </button>
                    <button
                      type="button"
                      className={backupMethod === "incremental" ? "active" : ""}
                      onClick={() => setBackupMethod("incremental")}
                    >
                      {tr.initIncremental}
                    </button>
                    <button
                      type="button"
                      className={backupMethod === "standard" ? "active" : ""}
                      onClick={() => setBackupMethod("standard")}
                    >
                      {tr.initStandard}
                    </button>
                  </div>
                  <p className="settings-method-hint">
                    {backupMethod === "mirror" && tr.initMirrorHint}
                    {backupMethod === "incremental" && tr.initIncrementalHint}
                    {backupMethod === "standard" && tr.initStandardHint}
                  </p>
                  {backupMethod === "mirror" && (
                    <div className="workbench-delay-row">
                      <label className="workbench-delay-label">{tr.initMirrorDelay}</label>
                      <input
                        type="number"
                        min={0}
                        className="settings-input workbench-delay-input"
                        value={mirrorDelay}
                        onChange={(e) =>
                          setMirrorDelay(Math.max(0, Number(e.target.value) || 0))
                        }
                      />
                    </div>
                  )}
                </div>
              )}

              <div className="workbench-config-foot">
                {configured && (
                  <button
                    type="button"
                    className="workbench-cancel"
                    onClick={() => setEditing(false)}
                  >
                    {tr.markCancel}
                  </button>
                )}
                <button
                  type="button"
                  className="primary"
                  onClick={handleConfirm}
                  disabled={!backupTarget.trim()}
                >
                  {tr.workbenchConfirm}
                </button>
              </div>
            </>
          </div>
        )}
      </section>
    </div>
  );
}

// ---------------------------------------------------------------------------
// BridgeSplash — shown while the Tauri IPC bridge is still warming up
// ---------------------------------------------------------------------------

function BridgeSplash({ tr, locale }: { tr: Dict; locale: Locale }) {
  return (
    <div className="app-shell">
      <Titlebar locale={locale} />
      <div className="bridge-splash">
        <div className="bridge-splash-mark">
          <Logo size={28} withWordmark={false} />
        </div>
        <Typewriter text={tr.welcome} speed={70} />
        <div className="bridge-splash-label">{tr.errorTauriBridge}</div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// NoProjectHint — inline "select a project" prompt shown in the main area
// when the user is on a page but no project is active. Minimal: just a
// centered hint + add button. The full Welcome page is reserved for the
// zero-projects case.
// ---------------------------------------------------------------------------

function NoProjectHint({
  text,
  cta,
  onAddProject,
}: {
  text: string;
  cta: string;
  onAddProject: () => void;
}) {
  return (
    <div className="no-project-hint">
      <div className="no-project-hint-text">{text}</div>
      <button type="button" className="primary" onClick={onAddProject}>
        {cta}
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// App — root component
// ---------------------------------------------------------------------------

export default function App() {
  // ---- core state ----
  const [bridgeReady, setBridgeReady] = useState(false);
  const [info, setInfo] = useState<StatusDto | null>(null);
  const [tree, setTree] = useState<TreeNode | null>(null);
  const [commits, setCommits] = useState<CommitDto[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [projects, setProjects] = useState<Project[]>(loadProjects);
  const [activeProjectId, setActiveProjectId] = useState<string | null>(null);
  const [page, setPage] = useState<Page>(loadPage);
  const [locale, setLocale] = useState<Locale>(getInitialLocale);
  const [theme, setTheme] = useState<"dark" | "light">(loadTheme);
  const [languageStage] = useState(0);
  const [projectModes, setProjectModes] = useState<Record<string, ProjectMode>>(loadProjectModes);
  const [projectInitialized, setProjectInitialized] = useState<Record<string, boolean>>(
    loadProjectInitialized,
  );
  const [cliMcpEnabled, setCliMcpEnabled] = useState<boolean>(loadCliMcpEnabled);
  const [aiAssistantMode, setAiAssistantMode] = useState<AiAssistantMode>(loadAiAssistantMode);
  const [aiApiKey, setAiApiKey] = useState<string>(loadAiApiKey);
  const [aiApiEndpoint, setAiApiEndpoint] = useState<string>(loadAiApiEndpoint);
  const [aiProvider, setAiProvider] = useState<AiProvider>(loadAiProvider);
  const [aiModel, setAiModel] = useState<string>(loadAiModel);
  const [aiTesting, setAiTesting] = useState<boolean>(false);
  const [aiTestResult, setAiTestResult] = useState<{ ok: boolean; msg: string } | null>(null);
  const [gitModeEnabled, setGitModeEnabledState] = useState<boolean>(loadGitModeEnabled);
  const [gitDetecting, setGitDetecting] = useState<boolean>(false);
  const [gitDetectResult, setGitDetectResult] = useState<GitDetectResult | null>(null);
  const [trackingStates, setTrackingStates] = useState<Record<string, TrackingState>>(loadTrackingStates);
  const [aiOperator, setAiOperator] = useState<AiOperatorDto | null>(null);
  const [aiPrompt, setAiPrompt] = useState<string>("");
  const [aiIndexPath, setAiIndexPath] = useState<string | null>(null);
  const [aiPromptLoading, setAiPromptLoading] = useState(false);
  const [aiConflictOpen, setAiConflictOpen] = useState(false);
  const [projectSortMode, setProjectSortMode] = useState<ProjectSort>(loadProjectSortMode);
  const [track, setTrack] = useState<TrackConfigDto | null>(null);
  const [watchStatus, setWatchStatus] = useState<WatchStatusDto | null>(null);
  const [backupStatus, setBackupStatus] = useState<"idle" | "running" | "done" | "fail">("idle");

  // ---- refs (mirror for stale-closure safety in callbacks) ----
  const projectsRef = useRef<Project[]>(projects);
  const undoStackRef = useRef<CommitDto[]>([]);
  const infoRef = useRef<StatusDto | null>(info);

  useEffect(() => {
    projectsRef.current = projects;
  }, [projects]);
  useEffect(() => {
    infoRef.current = info;
  }, [info]);
  useEffect(() => {
    undoStackRef.current = commits;
  }, [commits]);

  // ---- derived ----
  const tr = i18nT(locale);
  const activeProject = useMemo(
    () => projects.find((p) => p.id === activeProjectId) || null,
    [projects, activeProjectId],
  );
  const projectPath = activeProject?.path || null;
  const projectMode: ProjectMode = activeProjectId
    ? projectModes[activeProjectId] || "standard"
    : "standard";

  // Per-project tracking state. Each project has its own independent
  // switch: off (gray) → starting (accent light) → running (accent).
  const activeTrackingState: TrackingState = activeProjectId
    ? trackingStates[activeProjectId] || "off"
    : "off";

  // appOn is derived: true when the active project is starting or running.
  const appOn = activeTrackingState !== "off";

  const handleSetTracking = async (projectId: string, on: boolean) => {
    const current = trackingStates[projectId] || "off";
    if (on && current === "off") {
      // off → starting → running. The watcher is started for real now —
      // the sidebar power switch is no longer a UI-only simulation. The
      // "starting" state gives the user immediate feedback while the
      // backend spins up the polling thread; "running" lands once the
      // first watch_status poll confirms the thread is alive.
      setTrackingStates((prev) => ({ ...prev, [projectId]: "starting" }));
      try {
        await watchStart();
        const s = await watchStatusApi();
        setWatchStatus(s);
        setTrackingStates((prev) => ({ ...prev, [projectId]: "running" }));
      } catch (e) {
        // Roll back to off so the switch doesn't lie about its state.
        setTrackingStates((prev) => ({ ...prev, [projectId]: "off" }));
        setError(localizeBridgeError(e));
      }
    } else if (!on && current !== "off") {
      // starting or running → off
      try {
        await watchStop();
        const s = await watchStatusApi();
        setWatchStatus(s);
      } catch (e) {
        setError(localizeBridgeError(e));
      }
      setTrackingStates((prev) => ({ ...prev, [projectId]: "off" }));
    }
  };

  const handleToggleTracking = (projectId: string) => {
    const current = trackingStates[projectId] || "off";
    handleSetTracking(projectId, current === "off");
  };

  // ---- effects: persist + bridge detect + initial sync ----

  // 0. Auto-select first project when none is active.
  useEffect(() => {
    if (!activeProjectId && projects.length > 0) {
      setActiveProjectId(projects[0].id);
    }
  }, [activeProjectId, projects]);

  // 1. Save projects whenever they change.
  useEffect(() => {
    saveProjects(projects);
  }, [projects]);

  // 2. Persist locale.
  useEffect(() => {
    persistLocale(locale);
  }, [locale]);

  // 2b. Theme — keep React state, the DOM attribute and localStorage in sync.
  // Using a single effect avoids direct DOM writes inside render / handlers,
  // which can race with React and stress WebView2's software renderer.
  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    persistTheme(theme);
  }, [theme]);

  // 3. Persist page.
  useEffect(() => {
    persistPage(page);
  }, [page]);

  // 4. Persist tracking states (per-project).
  useEffect(() => {
    saveTrackingStates(trackingStates);
  }, [trackingStates]);

  // 5. Persist project modes.
  useEffect(() => {
    persistProjectModes(projectModes);
  }, [projectModes]);

  // 6. Persist init flags.
  useEffect(() => {
    persistProjectInitialized(projectInitialized);
  }, [projectInitialized]);

  // 7. Persist sort mode.
  useEffect(() => {
    persistProjectSortMode(projectSortMode);
  }, [projectSortMode]);

  // 8. Persist CLI / MCP.
  useEffect(() => {
    persistCliMcpEnabled(cliMcpEnabled);
  }, [cliMcpEnabled]);

  // 8b. Persist AI assistant mode + API key + endpoint.
  useEffect(() => {
    persistAiAssistantMode(aiAssistantMode);
  }, [aiAssistantMode]);
  useEffect(() => {
    persistAiApiKey(aiApiKey);
  }, [aiApiKey]);
  useEffect(() => {
    persistAiApiEndpoint(aiApiEndpoint);
  }, [aiApiEndpoint]);
  useEffect(() => {
    persistAiProvider(aiProvider);
  }, [aiProvider]);
  useEffect(() => {
    persistAiModel(aiModel);
  }, [aiModel]);

  // 8c. Persist git mode toggle + re-fetch timeline when the mode flips.
  // When git mode is ON, the timeline reads from the user's git log; when
  // OFF, it reads from the built-in route_basic log. Either way the
  // commit list needs a refresh on toggle so the displayed history
  // matches the active backend.
  useEffect(() => {
    persistGitModeEnabled(gitModeEnabled);
    // Re-fetch the commit list so the timeline switches sources. The
    // refresh() function checks `gitModeEnabled` and routes accordingly.
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [gitModeEnabled]);

  // 9. Bridge detection.
  // In a plain browser (no Tauri host), the bridge never appears.
  // After 2s of polling we give up and set bridgeReady anyway so the
  // user can at least see the UI (welcome page, settings, etc.)
  // without the full Tauri runtime. IPC calls will fail gracefully.
  useEffect(() => {
    if (isTauriBridgeAvailable()) {
      setBridgeReady(true);
      return;
    }
    const id = window.setInterval(() => {
      if (isTauriBridgeAvailable()) {
        setBridgeReady(true);
        window.clearInterval(id);
      }
    }, 80);
    // Fallback: after 2s with no bridge, enter browser preview mode.
    const fallback = window.setTimeout(() => {
      if (!isTauriBridgeAvailable()) {
        setBridgeReady(true);
      }
    }, 2000);
    return () => {
      window.clearInterval(id);
      window.clearTimeout(fallback);
    };
  }, []);

  // #region debug-point global-error-handlers
  useEffect(() => {
    dbg("app:mount", { bridgeReady, page, projectsCount: projects.length });
    const onError = (e: ErrorEvent) => {
      dbg("app:error", {
        message: e.message,
        filename: e.filename,
        lineno: e.lineno,
        colno: e.colno,
        error: e.error ? String(e.error) : null,
      });
    };
    const onRejection = (e: PromiseRejectionEvent) => {
      dbg("app:rejection", { reason: e.reason ? String(e.reason) : null });
    };
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  }, []);
  // #endregion debug-point global-error-handlers

  // 10. AI operator sync (on mount + on project change).
  useEffect(() => {
    if (!bridgeReady) return;
    let cancelled = false;
    getAiOperator()
      .then((op) => {
        if (!cancelled) setAiOperator(op);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [bridgeReady, activeProjectId]);

  // 11. Load the active project whenever the user switches.
  useEffect(() => {
    if (!bridgeReady) return;
    if (!activeProjectId || !projectPath) {
      setInfo(null);
      setTree(null);
      setCommits([]);
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    (async () => {
      try {
        try {
          await openRepo(projectPath);
        } catch {
          await initRepo(projectPath);
        }
        // Mirror `refresh()`: in git mode the timeline comes from `gitLog`,
        // NOT route_basic's historyTree/logCommits — otherwise switching
        // projects in git mode would show route_basic history until the next
        // operation triggers a refresh. We branch inline (rather than calling
        // refresh()) to preserve this effect's own loading/cancel semantics
        // and avoid a stale-closure over `refresh` (defined later, re-created
        // every render). `gitModeEnabled` is intentionally NOT in this
        // effect's dep array — toggling git mode routes through the toggle
        // handler's own refresh(), not this project-switch effect.
        if (gitModeEnabled) {
          const [s, gitEntries] = await Promise.all([
            status(),
            gitLog(200).catch(() => [] as GitLogEntryDto[]),
          ]);
          if (cancelled) return;
          setInfo(s);
          setCommits(gitEntries.map(gitLogEntryToCommitDto));
          // historyTree is route_basic-specific; leave the previous tree in
          // place so any consumer that reads it doesn't crash.
        } else {
          const [s, t, c] = await Promise.all([
            status(),
            historyTree(),
            logCommits(100, null),
          ]);
          if (cancelled) return;
          setInfo(s);
          setTree(t);
          setCommits(c);
        }
      } catch (e) {
        if (!cancelled) setError(localizeBridgeError(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [bridgeReady, activeProjectId, projectPath]);

  // 12. Track config — load on demand when the settings page is shown.
  useEffect(() => {
    if (!bridgeReady) return;
    if (page !== "settings") return;
    let cancelled = false;
    trackGet()
      .then((c) => {
        if (!cancelled) setTrack(c);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [bridgeReady, page]);

  // 13. AI prompt + index path — load on demand when the settings page is shown.
  useEffect(() => {
    if (!bridgeReady) return;
    if (page !== "settings") return;
    let cancelled = false;
    setAiPromptLoading(true);
    aiSummaryPrompt()
      .then((p) => {
        if (!cancelled) setAiPrompt(p);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) setAiPromptLoading(false);
      });
    routeIndexPath()
      .then((p) => {
        if (!cancelled) setAiIndexPath(p);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [bridgeReady, page]);

  // 14. Watch status — poll every 2s while a project is active.
  useEffect(() => {
    if (!bridgeReady || !activeProjectId) {
      setWatchStatus(null);
      return;
    }
    let cancelled = false;
    const tick = async () => {
      try {
        const s = await watchStatusApi();
        if (!cancelled) setWatchStatus(s);
      } catch {
        // ignore
      }
    };
    tick();
    const id = window.setInterval(tick, 2000);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [bridgeReady, activeProjectId]);

  // ---- handlers ----

  const handleLanguageSelect = (newLocale: Locale) => {
    setLocale(newLocale);
  };

  const refresh = async () => {
    if (!projectPath) return;
    setLoading(true);
    setError(null);
    try {
      // Git mode reads the timeline from the user's own git log; the
      // built-in route_basic log is skipped. status()/historyTree() are
      // still fetched because the workbench header and other surfaces
      // read project_path / current_branch from them — git mode only
      // swaps the commit history source, not the repo status.
      if (gitModeEnabled) {
        const [s, gitEntries] = await Promise.all([
          status(),
          gitLog(200).catch(() => [] as GitLogEntryDto[]),
        ]);
        setInfo(s);
        setCommits(gitEntries.map(gitLogEntryToCommitDto));
        // historyTree is route_basic-specific; leave the previous tree
        // in place when git mode is on so any consumer that reads it
        // doesn't crash, but don't fetch a fresh one.
      } else {
        const [s, t, c] = await Promise.all([
          status(),
          historyTree(),
          logCommits(100, null),
        ]);
        setInfo(s);
        setTree(t);
        setCommits(c);
      }
    } catch (e) {
      setError(localizeBridgeError(e));
    } finally {
      setLoading(false);
    }
  };

  const handlePickFolder = async () => {
    try {
      const path = await pickFolder();
      if (!path) return;
      const name = shortName(path) || "project";
      const id = `p${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
      const newProject: Project = {
        id,
        name,
        path,
        routeA: false,
        addedAt: Date.now(),
      };
      setProjects((prev) => [...prev, newProject]);
      setActiveProjectId(id);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  const addProject = handlePickFolder;

  const removeProject = (id: string) => {
    setProjects((prev) => prev.filter((p) => p.id !== id));
    setProjectModes((prev) => {
      const n = { ...prev };
      delete n[id];
      return n;
    });
    setProjectInitialized((prev) => {
      const n = { ...prev };
      delete n[id];
      return n;
    });
    if (activeProjectId === id) {
      const remaining = projects.filter((p) => p.id !== id);
      setActiveProjectId(remaining.length > 0 ? remaining[0].id : null);
    }
  };

  const selectProject = (id: string) => {
    setActiveProjectId(id);
  };

  const reorderProjects = (fromId: string, toId: string) => {
    if (fromId === toId) return;
    setProjects((prev) => {
      const fromIdx = prev.findIndex((p) => p.id === fromId);
      const toIdx = prev.findIndex((p) => p.id === toId);
      if (fromIdx === -1 || toIdx === -1) return prev;
      const next = prev.slice();
      const [moved] = next.splice(fromIdx, 1);
      next.splice(toIdx, 0, moved);
      return next;
    });
  };

  const handleRollbackTo = async (snapshotId: string) => {
    try {
      if (gitModeEnabled) {
        // In Git mode `snapshotId` is the git SHA (see gitLogEntryToCommitDto:
        // to_snapshot = g.sha). Route to a non-destructive git rollback that
        // restores the tree to that commit and records it as a new commit on
        // top of HEAD — the Git mode counterpart of route_basic's rollback.
        await gitRollback(snapshotId);
      } else {
        await rollback(snapshotId, null);
      }
      await refresh();
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  // ---- branch operations (workbench branch tree) ----
  // Each handler routes to the git or route_basic backend depending on the
  // active mode, so the BranchTree component stays mode-agnostic. Git mode
  // ignores the `kind` / `from` distinction (all git branches are regular);
  // route_basic uses them to pick inherited vs sandbox semantics.

  const handleSwitchBranch = async (name: string) => {
    try {
      if (gitModeEnabled) {
        await gitBranchSwitch(name);
      } else {
        await branchSwitch(name);
      }
      await refresh();
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  const handleCreateBranch = async (
    name: string,
    kind: "inherited" | "sandbox",
    from: string | null,
  ) => {
    try {
      if (gitModeEnabled) {
        // Git has no kind distinction — every branch is a regular branch
        // created at HEAD. `from` is ignored.
        await gitBranchCreate(name);
      } else {
        await branchCreate(name, kind, from);
      }
      await refresh();
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  const handleMergeBranch = async (source: string) => {
    try {
      if (gitModeEnabled) {
        await gitMerge(source);
      } else {
        await branchMerge(source, null);
      }
      await refresh();
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  const handleClaimAiControl = async () => {
    try {
      let prompt = aiPrompt;
      if (!prompt) {
        prompt = await aiSummaryPrompt();
        setAiPrompt(prompt);
      }
      const op = await setAiOperatorApi("AI Agent", prompt);
      setAiOperator(op);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  const handleReleaseAiControl = async () => {
    try {
      await clearAiOperator();
      setAiOperator(null);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  const openAiConflictDialog = () => setAiConflictOpen(true);
  const closeAiConflictDialog = () => setAiConflictOpen(false);

  const handleCheckpoint = async (title: string, body: string | null) => {
    try {
      // Git mode: the 打点 button becomes a real `git add -A` + `git commit`.
      // The backend stamps the commit with a Route-Checkpoint trailer so
      // the timeline can still tell checkpoints apart from plain commits.
      if (gitModeEnabled) {
        await gitCommit(title, body);
      } else {
        await checkpointCreate(title, body);
      }
      await refresh();
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  /// Ask the configured AI to draft a checkpoint title from the current
  /// uncommitted changes. Returns the generated title text. Only callable
  /// when Active AI mode is on — the workbench hides the button otherwise.
  const handleAiGenerateMark = async (): Promise<string> => {
    if (aiAssistantMode !== "active") {
      throw new Error(tr.aiGenerateNotConfigured);
    }
    // In Git mode, uncommitted changes live in `git status` (the route_basic
    // working-dir probe doesn't see git's index / untracked files accurately).
    // In standard mode, route_basic's `working_dir_status` is the source of
    // truth. Both are normalized into the same `{change, path}` tuples so the
    // prompt is identical regardless of backend. Git's untracked files map to
    // "added" because `git add -A` will stage them as additions at commit time.
    const changeList: { change: string; path: string }[] = [];
    if (gitModeEnabled) {
      const s = await gitStatus();
      for (const p of s.added) changeList.push({ change: "added", path: p });
      for (const p of s.untracked) changeList.push({ change: "added", path: p });
      for (const p of s.modified) changeList.push({ change: "modified", path: p });
      for (const p of s.removed) changeList.push({ change: "removed", path: p });
    } else {
      const changes = await workingDirStatus();
      for (const c of changes) changeList.push({ change: c.change, path: c.path });
    }
    if (!changeList.length) {
      throw new Error(tr.aiGenerateNoChanges);
    }
    const fileList = changeList.map((c) => `- ${c.change}: ${c.path}`).join("\n");
    // In Git mode, also include the actual unified diff (truncated) so the
    // model can draft a more accurate title than file names alone allow.
    // Capped to keep the prompt bounded — a huge diff would blow the token
    // budget without materially improving a one-line title. Standard mode has
    // no text-diff API, so it stays file-list-only.
    let diffBlock = "";
    if (gitModeEnabled) {
      const DIFF_CAP = 6000;
      const d = await gitDiff();
      if (d.trim()) {
        diffBlock =
          d.length > DIFF_CAP
            ? d.slice(0, DIFF_CAP) +
              `\n... (diff truncated, ${d.length - DIFF_CAP} more chars omitted)`
            : d;
      }
    }
    const userContent = diffBlock
      ? `Changed files:\n${fileList}\n\nUnified diff (may be truncated):\n${diffBlock}`
      : `Changed files:\n${fileList}`;
    const messages: ChatMessage[] = [
      {
        role: "system",
        content:
          "You draft concise commit messages for the route version-control app. " +
          "Given a list of changed files (change type + relative path), and " +
          "optionally a unified diff, reply with ONE short imperative title " +
          "(<= 72 chars) summarizing the change. No quotes, no bullet points, " +
          "no body — just the single title line. " +
          `Reply in ${locale === "zh" ? "Chinese" : "English"}.`,
      },
      { role: "user", content: userContent },
    ];
    const reply = await aiChat(aiProvider, aiApiEndpoint, aiApiKey, aiModel, messages);
    // Take the first non-empty line and strip wrapping quotes/markdown the
    // model may add, so the result drops cleanly into the title input.
    let title =
      reply.trim().split("\n").map((l) => l.trim()).find((l) => l.length > 0) || reply.trim();
    title = title.replace(/^["'`*]+|["'`*.]+$/g, "").trim();
    return title.slice(0, 120);
  };

  /// Detect git on the user's PATH. Called from the Advanced settings card
  /// so the user can confirm git is reachable before relying on it.
  const handleGitDetect = async () => {
    setGitDetecting(true);
    try {
      const r = await gitDetect();
      setGitDetectResult(r);
    } catch (e) {
      setGitDetectResult({ available: false, version: "", error: localizeBridgeError(e) });
    } finally {
      setGitDetecting(false);
    }
  };

  /// Toggle git mode. When turning ON, best-effort run `git init` on the
  /// current project so the first checkpoint doesn't fail with "not a git
  /// repository". When turning OFF, just fall back to route_basic — no
  /// cleanup is done on the .git folder (the user's history is theirs).
  const handleSetGitModeEnabled = async (on: boolean) => {
    // #region debug-point app-handlers
    dbg("app:gitMode:before", { on, projectPath });
    // #endregion debug-point app-handlers
    setGitModeEnabledState(on);
    // Sync to the backend so the file watcher knows whether to commit
    // via `git add`+`git commit` (git mode) or route_basic's snapshot
    // engine. Without this the watcher would keep using the old mode
    // after a toggle.
    try {
      await gitModeSet(on);
      // #region debug-point app-handlers
      dbg("app:gitMode:ipc-ok", { on });
      // #endregion debug-point app-handlers
    } catch (e) {
      // #region debug-point app-handlers
      dbg("app:gitMode:ipc-err", { error: String(e) });
      // #endregion debug-point app-handlers
      setError(localizeBridgeError(e));
    }
    if (on && projectPath) {
      try {
        await gitInit();
        // #region debug-point app-handlers
        dbg("app:gitMode:init-ok", { projectPath });
        // #endregion debug-point app-handlers
      } catch (e) {
        // #region debug-point app-handlers
        dbg("app:gitMode:init-err", { error: String(e) });
        // #endregion debug-point app-handlers
        // Don't roll back the toggle — the user may want to init
        // themselves. Surface the error so they know what happened.
        setError(localizeBridgeError(e));
      }
    }
  };

  /// Switch AI provider and auto-fill the endpoint with the provider's
  /// canonical default when the field is empty or still holds another
  /// provider's default — saves the user from editing the URL by hand.
  const handleSetAiProvider = (p: AiProvider) => {
    setAiProvider(p);
    const cur = aiApiEndpoint.trim();
    const isDefault = Object.values(PROVIDER_DEFAULT_ENDPOINT).includes(cur);
    if (cur === "" || isDefault) {
      setAiApiEndpoint(PROVIDER_DEFAULT_ENDPOINT[p]);
    }
    setAiTestResult(null);
  };

  /// Fire a one-shot "ping" completion to verify the provider / endpoint /
  /// key / model actually work end-to-end. Runs entirely in the backend;
  /// the API key never reaches the webview.
  const handleTestAiConnection = async () => {
    if (aiTesting) return;
    setAiTesting(true);
    setAiTestResult(null);
    try {
      const ping: ChatMessage[] = [{ role: "user", content: "ping" }];
      const reply = await aiChat(aiProvider, aiApiEndpoint, aiApiKey, aiModel, ping);
      const snippet = reply.trim().slice(0, 80);
      setAiTestResult({
        ok: true,
        msg: snippet ? `${tr.aiTestOk}：${snippet}` : tr.aiTestOk,
      });
    } catch (e) {
      setAiTestResult({ ok: false, msg: String(e) });
    } finally {
      setAiTesting(false);
    }
  };

  const handleCommit = async (message: string) => {
    try {
      await commit(message, null, false, null);
      await refresh();
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  const setBackup = (backup: { kind: "local" | "cloud"; target: string } | undefined) => {
    if (!activeProjectId) return;
    setProjects((prev) =>
      prev.map((p) => (p.id === activeProjectId ? { ...p, backup } : p)),
    );
  };

  const toggleRoutea = () => {
    if (!activeProjectId) return;
    setProjects((prev) =>
      prev.map((p) => (p.id === activeProjectId ? { ...p, routeA: !p.routeA } : p)),
    );
  };

  const handleResetSettings = () => {
    setLocale(getInitialLocale());
    setCliMcpEnabled(false);
    setAiAssistantMode("follow");
    setAiApiKey("");
    setAiApiEndpoint("");
    setProjectSortMode("alpha");
    setGitModeEnabledState(false);
    setGitDetectResult(null);
  };

  const handleClearAllData = () => {
    setProjects([]);
    setActiveProjectId(null);
    setProjectModes({});
    setProjectInitialized({});
    setInfo(null);
    setTree(null);
    setCommits([]);
    setError(null);
  };

  const handleTrackSetAll = async (on: boolean) => {
    try {
      const updated = await trackSetAll(on);
      setTrack(updated);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };
  const handleTrackSetVerifySha256 = async (on: boolean) => {
    try {
      const updated = await trackSetVerifySha256(on);
      setTrack(updated);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };
  const handleTrackSetMemoryBufferMs = async (ms: number) => {
    try {
      const updated = await trackSetMemoryBufferMs(ms);
      setTrack(updated);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };
  const handleTrackSetOn = async (on: { windows: boolean; macos: boolean; linux: boolean }) => {
    try {
      const updated = await trackSetOn(on);
      setTrack(updated);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };
  const handleTrackSetSuffixes = async (s: string[]) => {
    try {
      const updated = await trackSet({ ...(track as TrackConfigDto), track_suffixes: s });
      setTrack(updated);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };
  const handleTrackSetPrefixes = async (p: string[]) => {
    try {
      const updated = await trackSet({ ...(track as TrackConfigDto), track_prefixes: p });
      setTrack(updated);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  const handleAiIndexRefresh = async () => {
    try {
      const path = await routeIndexRefresh();
      setAiIndexPath(path);
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };
  const handleCopyAiIndex = async () => {
    if (!aiIndexPath) return;
    try {
      await navigator.clipboard.writeText(aiIndexPath);
    } catch {
      // ignore
    }
  };

  const handleSetProjectBackup = (
    backup: { kind: "local" | "cloud"; target: string } | undefined,
  ) => {
    setBackup(backup);
  };
  const handleSetProjectRouteA = (routeA: boolean) => {
    if (!activeProjectId) return;
    setProjects((prev) =>
      prev.map((p) => (p.id === activeProjectId ? { ...p, routeA } : p)),
    );
  };
  const handleSetProjectMode = (mode: ProjectMode) => {
    if (!activeProjectId) return;
    setProjectModes((prev) => ({ ...prev, [activeProjectId]: mode }));
  };

  const handleInitSave = (patch: { mode: ProjectMode; backup?: Project["backup"] }) => {
    if (!activeProjectId) return;
    setProjects((prev) =>
      prev.map((p) => (p.id === activeProjectId ? { ...p, ...patch } : p)),
    );
    if (activeProjectId) {
      setProjectModes((prev) => ({ ...prev, [activeProjectId]: patch.mode }));
    }
    setProjectInitialized((prev) => ({ ...prev, [activeProjectId]: true }));

    // Immediate backup: as soon as the user picks a storage path, run a
    // full backup right away so the target folder is populated.
    if (patch.backup && patch.backup.target) {
      runBackup(patch.backup.target);
    }
  };

  // Run a full backup to the given target path. Updates `backupStatus`
  // so the workspace button can show a spinner / success / failure.
  const runBackup = (target: string) => {
    if (!target) return;
    setBackupStatus("running");
    backupToDir(target)
      .then(() => {
        setBackupStatus("done");
        window.setTimeout(() => setBackupStatus("idle"), 2500);
      })
      .catch(() => {
        setBackupStatus("fail");
        window.setTimeout(() => setBackupStatus("idle"), 3500);
      });
  };

  const handleManualBackup = () => {
    const target = activeProject?.backup?.target;
    if (!target) return;
    runBackup(target);
  };

  const handleToggleWatch = async () => {
    try {
      if (watchStatus && watchStatus.running) {
        await watchStop();
        const s = await watchStatusApi();
        setWatchStatus(s);
      } else {
        await watchStart();
        const s = await watchStatusApi();
        setWatchStatus(s);
      }
    } catch (e) {
      setError(localizeBridgeError(e));
    }
  };

  // ---- early returns ----

  if (!bridgeReady) {
    return <BridgeSplash tr={tr} locale={locale} />;
  }

  if (projects.length === 0) {
    return (
      <div className="app-shell">
        <Titlebar locale={locale} />
        <div className="app">
          <Sidebar
            projects={projects}
            activeProjectId={null}
            onSelectProject={() => {}}
            onAddProject={addProject}
            onRemoveProject={() => {}}
            onReorderProjects={() => {}}
            sortMode={projectSortMode}
            onSortModeChange={setProjectSortMode}
            page={page}
            onPageChange={setPage}
            activeTrackingState={"off"}
            onToggleTracking={() => {}}
            watchStatus={null}
            tr={tr}
          />
          <main className="main">
            <div className="empty-state">
              <p className="empty-state-hint">{tr.selectProjectFirst}</p>
              <button type="button" className="btn primary" onClick={addProject}>
                {tr.addProject}
              </button>
            </div>
          </main>
        </div>
      </div>
    );
  }

  // Main render
  return (
    <div className="app-shell">
      <Titlebar locale={locale} />
      <div className="app">
        <Sidebar
          projects={projects}
          activeProjectId={activeProjectId}
          onSelectProject={selectProject}
          onAddProject={addProject}
          onRemoveProject={removeProject}
          onReorderProjects={reorderProjects}
          sortMode={projectSortMode}
          onSortModeChange={setProjectSortMode}
          page={page}
          onPageChange={setPage}
          activeTrackingState={activeTrackingState}
          onToggleTracking={() => activeProjectId && handleToggleTracking(activeProjectId)}
          watchStatus={watchStatus}
          tr={tr}
        />
        <main className="main">
          {page === "workspace" && activeProject && info && (
            <WorkbenchPage
              project={activeProject}
              initialMode={projectMode}
              error={error}
              onSave={handleInitSave}
              onMark={handleCheckpoint}
              onBackup={handleManualBackup}
              backupStatus={backupStatus}
              branches={info.branches}
              currentBranch={info.current_branch}
              commits={commits}
              gitModeEnabled={gitModeEnabled}
              trackingState={activeTrackingState}
              onSwitchBranch={handleSwitchBranch}
              onCreateBranch={handleCreateBranch}
              onMergeBranch={handleMergeBranch}
              onRollback={handleRollbackTo}
              aiActive={aiAssistantMode === "active"}
              onAiGenerateMark={handleAiGenerateMark}
              tr={tr}
            />
          )}
          {page === "workspace" && (!activeProject || !info) && (
            <NoProjectHint
              text={tr.selectProjectFirst}
              cta={tr.addProject}
              onAddProject={addProject}
            />
          )}
          {page === "settings" && activeProject && info && (
            <SettingsPage
              project={activeProject}
              projectMode={projectMode}
              locale={locale}
              theme={theme}
              cliMcpEnabled={cliMcpEnabled}
              gitModeEnabled={gitModeEnabled}
              gitDetecting={gitDetecting}
              gitDetectResult={gitDetectResult}
              aiAssistantMode={aiAssistantMode}
              aiApiKey={aiApiKey}
              aiApiEndpoint={aiApiEndpoint}
              aiProvider={aiProvider}
              aiModel={aiModel}
              aiTesting={aiTesting}
              aiTestResult={aiTestResult}
              track={track}
              aiOperator={aiOperator}
              aiPrompt={aiPrompt}
              aiIndexPath={aiIndexPath}
              aiPromptLoading={aiPromptLoading}
              onSetLocale={setLocale}
              onSetTheme={setTheme}
              onSetCliMcpEnabled={setCliMcpEnabled}
              onSetGitModeEnabled={handleSetGitModeEnabled}
              onGitDetect={handleGitDetect}
              onSetAiAssistantMode={setAiAssistantMode}
              onSetAiApiKey={setAiApiKey}
              onSetAiApiEndpoint={setAiApiEndpoint}
              onSetAiProvider={handleSetAiProvider}
              onSetAiModel={setAiModel}
              onTestAiConnection={handleTestAiConnection}
              onSetProjectRouteA={handleSetProjectRouteA}
              onSetProjectMode={handleSetProjectMode}
              onTrackSetAll={handleTrackSetAll}
              onTrackSetVerifySha256={handleTrackSetVerifySha256}
              onTrackSetMemoryBufferMs={handleTrackSetMemoryBufferMs}
              onTrackSetSuffixes={handleTrackSetSuffixes}
              onTrackSetPrefixes={handleTrackSetPrefixes}
              onTrackSetOn={handleTrackSetOn}
              onClaimAiControl={handleClaimAiControl}
              onReleaseAiControl={handleReleaseAiControl}
              onOpenAiConflict={openAiConflictDialog}
              onAiIndexRefresh={handleAiIndexRefresh}
              onCopyAiIndex={handleCopyAiIndex}
              onResetSettings={handleResetSettings}
              onClearAllData={handleClearAllData}
              tr={tr}
            />
          )}
          {page === "settings" && (!activeProject || !info) && (
            <NoProjectHint
              text={tr.selectProjectFirst}
              cta={tr.addProject}
              onAddProject={addProject}
            />
          )}
        </main>
      </div>
      <AiConflictDialog
        open={aiConflictOpen}
        onClose={closeAiConflictDialog}
        tr={tr}
      />
    </div>
  );
}
