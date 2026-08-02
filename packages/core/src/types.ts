/** Route metadata directory name (like `.git`) */
export const ROUTE_DIR = '.route';

/** Current schema version */
export const SCHEMA_VERSION = 1;

/** Default full snapshot interval (every N turns) */
export const DEFAULT_FULL_SNAPSHOT_INTERVAL = 10;

/** Default ignore patterns */
export const DEFAULT_IGNORE_PATTERNS = [
  '.route/**',
  'node_modules/**',
  '.git/**',
  'dist/**',
  'build/**',
  '.DS_Store',
  'Thumbs.db',
  '*.log',
];

/** One instruction = one job, with four declared points */
export interface InstructionPoints {
  /** 开始点 — turn id or human-readable marker */
  start: string;
  /** 结束点 */
  end: string;
  /** 验收点 */
  acceptance: string;
  /** 紧急回退点 — rollback here immediately on trigger */
  emergencyRollback: string;
}

/** Minimum version unit: one user-AI conversation turn */
export interface ConversationTurn {
  id: string;
  timestamp: number;
  branch: string;
  parentId: string | null;
  /** User's instruction / prompt */
  userMessage: string;
  /** AI action summary (what was changed) */
  aiSummary: string;
  /** Optional structured instruction metadata */
  instruction?: InstructionPoints;
  /** Linked snapshot manifest id */
  snapshotId: string;
  snapshotType: 'incremental' | 'full';
  changedFiles: string[];
  /** Source adapter id, e.g. "manual", "cursor", "codex" */
  adapter: string;
}

/** File manifest at a snapshot point: relative path → content hash */
export interface SnapshotManifest {
  id: string;
  turnId: string;
  type: 'incremental' | 'full';
  parentManifestId: string | null;
  files: Record<string, string>;
  timestamp: number;
}

export type BranchStatus = 'active' | 'paused';

export interface Branch {
  name: string;
  headTurnId: string | null;
  status: BranchStatus;
  createdAt: number;
  /** If merged from another branch */
  mergedFrom?: string;
}

export interface RouteConfig {
  version: typeof SCHEMA_VERSION;
  projectPath: string;
  currentBranch: string;
  fullSnapshotInterval: number;
  ignorePatterns: string[];
  turnCount: number;
  createdAt: number;
}

export interface RouteRepositoryInfo {
  config: RouteConfig;
  branches: Branch[];
  currentBranch: Branch;
}

export type MergeStrategy = 'new-wins' | 'old-wins' | 'ask';

export interface MergeOptions {
  strategy?: MergeStrategy;
  message?: string;
}

export interface CommitTurnInput {
  userMessage: string;
  aiSummary: string;
  instruction?: InstructionPoints;
  adapter?: string;
  forceFullSnapshot?: boolean;
}

export interface TurnHistoryEntry extends ConversationTurn {
  shortId: string;
}

export interface AdapterInfo {
  id: string;
  name: string;
  description: string;
  enabled: boolean;
}

export interface EmergencyRollbackTrigger {
  turnId: string;
  reason: string;
}
