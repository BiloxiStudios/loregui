# Lore Source Control (StudioBrain) — Unreal Engine plugin

An Unreal Engine **editor** plugin that backs the editor's Revision Control
system with Epic's [Lore](https://github.com/EpicGames/lore) VCS, driving lore
**in-process** through StudioBrain's `lorevm-ffi` C ABI — **not** Epic's public
`lore` CLI, and not a subprocess.

> Status: **MVP (SBAI-4086)**. UE-build-pending: this tree is a complete, correct,
> internally-consistent plugin, but it has not been compiled by UE on this machine
> (no engine installed here). See "What is / isn't verified" below.

## What it does

Once enabled and selected as the editor's revision-control provider, the plugin
lights up the editor's native source-control surfaces against a lore repo:

- **Content Browser status overlays** — locked-by-me, locked-by-other (+ owner),
  modified/added, and out-of-date badges, from the lore status + lock state.
- **Check Out** (acquire a lore lock), **Check In** (commit + push), **Sync**
  (pull latest), **Revert** (release lock + unstage), **Mark For Add**, **Delete**.
- A status/connection summary in the Revision Control settings.

All of it runs over one warm `lorevm-ffi` handle (one tokio runtime + one
`LoreApi`) held for the editor session — the right cost curve for per-asset
overlay refresh (see `docs/ue-lorevm-bridge-spike.md` in the LoreGUI repo).

## Architecture (thin adapter over our stack)

```
Unreal Editor  ──ISourceControlProvider──>  FLoreSourceControlProvider
                                              │  (per-op Workers)
                                              ▼
                                   FLorevmFfi  (the thin FFI binding layer)
                                              │  loads liblorevm_ffi.{so,dylib,dll}
                                              │  open → call("<domain>.<op>", argsJson) → close
                                              ▼
                              crates/lorevm-ffi  (C ABI cdylib, SBAI-4081)
                                              ▼
                              lore_vm::dispatch → LoreApi + ops  →  Epic's `lore` crate
```

The only layer that knows lore op-ids + JSON shapes is the FFI binding
(`Ffi/LorevmFfi.*`) + the Workers (`LoreSourceControlOperations.cpp`) +
`LoreSourceControlUtils.cpp`. Everything above talks to `ISourceControlProvider`.
That isolation is deliberate: **re-targeting Epic's future first-party lore
provider is a provider-registration swap, not a rewrite** (see "Swappable design").

## File map

```
LoreSourceControl.uplugin                       plugin manifest (Editor module, UE 5.x)
Source/LoreSourceControl/
  LoreSourceControl.Build.cs                    deps + stages the cdylib as a RuntimeDependency
  Private/
    Ffi/lorevm_ffi.h                            C ABI mirror of crates/lorevm-ffi (hand-written)
    Ffi/LorevmFfi.h / .cpp                       the thin FFI binding: load lib, warm handle, call ops, parse JSON
    LoreSourceControlProvider.h / .cpp           FLoreSourceControlProvider : ISourceControlProvider
    LoreSourceControlState.h / .cpp              per-asset state + overlay icon mapping
    LoreSourceControlOperations.h / .cpp         per-op Workers (Connect/UpdateStatus/CheckOut/CheckIn/Sync/Revert/Add/Delete)
    LoreSourceControlCommand.h / .cpp            IQueuedWork unit run on the SC worker thread
    LoreSourceControlUtils.h / .cpp              path mapping + status/lock JSON → State
    LoreSourceControlModule.h / .cpp             module entry; registers provider + workers
    LoreSourceControlSettings.h / .cpp           ini-backed settings (lib path, in-memory/offline, identity)
    ILoreSourceControlWorker.h                   worker interface
    LoreSourceControlLog.h                       logs under LogSourceControl
docs/
  BUILD.md                                       how to build the cdylib + the plugin
  ARCHITECTURE.md                                the thin-adapter / swappable design, threading, ownership
```

## Quick start

1. **Build the cdylib** from the LoreGUI repo:
   `cargo build -p lorevm-ffi --release` → `target/release/liblorevm_ffi.{so,dylib,dll}`.
2. **Drop this plugin** into your UE project's `Plugins/` directory.
3. **Stage the library** at
   `Plugins/LoreSourceControl/Binaries/ThirdParty/LorevmFfi/<Platform>/<libname>`
   (or set the `LOREVM_FFI_LIB` env var to its path).
4. **Build** the project (the editor regenerates project files and compiles the
   plugin).
5. In the editor: **Revision Control → Connect**, pick **Lore (StudioBrain)**.

Full details, including how the FFI ABI is asserted and the threading/ownership
rules, are in [`docs/BUILD.md`](docs/BUILD.md) and
[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md).

## Swappable design (re-target Epic's provider later)

Epic is building a first-party lore source-control provider for UE. Because the
plugin is split into (1) the `ISourceControlProvider` adapter and (2) the thin
`FLorevmFfi` bridge, swapping later is a registration change: keep the editor
talking to `ISourceControlProvider`, drop our adapter, register Epic's — or keep
ours where StudioBrain wants richer DAM-aware behaviour than Epic exposes.

## Not in this MVP (future layers)

- **StudioBrain DAM / entity mapping** (UE asset ↔ lore path ↔ StudioBrain entity).
- **Tray / lock-messaging UX** (SBAI-4044) and the **relay** layer.
- File **history** panel population (the History accessors are stubbed).
- A rich settings Slate widget (settings are ini-driven for now).
- Changelists / shelving (lore's model is staging + locks, not changelists).

## Reference / credits

The provider/worker/state scaffolding shape follows Epic's own
GitSourceControl/PerforceSourceControl plugins and the MIT-licensed
[BenVlodgi/UE-LoreSourceControl](https://github.com/BenVlodgi/UE-LoreSourceControl)
(which shells the `lore` CLI). This plugin **adapts that shape** but replaces the
CLI bridge with our in-process `lorevm-ffi` C ABI. MIT licensed.
