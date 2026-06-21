#!/usr/bin/env python3
"""
lore-mcp — an MCP server that lets an AI agent drive Epic Games' ``lore`` VCS
in-process, the same way git/p4 MCP servers expose git/p4.

It exposes one MCP tool per supported lore op (status, history, diff, branches,
staging, locks, …). Each tool's name, description, and JSON input schema come
from the **LoreGUI command-palette manifest** (the single source of truth for
what an op is called and what it takes), pre-baked into ``lore-tools.json`` by
``generate_catalog.py``. When a tool is called the server shells out to the
``lorevm`` JSON CLI (built from ``crates/lorevm-cli`` in the loregui repo) with
the op id, the repo directory, and the JSON args, and returns the op's JSON
result verbatim.

Read ops (status / history / file history / diff / locks) are the repo
"metrics" surface; the ``lore_repo_summary`` convenience tool aggregates a few
of them into one snapshot.

Configuration (env):
  LORE_REPO   default repository working directory (a tool's ``repo`` arg wins)
  LOREVM_BIN  path to the ``lorevm`` binary (default: search PATH then the
              loregui ``target/{debug,release}`` dirs)
  LORE_OFFLINE  if set to 1/true, pass --offline to lorevm (handy for purely
              local repos with no remote configured)
  LORE_IDENTITY optional identity passed to lorevm (--identity)

Usage:
  python server.py            # stdio MCP server (default)
  python server.py --list     # print the tool catalog and exit (smoke test)
  python server.py --sse      # SSE mode on $SSE_PORT (default 11238)
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any, Optional

from mcp.server import Server
from mcp.server.stdio import stdio_server
import mcp.types as types

HERE = Path(__file__).resolve().parent
CATALOG_PATH = HERE / "lore-tools.json"
GENERATOR = HERE / "generate_catalog.py"

app = Server("lore")


# ---------------------------------------------------------------------------
# Catalog loading
# ---------------------------------------------------------------------------


def ensure_catalog() -> dict[str, Any]:
    """Load ``lore-tools.json``; generate it on the fly if it is missing."""
    if not CATALOG_PATH.exists():
        sys.stderr.write("lore-tools.json missing; generating from manifests...\n")
        subprocess.run([sys.executable, str(GENERATOR)], check=True)
    with open(CATALOG_PATH) as f:
        return json.load(f)


CATALOG = ensure_catalog()
# op_id -> tool record, and tool_name -> tool record.
TOOLS_BY_NAME: dict[str, dict[str, Any]] = {t["tool_name"]: t for t in CATALOG["tools"]}


# ---------------------------------------------------------------------------
# lorevm subprocess plumbing
# ---------------------------------------------------------------------------


def find_lorevm_bin() -> Optional[str]:
    """Locate the ``lorevm`` binary: LOREVM_BIN, then PATH, then loregui target."""
    env = os.environ.get("LOREVM_BIN")
    if env and Path(env).exists():
        return env
    on_path = shutil.which("lorevm")
    if on_path:
        return on_path
    loregui = Path(os.environ.get("LOREGUI_DIR", str(HERE.parent)))
    for profile in ("release", "debug"):
        cand = loregui / "target" / profile / "lorevm"
        if cand.exists():
            return str(cand)
    return None


def _truthy(value: Optional[str]) -> bool:
    return str(value).lower() in ("1", "true", "yes", "on") if value else False


def run_lorevm(op_id: str, repo: str, args: dict[str, Any]) -> dict[str, Any]:
    """Invoke ``lorevm <op_id> --dir <repo> --args <json>`` and return parsed JSON.

    Returns the op's JSON result on success, or ``{"error": {...}}`` describing
    the failure (the same shape lorevm itself emits) so the model always gets
    structured output.
    """
    binary = find_lorevm_bin()
    if not binary:
        return {
            "error": {
                "kind": "config",
                "message": (
                    "lorevm binary not found. Set LOREVM_BIN to its path, put it "
                    "on PATH, or build it: `cargo build -p lorevm-cli` in the "
                    "loregui repo (binary lands in target/debug/lorevm)."
                ),
            }
        }
    if not repo:
        return {
            "error": {
                "kind": "config",
                "message": "no repository directory: set LORE_REPO or pass a `repo` arg.",
            }
        }

    cmd = [binary, op_id, "--dir", repo, "--args", json.dumps(args)]
    if _truthy(os.environ.get("LORE_OFFLINE")):
        cmd.append("--offline")
    identity = os.environ.get("LORE_IDENTITY")
    if identity:
        cmd.extend(["--identity", identity])

    try:
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=120,
        )
    except subprocess.TimeoutExpired:
        return {"error": {"kind": "timeout", "message": f"{op_id} timed out after 120s"}}
    except OSError as e:
        return {"error": {"kind": "exec", "message": f"failed to launch lorevm: {e}"}}

    out = (proc.stdout or "").strip()
    if out:
        try:
            return json.loads(out)
        except json.JSONDecodeError:
            # Non-JSON on stdout — return raw, attaching stderr for context.
            return {
                "error": {
                    "kind": "parse",
                    "message": "lorevm produced non-JSON output",
                    "stdout": out,
                    "stderr": (proc.stderr or "").strip(),
                }
            }
    # No stdout — surface stderr / exit code.
    return {
        "error": {
            "kind": "exec",
            "message": (proc.stderr or "").strip() or f"lorevm exited {proc.returncode}",
        }
    }


def resolve_repo(arguments: dict[str, Any]) -> str:
    """A call's ``repo`` arg overrides the LORE_REPO env default."""
    return arguments.get("repo") or os.environ.get("LORE_REPO", "")


def strip_meta_args(arguments: dict[str, Any]) -> dict[str, Any]:
    """Drop the MCP-level ``repo`` key before forwarding args to the op."""
    return {k: v for k, v in arguments.items() if k != "repo"}


# ---------------------------------------------------------------------------
# MCP tool registration
# ---------------------------------------------------------------------------

SUMMARY_TOOL = types.Tool(
    name="lore_repo_summary",
    description=(
        "Aggregate snapshot of a lore repository: current branch/revision from "
        "status, branch count, and the most recent revisions. A cheap one-call "
        "overview (the repo 'metrics' dashboard)."
    ),
    inputSchema={
        "type": "object",
        "properties": {
            "repo": {
                "type": "string",
                "description": (
                    "Path to the lore repository working directory. "
                    "Overrides the LORE_REPO env var for this call."
                ),
            },
            "history_length": {
                "type": "number",
                "description": "How many recent revisions to include (default 10).",
                "default": 10,
            },
        },
    },
)


def build_tools() -> list[types.Tool]:
    tools = [
        types.Tool(
            name=t["tool_name"],
            description=t["description"],
            inputSchema=t["input_schema"],
        )
        for t in CATALOG["tools"]
    ]
    tools.append(SUMMARY_TOOL)
    return tools


@app.list_tools()
async def list_tools() -> list[types.Tool]:
    return build_tools()


def _as_text(payload: Any) -> list[types.TextContent]:
    return [types.TextContent(type="text", text=json.dumps(payload, indent=2))]


@app.call_tool()
async def call_tool(name: str, arguments: dict[str, Any]) -> list[types.TextContent]:
    arguments = arguments or {}

    if name == "lore_repo_summary":
        return _as_text(await asyncio.to_thread(handle_repo_summary, arguments))

    tool = TOOLS_BY_NAME.get(name)
    if not tool:
        return _as_text({"error": {"kind": "unknown_tool", "message": name}})

    repo = resolve_repo(arguments)
    op_args = strip_meta_args(arguments)
    # lorevm is blocking; run it off the event loop.
    result = await asyncio.to_thread(run_lorevm, tool["op_id"], repo, op_args)
    return _as_text(result)


def handle_repo_summary(arguments: dict[str, Any]) -> dict[str, Any]:
    """Aggregate a few cheap read ops into one snapshot."""
    repo = resolve_repo(arguments)
    length = int(arguments.get("history_length") or 10)

    status = run_lorevm("repository.status", repo, {})
    branches = run_lorevm("branch.list", repo, {})
    history = run_lorevm("revision.history", repo, {"length": length})

    summary: dict[str, Any] = {"repo": repo}

    rev = status.get("revision") if isinstance(status, dict) else None
    if isinstance(rev, dict):
        summary["current_branch"] = rev.get("branch_name")
        summary["current_revision"] = rev.get("revision")
        summary["revision_number"] = rev.get("revision_number")
    summary["pending_changes"] = (
        len(status.get("files", [])) if isinstance(status, dict) else None
    )

    if isinstance(branches, dict) and "entries" in branches:
        summary["branch_count"] = branches.get("count", len(branches["entries"]))
        summary["branches"] = [b.get("name") for b in branches["entries"]]
    else:
        summary["branches_error"] = branches.get("error") if isinstance(branches, dict) else None

    if isinstance(history, dict) and "entries" in history:
        summary["recent_revision_count"] = len(history["entries"])
        summary["recent_revisions"] = history["entries"][:length]
    else:
        summary["history_error"] = history.get("error") if isinstance(history, dict) else None

    return summary


# ---------------------------------------------------------------------------
# Entry points
# ---------------------------------------------------------------------------


async def run_stdio() -> None:
    async with stdio_server() as (read_stream, write_stream):
        await app.run(read_stream, write_stream, app.create_initialization_options())


def run_sse(port: int) -> None:
    from mcp.server.sse import SseServerTransport
    from starlette.applications import Starlette
    from starlette.responses import JSONResponse
    from starlette.routing import Route
    import uvicorn

    sse = SseServerTransport("/mcp/messages")

    async def handle_sse(request):
        async with sse.connect_sse(request.scope, request.receive, request._send) as s:
            await app.run(s[0], s[1], app.create_initialization_options())

    async def handle_messages(request):
        await sse.handle_post_message(request.scope, request.receive, request._send)

    async def health(_request):
        return JSONResponse({"status": "ok", "service": "lore-mcp"})

    starlette_app = Starlette(
        routes=[
            Route("/mcp/sse", endpoint=handle_sse),
            Route("/mcp/messages", endpoint=handle_messages, methods=["POST"]),
            Route("/health", endpoint=health),
        ]
    )
    print(f"Starting lore-mcp (SSE) on port {port}")
    uvicorn.run(starlette_app, host="0.0.0.0", port=port, log_level="info")


def print_catalog() -> None:
    """Smoke test: print every tool, its op id, and arg keys."""
    tools = build_tools()
    print(f"lore-mcp exposes {len(tools)} tools")
    print(f"  lorevm binary: {find_lorevm_bin() or 'NOT FOUND'}")
    print(f"  LORE_REPO: {os.environ.get('LORE_REPO', '(unset)')}")
    print()
    for t in tools:
        props = list((t.inputSchema or {}).get("properties", {}).keys())
        required = (t.inputSchema or {}).get("required", [])
        print(f"  {t.name}")
        print(f"    {t.description}")
        print(f"    args: {props}  required: {required}")
    print()


def main() -> int:
    parser = argparse.ArgumentParser(description="lore-mcp MCP server")
    parser.add_argument("--list", action="store_true", help="print the tool catalog and exit")
    parser.add_argument("--sse", action="store_true", help="run in SSE mode")
    parser.add_argument(
        "--port",
        type=int,
        default=int(os.environ.get("SSE_PORT", "11238")),
        help="port for SSE mode (default 11238 or $SSE_PORT)",
    )
    args = parser.parse_args()

    if args.list:
        print_catalog()
        return 0
    if args.sse:
        run_sse(args.port)
        return 0
    asyncio.run(run_stdio())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
