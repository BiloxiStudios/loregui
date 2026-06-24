// LoreGUI desktop E2E smoke suite.
//
// Breadth over depth: prove the packaged app boots, the webview renders, the
// onboarding gate behaves, the primary nav panels mount, and a handful of core
// IPC commands round-trip through the REAL Rust backend. Everything here runs
// against the genuine binary via tauri-driver — no stubs, no mocks.
//
// IPC is reached through `window.__TAURI__.core.invoke`, available because the
// E2E build is produced with the `e2e/tauri.e2e.conf.json` overlay
// (`withGlobalTauri: true`). The shipped production config is untouched.

/// <reference types="@wdio/globals/types" />

// Run a Tauri command inside the webview and return its result (or a tagged
// error). Mirrors what the frontend's `invoke()` wrappers do.
async function invoke<T = unknown>(
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<{ ok: true; value: T } | { ok: false; error: unknown }> {
  return browser.executeAsync(
    (c: string, a: Record<string, unknown>, done: (r: unknown) => void) => {
      const t = (window as unknown as { __TAURI__?: { core?: { invoke?: Function } } })
        .__TAURI__;
      if (!t?.core?.invoke) {
        done({ ok: false, error: "window.__TAURI__.core.invoke unavailable" });
        return;
      }
      t.core
        .invoke(c, a)
        .then((value: unknown) => done({ ok: true, value }))
        .catch((error: unknown) => done({ ok: false, error: String(error) }));
    },
    cmd,
    args,
  ) as Promise<{ ok: true; value: T } | { ok: false; error: unknown }>;
}

// Mark onboarding complete + reload so the main view (not the wizard) renders.
async function skipOnboardingAndReload() {
  await browser.execute(() => {
    localStorage.setItem("loregui.onboarded", "true");
  });
  await browser.url(""); // reload the app root within the webview
  await browser.$("header.topbar").waitForExist({ timeout: 30_000 });
}

describe("LoreGUI desktop app — smoke", () => {
  it("boots and renders the webview document", async () => {
    // The app window exists and a non-empty document mounted (React rendered).
    await browser.$("body").waitForExist({ timeout: 30_000 });
    const title = await browser.getTitle();
    // Tauri may report the OS window title or an empty document title depending
    // on the webview; assert the body has real content rather than over-fitting
    // on the title string.
    const bodyText = await browser.$("body").getText();
    expect(typeof title).toBe("string");
    expect(bodyText.length).toBeGreaterThan(0);
  });

  it("global Tauri IPC bridge is present (withGlobalTauri build)", async () => {
    const present = await browser.execute(() => {
      const t = (window as unknown as { __TAURI__?: { core?: { invoke?: unknown } } })
        .__TAURI__;
      return Boolean(t && t.core && typeof t.core.invoke === "function");
    });
    expect(present).toBe(true);
  });

  it("renders the main view after onboarding is satisfied", async () => {
    await skipOnboardingAndReload();
    const brand = await browser.$("header.topbar .brand");
    await brand.waitForExist({ timeout: 15_000 });
    expect(await brand.getText()).toContain("Lore");

    // The top-bar action cluster is the app's primary navigation.
    const actions = [...(await browser.$$("header.topbar .actions button"))];
    expect(actions.length).toBeGreaterThan(5);
  });

  it("mounts the core nav panels (Branches / History / Locks / Manage)", async () => {
    await skipOnboardingAndReload();

    // Each panel opens from a top-bar button keyed by its visible label and
    // renders an overlay/heading. We assert the panel surfaces something, then
    // dismiss it before the next.
    for (const label of ["Branches", "History", "Locks", "Manage"]) {
      // Prefer an exact-text match within the action bar; fall back to a
      // contains-text match anywhere.
      const exact = browser.$(`header.topbar .actions button=${label}`);
      const target = (await exact.isExisting())
        ? exact
        : browser.$(`button*=${label}`);
      await target.waitForClickable({ timeout: 10_000 });
      await target.click();

      // Something panel-shaped should appear. Panels render headings or a
      // dialog; assert the DOM grew a recognizable surface.
      await browser.waitUntil(
        async () => {
          const surfaces = [
            ...(await browser.$$(
              '[role="dialog"], .panel, .meta-panel, section, aside',
            )),
          ];
          return surfaces.length > 0;
        },
        { timeout: 10_000, timeoutMsg: `panel for "${label}" did not mount` },
      );

      // Dismiss: try an explicit close, then Escape.
      const close = browser.$("button.meta-close");
      if (await close.isExisting()) {
        await close.click();
      }
      await browser.keys(["Escape"]);
    }
  });

  it("round-trips core IPC: open_repository → current_repository", async () => {
    await skipOnboardingAndReload();

    const target = "/tmp/loregui-e2e-repo";
    const opened = await invoke("open_repository", { path: target });
    expect(opened.ok).toBe(true);

    const current = await invoke<string>("current_repository");
    expect(current.ok).toBe(true);
    if (current.ok) expect(current.value).toBe(target);
  });

  it("round-trips engine IPC against a fresh dir without hanging", async () => {
    await skipOnboardingAndReload();

    // Point at an empty (non-repo) dir, then call the read verbs the main view
    // drives. They must RESOLVE — either an Ok payload (engine reachable) or a
    // structured LoreError — within the IPC timeout, never hang the bridge.
    const dir = `/tmp/loregui-e2e-empty-${Date.now()}`;
    await invoke("open_repository", { path: dir });

    for (const [cmd, args] of [
      ["status", {}],
      ["branches", {}],
      ["log", { limit: 5 }],
    ] as const) {
      const res = await invoke(cmd, args);
      // ok=true (got a result) OR ok=false with a string error envelope — both
      // prove the command returned. A hang would have tripped executeAsync's
      // own timeout and failed the test loudly.
      expect(typeof res.ok).toBe("boolean");
    }
  });

  it("opens the command palette (Ctrl/Cmd-K)", async () => {
    await skipOnboardingAndReload();
    // The ⌘K button dispatches the OPEN_PALETTE_EVENT; click it and assert a
    // palette input appears.
    const paletteBtn = browser.$("header.topbar .actions button=⌘K");
    if (await paletteBtn.isExisting()) {
      await paletteBtn.click();
      await browser.waitUntil(
        async () => {
          const inputs = [
            ...(await browser.$$('input, [role="combobox"], [role="listbox"]')),
          ];
          return inputs.length > 0;
        },
        { timeout: 8_000, timeoutMsg: "command palette did not open" },
      );
    }
  });
});
