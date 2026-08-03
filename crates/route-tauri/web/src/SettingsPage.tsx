import { useEffect, useRef, useState } from "react";
import {
  type AutostartConfig,
  type AiOperatorDto,
  type TrackConfigDto,
  type GitDetectDto,
  type McpConfigDto,
  type AiProvider,
  type TrackedFolderDto,
  type ExtensionEntry,
  aiConflictReport,
  aiConflictResolve,
  autostartGet,
  autostartSet,
  mcpGetConfig,
  startRouteCli,
  stopRouteCli,
  startRouteMcp,
  stopRouteMcp,
  aiChat,
  type ChatMessage,
} from "./api";
import { type Locale, type Dict } from "./i18n";


// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

type ProjectMode = "standard" | "ai";
type AiAssistantMode = "follow" | "active";
type GitDetectResult = GitDetectDto;

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

// ---------------------------------------------------------------------------
// Switch — accessible toggle button
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
      <span className="switch-plate">
        <span className="switch-toggle" />
        <span className="switch-indicator" />
      </span>
      {statusText && <span className="status">{statusText}</span>}
    </button>
  );
}

// ---------------------------------------------------------------------------
// ThemeCapsuleSwitch — capsule-style toggle for light/dark theme
// No slider, click toggles. Active state shows accent color.
// ---------------------------------------------------------------------------

function ThemeCapsuleSwitch({
  theme,
  onChange,
  tr,
}: {
  theme: "light" | "dark";
  onChange: (t: "light" | "dark") => void;
  tr: Dict;
}) {
  const isLight = theme === "light";
  return (
    <button
      type="button"
      className={`theme-capsule${isLight ? " active" : ""}`}
      onClick={() => onChange(isLight ? "dark" : "light")}
      aria-label={`Switch to ${isLight ? "dark" : "light"} mode`}
    >
      <span className="theme-capsule-track">
        <span className="theme-capsule-knob" />
      </span>
      <span className="theme-capsule-label">
        {isLight ? tr.themeLight : tr.themeDark}
      </span>
    </button>
  );
}

// ---------------------------------------------------------------------------
// HoldButton — must be held for `holdMs` before firing
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
// TrackConfigPanel
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
    <div className="settings-section">
      <h4 className="settings-section-title">{tr.trackSection}</h4>
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
          />
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// DangerZone
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
  const [clearExpanded, setClearExpanded] = useState(false);
  const [countdown, setCountdown] = useState(10);
  const [clearing, setClearing] = useState(false);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const startClearCountdown = () => {
    setClearExpanded(true);
    setCountdown(10);
    setClearing(false);
    timerRef.current = setInterval(() => {
      setCountdown((prev) => {
        if (prev <= 1) {
          if (timerRef.current) clearInterval(timerRef.current);
          timerRef.current = null;
          setClearing(true);
          onClearAllData();
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
  };

  const cancelClear = () => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
    setClearExpanded(false);
    setCountdown(10);
    setClearing(false);
  };

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, []);

  return (
    <div className="settings-section">
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
            {!clearExpanded ? (
              <button
                type="button"
                className="ghost-btn danger clear-data-trigger"
                onClick={startClearCountdown}
              >
                {tr.clearDataStart}
              </button>
            ) : (
              <div className="clear-countdown-card">
                <div className="clear-countdown-number">
                  {clearing ? "0" : countdown}
                </div>
                <div className="clear-countdown-label">
                  {clearing ? tr.clearDataDone : tr.clearDataCountdown}
                </div>
                <div className="clear-countdown-bar">
                  <div
                    className="clear-countdown-fill"
                    style={{ width: `${clearing ? 100 : (10 - countdown) * 10}%` }}
                  />
                </div>
                {!clearing && (
                  <button
                    type="button"
                    className="ghost-btn"
                    onClick={cancelClear}
                  >
                    {tr.cancelBranch}
                  </button>
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// AutostartSection
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

  return (
    <div className="settings-section">
      <h4 className="settings-section-title">{tr.startupSection}</h4>
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
  );
}

// ---------------------------------------------------------------------------
// CloseBehaviorSection — standalone component for close behavior
// ---------------------------------------------------------------------------

function CloseBehaviorSection({ tr }: { tr: Dict }) {
  const [cfg, setCfg] = useState<AutostartConfig | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setError(null);
    autostartGet()
      .then((c) => {
        if (!cancelled) setCfg(c);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const update = (patch: Partial<AutostartConfig>) => {
    setError(null);
    autostartSet(patch)
      .then((c) => setCfg(c))
      .catch((e) => {
        setError(String(e));
      });
  };

  return (
    <div className="settings-section">
      <h4 className="settings-section-title">{tr.closeBehaviorLabel}</h4>
      <div className="settings-row">
        <div className="settings-row-label">
          <span>{tr.closeBehaviorCapsule}</span>
          <span className="settings-row-hint">{tr.closeBehaviorHint}</span>
        </div>
        <div className="settings-row-control">
          <div className="seg-control">
            <button
              type="button"
              className={cfg?.close_behavior === "quit" ? "active" : ""}
              onClick={() => update({ close_behavior: "quit" })}
            >
              {tr.closeBehaviorQuit}
            </button>
            <button
              type="button"
              className={cfg?.close_behavior === "hide" ? "active" : ""}
              onClick={() => update({ close_behavior: "hide" })}
            >
              {tr.closeBehaviorHide}
            </button>
          </div>
        </div>
      </div>
      {error && (
        <div className="settings-section-hint" style={{ color: 'var(--danger)' }}>
          {error}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// AiConflictDialog
// ---------------------------------------------------------------------------

type AiConflictReportLike = {
  commit_id: string;
  body: string;
  conflicts: { path: string; reason: string; recommendation: string }[];
};

// ---------------------------------------------------------------------------
// TrackingFolderList — add/remove remote repos for active tracking
// ---------------------------------------------------------------------------

function TrackingFolderList({
  projectPath,
  tr,
}: {
  projectPath: string;
  tr: Dict;
}) {
  const [folders, setFolders] = useState<TrackedFolderDto[]>([]);
  const [newRemote, setNewRemote] = useState("");
  const [newBranch, setNewBranch] = useState("main");
  const [newInterval, setNewInterval] = useState(10);
  const [syncingPath, setSyncingPath] = useState<string | null>(null);
  const [syncResult, setSyncResult] = useState<string | null>(null);
  const [schedulerRunning, setSchedulerRunning] = useState(false);

  const refresh = () => {
    import("./api").then((api) => {
      api.trackingList().then(setFolders).catch(() => {});
    });
  };

  const refreshSchedulerStatus = () => {
    import("./api").then((api) => {
      api.trackingStatus().then(setSchedulerRunning).catch(() => {});
    });
  };

  useEffect(() => {
    refresh();
    refreshSchedulerStatus();
  }, [projectPath]);

  const handleAdd = async () => {
    if (!newRemote.trim()) return;
    const { trackingAdd } = await import("./api");
    try {
      await trackingAdd({
        local_path: projectPath,
        remote_url: newRemote.trim(),
        branch: newBranch,
        enabled: true,
        interval_secs: newInterval * 60,
        last_sync: null,
        last_status: "pending",
        syncing: false,
      });
      setNewRemote("");
      refresh();
    } catch (e) {
      // ignore
    }
  };

  const handleRemove = async (localPath: string) => {
    const { trackingRemove } = await import("./api");
    try {
      await trackingRemove(localPath);
      refresh();
    } catch {
      // ignore
    }
  };

  const handleSync = async (localPath: string) => {
    setSyncingPath(localPath);
    setSyncResult(null);
    const { trackingSyncNow } = await import("./api");
    try {
      const result = await trackingSyncNow(localPath);
      setSyncResult(result.message);
    } catch {
      setSyncResult("Sync failed");
    }
    setSyncingPath(null);
    refresh();
  };

  const handleToggleScheduler = async (on: boolean) => {
    const { trackingStart, trackingStop } = await import("./api");
    try {
      if (on) {
        await trackingStart(300); // 5 minutes default
      } else {
        await trackingStop();
      }
      refreshSchedulerStatus();
    } catch {
      // ignore
    }
  };

  return (
    <div className="tracking-folder-list">
      <div className="tracking-scheduler-row">
        <span className="settings-row-label">
          <span>{tr.activeTracking}</span>
          <span className="settings-row-hint">{tr.activeTrackingHint}</span>
        </span>
        <Switch
          checked={schedulerRunning}
          onChange={handleToggleScheduler}
          statusText={
            <span className={schedulerRunning ? "on" : ""}>
              {schedulerRunning ? tr.cliMcpOn : tr.cliMcpOff}
            </span>
          }
        />
      </div>
      {folders.length === 0 && (
        <p className="tracking-empty">{tr.extensionEmpty}</p>
      )}
      {folders.map((f) => (
        <div key={f.remote_url} className="tracking-folder-row">
          <div className="tracking-folder-info">
            <span className="mono tracking-folder-remote">{f.remote_url}</span>
            <span className="tracking-folder-meta">
              {f.branch} · {f.interval_secs / 60}min
              {f.last_sync && (
                <>
                  {" · "}
                  {tr.activeTrackingLastSync}: {new Date(f.last_sync).toLocaleString()}
                </>
              )}
              {" · "}
              {tr.activeTrackingStatus}:{" "}
              <span className={`tracking-status-${f.last_status}`}>{f.last_status}</span>
            </span>
          </div>
          <div className="tracking-folder-actions">
            <button
              type="button"
              className="ghost-btn"
              onClick={() => handleSync(f.local_path)}
              disabled={syncingPath === f.local_path}
            >
              {syncingPath === f.local_path ? "…" : tr.activeTrackingSyncNow}
            </button>
            <button
              type="button"
              className="ghost-btn"
              onClick={() => handleRemove(f.local_path)}
            >
              ×
            </button>
          </div>
        </div>
      ))}
      {syncResult && (
        <p className="tracking-sync-result">{syncResult}</p>
      )}
      <div className="tracking-add-row">
        <input
          type="text"
          className="settings-input"
          placeholder={tr.activeTrackingRemote}
          value={newRemote}
          onChange={(e) => setNewRemote(e.target.value)}
        />
        <input
          type="text"
          className="settings-input tracking-branch-input"
          placeholder="main"
          value={newBranch}
          onChange={(e) => setNewBranch(e.target.value)}
        />
        <input
          type="number"
          className="settings-input tracking-interval-input"
          placeholder="10"
          value={newInterval}
          onChange={(e) => setNewInterval(Number(e.target.value))}
          min={1}
        />
        <button
          type="button"
          className="ghost-btn"
          onClick={handleAdd}
          disabled={!newRemote.trim()}
        >
          +
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// ExtensionFileList — list files in .route/skills/ or .route/references/
// ---------------------------------------------------------------------------

function ExtensionFileList({
  projectPath,
  subDir,
  tr,
}: {
  projectPath: string;
  subDir: string;
  tr: Dict;
}) {
  const [files, setFiles] = useState<ExtensionEntry[]>([]);

  const refresh = () => {
    const api = subDir === "skills" ? "skillsList" : "referencesList";
    import("./api").then((m) => {
      (m[api] as () => Promise<ExtensionEntry[]>)().then(setFiles).catch(() => {});
    });
  };

  useEffect(() => {
    refresh();
  }, [projectPath, subDir]);

  return (
    <div className="extension-file-list">
      {files.length === 0 ? (
        <p className="tracking-empty">{tr.extensionEmpty}</p>
      ) : (
        files.map((f) => (
          <div key={f.name} className="extension-file-row">
            <span className="mono extension-file-name">{f.name}</span>
            <span className="extension-file-meta">
              {(f.size / 1024).toFixed(1)} KB · {f.modified}
            </span>
          </div>
        ))
      )}
      <p className="extension-path-hint">
        <code>.route/{subDir}/</code>
      </p>
    </div>
  );
}

export function AiConflictDialog({
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
// AiChatDialog — simple chat dialog for talking to the configured AI.
// Shown in the sidebar when active AI mode is enabled.
// ---------------------------------------------------------------------------

export function AiChatDialog({
  open,
  onClose,
  aiApiKey,
  aiApiEndpoint,
  aiProvider,
  aiModel,
  tr,
}: {
  open: boolean;
  onClose: () => void;
  aiApiKey: string;
  aiApiEndpoint: string;
  aiProvider: AiProvider;
  aiModel: string;
  tr: Dict;
}) {
  const [messages, setMessages] = useState<{ role: "user" | "assistant"; text: string }[]>([]);
  const [input, setInput] = useState("");
  const [sending, setSending] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (listRef.current) {
      listRef.current.scrollTop = listRef.current.scrollHeight;
    }
  }, [messages]);

  if (!open) return null;

  const handleSend = async () => {
    const text = input.trim();
    if (!text || sending) return;
    setInput("");
    setMessages((p) => [...p, { role: "user", text }]);
    setSending(true);
    try {
      const chatMessages: ChatMessage[] = [
        { role: "system", content: "You are a helpful assistant for Route, a version management tool. Answer concisely and helpfully." },
        { role: "user", content: text },
      ];
      const reply = await aiChat(aiProvider, aiApiEndpoint, aiApiKey, aiModel, chatMessages);
      setMessages((p) => [...p, { role: "assistant", text: reply }]);
    } catch {
      setMessages((p) => [...p, { role: "assistant", text: "(Error: failed to get response)" }]);
    } finally {
      setSending(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal-card ai-chat-dialog"
        onClick={(e) => e.stopPropagation()}
      >
        <h3>{tr.aiChatTitle}</h3>
        <div className="ai-chat-messages" ref={listRef}>
          {messages.length === 0 && (
            <div className="ai-chat-empty">{tr.aiChatPlaceholder}</div>
          )}
          {messages.map((m, i) => (
            <div key={i} className={`ai-chat-msg ai-chat-msg-${m.role}`}>
              <div className="ai-chat-msg-role">
                {m.role === "user" ? "You" : "AI"}
              </div>
              <div className="ai-chat-msg-text">{m.text}</div>
            </div>
          ))}
          {sending && <div className="ai-chat-msg ai-chat-msg-assistant">
            <div className="ai-chat-msg-role">AI</div>
            <div className="ai-chat-msg-text ai-chat-typing">…</div>
          </div>}
        </div>
        <div className="ai-chat-input-row">
          <input
            className="settings-input"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={tr.aiChatPlaceholder}
            disabled={sending}
          />
          <button
            type="button"
            className="primary"
            onClick={handleSend}
            disabled={sending || !input.trim()}
          >
            {tr.aiChatSend}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// SettingsPage
// ---------------------------------------------------------------------------

export default function SettingsPage({
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
  const dataFolderPath = project.path ? `${project.path}/.route` : "";
  const [pathCopied, setPathCopied] = useState(false);
  const handleCopyPath = async () => {
    if (!dataFolderPath) return;
    try {
      await navigator.clipboard.writeText(dataFolderPath);
      setPathCopied(true);
      setTimeout(() => setPathCopied(false), 1400);
    } catch {
      // ignore
    }
  };

  const [mcpConfig, setMcpConfig] = useState<McpConfigDto | null>(null);
  const [mcpCopied, setMcpCopied] = useState(false);
  const [cliRunning, setCliRunning] = useState(false);
  const [mcpRunning, setMcpRunning] = useState(false);
  const [cliBusy, setCliBusy] = useState(false);
  const [mcpBusy, setMcpBusy] = useState(false);

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
        // Bridge error
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [cliMcpEnabled, project.path]);

  const handleToggleCli = async (on: boolean) => {
    if (cliBusy) return;
    setCliBusy(true);
    try {
      if (on) {
        await startRouteCli();
        setCliRunning(true);
      } else {
        await stopRouteCli();
        setCliRunning(false);
      }
    } catch {
      setCliRunning(false);
    } finally {
      setCliBusy(false);
    }
  };

  const handleToggleMcp = async (on: boolean) => {
    if (mcpBusy) return;
    setMcpBusy(true);
    try {
      if (on) {
        await startRouteMcp(project.path);
        setMcpRunning(true);
      } else {
        await stopRouteMcp();
        setMcpRunning(false);
      }
    } catch {
      setMcpRunning(false);
    } finally {
      setMcpBusy(false);
    }
  };

  const handleCopyMcpConfig = async () => {
    if (!mcpConfig) return;
    try {
      await navigator.clipboard.writeText(mcpConfig.config_snippet);
      setMcpCopied(true);
      setTimeout(() => setMcpCopied(false), 1400);
    } catch {
      // ignore
    }
  };

  const [commitOmitEnabled, setCommitOmitEnabled] = useState(false);
  const [permissionHigh, setPermissionHigh] = useState(false);

  return (
    <div className="card settings-page">
      <h2>{tr.settings}</h2>

      {/* ══ 基础 / Basic ════════════════════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.basicSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* Language */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.languageLabel}</h4>
            <p className="settings-section-hint">{tr.languageHint}</p>
            <div className="seg-control">
              <button
                type="button"
                className={locale === "zh" ? "active" : ""}
                onClick={() => onSetLocale("zh")}
              >
                中文
              </button>
              <button
                type="button"
                className={locale === "en" ? "active" : ""}
                onClick={() => onSetLocale("en")}
              >
                English
              </button>
            </div>
          </div>

          {/* Theme — capsule switch */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.themeSection}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.themeSection}</span>
                <span className="settings-row-hint">{tr.themeSectionHint}</span>
              </div>
              <div className="settings-row-control">
                <ThemeCapsuleSwitch
                  theme={theme}
                  onChange={onSetTheme}
                  tr={tr}
                />
              </div>
            </div>
          </div>

          {/* Startup settings */}
          <AutostartSection tr={tr} />

          {/* Close behavior — capsule buttons */}
          <CloseBehaviorSection tr={tr} />
        </div>
      </section>

      {/* ══ 工作模式 / Work Mode ════════════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.modeSection}</h3>
        </header>
        <div className="settings-category-body">
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.modeSection}</h4>
            <p className="settings-section-hint">{tr.modeSectionHint}</p>
            <div className="seg-control">
              <button
                type="button"
                className={projectMode === "standard" ? "active" : ""}
                onClick={() => onSetProjectMode("standard")}
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
        </div>
      </section>

      {/* ══ AI 辅助 / AI Assistant ══════════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.aiAssistantSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* AI 辅助模式选择 */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.aiAssistantSection}</h4>
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
                    onClick={() => onSetAiAssistantMode("follow")}
                  >
                    {tr.aiAssistantBasic}
                  </button>
                  <button
                    type="button"
                    className={aiAssistantMode === "active" ? "active" : ""}
                    onClick={() => onSetAiAssistantMode("active")}
                  >
                    {tr.aiActiveMode}
                    <span className="beta-badge">{tr.betaBadge}</span>
                  </button>
                </div>
              </div>
            </div>
            <p className="settings-section-aside">
              {aiAssistantMode === "follow" ? tr.aiAssistantBasicHint : tr.aiActiveModeHint}
            </p>
            <div className={`ai-api-config${aiAssistantMode === "active" ? " active" : ""}`}>
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
                      onClick={() => onSetAiProvider("openai")}
                    >
                      {tr.aiProviderOpenai}
                    </button>
                    <button
                      type="button"
                      className={aiProvider === "anthropic" ? "active" : ""}
                      onClick={() => onSetAiProvider("anthropic")}
                    >
                      {tr.aiProviderAnthropic}
                    </button>
                    <button
                      type="button"
                      className={aiProvider === "ollama" ? "active" : ""}
                      onClick={() => onSetAiProvider("ollama")}
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
                    onChange={(e) => onSetAiApiEndpoint(e.target.value)}
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
                    onChange={(e) => onSetAiApiKey(e.target.value)}
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
                    onChange={(e) => onSetAiModel(e.target.value)}
                    placeholder={tr.aiModelPlaceholder}
                  />
                </div>
              </div>
              <div className="ai-test-row">
                <button
                  type="button"
                  className="ghost-btn"
                  onClick={onTestAiConnection}
                  disabled={aiTesting}
                >
                  {aiTesting ? tr.aiTesting : tr.aiTestConnection}
                </button>
                {aiTestResult && (
                  <p className={`settings-section-aside ${aiTestResult.ok ? "ok" : "err"}`}>
                    {aiTestResult.msg}
                  </p>
                )}
              </div>
            </div>
          </div>

          {/* Preset prompt */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.aiPresetPrompt}</h4>
            <p className="settings-section-hint">{tr.aiPresetPromptHint}</p>
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
                    <button type="button" className="ghost-btn" onClick={onOpenAiConflict}>
                      {tr.aiConflictTitle}
                    </button>
                    <button type="button" className="ghost-btn" onClick={onReleaseAiControl}>
                      {tr.masterOff}
                    </button>
                  </div>
                ) : (
                  <button type="button" className="ghost-btn" onClick={onClaimAiControl}>
                    {tr.aiControlActive}
                  </button>
                )}
              </div>
            </div>
            <pre className="ai-prompt-pre">
              {aiPromptLoading ? "…" : aiPrompt || "(empty)"}
            </pre>
          </div>

          {/* Pre-injection files: skills + references */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.preInjectionFiles}</h4>
            <p className="settings-section-hint">{tr.preInjectionFilesHint}</p>
            <div className="pre-injection-grid">
              <div className="pre-injection-col">
                <h5 className="pre-injection-col-title">{tr.skillsSection}</h5>
                <p className="settings-section-hint">{tr.skillsSectionHint}</p>
                <ExtensionFileList
                  projectPath={project.path}
                  subDir="skills"
                  tr={tr}
                />
              </div>
              <div className="pre-injection-col">
                <h5 className="pre-injection-col-title">{tr.referencesSection}</h5>
                <p className="settings-section-hint">{tr.referencesSectionHint}</p>
                <ExtensionFileList
                  projectPath={project.path}
                  subDir="references"
                  tr={tr}
                />
              </div>
            </div>
            <p className="settings-section-aside pre-injection-hint">
              {tr.preInjectionHint}
            </p>
          </div>

          {/* AI index file */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.aiIndexSection}</h4>
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
                <button type="button" className="ghost-btn" onClick={onCopyAiIndex}>
                  {tr.aiIndexCopy}
                </button>
                <button type="button" className="ghost-btn" onClick={onAiIndexRefresh}>
                  {tr.aiIndexRefresh}
                </button>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ══ CLI ══════════════════════════════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>CLI</h3>
        </header>
        <div className="settings-category-body">
          <div className="settings-section">
            <h4 className="settings-section-title">CLI</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.cliStartLabel}</span>
                <span className="settings-row-hint">{tr.cliStartHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={cliRunning}
                  onChange={handleToggleCli}
                  statusText={
                    <span className={cliRunning ? "on" : ""}>
                      {cliRunning ? tr.cliRunning : tr.cliStopped}
                    </span>
                  }
                />
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.cliGuideLabel}</span>
                <span className="settings-row-hint">{tr.cliGuideHint}</span>
              </div>
              <div className="settings-row-control">
                <button
                  type="button"
                  className="ghost-btn"
                  onClick={() => {
                    // Open CLI guide markdown — using system file explorer
                    const guidePath = `${project.path || "."}/.route/cli-guide.md`;
                    window.open(`file://${guidePath}`);
                  }}
                >
                  {tr.cliGuideLabel}
                </button>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ══ MCP ══════════════════════════════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>MCP</h3>
        </header>
        <div className="settings-category-body">
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.mcpConfigLabel}</h4>
            <p className="settings-section-hint">{tr.mcpConfigHint}</p>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.cliMcpEnable}</span>
                <span className="settings-row-hint">{tr.cliMcpEnableHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={cliMcpEnabled}
                  onChange={(checked) => onSetCliMcpEnabled(checked)}
                  statusText={
                    <span className={cliMcpEnabled ? "on" : ""}>
                      {cliMcpEnabled ? tr.cliMcpOn : tr.cliMcpOff}
                    </span>
                  }
                />
              </div>
            </div>
            <p className="settings-section-aside">{tr.cliMcpStartHint}</p>
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
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.mcpStartLabel}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.mcpStartLabel}</span>
                <span className="settings-row-hint">{tr.mcpStartHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={mcpRunning}
                  onChange={handleToggleMcp}
                  statusText={
                    <span className={mcpRunning ? "on" : ""}>
                      {mcpRunning ? tr.mcpRunning : tr.mcpStopped}
                    </span>
                  }
                />
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ══ Git 模式 / Git Mode ═════════════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.gitModeLabel}</h3>
        </header>
        <div className="settings-category-body">
          <div className="settings-section">
            <h4 className="settings-section-title">
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
                  onChange={(checked) => onSetGitModeEnabled(checked)}
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
                    onClick={onGitDetect}
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

      {/* ══ 进阶 / Advanced ═════════════════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.advancedSection}</h3>
        </header>
        <div className="settings-category-body">
          {/* Active tracking */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.activeTracking}</h4>
            <p className="settings-section-hint">{tr.activeTrackingHint}</p>
            <TrackingFolderList
              projectPath={project.path}
              tr={tr}
            />
          </div>

          {/* Commit omit mode */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.commitOmitMode}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.commitOmitMode}</span>
                <span className="settings-row-hint">{tr.commitOmitHint}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={commitOmitEnabled}
                  onChange={setCommitOmitEnabled}
                  statusText={
                    <span className={commitOmitEnabled ? "on" : ""}>
                      {commitOmitEnabled ? tr.commitOmitOn : tr.commitOmitOff}
                    </span>
                  }
                />
              </div>
            </div>
          </div>

          {/* Permission control */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.permissionHint}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.permissionStandard}</span>
                <span className="settings-row-hint">{tr.permissionHint}</span>
              </div>
              <div className="settings-row-control">
                <span className="permission-badge">{tr.permissionStandard}</span>
              </div>
            </div>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.permissionHigh}</span>
                <span className="settings-row-hint">{tr.permissionWarning}</span>
              </div>
              <div className="settings-row-control">
                <Switch
                  checked={permissionHigh}
                  onChange={setPermissionHigh}
                  statusText={
                    <span className={permissionHigh ? "on" : ""}>
                      {permissionHigh ? tr.permissionHigh : tr.permissionStandard}
                    </span>
                  }
                />
              </div>
            </div>
          </div>

          {/* Branch display */}
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.branchDisplay}</h4>
            <div className="settings-row">
              <div className="settings-row-label">
                <span>{tr.branchDisplay}</span>
                <span className="settings-row-hint">{tr.branchDisplayHint}</span>
              </div>
              <div className="settings-row-control">
                <span className="permission-badge">{tr.branchDisplay}</span>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ══ 重置 / Reset ════════════════════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.resetSettings}</h3>
        </header>
        <div className="settings-category-body">
          <DangerZone
            onResetSettings={onResetSettings}
            onClearAllData={onClearAllData}
            tr={tr}
          />
        </div>
      </section>

      {/* ══ 工作台配置 / Workbench Config ═══════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.workbenchConfigSection}</h3>
        </header>
        <div className="settings-category-body">
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.workbenchConfigSection}</h4>
            <p className="settings-section-hint">{tr.cliMcpHint}</p>
            <div className="workbench-config-list">
              {/* S3 config */}
              <div className="workbench-config-item">
                <div className="workbench-config-item-header">
                  <span className="workbench-config-item-label">{tr.workbenchS3}</span>
                </div>
                <div className="workbench-config-item-body">
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Bucket</span>
                    <input className="settings-input" type="text" placeholder="my-bucket" />
                  </div>
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Region</span>
                    <input className="settings-input" type="text" placeholder="us-east-1" />
                  </div>
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Access Key</span>
                    <input className="settings-input" type="password" placeholder="AKIA..." />
                  </div>
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Secret Key</span>
                    <input className="settings-input" type="password" placeholder="..." />
                  </div>
                </div>
              </div>

              {/* WebDAV config */}
              <div className="workbench-config-item">
                <div className="workbench-config-item-header">
                  <span className="workbench-config-item-label">{tr.workbenchWebdav}</span>
                </div>
                <div className="workbench-config-item-body">
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">URL</span>
                    <input className="settings-input" type="text" placeholder="https://example.com/dav" />
                  </div>
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Username</span>
                    <input className="settings-input" type="text" placeholder="user" />
                  </div>
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Password</span>
                    <input className="settings-input" type="password" placeholder="••••••••" />
                  </div>
                </div>
              </div>

              {/* SSH config */}
              <div className="workbench-config-item">
                <div className="workbench-config-item-header">
                  <span className="workbench-config-item-label">{tr.workbenchSSH}</span>
                </div>
                <div className="workbench-config-item-body">
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Host</span>
                    <input className="settings-input" type="text" placeholder="192.168.1.100" />
                  </div>
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Port</span>
                    <input className="settings-input" type="text" placeholder="22" />
                  </div>
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Username</span>
                    <input className="settings-input" type="text" placeholder="root" />
                  </div>
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">Key Path</span>
                    <input className="settings-input" type="text" placeholder="~/.ssh/id_rsa" />
                  </div>
                </div>
              </div>

              {/* Mirror delay */}
              <div className="workbench-config-item">
                <div className="workbench-config-item-header">
                  <span className="workbench-config-item-label">{tr.workbenchMirrorDelay}</span>
                </div>
                <div className="workbench-config-item-body">
                  <div className="workbench-config-field">
                    <span className="workbench-config-field-label">{tr.workbenchMirrorDelay}</span>
                    <input className="settings-input" type="number" min={0} placeholder="0" />
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* ══ 配置与数据 / Config & Data ═══════════════════════════════════ */}
      <section className="settings-category">
        <header className="settings-category-header">
          <h3>{tr.dataSection}</h3>
        </header>
        <div className="settings-category-body">
          <div className="settings-section">
            <h4 className="settings-section-title">{tr.dataLocationLabel}</h4>
            <p className="settings-section-hint">{tr.dataLocationHint}</p>
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
        </div>
      </section>
    </div>
  );
}