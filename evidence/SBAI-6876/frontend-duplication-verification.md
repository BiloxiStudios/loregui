# SBAI-6876: LoreGUI Frontend Duplication Verification — DISMISSED

**Date:** 2026-08-13  
**Status:** DISMISSED — no genuine duplication found  
**Related:** SBAI-6860 (gitnexus duplication sweep)

## Summary

The SBAI-6860 duplication sweep flagged LoreGUI as a "duplication candidate." This ticket verifies whether that flag represents genuine duplication or a false positive.

**Conclusion: DISMISSED.** The LoreGUI frontend is a legitimate, separate application with no wasteful duplication of core/app code.

## Evidence

### 1. LoreGUI Frontend EXISTS (previous worker was incorrect)

The LoreGUI repository contains a full Vite/React TypeScript frontend at `frontend/`:

- **208 source files** (TypeScript + TSX)
- **~32,489 lines** of application code
- **Structure:** Panels (Repository, Branches, History, Locks, Storage, Settings, Dependencies), onboarding flows, content viewers, theme system, command palette with 116 op manifest entries, commercial modules (entitlements, licensing)
- **Framework:** Vite + React (NOT Next.js like core)
- **Shell:** Tauri 2.0 via `src-tauri/`

### 2. No Wasteful Duplication Found

| Area | LoreGUI | Core/App | Verdict |
|------|---------|----------|---------|
| **Theme model** | 12 surfaces × 7 slots, CSS custom properties | Same surface model, different rendering | Intentional port for cross-compatibility (SBAI-4605) |
| **Theme bridge** | Consumer (`bridge.ts`) reads from StudioBrain | Producer (`theme-bridge.ts`) broadcasts | Complementary halves of same bridge |
| **API layer** | `@tauri-apps/api/core` invoke() wrappers | Next.js server/client fetch patterns | Different transport (Tauri IPC vs HTTP) |
| **Palette manifest** | TypeScript mirrors of lore-vm ops | N/A (lore-vm is LoreGUI's own backend) | Intentional wiring, not duplication |
| **Commercial modules** | LoreGUI-specific entitlements/licensing | StudioBrain billing, subscriptions | Different domain |
| **UI panels** | Lore version-control panels | Core content management UI | Different application |

### 3. The One Intentional Stub

`crates/lore-vm/src/ops/lock/file_message_send.rs` is classified as a `KNOWN_INTENTIONAL_ORPHAN` (compatibility-stub) in `scripts/upstream-lore-parity.mjs`. It cannot be implemented because:

- Upstream `lore` crate lacks `notification::publish`
- No `ExtensionEvent` → `LoreEvent` mapping exists upstream
- Cloud relay (SBAI-4072) is not built

This is a documented, intentional gap — not a bug or omission.

### 4. What the Previous Worker Missed

The prior worker (claudeq6-local) concluded "LoreGUI has no frontend directory" and pushed an empty branch. This was incorrect — `frontend/` exists at the repository root with 208 source files. The preprocessor's INVALIDATION was based on the assumption that `packages/frontend` (a Next.js package pattern) doesn't exist, which is technically true — but the frontend exists as `frontend/` (a Vite app pattern) instead.

## Disposition

**DISMISSED.** The LoreGUI frontend duplication finding from SBAI-6860 was a false positive. The LoreGUI frontend is a standalone Vite/React application for the Lore version-control system, with intentional theme compatibility porting from StudioBrain (SBAI-4605) and no wasteful code duplication.

### Why the Flag Was Raised

The gitnexus duplication sweep likely flagged the LoreGUI theme system as "duplicated" because it shares the same 12-surface × 7-slot model with StudioBrain. However, this is an **intentional port for cross-compatibility** — themes exported from StudioBrain can be imported into LoreGUI and vice versa. This is a feature, not a bug.
