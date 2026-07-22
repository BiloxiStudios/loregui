# LoreGUI Standalone Context UX Design

**Status:** Approved by bizanator on 2026-07-21. P0 product-code delivery is
merged; P1 and P2 remain implementation plans governed by the StudioBrain trio.

**Tickets:** SBAI-5483, SBAI-5484, SBAI-5457

**Original baseline:** LoreGUI `d2d688108f0bb62eb88204444d4d1bcdf117e83b`

**Current durable baseline:** LoreGUI `746f343d0c28d7e06133ed474490d9363df31d80`.
P0 landed in `06a4378c83012fe43bf4307a6ecfd54fc629c482` (#420).
The installed-Windows/EROS acceptance packet for SBAI-5483 is still required;
these documents do not claim that runtime gate is complete.

## Product boundary

LoreGUI is a standalone Tauri v2 + React product. A user can create or open a
local repository, host a local Lore server, discover a LAN server, or connect
to a manually entered Lore server without a StudioBrain account.

StudioBrain is an optional premium integration. It can provide federation,
managed deployment, and other paid capabilities, but it is never the parent of
local repositories or ordinary Lore server connections. LoreGUI is not being
rewritten in Dioxus and is not embedded as a replacement UI inside StudioBrain.

The approved standalone setup model is:

1. A fresh profile starts with the existing **Set Up** or **Host a Server**
   decision.
2. An onboarded profile with no valid repository shows the four-action
   **Choose a project** hub: **Open existing**, **Create local**, **Connect**,
   or **Host**.
3. **Connect StudioBrain (Premium)** remains a visually secondary, optional
   integration entry. It is not required for local, LAN, or manual Lore use.

## Problem statement

At the original baseline, the application did not have a safe repository root
or a durable product model for the context in which an operation runs:

```text
Server -> Repository -> Local workspace -> Branch/revision -> Identity/auth mode
```

At that baseline, those concepts were held in separate transient components.
`AppState` started `working_dir` from the process current directory, which was
`C:\Users\Biza\AppData\Local\LoreGUI` in the installed EROS build. Hosting a
server at `E:\lore` prepares the server store but neither creates nor opens a
client repository. The wizard could still finish, and repository actions such
as Sync remained enabled. The reported AppData error was therefore
deterministic.

P0 (#420) removed that direct path-binding hazard: active repository state is
nullable and validated, repository actions fail closed, all seven real local
directory fields use the native picker, onboarding completion is owned by the
current flow, and hosting remains distinct from opening a client repository.

The remaining P1 product gaps are:

- no versioned saved/recent/favorite server and repository catalog;
- no persistent selected server, repository, local path, branch, or identity
  chain across all product surfaces;
- no visible distinction between connected, no-auth, offline, and auth-required;
- `List Repos` re-prompts for a URL and produces display-only results;
- the top bar exposes nearly every operation as a peer action;
- no context-level activity surface that keeps operation outcomes visible.

## Research-derived principles

The design combines the useful parts of established tools without copying
their density:

- **P4V:** an explicit server, identity, and workspace tuple; named favorites;
  an always-visible current workspace.
- **Unity Version Control / Plastic:** repository browsing leads to creating or
  opening a named workspace at a chosen filesystem location.
- **Anchorpoint:** an artist-friendly project and file-browser surface with a
  saved remote association and clear operation semantics.
- **GitKraken:** local workspaces remain complete and on-device while cloud
  integrations stay optional.
- **Drive/Dropbox:** operation status and failures remain visible instead of
  disappearing in transient toasts.

The result is **P4V-clear, not P4V-dense**.

## Terminology

The UI uses these terms consistently:

- **Server:** a Lore service endpoint or a server hosted on this device.
- **Repository:** a versioned Lore repository available locally or from a
  server.
- **Local path:** the filesystem root of the working copy used by commands.
- **Project:** a friendly LoreGUI bookmark that binds a repository to a local
  path and optional server profile. It is not a new Lore backend primitive.
- **Branch:** the active repository branch.
- **Identity:** the selected identity when authentication is required.
- **Authentication: Not required:** the complete and healthy state of a
  no-auth server. This is not an error or incomplete login.

Avoid using “workspace” alone in customer copy because P4, Git, and Lore users
give it different meanings. Use **Local path** in the persistent context chain
and **Project** for the user-facing bookmark.

## Architecture

### Persisted non-secret context

Extend application settings with versioned, non-secret records:

```rust
pub struct ServerProfile {
    pub id: String,
    pub alias: String,
    pub url: String,
    pub source: ServerSource,
    pub favorite: bool,
    pub auth_mode: AuthMode,
    pub credential_ref: Option<String>,
    pub last_seen_at: Option<String>,
}

pub struct RepositoryBookmark {
    pub id: String,
    pub server_id: Option<String>,
    pub display_name: String,
    pub url: Option<String>,
    pub favorite: bool,
}

pub struct LocalProject {
    pub id: String,
    pub repository_id: String,
    pub display_name: String,
    pub local_path: String,
    pub branch: Option<String>,
    pub favorite: bool,
    pub last_opened_at: String,
}

pub struct HostedServerProfile {
    pub id: String,
    pub display_name: String,
    pub store_path: String,
    pub advertised_url: String,
    // UTC timestamp metadata only; raw launch configuration is never persisted.
    pub last_configured_at: String,
}

pub struct ActiveContext {
    pub project_id: Option<String>,
    pub server_id: Option<String>,
    pub identity_ref: Option<String>,
}
```

The exact serialization uses the existing Tauri settings store. Secrets and
tokens remain in OS-secure credential storage; settings contain only opaque
references. Loading settings is fail-closed: malformed context is ignored with
an actionable warning, never converted into the process current directory.

### Runtime context

Repository commands consume a validated active local project. The process
current directory is not a repository fallback. At startup:

1. restore `ActiveContext.project_id`;
2. resolve its `LocalProject.local_path`;
3. validate it with repository status;
4. expose repository actions only after validation;
5. otherwise render the project hub with the stale entry marked unavailable.

Hosting a server and opening a repository remain separate states. Successful
hosting ends at a review screen with explicit next actions:

- **Browse server repositories**;
- **Create a repository**;
- **Clone to this device**;
- **Open an existing local project**;
- **Finish and manage server only**.

The wizard must never imply that a server store directory is itself a client
working copy.

### Server repository browser

`List Repos` becomes **Browse server**. It opens the selected server profile;
it never invokes `window.prompt`.

The browser contains:

- saved and favorite servers;
- recent servers;
- LAN-discovered servers;
- servers hosted on this device;
- manual Add Server;
- health and auth-mode status for each server;
- repositories underneath the selected server.

Every repository row offers:

- **Open Existing** — bind a validated local path;
- **Clone** — choose a native folder and create a local project;
- **Favorite / Unfavorite**;
- **Details** — URL, default branch, last activity, and server association.

Repository-list errors stay attached to the selected server card and include a
retry action. They are not silently dropped.

### Persistent context chain

The main shell always shows:

```text
Server or Local  ->  Project / Repository  ->  Local path  ->  Branch  ->  Status
```

Example:

```text
EROS Lore · No auth  ->  Game Lore  ->  E:\lore\game-lore  ->  main  ->  Up to date
```

Each segment opens its relevant switcher or details panel. The status segment
distinguishes Connected, Reconnecting, Offline, Auth required, Local only,
Dirty, Incoming changes, Outgoing commits, Conflict, and Operation failed.

### Shell information architecture

The top bar contains only:

- the context switcher;
- current operation/status;
- one contextual primary action;
- search / command palette;
- overflow.

Daily domains move into the existing documented sidebar model: Files, Changes,
Branches, History, Locks, and Activity. Rare or administrative operations move
under Manage: Verify, GC, Flush, Metadata, Dependencies, hosted-server
lifecycle, Settings, and Integrations.

`Sync` is renamed **Get latest** or **Update workspace** until LoreGUI provides
an atomic, previewed composite operation. Push remains separate. The UI names
the repository and local path before any operation.

### Optional StudioBrain premium path

The premium onboarding card appears after the three standalone choices. It
uses copy such as:

> Connect StudioBrain (Premium)
>
> Add managed servers, federation, and StudioBrain integrations. Local and LAN
> Lore projects do not require an account.

StudioBrain authentication is scoped only to premium capabilities. Signing out
must not hide, disable, or orphan standalone local projects and ordinary Lore
server profiles.

## P0 correctness delivery — SBAI-5483 (product code delivered)

P0 product-code acceptance landed on durable main in #420 / `06a4378c`:

1. Process CWD is not considered an active repository.
2. Repository-scoped actions cannot invoke IPC without a validated repository.
3. No-repository state renders guided Open, Create, Connect, and Host actions.
4. Host completion explicitly separates server store from local project.
5. Native folder pickers cover server store, mutable store, clone destination,
   and Open Existing.
6. Continue and Finish are gated by child-step completion.
7. Hosted-server identity and status survive leaving onboarding.
8. The automated Windows boundary proves commands do not fall back to AppData
   or process CWD.

Operational closure is separate: the real EROS installer/restart/screenshots
and selected-path evidence remain the SBAI-5483 close gate. P0 being merged
must not be read as a claim that this installed-runtime packet has passed.

## P1 context and browser delivery — SBAI-5484 (remaining scope)

P1 is not implemented by #420. It adds the persisted context model,
saved/recent/favorite servers, the server repository browser, current
context chain, shell IA, activity surface, and optional premium onboarding
path. It migrates existing valid settings without inventing a project from the
launch directory.

## P2 deterministic agentic QA — SBAI-5457 (separate future scope)

The full-surface loop is state-driven rather than an unrestricted “click every
button” agent.

Each machine-readable action record includes:

```json
{
  "surface_id": "server-browser.repository.clone",
  "state_id": "local-host-no-auth",
  "element_id": "server.repository.clone",
  "preconditions": ["fixture-owned-server", "repository-listed"],
  "risk": "write_reversible",
  "expected_ipc": ["repository_clone"],
  "oracles": ["dom", "ipc", "state", "filesystem", "screenshot"],
  "cleanup": "delete fixture-owned clone root"
}
```

Qwen receives only allowed action IDs and fixture-bound parameters. It cannot
invent paths, URLs, selectors, or coordinates. Screenshots are supporting
evidence; IPC, application state, filesystem state, and remote-client
observation are the correctness oracles. Destructive actions default-deny
unless a per-run ownership token proves the target belongs to the fixture.

Required fixtures include fresh/no-repo, local clean/dirty, hosted no-auth,
remote two-user no-auth, remote auth, host running, corrupt repo, network loss,
pending lock request, and persisted settings. Windows UI Automation covers
native dialogs, tray, notifications, installer, and updater surfaces that DOM
WebDriver cannot control.

## Documentation and website parity

Every major UX PR includes a local-Qwen parity report covering:

- `docs/INFORMATION-ARCHITECTURE.md`;
- website connect, host, guide, panels, model, and premium pages;
- screenshots and mockups;
- in-product onboarding and account copy;
- standalone versus optional-premium claims.

The parity report is advisory while the first implementation lands, then
becomes a deterministic CI gate over a versioned checklist. A major UI PR
cannot close with stale screenshots, references to StudioBrain as a connection
prerequisite, raw `List Repos` instructions, or undocumented context behavior.

## Acceptance journeys

These are program-level journeys, not claims about the current runtime. P0
code covers the fail-closed repository/path foundation; P1 owns the saved
context and browser journeys; P2 owns repeatable Qwen/Windows surface evidence.

1. **Fresh standalone local:** create/open a local project with a native folder
   picker, restart, and restore the same validated project.
2. **Host no-auth:** select `E:\lore` as the store, start the server, see its
   name/URL/store/status, browse its repositories, create or clone a project,
   and perform a round trip without a StudioBrain account.
3. **LAN no-auth:** discover a server, save/favorite it, browse repositories,
   clone, restart, and reconnect with “Authentication: Not required.”
4. **Authenticated Lore:** connect, show the selected identity, reject expired
   credentials honestly, and preserve unrelated local projects.
5. **StudioBrain premium:** optional sign-in adds premium services without
   changing standalone project ownership.
6. **No repository:** every repository action is disabled or absent with an
   explanatory next action; no IPC command targets AppData.
7. **Offline:** local work remains available, remote actions are gated, and the
   persistent status explains reconnection.
8. **Switch context:** favorite server/repository/project switching updates
   every operation target and never reuses stale state.

All journeys require automated behavior tests plus an installed-Windows EROS
evidence packet containing the installer and binary SHA, before/after
screenshots, redacted IPC/action log, selected context, filesystem proof, and
restart persistence.

## Non-goals

- rewriting LoreGUI in Dioxus or another UI framework;
- requiring StudioBrain authentication for standalone use;
- treating a server store as a local working copy;
- adding a new Lore backend “Project” primitive;
- implementing opaque automatic bidirectional sync;
- copying P4V's dense admin-first interface;
- allowing a visual agent to guess destructive targets.
