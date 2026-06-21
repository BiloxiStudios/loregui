#!/usr/bin/env python3
"""
Build step: generate ``lore-tools.json`` from the LoreGUI palette manifests.

The LoreGUI command-palette describes every op the GUI can invoke as a small
TypeScript object (``frontend/src/palette/manifest/<domain>/<op>.ts``): an
``id`` (``"<domain>.<op>"``), a human ``description``, and an ``args`` array of
FieldSpecs (``name`` / ``kind`` / ``label`` / ``description`` / ``required`` /
``default`` / ``placeholder`` / ``options``). That manifest is the single source
of truth for what an op is called and what it takes — so the MCP server derives
its tool catalog from it rather than hand-maintaining a parallel list.

This script parses the TS manifest files for the ops the ``lorevm`` CLI can
dispatch (``SUPPORTED_OPS`` below — kept in lock-step with the Rust binary's
dispatch match), converts each op's FieldSpecs into a JSON-Schema
``inputSchema``, and writes the combined catalog to ``lore-tools.json`` next to
this script. ``server.py`` loads that catalog at startup.

Two real-world wrinkles handled here:

1. **Key case.** Palette FieldSpec ``name``s are camelCase (Tauri v2 maps
   camelCase JS keys to snake_case Rust args). The ``lorevm`` CLI deserialises
   ``--args`` JSON straight into the snake_case serde fields of each op's
   ``Args`` struct, so we convert ``camelCase`` → ``snake_case`` here.

2. **Manifest/op divergence.** A few palette entries target the *Tauri command*
   layer, whose arg shape differs from the raw lore-vm op the CLI calls (e.g.
   ``branch.switch`` exposes ``name`` in the palette but the op's ``Args`` field
   is ``branch``). ``ARG_OVERRIDES`` remaps those, and ``EXTRA_OPS`` supplies a
   schema for the one supported op that has no palette manifest yet
   (``file.history``).

Run it directly (``python generate_catalog.py``) or let ``server.py`` invoke it
automatically when the catalog is missing/stale.
"""

from __future__ import annotations

import json
import os
import re
import sys
from pathlib import Path
from typing import Any

# ---------------------------------------------------------------------------
# Configuration
# ---------------------------------------------------------------------------

# Default location of the LoreGUI checkout (override with LOREGUI_DIR). The
# manifests live under frontend/src/palette/manifest/<domain>/<op>.ts.
DEFAULT_LOREGUI_DIR = str(Path(__file__).resolve().parent.parent)

HERE = Path(__file__).resolve().parent
CATALOG_PATH = HERE / "lore-tools.json"

# The ops the `lorevm` CLI dispatches today. MUST stay in sync with
# SUPPORTED_OPS in crates/lorevm-cli/src/main.rs. Read ops first (the repo
# "metrics" surface), then the common mutating ops.
SUPPORTED_OPS: list[str] = [
    # read / metrics
    "repository.status",
    "repository.info",
    "repository.list",
    "revision.history",
    "revision.diff",
    "revision.info",
    "revision.find",
    "branch.list",
    "branch.info",
    "file.info",
    "file.history",
    "file.diff",
    "lock.file_query",
    "lock.file_status",
    # common mutations
    "revision.commit",
    "branch.create",
    "branch.switch",
    "file.stage",
    "file.unstage",
    "lock.file_acquire",
    "lock.file_release",
]

# Read-only ops — surfaced to the model as the repo "metrics" set and used by
# the lore_repo_summary aggregate. Mutations are excluded from the summary.
READ_OPS: set[str] = {
    "repository.status",
    "repository.info",
    "repository.list",
    "revision.history",
    "revision.diff",
    "revision.info",
    "revision.find",
    "branch.list",
    "branch.info",
    "file.info",
    "file.history",
    "file.diff",
    "lock.file_query",
    "lock.file_status",
}

# Per-op remap of a manifest FieldSpec name → the lore-vm op's Args field name,
# for the few palette entries that target the Tauri-command layer rather than
# the raw op. Keyed by op id, then {manifest_name: op_field}.
ARG_OVERRIDES: dict[str, dict[str, str]] = {
    # The palette `branch.switch` entry uses the legacy `switch_branch` command
    # whose param is `name`; the lore-vm op's Args field is `branch`.
    "branch.switch": {"name": "branch"},
}

# Ops with no palette manifest yet: define their schema directly from the Rust
# Args struct (crates/lore-vm/src/ops/file/history.rs).
EXTRA_OPS: dict[str, dict[str, Any]] = {
    "file.history": {
        "id": "file.history",
        "description": "Show the revision history of a single file.",
        "args": [
            {
                "name": "path",
                "kind": "text",
                "description": "Repository-relative path to the file.",
                "required": True,
            },
            {
                "name": "revision",
                "kind": "text",
                "description": "Optional revision to start from; empty for current.",
                "required": False,
            },
            {
                "name": "branch",
                "kind": "text",
                "description": "Restrict history to this branch; empty for current.",
                "required": False,
            },
            {
                "name": "length",
                "kind": "number",
                "description": "Number of revisions to list (0 = default 100).",
                "required": False,
            },
            {
                "name": "depth",
                "kind": "number",
                "description": "Revisions to search initially (0 = default 10).",
                "required": False,
            },
        ],
    },
}


# ---------------------------------------------------------------------------
# TS manifest parsing
# ---------------------------------------------------------------------------


def camel_to_snake(name: str) -> str:
    """camelCase → snake_case (matches the Tauri v2 key mapping)."""
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name)
    return s.lower()


def _strip_block_comments(src: str) -> str:
    return re.sub(r"/\*.*?\*/", "", src, flags=re.DOTALL)


def _extract_manifest_object(src: str) -> str:
    """Return the brace-balanced text of the ``const manifest = {...}`` object."""
    src = _strip_block_comments(src)
    m = re.search(r"manifest\s*:\s*OpManifest\s*=\s*\{", src)
    if not m:
        m = re.search(r"const\s+manifest\s*=\s*\{", src)
    if not m:
        raise ValueError("no `manifest` object literal found")
    start = src.index("{", m.start())
    depth = 0
    for i in range(start, len(src)):
        c = src[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return src[start : i + 1]
    raise ValueError("unbalanced braces in manifest object")


def _ts_object_to_json(obj_src: str) -> Any:
    """Best-effort convert a TS object literal into JSON we can ``json.loads``.

    The manifests are plain data (strings, numbers, bools, arrays, nested
    objects) — no expressions. The tricky part is that string *contents* may
    contain characters that look like structure (apostrophes, the ``e.g. 'tag'``
    pattern, braces), so we scan the source left-to-right and only rewrite text
    *outside* string literals:

    - existing double-quoted strings are copied verbatim (escapes respected);
    - single-quoted strings are re-emitted as JSON double-quoted strings;
    - outside strings we collapse ``" + "`` concatenations, quote bare keys, and
      drop trailing commas.

    This keeps inner quotes/apostrophes in descriptions intact.
    """
    out: list[str] = []
    i = 0
    n = len(obj_src)
    while i < n:
        c = obj_src[i]
        if c == '"' or c == "'":
            quote = c
            j = i + 1
            buf: list[str] = []
            while j < n:
                cj = obj_src[j]
                if cj == "\\" and j + 1 < n:
                    buf.append(obj_src[j : j + 2])
                    j += 2
                    continue
                if cj == quote:
                    break
                buf.append(cj)
                j += 1
            content = "".join(buf)
            if quote == "'":
                # Re-encode single-quoted content as a JSON string (escapes any
                # embedded double quotes/backslashes correctly).
                out.append(json.dumps(content))
            else:
                out.append('"' + content + '"')
            i = j + 1
        else:
            out.append(c)
            i += 1
    s = "".join(out)
    # Collapse string concatenations now that strings are normalised: "a" + "b".
    s = re.sub(r'"\s*\+\s*\n?\s*"', "", s)
    # Quote bare object keys:  key:  ->  "key":  (only matches outside strings
    # because string contents were already emitted as quoted tokens).
    s = re.sub(r"([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*:", r'\1"\2":', s)
    # Remove trailing commas before } or ].
    s = re.sub(r",(\s*[}\]])", r"\1", s)
    return json.loads(s)


def parse_manifest_file(path: Path) -> dict[str, Any]:
    src = path.read_text(encoding="utf-8")
    obj_src = _extract_manifest_object(src)
    return _ts_object_to_json(obj_src)


# ---------------------------------------------------------------------------
# FieldSpec → JSON Schema
# ---------------------------------------------------------------------------


def field_to_schema(field: dict[str, Any]) -> dict[str, Any]:
    kind = field.get("kind", "text")
    desc = field.get("description") or field.get("label") or ""
    if kind == "number":
        prop: dict[str, Any] = {"type": "number"}
    elif kind == "boolean":
        prop = {"type": "boolean"}
    elif kind == "string-list":
        prop = {"type": "array", "items": {"type": "string"}}
    elif kind == "enum":
        prop = {"type": "string"}
        opts = field.get("options") or []
        values = [o["value"] for o in opts if isinstance(o, dict) and "value" in o]
        if values:
            prop["enum"] = values
    else:  # text / fallback
        prop = {"type": "string"}
    if desc:
        prop["description"] = desc
    if "default" in field and field["default"] not in ("", None):
        prop["default"] = field["default"]
    return prop


def build_input_schema(op_id: str, args: list[dict[str, Any]]) -> dict[str, Any]:
    """Build the MCP inputSchema for an op from its FieldSpecs.

    Always prepends an optional ``repo`` string (overrides the LORE_REPO env for
    a single call). FieldSpec names are converted camelCase → snake_case (then
    through any per-op override) so the keys match the lore-vm op's Args.
    """
    overrides = ARG_OVERRIDES.get(op_id, {})
    properties: dict[str, Any] = {
        "repo": {
            "type": "string",
            "description": (
                "Path to the lore repository working directory. "
                "Overrides the LORE_REPO env var for this call."
            ),
        }
    }
    required: list[str] = []
    for field in args:
        raw_name = field.get("name")
        if not raw_name:
            continue
        key = overrides.get(raw_name) or camel_to_snake(raw_name)
        properties[key] = field_to_schema(field)
        if field.get("required"):
            required.append(key)
    schema: dict[str, Any] = {"type": "object", "properties": properties}
    if required:
        schema["required"] = required
    return schema


# ---------------------------------------------------------------------------
# Catalog assembly
# ---------------------------------------------------------------------------


def op_id_to_tool_name(op_id: str) -> str:
    """`"revision.history"` → `"lore_revision_history"` (a valid MCP tool name)."""
    return "lore_" + op_id.replace(".", "_")


def build_catalog(loregui_dir: Path) -> dict[str, Any]:
    manifest_root = loregui_dir / "frontend" / "src" / "palette" / "manifest"
    tools: list[dict[str, Any]] = []
    missing: list[str] = []

    for op_id in SUPPORTED_OPS:
        domain, op = op_id.split(".", 1)
        if op_id in EXTRA_OPS:
            data = EXTRA_OPS[op_id]
            description = data["description"]
            args = data["args"]
        else:
            path = manifest_root / domain / f"{op}.ts"
            if not path.exists():
                missing.append(op_id)
                continue
            data = parse_manifest_file(path)
            description = data.get("description") or data.get("label") or op_id
            args = data.get("args") or []

        tools.append(
            {
                "tool_name": op_id_to_tool_name(op_id),
                "op_id": op_id,
                "read_only": op_id in READ_OPS,
                "description": description,
                "input_schema": build_input_schema(op_id, args),
            }
        )

    if missing:
        sys.stderr.write(
            "WARNING: no palette manifest for: " + ", ".join(missing) + "\n"
        )

    return {
        "generated_from": str(manifest_root),
        "tools": tools,
    }


def main() -> int:
    loregui_dir = Path(os.environ.get("LOREGUI_DIR", DEFAULT_LOREGUI_DIR))
    catalog = build_catalog(loregui_dir)
    CATALOG_PATH.write_text(json.dumps(catalog, indent=2) + "\n", encoding="utf-8")
    print(
        f"Wrote {len(catalog['tools'])} tools to {CATALOG_PATH} "
        f"(from {loregui_dir})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
