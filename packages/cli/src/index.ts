#!/usr/bin/env node
import { Command } from 'commander';
import path from 'node:path';
import {
  RouteRepository,
  parseInstructionPoints,
  shortId,
} from '@route/core';

const program = new Command();

program
  .name('route')
  .description('Route — lightweight version manager for Web Coding')
  .version('0.1.0');

function resolveProject(cwd?: string): RouteRepository {
  return new RouteRepository(path.resolve(cwd ?? process.cwd()));
}

program
  .command('init')
  .description('Initialize Route in current folder')
  .option('-b, --branch <name>', 'initial branch name', 'main')
  .action(async (opts) => {
    const repo = resolveProject();
    const info = await repo.init({ branchName: opts.branch });
    console.log(`✓ Route initialized on branch "${info.currentBranch.name}"`);
    console.log(`  Metadata: ${path.join(repo.projectPath, '.route')}`);
  });

program
  .command('status')
  .description('Show repository status')
  .action(async () => {
    const repo = resolveProject();
    const info = await repo.getInfo();
    const history = await repo.getHistory(undefined, 1);
    console.log(`Project:  ${info.config.projectPath}`);
    console.log(`Branch:   ${info.currentBranch.name} (${info.currentBranch.status})`);
    console.log(`Turns:    ${info.config.turnCount}`);
    if (history[0]) {
      console.log(`Latest:   [${shortId(history[0].id)}] ${history[0].userMessage.slice(0, 60)}`);
    }
    console.log('\nBranches:');
    for (const b of info.branches) {
      const marker = b.name === info.currentBranch.name ? '*' : ' ';
      console.log(`  ${marker} ${b.name} (${b.status})`);
    }
  });

program
  .command('commit')
  .description('Record a conversation turn (backup)')
  .requiredOption('-u, --user <message>', 'user instruction / prompt')
  .requiredOption('-a, --ai <summary>', 'AI action summary')
  .option('--start <point>', 'instruction start point')
  .option('--end <point>', 'instruction end point')
  .option('--acceptance <point>', 'acceptance checkpoint')
  .option('--emergency <point>', 'emergency rollback point')
  .option('--adapter <id>', 'source adapter id', 'manual')
  .option('--full', 'force full snapshot')
  .action(async (opts) => {
    const repo = await RouteRepository.open(process.cwd());
    const instruction = parseInstructionPoints({
      start: opts.start,
      end: opts.end,
      acceptance: opts.acceptance,
      emergencyRollback: opts.emergency,
    });
    const turn = await repo.commitTurn({
      userMessage: opts.user,
      aiSummary: opts.ai,
      instruction,
      adapter: opts.adapter,
      forceFullSnapshot: opts.full,
    });
    console.log(`✓ Turn [${shortId(turn.id)}] committed on branch "${turn.branch}"`);
    console.log(`  Changed: ${turn.changedFiles.length} file(s)`);
    console.log(`  Snapshot: ${turn.snapshotType}`);
  });

program
  .command('log')
  .description('Show conversation turn history')
  .option('-n, --limit <n>', 'max entries', '20')
  .option('-b, --branch <name>', 'branch name')
  .action(async (opts) => {
    const repo = await RouteRepository.open(process.cwd());
    const history = await repo.getHistory(opts.branch, Number(opts.limit));
    for (const t of history) {
      const date = new Date(t.timestamp).toLocaleString();
      console.log(`[${t.shortId}] ${date} (${t.branch})`);
      console.log(`  User: ${t.userMessage}`);
      console.log(`  AI:   ${t.aiSummary}`);
      if (t.instruction) {
        console.log(`  Points: start=${t.instruction.start} end=${t.instruction.end}`);
      }
      console.log('');
    }
  });

program
  .command('rollback <turnId>')
  .description('Rollback to a conversation turn')
  .option('--emergency', 'record emergency rollback turn')
  .option('-r, --reason <text>', 'rollback reason')
  .action(async (turnId, opts) => {
    const repo = await RouteRepository.open(process.cwd());
    if (opts.emergency) {
      await repo.emergencyRollback({ turnId, reason: opts.reason ?? 'Emergency' });
    } else {
      await repo.rollbackToTurn(turnId);
    }
    console.log(`✓ Rolled back to turn ${turnId.slice(0, 8)}`);
  });

const branch = program.command('branch').description('Manage branches');

branch
  .command('list')
  .description('List branches')
  .action(async () => {
    const repo = await RouteRepository.open(process.cwd());
    const current = await repo.getCurrentBranchName();
    const branches = await repo.listBranches();
    for (const b of branches) {
      const marker = b.name === current ? '*' : ' ';
      console.log(`${marker} ${b.name} (${b.status}) head=${b.headTurnId ? shortId(b.headTurnId) : 'none'}`);
    }
  });

branch
  .command('create <name>')
  .description('Create a new branch')
  .option('--from <branch>', 'source branch')
  .action(async (name, opts) => {
    const repo = await RouteRepository.open(process.cwd());
    const b = await repo.createBranch(name, opts.from);
    console.log(`✓ Branch "${b.name}" created`);
  });

branch
  .command('delete <name>')
  .description('Delete a branch')
  .action(async (name) => {
    const repo = await RouteRepository.open(process.cwd());
    await repo.deleteBranch(name);
    console.log(`✓ Branch "${name}" deleted`);
  });

branch
  .command('switch <name>')
  .description('Switch current branch')
  .action(async (name) => {
    const repo = await RouteRepository.open(process.cwd());
    await repo.switchBranch(name);
    console.log(`✓ Switched to branch "${name}"`);
  });

branch
  .command('pause <name>')
  .description('Pause a branch (no new commits)')
  .action(async (name) => {
    const repo = await RouteRepository.open(process.cwd());
    await repo.pauseBranch(name);
    console.log(`✓ Branch "${name}" paused`);
  });

branch
  .command('resume <name>')
  .description('Resume a paused branch')
  .action(async (name) => {
    const repo = await RouteRepository.open(process.cwd());
    await repo.resumeBranch(name);
    console.log(`✓ Branch "${name}" resumed`);
  });

branch
  .command('merge <source>')
  .description('Merge source branch into current (default: new branch wins)')
  .option('--into <target>', 'target branch')
  .option('--strategy <s>', 'new-wins | old-wins', 'new-wins')
  .option('-m, --message <text>', 'merge message')
  .action(async (source, opts) => {
    const repo = await RouteRepository.open(process.cwd());
    const turn = await repo.mergeBranch(source, opts.into, {
      strategy: opts.strategy,
      message: opts.message,
    });
    console.log(`✓ Merged "${source}" → turn [${shortId(turn.id)}]`);
  });

program.parseAsync(process.argv);
