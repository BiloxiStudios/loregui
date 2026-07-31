#!/usr/bin/env node
/**
 * SBAI-5910 — the single source of truth for "is this manifest pinned to the
 * accepted product source?", used BOTH by `.github/workflows/upstream-parity.yml`
 * and by executable fixtures (`scripts/lore-pin-policy.test.mjs`).
 *
 * Binding ruling (sb-lore + sb-fable): PRODUCT SOURCE is the BiloxiStudios
 * maintenance fork at the reviewed rev; EpicGames upstream is a DRIFT TARGET
 * ONLY. Observation and reporting continue; mutation fails closed.
 *
 * The earlier workflow logic treated *any* non-Epic host as "a maintenance
 * fork" and continued — so `attacker.example` with any 40-hex rev would have
 * been accepted as the product pin. The policy is therefore an EXACT match
 * against the accepted pair; anything else (empty, unknown host, unknown rev,
 * unparseable, short rev) STOPS.
 */

export const ACCEPTED_HOST = "https://github.com/BiloxiStudios/lore.git";
export const ACCEPTED_REV = "ba92f94305df15796283755040c0bdd9b351841e";
/** Upstream, tracked for drift reporting only — never a valid product pin. */
export const DRIFT_TARGET_HOST = "https://github.com/EpicGames/lore.git";

/**
 * Classify a workspace manifest's `lore` + `quinn-proto` pins.
 * @returns {{ok: true, host: string, rev: string}
 *          | {ok: false, reason: string}}
 */
export function classifyPin(manifestText) {
  if (typeof manifestText !== "string" || manifestText.trim() === "") {
    return { ok: false, reason: "manifest is empty or unreadable" };
  }
  const pins = {};
  for (const dep of ["lore", "quinn-proto"]) {
    const escaped = dep.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const line = manifestText.match(
      new RegExp(`^${escaped}\\s*=\\s*\\{[^\\n]*$`, "m"),
    );
    if (!line) return { ok: false, reason: `${dep} pin is missing` };
    const host = line[0].match(/\bgit\s*=\s*"([^"]+)"/);
    if (!host) return { ok: false, reason: `${dep} pin has no git host` };
    const rev = line[0].match(/\brev\s*=\s*"([0-9a-f]{40})"/);
    if (!rev) {
      return {
        ok: false,
        reason: `${dep} pin has no full 40-hex rev (branches and tags are not pins)`,
      };
    }
    pins[dep] = { host: host[1], rev: rev[1] };
  }

  if (pins.lore.host !== pins["quinn-proto"].host) {
    return {
      ok: false,
      reason: `mixed host: lore=${pins.lore.host} quinn-proto=${pins["quinn-proto"].host}`,
    };
  }
  if (pins.lore.rev !== pins["quinn-proto"].rev) {
    return {
      ok: false,
      reason: `mixed rev: lore=${pins.lore.rev} quinn-proto=${pins["quinn-proto"].rev}`,
    };
  }

  const { host, rev } = pins.lore;
  if (host !== ACCEPTED_HOST) {
    // Deliberately no "it's some other fork, carry on" branch: only the exact
    // accepted host is a product pin.
    return {
      ok: false,
      reason:
        host === DRIFT_TARGET_HOST
          ? `pin is on the drift target ${host}; the product pin must stay on ${ACCEPTED_HOST} (it carries the SBAI-5909 credential hardening)`
          : `unknown host ${host}; only ${ACCEPTED_HOST} is an accepted product source`,
    };
  }
  if (rev !== ACCEPTED_REV) {
    return {
      ok: false,
      reason: `unknown rev ${rev}; only ${ACCEPTED_REV} is the accepted product pin`,
    };
  }
  return { ok: true, host, rev };
}

/** Mutation is never permitted while the product pin is the fork. */
export function mutationAllowed() {
  return false;
}

// CLI: `node scripts/lore-pin-policy.mjs <manifest-path>` — exit 0 accepted,
// exit 1 with the reason on stderr otherwise. The workflow calls this.
if (process.argv[1] && import.meta.url.endsWith(process.argv[1].split("/").pop())) {
  const { readFileSync } = await import("node:fs");
  const path = process.argv[2];
  let text = "";
  try {
    text = readFileSync(path, "utf8");
  } catch (error) {
    console.error(`lore pin policy: cannot read ${path}: ${error.message}`);
    process.exit(1);
  }
  const verdict = classifyPin(text);
  if (!verdict.ok) {
    console.error(`lore pin policy: STOP — ${verdict.reason}`);
    process.exit(1);
  }
  console.log(`lore pin policy: accepted product pin ${verdict.host} @ ${verdict.rev}`);
}
