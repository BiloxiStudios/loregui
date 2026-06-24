// Canonical download URLs for LoreGUI installers.
//
// The release pipeline (.github/workflows/release.yml) publishes a rolling
// `nightly` PRERELEASE on every push to `main`. That is the only deterministic,
// non-draft release with assets for every platform — a pushed `v*` tag produces
// a DRAFT release (no public assets), and GitHub's `releases/latest/download/`
// shortcut deliberately SKIPS prereleases, so it would not resolve `nightly`.
// We therefore pin the stable `nightly` tag explicitly.
//
// Asset names are produced by tauri-action from the bundle config:
//   productName "LoreGUI", version 0.1.0 (src-tauri/tauri.conf.json).
// Confirmed against the live `nightly` release assets. NOTE: the bundle version
// is baked into each filename, so bump these when tauri.conf.json `version`
// changes (or move to a redirect endpoint that resolves the current asset).
const REPO = "https://github.com/BiloxiStudios/loregui";
const NIGHTLY = `${REPO}/releases/download/nightly`;

export const RELEASES_PAGE = `${REPO}/releases`;

export const DOWNLOADS = {
  // Windows
  windowsExe: `${NIGHTLY}/LoreGUI_0.1.0_x64-setup.exe`,
  windowsMsi: `${NIGHTLY}/LoreGUI_0.1.0_x64_en-US.msi`,
  // Linux
  linuxAppImage: `${NIGHTLY}/LoreGUI_0.1.0_amd64.AppImage`,
  linuxDeb: `${NIGHTLY}/LoreGUI_0.1.0_amd64.deb`,
  linuxRpm: `${NIGHTLY}/LoreGUI-0.1.0-1.x86_64.rpm`,
  // macOS (Apple Silicon) — published, but only signed when the Apple cert
  // secret is present in CI; otherwise the .dmg is unsigned.
  macosDmg: `${NIGHTLY}/LoreGUI_0.1.0_aarch64.dmg`,
} as const;
