// LoreGUI Tauri WebDriver smoke suite.
//
// Launches the BUILT desktop binary (via tauri-driver) and drives the real
// WebView. Goals (breadth over depth):
//
//   1. the app boots and the main window renders (the `.app` shell mounts),
//   2. onboarding can be skipped (the same localStorage gate the app uses),
//   3. the topbar and the key panels mount — Repository/Manage, Branches,
//      History, Status (the changes view),
//   4. a few core IPC commands round-trip end-to-end against a real on-disk
//      `.lore` repo created in a temp dir: create → write → stage → commit →
//      status.
//
// The IPC round-trip uses `window.__TAURI__.core.invoke`, which the E2E build
// exposes via `withGlobalTauri: true` (src-tauri/tauri.e2e.conf.json). The
// shipped binary keeps that OFF; this is a test-only affordance.

/// <reference types="@wdio/globals/types" />

// ---- helpers --------------------------------------------------------------

/** Invoke a Tauri IPC command from inside the WebView and return its result.
 * Mirrors `@tauri-apps/api`'s `invoke(cmd, args)`. Throws (rejects) if the
 * command errors, surfacing the LoreError back to the test. */
async function invoke<T = unknown>(
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  const result = await browser.executeAsync(
    (c: string, a: Record<string, unknown>, done: (r: unknown) => void) => {
      // @ts-expect-error injected by withGlobalTauri
      const tauri = window.__TAURI__;
      if (!tauri?.core?.invoke) {
        done({ __e2eError: "window.__TAURI__.core.invoke is unavailable" });
        return;
      }
      tauri.core
        .invoke(c, a)
        .then((r: unknown) => done({ __e2eOk: r }))
        .catch((e: unknown) =>
          done({ __e2eError: typeof e === "string" ? e : JSON.stringify(e) }),
        );
    },
    cmd,
    args,
  );
  const r = result as { __e2eOk?: T; __e2eError?: string };
  if (r && "__e2eError" in r && r.__e2eError !== undefined) {
    throw new Error(`invoke(${cmd}) failed: ${r.__e2eError}`);
  }
  return (r as { __e2eOk: T }).__e2eOk;
}

/** Skip the onboarding flow by flipping the same localStorage flag the app
 * reads (`App.tsx`: `localStorage.getItem("loregui.onboarded") === "true"`),
 * then reload so the app re-reads it on mount. */
async function skipOnboardingAndReload(): Promise<void> {
  await browser.execute(() => {
    window.localStorage.setItem("loregui.onboarded", "true");
  });
  await browser.refresh();
  await $(".app").waitForExist({ timeout: 30_000 });
}

/** Whether a topbar action button with the exact visible label exists.
 * WebKitWebDriver rejects WDIO's `button=text` / `*=text` pseudo-selectors
 * ("invalid selector"), so we resolve by exact text content in-page. */
async function navButtonExists(label: string): Promise<boolean> {
  return browser.execute((l: string) => {
    const buttons = Array.from(
      document.querySelectorAll<HTMLButtonElement>(".topbar .actions button"),
    );
    return buttons.some((b) => (b.textContent || "").trim() === l);
  }, label);
}

/** Click a topbar action button by its exact visible label, in-page (same
 * reason as navButtonExists). Returns true if a matching button was clicked. */
async function clickNavButton(label: string): Promise<boolean> {
  return browser.execute((l: string) => {
    const buttons = Array.from(
      document.querySelectorAll<HTMLButtonElement>(".topbar .actions button"),
    );
    const btn = buttons.find((b) => (b.textContent || "").trim() === l);
    if (!btn) return false;
    btn.click();
    return true;
  }, label);
}

/** Whether any element on the page contains the given visible text. Replaces
 * WDIO's `*=text` selector, which WebKitWebDriver rejects. */
async function pageContainsText(text: string): Promise<boolean> {
  return browser.execute(
    (t: string) => (document.body.textContent || "").includes(t),
    text,
  );
}

interface IpcErrorPayload {
  kind: string;
  message: string;
}

function parseIpcError(error: unknown, command: string): unknown {
  if (!(error instanceof Error)) throw error;
  const prefix = `invoke(${command}) failed: `;
  if (!error.message.startsWith(prefix)) throw error;
  try {
    return JSON.parse(error.message.slice(prefix.length));
  } catch {
    throw error;
  }
}

async function expectNoRepository(
  command: string,
  args: Record<string, unknown> = {},
): Promise<void> {
  try {
    await invoke(command, args);
  } catch (caught) {
    expect(parseIpcError(caught, command)).toEqual({
      kind: "NoRepository",
      message: "no repository is open",
    } satisfies IpcErrorPayload);
    return;
  }
  throw new Error(`invoke(${command}) unexpectedly resolved`);
}

// ---- suite ----------------------------------------------------------------

describe("LoreGUI desktop smoke", () => {
  before(async () => {
    // The window+WebView are created by tauri-driver before the session starts;
    // the React app may show onboarding first. Wait for SOME root to exist.
    await browser.waitUntil(
      async () => {
        const onboarding = await $('[class*="onboard" i], .app').isExisting();
        return onboarding;
      },
      { timeout: 60_000, timeoutMsg: "app root never rendered" },
    );
    await skipOnboardingAndReload();
  });

  it("boots and renders the main window shell", async () => {
    await expect($(".app")).toBeExisting();
    await expect($(".topbar")).toBeExisting();
    // The brand mark is static text in the topbar.
    await expect($(".topbar .brand")).toBeExisting();
  });

  it("mounts the core navigation buttons in the topbar", async () => {
    // These are the always-present nav affordances (App.tsx topbar). Breadth:
    // assert the key ones the task calls out plus a couple of neighbours.
    for (const label of [
      "Branches",
      "History",
      "Manage", // Repository management panel
      "Storage",
      "Account",
    ]) {
      expect(await navButtonExists(label)).toBe(true);
    }
  });

  it("mounts the Branches panel when opened", async () => {
    expect(await clickNavButton("Branches")).toBe(true);
    // Panels render as an overlay/dialog; assert a heading/text unique to it.
    expect(await pageContainsText("Branches")).toBe(true);
    // Close it again (Escape) so the next panel test starts clean.
    await browser.keys(["Escape"]);
  });

  it("mounts the History panel when opened", async () => {
    expect(await clickNavButton("History")).toBe(true);
    expect(await pageContainsText("History")).toBe(true);
    await browser.keys(["Escape"]);
  });

  it("mounts the Repository (Manage) panel when opened", async () => {
    expect(await clickNavButton("Manage")).toBe(true);
    expect(await pageContainsText("Manage")).toBe(true);
    await browser.keys(["Escape"]);
  });

  it("renders the Status / changes view", async () => {
    // The main changes view is `<main className="changes">` (App.tsx). It is
    // always present once onboarding is past, even with no repo open.
    await expect($("main.changes")).toBeExisting();
  });

  it("round-trips read-only IPC commands through the WebView bridge", async () => {
    // Proves the page can actually reach the Rust command layer (the whole
    // point of the E2E build's `withGlobalTauri`). `current_repository` is a
    // pure state read; `auth_local_user_info` exercises a differently-shaped
    // (object/optional) command. Both must cross IPC and deserialize cleanly.
    const repo = await invoke<string | null>("current_repository", {});
    expect(repo).toBeNull();

    // May resolve a cached local identity or reject with the one documented
    // neutral empty-state signal. NoRepository and transport failures are real
    // regressions and must fail this smoke test.
    try {
      const identity = await invoke<{ users: unknown[]; tokens: unknown[] }>(
        "auth_local_user_info",
        { authEndpoint: "", userIds: [], withToken: false },
      );
      expect(Array.isArray(identity.users)).toBe(true);
      expect(Array.isArray(identity.tokens)).toBe(true);
    } catch (error) {
      expect(parseIpcError(error, "auth_local_user_info")).toEqual({
        kind: "CommandFailed",
        message: "No auth endpoint available",
      } satisfies IpcErrorPayload);
    }
  });

  it("round-trips the core VCS read commands through IPC", async () => {
    // status / log / branches all cross the IPC boundary against the real
    // in-process lore engine and reject with the exact structured NoRepository
    // startup error. This proves the full page → invoke → #[tauri::command] →
    // lore-vm error path without swallowing transport or unrelated failures.
    await expectNoRepository("status");
    await expectNoRepository("log", { limit: 5 });
    await expectNoRepository("branches");
  });

  it("attempts the full create → write → stage → commit → status path", async () => {
    // The WRITE path (`repository_create` / `stage` / `commit`) drives the
    // engine in ONLINE mode — the command handlers build `LoreApi::new()`
    // (offline = false). With a reachable lore server hosting the repo this is
    // the real GUI happy path; in a bare CI runner with no server it fails with
    // a gRPC transport error, which we treat as a SKIP (not a failure) so the
    // smoke suite stays green. The deterministic engine-level write round trip
    // is covered by `integration.yml`; the in-process command round trip by
    // `src-tauri/src/ipc_harness_tests.rs::repo_write_lifecycle_through_ipc`
    // (also `#[ignore]`d for the same reason).
    const tag = `e2e-${Date.now()}`;
    const work = `loregui-e2e/${tag}/work`;
    const store = `loregui-e2e/${tag}/store`;

    let created = true;
    try {
      await invoke("repository_create", {
        repositoryUrl: `lore://localhost/${tag}`,
        description: "loregui e2e smoke repo",
        id: "",
        useSharedStore: true,
        sharedStorePath: store,
        path: work,
      });
    } catch (caught) {
      if (!(caught instanceof Error)) throw caught;
      if (caught.message.includes('"kind":"NoRepository"')) {
        throw new Error(
          `repository_create was incorrectly gated by active repository state: ${caught.message}`,
        );
      }
      const unavailableServer =
        /^invoke\(repository_create\) failed: \{"kind":"CommandFailed","message":"Disconnected from server"\}$/;
      if (!unavailableServer.test(caught.message)) throw caught;
      expect(caught.message).toMatch(unavailableServer);
      created = false;
      // eslint-disable-next-line no-console
      console.warn(
        `[e2e] skipping write lifecycle — repository_create needs a reachable ` +
          `lore server: ${caught.message}`,
      );
    }

    if (!created) {
      // Skip the rest of the write path; nothing to assert without a repo.
      return;
    }

    // Creation activates the new repository. Re-open it through the validating
    // command to prove an existing repository is accepted and remains active.
    await invoke("open_repository", { path: work });
    expect(await invoke<string | null>("current_repository", {})).toBe(work);

    await invoke("write_text_file", {
      path: "hello.txt",
      content: "hello from the loregui e2e smoke suite",
    });
    await invoke("stage", { paths: ["hello.txt"] });

    const rev = await invoke<string>("commit", {
      message: "initial commit (e2e smoke)",
    });
    expect(typeof rev).toBe("string");
    expect(rev.length).toBeGreaterThan(0);

    const status = await invoke<{ branch: string; changes: unknown[] }>(
      "status",
      {},
    );
    expect(status).toBeDefined();
    expect(typeof status.branch).toBe("string");
  });
});
