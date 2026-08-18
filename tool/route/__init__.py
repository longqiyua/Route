"""
tool/route/__init__.py — Route discovery adapter (NOT a copy of Route).
This is a thin reference layer that declares Route as a BUNDLED tool.
It does NOT import Route internals; it only exposes the manifest and
a probe function that Yuich's tool discovery can use.

The actual Route source remains at:
  crates/             (Rust workspace)
  packages/route-py/  (Python bindings, needs maturin build)

This adapter exists so that Yuich's tool discovery can find Route as a
bundled native handle without hardcoding "route" in Yuich Core.

IMPORTANT (P30 refresh — real friction): there is a NAME COLLISION between
the Route development system and the OS `route` command (Windows: "Manipulates
network routing tables"). The probe below therefore checks the canonical
source's presence first, and does NOT treat a shell `route` match as proof.
"""

import os
import json

_TOOL_DIR = os.path.dirname(os.path.abspath(__file__))
_MANIFEST_PATH = os.path.join(_TOOL_DIR, "TOOL.json")
# Canonical Route source lives at the repo root, NOT under tool/route/.
_PROJECT_ROOT = os.path.dirname(os.path.dirname(_TOOL_DIR))


def load_manifest():
    """Return the Route tool manifest as a dict."""
    with open(_MANIFEST_PATH, "r", encoding="utf-8") as f:
        return json.load(f)


def probe_availability():
    """Check Route availability HONESTLY.

    Returns source presence (BUNDLED means the source is physically here) and
    a runtime binary check, explicitly flagging the OS `route` name collision
    instead of trusting it as proof that the Route dev system is installed.
    """
    manifest = load_manifest()
    canonical = manifest.get("canonical_source", "crates/")
    source_path = os.path.join(_PROJECT_ROOT, canonical)
    source_present = os.path.isfile(os.path.join(source_path, "Cargo.toml"))

    entrypoint = manifest.get("entrypoint")
    runtime_available = False
    runtime_error = None
    if entrypoint == "route":
        # Name collision: OS `route` (network) vs Route dev system.
        # We do NOT treat an OS `route` binary as the dev system.
        runtime_available = False
        runtime_error = (
            "NAME_COLLISION: OS 'route' command is not the Route dev system. "
            "Route dev system is a Rust workspace at crates/ (build via cargo)."
        )
    return {
        "source_present": source_present,
        "canonical_source": canonical,
        "runtime_available": runtime_available,
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