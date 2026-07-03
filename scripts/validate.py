#!/usr/bin/env python3
"""
Validate compat metadata on LoreGUI template assets.

Scans ``templates/`` for layout JSON files, pack.json files, plugin.json
files, and skill YAML files.  For each asset it checks:

  1. A ``compat`` object is present.
  2. ``compat.min_core_version`` is present and is a valid semver
     (MAJOR.MINOR.PATCH, no pre-release suffix required here).
  3. If ``compat.target_api_version`` is present it is also a valid semver.

Exit codes:
  0 — all assets pass
  1 — one or more validation errors

Usage:
  python scripts/validate.py                   # strict (default)
  python scripts/validate.py --allow-missing-compat  # warn-only for missing compat
  python scripts/validate.py templates/layouts templates/plugins
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

try:
    import yaml as _yaml  # type: ignore[import]
    _YAML_AVAILABLE = True
except ImportError:
    _YAML_AVAILABLE = False

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_SEMVER = re.compile(r"^\d+\.\d+\.\d+$")


def _is_semver(value: str) -> bool:
    return bool(_SEMVER.match(str(value)))


def _load_json(path: Path) -> tuple[dict[str, Any] | None, str | None]:
    try:
        return json.loads(path.read_text(encoding="utf-8")), None
    except json.JSONDecodeError as exc:
        return None, f"JSON parse error: {exc}"


def _load_yaml(path: Path) -> tuple[dict[str, Any] | None, str | None]:
    if not _YAML_AVAILABLE:
        return None, "PyYAML not installed; run: pip install pyyaml"
    try:
        data = _yaml.safe_load(path.read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            return None, "YAML root must be a mapping"
        return data, None
    except _yaml.YAMLError as exc:
        return None, f"YAML parse error: {exc}"


# ---------------------------------------------------------------------------
# Asset discovery
# ---------------------------------------------------------------------------

def _discover(roots: list[Path]) -> list[Path]:
    """Return all asset files under *roots* in a deterministic order."""
    assets: list[Path] = []
    for root in roots:
        if not root.exists():
            continue
        for path in sorted(root.rglob("*")):
            if not path.is_file():
                continue
            name = path.name
            # Layout files: any .json directly under layouts/ subdirectory
            # Pack files:   pack.json
            # Plugin files: plugin.json
            # Skill files:  .yaml or .yml under skills/ subdirectory
            if name in ("pack.json", "plugin.json"):
                assets.append(path)
            elif path.suffix == ".json" and "layouts" in path.parts:
                assets.append(path)
            elif path.suffix in (".yaml", ".yml") and "skills" in path.parts:
                assets.append(path)
    return assets


# ---------------------------------------------------------------------------
# Validation
# ---------------------------------------------------------------------------

def _validate_asset(
    path: Path,
    allow_missing: bool,
) -> list[str]:
    """Return a list of error strings (empty = OK)."""
    errors: list[str] = []

    if path.suffix in (".yaml", ".yml"):
        data, err = _load_yaml(path)
    else:
        data, err = _load_json(path)

    if err is not None:
        return [err]

    compat = data.get("compat") if data else None

    if compat is None:
        msg = "missing 'compat' object"
        if allow_missing:
            # Treated as a warning — do not add to errors.
            print(f"  WARN  {path}: {msg}", file=sys.stderr)
            return []
        return [msg]

    if not isinstance(compat, dict):
        return ["'compat' must be a JSON object / YAML mapping"]

    mcv = compat.get("min_core_version")
    if mcv is None:
        errors.append("'compat.min_core_version' is missing")
    elif not _is_semver(mcv):
        errors.append(
            f"'compat.min_core_version' is not a valid semver: {mcv!r}"
        )

    tav = compat.get("target_api_version")
    if tav is not None and not _is_semver(str(tav)):
        errors.append(
            f"'compat.target_api_version' is not a valid semver: {tav!r}"
        )

    return errors


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------

def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate compat metadata on LoreGUI template assets.",
    )
    parser.add_argument(
        "paths",
        nargs="*",
        metavar="PATH",
        help="Directories or files to scan (default: templates/).",
    )
    parser.add_argument(
        "--allow-missing-compat",
        action="store_true",
        help=(
            "Treat missing 'compat' blocks as warnings rather than errors. "
            "Intended for the grandfathering period only; must be removed "
            "once every asset is migrated."
        ),
    )
    args = parser.parse_args(argv)

    repo_root = Path(__file__).resolve().parent.parent
    if args.paths:
        roots = [Path(p) for p in args.paths]
    else:
        roots = [repo_root / "templates"]

    assets = _discover(roots)
    if not assets:
        print("No template assets found.", file=sys.stderr)
        return 0

    total_errors = 0
    for asset in assets:
        rel = asset.relative_to(repo_root) if asset.is_relative_to(repo_root) else asset
        errors = _validate_asset(asset, args.allow_missing_compat)
        if errors:
            for msg in errors:
                print(f"  ERROR {rel}: {msg}")
            total_errors += len(errors)

    if total_errors:
        print(
            f"\n{total_errors} error(s) found across {len(assets)} asset(s). "
            "Fix compat metadata before merging.",
            file=sys.stderr,
        )
        return 1

    print(f"OK — {len(assets)} asset(s) validated, 0 errors.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
