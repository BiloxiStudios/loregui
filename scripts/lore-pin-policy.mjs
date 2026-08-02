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
export const ACCEPTED_REV = "2052749e36e1127c520a191b18141e23980b89d7";
/** Upstream, tracked for drift reporting only — never a valid product pin. */
export const DRIFT_TARGET_HOST = "https://github.com/EpicGames/lore.git";

/**
 * Extract the body of an exact TOML table header, e.g. `[workspace.dependencies]`.
 * Returns every occurrence — duplicates are themselves a violation.
 */
function tableBodies(tomlText, header) {
  const bodies = [];
  const lines = tomlText.split("\n");
  let capturing = false;
  let body = [];
  for (const raw of lines) {
    const line = raw.trim();
    if (line.startsWith("[") && !line.startsWith("[[")) {
      if (capturing) bodies.push(body.join("\n"));
      capturing = line === header;
      body = [];
      continue;
    }
    if (line.startsWith("[[")) {
      if (capturing) bodies.push(body.join("\n"));
      capturing = false;
      body = [];
      continue;
    }
    if (capturing) body.push(raw);
  }
  if (capturing) bodies.push(body.join("\n"));
  return bodies;
}

/**
 * Read `key = { git = "...", rev = "..." }` from EXACTLY ONE occurrence of
 * `header`, requiring exactly one matching key inside it.
 *
 * Review finding on f096255: the previous readers matched the first textual
 * `lore = {...}` ANYWHERE in the file, so valid TOML could park accepted
 * values in a `[workspace.metadata.*]` decoy while the real
 * `[workspace.dependencies]` / `[patch.crates-io]` entries pointed at an
 * attacker. Table identity is therefore part of the contract.
 */
export function readTablePin(tomlText, header, key) {
  const bodies = tableBodies(tomlText, header);
  if (bodies.length === 0) return { ok: false, reason: `missing table ${header}` };
  if (bodies.length > 1) {
    return { ok: false, reason: `duplicate table ${header} (${bodies.length} occurrences)` };
  }
  const escaped = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const keyLines = bodies[0]
    .split("\n")
    .filter((l) => new RegExp(`^\\s*(${escaped}|"${escaped}")\\s*=`).test(l));
  if (keyLines.length === 0) {
    return { ok: false, reason: `${header} has no ${key} entry` };
  }
  if (keyLines.length > 1) {
    return {
      ok: false,
      reason: `${header} declares ${key} ${keyLines.length} times (duplicate key)`,
    };
  }
  const line = keyLines[0];
  const host = line.match(/\bgit\s*=\s*"([^"]+)"/);
  if (!host) return { ok: false, reason: `${header}.${key} has no git host` };
  const rev = line.match(/\brev\s*=\s*"([0-9a-f]{40})"/);
  if (!rev) {
    return {
      ok: false,
      reason: `${header}.${key} has no full 40-hex rev (branches and tags are not pins)`,
    };
  }
  return { ok: true, host: host[1], rev: rev[1] };
}

/**
 * Classify a workspace manifest's `lore` + `quinn-proto` pins, read from the
 * EXACT tables that cargo actually consumes.
 * @returns {{ok: true, host: string, rev: string}
 *          | {ok: false, reason: string}}
 */
export function classifyPin(manifestText) {
  if (typeof manifestText !== "string" || manifestText.trim() === "") {
    return { ok: false, reason: "manifest is empty or unreadable" };
  }
  const surfaces = [
    ["[workspace.dependencies]", "lore"],
    ["[patch.crates-io]", "quinn-proto"],
  ];
  const pins = [];
  for (const [header, key] of surfaces) {
    const pin = readTablePin(manifestText, header, key);
    if (!pin.ok) return pin;
    pins.push({ label: `${header}.${key}`, ...pin });
  }

  if (pins[0].host !== pins[1].host) {
    return {
      ok: false,
      reason: `mixed host: ${pins[0].label}=${pins[0].host} ${pins[1].label}=${pins[1].host}`,
    };
  }
  if (pins[0].rev !== pins[1].rev) {
    return {
      ok: false,
      reason: `mixed rev: ${pins[0].label}=${pins[0].rev} ${pins[1].label}=${pins[1].rev}`,
    };
  }

  const { host, rev } = pins[0];
  if (host !== ACCEPTED_HOST) {
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

/**
 * The `lore-credential` dev-dependency in src-tauri/Cargo.toml must carry the
 * identical pin, or the behavioral DENY guard would exercise a different tree
 * than the shipped binary.
 */
export function classifyDevPin(tauriManifestText) {
  if (typeof tauriManifestText !== "string" || tauriManifestText.trim() === "") {
    return { ok: false, reason: "src-tauri manifest is empty or unreadable" };
  }
  const pin = readTablePin(tauriManifestText, "[dev-dependencies]", "lore-credential");
  if (!pin.ok) return pin;
  if (pin.host !== ACCEPTED_HOST) {
    return { ok: false, reason: `dev pin host ${pin.host} is not ${ACCEPTED_HOST}` };
  }
  if (pin.rev !== ACCEPTED_REV) {
    return { ok: false, reason: `dev pin rev ${pin.rev} is not ${ACCEPTED_REV}` };
  }
  return { ok: true, host: pin.host, rev: pin.rev };
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
