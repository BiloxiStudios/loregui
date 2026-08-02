#!/usr/bin/env node
/**
 * SBAI-5910 — fail-closed CANONICAL pin for `.github/workflows/upstream-parity.yml`.
 *
 * Two hand-written parsers were defeated by valid YAML (quoted keys, quoted or
 * commented values, folded `>-` bodies). Each fix invited the next quoting
 * trick, so the parse surface is removed entirely: the workflow must be
 * BYTE-IDENTICAL to a committed canonical copy whose own digest is pinned
 * here. Any alternate spelling of anything — quoting, anchors, flow mappings,
 * multi-document, added steps, escalated permissions — changes the bytes and
 * therefore fails closed, with no parser to outwit.
 *
 * Changing the workflow is a deliberate, reviewable act: update the workflow,
 * refresh the canonical copy, and update CANONICAL_SHA256 in the same PR. The
 * three-way check below means swapping the canonical copy alone does not help.
 */
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const WORKFLOW_PATH = ".github/workflows/upstream-parity.yml";
export const CANONICAL_PATH = "scripts/upstream-parity.canonical.yml";
/** Digest of the reviewed workflow bytes (SBAI-5910). */
export const CANONICAL_SHA256 =
  "d8354b15eb23bd885f3c66d9ce4bd5d240882e3c045d7fa309d6059d8d652f51";

export function sha256(text) {
  return createHash("sha256").update(text, "utf8").digest("hex");
}

/**
 * @returns {{ok: true, digest: string} | {ok: false, reason: string}}
 */
export function checkCanonical(workflowText, canonicalText) {
  if (typeof workflowText !== "string" || workflowText === "") {
    return { ok: false, reason: "workflow is empty or unreadable" };
  }
  if (typeof canonicalText !== "string" || canonicalText === "") {
    return { ok: false, reason: "canonical copy is empty or unreadable" };
  }
  const canonicalDigest = sha256(canonicalText);
  if (canonicalDigest !== CANONICAL_SHA256) {
    return {
      ok: false,
      reason:
        `canonical copy digest ${canonicalDigest} != pinned ${CANONICAL_SHA256} — ` +
        "the canonical copy was replaced without updating the pinned digest",
    };
  }
  const workflowDigest = sha256(workflowText);
  if (workflowDigest !== CANONICAL_SHA256) {
    return {
      ok: false,
      reason:
        `workflow digest ${workflowDigest} != canonical ${CANONICAL_SHA256} — ` +
        "the workflow differs from its reviewed canonical form (any alternate " +
        "YAML spelling, added step, or permission change lands here)",
    };
  }
  if (workflowText !== canonicalText) {
    return { ok: false, reason: "workflow and canonical copy differ despite equal digests" };
  }
  return { ok: true, digest: workflowDigest };
}

export function checkRepository() {
  return checkCanonical(
    readFileSync(join(repoRoot, WORKFLOW_PATH), "utf8"),
    readFileSync(join(repoRoot, CANONICAL_PATH), "utf8"),
  );
}

if (process.argv[1] && import.meta.url.endsWith(process.argv[1].split("/").pop())) {
  const verdict = checkRepository();
  if (!verdict.ok) {
    console.error(`workflow canonical policy: STOP — ${verdict.reason}`);
    process.exit(1);
  }
  console.log(`workflow canonical policy: upstream-parity.yml matches the reviewed canonical form (${verdict.digest.slice(0, 12)}…)`);
}
