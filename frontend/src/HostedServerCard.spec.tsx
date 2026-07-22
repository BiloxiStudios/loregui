import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HostStatus } from "./api";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

interface Deferred<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const RUNNING_NO_AUTH: HostStatus = {
  running: true,
  pid: 4242,
  port: 41337,
  httpPort: 41339,
  url: "lore://127.0.0.1:41337/world-bible",
  advertisedUrl: "lore://192.168.1.8:41337/world-bible",
  configPath: "E:\\lore\\.loregui-host\\local.toml",
  storeDir: "E:\\lore",
  serverName: "world-bible",
  authRequired: false,
};

const STOPPED: HostStatus = { running: false };

async function loadCard() {
  const module = await import("./HostedServerCard");
  return module.default;
}

function routeStatus(...responses: Array<HostStatus | Promise<HostStatus>>) {
  let index = 0;
  invokeMock.mockImplementation((command: string) => {
    if (command === "host_server_status") {
      const response = responses[Math.min(index, responses.length - 1)];
      index += 1;
      return Promise.resolve(response);
    }
    if (command === "host_server_stop") return Promise.resolve(STOPPED);
    return Promise.resolve(null);
  });
}

beforeEach(() => {
  vi.resetModules();
  invokeMock.mockReset();
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: vi.fn().mockResolvedValue(undefined) },
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe("HostedServerCard", () => {
  it("keeps exact running no-auth context and actions visible after onboarding unmount", async () => {
    routeStatus(RUNNING_NO_AUTH);
    const HostedServerCard = await loadCard();
    const onBrowseRepositories = vi.fn();
    const promptSpy = vi.spyOn(window, "prompt");
    const first = render(
      <HostedServerCard onBrowseRepositories={onBrowseRepositories} />,
    );

    expect(await screen.findByText("Hosted on this device")).toBeVisible();
    expect(screen.getByText("world-bible")).toBeVisible();
    expect(screen.getByText("E:\\lore")).toBeVisible();
    expect(
      screen.getByText("lore://192.168.1.8:41337/world-bible"),
    ).toBeVisible();
    expect(
      screen.getByText("lore://127.0.0.1:41337/world-bible"),
    ).toBeVisible();
    expect(screen.getByText("Authentication: Not required")).toBeVisible();
    expect(screen.getByText("PID 4242 · Process running")).toBeVisible();
    expect(screen.queryByText(/Healthy/i)).toBeNull();

    first.unmount();
    render(<HostedServerCard onBrowseRepositories={onBrowseRepositories} />);
    await screen.findByText("Hosted on this device");

    fireEvent.click(screen.getByRole("button", { name: "Browse repositories" }));
    expect(onBrowseRepositories).toHaveBeenCalledWith(
      "lore://192.168.1.8:41337/world-bible",
      expect.any(AbortSignal),
    );
    expect(promptSpy).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Copy URL" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Stop" })).toBeEnabled();
  });

  it("reports auth-required only from backend status and never renders secrets", async () => {
    routeStatus({
      ...RUNNING_NO_AUTH,
      authRequired: true,
      serverName: undefined,
    });
    const HostedServerCard = await loadCard();
    const { container } = render(
      <HostedServerCard onBrowseRepositories={() => {}} />,
    );

    expect(await screen.findByText("Authentication: Required")).toBeVisible();
    expect(screen.getByText("Unnamed server")).toBeVisible();
    expect(container.textContent).not.toContain("E:\\lore\\.loregui-host\\local.toml");
  });

  it("shows clipboard success and failure for the exact displayed client URL", async () => {
    routeStatus(RUNNING_NO_AUTH);
    const HostedServerCard = await loadCard();
    render(<HostedServerCard onBrowseRepositories={() => {}} />);
    await screen.findByRole("button", { name: "Copy URL" });
    const writeText = vi.mocked(navigator.clipboard.writeText);

    fireEvent.click(screen.getByRole("button", { name: "Copy URL" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledTimes(1));
    expect(writeText).toHaveBeenCalledWith(
      "lore://192.168.1.8:41337/world-bible",
    );
    expect(screen.getByRole("status")).toHaveTextContent("URL copied");

    writeText.mockRejectedValueOnce(new Error("clipboard denied"));
    fireEvent.click(screen.getByRole("button", { name: "Copy URL" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not copy the server URL.");
  });

  it.each(["resolve", "reject"] as const)(
    "suppresses a pending copy %s after a newer stopped poll",
    async (completion) => {
      vi.useFakeTimers();
      const copy = deferred<void>();
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: vi.fn(() => copy.promise) },
      });
      routeStatus(RUNNING_NO_AUTH, STOPPED);
      const HostedServerCard = await loadCard();
      render(
        <HostedServerCard onBrowseRepositories={() => {}} pollIntervalMs={25} />,
      );
      await act(async () => {
        await Promise.resolve();
      });
      fireEvent.click(screen.getByRole("button", { name: "Copy URL" }));

      await act(async () => {
        await vi.advanceTimersByTimeAsync(25);
      });
      expect(screen.getByText("Server stopped")).toBeVisible();

      await act(async () => {
        if (completion === "resolve") copy.resolve();
        else copy.reject(new Error("late clipboard failure"));
        await Promise.resolve();
      });
      expect(screen.queryByText("URL copied.")).toBeNull();
      expect(screen.queryByText("Could not copy the server URL.")).toBeNull();
    },
  );

  it("invalidates a pending copy when the hosted URL changes", async () => {
    vi.useFakeTimers();
    const copy = deferred<void>();
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText: vi.fn(() => copy.promise) },
    });
    const changed = {
      ...RUNNING_NO_AUTH,
      advertisedUrl: "lore://192.168.1.9:41337/world-bible",
    };
    routeStatus(RUNNING_NO_AUTH, changed);
    const HostedServerCard = await loadCard();
    render(<HostedServerCard onBrowseRepositories={() => {}} pollIntervalMs={25} />);
    await act(async () => {
      await Promise.resolve();
    });
    fireEvent.click(screen.getByRole("button", { name: "Copy URL" }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(25);
      copy.resolve();
      await Promise.resolve();
    });
    expect(screen.getByText(changed.advertisedUrl)).toBeVisible();
    expect(screen.queryByText("URL copied.")).toBeNull();
  });

  it("keeps Stop ownership when Copy is attempted during Stop", async () => {
    const stop = deferred<HostStatus>();
    routeStatus(RUNNING_NO_AUTH);
    invokeMock.mockImplementationOnce(() => Promise.resolve(RUNNING_NO_AUTH));
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_stop") return stop.promise;
      if (command === "host_server_status") return Promise.resolve(RUNNING_NO_AUTH);
      return Promise.resolve(null);
    });
    const HostedServerCard = await loadCard();
    render(<HostedServerCard onBrowseRepositories={() => {}} />);
    await screen.findByText("PID 4242 · Process running");

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(screen.getByRole("button", { name: "Copy URL" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Browse repositories" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Copy URL" }));

    await act(async () => {
      stop.resolve(STOPPED);
      await Promise.resolve();
    });
    expect(screen.getByText("Server stopped")).toBeVisible();
    expect(screen.queryByText("Process running")).toBeNull();
  });

  it.each(["stopped", "url-changed"] as const)(
    "aborts an in-flight Browse when status becomes %s",
    async (nextKind) => {
      vi.useFakeTimers();
      const browse = deferred<void>();
      let capturedSignal: AbortSignal | undefined;
      const onBrowseRepositories = vi.fn((_url: string, signal: AbortSignal) => {
        capturedSignal = signal;
        return browse.promise;
      });
      const next = nextKind === "stopped"
        ? STOPPED
        : {
            ...RUNNING_NO_AUTH,
            advertisedUrl: "lore://192.168.1.9:41337/world-bible",
          };
      routeStatus(RUNNING_NO_AUTH, next);
      const HostedServerCard = await loadCard();
      render(
        <HostedServerCard
          onBrowseRepositories={onBrowseRepositories}
          pollIntervalMs={25}
        />,
      );
      await act(async () => {
        await Promise.resolve();
      });
      fireEvent.click(screen.getByRole("button", { name: "Browse repositories" }));
      expect(capturedSignal?.aborted).toBe(false);

      await act(async () => {
        await vi.advanceTimersByTimeAsync(25);
      });
      expect(capturedSignal?.aborted).toBe(true);
      browse.resolve();
    },
  );

  it("aborts an in-flight Browse on a status error", async () => {
    vi.useFakeTimers();
    const failedPoll = deferred<HostStatus>();
    let capturedSignal: AbortSignal | undefined;
    routeStatus(RUNNING_NO_AUTH, failedPoll.promise);
    const HostedServerCard = await loadCard();
    render(
      <HostedServerCard
        onBrowseRepositories={(_url, signal) => {
          capturedSignal = signal;
          return new Promise<void>(() => {});
        }}
        pollIntervalMs={25}
      />,
    );
    await act(async () => {
      await Promise.resolve();
    });
    fireEvent.click(screen.getByRole("button", { name: "Browse repositories" }));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(25);
      failedPoll.reject(new Error("host status unavailable"));
      await Promise.resolve();
    });
    expect(capturedSignal?.aborted).toBe(true);
  });

  it("aborts Browse on newer Browse, Stop, and unmount", async () => {
    const stop = deferred<HostStatus>();
    const signals: AbortSignal[] = [];
    routeStatus(RUNNING_NO_AUTH);
    invokeMock.mockImplementationOnce(() => Promise.resolve(RUNNING_NO_AUTH));
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_stop") return stop.promise;
      if (command === "host_server_status") return Promise.resolve(RUNNING_NO_AUTH);
      return Promise.resolve(null);
    });
    const HostedServerCard = await loadCard();
    const view = render(
      <HostedServerCard
        onBrowseRepositories={(_url, signal) => {
          signals.push(signal);
          return new Promise<void>(() => {});
        }}
      />,
    );
    await screen.findByText("PID 4242 · Process running");
    const browse = screen.getByRole("button", { name: "Browse repositories" });

    fireEvent.click(browse);
    fireEvent.click(browse);
    expect(signals[0]?.aborted).toBe(true);
    expect(signals[1]?.aborted).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(signals[1]?.aborted).toBe(true);
    await act(async () => {
      stop.resolve(RUNNING_NO_AUTH);
      await Promise.resolve();
    });
    fireEvent.click(screen.getByRole("button", { name: "Browse repositories" }));
    expect(signals[2]?.aborted).toBe(false);
    view.unmount();
    expect(signals[2]?.aborted).toBe(true);
  });

  it.each(["resolve", "reject"] as const)(
    "suppresses a pending copy %s after a newer status error",
    async (completion) => {
      vi.useFakeTimers();
      const copy = deferred<void>();
      Object.defineProperty(navigator, "clipboard", {
        configurable: true,
        value: { writeText: vi.fn(() => copy.promise) },
      });
      const failedPoll = deferred<HostStatus>();
      routeStatus(RUNNING_NO_AUTH, failedPoll.promise);
      const HostedServerCard = await loadCard();
      render(
        <HostedServerCard onBrowseRepositories={() => {}} pollIntervalMs={25} />,
      );
      await act(async () => {
        await Promise.resolve();
      });
      fireEvent.click(screen.getByRole("button", { name: "Copy URL" }));

      await act(async () => {
        await vi.advanceTimersByTimeAsync(25);
        failedPoll.reject(new Error("host status unavailable"));
        await Promise.resolve();
      });
      expect(screen.getByRole("alert")).toHaveTextContent("host status unavailable");

      await act(async () => {
        if (completion === "resolve") copy.resolve();
        else copy.reject(new Error("late clipboard failure"));
        await Promise.resolve();
      });
      expect(screen.queryByText("URL copied.")).toBeNull();
      expect(screen.queryByText("Could not copy the server URL.")).toBeNull();
    },
  );

  it("lets a newer stopped poll win over an older running poll", async () => {
    vi.useFakeTimers();
    const older = deferred<HostStatus>();
    const newer = deferred<HostStatus>();
    routeStatus(older.promise, newer.promise);
    const HostedServerCard = await loadCard();
    render(
      <HostedServerCard
        onBrowseRepositories={() => {}}
        pollIntervalMs={25}
      />,
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(25);
      newer.resolve(STOPPED);
      await Promise.resolve();
    });
    expect(screen.getByText("Server stopped")).toBeVisible();

    await act(async () => {
      older.resolve(RUNNING_NO_AUTH);
      await Promise.resolve();
    });
    expect(screen.getByText("Server stopped")).toBeVisible();
    expect(screen.queryByText("Process running")).toBeNull();
  });

  it("lets a newer poll error win over an older running poll", async () => {
    vi.useFakeTimers();
    const older = deferred<HostStatus>();
    const newer = deferred<HostStatus>();
    routeStatus(older.promise, newer.promise);
    const HostedServerCard = await loadCard();
    render(
      <HostedServerCard onBrowseRepositories={() => {}} pollIntervalMs={25} />,
    );

    await act(async () => {
      await vi.advanceTimersByTimeAsync(25);
      newer.reject(new Error("status unavailable"));
      await Promise.resolve();
    });
    expect(screen.getByRole("alert")).toHaveTextContent("status unavailable");

    await act(async () => {
      older.resolve(RUNNING_NO_AUTH);
      await Promise.resolve();
    });
    expect(screen.getByRole("alert")).toHaveTextContent("status unavailable");
    expect(screen.queryByText("Process running")).toBeNull();
  });

  it.each(["poll-first", "stop-first"] as const)(
    "does not restore running state in the stop/poll race (%s completion)",
    async (completionOrder) => {
      vi.useFakeTimers();
      const poll = deferred<HostStatus>();
      const stop = deferred<HostStatus>();
      routeStatus(RUNNING_NO_AUTH, poll.promise);
      invokeMock.mockImplementationOnce(() => Promise.resolve(RUNNING_NO_AUTH));
      invokeMock.mockImplementation((command: string) => {
        if (command === "host_server_status") return poll.promise;
        if (command === "host_server_stop") return stop.promise;
        return Promise.resolve(null);
      });
      const HostedServerCard = await loadCard();
      render(
        <HostedServerCard
          onBrowseRepositories={() => {}}
          pollIntervalMs={25}
        />,
      );
      await act(async () => {
        await Promise.resolve();
      });
      expect(screen.getByText("PID 4242 · Process running")).toBeVisible();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(25);
      });
      fireEvent.click(screen.getByRole("button", { name: "Stop" }));

      await act(async () => {
        if (completionOrder === "poll-first") {
          poll.resolve(RUNNING_NO_AUTH);
          await Promise.resolve();
          stop.resolve(STOPPED);
        } else {
          stop.resolve(STOPPED);
          await Promise.resolve();
          poll.resolve(RUNNING_NO_AUTH);
        }
        await Promise.resolve();
      });

      expect(screen.getByText("Server stopped")).toBeVisible();
      expect(screen.queryByText("PID 4242 · Process running")).toBeNull();
    },
  );

  it("clears running claims on malformed status and poll errors but retains context", async () => {
    vi.useFakeTimers();
    const malformed: HostStatus = {
      running: true,
      pid: 4242,
      url: RUNNING_NO_AUTH.url,
      authRequired: false,
    };
    const failure = deferred<HostStatus>();
    routeStatus(RUNNING_NO_AUTH, malformed, failure.promise);
    const HostedServerCard = await loadCard();
    render(
      <HostedServerCard
        onBrowseRepositories={() => {}}
        pollIntervalMs={25}
      />,
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByText("PID 4242 · Process running")).toBeVisible();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(25);
    });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Hosted server status is incomplete.",
    );
    expect(screen.getByText("E:\\lore")).toBeVisible();
    expect(screen.queryByText("Process running")).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(25);
      failure.reject(new Error("status unavailable"));
      await Promise.resolve();
    });
    expect(screen.getByRole("alert")).toHaveTextContent("status unavailable");
    expect(screen.queryByText("Process running")).toBeNull();
  });

  it("invalidates pending polls and actions on unmount", async () => {
    const late = deferred<HostStatus>();
    routeStatus(late.promise, STOPPED);
    const HostedServerCard = await loadCard();
    const first = render(
      <HostedServerCard onBrowseRepositories={() => {}} />,
    );
    first.unmount();

    await act(async () => {
      late.resolve(RUNNING_NO_AUTH);
      await Promise.resolve();
    });

    render(<HostedServerCard onBrowseRepositories={() => {}} />);
    expect(await screen.findByText("Server stopped")).toBeVisible();
    expect(screen.queryByText("E:\\lore")).toBeNull();
  });

  it("does not apply a pending stop completion after unmount", async () => {
    const stop = deferred<HostStatus>();
    invokeMock.mockImplementation((command: string) => {
      if (command === "host_server_status") return Promise.resolve(RUNNING_NO_AUTH);
      if (command === "host_server_stop") return stop.promise;
      return Promise.resolve(null);
    });
    const HostedServerCard = await loadCard();
    const first = render(<HostedServerCard onBrowseRepositories={() => {}} />);
    fireEvent.click(await screen.findByRole("button", { name: "Stop" }));
    first.unmount();

    await act(async () => {
      stop.resolve({
        ...RUNNING_NO_AUTH,
        storeDir: "E:\\stale-stop",
        serverName: "stale-stop",
      });
      await Promise.resolve();
    });

    invokeMock.mockImplementation((command: string) =>
      command === "host_server_status" ? Promise.resolve(STOPPED) : Promise.resolve(null),
    );
    render(<HostedServerCard onBrowseRepositories={() => {}} />);
    expect(await screen.findByText("Server stopped")).toBeVisible();
    expect(screen.queryByText("E:\\stale-stop")).toBeNull();
    expect(screen.queryByText("stale-stop")).toBeNull();
  });

  it("retains last non-secret configuration when stopped and disables running actions", async () => {
    vi.useFakeTimers();
    routeStatus(RUNNING_NO_AUTH, STOPPED);
    const HostedServerCard = await loadCard();
    render(
      <HostedServerCard
        onBrowseRepositories={() => {}}
        pollIntervalMs={25}
      />,
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(screen.getByText("PID 4242 · Process running")).toBeVisible();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(25);
    });
    expect(screen.getByText("Server stopped")).toBeVisible();
    expect(screen.getByText("E:\\lore")).toBeVisible();
    expect(screen.queryByText(/PID 4242/)).toBeNull();
    expect(screen.queryByText(/Process running/)).toBeNull();
    expect(screen.getByRole("button", { name: "Browse repositories" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Copy URL" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Restart" })).toBeDisabled();
    expect(screen.getByText(/backend does not retain a secret-free full launch configuration/i)).toBeVisible();
  });
});
