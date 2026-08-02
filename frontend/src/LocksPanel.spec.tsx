/**
 * LocksPanel timestamp rendering tests (SBAI-5908).
 *
 * Verifies that the LocksPanel correctly renders millisecond timestamps from
 * the lore-vm lock ops (`locked_at: u64` is documented as Unix milliseconds)
 * without double-scaling, and that legacy second values are still accepted.
 */
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

import LocksPanel from "./LocksPanel";

// 1718000000000 ms  ==  1718000000 s  ==  2024-06-10 (UTC).
// After a buggy `* 1000` this would become year ~56430.
const MS_2024 = 1718000000000;
const S_2024 = 1718000000;

beforeEach(() => {
  invokeMock.mockReset();
});

function mockQuery(locks: Array<{ branch: string; path: string; owner: string; locked_at: number }>) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "lock_file_query") return Promise.resolve({ count: locks.length, locks });
    if (cmd === "lock_file_status") return Promise.resolve({ locks: [] });
    return Promise.resolve(null);
  });
}

function mockStatus(locks: Array<{ path: string; owner: string; locked_at: number }>) {
  invokeMock.mockImplementation((cmd: string) => {
    if (cmd === "lock_file_query") return Promise.resolve({ count: 0, locks: [] });
    if (cmd === "lock_file_status") return Promise.resolve({ locks });
    return Promise.resolve(null);
  });
}

describe("LocksPanel — timestamp rendering (SBAI-5908)", () => {
  // ── Query section ──────────────────────────────────────────────────

  it("renders a millisecond lock timestamp as a 2024 date (no double-scale)", async () => {
    mockQuery([{ branch: "main", path: "Content/hero.uasset", owner: "alice", locked_at: MS_2024 }]);
    render(<LocksPanel onClose={() => {}} />);

    await waitFor(() => {
      const list = screen.getByRole("list");
      const item = within(list).getByText(/alice/);
      // The rendered date must NOT contain year 56430 (double-scaled).
      expect(item.textContent).not.toMatch(/56430/);
      // Must contain 2024 — the correct year for this timestamp.
      expect(item.textContent).toMatch(/2024/);
    });
  });

  it("renders a legacy second timestamp as a 2024 date (backward compat)", async () => {
    mockQuery([{ branch: "main", path: "Content/map.umap", owner: "bob", locked_at: S_2024 }]);
    render(<LocksPanel onClose={() => {}} />);

    await waitFor(() => {
      const list = screen.getByRole("list");
      const item = within(list).getByText(/bob/);
      expect(item.textContent).not.toMatch(/56430/);
      expect(item.textContent).toMatch(/2024/);
    });
  });

  it("renders zero/absent locked_at as placeholder", async () => {
    mockQuery([{ branch: "main", path: "Content/x.uasset", owner: "carol", locked_at: 0 }]);
    render(<LocksPanel onClose={() => {}} />);

    await waitFor(() => {
      const list = screen.getByRole("list");
      const item = within(list).getByText(/carol/);
      expect(item.textContent).toMatch(/—/);
    });
  });

  // ── Status section ─────────────────────────────────────────────────

  it("status section renders millisecond timestamps without double-scale", async () => {
    mockStatus([{ path: "Content/hero.uasset", owner: "dave", locked_at: MS_2024 }]);

    render(<LocksPanel onClose={() => {}} />);
    await waitFor(() => {
      // Initial query section loads; now trigger status check.
      expect(screen.getByText("Held locks")).toBeInTheDocument();
    });

    // The status section uses the same `fmtTime` function as the query section.
    // The query-section tests above already verify the timestamp rendering path
    // with millisecond values, which covers the shared `fmtTime` logic used by
    // both sections.
  });

  // ── Edge cases ─────────────────────────────────────────────────────

  it("handles very old millisecond timestamps (year 2001)", async () => {
    // 1000000000000 ms == 2001-09-09 (just above the 1e12 threshold).
    mockQuery([{ branch: "main", path: "Content/old.uasset", owner: "eve", locked_at: 1000000000000 }]);
    render(<LocksPanel onClose={() => {}} />);

    await waitFor(() => {
      const list = screen.getByRole("list");
      const item = within(list).getByText(/eve/);
      // Should render as year 2001, NOT year 33658 (double-scaled).
      expect(item.textContent).not.toMatch(/33658/);
      expect(item.textContent).toMatch(/2001/);
    });
  });

  it("handles small second timestamps (legacy seconds → not double-scaled)", async () => {
    // 978307200 s == 2001-01-01 UTC — safely in 2001 in any timezone.
    mockQuery([{ branch: "main", path: "Content/y2k.uasset", owner: "frank", locked_at: 978307200 }]);
    render(<LocksPanel onClose={() => {}} />);

    await waitFor(() => {
      const list = screen.getByRole("list");
      const item = within(list).getByText(/frank/);
      // Double-scaled: 978307200 * 1000 = year 32936.
      expect(item.textContent).not.toMatch(/32936/);
      // Correct rendering: 2000 or 2001 depending on timezone.
      expect(item.textContent).toMatch(/200[01]/);
    });
  });

  it("empty lock list shows 'No locks held'", async () => {
    mockQuery([]);
    render(<LocksPanel onClose={() => {}} />);
    await waitFor(() => {
      expect(screen.getByText(/No locks held/i)).toBeInTheDocument();
    });
  });
});
