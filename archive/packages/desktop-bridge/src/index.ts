import readline from 'node:readline';
import { RouteRepository, parseInstructionPoints } from '@route/core';

interface BridgeRequest {
  id: string;
  method: string;
  params?: Record<string, unknown>;
}

interface BridgeResponse {
  id: string;
  ok: boolean;
  data?: unknown;
  error?: string;
}

async function handle(req: BridgeRequest): Promise<unknown> {
  const projectPath = req.params?.projectPath as string | undefined;
  if (!projectPath && req.method !== 'ping') {
    throw new Error('projectPath is required');
  }

  switch (req.method) {
    case 'ping':
      return { pong: true };

    case 'init': {
      const repo = new RouteRepository(projectPath!);
      return repo.init({
        branchName: req.params?.branchName as string | undefined,
      });
    }

    case 'info': {
      const repo = await RouteRepository.open(projectPath!);
      return repo.getInfo();
    }

    case 'isInitialized': {
      const repo = new RouteRepository(projectPath!);
      return repo.isInitialized();
    }

    case 'commit': {
      const repo = await RouteRepository.open(projectPath!);
      const instruction = parseInstructionPoints({
        start: req.params?.start as string | undefined,
        end: req.params?.end as string | undefined,
        acceptance: req.params?.acceptance as string | undefined,
        emergencyRollback: req.params?.emergency as string | undefined,
      });
      return repo.commitTurn({
        userMessage: req.params?.userMessage as string,
        aiSummary: req.params?.aiSummary as string,
        instruction,
        adapter: (req.params?.adapter as string) ?? 'manual',
        forceFullSnapshot: req.params?.forceFull as boolean | undefined,
      });
    }

    case 'history': {
      const repo = await RouteRepository.open(projectPath!);
      return repo.getHistory(
        req.params?.branch as string | undefined,
        (req.params?.limit as number) ?? 50,
      );
    }

    case 'rollback': {
      const repo = await RouteRepository.open(projectPath!);
      if (req.params?.emergency) {
        return repo.emergencyRollback({
          turnId: req.params?.turnId as string,
          reason: (req.params?.reason as string) ?? 'Emergency',
        });
      }
      return repo.rollbackToTurn(req.params?.turnId as string);
    }

    case 'branch.create':
      return (await RouteRepository.open(projectPath!)).createBranch(
        req.params?.name as string,
        req.params?.from as string | undefined,
      );

    case 'branch.delete':
      await (await RouteRepository.open(projectPath!)).deleteBranch(
        req.params?.name as string,
      );
      return { deleted: req.params?.name };

    case 'branch.switch':
      await (await RouteRepository.open(projectPath!)).switchBranch(
        req.params?.name as string,
      );
      return { switched: req.params?.name };

    case 'branch.pause':
      return (await RouteRepository.open(projectPath!)).pauseBranch(
        req.params?.name as string,
      );

    case 'branch.resume':
      return (await RouteRepository.open(projectPath!)).resumeBranch(
        req.params?.name as string,
      );

    case 'branch.merge':
      return (await RouteRepository.open(projectPath!)).mergeBranch(
        req.params?.source as string,
        req.params?.target as string | undefined,
        {
          strategy: req.params?.strategy as 'new-wins' | 'old-wins' | undefined,
          message: req.params?.message as string | undefined,
        },
      );

    default:
      throw new Error(`Unknown method: ${req.method}`);
  }
}

function respond(res: BridgeResponse): void {
  process.stdout.write(JSON.stringify(res) + '\n');
}

const rl = readline.createInterface({ input: process.stdin });

rl.on('line', async (line) => {
  let req: BridgeRequest;
  try {
    req = JSON.parse(line) as BridgeRequest;
  } catch {
    respond({ id: 'parse-error', ok: false, error: 'Invalid JSON' });
    return;
  }

  try {
    const data = await handle(req);
    respond({ id: req.id, ok: true, data });
  } catch (err) {
    respond({
      id: req.id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
});
