# LoreGUI install-test + marketing screenshots

Captured 2026-06-25 from a **real install** of the nightly Windows build
(LoreGUI 0.1.0, NSIS `LoreGUI_0.1.0_x64-setup.exe`) on a clean Windows 11
first-run — which also served as the real-install verification of the **#331**
onboarding-crash fix (SBAI-4205).

| File | Shot |
|---|---|
| `00-vm-state.png` | Win11 (context) |
| `01-installer-welcome.png` | NSIS "Welcome to LoreGUI Setup" |
| `02-installer-installing.png` | Install progress |
| `03-installer-complete.png` | "Installation Complete" |
| `04-installer-finish.png` | "Completing LoreGUI Setup" |
| `05-onboarding-first-run.png` | **#331 proof** — onboarding rendered on fresh launch |
| `06-onboarding-clean.png` | **Primary marketing shot** — "Choose Your Setup Mode" |
| `07-setup-storage-backend.png` | Host-a-server → Choose Storage Backend wizard |
| `08-connect-to-server.png` | Connect to a Lore Server |
| `09-graceful-error-handling.png` | Inline "missing .lore" error + Retry (no crash) |
| `10-version-process-evidence.png` | v0.1.0 + process Responding=True |

## Functional test (host-a-server + repo) — 2026-06-25
Hosting verified end-to-end: loreserver running at `lore://127.0.0.1:41337`, store created, main DAM UI loaded.

| File | Shot |
|---|---|
| `15-host-server-hosting-success.png` | **Money shot** — "✓ Server is hosting" + `lore://` URL |
| `16-main-ui-repository-view.png` | **Money shot** — main DAM / repository UI |
| `13-loreserver-running-console.png` | loreserver.exe config/log output |
| `17-evidence-loreserver-running-store-created.png` | loreserver PID + store dirs (evidence) |
| `11-host-wizard-*.png` | host-a-server wizard steps (incl. the step1/step3 first-run bug evidence) |
| `cropped/` | clean content-only crops (no taskbar) for website use |

Uncropped shots keep the Win11 taskbar (good for the GitHub README — shows it running on real Windows); `cropped/` are for polished web use.
