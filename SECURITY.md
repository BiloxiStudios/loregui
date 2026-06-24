# Security Policy

We take the security of LoreGUI seriously. LoreGUI is a desktop application that
**bundles a `loreserver` sidecar** and **handles credentials for your lore
repositories and storage backends**, so we treat vulnerability reports with
priority.

## Supported versions

LoreGUI is pre-1.0 and under active development. Security fixes land on `main`
and ship in the next signed release. Please test against the
[latest release](https://github.com/BiloxiStudios/loregui/releases/latest) or a
recent `main` build before reporting.

## Reporting a vulnerability

**Please do not open a public GitHub issue for security vulnerabilities.**

Report privately using either of:

1. **GitHub Private Vulnerability Reporting** — preferred. Go to the
   [Security tab](https://github.com/BiloxiStudios/loregui/security/advisories/new)
   and open a draft advisory. This keeps the report private until a fix is ready.
2. **Email** — `security@braindeadguild.com` with a clear subject line. If you
   want to encrypt, ask for a key in your first (non-sensitive) message.

Please include, where possible:

- A description of the vulnerability and its impact.
- Steps to reproduce (a proof of concept helps a lot).
- Affected version / commit, platform, and configuration.
- Any relevant logs — but **redact credentials, tokens, and repo paths**.

## What to expect

- **Acknowledgement** within 5 business days.
- An initial assessment and severity triage shortly after.
- Coordinated disclosure: we'll agree on a timeline with you and credit you in
  the advisory and release notes (unless you'd prefer to remain anonymous).

We ask that you give us a reasonable window to ship a fix before any public
disclosure.

## Scope & sensitive surfaces

LoreGUI's threat surface is mostly local, but a few areas warrant extra care:

- **Bundled `loreserver` sidecar.** Releases bundle a real `loreserver` binary
  that LoreGUI can launch when you host a server. Reports about how the sidecar
  is launched, configured, or exposed on the network are in scope. (LoreGUI
  *generates* the `loreserver` TOML — including network/QUIC/gRPC/HTTP and mTLS
  replication settings — so misconfiguration paths that could expose a server
  insecurely are relevant.)
- **Repository & storage credentials.** LoreGUI connects to lore servers and
  storage backends and may handle access tokens, keys, and mTLS material.
  Issues around how this data is stored, logged, or transmitted are high
  priority.
- **MCP server.** The built-in MCP server exposes lore operations as tools to AI
  agents. Reports about command/argument injection, path traversal, or
  unintended capability exposure through the MCP surface are in scope.
- **Tauri IPC / command surface.** The desktop shell exposes lore ops as Tauri
  commands; issues that let untrusted content invoke privileged commands are in
  scope.

### Out of scope

- The upstream `lore` engine itself — please report those to
  [EpicGames/lore](https://github.com/EpicGames/lore). If a LoreGUI default or
  the generated `loreserver` config makes an upstream issue materially worse,
  that part is in scope here.
- Vulnerabilities requiring a fully compromised local machine or physical access.
- Findings from automated scanners without a demonstrated, realistic impact.

## Privacy note

LoreGUI is local-first and ships **no telemetry or analytics** — see
[`PRIVACY.md`](PRIVACY.md). If you believe you've found code that exfiltrates
data, that is squarely a security issue and we want to hear about it.
