# LoreGUI — Desktop client features (tray · service · auto-update · messaging)

Scope for the native-client polish pass. Most of this rides on first-class Tauri v2
plugins — kept deliberately not-too-crazy. The lock-messaging piece is the one
real design item (server-side).

## 1. Auto-update (highest confidence — `tauri-plugin-updater`)

On launch, check GitHub Releases for a newer version; if found, show release notes
+ "Update now" → download, verify signature, install, relaunch.

- **Plugin:** `tauri-plugin-updater` (+ `tauri-plugin-process` for relaunch).
- **Source:** GitHub Releases. `windows-build.yml` already uses `tauri-action`; enable
  its updater artifacts + a `latest.json` (the updater endpoint). NSIS/MSI/AppImage
  all support in-place update.
- **Signing:** generate a Tauri updater keypair; `TAURI_SIGNING_PRIVATE_KEY` +
  password as CI secrets; public key in `tauri.conf.json`.
- **UX:** silent check on launch (non-blocking); a small "Update available → vX.Y.Z"
  banner/toast with notes + Update/Later; a manual "Check for updates" in the tray +
  Account/Settings. Respect offline.
- **Tickets:** (a) add updater+process plugins + config + pubkey; (b) CI: emit signed
  updater artifacts + `latest.json` to the release; (c) in-app update-check UI.

## 2. System tray (`app.tray` API + a lore icon)

A tray-resident presence with quick actions, so users don't need the full window for
the common loop.

- **Icon:** a distinctive **lore icon** (see §5) — `.ico` (win), `.png` set (lin/mac),
  template icon for macOS.
- **Menu:** Open LoreGUI · ─ · **Sync** · **Check in** (commit staged) · **Release
  lock** (current file) · ─ · Check for updates · Quit. Tooltip/title shows current
  branch + dirty count; a status dot (clean / dirty / syncing / conflict).
- **Behavior:** left-click toggles the window; quick actions invoke the existing
  in-process commands (`sync`, `commit`, lock `release`) with a toast result; if an
  action needs input (commit message) open the window focused on it.
- **Tickets:** (a) tray scaffolding + menu + window toggle; (b) wire quick actions to
  commands + result toasts; (c) live status (branch/dirty/sync) in title + dot.

## 3. Run on login / minimize-to-tray (`tauri-plugin-autostart`)

For a GUI client, "install as service" = **launch at login + live in the tray**
(a true Windows Service is the wrong tool for a UI app; the *lore background
service* is separate and already exposed via `service start`).

- **Plugin:** `tauri-plugin-autostart` (register/unregister at login).
- **UX:** Settings/Account toggles — "Start LoreGUI at login" and "Close to tray
  instead of quitting." Closing the window hides to tray when enabled.
- **Tickets:** (a) autostart plugin + setting; (b) close-to-tray behavior + setting.

## 4. Lock messaging / coordination (NEEDS DESIGN — server-side)

When another user holds a lock/checkout you need, request it (or message them) from
inside the app, relayed by the server.

- **Today:** lock `query`/`status` already returns the **holder identity** — so the
  *who* is known. What's missing is a **transport** to reach them.
- **Design questions (the spike):**
  - **Transport:** reuse the StudioBrain sync substrate — **Valkey pub/sub on
    `tenant:{id}:sync`** (already planned, SBAI-2326/2381) extended with a
    `tenant:{id}:messages` channel, relayed by the cloud backend; *or* a small
    accounts/cloud `/messages` endpoint. Desktop subscribes when online.
  - **Message model:** `request_unlock{file, fromUser, toUser, note}` and free-text
    `message{...}`; delivered as a toast + an inbox. Holder can Release-and-notify or
    Decline.
  - **Boundaries:** identity/relay touches the **cloud** + **accounts** repos (PII,
    auth) — must respect the accounts security boundary; LoreGUI only sends/receives,
    never stores identity/PII.
- **UX:** on a locked file → "Request from <holder>" → sends; holder gets a toast +
  inbox item with Release / Decline; optional free-text note. A small tray/inbox
  badge for unread.
- **Tickets:** (a) **design spike** (transport + model + cross-repo plan); then
  (b) cloud relay endpoint/channel, (c) desktop send UI, (d) desktop inbox + toasts.
  Phase this after the spike resolves the transport.

## 5. Lore icon (asset)

A distinctive mark for the tray, app, installers, favicon. Generate a few concepts
(the AI image infra can produce options) → pick one → produce the full set:
`.ico` (16/32/48/256), `.png` (32→1024), macOS template, the website favicon/OG.
- **Ticket:** design + produce the icon set; wire into `tauri.conf.json` bundle icons
  + tray + website.

## Rollout order
1. Auto-update (self-contained, high value, low risk).
2. Lore icon (unblocks tray + installers + branding).
3. Tray + run-on-login/minimize-to-tray.
4. Lock-messaging spike → then the cloud relay + desktop UI.
