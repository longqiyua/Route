"""
Route — lightweight version manager for Vibe Coding.

One Markdown, give it to your AI, and start using Route.
一个 Markdown，交给你的 AI，然后开始使用 Route。

Python native bindings powered by PyO3.

Usage:
    >>> import route
    >>> route.init("/path/to/project")
    >>> status = route.status()
    >>> print(status["current_branch"])
"""

from __future__ import annotations

import os
import sys
from typing import Any, Optional

# ── Import the native Rust extension ──────────────────────────────────────

try:
    from route._native import (
        # Repository
        init,
        open,
        status,
        commit,
        log,
        rollback,
        changes,
        diff,
        undo,
        redo,
        history,
        checkpoint,
        stats,
        all_snapshots,
        backup,
        annotate,
        annotations,
        # Branch
        branch_list,
        branch_create,
        branch_switch,
        branch_delete,
        # Tag
        tag_list,
        tag_create,
        tag_delete,
        # Conversation
        conversation_new,
        conversation_list,
        conversation_show,
        conversation_record,
        conversation_rollback,
        conversation_archive,
        conversation_delete,
    )
except ImportError as e:
    msg = (
        "Failed to import the Route native module.\n\n"
        "Possible causes:\n"
        "  1. The module was not built — run:\n"
        "       cargo build --release -p route-pyo3\n"
        "  2. The compiled .pyd/.so file is not in Python's search path.\n"
        "     Try setting PYTHONPATH:\n"
        "       $env:PYTHONPATH = 'target/release'  (Windows PowerShell)\n"
        "       export PYTHONPATH=target/release    (macOS/Linux)\n"
        "  3. If you installed via pip, make sure maturin built successfully:\n"
        "       pip install maturin && maturin develop\n"
        "  4. Architecture mismatch — Rust and Python must be the same bitness.\n\n"
        f"Original error: {e}"
    )
    raise ImportError(msg) from e


# ── Convenience wrappers ──────────────────────────────────────────────────

__all__ = [
    # Repository
    "init",
    "open",
    "status",
    "commit",
    "log",
    "rollback",
    "changes",
    "diff",
    "undo",
    "redo",
    "history",
    "checkpoint",
    "stats",
    "all_snapshots",
    "backup",
    "annotate",
    "annotations",
    # Branch
    "branch_list",
    "branch_create",
    "branch_switch",
    "branch_delete",
    # Tag
    "tag_list",
    "tag_create",
    "tag_delete",
    # Conversation
    "conversation_new",
    "conversation_list",
    "conversation_show",
    "conversation_record",
    "conversation_rollback",
    "conversation_archive",
    "conversation_delete",
    # Metadata
    "__version__",
    "get_project_path",
    "require_repo",
]

__version__ = "1.0.0-beta"


def get_project_path() -> Optional[str]:
    """Return the currently open Route repository path, or None."""
    try:
        s = status()
        return s.get("project_path")
    except RuntimeError:
        return None


def require_repo() -> None:
    """Raise RuntimeError if no Route repository is open."""
    if get_project_path() is None:
        raise RuntimeError(
            "No Route repository open. Call route.init(path) or route.open(path) first."
        )


# ── Auto-detect: try to open a Route repo in CWD ─────────────────────────

def auto_open() -> bool:
    """Try to open a Route repository in the current directory.

    Returns True if successful, False if no repository exists.
    This is safe to call — it will not create a new repo.
    """
    try:
        open(".")
        return True
    except RuntimeError:
        return False