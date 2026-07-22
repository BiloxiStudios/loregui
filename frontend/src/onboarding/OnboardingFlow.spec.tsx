import { useState } from "react";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

const pendingReports = vi.hoisted(() => ({
  connect: null as null | (() => void),
  repository: null as null | (() => void),
}));

vi.mock("./ModeSelect", () => ({
  default: ({ onModeSelect }: { onModeSelect: (mode: "client" | "host") => void }) => (
    <div>
      <button onClick={() => onModeSelect("client")}>Choose client</button>
      <button onClick={() => onModeSelect("host")}>Choose host</button>
    </div>
  ),
}));

vi.mock("./ClientConnect", () => ({
  default: ({ onStateChange }: StepProps<string>) => (
    <div>
      <h2>Connect mock</h2>
      <button onClick={() => onStateChange({ status: "working" })}>Connect working</button>
      <button onClick={() => onStateChange({ status: "error", message: "refused" })}>Connect error</button>
      <button
        onClick={() =>
          onStateChange({ status: "success", value: "lore://server.example/team" })
        }
      >
        Connect success
      </button>
      <button
        onClick={() => {
          pendingReports.connect = () =>
            onStateChange({
              status: "success",
              value: "lore://stale.example/team",
            });
        }}
      >
        Connect pending
      </button>
    </div>
  ),
}));

vi.mock("./ClientClone", () => ({
  default: ({ initialMode = "choice", initialCloneUrl, onStateChange }: StepProps<string> & { initialMode?: "choice" | "clone" | "open" | "create"; initialCloneUrl?: string }) => {
    // Mirrors ClientClone's useState(initialMode) behavior so the shell must
    // remount it when a host next action changes.
    const [renderedMode] = useState(initialMode);
    return (
      <div>
        <h2>Repository mock</h2>
        {renderedMode === "clone" && <h3>Clone Repository</h3>}
        {renderedMode === "open" && <h3>Open Working Tree</h3>}
        {renderedMode === "create" && <h3>Create Local Project</h3>}
        <label>
          Forwarded server
          <input readOnly value={initialCloneUrl ?? ""} />
        </label>
        <button onClick={() => onStateChange({ status: "working" })}>Repository working</button>
        <button onClick={() => onStateChange({ status: "error", message: "open failed" })}>Repository error</button>
        <button onClick={() => onStateChange({ status: "success", value: "/repo" })}>
          Repository success
        </button>
        <button
          onClick={() => {
            pendingReports.repository = () =>
              onStateChange({ status: "success", value: "/stale-repo" });
          }}
        >
          Repository pending
        </button>
      </div>
    );
  },
}));

vi.mock("./server/BackendPicker", () => ({
  default: ({ onStateChange, onConfigured }: StepProps<Record<string, unknown>> & { onConfigured?: (value: Record<string, unknown>) => void }) => (
    <div>
      <h2>Backend mock</h2>
      <button onClick={() => onStateChange({ status: "working" })}>Backend working</button>
      <button onClick={() => onStateChange({ status: "error", message: "bad backend" })}>Backend error</button>
      <button
        onClick={() => {
          const value = { kind: "local", path: "/store" };
          onConfigured?.(value);
          onStateChange({ status: "success", value });
        }}
      >
        Backend success
      </button>
    </div>
  ),
}));

vi.mock("./server/ValidateConnectivity", () => ({
  default: ({ onStateChange }: StepProps) => (
    <div>
      <h2>Validate mock</h2>
      <button onClick={() => onStateChange({ status: "success" })}>Validate success</button>
    </div>
  ),
}));

vi.mock("./server/InitStore", () => ({
  default: ({ onStateChange, onInitialized }: StepProps<{ storePath: string; repoName: string }> & { onInitialized?: (value: { storePath: string; repoName: string }) => void }) => (
    <div>
      <h2>Initialize mock</h2>
      <button
        onClick={() => {
          const value = { storePath: "/store", repoName: "team" };
          onInitialized?.(value);
          onStateChange({ status: "success", value });
        }}
      >
        Initialize success
      </button>
    </div>
  ),
}));

vi.mock("./server/ServiceSetup", () => ({
  default: ({ onStateChange }: StepProps<string>) => (
    <div>
      <h2>Service mock</h2>
      <button onClick={() => onStateChange({ status: "working" })}>Service working</button>
      <button onClick={() => onStateChange({ status: "error", message: "start failed" })}>Service error</button>
      <button
        onClick={() =>
          onStateChange({
            status: "error",
            message:
              "A Lore server is already running from /other, not this flow's store /store. Stop it before continuing.",
          })
        }
      >
        Unrelated running service
      </button>
      <button
        onClick={() =>
          onStateChange({ status: "success", value: "lore://localhost/team" })
        }
      >
        Service success
      </button>
    </div>
  ),
}));

import OnboardingFlow from "./OnboardingFlow";

type Result<T = void> = {
  status: "idle" | "working" | "success" | "error";
  value?: T;
  message?: string;
};

type StepProps<T = void> = {
  onStateChange: (result: Result<T>) => void;
};

function forceNavigation(buttonName: "Continue" | "Finish") {
  const button = screen.getByRole("button", { name: buttonName });
  button.removeAttribute("disabled");
  fireEvent.click(button);
}

function completeHostThroughService() {
  fireEvent.click(screen.getByRole("button", { name: "Backend success" }));
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  fireEvent.click(screen.getByRole("button", { name: "Validate success" }));
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  fireEvent.click(screen.getByRole("button", { name: "Initialize success" }));
  fireEvent.click(screen.getByRole("button", { name: "Continue" }));
  fireEvent.click(screen.getByRole("button", { name: "Service success" }));
}

beforeEach(() => {
  vi.clearAllMocks();
  pendingReports.connect = null;
  pendingReports.repository = null;
});

describe("explicit onboarding navigation state", () => {
  it("guards navigation through idle, working, and error, then advances on success", () => {
    render(<OnboardingFlow initialIntent="host" onComplete={vi.fn()} />);

    forceNavigation("Continue");
    expect(screen.queryByRole("heading", { name: "Validate mock" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Backend working" }));
    forceNavigation("Continue");
    expect(screen.queryByRole("heading", { name: "Validate mock" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Backend error" }));
    forceNavigation("Continue");
    expect(screen.queryByRole("heading", { name: "Validate mock" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Backend success" }));
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    expect(screen.getByRole("heading", { name: "Validate mock" })).toBeVisible();
  });

  it("clears stale success when a child reports an error", () => {
    render(<OnboardingFlow initialIntent="host" onComplete={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Backend success" }));
    fireEvent.click(screen.getByRole("button", { name: "Backend error" }));

    forceNavigation("Continue");
    expect(screen.queryByRole("heading", { name: "Validate mock" })).toBeNull();
  });

  it("clears downstream success after moving backward and switching modes", () => {
    render(<OnboardingFlow onComplete={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Choose host" }));
    fireEvent.click(screen.getByRole("button", { name: "Backend success" }));
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    fireEvent.click(screen.getByRole("button", { name: "Validate success" }));
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose client" }));

    forceNavigation("Continue");
    expect(screen.queryByRole("heading", { name: "Repository mock" })).toBeNull();
  });

  it("forwards the exact successful Connect URL into the repository step", () => {
    render(<OnboardingFlow initialIntent="connect" onComplete={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Connect success" }));
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));

    expect(screen.getByLabelText("Forwarded server")).toHaveValue(
      "lore://server.example/team",
    );
  });

  it("finishes client onboarding only after a repository succeeds", () => {
    const onComplete = vi.fn();
    render(<OnboardingFlow initialIntent="open" onComplete={onComplete} />);

    forceNavigation("Finish");
    expect(onComplete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Repository working" }));
    forceNavigation("Finish");
    expect(onComplete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Repository error" }));
    forceNavigation("Finish");
    expect(onComplete).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Repository success" }));
    fireEvent.click(screen.getByRole("button", { name: "Finish" }));
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it("requires an explicit Manage server only choice after host success", () => {
    const onComplete = vi.fn();
    render(<OnboardingFlow initialIntent="host" onComplete={onComplete} />);
    completeHostThroughService();

    forceNavigation("Finish");
    expect(onComplete).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("radio", { name: "Manage server only" }));
    expect(screen.getByText(/Repository actions will remain unavailable/i)).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "Finish" }));
    expect(onComplete).toHaveBeenCalledTimes(1);
  });

  it("remounts repository acquisition when host next action changes", () => {
    render(<OnboardingFlow initialIntent="host" onComplete={vi.fn()} />);
    completeHostThroughService();

    fireEvent.click(screen.getByRole("radio", { name: "Create repository" }));
    expect(screen.getByRole("heading", { name: "Create Local Project" })).toBeVisible();

    fireEvent.click(screen.getByRole("radio", { name: "Open existing" }));
    expect(screen.getByRole("heading", { name: "Open Working Tree" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Create Local Project" })).toBeNull();

    fireEvent.click(screen.getByRole("radio", { name: "Browse repositories" }));
    expect(screen.getByRole("heading", { name: "Clone Repository" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Open Working Tree" })).toBeNull();
  });

  it("does not finish for an unrelated running host service", () => {
    const onComplete = vi.fn();
    render(<OnboardingFlow initialIntent="host" onComplete={onComplete} />);
    fireEvent.click(screen.getByRole("button", { name: "Backend success" }));
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    fireEvent.click(screen.getByRole("button", { name: "Validate success" }));
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    fireEvent.click(screen.getByRole("button", { name: "Initialize success" }));
    fireEvent.click(screen.getByRole("button", { name: "Continue" }));
    fireEvent.click(screen.getByRole("button", { name: "Unrelated running service" }));

    const finish = screen.getByRole("button", { name: "Finish" });
    expect(finish).toHaveAttribute(
      "title",
      "A Lore server is already running from /other, not this flow's store /store. Stop it before continuing.",
    );
    forceNavigation("Finish");
    expect(onComplete).not.toHaveBeenCalled();
  });

  it("ignores a Connect success reported after Back and a mode change", async () => {
    render(<OnboardingFlow initialIntent="connect" onComplete={vi.fn()} />);
    fireEvent.click(screen.getByRole("button", { name: "Connect pending" }));
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(screen.getByRole("button", { name: "Choose client" }));

    await act(async () => pendingReports.connect?.());
    forceNavigation("Continue");
    expect(screen.queryByRole("heading", { name: "Repository mock" })).toBeNull();
  });

  it("ignores a host repository success after switching its next action", async () => {
    const onComplete = vi.fn();
    render(<OnboardingFlow initialIntent="host" onComplete={onComplete} />);
    completeHostThroughService();
    fireEvent.click(screen.getByRole("radio", { name: "Create repository" }));
    fireEvent.click(screen.getByRole("button", { name: "Repository pending" }));
    fireEvent.click(screen.getByRole("radio", { name: "Open existing" }));

    await act(async () => pendingReports.repository?.());
    forceNavigation("Finish");
    expect(onComplete).not.toHaveBeenCalled();
  });
});
