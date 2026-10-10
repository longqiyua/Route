import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { RouteRepository } from './repository.js';

async function createTempProject(): Promise<string> {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'route-test-'));
  await fs.writeFile(path.join(dir, 'index.html'), '<html></html>', 'utf8');
  return dir;
}

describe('RouteRepository', () => {
  let projectPath: string;

  beforeEach(async () => {
    projectPath = await createTempProject();
  });

  afterEach(async () => {
    await fs.rm(projectPath, { recursive: true, force: true });
  });

  it('initializes a repository', async () => {
    const repo = new RouteRepository(projectPath);
    const info = await repo.init();
    assert.equal(info.currentBranch.name, 'main');
    assert.equal(info.config.turnCount, 0);
  });

  it('commits and tracks conversation turns', async () => {
    const repo = new RouteRepository(projectPath);
    await repo.init();

    const turn = await repo.commitTurn({
      userMessage: 'Add hero section',
      aiSummary: 'Created hero section in index.html',
    });

    assert.ok(turn.id);
    assert.equal(turn.userMessage, 'Add hero section');

    const history = await repo.getHistory();
    assert.equal(history.length, 1);
  });

  it('supports branch create, pause, resume', async () => {
    const repo = new RouteRepository(projectPath);
    await repo.init();
    await repo.commitTurn({ userMessage: 'A', aiSummary: 'Did A' });

    const feature = await repo.createBranch('feature');
    assert.equal(feature.name, 'feature');

    await repo.pauseBranch('feature');
    const paused = await repo.getBranch('feature');
    assert.equal(paused.status, 'paused');

    await repo.resumeBranch('feature');
    const active = await repo.getBranch('feature');
    assert.equal(active.status, 'active');
  });

  it('rolls back to a previous turn', async () => {
    const repo = new RouteRepository(projectPath);
    await repo.init();

    await repo.commitTurn({ userMessage: 'A', aiSummary: 'Version A' });
    await fs.writeFile(path.join(projectPath, 'index.html'), '<html>A</html>', 'utf8');
    const turnB = await repo.commitTurn({ userMessage: 'B', aiSummary: 'Version B' });

    const history = await repo.getHistory();
    const turnA = history[1];

    await repo.rollbackToTurn(turnA.id);
    const content = await fs.readFile(path.join(projectPath, 'index.html'), 'utf8');
    assert.equal(content, '<html></html>');
  });

  it('merges with new-wins strategy', async () => {
    const repo = new RouteRepository(projectPath);
    await repo.init();

    await repo.commitTurn({ userMessage: 'main init', aiSummary: 'main' });
    await repo.createBranch('feature');
    await repo.switchBranch('feature');
    await fs.writeFile(path.join(projectPath, 'feature.txt'), 'new', 'utf8');
    await repo.commitTurn({ userMessage: 'feature work', aiSummary: 'feature' });

    await repo.switchBranch('main');
    const mergeTurn = await repo.mergeBranch('feature', 'main');
    assert.ok(mergeTurn.id);

    const exists = await fs
      .access(path.join(projectPath, 'feature.txt'))
      .then(() => true)
      .catch(() => false);
    assert.equal(exists, true);
  });
});
