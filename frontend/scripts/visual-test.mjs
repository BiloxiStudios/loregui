// LoreGUI visual harness.
//
// Renders the built frontend (frontend/dist, served by `vite preview`) in a
// headless Chromium via Playwright and screenshots every primary UI surface:
// onboarding, the main view, each nav panel, the command palette, and the
// theme editor (light + dark).
//
// The real app talks to a Tauri Rust backend via `invoke()`. There is no
// backend in a browser, so we install a stub at `window.__TAURI_INTERNALS__`
// that resolves a small library of realistic sample payloads keyed by command
// name. This lets panels render *content* instead of error states. Anything we
// don't have a sample for resolves to a benign empty value, so unknown commands
// degrade to empty (not crashing) states.
//
// Usage:
//   node scripts/visual-test.mjs            # assumes a server on $BASE_URL or :4173
//   BASE_URL=http://localhost:4173 node scripts/visual-test.mjs
//
// Output: PNGs in $OUT_DIR (default /tmp/loregui-shots).

import { chromium } from "playwright";
import { mkdirSync } from "node:fs";

const BASE_URL = process.env.BASE_URL || "http://localhost:4173";
const OUT_DIR = process.env.OUT_DIR || "/tmp/loregui-shots";
const VIEWPORT = { width: 1440, height: 900 };

mkdirSync(OUT_DIR, { recursive: true });

// --- Sample data returned by the stubbed Tauri invoke() ---------------------
// Keyed by command name. Shapes mirror the typed wrappers in src/api.ts.
const SAMPLES = {
  current_repository: "lore://localhost/demo-world",
  status: {
    repo_id: "repo-001",
    branch: "main",
    revision: "a1b2c3d4e5f6a7b8",
    ahead: 2,
    behind: 1,
    changes: [
      { path: "world/characters/aria.md", kind: "modified", staged: true },
      { path: "world/locations/citadel.md", kind: "added", staged: true },
      { path: "world/lore/timeline.md", kind: "modified", staged: false },
      { path: "assets/maps/overworld.png", kind: "added", staged: false },
      { path: "world/factions/old.md", kind: "deleted", staged: false },
    ],
  },
  branches: [
    { name: "main", id: "br-main", latest_revision: "a1b2c3d4", is_current: true },
    { name: "feature/quest-arc", id: "br-quest", latest_revision: "99887766", is_current: false },
    { name: "release/v1", id: "br-rel", latest_revision: "12340000", is_current: false },
  ],
  log: [
    { hash: "a1b2c3d4e5f6", message: "Add citadel location and timeline", author: "aria@studio", timestamp: "2026-06-20T10:00:00Z", parent: "99887766" },
    { hash: "99887766aabb", message: "Quest arc: introduce the seer", author: "bram@studio", timestamp: "2026-06-19T14:30:00Z", parent: "12340000" },
    { hash: "12340000ccdd", message: "Initial world bible import", author: "aria@studio", timestamp: "2026-06-18T09:15:00Z", parent: null },
  ],
  // --- repository panel ---
  repository_instance_list: {
    instance_count: 2,
    instances: [
      { id: "inst-aaa", path: "/home/dev/demo-world", branch: "main", revision: "a1b2c3d4" },
      { id: "inst-bbb", path: "/home/dev/demo-world-wt", branch: "feature/quest-arc", revision: "99887766" },
    ],
  },
  repository_verify_state: {
    healed_staged_state: "deadbeefcafef00d",
    fragments: [{}, {}, {}],
    remote_fragments: [{}, {}],
    error_count: 0,
    corrupted_count: 0,
  },
  repository_metadata_get: {
    entries: [
      { key: "project.name", value: "Demo World", value_type: "string" },
      { key: "project.engine", value: "Unreal", value_type: "string" },
    ],
  },
  // --- branches panel ---
  branch_list: {
    count: 3,
    entries: [
      { location: "local", id: "br-main", name: "main", category: "trunk", latest: "a1b2c3d4", stack: [], creator: "aria@studio", created: 1750000000, is_current: true, archived: false },
      { location: "local", id: "br-quest", name: "feature/quest-arc", category: "feature", latest: "99887766", stack: [], creator: "bram@studio", created: 1750100000, is_current: false, archived: false },
      { location: "remote", id: "br-rel", name: "release/v1", category: "release", latest: "12340000", stack: [], creator: "aria@studio", created: 1749900000, is_current: false, archived: false },
    ],
  },
  branch_info: {
    name: "main", id: "br-main0000", archived: false, category: "trunk",
    creator: "aria@studio", created: 1750000000, latest: "a1b2c3d4e5f6",
    latest_remote: "a1b2c3d4e5f6", parent: "", branch_point: "",
  },
  // --- history panel ---
  revision_history: {
    entries: [
      { revision: "a1b2c3d4e5f6", revision_number: 3, parents: ["99887766aabb"] },
      { revision: "99887766aabb", revision_number: 2, parents: ["12340000ccdd"] },
      { revision: "12340000ccdd", revision_number: 1, parents: [] },
    ],
  },
  revision_info: {
    revision: "a1b2c3d4e5f6", revision_number: 3, author: "aria@studio",
    message: "Add citadel location and timeline", parents: ["99887766aabb"],
    timestamp: 1750413600, branch: "main",
  },
  revision_diff: {
    files: [
      { path: "world/locations/citadel.md", action: "added", action_short: "A" },
      { path: "world/lore/timeline.md", action: "modified", action_short: "M" },
    ],
  },
  revision_find: { revisions: [{ signature: "a1b2c3d4e5f6" }] },
  revision_find_local: { revisions: [{ signature: "a1b2c3d4e5f6" }] },
  // --- locks panel ---
  lock_file_query: {
    count: 2,
    locks: [
      { path: "world/characters/aria.md", owner: "bram@studio", branch: "main", locked_at: 1750400000 },
      { path: "assets/maps/overworld.png", owner: "aria@studio", branch: "main", locked_at: 1750390000 },
    ],
  },
  lock_file_status: {
    locks: [
      { path: "world/characters/aria.md", owner: "bram@studio", locked_at: 1750400000 },
    ],
  },
  // --- dependencies panel ---
  dependency_list: {
    file_count: 2,
    total_entry_count: 3,
    files: [
      { path: "world/locations/citadel.md", dependencies: ["world/factions/order.md", "assets/maps/overworld.png"] },
      { path: "world/characters/aria.md", dependencies: ["world/locations/citadel.md"] },
    ],
  },
  // --- account panel ---
  auth_user_info: { id: "user-aria", name: "Aria (aria@studio)" },
  auth_local_user_info: {
    users: [
      { user_id: "user-aria", display_name: "Aria" },
      { user_id: "user-bram", display_name: "Bram" },
    ],
    tokens: [],
  },
  // --- storage panel ---
  shared_store_info: {
    use_automatically: true,
    stores: [
      { remote_url: "lore://localhost/demo-world", path: "/home/dev/.lore/shared", exists: true },
    ],
  },
};

// Resolve a stub value for a command. Unknown commands get a benign default so
// the UI degrades to an empty/loaded state rather than throwing.
function stubValue(cmd) {
  if (Object.prototype.hasOwnProperty.call(SAMPLES, cmd)) return SAMPLES[cmd];
  // Heuristic empties for command families we don't explicitly sample.
  if (/_list$|_history$|_query$|_find/.test(cmd)) return { entries: [], count: 0, files: [], locks: [], revisions: [] };
  return null;
}

// Injected into the page *before* any app code runs.
function installTauriStub(samples) {
  const SAMPLES = samples;
  // eslint-disable-next-line no-undef
  window.__TAURI_INTERNALS__ = {
    transformCallback(cb) {
      const id = Math.floor(Math.random() * 1e9);
      // eslint-disable-next-line no-undef
      window[`_tauri_cb_${id}`] = cb;
      return id;
    },
    unregisterCallback() {},
    convertFileSrc(p) { return p; },
    async invoke(cmd) {
      const has = Object.prototype.hasOwnProperty.call(SAMPLES, cmd);
      if (has) return SAMPLES[cmd];
      if (/_list$|_history$|_query$|_find/.test(cmd)) {
        return { entries: [], count: 0, files: [], locks: [], revisions: [] };
      }
      return null;
    },
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
  };
  // Mark as a Tauri environment so any isTauri() checks pass.
  // eslint-disable-next-line no-undef
  window.__TAURI__ = window.__TAURI__ || {};
}

async function shoot(page, name) {
  const file = `${OUT_DIR}/${name}.png`;
  await page.screenshot({ path: file, fullPage: false });
  console.log(`  shot: ${file}`);
}

async function setOnboarded(page, value) {
  await page.evaluate((v) => {
    if (v) localStorage.setItem("loregui.onboarded", "true");
    else localStorage.removeItem("loregui.onboarded");
  }, value);
}

// Click a top-bar action button by its visible text, wait for the panel.
async function openByText(page, text) {
  const btn = page.locator(`header.topbar .actions button`, { hasText: new RegExp(`^${text}$`) }).first();
  await btn.click();
  await page.waitForTimeout(400);
}

async function closeOverlay(page) {
  // Most panels render a "Close" button; the theme modal too. Try a few.
  const close = page.locator("button", { hasText: /^(Close|x|×)$/ }).first();
  if (await close.count()) {
    await close.click().catch(() => {});
    await page.waitForTimeout(200);
  }
  await page.keyboard.press("Escape").catch(() => {});
  await page.waitForTimeout(150);
}

async function run() {
  const browser = await chromium.launch();
  const results = [];

  async function page(colorScheme, samples = SAMPLES) {
    const ctx = await browser.newContext({ viewport: VIEWPORT, colorScheme });
    const p = await ctx.newPage();
    p.on("pageerror", (e) => console.warn(`  [pageerror] ${e.message}`));
    await p.addInitScript(installTauriStub, samples);
    return { ctx, p };
  }

  // 1. Onboarding. The app skips onboarding if a repository is already open
  // (App.tsx auto-marks onboarded when current_repository is truthy), so we
  // serve a "no repo open" stub for these shots.
  {
    const noRepoSamples = { ...SAMPLES, current_repository: "" };
    const { ctx, p } = await page("light", noRepoSamples);
    await p.goto(BASE_URL, { waitUntil: "networkidle" });
    await setOnboarded(p, false);
    await p.reload({ waitUntil: "networkidle" });
    await p.waitForTimeout(600);
    await shoot(p, "onboarding-mode-select");

    // Enter the host flow (4-step stepper) to capture a wizard step.
    const hostCard = p.locator(".onboarding-mode-card", { hasText: /Host a Server/i }).first();
    if (await hostCard.count()) {
      await hostCard.click().catch(() => {});
      await p.waitForTimeout(400);
      await shoot(p, "onboarding-host-step1");
    }

    // Enter the client flow (connect to server) from a fresh load.
    await p.reload({ waitUntil: "networkidle" });
    await p.waitForTimeout(400);
    const clientCard = p.locator(".onboarding-mode-card", { hasText: /Connect to a Lore Server/i }).first();
    if (await clientCard.count()) {
      await clientCard.click().catch(() => {});
      await p.waitForTimeout(400);
      await shoot(p, "onboarding-client-step1");
    }
    await ctx.close();
  }

  // 2..N — main view + each panel, both color schemes.
  for (const scheme of ["light", "dark"]) {
    const { ctx, p } = await page(scheme);
    await p.goto(BASE_URL, { waitUntil: "networkidle" });
    await setOnboarded(p, true);
    await p.reload({ waitUntil: "networkidle" });
    await p.waitForTimeout(700);

    await shoot(p, `main-view-${scheme}`);

    const panels = [
      ["Storage", "panel-storage"],
      ["Manage", "panel-manage"],
      ["Locks", "panel-locks"],
      ["Dependencies", "panel-dependencies"],
      ["History", "panel-history"],
      ["Branches", "panel-branches"],
      ["Account", "panel-account"],
      ["Theme", "panel-theme"],
    ];
    for (const [label, name] of panels) {
      try {
        await openByText(p, label);
        await shoot(p, `${name}-${scheme}`);
        results.push({ name, scheme, ok: true });
      } catch (e) {
        console.warn(`  [fail] ${label}: ${e.message}`);
        results.push({ name, scheme, ok: false, err: e.message });
      }
      await closeOverlay(p);
    }

    // Command palette (Ctrl+K).
    try {
      await p.keyboard.press("Control+k");
      await p.waitForTimeout(400);
      await shoot(p, `palette-${scheme}`);
      // Type a query to surface results.
      await p.keyboard.type("branch");
      await p.waitForTimeout(300);
      await shoot(p, `palette-query-${scheme}`);
      await p.keyboard.press("Escape");
    } catch (e) {
      console.warn(`  [fail] palette: ${e.message}`);
    }

    await ctx.close();
  }

  await browser.close();
  console.log("\nDone. Screenshots in", OUT_DIR);
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
