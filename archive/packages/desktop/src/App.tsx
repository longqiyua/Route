import { useCallback, useEffect, useState } from 'react';
import {
  branchCreate,
  branchDelete,
  branchMerge,
  branchPause,
  branchResume,
  branchSwitch,
  commitTurn,
  getHistory,
  getInfo,
  initRepo,
  isInitialized,
  pickProjectFolder,
  rollback,
  type RouteInfo,
  type TurnEntry,
} from './api/route';

export default function App() {
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [info, setInfo] = useState<RouteInfo | null>(null);
  const [history, setHistory] = useState<TurnEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [userMessage, setUserMessage] = useState('');
  const [aiSummary, setAiSummary] = useState('');
  const [start, setStart] = useState('');
  const [end, setEnd] = useState('');
  const [acceptance, setAcceptance] = useState('');
  const [emergency, setEmergency] = useState('');
  const [newBranch, setNewBranch] = useState('');

  const refresh = useCallback(async (path: string) => {
    const initialized = await isInitialized(path);
    if (!initialized) {
      await initRepo(path);
    }
    const [repoInfo, turns] = await Promise.all([getInfo(path), getHistory(path)]);
    setInfo(repoInfo);
    setHistory(turns);
  }, []);

  const run = useCallback(
    async (fn: () => Promise<void>) => {
      if (!projectPath) return;
      setLoading(true);
      setError(null);
      try {
        await fn();
        await refresh(projectPath);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setLoading(false);
      }
    },
    [projectPath, refresh],
  );

  const handlePickFolder = async () => {
    setError(null);
    const path = await pickProjectFolder();
    if (!path) return;
    setProjectPath(path);
    setLoading(true);
    try {
      await refresh(path);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const saved = localStorage.getItem('route:lastProject');
    if (saved) {
      setProjectPath(saved);
      refresh(saved).catch(() => undefined);
    }
  }, [refresh]);

  useEffect(() => {
    if (projectPath) localStorage.setItem('route:lastProject', projectPath);
  }, [projectPath]);

  if (!projectPath) {
    return (
      <div className="empty-state" style={{ minHeight: '100vh', display: 'grid', placeContent: 'center' }}>
        <h1>
          <span style={{ color: 'var(--accent)' }}>Route</span> — Web Coding 版本管理
        </h1>
        <p>选择一个项目文件夹，Route 会在其中创建 <code>.route/</code> 目录来追踪对话与备份。</p>
        <button className="primary" onClick={handlePickFolder}>
          选择项目文件夹
        </button>
        {error && <p className="error-banner" style={{ marginTop: '1rem' }}>{error}</p>}
      </div>
    );
  }

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="logo">
          <span>Route</span>
        </div>
        <div className="project-path">{projectPath}</div>

        <div>
          <div className="section-title">分支</div>
          <div className="branch-list">
            {info?.branches.map((b) => (
              <div
                key={b.name}
                className={`branch-item ${b.name === info.currentBranch.name ? 'active' : ''}`}
              >
                <button
                  style={{ background: 'none', border: 'none', padding: 0, textAlign: 'left' }}
                  onClick={() => run(() => branchSwitch(projectPath, b.name))}
                  disabled={loading}
                >
                  {b.name === info?.currentBranch.name ? '• ' : ''}{b.name}
                </button>
                <span className={`status ${b.status}`}>{b.status}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="inline-form">
          <input
            placeholder="新分支名"
            value={newBranch}
            onChange={(e) => setNewBranch(e.target.value)}
          />
          <button
            disabled={!newBranch || loading}
            onClick={() =>
              run(async () => {
                await branchCreate(projectPath, newBranch);
                setNewBranch('');
              })
            }
          >
            创建
          </button>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.35rem' }}>
          <button disabled={loading} onClick={() => run(() => branchPause(projectPath, info!.currentBranch.name))}>
            暂停当前分支
          </button>
          <button disabled={loading} onClick={() => run(() => branchResume(projectPath, info!.currentBranch.name))}>
            恢复当前分支
          </button>
          <button
            disabled={loading}
            onClick={() => {
              const source = prompt('要并入当前分支的源分支名：');
              if (source) run(() => branchMerge(projectPath, source, 'new-wins'));
            }}
          >
            并入分支（新分支优先）
          </button>
          <button
            className="danger"
            disabled={loading}
            onClick={() => {
              const name = prompt('要删除的分支名：');
              if (name) run(() => branchDelete(projectPath, name));
            }}
          >
            删除分支
          </button>
        </div>

        <button onClick={handlePickFolder} disabled={loading}>
          切换项目
        </button>
      </aside>

      <main className="main">
        {error && <div className="error-banner">{error}</div>}

        <div className="toolbar stats">
          <span>当前分支：<strong>{info?.currentBranch.name}</strong> ({info?.currentBranch.status})</span>
          <span>对话轮次：<strong>{info?.config.turnCount ?? 0}</strong></span>
        </div>

        <div className="card">
          <h2>记录对话轮次（备份）</h2>
          <p style={{ color: 'var(--muted)', fontSize: '0.85rem', marginTop: 0 }}>
            一个指令干一个活儿 — 声明开始点、结束点、验收点、紧急回退点
          </p>
          <div className="form-grid">
            <label>
              用户指令
              <textarea value={userMessage} onChange={(e) => setUserMessage(e.target.value)} placeholder="例如：给首页加一个 Hero 区域" />
            </label>
            <label>
              AI 修改摘要
              <textarea value={aiSummary} onChange={(e) => setAiSummary(e.target.value)} placeholder="例如：在 index.html 添加了 hero section" />
            </label>
            <div className="points-grid">
              <label>开始点<input value={start} onChange={(e) => setStart(e.target.value)} placeholder="turn id 或描述" /></label>
              <label>结束点<input value={end} onChange={(e) => setEnd(e.target.value)} /></label>
              <label>验收点<input value={acceptance} onChange={(e) => setAcceptance(e.target.value)} /></label>
              <label>紧急回退点<input value={emergency} onChange={(e) => setEmergency(e.target.value)} placeholder="turn id" /></label>
            </div>
            <button
              className="primary"
              disabled={loading || !userMessage || !aiSummary || info?.currentBranch.status === 'paused'}
              onClick={() =>
                run(async () => {
                  await commitTurn(projectPath, { userMessage, aiSummary, start, end, acceptance, emergency });
                  setUserMessage('');
                  setAiSummary('');
                })
              }
            >
              提交备份
            </button>
          </div>
        </div>

        <div className="card">
          <h2>历史追踪</h2>
          <div className="history-list">
            {history.length === 0 && <p style={{ color: 'var(--muted)' }}>暂无记录，提交第一条对话轮次吧。</p>}
            {history.map((t) => (
              <div key={t.id} className="turn-card">
                <div className="turn-header">
                  <span className="turn-id">{t.shortId}</span>
                  <span>{new Date(t.timestamp).toLocaleString()} · {t.branch} · {t.snapshotType}</span>
                </div>
                <div><strong>用户：</strong>{t.userMessage}</div>
                <div><strong>AI：</strong>{t.aiSummary}</div>
                {t.instruction && (
                  <div style={{ fontSize: '0.8rem', color: 'var(--muted)', marginTop: '0.35rem' }}>
                    四点：{t.instruction.start} → {t.instruction.end} | 验收 {t.instruction.acceptance} | 紧急 {t.instruction.emergencyRollback}
                  </div>
                )}
                <div className="turn-actions">
                  <button disabled={loading} onClick={() => run(() => rollback(projectPath, t.id))}>
                    回退到此
                  </button>
                  <button className="danger" disabled={loading} onClick={() => run(() => rollback(projectPath, t.id, true))}>
                    紧急回退
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      </main>
    </div>
  );
}
