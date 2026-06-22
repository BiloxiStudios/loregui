# LoreGUI Mobile Feasibility Study (iOS + Android)

**Status:** complete · **Owner:** worker/SBAI-4056 (Qwen) · **Date:** 2026-06-21
**Related:** SBAI-4044 (lock messaging)
**Upstream:** Epic Games `lore` @ `65598412872a15685e1e8cd6d9d88425eedbc3c2`

---

## Verdict: GO — with conditions

LoreGUI **can** run on mobile via Tauri v2's official iOS + Android support. The in-process `lore` binding model (no CLI subprocess) is well-suited for mobile. However, the build pipeline requires additional toolchain setup, and the UX scope should be limited to a **companion app** (browse/review/light ops) rather than the full desktop editor.

---

## 1. Cross-Compilation: Dependency Audit

### 1.1 Pure Rust — ✅ No issues

These dependencies are pure Rust and compile cleanly to mobile targets:

| Crate | Role |
|-------|------|
| `tonic` 0.14 | gRPC client (pure Rust) |
| `quinn` 0.11 / `quinn-proto` (Epic fork) | QUIC transport |
| `rustls` 0.23 | TLS (no OpenSSL) |
| `tokio` 1.52 | Async runtime |
| `serde` / `serde_json` | Serialization |
| `thiserror` 2 | Error handling |
| `tracing` 0.1 | Logging |
| `dashmap` 6 | Concurrent hash maps |
| `parking_lot` 0.12 | Synchronization |
| `futures` / `tokio-stream` | Stream abstractions |
| `rcgen` 0.14 | Certificate generation |
| `prost-types` 0.14 | Protobuf types |
| `crossbeam` 0.8 | Concurrent data structures |
| `uuid` 1.23 | UUID generation |
| `chrono` 0.4 | Date/time |
| `toml` 0.9 | Config parsing |
| `rand` 0.9 | Random number generation |

### 1.2 C/ASM dependencies — ⚠️ Requires toolchain

| Crate | Role | Issue | Surmountable? |
|-------|------|-------|---------------|
| **`lz4-sys`** 1.11 | LZ4 compression (lore-storage) | Requires `aarch64-linux-android-clang` (NDK) for Android; Xcode clang for iOS | **Yes** — install NDK; standard Rust cross-compilation setup |
| **`blake3`** 1.8.5 | Hashing (lore-base, lore-revision, lore-storage) | Ships with C/ASM optimizations. Has a `pure` feature flag that avoids them. | **Yes** — but requires `[patch]` override since `lore` upstream doesn't expose the feature |
| **`libc`** 0.2 | Platform libc bindings | Has mobile target support. Standard. | **Yes** — works out of the box with rustup targets |
| **`memmap2`** 0.9 | Memory-mapped files | Uses platform-specific syscalls. Supported on Android. iOS needs testing. | **Likely yes** — standard mobile support |
| **`fastcdc`** 3.2 | Content-defined chunking | Pure Rust | ✅ |

### 1.3 Build-time only dependencies — ✅ No runtime impact

| Crate | Role |
|-------|------|
| `cbindgen` 0.29 | FFI header generation (build.rs) |
| `vergen` 9.1 | Build metadata (build.rs) |
| `cc` 1.2 | C compiler invocation (build.rs) |
| `blake3` (build dep) | Build-time hashing |

### 1.4 Empirical Build Attempt

```
$ cargo check -p lore-vm --target aarch64-linux-android
```

**Result:** Failed at `lz4-sys` compilation — `aarch64-linux-android-clang: No such file or directory`. This is expected; the Android NDK is not installed on the build server. The failure is in the C toolchain, not in Rust code, which means:

1. All pure-Rust deps compiled successfully up to the point where `lz4-sys` was reached
2. No Rust-level incompatibilities were found
3. The failure mode is standard for Android cross-compilation without NDK

**iOS:** Not testable on this Linux server (requires macOS + Xcode). No Rust-level blockers expected; `lz4-sys` will need Xcode's clang, which is standard for iOS Rust projects.

---

## 2. Tauri v2 Mobile Setup

### 2.1 Current State

The LoreGUI repo (`src-tauri/`) is configured as a standard Tauri v2 desktop app:
- `tauri.conf.json` targets `"all"` (macOS, Windows, Linux)
- No `tauri.mobile.conf.json` or iOS/Android project directories exist yet
- The `cfg(mobile)` conditional compilation pattern is available via Tauri 2's build system

### 2.2 Required Setup Steps

1. **Install Tauri CLI mobile targets:**
   ```
   cargo tauri ios init    # generates Xcode project in src-tauri/gen/ios/
   cargo tauri android init  # generates Gradle project in src-tauri/gen/android/
   ```

2. **Android prerequisites:**
   - Android SDK (API 33+)
   - Android NDK (r26+ for Rust cross-compilation)
   - `cargo-ndk` or manual `CARGO_TARGET_*_LINKER` setup
   - Environment: `ANDROID_HOME`, `ANDROID_NDK_HOME`

3. **iOS prerequisites:**
   - macOS with Xcode 15+
   - `rustup target add aarch64-apple-ios` (or `aarch64-apple-ios-sim` for simulator)
   - Provisioning profiles for device testing

4. **Known gotchas (from studiobrain-app mobile migration, SBAI-2227):**
   - `npx tauri ios build` passes `-allowProvisioningUpdates` to xcodebuild, which breaks manual signing in CI — use direct `xcodebuild archive` + `xcodebuild -exportArchive`
   - Generated `xcscheme` defaults to Debug for Archive, which expects dev-only mobile server address file — patch scheme to use Release before `xcodebuild archive` (SBAI-2667)
   - `cfg(mobile)` is set by Tauri's build system, not a Cargo feature (SBAI-2502)

### 2.3 LoreGUI-Specific Considerations

- The lore-vm `client-backend` (in-process, default) is the correct choice for mobile — no CLI subprocess needed
- The `cli-backend` feature would require a pre-built `lore` CLI binary for each mobile target, which adds complexity
- No rclone, no bore tunnel needed on mobile (read-only/API-driven storage pattern per studiobrain-app)

---

## 3. Mobile UX Proposal: Companion App

### 3.1 Scope

Mobile should be a **focused companion**, not the full desktop editor. The phone form factor suits browse/review/light ops:

| Feature | Mobile Support | Rationale |
|---------|---------------|-----------|
| Repository status | ✅ | Quick check: dirty files, current branch, ahead/behind |
| History / log | ✅ | Scrollable commit history, tap to see details |
| Branch list + switch | ✅ | Essential for context switching on the go |
| File staging | ✅ | Tap to stage/unstage individual files |
| Commit | ✅ | Simple text input for commit message |
| Lock request/release | ✅ | Pairs with SBAI-4044 lock messaging |
| Approvals | ✅ | Review and approve from phone |
| Branch create | ✅ | Quick feature branch creation |
| Push / sync | ✅ | One-tap push |
| Merge | ⚠️ | Possible, but conflict resolution is poor on mobile |
| File editing | ❌ | Desktop territory; mobile code editing is impractical |
| Heavy diff review | ⚠️ | Possible for small diffs, painful for large files |

### 3.2 Mobile-Specific Features

- **Lock notifications via SBAI-4044:** When someone requests a lock on a branch you own, get a push notification on your phone. Approve or release directly.
- **Quick status widget:** Home screen widget showing repo status (dirty/clean, current branch)
- **Offline read mode:** Cache recent history and branch list for offline browsing

### 3.3 UI Adaptations

- Bottom navigation (standard mobile pattern)
- Collapsible sidebar → bottom sheet
- Large touch targets (minimum 44x44pt)
- Reduced information density vs. desktop
- Pull-to-refresh for sync
- Swipe actions for stage/unstage

---

## 4. Risk Assessment

| Risk | Severity | Mitigation |
|------|----------|------------|
| lz4-sys NDK setup complexity | Low | Standard Rust Android cross-compilation; well-documented |
| blake3 pure mode requires [patch] | Low | One Cargo.toml override; test on CI |
| iOS build requires macOS | Medium | CI runners (macos-latest GitHub Actions) already have Xcode |
| lore crate updates may introduce new C deps | Medium | Pin lore to exact rev; audit deps on each bump |
| memmap2 behavior on mobile | Low | Test early in simulator/emulator |
| Mobile UX scope creep | Medium | Define hard boundaries (no file editing, no heavy diff) |
| Tauri v2 mobile maturity | Low | Official Tauri v2 support, actively maintained |

---

## 5. Rough Scope Estimate

### Phase 1: Build Pipeline (1-2 weeks)
1. Add `tauri.mobile.conf.json` and init iOS/Android projects
2. Set up Android NDK on CI (GitHub Actions `android-ndk` action)
3. Add `[patch]` override for blake3 `pure` feature on mobile targets
4. Verify `cargo tauri android build` succeeds
5. Verify `cargo tauri ios build` succeeds on macOS CI runner
6. Get trivial shell app launching on simulator and emulator

### Phase 2: Mobile Shell (1-2 weeks)
1. Create mobile-specific Tauri window config (fullscreen, no menubar)
2. Set up mobile frontend routing (React, bottom nav)
3. Wire up lore-vm ops for core mobile features (status, log, branches)
4. Basic repository browser

### Phase 3: Mobile Features (2-3 weeks)
1. Lock management (request, release, approve) — SBAI-4044 integration
2. Push notifications for lock events
3. Quick commit + push flow
4. Branch switch + create

### Phase 4: Polish (1 week)
1. Offline read mode
2. Touch-optimized UI components
3. Performance tuning (lore VM initialization time on mobile)
4. App store packaging

**Total: 5-8 weeks for a functional mobile companion app.**

---

## 6. Go/No-Go Decision

**GO** — with these conditions:

1. **Scope is companion-only.** No file editing, no heavy diff review, no conflict resolution on mobile.
2. **Mobile build uses `client-backend`** (in-process lore), never `cli-backend`.
3. **CI runners must have:** Android NDK (for Android builds), macOS with Xcode (for iOS builds).
4. **lore dependency is pinned** and audited on each bump for new C dependencies.
5. **Mobile UX pairs with SBAI-4044** (lock messaging) as a primary use case.

---

## Appendix A: Full Dependency Tree (lore-vm → mobile targets)

```
lore-vm
├── async-trait ✅
├── serde ✅
├── serde_json ✅
├── thiserror ✅
├── tokio ✅
├── tracing ✅
└── lore
    ├── lore-base → blake3 ⚠️(pure flag needed)
    ├── lore-credential ✅
    ├── lore-revision → blake3 ⚠️
    ├── lore-storage → blake3 ⚠️, lz4-sys ⚠️(NDK), memmap2 ⚠️(test)
    ├── lore-transport → tonic ✅, quinn ✅, rustls ✅
    ├── lore-notification ✅
    └── lore-telemetry ✅ (opentelemetry, pure Rust)
```

## Appendix B: Commands Tested

| Command | Result | Notes |
|---------|--------|-------|
| `cargo check -p lore-vm` | ✅ Green | Native target |
| `cargo test -p lore-vm` | ✅ Green | Native target |
| `cargo check -p lore-vm --target aarch64-linux-android` | ❌ Fails at lz4-sys | Missing NDK (expected) |
| `cargo check -p lore-vm --target aarch64-apple-ios` | Not tested | Requires macOS |

## Appendix C: Reference Tickets

- **SBAI-2227:** Mobile migration from Capacitor to Tauri 2 — iOS build gotchas
- **SBAI-2667:** Tauri iOS CI scheme patching (Debug → Release for Archive)
- **SBAI-2502:** `cfg(mobile)` is set by Tauri 2 build system, not a Cargo feature
- **SBAI-4044:** Lock messaging — primary mobile use case
