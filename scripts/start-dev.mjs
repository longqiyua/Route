// scripts/start-dev.mjs
//
// Dev entry point for the desktop app. Run with `npm run tauri:dev`
// instead of `cargo tauri dev` to get automatic port fallback.
//
// Why a wrapper is necessary
// --------------------------
// Tauri 2 reads `tauri.conf.json` *once*, before any beforeDevCommand
// runs. The `devUrl` baked into that initial read is the one the
// Tauri webview uses to load the frontend, and there is no env-var
// override for it. So the only way to coordinate "Vite picked port
// 1421 because 1420 was busy" with "Tauri must load 1421 too" is to
// update the JSON file *before* Tauri's CLI opens it. That's what
// this script does, in this order:
//
//   1. Probe a free port in PREFERRED_PORT..PREFERRED_PORT+30 using
//      `node:net`. The probe runs in parallel so a wide range
//      doesn't slow startup.
//   2. Update `crates/route-tauri/tauri.conf.json` so `build.devUrl`
//      points at the chosen port. The write is idempotent — running
//      twice with the same port is a no-op and doesn't churn the
//      file's mtime.
//   3. Spawn `npm run dev` (Vite) in the `web/` directory with
//      `ROUTE_DEV_PORT` set, so Vite binds the same port. We then
//      wait for Vite to actually accept connections — otherwise
//      Tauri would race ahead and try to load the URL before
//      anything is listening.
//   4. Spawn `cargo tauri dev`. We forward stdio so the user sees
//      the Tauri output directly, and we trap SIGINT/SIGTERM so a
//      Ctrl-C in the wrapper kills both Vite and the Tauri child
//      cleanly.
//
// If any of steps 1-3 fails, we exit non-zero *before* spawning
// Tauri, so the user gets a clear error rather than a half-broken
// app that the webview can't reach.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, "..");
const CONFIG_PATH = resolve(ROOT, "crates/route-tauri/tauri.conf.json");
const WEB_DIR = resolve(ROOT, "crates/route-tauri/web");
const PREFERRED_PORT = 1420;
const FALLBACK_RANGE = 30;

async function findFreePort(start, count) {
  const candidates = [];
  for (let i = 0; i < count; i++) candidates.push(start + i);
  const probes = candidates.map(
    (port) =>
      new Promise((resolveProbe) => {
        const srv = createServer();
        srv.once("error", () => {
          srv.close();
          resolveProbe(null);
        });
        srv.once("listening", () => {
          srv.close(() => resolveProbe(port));
        });
        srv.listen(port, "0.0.0.0");
      })
  );
  for (const r of probes) {
    const v = await r;
    if (v !== null) return v;
  }
  throw new Error(
    `No free port found in range ${start}..${start + count - 1}.`
  );
}

function updateTauriConfig(port) {
  if (!existsSync(CONFIG_PATH)) {
    throw new Error(`tauri.conf.json not found at ${CONFIG_PATH}`);
  }
  const raw = readFileSync(CONFIG_PATH, "utf8");
  const cfg = JSON.parse(raw);
  const next = `http://localhost:${port}`;
  if (cfg?.build?.devUrl === next) {
    // Already matches — leave the file alone so the timestamp doesn't
    // churn and downstream `cargo tauri dev` doesn't think we touched
    // anything.
    return next;
  }
  cfg.build = cfg.build || {};
  cfg.build.devUrl = next;
  const pretty = JSON.stringify(cfg, null, 2) + "\n";
  writeFileSync(CONFIG_PATH, pretty, "utf8");
  return next;
}

/// Wait until `host:port` is actually serving HTTP responses, not
/// just accepting TCP connections. Vite takes a moment after `listen`
/// to finish compiling its server module graph; a connection that
/// arrives during that window would race ahead of the Tauri webview
/// and get aborted (`net::ERR_ABORTED` in devtools). We issue real
/// GET / requests in a tight loop until one returns 200 (or any
/// successful response — Vite always returns 200 for `/`), and bail
/// out with a clear error if the server doesn't come up in time.
///
/// Beyond just `/`, we also touch the entry module (`/src/main.tsx`).
/// This warms Vite's dependency optimizer so the webview's first
/// navigation never has to wait for esbuild to finish walking the
/// import graph. Without this warm-up, the webview would issue a
/// request that Vite can't answer for several seconds (during which
/// time the user might reload), and the request would be aborted —
/// which is exactly the `net::ERR_ABORTED http://localhost:1420/`
/// message users see in devtools.
function waitForHttpReady(port, timeoutMs = 60000) {
  const start = Date.now();
  return new Promise((resolveReady, reject) => {
    const probe = async (path) => {
      try {
        const res = await fetch(`http://127.0.0.1:${port}${path}`, {
          redirect: "manual",
          cache: "no-store",
        });
        // Drain the body so Vite's connection slot is freed and the
        // request is fully considered "done" by the dev server.
        try {
          await res.arrayBuffer();
        } catch {
          /* body drain is best-effort */
        }
        // 2xx and 3xx are both fine. 4xx means the URL exists but
        // Vite isn't ready (e.g. /favicon.ico during the very first
        // tick); treat those as "not ready yet" and let the caller
        // retry.
        if (res.status >= 200 && res.status < 400) return true;
        if (res.status === 404 && path === "/") return false;
        return res.status < 500;
      } catch {
        return false;
      }
    };
    const attempt = async () => {
      // Phase 1: index page must respond. This is the minimum the
      // Tauri webview needs to start rendering.
      const rootOk = await probe("/");
      if (!rootOk) {
        if (Date.now() - start > timeoutMs) {
          reject(
            new Error(
              `Vite did not become ready on port ${port} within ${timeoutMs}ms`
            )
          );
          return;
        }
        setTimeout(attempt, 100);
        return;
      }
      // Phase 2: warm up the entry module so the dependency optimizer
      // finishes its first walk. If this fails we don't bail out —
      // the page is still usable, it just won't have the warm cache.
      try {
        await probe("/src/main.tsx");
      } catch {
        /* non-fatal: see comment above */
      }
      resolveReady();
    };
    attempt();
  });
}

/// Kill a child process AND its entire descendant tree.
///
/// Why this exists: Vite is spawned with `shell: true` (line below),
/// which on Windows means Node spawns `cmd.exe /c npm run dev`, and
/// `cmd.exe` then launches `node.exe` (the real Vite) as its own
/// child. `child.kill("SIGTERM")` only terminates `cmd.exe`; the
/// `node.exe` grandchild survives as an orphan and keeps holding the
/// dev port. The next `tauri:dev` then fails with
/// "Port 1420 is already in use" — exactly the "app won't start"
/// symptom. `taskkill /T` recursively kills the whole tree so the
/// port is released cleanly on exit.
function killTree(child) {
  if (!child || child.exitCode !== null) return; // already exited
  const pid = child.pid;
  if (!pid) return;
  try {
    if (process.platform === "win32") {
      // /T = kill the process tree (all descendants), /F = force.
      spawnSync("taskkill", ["/F", "/T", "/PID", String(pid)], {
        stdio: "ignore",
        shell: false,
      });
    } else {
      // Try the process group first (POSIX), then fall back to the
      // direct pid so we don't leave stragglers either way.
      try { process.kill(-pid, "SIGTERM"); } catch { /* not a group leader */ }
      try { child.kill("SIGTERM"); } catch { /* already gone */ }
    }
  } catch { /* best-effort cleanup */ }
}

function main() {
  let vite = null;
  let tauri = null;
  const cleanup = () => {
    try { if (tauri && !tauri.killed) tauri.kill("SIGTERM"); } catch { /* ignore */ }
    try { killTree(vite); } catch { /* ignore */ }
  };
  process.on("SIGINT", () => { cleanup(); process.exit(130); });
  process.on("SIGTERM", () => { cleanup(); process.exit(143); });

  findFreePort(PREFERRED_PORT, FALLBACK_RANGE)
    .then(async (port) => {
      const url = updateTauriConfig(port);
      console.log(`[route-dev] devUrl → ${url}`);

      // Spawn Vite first, with the agreed port in env so its own
      // probe can short-circuit. We keep this process's stdio so the
      // user sees Vite output, and we wait for the port before
      // continuing.
      const childEnv = { ...process.env, ROUTE_DEV_PORT: String(port) };
      vite = spawn("npm", ["run", "dev"], {
        cwd: WEB_DIR,
        stdio: "inherit",
        env: childEnv,
        shell: true,
      });
      vite.on("exit", (code) => {
        // If Vite dies on its own (not because we killed it), we
        // can't continue — the Tauri webview would be loading a
        // dead URL. Exit with the same code.
        if (code !== null && code !== 0 && tauri && !tauri.killed) {
          try { tauri.kill("SIGTERM"); } catch { /* ignore */ }
        }
        if (code !== null) process.exit(code);
      });

      try {
        await waitForHttpReady(port, 60000);
      } catch (err) {
        console.error(`[route-dev] ${err.message}`);
        cleanup();
        process.exit(1);
      }

      // Now it's safe to start Tauri.
      tauri = spawn(
        "cargo",
        ["tauri", "dev", "--no-watch"],
        {
          cwd: ROOT,
          stdio: "inherit",
          env: childEnv,
        }
      );
      tauri.on("exit", (code) => {
        try { killTree(vite); } catch { /* ignore */ }
        process.exit(code ?? 0);
      });
    })
    .catch((err) => {
      console.error("[route-dev] failed to start:", err.message);
      cleanup();
      process.exit(1);
    });
}

main();
