import fs from 'node:fs/promises';
import path from 'node:path';
import fg from 'fast-glob';
import { hashContent, hashFilePath } from '../utils/hash.js';
import { RoutePaths, ensureDir, exists } from './paths.js';

export interface ProjectScanResult {
  files: Record<string, string>;
  /** relative path → absolute path */
  absolutePaths: Record<string, string>;
}

export async function scanProject(
  projectPath: string,
  ignorePatterns: string[],
): Promise<ProjectScanResult> {
  const entries = await fg('**/*', {
    cwd: projectPath,
    ignore: ignorePatterns,
    dot: false,
    onlyFiles: true,
    absolute: false,
  });

  const files: Record<string, string> = {};
  const absolutePaths: Record<string, string> = {};

  for (const rel of entries) {
    const normalized = hashFilePath(rel);
    const abs = path.join(projectPath, rel);
    const stat = await fs.stat(abs);
    if (!stat.isFile()) continue;
    const content = await fs.readFile(abs);
    files[normalized] = hashContent(content);
    absolutePaths[normalized] = abs;
  }

  return { files, absolutePaths };
}

export async function storeBlob(
  paths: RoutePaths,
  content: Buffer,
): Promise<string> {
  const hash = hashContent(content);
  const blobPath = paths.blobPath(hash);
  if (await exists(blobPath)) return hash;
  await ensureDir(path.dirname(blobPath));
  await fs.writeFile(blobPath, content);
  return hash;
}

export async function readBlob(paths: RoutePaths, hash: string): Promise<Buffer> {
  return fs.readFile(paths.blobPath(hash));
}

export async function storeChangedBlobs(
  paths: RoutePaths,
  projectPath: string,
  changedPaths: string[],
): Promise<void> {
  for (const rel of changedPaths) {
    const abs = path.join(projectPath, rel);
    if (!(await exists(abs))) continue;
    const content = await fs.readFile(abs);
    await storeBlob(paths, content);
  }
}

export async function applyManifestToProject(
  paths: RoutePaths,
  projectPath: string,
  manifestFiles: Record<string, string>,
  previousFiles?: Record<string, string>,
): Promise<void> {
  const prev = previousFiles ?? {};

  for (const rel of Object.keys(prev)) {
    if (!(rel in manifestFiles)) {
      const abs = path.join(projectPath, rel);
      await fs.unlink(abs).catch(() => undefined);
    }
  }

  for (const [rel, hash] of Object.entries(manifestFiles)) {
    const content = await readBlob(paths, hash);
    const abs = path.join(projectPath, rel);
    await ensureDir(path.dirname(abs));
    await fs.writeFile(abs, content);
  }
}

export function diffManifests(
  previous: Record<string, string>,
  current: Record<string, string>,
): { added: string[]; modified: string[]; removed: string[] } {
  const added: string[] = [];
  const modified: string[] = [];
  const removed: string[] = [];

  for (const rel of Object.keys(current)) {
    if (!(rel in previous)) added.push(rel);
    else if (previous[rel] !== current[rel]) modified.push(rel);
  }

  for (const rel of Object.keys(previous)) {
    if (!(rel in current)) removed.push(rel);
  }

  return { added, modified, removed };
}
