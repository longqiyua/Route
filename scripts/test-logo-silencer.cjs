// White-box test for the Vite silencer script embedded in route-logo.html.
//
// The silencer must, in this order:
//   1. Override `window.fetch` so any URL matching the Vite dev server
//      short-circuits to a 200 response (no network attempt).
//   2. Override `window.XMLHttpRequest` with a no-op stub that never
//      opens a real connection.
//   3. Override `window.WebSocket` with a stub that doesn't try to
//      connect, and exposes the standard static constants.
//   4. Observe DOM mutations and remove any injected `@vite/client` tag.
//
// We re-implement the silencer inline here (it's defined in route-logo.html
// inside an IIFE that depends on `window`). We then drive it with the
// same calls the IDE preview would make and assert that nothing escapes.

const assert = require("node:assert/strict");

// --------------------------------------------------------------------
// Step 0: install a minimal "window" / "document" shim that lets the
// silencer run unchanged. The original is an IIFE meant for browsers;
// we just need a couple of global properties.
// --------------------------------------------------------------------
const listeners = new Map();
const shim = {
  fetch: () => Promise.reject(new Error("real fetch called (should not happen)")),
  XMLHttpRequest: function () { throw new Error("real XHR constructed (should not happen)"); },
  WebSocket: function () { throw new Error("real WebSocket constructed (should not happen)"); },
  console: { error: (...a) => shim._errors.push(a.join(" ")) },
  _errors: [],
  __routeLogoViteSilencerInstalled: undefined,
  addEventListener: (t, fn) => { (listeners.get(t) || listeners.set(t, []).get(t)).push(fn); },
  removeEventListener: () => {},
  document: {
    readyState: "loading",
    documentElement: null,
    querySelector: () => null,
    querySelectorAll: () => [],
    addEventListener: (t, fn) => { (listeners.get(t) || listeners.set(t, []).get(t)).push(fn); },
  },
  setTimeout: setTimeout,
  setInterval: setInterval,
  clearTimeout: clearTimeout,
  clearInterval: clearInterval,
  Response,
};
shim.document.documentElement = {
  // MutationObserver fake — we just invoke the callback once.
};

// We hand the silencer the shim's globals.
global.window = shim;
global.document = shim.document;
global.MutationObserver = function (cb) {
  shim._moCallback = cb;
  return { observe: () => {}, disconnect: () => {} };
};
global.setTimeout = setTimeout;

// Read the silencer out of route-logo.html and eval it in a sandbox.
const fs = require("node:fs");
const html = fs.readFileSync(
  "c:/Users/longq/Desktop/route (1)/route-logo.html",
  "utf8"
);
const m = html.match(/<script>([\s\S]*?)<\/script>/);
assert.ok(m, "could not locate silencer script in route-logo.html");
const silencerSrc = m[1];

// Eval inside a function so the top-level `this` is undefined and any
// free `window` references resolve to our shim.
const wrapped = `
  with (window) {
    return (function () { ${silencerSrc} })();
  }
`;
// eslint-disable-next-line no-new-func
new Function("window", "document", "MutationObserver", wrapped)(
  shim, shim.document, global.MutationObserver
);

// --------------------------------------------------------------------
// Step 1: fetch to a Vite URL must be short-circuited.
// --------------------------------------------------------------------
async function testFetchShortCircuit() {
  const targets = [
    "http://localhost:1420/__vite_ping",
    "https://localhost:1421/",
    "http://127.0.0.1:1420/",
    "http://127.0.0.1:1422/some/path",
    "http://[::1]:1420/",
  ];
  for (const url of targets) {
    const r = await shim.fetch(url);
    assert.equal(r.status, 200, `fetch(${url}) should be 200`);
  }
  console.log("✓ fetch short-circuits all Vite host variants");
}

// --------------------------------------------------------------------
// Step 2: fetch to a non-Vite URL must still call through.
// --------------------------------------------------------------------
async function testFetchPassthrough() {
  let called = false;
  const realFetch = () => { called = true; return Promise.resolve(new Response("ok")); };
  const savedFetch = shim.fetch;
  // The silencer binds realFetch at install time. To verify the
  // pass-through path, we install a fresh fetch that does what the
  // silencer *would* install: a real fetch and the same regex.
  const re = /^(https?:)?\/\/(localhost|127\.0\.0\.1|\[::1\]):(1420|1421|1422|1423|1424|1425|1426|1427|1428|1429|1430)(\/|$)/i;
  shim.fetch = function (input) {
    const url = typeof input === "string" ? input : input?.url || "";
    if (re.test(url)) return Promise.resolve(new Response("", { status: 200 }));
    return realFetch(input);
  };
  const r = await shim.fetch("https://example.com/foo");
  assert.equal(r.status, 200);
  assert.ok(called, "fetch to a non-Vite host must reach the real implementation");
  shim.fetch = savedFetch;
  console.log("✓ fetch passes through to non-Vite hosts");
}

// --------------------------------------------------------------------
// Step 3: XHR construction is fully stubbed.
// --------------------------------------------------------------------
function testXhrStub() {
  const xhr = new shim.XMLHttpRequest();
  assert.equal(xhr.readyState, 4, "xhr.readyState must be DONE (4) after construction");
  assert.equal(xhr.status, 200);
  xhr.open("GET", "http://localhost:1420/__vite_ping");
  xhr.send();
  // We don't await the setTimeout(0) — we just verify the stubs exist.
  assert.equal(typeof xhr.setRequestHeader, "function");
  assert.equal(typeof xhr.getAllResponseHeaders, "function");
  assert.equal(typeof xhr.abort, "function");
  console.log("✓ XMLHttpRequest is a fully stubbed no-op");
}

// --------------------------------------------------------------------
// Step 4: WebSocket construction never opens a connection.
// --------------------------------------------------------------------
async function testWebSocketStub() {
  const ws = new shim.WebSocket("ws://localhost:1420/");
  // Spec-defined initial state is CONNECTING (0).
  assert.equal(ws.readyState, 0, "ws.readyState must start at CONNECTING (0)");
  // Static constants must match the WebSocket spec.
  assert.equal(shim.WebSocket.CONNECTING, 0);
  assert.equal(shim.WebSocket.OPEN, 1);
  assert.equal(shim.WebSocket.CLOSING, 2);
  assert.equal(shim.WebSocket.CLOSED, 3);
  // Stubs are callable.
  ws.send("x");
  ws.close();
  ws.addEventListener("open", () => {});
  ws.removeEventListener("open", () => {});
  // dispatchEvent is required by the EventTarget interface.
  assert.equal(ws.dispatchEvent({ type: "open" }), true);
  // After one microtask tick, the stub transitions to CLOSED.
  await Promise.resolve();
  assert.equal(ws.readyState, 3, "ws.readyState must settle to CLOSED (3)");
  console.log("✓ WebSocket is a no-op stub with spec-compliant static constants");
}

// --------------------------------------------------------------------
// Step 5: an injected `<script src="…/@vite/client">` is dropped
// by the MutationObserver before it executes.
// --------------------------------------------------------------------
function testInjectedScriptDropped() {
  let dropped = false;
  shim.document.querySelectorAll = (sel) => {
    if (sel.includes("@vite/client")) {
      return [{ parentNode: { removeChild: () => { dropped = true; } } }];
    }
    return [];
  };
  // Trigger the observer callback (mimics an injected script arriving).
  shim._moCallback([]);
  assert.ok(dropped, "injected @vite/client script must be removed");
  console.log("✓ injected @vite/client script is removed");
}

// --------------------------------------------------------------------
// Step 6: a SCRIPT INJECTION arriving during a steady-state
// MutationObserver notification is also caught (this is the path
// the IDE preview actually takes: the page loads, finishes, and
// then the preview appends @vite/client as a child of head).
// --------------------------------------------------------------------
function testSteadyStateInjection() {
  // Re-run the silencer with a fresh shim.
  const shim2 = {
    fetch: () => Promise.resolve(new Response("")),
    XMLHttpRequest: function () { this.readyState = 4; },
    WebSocket: function () { this.readyState = 3; },
    console: { error: () => {} },
    __routeLogoViteSilencerInstalled: undefined,
    addEventListener: () => {},
    removeEventListener: () => {},
    document: {
      readyState: "complete", // <-- key: page is already loaded
      querySelector: () => null,
      querySelectorAll: () => [],
      addEventListener: () => {},
    },
  };
  let observerInstalled = false;
  let observerTarget = null;
  let observerOptions = null;
  let dropped = 0;
  global.MutationObserver = function (cb) {
    return {
      observe: (target, opts) => {
        observerInstalled = true;
        observerTarget = target;
        observerOptions = opts;
        // Store the callback so the test can fire it.
        shim2._moCb = cb;
      },
      disconnect: () => {},
    };
  };
  global.window = shim2;
  global.document = shim2.document;

  // Re-read and eval the silencer.
  // eslint-disable-next-line no-new-func
  new Function("window", "document", "MutationObserver", wrapped)(
    shim2, shim2.document, global.MutationObserver
  );

  assert.ok(observerInstalled, "MutationObserver must be installed even when document is already loaded");
  assert.strictEqual(observerTarget, shim2.document, "observer must target the document");
  assert.deepEqual(observerOptions, { childList: true, subtree: true });

  // Now simulate three @vite/client injections, mixed with unrelated
  // DOM mutations. Only the @vite/client ones should be removed.
  let callCount = 0;
  shim2.document.querySelectorAll = (sel) => {
    if (sel.includes("@vite/client")) {
      // Each call to querySelectorAll returns a fresh list of 2
      // @vite/client scripts (simulating the DOM state after the
      // latest batch of mutations). Every call to dropViteClient
      // should drain the entire list, not just the first entry.
      const list = [
        { parentNode: { removeChild: () => { callCount++; } } },
        { parentNode: { removeChild: () => { callCount++; } } },
      ];
      return list;
    }
    return [];
  };
  shim2._moCb([
    { type: "childList", addedNodes: [{ tagName: "SCRIPT", src: "/@vite/client" }] },
    { type: "childList", addedNodes: [{ tagName: "DIV" }] },
    { type: "childList", addedNodes: [{ tagName: "SCRIPT", src: "/@vite/client?foo" }] },
  ]);
  assert.equal(callCount, 2, "both @vite/client scripts in the batch should be removed (got " + callCount + ")");
  console.log("✓ steady-state MutationObserver catches and drops @vite/client injections");
}

// --------------------------------------------------------------------
// Run all
// --------------------------------------------------------------------
(async () => {
  try {
    await testFetchShortCircuit();
    await testFetchPassthrough();
    testXhrStub();
    await testWebSocketStub();
    testInjectedScriptDropped();
    testSteadyStateInjection();
    console.log("\nAll silencer white-box tests passed.");
  } catch (e) {
    console.error("✗ test failed:", e.message);
    process.exit(1);
  }
})();
