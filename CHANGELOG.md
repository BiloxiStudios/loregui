# Changelog

All notable changes to LoreGUI are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

LoreGUI is **pre-alpha**. Until the first `v*` tag, every push to `main` refreshes
the rolling [`nightly`](https://github.com/BiloxiStudios/loregui/releases/tag/nightly)
prerelease with current Windows / Linux / macOS installers.

## [Unreleased]

### Added

- **In-app auto-update** (SBAI-4040). The desktop app now checks for updates on
  launch, prompts the user, downloads + verifies the signed update, and
  relaunches. Built on `tauri-plugin-updater`; update artifacts and a
  `latest.json` manifest are signed in CI and published to the GitHub Release.
- **Windows Authenticode signing** in the release workflow, gated on the
  `WINDOWS_CERTIFICATE` secret. Absent the secret, the build is produced unsigned
  and the release is never blocked (mirrors the existing macOS-signing fallback).
- This changelog. The release workflow now uses the latest `[Unreleased]` /
  versioned section as the release body for tagged builds.

### Notes

- **Updater endpoint** points at the `nightly` tag's `latest.json`
  (`releases/download/nightly/latest.json`) because GitHub's `releases/latest`
  redirect **excludes prereleases**, and the pre-alpha channel publishes the
  `nightly` build as a prerelease. When the first stable `v*` release ships,
  switch the endpoint in `src-tauri/tauri.conf.json` to
  `releases/latest/download/latest.json`.
- The updater **private** key is held only as the GitHub Actions secret
  `TAURI_SIGNING_PRIVATE_KEY` (with optional `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`).
  The matching **public** key is committed in `src-tauri/tauri.conf.json`
  (`plugins.updater.pubkey`). Never commit or print the private key.

[Unreleased]: https://github.com/BiloxiStudios/loregui/compare/nightly...HEAD
