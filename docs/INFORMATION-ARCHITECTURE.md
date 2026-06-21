# LoreGUI — Information Architecture

Where every op lives in the app, and the rule for choosing its surface(s).

## Top-level structure

```
┌ Top bar (navigation surface) ───────────────────────────────┐
│ LoreGUI · <repo>            ⌘K  Theme  Sync  Push  Verify    │
├──────────────┬──────────────────────────────────────────────┤
│ Sidebar nav  │  Main work area (domain panels)              │
│  Changes     │   (status / staging / history / diff …)      │
│  Branches    │                                              │
│  History     │                                              │
│  Locks       │                                              │
│  Storage     │                                              │
│  …per domain │                                              │
└──────────────┴──────────────────────────────────────────────┘
Command palette (⌘K) overlays everything — runs ANY op.
First run → OnboardingFlow (client connect | host setup).
```

- **Top bar** = global actions + identity + repo + ⌘K + Theme.
- **Sidebar** = one entry per **primary domain** the user works in daily
  (Changes, Branches, History, Locks, Storage, Links, Layers). Secondary/admin
  domains (service, dependency, repository-admin) live under a **Settings/Manage**
  area, not the main sidebar.
- **Command palette** = the universal surface; **every** op is here (parity gate).
- **Per-domain panel** = the rich home for a domain's common workflow.

## Choosing an op's surface(s)

Each op declares a `surface` in its palette manifest (`surface?: "panel" | "menu"
| "palette"`, default `palette`). Decision rule:

| If the op is… | Surface(s) |
|---|---|
| part of the **daily core loop** (status, stage, commit, branch switch, sync, history, diff, lock acquire/release) | **panel** (primary, rich UI) **+ palette** |
| a **per-entity action** (branch protect, file obliterate, revision revert, lock release) | **menu** (context/row action on the entity) **+ palette** |
| **occasional / admin** (repository gc/flush/delete, instance prune, service start/stop, dependency edit, metadata get/set) | **menu** under Settings/Manage **+ palette** |
| **rare / power-user / scriptable** (store_immutable_query, verify_fragment, storage put/get, config_get) | **palette only** |
| **streaming / non-request-response** (notification subscribe/unsubscribe) | neither — wired into live UI, `excluded` from palette |

Every op is **at least** in the palette. Panels and menus are added when the rule
above warrants — coherently, not one-off.

## Per-domain placement (summary)

| Domain | Primary surface | Notes |
|---|---|---|
| repository | top bar (open/clone) + Settings/Manage panel | status is its own Changes panel |
| branch | **Branches** panel + row menus | create/switch/list/merge/protect |
| revision | **History** panel + revision row menus | commit via Changes; diff/info/revert/restore |
| file | **Changes** panel (staging) + file row menus | stage/unstage/dirty/obliterate/diff/history |
| lock | **Locks** panel + file row menu | acquire/release/query/status |
| link | **Links** panel | add/remove/update/list |
| layer | **Layers** panel | add/remove/list |
| dependency | file detail / Settings | add/remove/list |
| storage | **Storage** panel (top-bar nav entry) + onboarding | backends, connectivity, fragments, flush; see `docs/domains/storage.md` |
| shared_store | Storage panel / onboarding | create/info/set_use_automatically |
| auth | top bar identity menu + onboarding | login/logout/user_info/providers |
| service | Settings/Manage | start/stop |
| notification | live (toasts/badges) | not in palette |

## Adding to the IA

When a domain gets its panel, add a **sidebar/Settings entry** (navigation
surface) and route it. The IA ratchet (planned `scripts/ia-parity.mjs`) checks
every domain with `surface: panel` ops has a nav entry, and every manifest entry
has a valid `surface`. Update this doc when structure changes.
