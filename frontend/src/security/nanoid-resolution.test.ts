/**
 * SBAI-7198: lockfile must resolve nanoid >= 3.3.18 (GHSA-2v37-7h3g-55p8).
 *
 * nanoid is not a product import — it arrives via postcss (Vite). The advisory
 * is a hang in custom generators when `size` is 0; this file pins the
 * resolved version and exercises that path so a lockfile regression fails CI.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const FRONTEND_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../..");
const MIN_PATCHED = [3, 3, 18] as const;

function parseSemver(raw: string): [number, number, number] {
  const m = raw.trim().match(/^(\d+)\.(\d+)\.(\d+)/);
  if (!m) throw new Error(`not a x.y.z version: ${raw}`);
  return [Number(m[1]), Number(m[2]), Number(m[3])];
}

function gte(a: [number, number, number], b: readonly [number, number, number]): boolean {
  for (let i = 0; i < 3; i++) {
    if (a[i] > b[i]) return true;
    if (a[i] < b[i]) return false;
  }
  return true;
}

test("package-lock resolves nanoid to a GHSA-2v37-7h3g-55p8 patched version", async () => {
  const lock = JSON.parse(
    await readFile(join(FRONTEND_ROOT, "package-lock.json"), "utf8"),
  ) as {
    packages?: Record<string, { version?: string }>;
  };
  const entry = lock.packages?.["node_modules/nanoid"];
  assert.ok(entry?.version, "lockfile must contain node_modules/nanoid");
  assert.ok(
    gte(parseSemver(entry.version), MIN_PATCHED),
    `resolved nanoid ${entry.version} is below 3.3.18 (GHSA-2v37-7h3g-55p8)`,
  );
});

test("custom nanoid generator with size 0 returns without hanging", async () => {
  const { customAlphabet } = await import("nanoid");
  const gen = customAlphabet("abcdefghijklmnopqrstuvwxyz", 0);
  const started = Date.now();
  const out = gen();
  assert.ok(Date.now() - started < 1000, "size-0 custom generator hung");
  assert.equal(out, "");
});
