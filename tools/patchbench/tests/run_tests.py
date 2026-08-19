#!/usr/bin/env python3
"""Self-tests for PatchBench. Standard library only.

Usage:
    python tests/run_tests.py

Runs real CLI subprocesses and pure-function assertions against controlled
fixtures. These are DETERMINISTIC_FIXTURE tests, not real project evidence.
"""
from __future__ import annotations

import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOL = ROOT / "patchbench.py"
sys.path.insert(0, str(ROOT))

spec = importlib.util.spec_from_file_location("patchbench", TOOL)
pbench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(pbench)

PASS = 0
FAILS = 0


def check(name: str, cond: bool, detail: str = "") -> None:
    global PASS, FAILS
    if cond:
        PASS += 1
        print(f"  ok  {name}")
    else:
        FAILS += 1
        print(f"FAIL  {name}  {detail}")


def cli(*args: str, cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(TOOL), *args],
        capture_output=True,
        text=True,
        timeout=120,
        cwd=str(cwd) if cwd else None,
    )


def test_version() -> None:
    r = cli("--version")
    check("version exit 0", r.returncode == 0)
    check("version string", "patchbench 0.1.0-beta" in r.stdout, r.stdout)
    check("version no deps crash", "Traceback" not in r.stderr, r.stderr[:200])


def test_help() -> None:
    r = cli("--help")
    check("help exit 0", r.returncode == 0)
    check("help lists run/compare/demo", all(s in r.stdout for s in ("run", "compare", "demo")))


def test_pure_scope() -> None:
    changed = ["src/a.py", "tests/t.py", "Cargo.lock", "docs/x/secret.md"]
    forbid = ["Cargo.lock", "docs/**"]
    allow = ["src/**", "tests/**"]
    viol = pbench.verify_scope(changed, allow, forbid)
    kinds = {v["kind"]: v["file"] for v in viol}
    forbidden_files = {v["file"] for v in viol if v["kind"] == "FORBIDDEN"}
    check("forbidden caught", "Cargo.lock" in forbidden_files, kinds)
    # docs/x/secret.md matches forbidden docs/** -> classified FORBIDDEN (priority
    # over OUT_OF_SCOPE); at minimum it must be reported as a violation.
    reported_docs = any(v["file"] == "docs/x/secret.md" for v in viol)
    check("docs reported", reported_docs, kinds)
    check("in-scope not flagged", all(v["file"] not in ("src/a.py", "tests/t.py") for v in viol), len(viol))


def test_pure_claim_check_ref() -> None:
    checks = [{"index": 0, "status": "PASS"}, {"index": 1, "status": "FAIL"}]
    claims = [
        {"text": "c0", "kind": "check_ref", "ref": 0},
        {"text": "c1", "kind": "check_ref", "ref": 1},
        {"text": "c2", "kind": "check_ref", "ref": 9},
        {"text": "note", "kind": "info"},
    ]
    v = pbench.verify_claims(claims, checks, [])
    status = {c["text"]: c["status"] for c in v}
    check("matches pass -> VERIFIED", status.get("c0") == "VERIFIED", status)
    check("matches fail -> CONTRADICTED", status.get("c1") == "CONTRADICTED", status)
    check("unexecuted ref -> NOT_VERIFIED", status.get("c2") == "NOT_VERIFIED", status)
    check("unknown kind -> NOT_VERIFIED", status.get("note") == "NOT_VERIFIED", status)


def test_pure_verdict() -> None:
    check(
        "all good -> VERIFIED",
        pbench.compute_verdict([{"status": "PASS"}], [], [{"status": "VERIFIED"}]) == "VERIFIED",
    )
    check(
        "violation -> FAILED",
        pbench.compute_verdict([], [{"file": "x", "kind": "FORBIDDEN"}], []) == "FAILED",
    )
    check(
        "check fail -> FAILED",
        pbench.compute_verdict([{"status": "FAIL"}], [], []) == "FAILED",
    )
    check(
        "untested claim -> INCOMPLETE",
        pbench.compute_verdict([], [], [{"status": "NOT_VERIFIED"}]) == "INCOMPLETE",
    )


def test_demo() -> None:
    r = cli("demo", "--json")
    check("demo exit 0", r.returncode == 0, r.stderr[:200])
    data = json.loads(r.stdout)
    check("demo is deterministic fixture", data["kind"] == "DETERMINISTIC_FIXTURE")
    by_name = {n: x for n, x in data["scenarios"]}
    check("scope-ok VERIFIED", by_name["scope-ok"]["verdict"] == "VERIFIED")
    check("scope-violation FAILED", by_name["scope-violation"]["verdict"] == "FAILED")
    check("check-pass VERIFIED", by_name["check-pass"]["verdict"] == "VERIFIED")
    check("check-fail FAILED", by_name["check-fail"]["verdict"] == "FAILED")
    check("compare UNCHANGED", by_name["compare"]["verdict"] == "UNCHANGED")


def test_example_run() -> None:
    r = cli("run", "examples/task.json", "--repo", "examples/fixture", "--json", cwd=ROOT)
    check("example run exit 0", r.returncode == 0, r.stderr[:300])
    data = json.loads(r.stdout)
    check("example VERIFIED", data["verdict"] == "VERIFIED", data["verdict"])
    check("example no violations", len(data["violations"]) == 0)
    check("example claim verified", any(c["status"] == "VERIFIED" for c in data["claims"]))


def test_invalid_task_exit2() -> None:
    d = Path(tempfile.mkdtemp(prefix="pbtest-invalid-"))
    bad = d / "bad.json"
    bad.write_text("{}", encoding="utf-8")  # missing goal -> INVALID (2)
    r = cli("run", str(bad))
    check("missing-goal exits 2", r.returncode == 2, f"rc={r.returncode}")
    bad2 = d / "bad2.json"
    bad2.write_text(json.dumps({"goal": "g", "checks": "not-a-list"}), encoding="utf-8")
    r2 = cli("run", str(bad2))
    check("bad-schema exits 2", r2.returncode == 2, f"rc={r2.returncode}")


def main() -> int:
    tests = [
        test_version,
        test_help,
        test_pure_scope,
        test_pure_claim_check_ref,
        test_pure_verdict,
        test_demo,
        test_example_run,
        test_invalid_task_exit2,
    ]
    for t in tests:
        t()
    print(f"\n{PASS} passed, {FAILS} failed")
    return 0 if FAILS == 0 else 1


if __name__ == "__main__":
    sys.exit(main())