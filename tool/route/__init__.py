"""
tool/route/__init__.py — Route discovery reference (NOT a copy of Route).
This is a thin declaration layer that exposes Route as a BUNDLED tool to
Yuich's tool discovery. It does NOT import Route internals and Route does
not import this module.

The complete, standalone Route canonical source physically lives in THIS
directory (the same directory as this file and TOOL.json):
  ./
    Cargo.toml / Cargo.lock   (Rust workspace root)
    crates/                   (route-core, route-basic, ... )
    packages/route-py/        (Python bindings, needs maturin build)
    .cargo/config.toml        (build config)
    TOOL.json                 (this manifest)
    __init__.py               (this reference)

Temporary migration redirects (previously pointing at a repo-root crates/)
were removed when the physical relocation completed. `canonical_source` in
TOOL.json is now "." (this tool root itself).

NAME COLLISION (P8/P30): the OS `route` command on Windows is the network
routing-table utility, NOT the Route dev system. The probe therefore decides
availability from this manifest + this directory's Cargo.toml, and never
treats a shell `route` match as proof.
"""

import os
import json

_TOOL_DIR = os.path.dirname(os.path.abspath(__file__))
_MANIFEST_PATH = os.path.join(_TOOL_DIR, "TOOL.json")
# Route canonical root is THIS directory after physical relocation (P7).
_ROUTE_ROOT = _TOOL_DIR


def load_manifest():
    """Return the Route tool manifest as a dict."""
    with open(_MANIFEST_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


def probe_availability():
    """Check Route availability HONESTLY.

    Availability is decided from the manifest + the presence of the Route
    workspace root (Cargo.toml) inside this directory — NOT from a shell
    `route` binary, which on Windows collides with the network utility.
    """
    manifest = load_manifest()
    canonical = manifest.get("canonical_source", ".")
    source_path = os.path.join(_ROUTE_ROOT, canonical)
    source_present = os.path.isfile(os.path.join(source_path, "Cargo.toml"))

    runtime_error = None
    if source_present:
        runtime_error = (
            "Route dev system requires building the Rust workspace in this "
            "directory (cargo build -p route-cli); the OS `route` command is "
            "not the Route dev system (NAME_COLLISION)."
        )
    return {
        "source_present": source_present,
        "canonical_source": canonical,
        "runtime_available": False,
        "runtime_error": runtime_error,
    }


def get_discovery_info():
    """Return the discovery info for Yuich's HandleGateway."""
    manifest = load_manifest()
    probe = probe_availability()
    return {
        "tool_id": manifest["tool_id"],
        "name": manifest["name"],
        "source": manifest["source"],
        "native_handle": manifest["native_handle"],
        "handle_id": manifest.get("handle_id"),
        "handle_protocol_revision_min": manifest.get("handle_protocol_revision_min"),
        "handle_protocol_revision_max": manifest.get("handle_protocol_revision_max"),
        "capabilities": manifest.get("capabilities", []),
        "standalone": manifest.get("standalone", False),
        "availability": probe,
    }