# Contributing to LoreGUI

Thanks for your interest in LoreGUI — a fast, themeable, cross-platform desktop
GUI for [Lore](https://github.com/EpicGames/lore), Epic Games' next-generation
version control. This guide covers how to build, test, and land changes.

> **Community project.** Not affiliated with or endorsed by Epic Games. "Lore"
> is a trademark of Epic Games, Inc. Licensed under MIT.

> **Agents:** read [`.claude/skills/loregui/SKILL.md`](.claude/skills/loregui/SKILL.md) —
> it's the single entry point to install, set up the MCP, configure, and drive
> LoreGUI.

## Repository layout

| Path | What |
|---|---|
| `crates/lore-vm/` | Reusable, GUI-agnostic core. Binds the upstream `lore` crate in-process; one file per operation. |
| `crates/lorevm-cli/` | `lorevm` CLI — the external-driver seam (used by `lore-mcp` and the VS Code extension). |
| `crates/lorevm-ffi/` | C-ABI bridge (`lorevm-ffi`) for native consumers such as the Unreal Engine plugin. |
| `src-tauri/` | Tauri v2 desktop shell. One `#[tauri::command]` per operation. |
| `frontend/` | The GUI (Vite + React 19 + TypeScript). Per-domain panels + the universal command palette. |
| `lore-mcp/` | MCP server exposing lore ops as agent tools (one tool per op). |
| `vscode-extension/` | The `loregui-lore` VS Code extension (shells out to `lorevm`). |
| `website/` | Marketing landing site (Next.js) for [loregui.com](https://loregui.com). |
| `docs/` | The full-parity build plan, per-domain design notes, and ADRs. |

`lore-vm` is intentionally decoupled from the GUI so it can be embedded in larger
tooling. See [`docs/IMPLEMENTATION-PLAN.md`](docs/IMPLEMENTATION-PLAN.md) for the
full architecture and parity roadmap.

## Prerequisites

- **Rust** — stable toolchain, edition-2024-capable, with `clippy` and `rustfmt`.
- **Node.js 20+** (for `frontend/` and `vscode-extension/`).
- **Tauri v2 system dependencies** for your platform — see the
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
- On Debian/Ubuntu, the core build also needs `build-essential` and `pkg-config`.

## The upstream `lore` pin (read this first)

LoreGUI binds the upstream `lore` Rust crate **directly, in-process** — it never
shells out to the CLI and never runs a background daemon. The crate is pinned to
an **exact git revision** in the root [`Cargo.toml`](Cargo.toml):

```toml
lore = { git = "https://github.com/EpicGames/lore.git", rev = "<sha>" }
```

A couple of consequences you need to know:

- **`quinn-proto` patch is mandatory.** Upstream `lore` patches `quinn-proto` to a
  vendored fork (it adds `TransportErrorCode::is_crypto()`). A Cargo `[patch]`
  does **not** propagate through a git dependency, so the root `Cargo.toml`
  re-declares the same `[patch.crates-io]` at the same rev. Without it,
  `lore-transport` fails to compile (`no method named is_crypto`). Every consumer
  of the `lore` crate needs this patch.
- **The pin is manager-owned.** Lore is pre-1.0; its op surface and signatures
  can change between revisions. Bumping the pin is a **deliberate, standalone PR**
  — it is not something to fold into a feature change. The scheduled
  `upstream-parity` workflow watches for drift and proposes bumps. If you hit a
  signature mismatch against the pinned rev, that's an upstream-coupling issue,
  not a bug in your op — flag it rather than working around it.

## Building

```bash
# Frontend deps (first time / after package.json changes)
npm --prefix frontend install

# Hot-reload desktop development (from the repo root)
cargo tauri dev

# Full platform installer
cargo tauri build
```

The first build compiles the upstream `lore` crate (and the vendored
`quinn-proto`), so expect it to be slow the first time. `Swatinem/rust-cache`
keeps CI fast; locally, the `target/` cache does the same.

## Running tests

| Surface | Command |
|---|---|
| Core engine (fast unit tests) | `cargo test -p lore-vm` |
| Format check | `cargo fmt --all --check` |
| Lints | `cargo clippy -p lore-vm -- -D warnings` |
| Tauri shell compiles | `cargo check -p loregui` |
| Frontend type-check + build | `npm --prefix frontend run build` |
| Frontend unit tests | `npm --prefix frontend test` |
| Command-palette parity | `node frontend/scripts/palette-parity.mjs` |
| VS Code extension E2E | `npm --prefix vscode-extension test` |

Heavier suites, gated behind the `integration-tests` feature, drive the **real**
in-process `lore` engine against temp repos:

```bash
# Happy-path roundtrip (create → stage → commit → status → branch → history)
cargo test -p lore-vm --features integration-tests --test integration_roundtrip

# Full revision lifecycle + two-repo shared-store sync (non-skippable in CI)
cargo test -p lore-vm --features integration-tests --test e2e_lifecycle
```

The VS Code extension E2E (`@vscode/test-electron` + mocha) builds the real
`lorevm` engine, seeds a `.lore` repo, launches a headless VS Code, and drives
the extension's SCM provider / commands / tree views. Its `pretest` step compiles
and seeds a workspace automatically.

## CI gates

Pull requests must pass these checks before merge (see
[`.github/workflows/`](.github/workflows/)):

| Workflow | What it proves |
|---|---|
| `ci.yml` → `core-check` | `cargo fmt --all --check`, `cargo check -p lore-vm`, `cargo clippy -p lore-vm -D warnings`, `cargo test -p lore-vm`. |
| `ci.yml` → `palette-parity` | Every registered Tauri command is exposed in the command palette (a manifest entry) or explicitly allowlisted — the GUI stays in lock-step with the API surface. |
| `integration.yml` | The `integration_roundtrip` and non-skippable `e2e_lifecycle` suites against a real on-disk `lore` instance (runs when `crates/lore-vm/**` changes). |
| `vscode-test.yml` | Headless VS Code E2E for the extension (runs when the extension or the engine it shells out to changes). |
| `windows-build.yml` | Proves the Windows installer (Tauri → NSIS + MSI) still builds. |

`release.yml` (multi-platform installers) and `publish-vscode.yml` / `upstream-parity.yml`
run on pushes/tags/schedules, not as PR gates.

> Note: `main` is **not** branch-protected, so green CI is on you — please don't
> merge red.

## Branch & PR conventions

- **Branch names** describe the change; ticketed work uses the Jira key, e.g.
  `SBAI-XXXX-<domain>-<op>` or `docs/<slug>`.
- **PR titles** lead with the ticket where there is one: `SBAI-XXXX: <summary>`.
- **One op = one file per layer.** Palette manifest entries auto-discover via
  `import.meta.glob` — there are no index files to edit. Do **not** reformat or
  touch files outside the scope of your change; PRs that do are bounced.
- **Adding or exposing a lore op is more than a palette row.** Every operation
  must land in the full app coherently — decide its surface (panel, nav/menu,
  and/or palette entry), use the theme's semantic surface tokens (never hardcode
  colors), and add help/description text. See the coherence mandate in
  [`CLAUDE.md`](CLAUDE.md) and the design docs under `docs/`.
- Run the full verify set locally before opening a PR:
  `cargo check -p loregui`, `cargo fmt --all --check`,
  `npm --prefix frontend run build`, `node frontend/scripts/palette-parity.mjs`.

## Reporting bugs & requesting features

Use the issue templates under
[`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/). For security issues, do
**not** open a public issue — follow [`SECURITY.md`](SECURITY.md).

## Code of Conduct

By participating, you agree to abide by our
[Code of Conduct](CODE_OF_CONDUCT.md).

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](LICENSE).
