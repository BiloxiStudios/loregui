import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import ModeSelect from "./ModeSelect";

// SBAI-5566 / SBAI-5573: "Choose Your Setup Mode" is the very first screen a
// fresh install lands on. Before this fix it had no exit at all — no Back
// (nothing to go back to) and no way to reach the main app without picking a
// mode and completing (or abandoning mid-way) the wizard. Closing from the
// tray and reopening re-rendered this exact screen, so the trap persisted
// across restarts.
describe("ModeSelect escape route (SBAI-5566, SBAI-5573)", () => {
  it("does not render a skip action when the caller doesn't provide one", () => {
    render(<ModeSelect onModeSelect={vi.fn()} />);
    expect(
      screen.queryByRole("button", { name: /skip/i }),
    ).not.toBeInTheDocument();
  });

  it("renders a visible skip action and invokes onSkip without requiring a mode pick", () => {
    const onSkip = vi.fn();
    render(<ModeSelect onModeSelect={vi.fn()} onSkip={onSkip} />);

    const skipButton = screen.getByRole("button", {
      name: /skip for now/i,
    });
    expect(skipButton).toBeVisible();

    fireEvent.click(skipButton);
    expect(onSkip).toHaveBeenCalledTimes(1);
  });

  it("keeps the skip action available after a mode card is selected", () => {
    const onSkip = vi.fn();
    const onModeSelect = vi.fn();
    render(<ModeSelect onModeSelect={onModeSelect} onSkip={onSkip} />);

    fireEvent.click(
      screen.getByRole("button", { name: /connect to a lore server/i }),
    );
    expect(onModeSelect).toHaveBeenCalledWith("client");

    fireEvent.click(screen.getByRole("button", { name: /skip for now/i }));
    expect(onSkip).toHaveBeenCalledTimes(1);
  });
});
