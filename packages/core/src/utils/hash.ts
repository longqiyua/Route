import { createHash, randomUUID } from 'node:crypto';

export function hashContent(content: Buffer | string): string {
  const buf = typeof content === 'string' ? Buffer.from(content) : content;
  return createHash('sha256').update(buf).digest('hex');
}

export function hashFilePath(relativePath: string): string {
  return relativePath.replace(/\\/g, '/');
}

export function shortId(fullId: string, length = 8): string {
  return fullId.slice(0, length);
}

export function newId(): string {
  return randomUUID();
}

export function now(): number {
  return Date.now();
}
