import { invoke } from '@tauri-apps/api/core';

export interface RouteInfo {
  config: {
    projectPath: string;
    currentBranch: string;
    turnCount: number;
  };
  branches: Array<{
    name: string;
    status: 'active' | 'paused';
    headTurnId: string | null;
  }>;
  currentBranch: {
    name: string;
    status: 'active' | 'paused';
  };
}

export interface TurnEntry {
  id: string;
  shortId: string;
  timestamp: number;
  branch: string;
  userMessage: string;
  aiSummary: string;
  changedFiles: string[];
  snapshotType: string;
  instruction?: {
    start: string;
    end: string;
    acceptance: string;
    emergencyRollback: string;
  };
}

export async function pickProjectFolder(): Promise<string | null> {
  return invoke<string | null>('pick_folder');
}

export async function routeCall<T>(
  projectPath: string,
  method: string,
  params: Record<string, unknown> = {},
): Promise<T> {
  return invoke<T>('route_call', {
    projectPath,
    method,
    params,
  });
}

export async function isInitialized(projectPath: string): Promise<boolean> {
  return routeCall<boolean>(projectPath, 'isInitialized', { projectPath });
}

export async function initRepo(projectPath: string): Promise<RouteInfo> {
  return routeCall<RouteInfo>(projectPath, 'init', { projectPath });
}

export async function getInfo(projectPath: string): Promise<RouteInfo> {
  return routeCall<RouteInfo>(projectPath, 'info', { projectPath });
}

export async function getHistory(projectPath: string): Promise<TurnEntry[]> {
  return routeCall<TurnEntry[]>(projectPath, 'history', { projectPath, limit: 100 });
}

export async function commitTurn(
  projectPath: string,
  input: {
    userMessage: string;
    aiSummary: string;
    start?: string;
    end?: string;
    acceptance?: string;
    emergency?: string;
  },
): Promise<TurnEntry> {
  return routeCall<TurnEntry>(projectPath, 'commit', { projectPath, ...input });
}

export async function rollback(
  projectPath: string,
  turnId: string,
  emergency = false,
): Promise<void> {
  await routeCall(projectPath, 'rollback', { projectPath, turnId, emergency });
}

export async function branchCreate(projectPath: string, name: string): Promise<void> {
  await routeCall(projectPath, 'branch.create', { projectPath, name });
}

export async function branchDelete(projectPath: string, name: string): Promise<void> {
  await routeCall(projectPath, 'branch.delete', { projectPath, name });
}

export async function branchSwitch(projectPath: string, name: string): Promise<void> {
  await routeCall(projectPath, 'branch.switch', { projectPath, name });
}

export async function branchPause(projectPath: string, name: string): Promise<void> {
  await routeCall(projectPath, 'branch.pause', { projectPath, name });
}

export async function branchResume(projectPath: string, name: string): Promise<void> {
  await routeCall(projectPath, 'branch.resume', { projectPath, name });
}

export async function branchMerge(
  projectPath: string,
  source: string,
  strategy: 'new-wins' | 'old-wins' = 'new-wins',
): Promise<void> {
  await routeCall(projectPath, 'branch.merge', { projectPath, source, strategy });
}
