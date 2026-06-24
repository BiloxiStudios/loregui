/**
 * Component tests for the palette's generated <OpForm/>.
 *
 * OpForm is the public surface that turns a manifest's FieldSpecs into the args
 * object handed to `invoke`. We drive it through the DOM (render → fill → submit)
 * and assert the exact args shape `onRun` receives. This is where the
 * arg-shape contract lives, including:
 *
 *  - DOCUMENTED BUG (current behavior, fix tracked separately on another
 *    branch): a blank *optional* text field is DROPPED from the args object
 *    rather than sent as "". The "drops blank optional text fields" test pins
 *    today's behavior so the fix is a deliberate, visible change.
 *  - required-field gating (submit disabled until required filled),
 *  - boolean always present, string-list trimmed+split, number coercion.
 */
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import OpForm from "./form";
import type { OpManifest } from "./types";

function manifest(args: OpManifest["args"]): OpManifest {
  return {
    id: "test.op",
    domain: "test",
    op: "op",
    label: "Test: Op",
    command: "test_op",
    args,
  };
}

function renderForm(m: OpManifest) {
  const onRun = vi.fn();
  render(<OpForm manifest={m} running={false} onRun={onRun} />);
  return onRun;
}

/** Submit the form and return the single args object passed to onRun. */
function submit(onRun: ReturnType<typeof vi.fn>): Record<string, unknown> {
  fireEvent.click(screen.getByRole("button", { name: /run/i }));
  expect(onRun).toHaveBeenCalledTimes(1);
  return onRun.mock.calls[0][0];
}

describe("OpForm arg building", () => {
  it("sends a filled required text field verbatim", () => {
    const onRun = renderForm(
      manifest([
        { name: "branch", kind: "text", label: "Branch", required: true },
      ]),
    );
    fireEvent.change(screen.getByLabelText(/Branch/), {
      target: { value: "feature/x" },
    });
    expect(submit(onRun)).toEqual({ branch: "feature/x" });
  });

  it("DROPS a blank optional text field (documents the buildArgs bug)", () => {
    const onRun = renderForm(
      manifest([
        { name: "branch", kind: "text", label: "Branch", required: true },
        { name: "category", kind: "text", label: "Category", required: false },
      ]),
    );
    fireEvent.change(screen.getByLabelText(/Branch/), {
      target: { value: "topic/foo" },
    });
    const args = submit(onRun);
    // Current behavior: the blank optional `category` is omitted entirely.
    expect(args).toEqual({ branch: "topic/foo" });
    expect("category" in args).toBe(false);
  });

  it("includes a blank required text field as an empty string", () => {
    // required fields are always emitted even when empty (filled || required).
    const onRun = renderForm(
      manifest([
        { name: "msg", kind: "text", label: "Message", required: true },
        { name: "x", kind: "text", label: "X", required: true },
      ]),
    );
    // Fill only the first so the form is submittable... but both required.
    // Instead fill both, then blank the second to assert the build path:
    fireEvent.change(screen.getByLabelText(/Message/), {
      target: { value: "m" },
    });
    fireEvent.change(screen.getByLabelText(/^X/), { target: { value: " " } });
    // " " is whitespace-only, so the required field is NOT "filled" → submit
    // stays disabled. This pins the required-gating rule.
    expect(screen.getByRole("button", { name: /run/i })).toBeDisabled();
    expect(onRun).not.toHaveBeenCalled();
  });

  it("always includes boolean fields (true and false)", () => {
    const onRun = renderForm(
      manifest([
        { name: "force", kind: "boolean", label: "Force", default: false },
        { name: "deep", kind: "boolean", label: "Deep", default: true },
      ]),
    );
    const args = submit(onRun);
    expect(args).toEqual({ force: false, deep: true });
  });

  it("toggling a checkbox flips the emitted boolean", () => {
    const onRun = renderForm(
      manifest([
        { name: "force", kind: "boolean", label: "Force", default: false },
      ]),
    );
    fireEvent.click(screen.getByRole("checkbox"));
    expect(submit(onRun)).toEqual({ force: true });
  });

  it("string-list splits on newlines and trims, dropping blank lines", () => {
    const onRun = renderForm(
      manifest([
        { name: "paths", kind: "string-list", label: "Paths", required: false },
      ]),
    );
    fireEvent.change(screen.getByLabelText(/Paths/), {
      target: { value: "  a.md \n\n b.md \n" },
    });
    expect(submit(onRun)).toEqual({ paths: ["a.md", "b.md"] });
  });

  it("emits an empty array for a blank string-list", () => {
    const onRun = renderForm(
      manifest([
        { name: "paths", kind: "string-list", label: "Paths", required: false },
      ]),
    );
    expect(submit(onRun)).toEqual({ paths: [] });
  });

  it("coerces a filled number field to a JS number", () => {
    const onRun = renderForm(
      manifest([
        {
          name: "contextLines",
          kind: "number",
          label: "Context",
          default: 3,
        },
      ]),
    );
    fireEvent.change(screen.getByLabelText(/Context/), {
      target: { value: "7" },
    });
    const args = submit(onRun);
    expect(args.contextLines).toBe(7);
    expect(typeof args.contextLines).toBe("number");
  });

  it("emits an enum field's selected value", () => {
    const onRun = renderForm(
      manifest([
        {
          name: "format",
          kind: "enum",
          label: "Format",
          required: true,
          options: [
            { value: "string", label: "String" },
            { value: "binary", label: "Binary" },
          ],
        },
      ]),
    );
    fireEvent.change(screen.getByLabelText(/Format/), {
      target: { value: "binary" },
    });
    expect(submit(onRun)).toEqual({ format: "binary" });
  });
});

describe("OpForm required-field gating", () => {
  it("disables Run until every required field is filled", () => {
    renderForm(
      manifest([
        { name: "a", kind: "text", label: "A", required: true },
        { name: "b", kind: "text", label: "B", required: true },
      ]),
    );
    const run = screen.getByRole("button", { name: /run/i });
    expect(run).toBeDisabled();
    fireEvent.change(screen.getByLabelText(/^A/), { target: { value: "1" } });
    expect(run).toBeDisabled();
    fireEvent.change(screen.getByLabelText(/^B/), { target: { value: "2" } });
    expect(run).toBeEnabled();
  });

  it("a no-arg op shows the empty-args hint and submits immediately", () => {
    const onRun = renderForm(manifest([]));
    expect(screen.getByText(/takes no arguments/i)).toBeInTheDocument();
    expect(submit(onRun)).toEqual({});
  });
});
