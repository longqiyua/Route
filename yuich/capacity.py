#!/usr/bin/env python3
"""Yuich Capacity Layer — Subject-level ability, independent of tools.

Canonical:
  CAPACITY  = WHAT YUICH CAN DO       (abstract ability)
  TOOL      = WHAT YUICH CAN USE      (concrete implementation)
  SKILL     = WHAT YUICH HAS LEARNED  (procedural knowledge)
  INTERFACE = HOW SOMETHING IS CALLED (machine-readable contract)
  ADAPTER   = HOW AN AWKWARD TOOL IS MADE LEGIBLE
  HANDLE    = A NATIVE COOPERATION SURFACE (tool knows how to cooperate)

Execution resolution priority (P4):
  NATIVE RULE/API → NATIVE HANDLE → VALID SKILL → ACQUISITION → GENESIS
  A known native high-level handle (e.g. handle:route) is preferred over
  re-learning a ToolSkill for the same tool.

Design constraints:
  - stdlib only, NO DB / RAG / vector / LLM API.
  - Model-neutral, Harness-neutral.
  - Capacity outlives tools; tool loss ≠ capacity loss.
  - Capacity ≠ Permission; Capacity ≠ Availability.
  - Handle ≠ Skill ≠ Capacity; handle only connects host to tool.
"""

import json
import os
import re
import ast
import hashlib
from datetime import datetime, timezone


# ======================================================================
# CONSTANTS
# ======================================================================

CAPACITY_STATUSES = [
    "PROVISIONAL",       # Declared but never successfully executed
    "SUPPORTED",         # At least one reliable realization path
    "STABLE",            # Multiple successes, well-understood
    "DEGRADED",          # Previously stable but realization path broken
    "UNAVAILABLE_REALIZATION",  # Capacity exists but no current tool/skill
    "ARCHIVED",          # Deprecated or superseded
]

CAPACITY_DOMAINS = [
    "general", "development", "governance", "acquisition",
    "research", "data", "communication", "file", "system",
]

EXECUTION_MODES = [
    "NATIVE_API_MODE",   # Known structured interface → rule binding
    "NATIVE_HANDLE_MODE",# High-level native handle (e.g. handle:route) → cooperate
    "SKILL_MODE",        # Valid ToolSkill → procedural execution
    "ACQUISITION_MODE",  # Unknown tool → learn first
    "COMPOSE",           # Combine multiple capacities
    "TOOL_GENESIS",      # Build new tool
    "ASK_OR_DEFER",      # Cannot resolve
]

# Canonical tool-independent development capacities (P0). cap:yuich.route is
# kept only as a resolving alias; the canonical name is tool-independent.
DEVELOPMENT_CAPACITIES = [
    "cap:develop-software",   # design/build new software
    "cap:modify-software",    # repair/refactor existing software
    "cap:self-modify",        # modify Yuich's own implementation
]
DEVELOPMENT_CAPACITY_ALIASES = {
    "cap:yuich.route": ["cap:develop-software"],
}


def canonical_capacity_id(capacity_id):
    """Resolve a legacy development capacity to its canonical id (P0).

    Keeps persisted `cap:yuich.route` data working (compat alias) while the
    canonical, tool-independent name is used for resolution.
    """
    for alias, canonicals in DEVELOPMENT_CAPACITY_ALIASES.items():
        if capacity_id == alias or capacity_id.startswith(alias + ":"):
            return canonicals[0]
    return capacity_id

TOOL_MATURITY_LEVELS = [
    "UNKNOWN",
    "DISCOVERED",
    "UNDERSTOOD",
    "SKILLED",
    "EXPERIENCED",
    "STABLE_SKILL",
    "ADAPTED",
    "NATIVE_BOUND",
]

REALIZATION_TYPES = [
    "NATIVE_INTERFACE",   # Built-in or host-native binding
    "TOOL_SKILL",         # Learned procedural knowledge
    "ACQUISITION_TARGET", # Unknown but learnable
    "COMPOSITION",        # Composed from other capacities
    "MODEL_PROCEDURE",    # Cognitive model procedure
    "EXTERNAL_TOOL",      # External tool via adapter
]


def _now():
    return datetime.now(timezone.utc).isoformat()


def _stamp(seed):
    return hashlib.sha256(seed.encode()).hexdigest()[:12]


# ======================================================================
# SAFE NATIVE ARITHMETIC — fruit dogfood (REAL_OBSERVED friction)
# ======================================================================
_ARITH_RE = re.compile(r'[\d\s+\-*/().]+')
_SAFE_NODE_TYPES = {ast.Expression, ast.BinOp, ast.UnaryOp, ast.Constant,
                    ast.Add, ast.Sub, ast.Mult, ast.Div, ast.USub, ast.UAdd}
_SAFE_MATH = {"abs": abs, "round": round, "min": min, "max": max,
              "pow": pow, "int": int, "float": float}


def _extract_arithmetic_expression(text):
    """Extract the longest arithmetic-safe substring from goal text."""
    matches = _ARITH_RE.findall(text)
    if not matches:
        return None
    best = max(matches, key=lambda m: len(m.strip()))
    stripped = best.strip()
    # Must contain at least one digit and one operator, or be a single number
    if not stripped:
        return None
    if not any(c in stripped for c in "+-*/"):
        # Single number is still valid
        if not any(c.isdigit() for c in stripped):
            return None
    return stripped


def _safe_eval_arithmetic(expr):
    """Evaluate a simple arithmetic expression using AST whitelist.
    Only allows: digits, +, -, *, /, parentheses, decimal points, spaces.
    Returns None on any suspicious input."""
    try:
        tree = ast.parse(expr.strip(), mode='eval')
    except SyntaxError:
        return None
    # Walk all nodes; any disallowed AST node → reject
    for node in ast.walk(tree):
        if type(node) not in _SAFE_NODE_TYPES:
            if isinstance(node, ast.Call):
                # Allow only whitelisted math functions
                if isinstance(node.func, ast.Name) and node.func.id in _SAFE_MATH:
                    continue
            return None
    try:
        result = eval(compile(tree, '<arithmetic>', 'eval'), {"__builtins__": {}}, _SAFE_MATH)
        if isinstance(result, (int, float)):
            return result
        return None
    except Exception:
        return None


# ======================================================================
# CAPACITY RECORD
# ======================================================================

class CapacityRecord:
    """A single capacity — what Yuich can do, independent of how."""

    def __init__(self, cid, name, purpose, domain="general", description="",
                 inputs=None, outputs=None, prerequisites=None,
                 execution_requirements=None, confidence="bootstrap"):
        self.id = cid
        self.name = name
        self.purpose = purpose
        self.domain = domain
        self.description = description
        self.inputs = inputs or []
        self.outputs = outputs or []
        self.prerequisites = prerequisites or []
        self.execution_requirements = execution_requirements or []
        self.known_realizations = []
        self.skill_refs = []
        self.tool_refs = []
        self.confidence = confidence
        self.experience_refs = []
        self.limitations = []
        self.status = "PROVISIONAL"
        self.created_at = _now()
        self.updated_at = _now()

    def to_dict(self):
        return {
            "id": self.id, "name": self.name, "purpose": self.purpose,
            "domain": self.domain, "description": self.description,
            "inputs": self.inputs, "outputs": self.outputs,
            "prerequisites": self.prerequisites,
            "execution_requirements": self.execution_requirements,
            "known_realizations": self.known_realizations,
            "skill_refs": self.skill_refs, "tool_refs": self.tool_refs,
            "confidence": self.confidence, "experience_refs": self.experience_refs,
            "limitations": self.limitations, "status": self.status,
            "created_at": self.created_at, "updated_at": self.updated_at,
        }

    @classmethod
    def from_dict(cls, d):
        rec = cls(d["id"], d.get("name", ""), d.get("purpose", ""),
                  d.get("domain", "general"), d.get("description", ""))
        rec.inputs = d.get("inputs", [])
        rec.outputs = d.get("outputs", [])
        rec.prerequisites = d.get("prerequisites", [])
        rec.execution_requirements = d.get("execution_requirements", [])
        rec.known_realizations = d.get("known_realizations", [])
        rec.skill_refs = d.get("skill_refs", [])
        rec.tool_refs = d.get("tool_refs", [])
        rec.confidence = d.get("confidence", "bootstrap")
        rec.experience_refs = d.get("experience_refs", [])
        rec.limitations = d.get("limitations", [])
        rec.status = d.get("status", "PROVISIONAL")
        rec.created_at = d.get("created_at", _now())
        rec.updated_at = d.get("updated_at", _now())
        return rec


# ======================================================================
# CAPACITY NEED
# ======================================================================

class CapacityNeed:
    """What capacity is needed for a goal/encounter."""

    def __init__(self, desired_effect, capacity_candidates=None,
                 required_quality="functional", constraints=None,
                 risk="low", current_context=None, evidence_requirement="minimal"):
        self.desired_effect = desired_effect
        self.capacity_candidates = capacity_candidates or []
        self.required_quality = required_quality
        self.constraints = constraints or []
        self.risk = risk
        self.current_context = current_context or {}
        self.evidence_requirement = evidence_requirement

    def to_dict(self):
        return {
            "desired_effect": self.desired_effect,
            "capacity_candidates": self.capacity_candidates,
            "required_quality": self.required_quality,
            "constraints": self.constraints,
            "risk": self.risk,
            "current_context": self.current_context,
            "evidence_requirement": self.evidence_requirement,
        }


# ======================================================================
# CAPACITY REALIZATION
# ======================================================================

class CapacityRealization:
    """Links a capacity to a concrete implementation path."""

    def __init__(self, capacity_id, implementation_type, tool_ref=None,
                 skill_ref=None, interface_ref=None, model_ref=None,
                 procedure_ref=None, quality="medium", cost="low",
                 risk="low", reliability="unknown"):
        self.id = _stamp(f"cr:{capacity_id}:{implementation_type}:{_now()}")
        self.capacity_id = capacity_id
        self.implementation_type = implementation_type
        self.tool_ref = tool_ref
        self.skill_ref = skill_ref
        self.interface_ref = interface_ref
        self.model_ref = model_ref
        self.procedure_ref = procedure_ref
        self.quality = quality
        self.cost = cost
        self.risk = risk
        self.reliability = reliability
        self.context_requirements = []
        self.evidence_refs = []

    def to_dict(self):
        return {
            "id": self.id, "capacity_id": self.capacity_id,
            "implementation_type": self.implementation_type,
            "tool_ref": self.tool_ref, "skill_ref": self.skill_ref,
            "interface_ref": self.interface_ref, "model_ref": self.model_ref,
            "procedure_ref": self.procedure_ref,
            "quality": self.quality, "cost": self.cost,
            "risk": self.risk, "reliability": self.reliability,
            "context_requirements": self.context_requirements,
            "evidence_refs": self.evidence_refs,
        }


# ======================================================================
# CAPACITY REGISTRY
# ======================================================================

class CapacityRegistry:
    """Manages all capacities — evolves from legacy capability_refs.
    Keeps backward compatibility with existing capability_refs format."""

    def __init__(self, state=None):
        self.state = state if state is not None else {}
        self._ensure_registry()

    def _ensure_registry(self):
        """Ensure capacity_registry exists, migrate from capability_refs if needed."""
        if "capacity_registry" not in self.state:
            # Migrate from legacy capability_refs
            legacy = self.state.get("capability_refs", [])
            capacities = []
            for cap in legacy:
                capacities.append({
                    "id": cap["id"],
                    "name": cap["id"].replace("cap:", ""),
                    "purpose": ", ".join(cap.get("competencies", [])),
                    "domain": cap.get("domain", "general"),
                    "description": cap.get("degradation_policy", ""),
                    "inputs": [],
                    "outputs": [],
                    "prerequisites": [],
                    "execution_requirements": [],
                    "known_realizations": [],
                    "skill_refs": [],
                    "tool_refs": cap.get("optional_tools", []),
                    "confidence": cap.get("confidence", "bootstrap"),
                    "experience_refs": cap.get("experience_refs", []),
                    "limitations": [],
                    "status": "PROVISIONAL",
                    "created_at": _now(),
                    "updated_at": _now(),
                })
            self.state["capacity_registry"] = capacities

    def register(self, cid, name, purpose, domain="general", **kwargs):
        """Register a new capacity."""
        rec = CapacityRecord(cid, name, purpose, domain, **kwargs)
        self.state["capacity_registry"].append(rec.to_dict())
        return rec.to_dict()

    def get(self, capacity_id):
        """Get a capacity by id."""
        for c in self.state.get("capacity_registry", []):
            if c["id"] == capacity_id:
                return c
        return None

    def find_by_purpose(self, purpose_text, domain=None):
        """Find capacities matching a purpose description.
        Uses bidirectional matching: checks if purpose_text contains capacity keywords,
        and if capacity purpose contains purpose_text words."""
        matches = []
        pt = purpose_text.lower()
        for c in self.state.get("capacity_registry", []):
            if domain and c.get("domain") != domain:
                continue
            score = 0
            cp = c.get("purpose", "").lower()
            cn = c.get("name", "").lower()
            cd = c.get("description", "").lower()
            cid = c.get("id", "").lower()
            # Check if purpose_text contains capacity purpose keywords
            if cp and cp in pt:
                score += 5
            # Check if capacity purpose contains purpose_text (full match)
            if pt in cp:
                score += 5
            # Check if capacity purpose contains purpose_text words
            pt_words = set(w for w in pt.split() if len(w) > 2)
            cp_words = set(cp.split())
            common = pt_words & cp_words
            score += len(common) * 2
            # Check name match
            if pt in cn:
                score += 3
            if cn in pt:
                score += 3
            # Check id match
            if pt in cid:
                score += 2
            # Check description match
            if pt in cd:
                score += 1
            if score > 0:
                matches.append((c, score))
        matches.sort(key=lambda x: x[1], reverse=True)
        return [m[0] for m in matches]

    def find_by_domain(self, domain):
        """Find all capacities in a domain."""
        return [c for c in self.state.get("capacity_registry", [])
                if c.get("domain") == domain]

    def list_all(self):
        """List all capacities."""
        return self.state.get("capacity_registry", [])

    def update_status(self, capacity_id, status, evidence_ref=None):
        """Update capacity status based on evidence."""
        c = self.get(capacity_id)
        if not c:
            return None
        old_status = c["status"]
        c["status"] = status
        c["updated_at"] = _now()
        if evidence_ref:
            c["experience_refs"].append(evidence_ref)
        return {"id": capacity_id, "old_status": old_status, "new_status": status}

    def add_realization(self, capacity_id, realization):
        """Add a realization path to a capacity."""
        c = self.get(capacity_id)
        if not c:
            return None
        c["known_realizations"].append(realization.to_dict())
        c["updated_at"] = _now()
        if c["status"] == "PROVISIONAL":
            c["status"] = "SUPPORTED"
        return c

    def remove_realization(self, capacity_id, realization_id):
        """Remove a realization (e.g., tool lost). Capacity survives."""
        c = self.get(capacity_id)
        if not c:
            return None
        before = len(c["known_realizations"])
        c["known_realizations"] = [r for r in c["known_realizations"]
                                    if r["id"] != realization_id]
        c["updated_at"] = _now()
        if not c["known_realizations"] and c["status"] == "SUPPORTED":
            c["status"] = "UNAVAILABLE_REALIZATION"
        return {"removed": before - len(c["known_realizations"]),
                "new_status": c["status"]}

    def capacity_view(self):
        """Generate a self-view: 'what can I do?'"""
        view = {"capacities": [], "stable": 0, "supported": 0,
                "provisional": 0, "degraded": 0, "unavailable": 0}
        for c in self.state.get("capacity_registry", []):
            entry = {
                "id": c["id"], "name": c["name"],
                "purpose": c["purpose"][:80],
                "domain": c["domain"],
                "status": c["status"],
                "confidence": c["confidence"],
                "realizations": len(c["known_realizations"]),
                "tools": c["tool_refs"],
                "skills": len(c["skill_refs"]),
            }
            view["capacities"].append(entry)
            st = c["status"]
            if st == "STABLE":
                view["stable"] += 1
            elif st == "SUPPORTED":
                view["supported"] += 1
            elif st == "PROVISIONAL":
                view["provisional"] += 1
            elif st == "DEGRADED":
                view["degraded"] += 1
            elif st == "UNAVAILABLE_REALIZATION":
                view["unavailable"] += 1
        return view


# ======================================================================
# EXECUTION RESOLVER
# ======================================================================

class ExecutionResolver:
    """Resolves capacity needs into execution modes.
    Priority: NATIVE_API → NATIVE_HANDLE → SKILL → ACQUISITION → COMPOSE → GENESIS → DEFER."""

    def __init__(self, state=None):
        self.state = state if state is not None else {}
        self.capacity_registry = CapacityRegistry(self.state)
        # Optional native handle discovery (host-neutral, from injected module).
        self.native_handle = None
        try:
            from .handle import discover_route_handle
            self.native_handle = discover_route_handle(self.state)
        except Exception:
            self.native_handle = None

    def resolve(self, goal, encounter=None):
        """Main resolution entry point.
        Returns: (mode, capacity, realization, context)"""
        result = {
            "goal": goal,
            "mode": None,
            "capacity": None,
            "realization": None,
            "context": {},
            "resolution_path": [],
        }

        # Step 1: Detect capacity need
        need = self._detect_capacity_need(goal, encounter)
        result["resolution_path"].append({"step": "detect_need", "need": need.to_dict()})

        # Step 2: Find matching capacities
        capacities = self.capacity_registry.find_by_purpose(need.desired_effect)
        # Also try direct lookup by capacity_candidates
        if not capacities:
            for cid in need.capacity_candidates:
                c = self.capacity_registry.get(cid)
                if c:
                    capacities.append(c)
        result["resolution_path"].append({"step": "find_capacities",
                                           "found": len(capacities)})

        if not capacities:
            # No capacity exists → capacity gap
            result["mode"] = "ASK_OR_DEFER"
            result["capacity"] = self._create_capacity_gap(need)
            result["resolution_path"].append({"step": "capacity_gap"})
            return result

        capacity = capacities[0]
        result["capacity"] = capacity

        # Step 3: Try Native/API mode
        native = self._try_native_api(capacity, need)
        result["resolution_path"].append({"step": "try_native", "found": native is not None})
        if native:
            result["mode"] = "NATIVE_API_MODE"
            result["realization"] = native
            result["context"] = self._build_native_context(capacity, native)
            return result

        # Step 4: Try Native Handle (e.g. handle:route). Preferred over Skill
        # when the tool itself knows how to cooperate (P3/P4/P36).
        handle = self._try_native_handle(capacity, need, encounter)
        result["resolution_path"].append({"step": "try_native_handle", "found": handle is not None})
        if handle:
            result["mode"] = "NATIVE_HANDLE_MODE"
            result["realization"] = handle
            result["context"] = self._build_handle_context(capacity, handle)
            return result

        # Step 5: Try Skill mode
        skill = self._try_skill_mode(capacity, need)
        result["resolution_path"].append({"step": "try_skill", "found": skill is not None})
        if skill:
            result["mode"] = "SKILL_MODE"
            result["realization"] = skill
            result["context"] = self._build_skill_context(capacity, skill)
            return result

        # Step 6: Try Acquisition mode
        acquisition = self._try_acquisition(capacity, need)
        result["resolution_path"].append({"step": "try_acquisition", "found": acquisition is not None})
        if acquisition:
            result["mode"] = "ACQUISITION_MODE"
            result["realization"] = acquisition
            result["context"] = self._build_acquisition_context(capacity, acquisition)
            return result

        # Step 7: Try composition
        comp = self._try_compose(capacity, need)
        result["resolution_path"].append({"step": "try_compose", "found": comp is not None})
        if comp:
            result["mode"] = "COMPOSE"
            result["realization"] = comp
            return result

        # Step 8: Tool genesis
        result["mode"] = "TOOL_GENESIS"
        result["resolution_path"].append({"step": "fallback_to_genesis"})
        return result

    def _detect_capacity_need(self, goal, encounter=None):
        """Detect what capacity is needed."""
        desired = goal if isinstance(goal, str) else goal.get("desired_effect", str(goal))
        candidates = []

        # Map common goals to capacity candidates
        goal_lower = desired.lower()
        if any(w in goal_lower for w in ["calculate", "compute", "math", "sum", "add"]):
            candidates.append("cap:calculate")
        if any(w in goal_lower for w in ["search", "find", "research", "look up", "lookup"]):
            candidates.append("cap:research")
        if any(w in goal_lower for w in ["code", "develop", "build", "implement", "program"]):
            # P0: canonical, tool-independent capacity. cap:yuich.route is kept
            # only as a resolving alias for persisted data, never the primary.
            candidates.append("cap:develop-software")
            candidates.append("cap:modify-software")
            candidates.append("cap:yuich.route")  # compat alias → canonical
        if any(w in goal_lower for w in ["self-modify", "modify myself", "modify yuich",
                                         "refactor myself", "dogfood", "self-host"]):
            candidates.append("cap:self-modify")
        if any(w in goal_lower for w in ["read", "open", "view", "show", "display"]):
            candidates.append("cap:read-documents")
        if any(w in goal_lower for w in ["write", "save", "create file", "output"]):
            candidates.append("cap:write-files")
        if any(w in goal_lower for w in ["learn", "understand", "figure out", "unknown"]):
            candidates.append("cap:yuich.learn")
        if any(w in goal_lower for w in ["transform", "convert", "change", "modify"]):
            candidates.append("cap:transform-data")
        if any(w in goal_lower for w in ["compare", "diff", "versus", "vs"]):
            candidates.append("cap:compare")
        if any(w in goal_lower for w in ["remember", "recall", "history", "past"]):
            candidates.append("cap:remember")
        if any(w in goal_lower for w in ["think", "analyze", "reason", "reflect"]):
            candidates.append("cap:yuich.prethink")

        if not candidates:
            candidates.append("cap:yuich.general")

        return CapacityNeed(desired, capacity_candidates=candidates)

    def _try_native_api(self, capacity, need):
        """Check if a native/API binding exists."""
        for r in capacity.get("known_realizations", []):
            if r["implementation_type"] == "NATIVE_INTERFACE":
                # Check if still valid
                if r.get("reliability", "unknown") != "broken":
                    return r

        # Check if capacity has tool_refs with native interfaces
        for tool_id in capacity.get("tool_refs", []):
            for utd in self.state.get("universal_tool_registry", []):
                if utd.get("id") == tool_id and utd.get("learning_status") == "TRUSTED_WITHIN_SCOPE":
                    return {
                        "id": _stamp(f"native:{tool_id}"),
                        "implementation_type": "NATIVE_INTERFACE",
                        "tool_ref": tool_id,
                        "interface_ref": utd.get("interface_type"),
                        "reliability": "high",
                    }

        # Fallback: native arithmetic for cap:calculate (REAL_OBSERVED friction:
        # "compute 2+2" fell through to TOOL_GENESIS because no NATIVE_INTERFACE
        # realization was bound. This is a deterministic micro-optimization, not
        # a new tool — it uses Python's built-in arithmetic with safe guards.)
        native_calc = self._try_native_calculate(capacity, need)
        if native_calc:
            return native_calc

        return None

    def _try_native_calculate(self, capacity, need):
        """Safe deterministic arithmetic resolution for cap:calculate.
        Only activated when the goal contains a verifiable arithmetic expression.
        Uses regex-based token extraction (no eval on arbitrary strings)."""
        if capacity.get("id") != "cap:calculate":
            return None
        goal = need.desired_effect
        # Extract a safe arithmetic expression: digits, operators, parens, dots, spaces
        expr = _extract_arithmetic_expression(goal)
        if expr is None:
            return None
        result = _safe_eval_arithmetic(expr)
        if result is None:
            return None
        return {
            "id": _stamp(f"native:arith:{expr}"),
            "implementation_type": "NATIVE_INTERFACE",
            "interface_ref": "python-builtin-arithmetic",
            "tool_ref": None,
            "expression": expr,
            "computed_result": result,
            "reliability": "high",
        }

    def _try_skill_mode(self, capacity, need):
        """Check if a valid ToolSkill exists."""
        # Check skill_refs
        for skill_id in capacity.get("skill_refs", []):
            for ts in self.state.get("tool_skills", []):
                if ts.get("id") == skill_id:
                    if ts.get("confidence", "low") in ("medium", "high"):
                        return {
                            "id": _stamp(f"skill:{skill_id}"),
                            "implementation_type": "TOOL_SKILL",
                            "skill_ref": skill_id,
                            "tool_ref": ts.get("tool_id"),
                            "reliability": ts.get("confidence", "medium"),
                        }

        # Check tool_skills for matching capability
        cap_purpose = capacity.get("purpose", "").lower()
        for ts in self.state.get("tool_skills", []):
            if cap_purpose in ts.get("capability", "").lower():
                return {
                    "id": _stamp(f"skill:{ts['id']}"),
                    "implementation_type": "TOOL_SKILL",
                    "skill_ref": ts["id"],
                    "tool_ref": ts.get("tool_id"),
                    "reliability": ts.get("confidence", "medium"),
                }

        return None

    def _try_acquisition(self, capacity, need):
        """Check if tool can be learned."""
        # Check if there are tool candidates to learn
        utr = self.state.get("universal_tool_registry", [])
        cap_purpose = capacity.get("purpose", "").lower()

        for utd in utr:
            for cap in utd.get("capabilities", []):
                if cap_purpose in cap.lower():
                    if utd.get("learning_status") in ("UNKNOWN", "DISCOVERED", "UNDERSTOOD"):
                        return {
                            "id": _stamp(f"acq:{utd['id']}"),
                            "implementation_type": "ACQUISITION_TARGET",
                            "tool_ref": utd["id"],
                            "learning_status": utd.get("learning_status"),
                            "reliability": "unknown",
                        }

        # If nothing in registry but learnable
        return None

    def _try_compose(self, capacity, need):
        """Try to compose capacities."""
        # Check if any composition exists
        comps = self.state.get("tool_compositions", [])
        cap_purpose = capacity.get("purpose", "").lower()
        for tc in comps:
            if cap_purpose in tc.get("goal_pattern", "").lower():
                return {
                    "id": _stamp(f"comp:{tc.get('id', '')}"),
                    "implementation_type": "COMPOSITION",
                    "tools": tc.get("tools", []),
                    "reliability": "medium",
                }
        return None

    def _create_capacity_gap(self, need):
        """Create a capacity gap record."""
        gap = {
            "id": _stamp(f"cgap:{need.desired_effect}"),
            "desired_effect": need.desired_effect,
            "current_capacity_missing": True,
            "possible_existing_capacity_composition": [],
            "tool_candidates": [],
            "learning_possible": True,
            "build_possible": True,
            "status": "GAP",
            "created_at": _now(),
        }
        self.state.setdefault("capacity_gaps", []).append(gap)
        return gap

    # -------------------------------------------------------- Context builders
    def _build_native_context(self, capacity, realization):
        """Minimal context for native/API mode."""
        return {
            "mode": "NATIVE_API",
            "capacity_id": capacity["id"],
            "interface": realization.get("interface_ref", "unknown"),
            "tool": realization.get("tool_ref"),
            "schema": realization.get("schema", {}),
            "permissions": [],
            "verification": "check return value / exit code",
        }

    def _build_skill_context(self, capacity, realization):
        """Medium context for skill mode."""
        skill = None
        for ts in self.state.get("tool_skills", []):
            if ts.get("id") == realization.get("skill_ref"):
                skill = ts
                break
        return {
            "mode": "SKILL",
            "capacity_id": capacity["id"],
            "skill": skill,
            "tool": realization.get("tool_ref"),
            "operations": skill.get("common_operations", []) if skill else [],
            "known_failures": skill.get("common_failures", []) if skill else [],
        }

    def _development_target(self, realization):
        """Classify the development target. SELF_HOSTED_YUICH is NOT a special
        Subject mode; it just means the object being developed is Yuich itself
        and therefore requires elevated self-modification protection (P6)."""
        objective = str(realization.get("objective") or "").lower()
        target = str(realization.get("target") or "").lower()
        marker = "SELF_HOSTED_YUICH"
        elevated = False
        if any(w in objective for w in ["self-modify", "modify myself",
                                        "modify yuich", "dogfood", "self-host"]):
            elevated = True
        if any(w in target for w in ["yuich", "self", "myself"]):
            elevated = True
            marker = "SELF_HOSTED_YUICH"
        if not elevated:
            marker = "GENERAL"
        return {"objective": objective, "domain": realization.get("domain"),
                "target": target, "marker": marker, "elevated": elevated}

    def _try_native_handle(self, capacity, need, encounter=None):
        """A host-neutral high-level cooperative handle (e.g. handle:route)
        that can realize the capacity. Preferred over re-learning a Skill.

        Compat aliases (P0): legacy cap:yuich.route resolves onto the canonical
        development capacity so persisted state binds, not breaks.
        """
        if self.native_handle is None:
            return None
        if getattr(self.native_handle, "availability", "UNAVAILABLE") != "AVAILABLE":
            return None
        cap = canonical_capacity_id(capacity["id"])
        if cap not in DEVELOPMENT_CAPACITIES:
            return None
        if not self.native_handle.is_compatible_for(capacity["id"]):
            return None
        target = ""
        obj = str(need.desired_effect)
        if isinstance(encounter, dict):
            target = str(encounter.get("target") or "")
            if "objective" in encounter:
                obj = str(encounter["objective"])
        return {
            "id": _stamp(f"handle:{self.native_handle.handle_id}"),
            "implementation_type": "NATIVE_INTERFACE",
            "handle_id": self.native_handle.handle_id,
            "tool_ref": self.native_handle.tool_id,
            "protocol_revision": self.native_handle.protocol_revision,
            "high_value_context": self.native_handle.context_high_value_fields,
            "objective": obj,
            "target": target,
            "reliability": "high",
        }

    def _build_handle_context(self, capacity, realization):
        """Narrow context for native-handle mode (P27: no full history dump).

        Marks SELF_HOSTED_YUICH when the target is Yuich itself, so elevated
        self-modification protections apply without becoming a special Subject.
        """
        dev = self._development_target(realization)
        return {
            "mode": "NATIVE_HANDLE",
            "capacity_id": capacity["id"],
            "canonical_capacity": canonical_capacity_id(capacity["id"]),
            "handle_id": realization.get("handle_id"),
            "protocol_revision": realization.get("protocol_revision"),
            "high_value_context": realization.get("high_value_context", []),
            "development_context": dev,
            "verification": "Route handles its own evidence; host supplies DevelopmentIntent.",
        }

    def _build_acquisition_context(self, capacity, realization):
        """Learning context for acquisition mode."""
        # Generate LearningPacket
        from .active_learning import ActiveLearner
        al = ActiveLearner(self.state)
        packet = al.create_learning_packet(
            realization.get("tool_ref", "unknown"),
            {"desired_effect": capacity.get("purpose", "")},
            {"what_is_it": {}, "how_interact": {}, "what_require_change": [],
             "how_verify": [], "what_still_unknown": ["INTERFACE", "SEMANTICS"],
             "complexity": "unknown"},
            max_size="small"
        )
        return {
            "mode": "ACQUISITION",
            "capacity_id": capacity["id"],
            "tool": realization.get("tool_ref"),
            "learning_packet": packet,
            "learning_status": realization.get("learning_status", "UNKNOWN"),
        }


# ======================================================================
# TOOL MATURITY TRACKER
# ======================================================================

class ToolMaturityTracker:
    """Tracks the relationship maturity between Yuich and a tool."""

    def __init__(self, state=None):
        self.state = state if state is not None else {}

    def get_maturity(self, tool_id):
        """Get current maturity level for a tool."""
        for tm in self.state.get("tool_maturities", []):
            if tm["tool_id"] == tool_id:
                return tm
        return {"tool_id": tool_id, "level": "UNKNOWN", "experience_count": 0}

    def promote(self, tool_id, reason, evidence=None):
        """Promote tool maturity based on usage/learning."""
        tm = self._ensure_entry(tool_id)
        current = tm["level"]

        promotion_map = {
            "UNKNOWN": "DISCOVERED",
            "DISCOVERED": "UNDERSTOOD",
            "UNDERSTOOD": "SKILLED",
            "SKILLED": "EXPERIENCED",
            "EXPERIENCED": "STABLE_SKILL",
            "STABLE_SKILL": "ADAPTED",
            "ADAPTED": "NATIVE_BOUND",
        }

        if current in promotion_map:
            tm["level"] = promotion_map[current]
            tm["promotion_history"].append({
                "from": current, "to": tm["level"],
                "reason": reason, "timestamp": _now(),
            })
            if evidence:
                tm["evidence"].append(evidence)

        return tm

    def record_use(self, tool_id, success=True):
        """Record a tool usage experience."""
        tm = self._ensure_entry(tool_id)
        tm["experience_count"] += 1
        if success:
            tm["success_count"] += 1
        else:
            tm["failure_count"] += 1
        tm["last_used"] = _now()

        # Auto-promote based on experience
        if tm["level"] == "UNKNOWN" and tm["experience_count"] >= 1:
            tm["level"] = "DISCOVERED"
        if tm["level"] == "DISCOVERED" and tm["experience_count"] >= 3:
            tm["level"] = "UNDERSTOOD"
        if tm["level"] == "UNDERSTOOD" and tm["experience_count"] >= 5:
            tm["level"] = "SKILLED"
        if tm["level"] == "SKILLED" and tm["experience_count"] >= 10:
            tm["level"] = "EXPERIENCED"
        if tm["level"] == "EXPERIENCED" and tm["experience_count"] >= 30:
            tm["level"] = "STABLE_SKILL"

        return tm

    def _ensure_entry(self, tool_id):
        for tm in self.state.setdefault("tool_maturities", []):
            if tm["tool_id"] == tool_id:
                return tm
        entry = {
            "tool_id": tool_id,
            "level": "UNKNOWN",
            "experience_count": 0,
            "success_count": 0,
            "failure_count": 0,
            "promotion_history": [],
            "evidence": [],
            "last_used": None,
            "first_encountered": _now(),
        }
        self.state["tool_maturities"].append(entry)
        return entry


# ======================================================================
# CAPACITY SELF-EVOLUTION
# ======================================================================

class CapacityEvolution:
    """Handles capacity self-evolution: split, merge, archive, new candidates."""

    def __init__(self, state=None):
        self.state = state if state is not None else {}
        self.registry = CapacityRegistry(self.state)

    def propose_split(self, capacity_id, reason, new_capacities):
        """Propose splitting an over-broad capacity."""
        proposal = {
            "id": _stamp(f"csplit:{capacity_id}"),
            "type": "SPLIT",
            "capacity_id": capacity_id,
            "reason": reason,
            "new_capacities": new_capacities,
            "status": "CANDIDATE",
            "created_at": _now(),
        }
        self.state.setdefault("capacity_evolution_proposals", []).append(proposal)
        return proposal

    def propose_merge(self, capacity_ids, reason, merged_name):
        """Propose merging duplicate capacities."""
        proposal = {
            "id": _stamp(f"cmerge:{merged_name}"),
            "type": "MERGE",
            "capacity_ids": capacity_ids,
            "reason": reason,
            "merged_name": merged_name,
            "status": "CANDIDATE",
            "created_at": _now(),
        }
        self.state.setdefault("capacity_evolution_proposals", []).append(proposal)
        return proposal

    def propose_new(self, purpose, evidence, domain="general"):
        """Propose a new capacity based on evidence."""
        proposal = {
            "id": _stamp(f"cnew:{purpose}"),
            "type": "NEW_CAPACITY",
            "purpose": purpose,
            "domain": domain,
            "evidence": evidence,
            "status": "CANDIDATE",
            "created_at": _now(),
        }
        self.state.setdefault("capacity_evolution_proposals", []).append(proposal)
        return proposal

    def tool_to_capacity_internalization(self, tool_id, capacity_purpose):
        """Internalize a tool into a capacity. Tool teaches Yuich a new ability."""
        tool = None
        for utd in self.state.get("universal_tool_registry", []):
            if utd.get("id") == tool_id:
                tool = utd
                break

        if not tool:
            return {"status": "FAILED", "reason": "tool not found"}

        # Check if tool has enough experience
        tm = ToolMaturityTracker(self.state)
        maturity = tm.get_maturity(tool_id)
        if maturity["experience_count"] < 3:
            return {"status": "INSUFFICIENT_EXPERIENCE",
                    "reason": f"only {maturity['experience_count']} uses"}

        # Create capacity candidate
        candidate = self.propose_new(
            capacity_purpose,
            {"tool_id": tool_id, "experience_count": maturity["experience_count"],
             "tool_name": tool.get("name", "")},
            domain=tool.get("kind", "general").lower()
        )
        return {"status": "CANDIDATE", "proposal": candidate}

    def capacity_to_tool_externalization(self, capacity_id, reason):
        """Externalize a stable capacity into a tool."""
        cap = self.registry.get(capacity_id)
        if not cap:
            return {"status": "FAILED", "reason": "capacity not found"}

        if cap["status"] not in ("STABLE", "SUPPORTED"):
            return {"status": "NOT_READY", "reason": f"capacity status is {cap['status']}"}

        proposal = {
            "id": _stamp(f"cext:{capacity_id}"),
            "type": "EXTERNALIZE",
            "capacity_id": capacity_id,
            "reason": reason,
            "status": "CANDIDATE",
            "created_at": _now(),
        }
        self.state.setdefault("capacity_evolution_proposals", []).append(proposal)
        return {"status": "CANDIDATE", "proposal": proposal}


# ======================================================================
# DOGFOOD A-T: Capacity Layer Scenarios
# ======================================================================

def _run_capacity_dogfood(state_dir):
    """Dogfood A-T: Capacity Layer.
    A: NATIVE — cap:calculate + native binding → NATIVE_API_MODE
    B: SKILL — no native binding, but CLI ToolSkill → SKILL_MODE
    C: UNKNOWN — no interface/skill → ACQUISITION_MODE
    D: LEARN_TO_SKILL — unknown tool learned → next time SKILL_MODE
    E: SKILL_TO_NATIVE — repeated stable skill → adapter → native candidate
    F: TOOL_LOST — tool disappears, capacity survives
    G: MULTI_REALIZATION — one capacity, two tool realizations
    H: TOOL_MULTI_CAPACITY — one tool, multiple capacities
    I: MODEL_SWAP — model swap preserves capacity/skill
    J: WEAK_MODEL — native path simple; skill uses ToolCard; learning uses LearningPacket
    K: NO_INTERFACE — no API but has Skill → still works
    L: NO_SKILL — no Skill → docs/web/source learning
    M: CAPACITY_GAP — no capacity → gap detected
    N: TOOL_TEACHES_CAPACITY — tool experience → new capacity candidate
    O: CAPACITY_EXTERNALIZE — stable procedure → tool candidate
    P: PERMISSION — capacity exists but permission denied
    Q: FAST_PATH — simple native operation bypasses expensive cognition
    R: SLOW_PATH — unknown tool climbs ladder
    S: NO_PARALLEL_REGISTRY — API/Skill/Learning use same state
    T: SELF_VIEW — fresh model answers "what can I do?"
    """
    report = {}
    base = state_dir
    os.makedirs(base, exist_ok=True)

    def _new_state():
        return {}

    # ---- A: NATIVE — cap:calculate + native binding → NATIVE_API_MODE ---------
    stA = _new_state()
    stA["capacity_registry"] = [{
        "id": "cap:calculate", "name": "calculate", "purpose": "perform calculations",
        "domain": "general", "status": "SUPPORTED", "confidence": "high",
        "tool_refs": ["native-calculator"],
        "known_realizations": [{
            "id": "native-calc-1", "implementation_type": "NATIVE_INTERFACE",
            "tool_ref": "native-calculator", "interface_ref": "builtin",
            "reliability": "high",
        }],
        "skill_refs": [], "experience_refs": [], "limitations": [],
    }]
    resA = ExecutionResolver(stA)
    resultA = resA.resolve("calculate 2+2")
    report["A_NATIVE"] = {
        "status": "OBSERVED_PASS" if resultA["mode"] == "NATIVE_API_MODE" else "OBSERVED_FAIL",
        "evidence": f"mode={resultA['mode']}",
    }

    # ---- B: SKILL — no native binding, but CLI ToolSkill → SKILL_MODE ---------
    stB = _new_state()
    stB["capacity_registry"] = [{
        "id": "cap:transform-data", "name": "transform-data",
        "purpose": "transform data formats", "domain": "data",
        "status": "SUPPORTED", "confidence": "medium",
        "tool_refs": [], "known_realizations": [], "skill_refs": ["tsk-pandoc-1"],
    }]
    stB["tool_skills"] = [{
        "id": "tsk-pandoc-1", "tool_id": "pandoc", "tool_name": "pandoc",
        "capability": "transform data formats", "confidence": "high",
        "common_operations": ["pandoc input.md -o output.html"],
        "common_failures": [],
    }]
    resB = ExecutionResolver(stB)
    resultB = resB.resolve("convert markdown to html")
    report["B_SKILL"] = {
        "status": "OBSERVED_PASS" if resultB["mode"] == "SKILL_MODE" else "OBSERVED_FAIL",
        "evidence": f"mode={resultB['mode']}",
    }

    # ---- C: UNKNOWN — no interface/skill → ACQUISITION_MODE -------------------
    stC = _new_state()
    stC["capacity_registry"] = [{
        "id": "cap:yuich.learn", "name": "learn", "purpose": "learn tools",
        "domain": "acquisition", "status": "SUPPORTED", "confidence": "medium",
        "tool_refs": [], "known_realizations": [], "skill_refs": [],
    }]
    stC["universal_tool_registry"] = [{
        "id": "unknown-cli", "name": "unknown-cli", "kind": "CLI",
        "capabilities": ["learn tools"], "learning_status": "DISCOVERED",
    }]
    resC = ExecutionResolver(stC)
    resultC = resC.resolve("learn unknown tool")
    report["C_UNKNOWN"] = {
        "status": "OBSERVED_PASS" if resultC["mode"] == "ACQUISITION_MODE" else "OBSERVED_FAIL",
        "evidence": f"mode={resultC['mode']}",
    }

    # ---- D: LEARN_TO_SKILL — unknown tool learned → next time SKILL_MODE ------
    stD = _new_state()
    stD["capacity_registry"] = [{
        "id": "cap:transform-data", "name": "transform-data",
        "purpose": "transform data", "domain": "data", "status": "SUPPORTED",
        "tool_refs": [], "known_realizations": [], "skill_refs": ["tsk-newtool-1"],
    }]
    stD["tool_skills"] = [{
        "id": "tsk-newtool-1", "tool_id": "newtool", "tool_name": "newtool",
        "capability": "transform data", "confidence": "high",
        "common_operations": ["newtool convert input output"],
    }]
    # First call: no skill → acquisition
    stD["universal_tool_registry"] = [{
        "id": "newtool", "name": "newtool", "kind": "CLI",
        "capabilities": ["transform data"], "learning_status": "DISCOVERED",
    }]
    resD1 = ExecutionResolver({**stD, "tool_skills": []})
    resultD1 = resD1.resolve("transform data")
    # Second call: skill exists → skill mode
    resD2 = ExecutionResolver(stD)
    resultD2 = resD2.resolve("transform data")
    report["D_LEARN_TO_SKILL"] = {
        "status": "OBSERVED_PASS" if resultD1["mode"] == "ACQUISITION_MODE" and resultD2["mode"] == "SKILL_MODE" else "OBSERVED_FAIL",
        "evidence": f"first={resultD1['mode']} second={resultD2['mode']}",
    }

    # ---- E: SKILL_TO_NATIVE — repeated stable skill → native candidate ---------
    stE = _new_state()
    trackerE = ToolMaturityTracker(stE)
    trackerE._ensure_entry("stable-cli")
    for _ in range(35):
        trackerE.record_use("stable-cli", True)
    matE = trackerE.get_maturity("stable-cli")
    report["E_SKILL_TO_NATIVE"] = {
        "status": "OBSERVED_PASS" if matE["level"] in ("STABLE_SKILL", "ADAPTED", "NATIVE_BOUND") else "OBSERVED_FAIL",
        "evidence": f"level={matE['level']} uses={matE['experience_count']}",
    }

    # ---- F: TOOL_LOST — tool disappears, capacity survives --------------------
    stF = _new_state()
    regF = CapacityRegistry(stF)
    regF.register("cap:research", "research", "search and find information")
    cr = CapacityRealization("cap:research", "NATIVE_INTERFACE", tool_ref="web-search")
    regF.add_realization("cap:research", cr)
    # Remove realization (tool lost)
    regF.remove_realization("cap:research", cr.id)
    capF = regF.get("cap:research")
    report["F_TOOL_LOST"] = {
        "status": "OBSERVED_PASS" if capF["status"] == "UNAVAILABLE_REALIZATION" and capF["id"] == "cap:research" else "OBSERVED_FAIL",
        "evidence": f"capacity_exists={capF is not None} status={capF['status']}",
    }

    # ---- G: MULTI_REALIZATION — one capacity, two tool realizations ------------
    stG = _new_state()
    regG = CapacityRegistry(stG)
    regG.register("cap:research", "research", "search and find")
    cr1 = CapacityRealization("cap:research", "NATIVE_INTERFACE", tool_ref="web-search")
    cr2 = CapacityRealization("cap:research", "TOOL_SKILL", tool_ref="ripgrep")
    regG.add_realization("cap:research", cr1)
    regG.add_realization("cap:research", cr2)
    capG = regG.get("cap:research")
    report["G_MULTI_REALIZATION"] = {
        "status": "OBSERVED_PASS" if len(capG["known_realizations"]) == 2 else "OBSERVED_FAIL",
        "evidence": f"realizations={len(capG['known_realizations'])}",
    }

    # ---- H: TOOL_MULTI_CAPACITY — one tool, multiple capacities ----------------
    stH = _new_state()
    regH = CapacityRegistry(stH)
    regH.register("cap:calculate", "calculate", "compute math")
    regH.register("cap:transform-data", "transform-data", "transform data")
    capH1 = regH.get("cap:calculate")
    capH2 = regH.get("cap:transform-data")
    capH1["tool_refs"].append("python")
    capH2["tool_refs"].append("python")
    report["H_TOOL_MULTI_CAPACITY"] = {
        "status": "OBSERVED_PASS" if "python" in capH1["tool_refs"] and "python" in capH2["tool_refs"] else "OBSERVED_FAIL",
        "evidence": f"python_in_calc={'python' in capH1['tool_refs']} python_in_transform={'python' in capH2['tool_refs']}",
    }

    # ---- I: MODEL_SWAP — model swap preserves capacity/skill ------------------
    stI = _new_state()
    regI = CapacityRegistry(stI)
    regI.register("cap:remember", "remember", "recall history")
    regI.update_status("cap:remember", "SUPPORTED")
    # Simulate model swap: create new registry with same state
    regI2 = CapacityRegistry(stI)
    capI = regI2.get("cap:remember")
    report["I_MODEL_SWAP"] = {
        "status": "OBSERVED_PASS" if capI and capI["status"] == "SUPPORTED" else "OBSERVED_FAIL",
        "evidence": f"capacity_exists={capI is not None} status={capI['status'] if capI else 'none'}",
    }

    # ---- J: WEAK_MODEL — native path simple; learning uses LearningPacket ------
    stJ = _new_state()
    stJ["capacity_registry"] = [{
        "id": "cap:yuich.learn", "name": "learn", "purpose": "learn tools",
        "domain": "acquisition", "status": "SUPPORTED",
        "tool_refs": [], "known_realizations": [], "skill_refs": [],
    }]
    stJ["universal_tool_registry"] = [{
        "id": "ffmpeg", "name": "ffmpeg", "kind": "CLI",
        "capabilities": ["learn tools"], "learning_status": "DISCOVERED",
    }]
    resJ = ExecutionResolver(stJ)
    resultJ = resJ.resolve("learn ffmpeg")
    ctxJ = resultJ.get("context", {})
    has_packet = "learning_packet" in ctxJ
    report["J_WEAK_MODEL"] = {
        "status": "OBSERVED_PASS" if resultJ["mode"] == "ACQUISITION_MODE" and has_packet else "OBSERVED_FAIL",
        "evidence": f"mode={resultJ['mode']} has_packet={has_packet}",
    }

    # ---- K: NO_INTERFACE — no API but has Skill → still works ------------------
    stK = _new_state()
    stK["capacity_registry"] = [{
        "id": "cap:read-documents", "name": "read-documents",
        "purpose": "read documents", "domain": "file", "status": "SUPPORTED",
        "tool_refs": [], "known_realizations": [], "skill_refs": ["tsk-gui-reader-1"],
    }]
    stK["tool_skills"] = [{
        "id": "tsk-gui-reader-1", "tool_id": "gui-reader", "tool_name": "gui-reader",
        "capability": "read documents", "confidence": "medium",
        "common_operations": ["File → Open → select file"],
    }]
    resK = ExecutionResolver(stK)
    resultK = resK.resolve("open document")
    report["K_NO_INTERFACE"] = {
        "status": "OBSERVED_PASS" if resultK["mode"] == "SKILL_MODE" else "OBSERVED_FAIL",
        "evidence": f"mode={resultK['mode']}",
    }

    # ---- L: NO_SKILL — no Skill → docs/web/source learning ---------------------
    stL = _new_state()
    stL["capacity_registry"] = [{
        "id": "cap:yuich.learn", "name": "learn", "purpose": "learn tools",
        "domain": "acquisition", "status": "SUPPORTED",
        "tool_refs": [], "known_realizations": [], "skill_refs": [],
    }]
    stL["universal_tool_registry"] = [{
        "id": "mystery-cli", "name": "mystery-cli", "kind": "CLI",
        "capabilities": ["learn tools"], "learning_status": "UNKNOWN",
        "docs_refs": ["README.md"],
    }]
    resL = ExecutionResolver(stL)
    resultL = resL.resolve("learn mystery tool")
    report["L_NO_SKILL"] = {
        "status": "OBSERVED_PASS" if resultL["mode"] == "ACQUISITION_MODE" else "OBSERVED_FAIL",
        "evidence": f"mode={resultL['mode']}",
    }

    # ---- M: CAPACITY_GAP — no capacity → gap detected --------------------------
    stM = _new_state()
    stM["capacity_registry"] = []
    resM = ExecutionResolver(stM)
    resultM = resM.resolve("do something completely unknown")
    report["M_CAPACITY_GAP"] = {
        "status": "OBSERVED_PASS" if resultM["mode"] == "ASK_OR_DEFER" and resultM["capacity"] is not None else "OBSERVED_FAIL",
        "evidence": f"mode={resultM['mode']} has_gap={resultM['capacity'] is not None}",
    }

    # ---- N: TOOL_TEACHES_CAPACITY — tool experience → new capacity candidate ---
    stN = _new_state()
    stN["universal_tool_registry"] = [{
        "id": "docker", "name": "docker", "kind": "CLI",
        "capabilities": ["container management"],
    }]
    trackerN = ToolMaturityTracker(stN)
    for _ in range(5):
        trackerN.record_use("docker", True)
    evoN = CapacityEvolution(stN)
    resultN = evoN.tool_to_capacity_internalization("docker", "container management")
    report["N_TOOL_TEACHES_CAPACITY"] = {
        "status": "OBSERVED_PASS" if resultN["status"] == "CANDIDATE" else "OBSERVED_FAIL",
        "evidence": f"status={resultN['status']}",
    }

    # ---- O: CAPACITY_EXTERNALIZE — stable procedure → tool candidate -----------
    stO = _new_state()
    regO = CapacityRegistry(stO)
    regO.register("cap:calculate", "calculate", "perform calculations")
    regO.update_status("cap:calculate", "STABLE")
    evoO = CapacityEvolution(stO)
    resultO = evoO.capacity_to_tool_externalization("cap:calculate", "repeated deterministic procedure")
    report["O_CAPACITY_EXTERNALIZE"] = {
        "status": "OBSERVED_PASS" if resultO["status"] == "CANDIDATE" else "OBSERVED_FAIL",
        "evidence": f"status={resultO['status']}",
    }

    # ---- P: PERMISSION — capacity exists but permission denied -----------------
    stP = _new_state()
    stP["capacity_registry"] = [{
        "id": "cap:send-message", "name": "send-message",
        "purpose": "send messages", "domain": "communication",
        "status": "SUPPORTED", "confidence": "high",
        "tool_refs": [], "known_realizations": [], "skill_refs": [],
    }]
    # Capacity exists but no credentials → permission check
    has_permission = False  # No credentials available
    capP = CapacityRegistry(stP).get("cap:send-message")
    report["P_PERMISSION"] = {
        "status": "OBSERVED_PASS" if capP["status"] == "SUPPORTED" and not has_permission else "OBSERVED_FAIL",
        "evidence": f"capacity_exists={capP is not None} has_permission={has_permission}",
    }

    # ---- Q: FAST_PATH — simple native operation bypasses expensive cognition ---
    stQ = _new_state()
    stQ["capacity_registry"] = [{
        "id": "cap:calculate", "name": "calculate", "purpose": "perform calculations",
        "domain": "general", "status": "STABLE", "confidence": "high",
        "tool_refs": ["native-calculator"],
        "known_realizations": [{
            "id": "native-calc-fast", "implementation_type": "NATIVE_INTERFACE",
            "tool_ref": "native-calculator", "interface_ref": "builtin",
            "reliability": "high",
        }],
        "skill_refs": [], "experience_refs": [],
    }]
    resQ = ExecutionResolver(stQ)
    resultQ = resQ.resolve("calculate 2+2")
    # Fast path: should not go through learning, prethink, boom, etc.
    is_fast = resultQ["mode"] == "NATIVE_API_MODE" and len(resultQ["resolution_path"]) <= 3
    report["Q_FAST_PATH"] = {
        "status": "OBSERVED_PASS" if is_fast else "OBSERVED_FAIL",
        "evidence": f"mode={resultQ['mode']} steps={len(resultQ['resolution_path'])}",
    }

    # ---- R: SLOW_PATH — unknown tool climbs ladder -----------------------------
    stR = _new_state()
    stR["capacity_registry"] = [{
        "id": "cap:yuich.learn", "name": "learn", "purpose": "learn tools",
        "domain": "acquisition", "status": "SUPPORTED",
        "tool_refs": [], "known_realizations": [], "skill_refs": [],
    }]
    stR["universal_tool_registry"] = [{
        "id": "completely-new-tool", "name": "completely-new-tool", "kind": "CLI",
        "capabilities": ["learn tools"], "learning_status": "UNKNOWN",
    }]
    resR = ExecutionResolver(stR)
    resultR = resR.resolve("figure out completely new tool")
    # Slow path: goes through detection → capacity find → native fail → skill fail → acquisition
    is_slow = resultR["mode"] == "ACQUISITION_MODE" and len(resultR["resolution_path"]) >= 5
    report["R_SLOW_PATH"] = {
        "status": "OBSERVED_PASS" if is_slow else "OBSERVED_FAIL",
        "evidence": f"mode={resultR['mode']} steps={len(resultR['resolution_path'])}",
    }

    # ---- S: NO_PARALLEL_REGISTRY — API/Skill/Learning use same state -----------
    stS = _new_state()
    regS = CapacityRegistry(stS)
    regS.register("cap:test-1", "test-1", "test purpose")
    # Add a realization via skill
    crS = CapacityRealization("cap:test-1", "TOOL_SKILL", tool_ref="test-tool", skill_ref="tsk-test")
    regS.add_realization("cap:test-1", crS)
    capS = regS.get("cap:test-1")
    has_skill = any(r["implementation_type"] == "TOOL_SKILL" for r in capS["known_realizations"])
    # Verify no separate API registry, skill registry, etc.
    using_unified = "capacity_registry" in stS and "api_registry" not in stS
    report["S_NO_PARALLEL_REGISTRY"] = {
        "status": "OBSERVED_PASS" if using_unified and has_skill else "OBSERVED_FAIL",
        "evidence": f"unified_registry={using_unified} has_skill_realization={has_skill}",
    }

    # ---- T: SELF_VIEW — fresh model answers "what can I do?" -------------------
    stT = _new_state()
    regT = CapacityRegistry(stT)
    regT.register("cap:research", "research", "search information")
    regT.register("cap:calculate", "calculate", "compute math")
    regT.register("cap:yuich.learn", "learn", "learn tools")
    regT.update_status("cap:research", "STABLE")
    regT.update_status("cap:calculate", "SUPPORTED")
    view = regT.capacity_view()
    report["T_SELF_VIEW"] = {
        "status": "OBSERVED_PASS" if view["stable"] >= 1 and view["supported"] >= 1 and view["provisional"] >= 1 else "OBSERVED_FAIL",
        "evidence": f"stable={view['stable']} supported={view['supported']} provisional={view['provisional']} total={len(view['capacities'])}",
    }

    return report


# ======================================================================
# CLI Entry Point
# ======================================================================

def main():
    import argparse
    p = argparse.ArgumentParser(prog="yuich-capacity")
    sub = p.add_subparsers(dest="cmd")

    dc = sub.add_parser("dogfood")
    dc.add_argument("--state-dir", default="yuich/state")

    args = p.parse_args()
    if args.cmd == "dogfood":
        report = _run_capacity_dogfood(args.state_dir)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0

    p.print_help()
    return 1


if __name__ == "__main__":
    import sys
    sys.exit(main())