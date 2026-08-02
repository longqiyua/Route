import fs from 'node:fs/promises';
import path from 'node:path';
import {
  Branch,
  CommitTurnInput,
  ConversationTurn,
  DEFAULT_FULL_SNAPSHOT_INTERVAL,
  DEFAULT_IGNORE_PATTERNS,
  EmergencyRollbackTrigger,
  MergeOptions,
  RouteConfig,
  RouteRepositoryInfo,
  SCHEMA_VERSION,
  SnapshotManifest,
  TurnHistoryEntry,
} from './types.js';
import { newId, now, shortId } from './utils/hash.js';
import {
  RoutePaths,
  ensureDir,
  exists,
  readJson,
  readText,
  removeFileSafe,
  writeJson,
  writeText,
} from './storage/paths.js';
import {
  applyManifestToProject,
  diffManifests,
  scanProject,
  storeChangedBlobs,
} from './storage/snapshot.js';

export class RouteRepository {
  readonly paths: RoutePaths;

  constructor(public readonly projectPath: string) {
    this.paths = new RoutePaths(path.resolve(projectPath));
  }

  static async open(projectPath: string): Promise<RouteRepository> {
    const repo = new RouteRepository(projectPath);
    if (!(await repo.isInitialized())) {
      throw new Error(`Not a Route repository: ${repo.paths.routeDir}`);
    }
    return repo;
  }

  async isInitialized(): Promise<boolean> {
    return exists(this.paths.configPath);
  }

  async init(options?: {
    branchName?: string;
    ignorePatterns?: string[];
    fullSnapshotInterval?: number;
  }): Promise<RouteRepositoryInfo> {
    if (await this.isInitialized()) {
      throw new Error('Route repository already initialized');
    }

    const branchName = options?.branchName ?? 'main';
    const config: RouteConfig = {
      version: SCHEMA_VERSION,
      projectPath: this.paths.projectPath,
      currentBranch: branchName,
      fullSnapshotInterval:
        options?.fullSnapshotInterval ?? DEFAULT_FULL_SNAPSHOT_INTERVAL,
      ignorePatterns: options?.ignorePatterns ?? [...DEFAULT_IGNORE_PATTERNS],
      turnCount: 0,
      createdAt: now(),
    };

    await ensureDir(this.paths.routeDir);
    await ensureDir(this.paths.branchesDir());
    await ensureDir(this.paths.turnsDir());
    await ensureDir(this.paths.manifestsDir());
    await ensureDir(this.paths.blobsDir());
    await ensureDir(this.paths.adaptersDir());

    const branch: Branch = {
      name: branchName,
      headTurnId: null,
      status: 'active',
      createdAt: now(),
    };

    await writeJson(this.paths.configPath, config);
    await writeJson(this.paths.branchPath(branchName), branch);
    await writeText(this.paths.headPath, branchName);

    return this.getInfo();
  }

  async getConfig(): Promise<RouteConfig> {
    return readJson<RouteConfig>(this.paths.configPath);
  }

  async saveConfig(config: RouteConfig): Promise<void> {
    await writeJson(this.paths.configPath, config);
  }

  async getBranch(name: string): Promise<Branch> {
    const branchPath = this.paths.branchPath(name);
    if (!(await exists(branchPath))) {
      throw new Error(`Branch not found: ${name}`);
    }
    return readJson<Branch>(branchPath);
  }

  async saveBranch(branch: Branch): Promise<void> {
    await writeJson(this.paths.branchPath(branch.name), branch);
  }

  async listBranches(): Promise<Branch[]> {
    const dir = this.paths.branchesDir();
    if (!(await exists(dir))) return [];
    const names = await fs.readdir(dir);
    return Promise.all(names.map((name) => this.getBranch(name)));
  }

  async getCurrentBranchName(): Promise<string> {
    return readText(this.paths.headPath).then((s: string) => s.trim());
  }

  async switchBranch(name: string): Promise<void> {
    await this.getBranch(name);
    const config = await this.getConfig();
    config.currentBranch = name;
    await this.saveConfig(config);
    await writeText(this.paths.headPath, name);
  }

  async getInfo(): Promise<RouteRepositoryInfo> {
    const config = await this.getConfig();
    const branches = await this.listBranches();
    const currentBranch = await this.getBranch(config.currentBranch);
    return { config, branches, currentBranch };
  }

  async getTurn(id: string): Promise<ConversationTurn> {
    return readJson<ConversationTurn>(this.paths.turnPath(id));
  }

  async saveTurn(turn: ConversationTurn): Promise<void> {
    await writeJson(this.paths.turnPath(turn.id), turn);
  }

  async getManifest(id: string): Promise<SnapshotManifest> {
    return readJson<SnapshotManifest>(this.paths.manifestPath(id));
  }

  async saveManifest(manifest: SnapshotManifest): Promise<void> {
    await writeJson(this.paths.manifestPath(manifest.id), manifest);
  }

  /** Resolve full file manifest by walking incremental chain */
  async resolveManifest(manifestId: string): Promise<Record<string, string>> {
    const manifest = await this.getManifest(manifestId);
    if (manifest.type === 'full' || !manifest.parentManifestId) {
      return { ...manifest.files };
    }
    const parent = await this.resolveManifest(manifest.parentManifestId);
    return { ...parent, ...manifest.files };
  }

  async getHeadManifest(branchName?: string): Promise<SnapshotManifest | null> {
    const branch = await this.getBranch(branchName ?? (await this.getCurrentBranchName()));
    if (!branch.headTurnId) return null;
    const turn = await this.getTurn(branch.headTurnId);
    return this.getManifest(turn.snapshotId);
  }

  /**
   * Backup current project state as a conversation turn (minimum version unit).
   */
  async commitTurn(input: CommitTurnInput): Promise<ConversationTurn> {
    const config = await this.getConfig();
    const branchName = config.currentBranch;
    const branch = await this.getBranch(branchName);

    if (branch.status === 'paused') {
      throw new Error(
        `Branch "${branchName}" is paused. Resume it before committing.`,
      );
    }

    const scan = await scanProject(this.paths.projectPath, config.ignorePatterns);
    const headManifest = await this.getHeadManifest(branchName);
    const previousFiles = headManifest
      ? await this.resolveManifest(headManifest.id)
      : {};

    const diff = diffManifests(previousFiles, scan.files);
    const changedFiles = [...diff.added, ...diff.modified, ...diff.removed];

    const shouldFull =
      input.forceFullSnapshot === true ||
      !headManifest ||
      config.turnCount % config.fullSnapshotInterval === 0;

    const snapshotType = shouldFull ? 'full' : 'incremental';
    const manifestId = newId();
    const turnId = newId();

    let manifestFiles: Record<string, string>;
    let parentManifestId: string | null = null;

    if (snapshotType === 'full') {
      manifestFiles = { ...scan.files };
      await storeChangedBlobs(
        this.paths,
        this.paths.projectPath,
        Object.keys(scan.files),
      );
    } else {
      manifestFiles = {};
      for (const rel of [...diff.added, ...diff.modified]) {
        manifestFiles[rel] = scan.files[rel];
      }
      for (const rel of diff.removed) {
        manifestFiles[rel] = '';
      }
      parentManifestId = headManifest!.id;
      await storeChangedBlobs(
        this.paths,
        this.paths.projectPath,
        [...diff.added, ...diff.modified],
      );
    }

    const manifest: SnapshotManifest = {
      id: manifestId,
      turnId,
      type: snapshotType,
      parentManifestId,
      files: manifestFiles,
      timestamp: now(),
    };

    const turn: ConversationTurn = {
      id: turnId,
      timestamp: now(),
      branch: branchName,
      parentId: branch.headTurnId,
      userMessage: input.userMessage,
      aiSummary: input.aiSummary,
      instruction: input.instruction,
      snapshotId: manifestId,
      snapshotType,
      changedFiles,
      adapter: input.adapter ?? 'manual',
    };

    await this.saveManifest(manifest);
    await this.saveTurn(turn);

    branch.headTurnId = turnId;
    await this.saveBranch(branch);

    config.turnCount += 1;
    await this.saveConfig(config);

    if (input.instruction?.emergencyRollback) {
      await this.checkEmergencyRollback(turn, input.instruction.emergencyRollback);
    }

    return turn;
  }

  /** Check if emergency rollback should trigger based on instruction metadata */
  private async checkEmergencyRollback(
    turn: ConversationTurn,
    emergencyPoint: string,
  ): Promise<void> {
    // Emergency point can be a turn id prefix or keyword in user message
    const triggerKeywords = ['rollback', '回退', '紧急', 'emergency'];
    const shouldTrigger = triggerKeywords.some(
      (kw) =>
        turn.userMessage.toLowerCase().includes(kw) ||
        emergencyPoint.toLowerCase().includes(kw),
    );
    if (shouldTrigger && emergencyPoint.length >= 8) {
      try {
        await this.rollbackToTurn(emergencyPoint.slice(0, 36));
      } catch {
        // emergency point may be descriptive, not an id
      }
    }
  }

  /** Trigger explicit emergency rollback */
  async emergencyRollback(trigger: EmergencyRollbackTrigger): Promise<ConversationTurn> {
    return this.rollbackToTurn(trigger.turnId, {
      reason: trigger.reason,
      emergency: true,
    });
  }

  /** Rollback project files to a specific conversation turn */
  async rollbackToTurn(
    turnId: string,
    options?: { reason?: string; emergency?: boolean },
  ): Promise<ConversationTurn> {
    const targetTurn = await this.findTurn(turnId);
    const resolved = await this.resolveManifest(targetTurn.snapshotId);

    const headManifest = await this.getHeadManifest(targetTurn.branch);
    const currentFiles = headManifest
      ? await this.resolveManifest(headManifest.id)
      : {};

    await applyManifestToProject(
      this.paths,
      this.paths.projectPath,
      resolved,
      currentFiles,
    );

    const branch = await this.getBranch(targetTurn.branch);
    branch.headTurnId = targetTurn.id;
    await this.saveBranch(branch);

    if (options?.emergency) {
      const rollbackTurn = await this.commitTurn({
        userMessage: `[EMERGENCY ROLLBACK] ${options.reason ?? 'Safety rollback'}`,
        aiSummary: `Rolled back to turn ${shortId(targetTurn.id)} on branch ${targetTurn.branch}`,
        adapter: 'system',
        forceFullSnapshot: true,
      });
      return rollbackTurn;
    }

    return targetTurn;
  }

  async findTurn(partialId: string): Promise<ConversationTurn> {
    const dir = this.paths.turnsDir();
    const files = await fs.readdir(dir);
    const match = files.find((f) => f.startsWith(partialId) || f === `${partialId}.json`);
    if (!match) {
      throw new Error(`Turn not found: ${partialId}`);
    }
    return readJson<ConversationTurn>(path.join(dir, match));
  }

  async getHistory(branchName?: string, limit = 50): Promise<TurnHistoryEntry[]> {
    const branch = await this.getBranch(branchName ?? (await this.getCurrentBranchName()));
    const entries: TurnHistoryEntry[] = [];
    let currentId = branch.headTurnId;

    while (currentId && entries.length < limit) {
      const turn = await this.getTurn(currentId);
      entries.push({ ...turn, shortId: shortId(turn.id) });
      currentId = turn.parentId;
    }

    return entries;
  }

  async createBranch(name: string, fromBranch?: string): Promise<Branch> {
    const sourceName = fromBranch ?? (await this.getCurrentBranchName());
    const source = await this.getBranch(sourceName);

    if (await exists(this.paths.branchPath(name))) {
      throw new Error(`Branch already exists: ${name}`);
    }

    const branch: Branch = {
      name,
      headTurnId: source.headTurnId,
      status: 'active',
      createdAt: now(),
    };

    await this.saveBranch(branch);
    return branch;
  }

  async deleteBranch(name: string): Promise<void> {
    const config = await this.getConfig();
    if (name === config.currentBranch) {
      throw new Error('Cannot delete the current branch. Switch first.');
    }
    const branches = await this.listBranches();
    if (branches.length <= 1) {
      throw new Error('Cannot delete the only branch.');
    }
    await removeFileSafe(this.paths.branchPath(name));
  }

  async pauseBranch(name: string): Promise<Branch> {
    const branch = await this.getBranch(name);
    branch.status = 'paused';
    await this.saveBranch(branch);
    return branch;
  }

  async resumeBranch(name: string): Promise<Branch> {
    const branch = await this.getBranch(name);
    branch.status = 'active';
    await this.saveBranch(branch);
    return branch;
  }

  /**
   * Merge source branch into target branch.
   * Default strategy: new branch (source) wins on file conflicts.
   */
  async mergeBranch(
    sourceBranchName: string,
    targetBranchName?: string,
    options?: MergeOptions,
  ): Promise<ConversationTurn> {
    const strategy = options?.strategy ?? 'new-wins';
    const targetName = targetBranchName ?? (await this.getCurrentBranchName());

    const source = await this.getBranch(sourceBranchName);
    const target = await this.getBranch(targetName);

    if (!source.headTurnId) {
      throw new Error(`Source branch "${sourceBranchName}" has no commits.`);
    }

    const sourceTurn = await this.getTurn(source.headTurnId);
    const sourceFiles = await this.resolveManifest(sourceTurn.snapshotId);

    const targetHead = target.headTurnId
      ? await this.getTurn(target.headTurnId)
      : null;
    const targetFiles = targetHead
      ? await this.resolveManifest(targetHead.snapshotId)
      : {};

    let mergedFiles: Record<string, string>;

    switch (strategy) {
      case 'new-wins':
        mergedFiles = { ...targetFiles, ...sourceFiles };
        break;
      case 'old-wins':
        mergedFiles = { ...sourceFiles, ...targetFiles };
        break;
      case 'ask':
        throw new Error(
          'Merge strategy "ask" requires UI interaction. Use desktop app or specify strategy explicitly.',
        );
      default:
        mergedFiles = { ...targetFiles, ...sourceFiles };
    }

    await this.switchBranch(targetName);

    const headManifest = await this.getHeadManifest(targetName);
    const currentProject = await scanProject(
      this.paths.projectPath,
      (await this.getConfig()).ignorePatterns,
    );

    await applyManifestToProject(
      this.paths,
      this.paths.projectPath,
      mergedFiles,
      currentProject.files,
    );

    const turn = await this.commitTurn({
      userMessage: options?.message ?? `Merge branch '${sourceBranchName}' into '${targetName}'`,
      aiSummary: `Merged with strategy "${strategy}". Source: ${sourceBranchName}, target: ${targetName}`,
      adapter: 'system',
      forceFullSnapshot: true,
    });

    const updatedTarget = await this.getBranch(targetName);
    updatedTarget.mergedFrom = sourceBranchName;
    await this.saveBranch(updatedTarget);

    return turn;
  }
}
