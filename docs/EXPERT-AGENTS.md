# LoreGUI — Expert Agents & Skills (master list)

**Epic:** SBAI-4024 · **Status:** active

## Purpose

Every lore API endpoint — new or existing — must be integrated into the **full**
application coherently: command palette **and** panels, navigation/menus, buttons,
help, tutorials, and correct auth/storage/VCS behavior. Mechanical "op → palette
row" is not enough. This is achieved by a roster of **domain-expert agents** that
share **single sources of design truth** and are held to **coherence gates**.

## Where things live (so ALL agents load them)

Pipeline workers run `claude` inside a loregui repo worktree, so anything in the
repo is automatically available to **both** the BrainMon pipeline workers and
Claude Code. BrainMon holds only the thin worker wiring that points at the repo.

| Artifact | Location | Consumed by |
|---|---|---|
| Repo overview + mandate | `loregui/CLAUDE.md` | every agent in the repo (auto-loaded) |
| Design system | `docs/DESIGN-SYSTEM.md` | all UI work |
| Information architecture | `docs/INFORMATION-ARCHITECTURE.md` | all UI work |
| Per-domain expert guides | `docs/domains/<domain>.md` | the domain's tickets |
| Expert subagents | `.claude/agents/loregui-*.md` | Claude Code + repo workers (spawn) |
| Skills | `.claude/skills/*/SKILL.md` | Claude Code + repo workers (invoke) |
| Pipeline worker wiring | `/opt/BrainMon/agents/loregui-domain-worker.md` | BrainMon dispatch |

## Agents

| Agent | File | Owns / expertise |
|---|---|---|
| **ux-designer** | `.claude/agents/loregui-ux-designer.md` | design system, information architecture, accessibility, copy voice; the **"does this make sense?" reviewer** on every domain PR |
| **frontend-engineer** | `.claude/agents/loregui-frontend-engineer.md` | React 19 + Tauri v2 + TS patterns; palette, panels, forms, state, error handling, the `api.ts` seam |
| **auth-expert** | `.claude/agents/loregui-auth-expert.md` | auth domain; login/session/token flows; providers (interactive, token, OAuth/SSO); the accounts security boundary |
| **storage-expert** | `.claude/agents/loregui-storage-expert.md` | storage + shared_store; backends (local/S3/MinIO/Garage); content-addressed model; onboarding storage flow |
| **vcs-domain-expert** | `.claude/agents/loregui-vcs-domain-expert.md` | branch/revision/file/lock/link/layer/dependency — the lore VCS mental model so UIs match real behavior |
| **docs-writer** | `.claude/agents/loregui-docs-writer.md` | in-app help, empty states, tutorials, how-tos, the website docs |
| **integration-manager** | `.claude/agents/loregui-integration-manager.md` | decompose work, assign experts, run gates, merge PRs in registry order |

## Skills

| Skill | Dir | What it does |
|---|---|---|
| **integrate-endpoint** | `.claude/skills/integrate-endpoint/` | the full vertical for one op: lore-vm binding → `#[tauri::command]` → palette entry → panel/menu if warranted → help + tutorial → design review |
| **add-domain-ui** | `.claude/skills/add-domain-ui/` | build a domain's whole UI surface: panel, nav entry, palette entries, help — coherently, per the IA |
| **design-review** | `.claude/skills/design-review/` | run the ux-designer coherence checklist over a diff; pass/fail with specifics |
| **write-tutorial** | `.claude/skills/write-tutorial/` | generate a guided how-to / in-app help for a flow |
| **palette-entry** | `.claude/skills/palette-entry/` | add one op's palette manifest entry (the mechanical Phase-2 unit; formalizes `palette/README.md`) |

## Coherence gates (CI + review)

Beyond the **palette parity ratchet** (`frontend/scripts/palette-parity.mjs`):

1. **IA ratchet** — every op declares a `surface` (`panel` | `menu` | `palette`)
   and every domain has a navigation entry. (`scripts/ia-parity.mjs`, planned.)
2. **Help ratchet** — every palette manifest entry has a non-empty `description`;
   multi-step flows link a tutorial. (extend the parity script.)
3. **Design review** — `loregui-ux-designer` is a required reviewer on every
   domain PR; the `design-review` skill encodes the checklist.

## Ticket-reference protocol (how a worker uses this)

Every LoreGUI ticket body must include a **Build context** block, e.g.:

> **Build context:** domain=`storage`. Read `docs/domains/storage.md`,
> `docs/DESIGN-SYSTEM.md`, `docs/INFORMATION-ARCHITECTURE.md`. Use the
> `integrate-endpoint` skill. Spawn `loregui-storage-expert` for behavior and
> `loregui-ux-designer` for the design review before opening the PR. Acceptance:
> palette-parity + IA + help gates green, design review passed.

`CLAUDE.md` makes this mandatory so even tickets that forget the block are held to
it. The BrainMon `loregui-domain-worker` injects the block from the ticket's
domain label when missing.

## Rollout

1. Foundation: this doc + CLAUDE.md + DESIGN-SYSTEM + INFORMATION-ARCHITECTURE +
   all agents + all skills.
2. Template: ship **storage** as a complete coherent slice.
3. Iterate every domain until all coherence gates pass.
