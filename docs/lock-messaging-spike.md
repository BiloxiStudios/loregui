# Lock-coordination messaging — transport spike (SBAI-4044)

**Goal:** let a user ask the current lock holder of a file to check it in /
release, delivered to the holder's client as a **tray notification** ("X is
asking you to check in `<file>`") plus an inbox entry with Release / Dismiss.

This is a SPIKE + MVP. The crux is **transport**: how does a request reach
*another user's* client? This document is the spike. The MVP that was built on
top of its conclusion is summarised at the end.

---

## TL;DR — transport verdict

| Layer | Can it carry an arbitrary user→user message? |
|-------|----------------------------------------------|
| lore **gRPC proto** (`lore.notification`) | **Yes** — `NotificationServiceClient::publish` + `Event::Other(ExtensionEvent { type, payload: Any })`. |
| lore **high-level crate** (`lore::notification`, what lore-vm binds) | **No** — only `subscribe`/`unsubscribe`; no `publish`; `ExtensionEvent` is **dropped** before it becomes a `LoreEvent`. |
| **Conclusion** | lore-vm (in-process, never shells out / never FFI) **cannot** send or receive arbitrary lock-request messages today. The capability exists one layer down in the protocol but is not surfaced by the crate API the GUI is allowed to bind. |

So the cross-network delivery path requires either (a) upstream lore exposing
`publish`/`ExtensionEvent` in its high-level crate, or (b) a small **server
side-channel** (the relay, SBAI-4072 — premium). Neither is in scope for a
core/MIT MVP. The MVP therefore delivers **locally** (same machine / same app
state) and leaves a clearly-marked seam for the relay.

---

## 1. lore's `notification` domain — does it carry messages?

### What the lore-vm binding exposes

`crates/lore-vm/src/ops/notification/` has exactly two ops:

- `subscribe.rs` → `lore::notification::subscribe(globals, LoreNotificationSubscribeArgs {}, cb)`
- `unsubscribe.rs` → `lore::notification::unsubscribe(globals, LoreNotificationUnsubscribeArgs {}, cb)`

Both arg structs are **empty** (`{}`). There is no `publish` op, and no way to
attach a payload. Subscribe just opens a server→client push stream.

### What the high-level `lore` crate exposes

Pinned rev `65598412` (`Cargo.lock`), at
`~/.cargo/git/checkouts/lore-*/6559841/lore/src/notification.rs`:

```rust
pub async fn subscribe(globals, LoreNotificationSubscribeArgs {}, callback) -> i32
pub async fn unsubscribe(globals, LoreNotificationUnsubscribeArgs {}, callback) -> i32
```

That is the **entire** public surface. No `publish`. `grep -rn publish lore/src/`
→ nothing.

### What the subscribe stream actually delivers

`lore-notification/src/client.rs` maps incoming server events to `LoreEvent`s:

| proto `event::Event` variant | mapped to `LoreEvent` |
|------------------------------|------------------------|
| `BranchCreated`              | `NotificationBranchCreated` |
| `BranchDeleted`              | `NotificationBranchDeleted` |
| `BranchPushed`               | `NotificationBranchPushed` |
| `ResourceLocked`             | `NotificationResourceLocked` |
| `ResourceUnlocked`           | `NotificationResourceUnlocked` |
| `Other(ExtensionEvent)`      | **dropped — `_ => {}`** |
| `Obliterate`                 | dropped |

So the channel only carries **lock/branch lifecycle events**, never an arbitrary
message. A "user A asks user B to check in file F" event is not representable as
any of the surfaced variants.

### The capability that *does* exist (one layer down)

`lore-proto/src/grpc/lore.notification.rs` is richer than the crate API admits:

```rust
pub struct ExtensionEvent {
    pub r#type: String,                       // e.g. "org.lore.security.compliance"
    pub payload: Option<prost_types::Any>,    // arbitrary
}
pub enum event::Event { Other(ExtensionEvent), BranchPushed(..), ResourceLocked(..), .. }

impl NotificationServiceClient {
    pub async fn subscribe(SubscribeRequest { repository }) -> Stream<Event>;
    pub async fn publish(PublishRequest { event: Event }) -> ();   // <-- exists!
}
```

`ExtensionEvent` is an explicit, namespaced, arbitrary-payload extension hook,
and `publish` is a real RPC. A lock-request could be modelled as
`ExtensionEvent { type: "games.studiobrain.lore.lock.request", payload }` and
`publish`ed, then received by every subscriber and filtered by `type`.

**But** to use it, lore-vm would have to either:
- talk raw gRPC (`lore_transport` / `lore_proto`) — which violates the "bind the
  high-level `lore` crate in-process, never shell, never FFI" rule in CLAUDE.md;
  or
- depend on upstream lore adding `lore::notification::publish` + an
  `ExtensionEvent`/`Other` `LoreEvent` variant.

Neither is available from the binding we are allowed to use. **The in-band path
is blocked at the crate boundary.**

> Upstream ask (file against EpicGames/lore, do NOT fork): surface
> `notification::publish` and an `Other(ExtensionEvent)` → `LoreEvent` mapping in
> the high-level crate. That single change would make the relay unnecessary for
> same-server teams — the lock-request becomes a normal `ExtensionEvent` rode in
> on the existing subscribe stream.

---

## 2. Server side-channel (the fallback)

If lore won't carry the message in-band, the alternative is a tiny relay the
clients both reach:

- **Endpoint:** `POST /api/v1/lock-messages` (send) + a delivery stream/poll
  (`GET /api/v1/lock-messages?since=…` or an SSE/WebSocket push) keyed by
  recipient `to_user_id`.
- **Store:** a small per-tenant table `lock_messages(id, repo, branch, path,
  from_user, to_user, kind, note, created_at, state)`. Ephemeral; a request is
  done once the holder releases or dismisses.
- **Who hosts it:** the hosted lore server already in the loop, or the
  StudioBrain cloud relay overlay (SBAI-4072). This is **premium** — cross-
  network delivery is the paid extension; basic same-machine coordination stays
  core/MIT.
- **Auth:** sender's bearer token; recipient resolves messages addressed to their
  `user_id` (already known from `auth_user_info`).

This is intentionally *not* built here — it is documented so the stubbed sender
can be pointed at it with one wiring change. The Rust op
`lock::file_message_send` already encodes exactly this contract in its doc
comment (it predates this spike) and is kept as the typed shape.

---

## 3. Reuse: the "who" is already known

The hard half of "ask the holder" — *identifying* the holder — is already solved:

- `lock.file_status(paths, branch)` → `[{ path, owner, locked_at }]`
- `lock.file_query(branch, owner, path)` → held locks with `{ path, owner, branch, locked_at }`

Both return the **owner** (holder) identity. The sender therefore always knows
*who* to address; the only missing piece is the *channel to reach them*, which is
section 1/2. The MVP exploits this: the Locks panel and the file view already
show the holder, so "Request check-in from `<holder>`" needs no new lookup.

---

## MVP delivered on top of this verdict

Because in-band lore delivery is blocked and the relay is premium/out-of-scope,
the MVP delivers **locally** and stubs the network hop:

- **Send** — `lock_request_checkin` Tauri command. It records the request in a
  process-local **inbox** (`AppState.lock_inbox`) and immediately fires an OS
  **tray notification** + emits a `lock/request` event to the frontend. On one
  machine (the common dev/demo/single-host case, and the realistic path until the
  relay lands) this is a *real, working* end-to-end loop: send → tray toast →
  inbox → Release. The cross-*network* hop is the single clearly-marked TODO
  (`// TODO(SBAI-4072 relay)`), where the same `LockRequest` would be POSTed to
  the side-channel in section 2 instead of (or in addition to) the local inbox.
- **Receive** — the holder's client shows a tray/OS notification ("X wants you to
  check in `<file>`") and an **inbox** entry with **Release** (calls
  `lock.file_release`) / **Dismiss**.
- **Surfaces** — a "Request check-in…" action on any lock held by *someone else*
  in the **Locks panel** and the **content/file view**, plus a palette entry, an
  inbox drawer, and tray wiring. Themeable via `--surface-*`, accessible.

What is **real** vs **stubbed**, precisely:

| Piece | Status |
|-------|--------|
| Holder identity (`file_status`/`file_query`) | real |
| Send request → local inbox + tray OS notification + `lock/request` event | real, in-process |
| Inbox UI (list, Release, Dismiss) | real |
| Release → `lock.file_release` | real (binds lore) |
| **Cross-network delivery to another machine's client** | **stubbed** — one `TODO(SBAI-4072 relay)` seam; needs the section-2 side-channel or an upstream lore `publish`/`ExtensionEvent` change |

The typed `lock::file_message_send` op in lore-vm is retained as the canonical
shape of the relay payload; it stays `Err(... relay not implemented)` until the
side-channel exists.
