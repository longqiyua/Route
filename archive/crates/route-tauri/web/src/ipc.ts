// IPC bridge wrapper.
//
// The Tauri 2 IPC bridge (`window.__TAURI_INTERNALS__`) is injected
// into the webview by the host process. In a normal Tauri runtime it
// is available *immediately* when the page first paints, but there
// are a handful of real-world conditions where the bridge is briefly
// absent:
//
//   - The page is opened in a regular browser (debugging with
//     `npm run dev` + opening `http://localhost:1420/` manually).
//   - The webview is mid-reload (HMR push) when a `useEffect`
//     fires and calls `invoke`.
//   - The Tauri host is still warming up after `cargo tauri dev`
//     recompiled the Rust binary.
//
// In all of those cases calling `invoke` synchronously throws
// `TypeError: Cannot read properties of undefined (reading 'invoke')`
// — the `__TAURI_INTERNALS__.invoke` lookup fails because the
// internals object hasn't been installed yet.
//
// We can't rewrite the upstream package, but we *can* wrap every
// call. This module exports a `safeInvoke` that:
//
//   1. Polls for the bridge to appear (up to 5 s).
//   2. Caches the resolved `invoke` function so subsequent calls are
//      synchronous and zero-cost.
//   3. Throws a stable, user-readable error (`route: tauri bridge
//      unavailable`) if the bridge never appears, instead of the
//      cryptic "Cannot read properties of undefined".
//
// All `api.ts` entry points go through `safeInvoke`, so the rest of
// the app sees one consistent error contract.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";

const POLL_INTERVAL_MS = 25;
const READY_TIMEOUT_MS = 5000;

type AnyArgs = Record<string, unknown>;

let cached: typeof tauriInvoke | null = null;
let pending: Promise<typeof tauriInvoke> | null = null;

function readBridge(): typeof tauriInvoke | null {
  const w = typeof window === "undefined" ? null : (window as unknown as { __TAURI_INTERNALS__?: { invoke?: typeof tauriInvoke } });
  const fn = w?.__TAURI_INTERNALS__?.invoke;
  return typeof fn === "function" ? fn : null;
}

async function waitForBridge(): Promise<typeof tauriInvoke> {
  if (cached) return cached;
  if (pending) return pending;
  pending = (async () => {
    const start = Date.now();
    // First, an immediate probe so the happy path is zero-delay.
    const direct = readBridge();
    if (direct) {
      cached = direct;
      return direct;
    }
    // Then a polling loop, in case the bridge appears a moment after
    // the page first paints (HMR reload, just-rebuilt Tauri host,
    // etc.). We cap at READY_TIMEOUT_MS so a misconfigured dev
    // session surfaces a clear error instead of hanging forever.
    while (Date.now() - start < READY_TIMEOUT_MS) {
      await new Promise<void>((r) => setTimeout(r, POLL_INTERVAL_MS));
      const v = readBridge();
      if (v) {
        cached = v;
        return v;
      }
    }
    throw new Error(
      "route: tauri bridge unavailable (run via `cargo tauri dev` or the installed app)",
    );
  })();
  try {
    return await pending;
  } finally {
    pending = null;
  }
}

/**
 * Like `@tauri-apps/api/core::invoke`, but waits for the Tauri IPC
 * bridge to become available and throws a clear, stable error if
 * the page is being run outside the Tauri host (e.g. opened in a
 * regular browser for debugging).
 */
export async function safeInvoke<T = unknown>(
  cmd: string,
  args?: AnyArgs,
): Promise<T> {
  const fn = await waitForBridge();
  return fn<T>(cmd, args);
}

/** Test-only: forget the cached bridge. Used by the smoke harness
 * to simulate a Tauri-cold start. */
export function __resetInvokeCacheForTest(): void {
  cached = null;
  pending = null;
}

/** Diagnostic: is the bridge currently available? Used by the
 * startup splash so the UI can show a "Connecting…" hint instead of
 * a confusing TypeError when the bridge hasn't appeared yet. */
export function isTauriBridgeAvailable(): boolean {
  return readBridge() !== null;
}
