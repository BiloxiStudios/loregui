/**
 * Unit tests for the palette arg-marshalling logic (`buildArgs`).
 *
 * Run with Node's built-in test runner (type-stripping handles the .ts import):
 *   node --test --experimental-strip-types frontend/src/palette/buildArgs.test.ts
 *
 * Regression cover for the "missing required key" bug: Tauri commands declare
 * plain (non-Option) Rust params, so an OPTIONAL field left blank must still be
 * present in the invoke payload (empty string / default number) — never dropped.
 */
import { test } from "node:test";
import assert from "node:assert/strict";

import { buildArgs, initialValues } from "./buildArgs.ts";
import type { OpManifest } from "./types.ts";

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

test("blank optional text field is emitted as an empty string, not dropped", () => {
  const m = manifest([
    { name: "branch", kind: "text", label: "Branch", required: false },
  ]);
  const out = buildArgs(m, initialValues(m));
  assert.ok(
    Object.prototype.hasOwnProperty.call(out, "branch"),
    "optional text key must be present",
  );
  assert.equal(out.branch, "");
});

test("filled optional text field carries its value", () => {
  const m = manifest([
    { name: "branch", kind: "text", label: "Branch", required: false },
  ]);
  const out = buildArgs(m, { branch: "  main  " });
  // text is stringified verbatim (no trim) so the user's value reaches Rust.
  assert.equal(out.branch, "  main  ");
});

test("blank optional enum field is emitted as an empty string", () => {
  const m = manifest([
    {
      name: "category",
      kind: "enum",
      label: "Category",
      required: false,
      options: [{ value: "feature", label: "Feature" }],
    },
  ]);
  const out = buildArgs(m, initialValues(m));
  assert.ok(Object.prototype.hasOwnProperty.call(out, "category"));
  assert.equal(out.category, "");
});

test("blank optional number field is emitted as 0 (or its default)", () => {
  const m = manifest([
    { name: "limit", kind: "number", label: "Limit", required: false },
    {
      name: "count",
      kind: "number",
      label: "Count",
      required: false,
      default: 10,
    },
  ]);
  const out = buildArgs(m, initialValues(m));
  assert.ok(Object.prototype.hasOwnProperty.call(out, "limit"));
  assert.equal(out.limit, 0);
  // a blank number with a default falls back to that default
  assert.equal(out.count, 10);
});

test("filled number field is parsed to a Number", () => {
  const m = manifest([
    { name: "limit", kind: "number", label: "Limit", required: false },
  ]);
  const out = buildArgs(m, { limit: "25" });
  assert.equal(out.limit, 25);
  assert.equal(typeof out.limit, "number");
});

test("boolean always emits a concrete boolean", () => {
  const m = manifest([
    { name: "force", kind: "boolean", label: "Force" },
    { name: "dry", kind: "boolean", label: "Dry", default: true },
  ]);
  const out = buildArgs(m, initialValues(m));
  assert.equal(out.force, false);
  assert.equal(out.dry, true);
});

test("string-list splits, trims, and drops blank lines", () => {
  const m = manifest([
    { name: "paths", kind: "string-list", label: "Paths" },
  ]);
  const out = buildArgs(m, { paths: " a \n\n b \n" });
  assert.deepEqual(out.paths, ["a", "b"]);
});

test("every arg key is present even for an all-optional manifest", () => {
  // Mirrors the shape of affected ops (e.g. file_diff / revision_diff): a mix of
  // optional text + number fields, all left blank.
  const m = manifest([
    { name: "from", kind: "text", label: "From", required: false },
    { name: "to", kind: "text", label: "To", required: false },
    { name: "limit", kind: "number", label: "Limit", required: false },
  ]);
  const out = buildArgs(m, initialValues(m));
  assert.deepEqual(Object.keys(out).sort(), ["from", "limit", "to"]);
  assert.equal(out.from, "");
  assert.equal(out.to, "");
  assert.equal(out.limit, 0);
});
