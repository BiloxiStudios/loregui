/**
 * Unit tests for the unified-diff parser that backs the content workspace Diff
 * tab. Pure functions over a patch string: line-number tracking across hunks,
 * +/-/context classification, meta/hunk handling, side-by-side folding
 * (del+add → change), and the add/remove stat.
 */
import { describe, it, expect } from "vitest";
import { parseUnified, toSideBySide, diffStat } from "./diff-parse";

const PATCH = [
  "--- a/file.txt",
  "+++ b/file.txt",
  "@@ -1,3 +1,3 @@",
  " context line",
  "-old line",
  "+new line",
  " trailing",
].join("\n");

describe("parseUnified", () => {
  it("returns no rows for an empty patch", () => {
    expect(parseUnified("")).toEqual([]);
  });

  it("classifies meta, hunk, context, add and del rows", () => {
    const rows = parseUnified(PATCH);
    const kinds = rows.map((r) => r.kind);
    expect(kinds).toEqual([
      "meta",
      "meta",
      "hunk",
      "context",
      "del",
      "add",
      "context",
    ]);
  });

  it("tracks old/new line numbers from the hunk header", () => {
    const rows = parseUnified(PATCH);
    const ctx = rows.find((r) => r.kind === "context")!;
    expect(ctx.oldNo).toBe(1);
    expect(ctx.newNo).toBe(1);
    const del = rows.find((r) => r.kind === "del")!;
    expect(del.oldNo).toBe(2);
    expect(del.newNo).toBeNull();
    const add = rows.find((r) => r.kind === "add")!;
    expect(add.newNo).toBe(2);
    expect(add.oldNo).toBeNull();
  });

  it("strips the leading +/-/space from row text", () => {
    const rows = parseUnified(PATCH);
    expect(rows.find((r) => r.kind === "add")!.text).toBe("new line");
    expect(rows.find((r) => r.kind === "del")!.text).toBe("old line");
    expect(rows.find((r) => r.kind === "context")!.text).toBe("context line");
  });

  it("treats a '\\ No newline' line as meta", () => {
    const rows = parseUnified("@@ -1 +1 @@\n-a\n+b\n\\ No newline at end of file");
    expect(rows[rows.length - 1].kind).toBe("meta");
  });
});

describe("toSideBySide", () => {
  it("folds a del+add pair into a single change row", () => {
    const rows = parseUnified(PATCH);
    const side = toSideBySide(rows);
    const change = side.find((r) => r.kind === "change")!;
    expect(change.oldText).toBe("old line");
    expect(change.newText).toBe("new line");
  });

  it("drops hunk/meta rows and keeps context aligned on both sides", () => {
    const side = toSideBySide(parseUnified(PATCH));
    expect(side.some((r) => r.kind === "context")).toBe(true);
    const ctx = side.find((r) => r.kind === "context")!;
    expect(ctx.oldText).toBe(ctx.newText);
  });

  it("emits a lone del / lone add when the runs are unbalanced", () => {
    const rows = parseUnified("@@ -1,2 +1,1 @@\n-a\n-b\n+c");
    const side = toSideBySide(rows);
    // 2 dels + 1 add → one change (a/c) + one lone del (b).
    expect(side.filter((r) => r.kind === "change")).toHaveLength(1);
    expect(side.filter((r) => r.kind === "del")).toHaveLength(1);
  });
});

describe("diffStat", () => {
  it("counts added and removed lines", () => {
    expect(diffStat(parseUnified(PATCH))).toEqual({ added: 1, removed: 1 });
  });

  it("is zero for a context-only patch", () => {
    expect(diffStat(parseUnified("@@ -1 +1 @@\n unchanged"))).toEqual({
      added: 0,
      removed: 0,
    });
  });
});
