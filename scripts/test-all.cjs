// Aggregator: runs all white-box tests in scripts/ and prints a summary.
const { spawnSync } = require("node:child_process");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
const tests = [
  "test-logo-silencer.cjs",
  "test-start-dev.cjs",
  "test-ipc.cjs",
];

let failed = 0;
for (const t of tests) {
  console.log(`\n=== ${t} ===`);
  const r = spawnSync(process.execPath, [path.join(__dirname, t)], {
    cwd: root,
    stdio: "inherit",
  });
  if (r.status !== 0) failed++;
}

console.log(`\n${"=".repeat(50)}`);
if (failed === 0) {
  console.log(`All ${tests.length} test suites passed.`);
  process.exit(0);
} else {
  console.log(`${failed} of ${tests.length} test suites failed.`);
  process.exit(1);
}
