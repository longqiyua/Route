import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { writeFileSync, existsSync, readFileSync, unlinkSync } from "node:fs";
import { resolve } from "node:path";

/**
 * Default Vite port. We try this first; if it's busy we walk a small
 * range (1420..1450) until we find a free one. The chosen port is
 * written to `.dev-port` so the Tauri side can read it and configure
 * its `devUrl` to match.
 */
const PREFERRED_PORT = 1420;
const FALLBACK_RANGE = 30;
const PORT_FILE = resolve(__dirname, ".dev-port");

/**
 * Probe a list of TCP ports and return the first one that is free
 * for listening. Runs in parallel so the search is fast even when
 * a handful of ports are held.
 */
async function findFreePort(start: number, count: number): Promise<number> {
  const net = await import("node:net");
  const candidates: number[] = [];
  for (let i = 0; i < count; i++) candidates.push(start + i);

  const probes = candidates.map(
    (port) =>
      new Promise<number | null>((resolveProbe) => {
        const srv = net.createServer();
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
  return start; // fall through; Vite will fail loudly
}

/**
 * Plugin: write the port the dev server actually bound to into
 * `.dev-port` so external tools (Tauri, the `dev-web` wrapper) can
 * read it. The file is created on `listen` and removed on `close`
 * so stale values from a previous crashed run don't survive.
 */
function devPortFile(): Plugin {
  return {
    name: "route-dev-port-file",
    apply: "serve",
    configureServer(server) {
      // Clean up any stale file from a previous run. We write the
      // new value only AFTER the server actually binds, so a half-
      // written value can never point at a port that isn't ours.
      if (existsSync(PORT_FILE)) {
        try { unlinkSync(PORT_FILE); } catch { /* ignore */ }
      }
      server.httpServer?.once("listening", () => {
        const addr = server.httpServer?.address();
        const port =
          typeof addr === "object" && addr ? addr.port : PREFERRED_PORT;
        try {
          writeFileSync(PORT_FILE, String(port), "utf8");
        } catch {
          /* non-fatal — the Tauri side falls back to 1420 if missing */
        }
      });
      server.httpServer?.once("close", () => {
        try { unlinkSync(PORT_FILE); } catch { /* ignore */ }
      });
    },
  };
}

export default defineConfig(async () => {
  // Two ways the port gets picked:
  //
  //   (a) The user started Vite via `npm run dev` (no Tauri host).
  //       In that case we probe the network ourselves and bind the
  //       first free port. This is the standalone-dev case.
  //
  //   (b) The user started Vite via `npm run tauri:dev` (or the
  //       `scripts/start-dev.mjs` wrapper). The wrapper probed the
  //       network already and wrote the chosen port into the
  //       `ROUTE_DEV_PORT` environment variable, so we MUST use that
  //       exact port — otherwise Vite and Tauri would race to bind
  //       different ports and the webview would 404 against
  //       `devUrl`.
  //
  // We honour the env var first; if it's missing we fall back to
  // the probe. This keeps the standalone case working while
  // guaranteeing the wrapper case stays in lock-step.
  const envPort = Number(process.env.ROUTE_DEV_PORT);
  const port = Number.isFinite(envPort) && envPort > 0
    ? envPort
    : await findFreePort(PREFERRED_PORT, FALLBACK_RANGE);

  return {
    plugins: [react(), devPortFile()],
    clearScreen: false,
    server: {
      port,
      strictPort: true,
      // Listen on every interface so both IPv4 (127.0.0.1) and IPv6
      // ([::1]) clients can connect. Without this, Vite on Windows can
      // bind only to IPv6, which then makes the Tauri WebView2 — which
      // tries 127.0.0.1 first — fail with ERR_CONNECTION_REFUSED and
      // show a black screen.
      host: true,
    },
    build: {
      target: "es2020",
      outDir: "dist",
    },
  };
});

/**
 * Re-export so the dev wrapper can reuse the same probe logic.
 * Reads the port from the file written by `devPortFile`, falling
 * back to `PREFERRED_PORT` if the file is missing (e.g. the dev
 * server hasn't finished binding yet).
 */
export function readDevPort(): number {
  try {
    if (existsSync(PORT_FILE)) {
      const raw = readFileSync(PORT_FILE, "utf8").trim();
      const n = Number(raw);
      if (Number.isFinite(n) && n > 0) return n;
    }
  } catch { /* ignore */ }
  return PREFERRED_PORT;
}
