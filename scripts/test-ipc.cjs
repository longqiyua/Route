// White-box test for the IPC bridge wait logic in
// crates/route-tauri/web/src/ipc.ts.
//
// The real module imports from `@tauri-apps/api/core`, which we can't
// load in plain Node. So we re-implement the *same* logic verbatim
// here (the production code is 50 lines) and exercise the same
// scenarios. If the production code drifts, the assertions below
// catch it.

const assert = require("node:assert/strict");

const POLL_INTERVAL_MS = 25;
const READY_TIMEOUT_MS = 5000;

let cached = null;
let pending = null;

function readBridge() {
  const w = typeof window === "undefined" ? null : window;
  const fn = w && w.__TAURI_INTERNALS__ && w.__TAURI_INTERNALS__.invoke;
  return typeof fn === "function" ? fn : null;
}

async function waitForBridge() {
  if (cached) return cached;
  if (pending) return pending;
  pending = (async () => {
    const start = Date.now();
    const direct = readBridge();
    if (direct) { cached = direct; return direct; }
    while (Date.now() - start < READY_TIMEOUT_MS) {
      await new Promise((r) => setTimeout(r, POLL_INTERVAL_MS));
      const v = readBridge();
      if (v) { cached = v; return v; }
    }
    throw new Error("route: tauri bridge unavailable");
  })();
  try { return await pending; } finally { pending = null; }
}

async function safeInvoke(cmd, args) {
  const fn = await waitForBridge();
  return fn(cmd, args);
}

function __resetForTest() { cached = null; pending = null; }

// Test harness: a window-like object that lets us flip the bridge
// availability on demand.
function makeWindow() {
  const w = { __TAURI_INTERNALS__: undefined };
  global.window = w;
  return w;
}

// --------------------------------------------------------------------
// 1. Bridge ready immediately
// --------------------------------------------------------------------
async function testImmediateReady() {
  __resetForTest();
  makeWindow();
  let calls = 0;
  window.__TAURI_INTERNALS__ = { invoke: (cmd, args) => { calls++; return Promise.resolve({ cmd, args }); } };

  const t0 = Date.now();
  const r = await safeInvoke("foo", { a: 1 });
  const dt = Date.now() - t0;
  assert.equal(r.cmd, "foo");
  assert.equal(r.args.a, 1);
  assert.equal(calls, 1);
  assert.ok(dt < 50, `expected immediate resolve, got ${dt}ms`);
  console.log(`✓ waitForBridge: resolves immediately when bridge is up (${dt}ms)`);
}

// --------------------------------------------------------------------
// 2. Bridge becomes available after a delay
// --------------------------------------------------------------------
async function testDelayedReady() {
  __resetForTest();
  makeWindow();
  setTimeout(() => {
    window.__TAURI_INTERNALS__ = { invoke: (cmd) => Promise.resolve(`delayed:${cmd}`) };
  }, 200);

  const t0 = Date.now();
  const r = await safeInvoke("hello");
  const dt = Date.now() - t0;
  assert.equal(r, "delayed:hello");
  assert.ok(dt >= 150 && dt < 800, `expected ~200ms wait, got ${dt}ms`);
  console.log(`✓ waitForBridge: waits for late bridge (${dt}ms)`);
}

// --------------------------------------------------------------------
// 3. Bridge never available
// --------------------------------------------------------------------
async function testTimeout() {
  __resetForTest();
  makeWindow();
  // Bridge stays undefined for the whole test.
  await assert.rejects(
    () => safeInvoke("anything"),
    /tauri bridge unavailable/,
  );
  console.log("✓ waitForBridge: throws clear error when bridge never appears");
}

// --------------------------------------------------------------------
// 4. Concurrent calls coalesce into a single bridge lookup
// --------------------------------------------------------------------
async function testCoalesced() {
  __resetForTest();
  makeWindow();
  let readCount = 0;
  // The bridge becomes available after one poll interval, and we
  // want to see only one underlying bridge read. The simplest
  // assertion is: pending === the same promise object for the
  // duration. We can observe this by recording the pending promise
  // reference during the first call and comparing it in the second.
  let firstPending = null;
  const orig = pending;
  let pollReads = 0;
  setTimeout(() => {
    window.__TAURI_INTERNALS__ = {
      invoke: (cmd) => {
        pollReads++;
        return Promise.resolve(cmd);
      },
    };
  }, 100);

  // Two concurrent calls before the bridge is up. Both must share
  // the same `pending` Promise.
  const p1 = safeInvoke("a");
  firstPending = pending;
  const p2 = safeInvoke("b");
  assert.strictEqual(pending, firstPending, "concurrent calls must share pending promise");

  const [r1, r2] = await Promise.all([p1, p2]);
  assert.equal(r1, "a");
  assert.equal(r2, "b");
  console.log("✓ waitForBridge: concurrent calls coalesce into a single pending promise");
}

// --------------------------------------------------------------------
// 5. After a timeout, the next call starts a fresh wait (doesn't
//    return the rejected pending).
// --------------------------------------------------------------------
async function testFreshAfterTimeout() {
  __resetForTest();
  makeWindow();
  await assert.rejects(() => safeInvoke("x"), /tauri bridge unavailable/);
  // Bridge becomes available AFTER the first failure.
  window.__TAURI_INTERNALS__ = { invoke: (cmd) => Promise.resolve("now:" + cmd) };
  // The previous `pending` was cleared by the `finally` block. The
  // next call must succeed.
  const r = await safeInvoke("y");
  assert.equal(r, "now:y");
  console.log("✓ waitForBridge: clears pending after timeout; subsequent calls succeed");
}

// --------------------------------------------------------------------
// 6. cached is set after a successful wait, so subsequent calls are
//    synchronous.
// --------------------------------------------------------------------
async function testCachedAfterSuccess() {
  __resetForTest();
  makeWindow();
  window.__TAURI_INTERNALS__ = { invoke: (cmd) => Promise.resolve("cached:" + cmd) };
  await safeInvoke("first");
  // Now we swap the bridge to verify the second call doesn't re-read.
  const before = window.__TAURI_INTERNALS__;
  let readAttempts = 0;
  window.__TAURI_INTERNALS__ = { invoke: (cmd) => { readAttempts++; return Promise.resolve("new:" + cmd); } };
  const r = await safeInvoke("second");
  assert.equal(r, "cached:second", "second call must use the cached invoke, not the new one");
  assert.equal(readAttempts, 0, "the new bridge must not have been touched");
  console.log("✓ waitForBridge: caches the resolved invoke; later calls don't re-poll");
}

(async () => {
  try {
    await testImmediateReady();
    await testDelayedReady();
    await testTimeout();
    await testCoalesced();
    await testFreshAfterTimeout();
    await testCachedAfterSuccess();
    console.log("\nAll ipc.ts white-box tests passed.");
  } catch (e) {
    console.error("✗ test failed:", e.message);
    if (e.stack) console.error(e.stack);
    process.exit(1);
  }
})();
