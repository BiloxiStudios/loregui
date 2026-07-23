/**
 * Component tests for UrcStatusCard (SBAI-5499).
 *
 * The api wrappers are mocked at the module boundary; the card is fed a
 * UrcStatus snapshot (or a fetch error) exactly as App.tsx's refresh() hands
 * them down. Covers every first-class state — loading, healthy (quiet),
 * pendingMerge, conflicts, diverged, error/recovery — plus action wiring,
 * destructive confirms, and action-error surfacing.
 */
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { UrcStatus } from "./api";

const recoverLocalMock = vi.fn();
const syncMock = vi.fn();
const mergeAbortMock = vi.fn();
const resolveMineMock = vi.fn();
const resolveTheirsMock = vi.fn();

vi.mock("./api", () => ({
  repositoryRecoverApi: {
    recoverLocal: (...args: unknown[]) => recoverLocalMock(...args),
  },
  revisionSyncApi: { sync: (...args: unknown[]) => syncMock(...args) },
  branchMergeAbortApi: {
    mergeAbort: (...args: unknown[]) => mergeAbortMock(...args),
  },
  branchMergeResolveMineApi: {
    mergeResolveMine: (...args: unknown[]) => resolveMineMock(...args),
  },
  branchMergeResolveTheirsApi: {
    mergeResolveTheirs: (...args: unknown[]) => resolveTheirsMock(...args),
  },
}));

import UrcStatusCard from "./UrcStatusCard";

const HEALTHY: UrcStatus = {
  currentRev: "aaa111bbb222",
  remoteRev: "aaa111bbb222",
  pendingMerge: false,
  branch: "main",
  diverged: false,
  staged: [],
  conflicts: [],
  healthy: true,
};

const PENDING_MERGE: UrcStatus = {
  ...HEALTHY,
  healthy: false,
  pendingMerge: true,
  remoteRev: "ccc333ddd444",
  conflicts: ["assets/map.umap"],
};

const CONFLICTS: UrcStatus = {
  ...HEALTHY,
  healthy: false,
  conflicts: ["a.txt", "b.txt"],
};

const DIVERGED: UrcStatus = {
  ...HEALTHY,
  healthy: false,
  diverged: true,
  currentRev: "aaa111bbb222",
  remoteRev: "ccc333ddd444",
  staged: ["foo.txt"],
};

beforeEach(() => {
  recoverLocalMock.mockReset().mockResolvedValue({
    recoveredDir: "/srv/repos/world-bible",
    preservedDir: "/srv/repos/world-bible.preserved",
    status: HEALTHY,
  });
  syncMock.mockReset().mockResolvedValue({
    files: [],
    revisions: [],
    files_updated: 0,
    files_deleted: 0,
  });
  mergeAbortMock.mockReset().mockResolvedValue({
    staged_revision: "",
    current_revision: "aaa111bbb222",
  });
  resolveMineMock.mockReset().mockResolvedValue({
    resolved_paths: [],
    revision: "aaa111bbb222",
  });
  resolveTheirsMock.mockReset().mockResolvedValue({
    resolved_paths: [],
    revision: "ccc333ddd444",
  });
});

describe("UrcStatusCard", () => {
  it("renders a quiet loading line while the first fetch is in flight", () => {
    render(
      <UrcStatusCard status={null} error={null} loading onRefresh={vi.fn()} />,
    );
    expect(screen.getByText("checking repository state…")).toBeVisible();
  });

  it("renders nothing for a healthy tree (quiet success)", () => {
    const { container } = render(
      <UrcStatusCard status={HEALTHY} error={null} onRefresh={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("pendingMerge shows the incoming revision and aborts via the merge wrapper", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const onRefresh = vi.fn().mockResolvedValue(undefined);
    render(
      <UrcStatusCard status={PENDING_MERGE} error={null} onRefresh={onRefresh} />,
    );

    expect(screen.getByText("merge in progress")).toBeVisible();
    expect(screen.getByText("needs resolution")).toBeVisible();
    expect(screen.getByText("ccc333ddd444")).toBeVisible();
    expect(screen.getByText("assets/map.umap")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "abort merge" }));
    await waitFor(() => expect(mergeAbortMock).toHaveBeenCalledWith());
    await waitFor(() => expect(onRefresh).toHaveBeenCalled());
  });

  it("abort merge does nothing when the confirm is declined", () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(
      <UrcStatusCard status={PENDING_MERGE} error={null} onRefresh={vi.fn()} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "abort merge" }));
    expect(mergeAbortMock).not.toHaveBeenCalled();
  });

  it("conflicts lists every path and resolves mine/theirs with those paths", async () => {
    const onRefresh = vi.fn().mockResolvedValue(undefined);
    render(
      <UrcStatusCard status={CONFLICTS} error={null} onRefresh={onRefresh} />,
    );

    expect(screen.getByText("a.txt")).toBeVisible();
    expect(screen.getByText("b.txt")).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "resolve mine" }));
    await waitFor(() =>
      expect(resolveMineMock).toHaveBeenCalledWith(["a.txt", "b.txt"]),
    );
    await waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: "resolve theirs" }));
    await waitFor(() =>
      expect(resolveTheirsMock).toHaveBeenCalledWith(["a.txt", "b.txt"]),
    );
    await waitFor(() => expect(onRefresh).toHaveBeenCalledTimes(2));
  });

  it("diverged shows both revisions and hard-syncs only behind an explicit confirm", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(false);
    const onRefresh = vi.fn().mockResolvedValue(undefined);
    render(
      <UrcStatusCard status={DIVERGED} error={null} onRefresh={onRefresh} />,
    );

    expect(screen.getByText("branch diverged")).toBeVisible();
    expect(screen.getByText("aaa111bbb222")).toBeVisible();
    expect(screen.getByText("ccc333ddd444")).toBeVisible();
    expect(screen.getByText(/1 staged file/)).toBeVisible();

    fireEvent.click(screen.getByRole("button", { name: "reset to remote" }));
    expect(syncMock).not.toHaveBeenCalled();

    confirmSpy.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "reset to remote" }));
    await waitFor(() =>
      expect(syncMock).toHaveBeenCalledWith("ccc333ddd444", false, true),
    );
    await waitFor(() => expect(onRefresh).toHaveBeenCalled());
  });

  it("error state offers recovery, shows the result, and re-runs refresh", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    const onRefresh = vi.fn().mockResolvedValue(undefined);
    render(
      <UrcStatusCard
        status={null}
        error="io error: working tree unreadable"
        onRefresh={onRefresh}
      />,
    );

    expect(screen.getByText("repository unreachable")).toBeVisible();
    expect(screen.getByText("io error: working tree unreadable")).toBeVisible();

    fireEvent.click(
      screen.getByRole("button", { name: "recover local repository" }),
    );
    await waitFor(() => expect(recoverLocalMock).toHaveBeenCalledWith());
    expect(await screen.findByText("/srv/repos/world-bible")).toBeVisible();
    expect(
      screen.getByText("/srv/repos/world-bible.preserved"),
    ).toBeVisible();
    await waitFor(() => expect(onRefresh).toHaveBeenCalled());
  });

  it("recovery requires the confirm too", () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(
      <UrcStatusCard status={null} error="unreachable" onRefresh={vi.fn()} />,
    );
    fireEvent.click(
      screen.getByRole("button", { name: "recover local repository" }),
    );
    expect(recoverLocalMock).not.toHaveBeenCalled();
  });

  it("surfaces the real backend message when an action fails", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    recoverLocalMock.mockRejectedValue(new Error("no space left on device"));
    render(
      <UrcStatusCard status={null} error="unreachable" onRefresh={vi.fn()} />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: "recover local repository" }),
    );
    expect(
      await screen.findByText("no space left on device"),
    ).toBeVisible();
  });
});
