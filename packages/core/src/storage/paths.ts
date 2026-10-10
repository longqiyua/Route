import fs from 'node:fs/promises';
import path from 'node:path';
import { ROUTE_DIR } from '../types.js';

export class RoutePaths {
  constructor(public readonly projectPath: string) {}

  get routeDir(): string {
    return path.join(this.projectPath, ROUTE_DIR);
  }

  get configPath(): string {
    return path.join(this.routeDir, 'config.json');
  }

  get headPath(): string {
    return path.join(this.routeDir, 'HEAD');
  }

  branchesDir(): string {
    return path.join(this.routeDir, 'refs', 'branches');
  }

  branchPath(name: string): string {
    return path.join(this.branchesDir(), name);
  }

  turnsDir(): string {
    return path.join(this.routeDir, 'turns');
  }

  turnPath(id: string): string {
    return path.join(this.turnsDir(), `${id}.json`);
  }

  manifestsDir(): string {
    return path.join(this.routeDir, 'objects', 'manifests');
  }

  manifestPath(id: string): string {
    return path.join(this.manifestsDir(), `${id}.json`);
  }

  blobsDir(): string {
    return path.join(this.routeDir, 'objects', 'blobs');
  }

  blobPath(hash: string): string {
    const prefix = hash.slice(0, 2);
    return path.join(this.blobsDir(), prefix, hash);
  }

  adaptersDir(): string {
    return path.join(this.routeDir, 'adapters');
  }

  adapterPath(id: string): string {
    return path.join(this.adaptersDir(), `${id}.json`);
  }
}

export async function ensureDir(dir: string): Promise<void> {
  await fs.mkdir(dir, { recursive: true });
}

export async function readJson<T>(filePath: string): Promise<T> {
  const raw = await fs.readFile(filePath, 'utf8');
  return JSON.parse(raw) as T;
}

export async function writeJson(filePath: string, data: unknown): Promise<void> {
  await ensureDir(path.dirname(filePath));
  await fs.writeFile(filePath, JSON.stringify(data, null, 2), 'utf8');
}

export async function exists(filePath: string): Promise<boolean> {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

export async function readText(filePath: string): Promise<string> {
  return fs.readFile(filePath, 'utf8');
}

export async function writeText(filePath: string, content: string): Promise<void> {
  await ensureDir(path.dirname(filePath));
  await fs.writeFile(filePath, content, 'utf8');
}

export async function copyFileSafe(src: string, dest: string): Promise<void> {
  await ensureDir(path.dirname(dest));
  await fs.copyFile(src, dest);
}

export async function removeFileSafe(filePath: string): Promise<void> {
  try {
    await fs.unlink(filePath);
  } catch {
    // ignore missing files
  }
}
