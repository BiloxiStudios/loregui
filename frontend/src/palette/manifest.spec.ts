/**
 * Structural tests over the command-palette op registry.
 *
 * The registry is auto-discovered (one file per op via import.meta.glob), so
 * these tests are the only thing that enforces the *shape* invariants every
 * manifest entry must hold: unique ids, well-formed `<domain>.<op>` ids, a
 * non-empty `command`, valid field kinds, and enum fields actually carrying
 * options. A malformed entry would otherwise only surface as a broken palette
 * row at runtime.
 */
import { describe, it, expect } from "vitest";
import { OP_MANIFEST, OP_BY_ID } from "./manifest";
import type { FieldKind } from "./types";

const FIELD_KINDS: FieldKind[] = [
  "text",
  "number",
  "boolean",
  "enum",
  "string-list",
];

describe("OP_MANIFEST registry", () => {
  it("discovers a non-trivial number of ops", () => {
    // The glob found real entries (guards against a broken import.meta.glob).
    expect(OP_MANIFEST.length).toBeGreaterThan(20);
  });

  it("is sorted by id and has no duplicate ids", () => {
    const ids = OP_MANIFEST.map((m) => m.id);
    expect([...ids].sort((a, b) => a.localeCompare(b))).toEqual(ids);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("OP_BY_ID indexes every manifest by id", () => {
    expect(Object.keys(OP_BY_ID).length).toBe(OP_MANIFEST.length);
    for (const m of OP_MANIFEST) {
      expect(OP_BY_ID[m.id]).toBe(m);
    }
  });

  it("every entry has a well-formed id matching domain + op", () => {
    for (const m of OP_MANIFEST) {
      expect(m.id, m.id).toBe(`${m.domain}.${m.op}`);
      expect(m.domain.length, m.id).toBeGreaterThan(0);
      expect(m.op.length, m.id).toBeGreaterThan(0);
    }
  });

  it("every entry has a label and a non-empty command", () => {
    for (const m of OP_MANIFEST) {
      expect(m.label.length, m.id).toBeGreaterThan(0);
      expect(typeof m.command, m.id).toBe("string");
      expect(m.command.length, m.id).toBeGreaterThan(0);
    }
  });

  it("every field has a known kind, a name, and a label", () => {
    for (const m of OP_MANIFEST) {
      for (const f of m.args) {
        expect(FIELD_KINDS, `${m.id}/${f.name}`).toContain(f.kind);
        expect(f.name.length, m.id).toBeGreaterThan(0);
        expect(f.label.length, `${m.id}/${f.name}`).toBeGreaterThan(0);
      }
    }
  });

  it("enum fields carry a non-empty options list with value+label", () => {
    for (const m of OP_MANIFEST) {
      for (const f of m.args.filter((a) => a.kind === "enum")) {
        expect(Array.isArray(f.options), `${m.id}/${f.name}`).toBe(true);
        expect((f.options ?? []).length, `${m.id}/${f.name}`).toBeGreaterThan(0);
        for (const o of f.options ?? []) {
          expect(typeof o.value).toBe("string");
          expect(typeof o.label).toBe("string");
        }
      }
    }
  });

  it("field names within an op are unique", () => {
    for (const m of OP_MANIFEST) {
      const names = m.args.map((a) => a.name);
      expect(new Set(names).size, m.id).toBe(names.length);
    }
  });

  it("resultKind, when set, is one of the allowed hints", () => {
    for (const m of OP_MANIFEST) {
      if (m.resultKind !== undefined) {
        expect(["void", "text", "json"], m.id).toContain(m.resultKind);
      }
    }
  });

  it("defaults commands to repository-required and only opts out explicitly", () => {
    expect(OP_BY_ID["branch.create"].requiresRepository).toBeUndefined();
    expect(OP_BY_ID["repository.gc"].requiresRepository).toBeUndefined();
    expect(OP_BY_ID["repository.list"].requiresRepository).toBe(false);
    expect(OP_BY_ID["repository.clone"].requiresRepository).toBe(false);
  });

  it("a known representative entry (branch.create) is wired correctly", () => {
    const bc = OP_BY_ID["branch.create"];
    expect(bc).toBeTruthy();
    expect(bc.command).toBe("branch_create");
    const required = bc.args.filter((a) => a.required).map((a) => a.name);
    expect(required).toContain("branch");
  });
});
