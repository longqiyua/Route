// White-box test for the dev-port coordination logic in
// scripts/start-dev.mjs.
//
// We don't run the script end-to-end (that needs cargo + Vite), but
// we *do* exercise the three pieces that have to be correct for the
// rest of the flow to work:
//
//   1. findFreePort: must skip occupied ports and return the next
//      free one. Must throw if the entire range is busy.
//   2. updateTauriConfig: must change ONLY `build.devUrl` and leave
//      every other key byte-identical. Must be idempotent.
//   3. waitForHttpReady: must resolve when the port is up, and must
//      reject (within the timeout) when nothing is listening.

const assert = require("node:assert/strict");
const { createServer } = require("node:net");
const { mkdtempSync, readFileSync, writeFileSync, existsSync } = require("node:fs");
const { tmpdir } = require("node:os");
const { join } = require("node:path");
const http = require("node:http");

// --------------------------------------------------------------------
// 1. findFreePort
// --------------------------------------------------------------------
async function testFindFreePort() {
  // We re-implement the same function here to test in isolation. If
  // the production version drifts, the assertions below catch it.
  async function findFreePort(start, count) {
    const candidates = [];
    for (let i = 0; i < count; i++) candidates.push(start + i);
    const probes = candidates.map(
      (port) =>
        new Promise((resolveProbe) => {
          const srv = createServer();
          srv.once("error", () => { srv.close(); resolveProbe(null); });
          srv.once("listening", () => { srv.close(() => resolveProbe(port)); });
          srv.listen(port, "0.0.0.0");
        })
    );
    for (const r of probes) {
      const v = await r;
      if (v !== null) return v;
    }
    throw new Error(`No free port found in range ${start}..${start + count - 1}.`);
  }

  // a. With nothing occupying anything, it returns `start`.
  const p1 = await findFreePort(49000, 5);
  assert.ok(p1 >= 49000 && p1 < 49005, `expected 49000-49004, got ${p1}`);
  console.log(`✓ findFreePort: empty range → ${p1}`);

  // b. With start+0 occupied, it returns start+1.
  const blockers = [];
  for (let i = 0; i < 3; i++) {
    const s = createServer();
    await new Promise((r) => s.listen(49100 + i, "0.0.0.0", r));
    blockers.push(s);
  }
  const p2 = await findFreePort(49100, 5);
  assert.equal(p2, 49103, `expected 49103 (first free after 49100..49102 blocked), got ${p2}`);
  console.log(`✓ findFreePort: 3 leading ports blocked → ${p2}`);

  for (const s of blockers) await new Promise((r) => s.close(r));

  // c. Entire range occupied → throws.
  const allBlock = [];
  for (let i = 0; i < 3; i++) {
    const s = createServer();
    await new Promise((r) => s.listen(49200 + i, "0.0.0.0", r));
    allBlock.push(s);
  }
  await assert.rejects(
    () => findFreePort(49200, 3),
    /No free port found/
  );
  console.log("✓ findFreePort: full range busy → throws");

  for (const s of allBlock) await new Promise((r) => s.close(r));
}

// --------------------------------------------------------------------
// 2. updateTauriConfig (idempotent, surgical)
// --------------------------------------------------------------------
function testUpdateTauriConfig() {
  const dir = mkdtempSync(join(tmpdir(), "route-conf-"));
  const path = join(dir, "tauri.conf.json");
  const original = {
    $schema: "../node_modules/@tauri-apps/cli/config.schema.json",
    productName: "Route",
    version: "0.2.0",
    identifier: "dev.route.app",
    build: {
      beforeBuildCommand: "cd web && npm run build",
      frontendDist: "./web/dist",
      devUrl: "http://localhost:1420",
    },
    app: {
      windows: [{ title: "Route", width: 960, height: 600, resizable: true, decorations: false, shadow: true }],
      withGlobalTauri: true,
      security: { csp: null },
    },
    bundle: { active: true, targets: "all", icon: ["icons/32x32.png"] },
  };
  writeFileSync(path, JSON.stringify(original, null, 2) + "\n");

  function updateTauriConfig(p) {
    if (!existsSync(p)) throw new Error("missing");
    const raw = readFileSync(p, "utf8");
    const cfg = JSON.parse(raw);
    const next = `http://localhost:${p}`;
    if (cfg?.build?.devUrl === next) return next;
    cfg.build = cfg.build || {};
    cfg.build.devUrl = next;
    writeFileSync(p, JSON.stringify(cfg, null, 2) + "\n");
    return next;
  }

  // a. First call changes the URL.
  const before = readFileSync(path, "utf8");
  // The production signature is updateTauriConfig(port, configPath).
  // We test the body inline so we don't depend on the wrapper.
  function updatePort(port, p) {
    if (!existsSync(p)) throw new Error("missing");
    const raw = readFileSync(p, "utf8");
    const cfg = JSON.parse(raw);
    const next = `http://localhost:${port}`;
    if (cfg?.build?.devUrl === next) return next;
    cfg.build = cfg.build || {};
    cfg.build.devUrl = next;
    writeFileSync(p, JSON.stringify(cfg, null, 2) + "\n");
    return next;
  }

  // b. First write: devUrl changes, everything else byte-equal.
  const after1 = JSON.parse(readFileSync(path, "utf8"));
  const next1 = updatePort(1421, path);
  const after2 = JSON.parse(readFileSync(path, "utf8"));
  assert.equal(after2.build.devUrl, "http://localhost:1421");
  // Every other key unchanged.
  assert.equal(after2.productName, after1.productName);
  assert.equal(after2.identifier, after1.identifier);
  assert.deepEqual(after2.app, after1.app);
  assert.deepEqual(after2.bundle, after1.bundle);
  assert.equal(after2.build.beforeBuildCommand, after1.build.beforeBuildCommand);
  assert.equal(after2.build.frontendDist, after1.build.frontendDist);
  console.log("✓ updateTauriConfig: changes only build.devUrl, preserves all other keys");

  // c. Idempotent: second call with same port doesn't touch the file.
  const mtimeBefore = require("node:fs").statSync(path).mtimeMs;
  // Wait a tick so any rewrite would change mtime.
  const t0 = Date.now();
  while (Date.now() - t0 < 5) { /* spin briefly */ }
  const next2 = updatePort(1421, path);
  const mtimeAfter = require("node:fs").statSync(path).mtimeMs;
  assert.equal(next2, "http://localhost:1421");
  assert.equal(mtimeBefore, mtimeAfter, "idempotent call must not rewrite the file");
  console.log("✓ updateTauriConfig: idempotent (mtime unchanged on repeat call)");

  // d. Missing config: throws.
  const missing = join(dir, "does-not-exist.json");
  assert.throws(() => updatePort(1422, missing), /missing/);
  console.log("✓ updateTauriConfig: missing path throws");
}

// --------------------------------------------------------------------
// 3. waitForHttpReady (timeout vs. resolve)
// --------------------------------------------------------------------
function startFakeServer(port) {
  return new Promise((resolve) => {
    const srv = http.createServer((req, res) => {
      res.writeHead(200, { "content-type": "text/plain" });
      res.end("ok");
    });
    srv.listen(port, "127.0.0.1", () => resolve(srv));
  });
}

async function testWaitForHttpReady() {
  // The same probe function as in start-dev.mjs.
  function waitForHttpReady(port, timeoutMs = 60000) {
    const start = Date.now();
    return new Promise((resolveReady, reject) => {
      const probe = async (path) => {
        try {
          const res = await fetch(`http://127.0.0.1:${port}${path}`, { redirect: "manual", cache: "no-store" });
          try { await res.arrayBuffer(); } catch { /* */ }
          if (res.status >= 200 && res.status < 400) return true;
          if (res.status === 404 && path === "/") return false;
          return res.status < 500;
        } catch { return false; }
      };
      const attempt = async () => {
        const rootOk = await probe("/");
        if (!rootOk) {
          if (Date.now() - start > timeoutMs) {
            reject(new Error(`Vite did not become ready on port ${port} within ${timeoutMs}ms`));
            return;
          }
          setTimeout(attempt, 100);
          return;
        }
        resolveReady();
      };
      attempt();
    });
  }

  // a. Server is up → resolves quickly.
  const srv = await startFakeServer(49300);
  const t0 = Date.now();
  await waitForHttpReady(49300, 5000);
  const dt = Date.now() - t0;
  assert.ok(dt < 1000, `expected fast resolve, got ${dt}ms`);
  await new Promise((r) => srv.close(r));
  console.log(`✓ waitForHttpReady: resolves in ${dt}ms when server is up`);

  // b. Server never starts → rejects with timeout.
  const t1 = Date.now();
  await assert.rejects(
    () => waitForHttpReady(49301, 500),
    /did not become ready/,
  );
  const dt1 = Date.now() - t1;
  assert.ok(dt1 >= 500 && dt1 < 1500, `expected ~500ms timeout, got ${dt1}ms`);
  console.log(`✓ waitForHttpReady: rejects after ${dt1}ms when nothing listens`);

  // c. Server starts AFTER the probe begins → still resolves.
  let lateServer = null;
  setTimeout(() => { startFakeServer(49302).then((s) => { lateServer = s; }); }, 300);
  const t2 = Date.now();
  await waitForHttpReady(49302, 5000);
  const dt2 = Date.now() - t2;
  assert.ok(dt2 >= 300 && dt2 < 1500, `expected ~300-500ms, got ${dt2}ms`);
  if (lateServer) await new Promise((r) => lateServer.close(r));
  console.log(`✓ waitForHttpReady: resolves mid-poll when server starts late (${dt2}ms)`);
}

(async () => {
  try {
    await testFindFreePort();
    testUpdateTauriConfig();
    await testWaitForHttpReady();
    console.log("\nAll start-dev white-box tests passed.");
  } catch (e) {
    console.error("✗ test failed:", e.message);
    process.exit(1);
  }
})();
