# Privacy

**Short version: LoreGUI has no telemetry, no analytics, and no phone-home.** It
is a local-first desktop application. Your repository data stays wherever your
storage backend puts it. We don't collect it, and we never see it.

## What LoreGUI does and does not do

- **No analytics or usage telemetry.** LoreGUI does not embed an analytics SDK,
  does not send usage events, crash pings, or "anonymous statistics" anywhere,
  and has no account or sign-in. There is nothing to opt out of.
- **No background phone-home.** The app does not call back to BiloxiStudios,
  BrainDeadGuild, or any first-party service on launch or in the background.
- **Local-first by design.** LoreGUI binds the `lore` engine in-process. The
  network traffic it makes is the traffic *you* direct: talking to the lore
  server you connect to or host, and to the storage backend you configure.
- **Your data stays where your storage backend is.** Repository contents,
  revisions, and assets live in the lore server / storage you point LoreGUI at.
  LoreGUI is a client and (optionally) a local host for that — it is not an
  intermediary that copies your data elsewhere.

## A note on "telemetry" in the server config

When you use the **Host a server** flow, LoreGUI generates a `loreserver` TOML
that includes a `[telemetry]` section. That is **`loreserver`'s own logging and
metrics configuration** (log format, log output/file, and optional metrics
endpoints you control) — it is *your server's* operational logging, written to
where *you* choose. It is **not** LoreGUI analytics and it does not report
anything to us. You own that server and its logs.

## Network connections LoreGUI makes

All network activity is initiated by your actions, not by background collection:

- **The lore server you connect to or host** — for version-control operations
  (status, commit, branch, sync, locks, …).
- **The storage backend you configure** — content-addressed object storage for
  assets and revisions.
- **GitHub Releases** — only when you choose to check for or download an update.

## Third-party services

LoreGUI does not integrate any third-party analytics, advertising, or tracking
services. The optional MCP server runs locally and is driven by an AI agent that
**you** configure; any data that agent sends to its own model provider is
governed by that agent's and that provider's policies, not LoreGUI's.

## Changes to this policy

LoreGUI is open source under the MIT license. If the privacy posture ever
changes, it will be a visible, reviewable change to this file in the public
repository.

## Questions

Open a [discussion or issue](https://github.com/BiloxiStudios/loregui/issues), or
reach the community via [BrainDeadGuild](https://braindeadguild.com/discord). For
security concerns specifically, see [`SECURITY.md`](SECURITY.md).
