import type { CommitTurnInput, InstructionPoints } from '../types.js';

/** Base interface for conversation source adapters */
export interface ConversationAdapter {
  readonly id: string;
  readonly name: string;
  readonly description: string;

  /** Whether this adapter can run in current environment */
  isAvailable(): Promise<boolean>;

  /** Poll or read latest conversation turn from external tool */
  pollLatest?(): Promise<Partial<CommitTurnInput> | null>;

  /** Watch for new turns (optional, for desktop) */
  watch?(onTurn: (input: Partial<CommitTurnInput>) => void): () => void;
}

/** Manual adapter — user/AI submits via CLI or desktop UI */
export class ManualAdapter implements ConversationAdapter {
  readonly id = 'manual';
  readonly name = 'Manual Commit';
  readonly description = 'Submit conversation turns via CLI or desktop UI';

  async isAvailable(): Promise<boolean> {
    return true;
  }
}

/** Placeholder for Cursor / Codex / OpenCode log ingestion */
export class FileWatchAdapter implements ConversationAdapter {
  readonly id: string;
  readonly name: string;
  readonly description: string;

  constructor(
    id: string,
    name: string,
    watchPath: string,
  ) {
    this.id = id;
    this.name = name;
    this.description = `Watch conversation logs at ${watchPath}`;
  }

  async isAvailable(): Promise<boolean> {
    return false; // enabled when path exists and adapter is configured
  }
}

export function parseInstructionPoints(raw: {
  start?: string;
  end?: string;
  acceptance?: string;
  emergencyRollback?: string;
}): InstructionPoints | undefined {
  if (!raw.start && !raw.end && !raw.acceptance && !raw.emergencyRollback) {
    return undefined;
  }
  return {
    start: raw.start ?? '',
    end: raw.end ?? '',
    acceptance: raw.acceptance ?? '',
    emergencyRollback: raw.emergencyRollback ?? '',
  };
}

export const BUILTIN_ADAPTERS: ConversationAdapter[] = [
  new ManualAdapter(),
  new FileWatchAdapter('cursor', 'Cursor Chat', '~/.cursor/'),
  new FileWatchAdapter('codex', 'Codex TUI', '~/.codex/'),
  new FileWatchAdapter('opencode', 'OpenCode', '~/.opencode/'),
];
