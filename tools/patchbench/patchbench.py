#!/usr/bin/env python3
"""PatchBench — a small standalone patch verification tool for AI-assisted development.

A development working group modifies a repo, then PatchBench reads a task spec and
verifies the patch against that spec deterministically:

    1. Which files changed
    2. Whether changes stayed inside allowed scope (allowed_paths) / avoided
       forbidden paths
    3. Whether declared checks (commands) actually pass when executed
    4. Whether the AI's claims are backed by real executed evidence
    5. Optional before/after comparison across two runs

PatchBench does NOT build or run an AI. Any host (Claude, Codex, Gemini, human,
Route, Yuich) can generate a task spec; PatchBench only executes it.

Commands:
  run      verify a patch against a task spec
  compare  compare a before/after pair of saved run outputs
  demo     run the deterministic fixture demos
  --help   this help
  --version

Exit codes:
  0  VERIFIED
  1  FAILED
  2  INVALID / INCOMPLETE ENVIRONMENT
"""
from __future__ import annotations

import argparse
import fnmatch
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

TOOL_VERSION = "0.1.0-beta"
SCHEMA_VERSION = 1
DEFAULT_EXIT_CODE = 0
DEFAULT_TIMEOUT = 60

# ---------------------------------------------------------------------------
# small JSON/text helpers
# ---------------------------------------------------------------------------


def norm_rel(p: str) -> str:
    return Path(p).as_posix()


def pat_matches(rel: str, pattern: str) -> bool:
    """Match a repo-relative path against a glob-ish pattern.

    '*' spans path separators (intended, so 'src/**' matches 'src/a/b/c.rs').
    """
    rel = norm_rel(rel)
    pat = pattern.rstrip("/") or "."
    return fnmatch.fnmatch(rel, pat) or fnmatch.fnmatch(rel, pat + "/*")


def load_json(path: str) -> dict:
    try:
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
    except (OSError, ValueError) as exc:
        raise TaskError(f"cannot read JSON {path}: {exc}")
    if not isinstance(data, dict):
        raise TaskError(f"{path}: expected a JSON object")
    return data


class TaskError(Exception):
    pass


def _verified(status: str) -> bool:
    return status in ("VERIFIED", "PASS")


# ---------------------------------------------------------------------------
# schema validation
# ---------------------------------------------------------------------------


def validate_task(task: dict) -> None:
    if not isinstance(task.get("goal"), str):
        raise TaskError("task.goal (string) is required")
    for key in ("allowed_paths", "forbidden_paths"):
        if key in task and not isinstance(task[key], list):
            raise TaskError(f"task.{key} must be a list")
    if "checks" in task:
        if not isinstance(task["checks"], list):
            raise TaskError("task.checks must be a list")
        for i, c in enumerate(task["checks"]):
            if not isinstance(c, dict) or not isinstance(c.get("command"), str):
                raise TaskError(f"task.checks[{i}].command (string) is required")
    if "claims" in task:
        if not isinstance(task["claims"], list):
            raise TaskError("task.claims must be a list")
    if "files" in task:
        f = task["files"]
        if not isinstance(f, dict) or not isinstance(f.get("changed"), list):
            raise TaskError("task.files must be an object with a changed list")


# ---------------------------------------------------------------------------
# changed-files resolution
# ---------------------------------------------------------------------------


def _git_changed_files(repo: Path) -> list[str]:
    """Union of modified/added/untracked files as seen by git status."""
    out = []
    try:
        proc = subprocess.run(
            ["git", "-C", str(repo), "status", "--porcelain", "-z"],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError):
        return []
    if proc.returncode != 0:
        return []
    for field in proc.stdout.split("\0"):
        field = field.strip()
        if not field:
            continue
        # porcelain -z: XY path ; a fully-untracked dir collapses to '?? dir/'
        rest = field[3:]
        path = rest.split("\t")[0].strip()
        if not path:
            continue
        if path.endswith("/"):
            # expand a collapsed untracked directory into its files
            base = repo / path
            for root, _dirs, files in os.walk(base):
                for name in files:
                    full = Path(root) / name
                    try:
                        out.append(norm_rel(str(full.relative_to(repo))))
                    except ValueError:
                        pass
        else:
            out.append(norm_rel(path))
    # dedupe, stable
    seen, uniq = set(), []
    for p in out:
        if p not in seen:
            seen.add(p)
            uniq.append(p)
    return uniq


def resolve_changed_files(task: dict, repo: Path, files_arg: str | None) -> list[str]:
    if files_arg:
        data = load_json(files_arg)
        changed = data.get("changed", [])
        if not isinstance(changed, list):
            raise TaskError("--files JSON must have a changed list")
        return [str(p) for p in changed]
    task_files = task.get("files", {}).get("changed")
    if task_files is not None:
        return [str(p) for p in task_files]
    git_files = _git_changed_files(repo)
    if git_files:
        return git_files
    raise TaskError(
        "cannot determine changed files: provide --files, task.files.changed, or run inside a git repo"
    )


# ---------------------------------------------------------------------------
# scope verification
# ---------------------------------------------------------------------------


def verify_scope(
    changed: list[str], allowed: list[str], forbidden: list[str]
) -> list[dict]:
    violations = []
    for f in changed:
        in_forbidden = any(pat_matches(f, p) for p in forbidden)
        if in_forbidden:
            violations.append(
                {
                    "file": f,
                    "kind": "FORBIDDEN",
                    "detail": "matches a forbidden_paths pattern",
                }
            )
            continue
        if allowed:
            if not any(pat_matches(f, p) for p in allowed):
                violations.append(
                    {
                        "file": f,
                        "kind": "OUT_OF_SCOPE",
                        "detail": "not matched by any allowed_paths pattern",
                    }
                )
    return violations


# ---------------------------------------------------------------------------
# check execution (verification-only, no side effects)
# ---------------------------------------------------------------------------


def run_check(idx: int, check: dict, repo: Path) -> dict:
    command = check["command"]
    timeout = int(check.get("timeout", DEFAULT_TIMEOUT))
    expected = int(check.get("expected_exit_code", DEFAULT_EXIT_CODE))
    started = time.monotonic()
    try:
        proc = subprocess.run(
            command,
            shell=False,
            cwd=str(repo),
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        timed_out = False
        exit_code = proc.returncode
        stdout = (proc.stdout or "")[-2000:]
        stderr = (proc.stderr or "")[-2000:]
    except subprocess.TimeoutExpired:
        timed_out = True
        exit_code = None
        stdout = ""
        stderr = f"timed out after {timeout}s"
    except OSError as exc:
        timed_out = False
        exit_code = None
        stdout = ""
        stderr = f"could not launch: {exc}"
    duration = round(time.monotonic() - started, 3)
    ok = (not timed_out) and exit_code == expected
    status = "PASS" if ok else ("TIMEOUT" if timed_out else "FAIL")
    return {
        "index": idx,
        "command": command,
        "exit_code": exit_code,
        "expected_exit_code": expected,
        "timed_out": timed_out,
        "duration_s": duration,
        "status": status,
        "evidence": {
            "exit_code": exit_code,
            "detail": ("stderr: " + (stderr or "(none)").strip())[:400]
            if not ok
            else f"exit code {exit_code} == expected {expected}",
        },
    }


def run_checks(checks: list, repo: Path) -> list[dict]:
    return [run_check(i, c, repo) for i, c in enumerate(checks)]


# ---------------------------------------------------------------------------
# claim vs evidence
# ---------------------------------------------------------------------------


def verify_claims(claims: list, checks: list, violations: list) -> list[dict]:
    resolved = []
    check_by_idx = {c["index"]: c for c in checks}
    has_violation = bool(violations)
    for claim in claims:
        text = claim.get("text", "(unnamed claim)")
        kind = claim.get("kind", "unknown")
        ref = claim.get("ref")
        when = claim.get("condition", "in_scope")
        if kind == "check_ref":
            if ref is None or ref not in check_by_idx:
                resolved.append(
                    {
                        "text": text,
                        "kind": kind,
                        "status": "NOT_VERIFIED",
                        "evidence": "claim references check that was not executed",
                    }
                )
            else:
                c = check_by_idx[ref]
                matched = _verified(c["status"])
                resolved.append(
                    {
                        "text": text,
                        "kind": kind,
                        "status": "VERIFIED" if matched else "CONTRADICTED",
                        "evidence": f"check[{ref}] {c['status']} exit={c.get('exit_code')}"
                        if c.get("exit_code") is not None
                        else f"check[{ref}] did not complete",
                    }
                )
        elif kind == "scope":
            # claim about staying in scope
            ok = (when == "in_scope" and not has_violation) or (
                when == "out_of_scope" and has_violation
            )
            resolved.append(
                {
                    "text": text,
                    "kind": kind,
                    "status": "VERIFIED" if ok else "CONTRADICTED",
                    "evidence": f"scope violations={len(violations)}",
                }
            )
        else:
            # any other textual claim -> cannot be verified from execution
            resolved.append(
                {
                    "text": text,
                    "kind": kind,
                    "status": "NOT_VERIFIED",
                    "evidence": "no executable rule maps to this claim",
                }
            )
    return resolved


# ---------------------------------------------------------------------------
# metrics & verdict
# ---------------------------------------------------------------------------


def extract_metrics(task_metrics: list, checks: list, total_duration: float) -> dict:
    out = {}
    default_sources = {"duration_s": total_duration}
    for c in checks:
        default_sources[f"check.{c['index']}.duration_s"] = c["duration_s"]
        default_sources[f"check.{c['index']}.exit_code"] = c["exit_code"]
        default_sources[f"check.{c['index']}.status"] = c["status"]
    for m in task_metrics or []:
        key = m.get("id", str(m))
        field = m.get("field", "duration_s")
        check = m.get("check")
        if check is not None:
            if check in check_by_index(checks):
                val = value_for_check(checks, check, field)
                out[key] = val
            else:
                out[key] = None
        elif field in default_sources:
            out[key] = default_sources[field]
        else:
            out[key] = None
    out["duration_s"] = total_duration
    return out


def check_by_index(checks):
    return {c["index"] for c in checks}


def value_for_check(checks, check, field):
    for c in checks:
        if c["index"] == check:
            if field in c:
                return c[field]
            if field == "status_ok":
                return _verified(c["status"])
    return None


def compute_verdict(checks, violations, claims) -> str:
    if violations:
        return "FAILED"
    if any(not _verified(c["status"]) for c in checks):
        return "FAILED"
    if any(cl["status"] == "CONTRADICTED" for cl in claims):
        return "FAILED"
    any_verified = bool(checks) or any(cl["status"] == "VERIFIED" for cl in claims)
    if not any_verified:
        return "INCOMPLETE"
    if any(cl["status"] == "NOT_VERIFIED" for cl in claims):
        return "INCOMPLETE"
    return "VERIFIED"


# ---------------------------------------------------------------------------
# run
# ---------------------------------------------------------------------------


def run_main(args: argparse.Namespace) -> int:
    task = load_json(args.task)
    validate_task(task)
    repo = Path(args.repo) if args.repo else Path(task.get("repo", ".")).expanduser()
    if not repo.is_dir():
        raise TaskError(f"repo path is not a directory: {repo}")
    changed = resolve_changed_files(task, repo, args.files)
    allowed = task.get("allowed_paths", [])
    forbidden = task.get("forbidden_paths", [])
    violations = verify_scope(changed, allowed, forbidden)
    checks = run_checks(task.get("checks", []), repo)
    claims = verify_claims(task.get("claims", []), checks, violations)
    total_duration = sum(c["duration_s"] for c in checks)
    metrics = extract_metrics(task.get("metrics", []), checks, total_duration)
    verdict = compute_verdict(checks, violations, claims)

    report = {
        "schema_version": SCHEMA_VERSION,
        "tool": "patchbench",
        "tool_version": TOOL_VERSION,
        "task": args.task,
        "repo": str(repo),
        "goal": task.get("goal"),
        "verdict": verdict,
        "changed_files": changed,
        "violations": violations,
        "checks": checks,
        "claims": claims,
        "metrics": metrics,
        "evidence": {
            "changed_files_obtained_from": (
                "--files"
                if args.files
                else ("task.files.changed" if task.get("files") else "git status")
            ),
            "checks_executed": len(checks),
            "claims_verified": sum(1 for c in claims if c["status"] == "VERIFIED"),
            "claims_not_verified": sum(
                1 for c in claims if c["status"] == "NOT_VERIFIED"
            ),
        },
    }
    if args.save:
        with open(args.save, "w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2)
    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print_human(report)
    return 0 if verdict == "VERIFIED" else (1 if verdict == "FAILED" else 2)


def print_human(report: dict) -> None:
    print("PatchBench")
    print("  Tool:", f"{report['tool']} {report['tool_version']}")
    print("  Goal:", report["goal"])
    print("  Changed files:", ", ".join(report["changed_files"]) or "(none)")
    print("  Source of changed files:", report["evidence"]["changed_files_obtained_from"])
    print("  Scope violations:", len(report["violations"]))
    for v in report["violations"]:
        print(f"    - {v['file']}  ({v['kind']}: {v['detail']})")
    print("  Checks:")
    for c in report["checks"]:
        print(
            f"    - [{c['status']}] {c['command']} "
            f"(exit={c['exit_code']}, {c['duration_s']}s)"
        )
    print("  Claims:")
    for cl in report["claims"]:
        print(f"    - [{cl['status']}] {cl['text']}  ({cl['evidence']})")
    print("  Metrics:", json.dumps(report["metrics"]))
    print("  Verdict:", report["verdict"])


# ---------------------------------------------------------------------------
# compare
# ---------------------------------------------------------------------------


def compare_dicts(before: dict, after: dict) -> dict:
    b_checks = {c["command"]: c for c in before.get("checks", [])}
    a_checks = {c["command"]: c for c in after.get("checks", [])}
    common = sorted(set(b_checks) & set(a_checks))

    deltas = []
    check_regressed = False
    check_improved = False
    for cmd in common:
        bs, as_ = _verified(b_checks[cmd]["status"]), _verified(a_checks[cmd]["status"])
        if bs and not as_:
            check_regressed = True
            deltas.append(f"check regressed: {cmd}")
        elif not bs and as_:
            check_improved = True
            deltas.append(f"check improved: {cmd}")

    b_viol = len(before.get("violations", []))
    a_viol = len(after.get("violations", []))
    if a_viol > b_viol:
        check_regressed = True
        deltas.append(f"scope violations regressed: {b_viol} -> {a_viol}")
    elif a_viol < b_viol:
        check_improved = True
        deltas.append(f"scope violations improved: {b_viol} -> {a_viol}")

    if not common and b_viol == a_viol:
        deltas.append("no comparable fields")

    if check_regressed:
        verdict = "REGRESSED"
    elif check_improved:
        verdict = "IMPROVED"
    elif common:
        verdict = "UNCHANGED"
    else:
        verdict = "INCOMPARABLE"

    return {
        "tool_version": TOOL_VERSION,
        "verdict": verdict,
        "before_verdict": before.get("verdict"),
        "after_verdict": after.get("verdict"),
        "common_checks": common,
        "deltas": deltas,
        "before_metrics": before.get("metrics"),
        "after_metrics": after.get("metrics"),
        "deterministic": False,
    }


def compare_main(args: argparse.Namespace) -> int:
    before = load_json(args.before) if isinstance(args.before, str) else args.before
    after = load_json(args.after) if isinstance(args.after, str) else args.after
    result = compare_dicts(before, after)
    if args.json:
        print(json.dumps(result, indent=2))
    else:
        print(f"PatchBench compare: {result['verdict']}")
        print("  before verdict:", result["before_verdict"])
        print("  after verdict:", result["after_verdict"])
        print("  common checks:", len(result["common_checks"]))
        for d in result["deltas"]:
            print("   -", d)
        print("  before metrics:", before.get("metrics"))
        print("  after metrics:", after.get("metrics"))
    return 0


# ---------------------------------------------------------------------------
# demo (DETERMINISTIC_FIXTURE)
# ---------------------------------------------------------------------------


def _write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def demo_main(args: argparse.Namespace) -> int:
    tmp = Path(tempfile.mkdtemp(prefix="patchbench-demo-"))
    results = []
    try:
        # --- scenario 1: scope ---
        repo = tmp / "repo_scope" / "app"
        _write(repo / "src/main.py", "def main():\n    return 42\n")
        _write(repo / "tests/test_main.py", "def test_main():\n    assert main() == 42\n")
        _git_init_empty(repo)
        # in-scope patch
        _write(repo / "src/main.py", "def main():\n    return 43\n")
        r1 = run_task(
            {
                "goal": "in-scope edit of app code",
                "allowed_paths": ["src/**", "tests/**"],
                "forbidden_paths": ["Cargo.toml", "docs/**"],
                "checks": [{"command": "python -m py_compile src/main.py", "timeout": 20}],
            },
            repo,
        )
        # out-of-scope patch
        _write(repo / "docs/private/secret.md", "do-not-touch")
        r2 = run_task(
            {
                "goal": "edit that strays into a forbidden path",
                "allowed_paths": ["src/**", "tests/**"],
                "forbidden_paths": ["docs/private/**"],
            },
            repo,
        )
        results.append(("scope-ok", r1))
        results.append(("scope-violation", r2))

        # --- scenario 2: check execution ---
        repo2 = tmp / "repo_check"
        _write(repo2 / "x.py", "print('ok')\n")
        _git_init_empty(repo2)
        r3 = run_task(
            {
                "goal": "checks pass when executed",
                "allowed_paths": ["**"],
                "checks": [{"command": "python -c \"import sys; sys.exit(0)\"", "timeout": 20}],
            },
            repo2,
        )
        r4 = run_task(
            {
                "goal": "check fails on non-zero exit",
                "allowed_paths": ["**"],
                "checks": [{"command": "python -c \"import sys; sys.exit(3)\"", "timeout": 20}],
            },
            repo2,
        )
        results.append(("check-pass", r3))
        results.append(("check-fail", r4))

        # --- scenario 3: before/after ---
        repo3 = tmp / "repo_ba"
        _write(repo3 / "solve.py", "def solve():\n    return 1\n")
        _git_init_empty(repo3)
        task3 = {
            "goal": "before-after improvement",
            "allowed_paths": ["solve.py"],
            "checks": [{"command": "python -c \"import sys; sys.exit(0)\"", "timeout": 20}],
        }
        before = run_task(task3, repo3)
        _write(repo3 / "solve.py", "def solve():\n    return 2\n")
        after = run_task(task3, repo3)
        cmp_result = compare_dicts(before, after)
        results.append(("before", before))
        results.append(("after", after))
        results.append(("compare", cmp_result))

        report = {
            "tool": "patchbench-demo",
            "tool_version": TOOL_VERSION,
            "kind": "DETERMINISTIC_FIXTURE",
            "note": "controlled demo fixtures, not real project evidence",
            "scenarios": results,
        }
        if args.json:
            print(json.dumps(report, indent=2))
        else:
            print("PatchBench demo  (DETERMINISTIC_FIXTURE)")
            print("  Fixtures are controlled; do not treat as real project evidence.")
            for name, r in results:
                if "verdict" in r and "changed_files" in r:
                    print(f"  - {name}: {r['verdict']}  (changed={len(r['changed_files'])}, "
                          f"violations={len(r['violations'])}, checks={len(r['checks'])})")
                else:
                    print(f"  - {name}: compare={r['verdict']}  "
                          f"(before={r.get('before_verdict')}, after={r.get('after_verdict')})")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    return 0


def _git_init_empty(repo: Path) -> None:
    try:
        subprocess.run(["git", "init", "-q", str(repo)], capture_output=True)
        subprocess.run(
            ["git", "-C", str(repo), "config", "user.email", "demo@pbench.local"],
            capture_output=True,
        )
        subprocess.run(
            ["git", "-C", str(repo), "config", "user.name", "PatchBench Demo"],
            capture_output=True,
        )
    except (OSError, subprocess.SubprocessError):
        pass


def run_task(task: dict, repo: Path) -> dict:
    # embedded verify (no printing), used by demo fixtures
    changed = resolve_changed_files(task, repo, None)
    violations = verify_scope(changed, task.get("allowed_paths", []), task.get("forbidden_paths", []))
    checks = run_checks(task.get("checks", []), repo)
    claims = verify_claims(task.get("claims", []), checks, violations)
    metrics = extract_metrics(task.get("metrics", []), checks, sum(c["duration_s"] for c in checks))
    verdict = compute_verdict(checks, violations, claims)
    return {
        "tool_version": TOOL_VERSION,
        "goal": task.get("goal"),
        "verdict": verdict,
        "changed_files": changed,
        "violations": violations,
        "checks": checks,
        "claims": claims,
        "metrics": metrics,
    }


# ---------------------------------------------------------------------------
# cli
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="patchbench",
        description="PatchBench — standalone patch verification for AI-assisted development.",
        epilog="More in README.md",
    )
    p.add_argument("--version", action="store_true", help="show version and exit")
    sub = p.add_subparsers(dest="cmd", required=False)

    r = sub.add_parser("run", help="verify a patch against a task spec")
    r.add_argument("task", help="path to task.json")
    r.add_argument("--repo", help="project repo dir (default: task.repo or cwd)")
    r.add_argument("--files", help="path to changed-files JSON {changed:[...]}")
    r.add_argument("--save", help="write the full run report JSON to this path")
    r.add_argument("--json", action="store_true", help="machine-readable output")
    r.set_defaults(func=run_main)

    c = sub.add_parser("compare", help="compare before/after run reports")
    c.add_argument("before", help="path to before.json")
    c.add_argument("after", help="path to after.json")
    c.add_argument("--json", action="store_true")
    c.set_defaults(func=compare_main)

    d = sub.add_parser("demo", help="run deterministic fixture demos")
    d.add_argument("--json", action="store_true")
    d.set_defaults(func=demo_main)

    return p


def main(argv: list[str]) -> int:
    if "--version" in argv:
        print(f"patchbench {TOOL_VERSION}")
        return 0
    parser = build_parser()
    args = parser.parse_args(argv)
    if not getattr(args, "cmd", None) or not getattr(args, "func", None):
        parser.print_help()
        return 2
    try:
        return args.func(args)
    except TaskError as exc:
        print(f"patchbench: error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))