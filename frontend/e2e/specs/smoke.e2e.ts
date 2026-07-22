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

/** Whether a topbar action button with the exact visible label is disabled.
 * Returns null when no exact-label button exists so absence cannot masquerade
 * as the expected fail-closed state. */
async function navButtonDisabled(label: string): Promise<boolean | null> {
  return browser.execute((l: string) => {
    const buttons = Array.from(
      document.querySelectorAll<HTMLButtonElement>(".topbar .actions button"),
    );
    const btn = buttons.find((b) => (b.textContent || "").trim() === l);
    return btn ? btn.disabled : null;
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

/** Install a page-owned audit wrapper around the E2E-only Tauri invoke global.
 * This is reset immediately before a user action, so the resulting log proves
 * that disabled project actions did not cross the WebView/IPC boundary. */
async function auditDisabledRepositoryActions(labels: string[]): Promise<{
  positiveResult: unknown;
  positiveEvents: string[];
  actionEvents: string[];
  missingButtons: string[];
}> {
  return browser.executeAsync(
    (buttonLabels: string[], done: (value: unknown) => void) => {
      const target = window as typeof window & {
        __LOREGUI_E2E_AUDITED_INVOKE__?: (
          command: string,
          args?: Record<string, unknown>,
        ) => Promise<unknown>;
      };
      const auditedInvoke = target.__LOREGUI_E2E_AUDITED_INVOKE__;
      if (!auditedInvoke) {
        done({ __e2eError: "LoreGUI E2E audited invoke seam unavailable" });
        return;
      }
      const root = document.documentElement;
      const attribute = "data-loregui-e2e-ipc-events";
      const readEvents = (): string[] => {
        const parsed: unknown = JSON.parse(root.getAttribute(attribute) ?? "[]");
        return Array.isArray(parsed) ? parsed.map(String) : [];
      };
      root.setAttribute(attribute, "[]");

      auditedInvoke("current_repository", {})
        .then((positiveResult) => {
          const positiveEvents = readEvents();
          root.setAttribute(attribute, "[]");
          const buttons = Array.from(
            document.querySelectorAll<HTMLButtonElement>(".topbar .actions button"),
          );
          const missingButtons: string[] = [];
          for (const label of buttonLabels) {
            const button = buttons.find(
              (candidate) => (candidate.textContent || "").trim() === label,
            );
            if (!button) missingButtons.push(label);
            else button.click();
          }
          window.setTimeout(() => {
            const actionEvents = readEvents();
            root.removeAttribute(attribute);
            done({ positiveResult, positiveEvents, actionEvents, missingButtons });
          }, 0);
        })
        .catch((error) => {
          root.removeAttribute(attribute);
          done({ __e2eError: String(error) });
        });
    },
    labels,
  ) as Promise<{
    positiveResult: unknown;
    positiveEvents: string[];
    actionEvents: string[];
    missingButtons: string[];
  }>;
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

  it("renders the fail-closed no-repository project hub", async () => {
    const projectHub = $("main.project-hub");
    await expect(projectHub).toBeExisting();
    await expect(projectHub).toBeDisplayed();
    await expect($("main.project-hub h1")).toHaveText("Choose a project");

    for (const label of [
      "Branches",
      "History",
      "Locks",
      "Manage",
      "Dependencies",
      "Sync",
      "Push",
      "Verify",
      "Flush",
      "GC",
      "Metadata",
    ]) {
      expect(await navButtonDisabled(label)).toBe(true);
    }

    await expect($("main.changes")).not.toBeExisting();

    // One WebDriver realm owns installation, positive control, action clicks,
    // and log readback. This avoids WebKit's isolated execute-script worlds
    // while proving the recorder sits on Tauri v2's actual internal invoke seam.
    const audit = await auditDisabledRepositoryActions([
      "Branches",
      "History",
      "Locks",
      "Manage",
      "Dependencies",
      "Sync",
      "Push",
      "Verify",
      "Flush",
      "GC",
      "Metadata",
    ]);
    expect(audit.positiveResult).toBeNull();
    expect(audit.positiveEvents).toEqual(["current_repository"]);
    expect(audit.missingButtons).toEqual([]);
    expect(audit.actionEvents).toEqual([]);
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

  it("propagates an exact repository-create failure without activating its local path", async () => {
    // Failure is evidence, not a green skip: this deliberately unreachable
    // fixture must preserve the exact backend error and keep repository state
    // null. The deterministic successful host→open→restart journey runs in the
    // MockRuntime IPC harness with fixture-owned server/client directories.
    const tag = `e2e-${Date.now()}`;
    const work = `loregui-e2e/${tag}/work`;
    const store = `loregui-e2e/${tag}/store`;

    try {
      await invoke("repository_create", {
        repositoryUrl: `lore://127.0.0.1:1/${tag}`,
        description: "loregui e2e smoke repo",
        id: "",
        useSharedStore: true,
        sharedStorePath: store,
        path: work,
      });
    } catch (caught) {
      expect(parseIpcError(caught, "repository_create")).toEqual({
        kind: "CommandFailed",
        message: "Disconnected from server",
      } satisfies IpcErrorPayload);
      expect(await invoke<string | null>("current_repository", {})).toBeNull();
      return;
    }
    throw new Error("repository_create unexpectedly resolved for unreachable fixture");
  });
});
