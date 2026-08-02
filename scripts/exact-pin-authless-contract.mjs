#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { readTablePin } from "./lore-pin-policy.mjs";

// Exact-pin contract: must move in lockstep with Cargo.toml's `lore` and
// [patch.crates-io].quinn-proto pins on every bump — HOST as well as rev.
// SBAI-5594: 826ad5d20 → 9664606f5 (JWT aud apex-match widening; the
// integration gate caught this constant still pointing at the old pin —
// working as designed).
// SBAI-5910: 9664606f5 (EpicGames) → ba92f943 on the BiloxiStudios
// MAINTENANCE FORK. The fork carries the signed SBAI-5909 credential fix
// (exact-domain JWT label boundary + legacy unscoped cached tokens fail
// closed) and exists ONLY there, so the host is part of the contract: a
// rev-only check would pass a move back to an unfixed tree.
export const EXPECTED_LORE_HOST = "https://github.com/BiloxiStudios/lore.git";
export const EXPECTED_LORE_REV =
  "2052749e36e1127c520a191b18141e23980b89d7";

// SBAI-5910 (review f096255): these readers were table-blind — they matched
// the first textual `lore = {...}` anywhere, so a `[workspace.metadata.*]`
// decoy could carry accepted values while the real dependency tables pointed
// at an attacker. They now delegate to the SHARED table-aware policy.
const TABLE_FOR = {
  lore: "[workspace.dependencies]",
  "quinn-proto": "[patch.crates-io]",
};

function tablePin(manifest, dependency) {
  const header = TABLE_FOR[dependency];
  if (!header) throw new Error(`no accepted table registered for ${dependency}`);
  const pin = readTablePin(manifest, header, dependency);
  if (!pin.ok) throw new Error(pin.reason);
  return pin;
}

function unusedManifestPin(manifest, dependency) {
  const escaped = dependency.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const line = manifest.match(new RegExp(`^${escaped}\\s*=\\s*\\{[^\\n]+$`, "m"));
  if (!line) throw new Error(`${dependency} manifest pin is missing`);
  const rev = line[0].match(/\brev\s*=\s*"([0-9a-f]{40})"/);
  if (!rev) throw new Error(`${dependency} manifest pin is missing a full 40-character rev`);
  return rev[1];
}

function unusedManifestHost(manifest, dependency) {
  const escaped = dependency.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const line = manifest.match(new RegExp(`^${escaped}\\s*=\\s*\\{[^\\n]+$`, "m"));
  if (!line) throw new Error(`${dependency} manifest pin is missing`);
  const git = line[0].match(/\bgit\s*=\s*"([^"]+)"/);
  if (!git) throw new Error(`${dependency} manifest pin is missing a git host`);
  return git[1];
}

function lockSource(lock, dependency) {
  const block = lock
    .split(/\n\[\[package\]\]\n/)
    .find((candidate) => new RegExp(`^name = "${dependency}"$`, "m").test(candidate));
  if (!block) throw new Error(`${dependency} lock package is missing`);
  const source = block.match(/^source = "([^"]+)"$/m);
  if (!source) throw new Error(`${dependency} lock source is missing`);
  return source[1];
}

function resolvedSha(source) {
  return source.match(/#([0-9a-f]{40})$/)?.[1] ?? source;
}

export function verifyManifestAndLock(
  repoRoot,
  expectedRev = EXPECTED_LORE_REV,
  expectedHost = EXPECTED_LORE_HOST,
) {
  const manifest = readFileSync(join(repoRoot, "Cargo.toml"), "utf8");
  const lock = readFileSync(join(repoRoot, "Cargo.lock"), "utf8");

  for (const dependency of ["lore", "quinn-proto"]) {
    const pin = tablePin(manifest, dependency).rev;
    if (pin !== expectedRev) {
      throw new Error(
        `${dependency} manifest pin ${pin} does not equal required ${expectedRev}`,
      );
    }
    // SBAI-5910: the HOST is part of the contract — a rev-only check would
    // accept a move to a fork that never received the credential fix.
    const host = tablePin(manifest, dependency).host;
    if (host !== expectedHost) {
      throw new Error(
        `${dependency} manifest host ${host} does not equal required ${expectedHost}`,
      );
    }
    const source = lockSource(lock, dependency);
    const sha = resolvedSha(source);
    if (
      !source.includes(`git+${expectedHost}`) ||
      !source.includes(`?rev=${expectedRev}#`) ||
      sha !== expectedRev
    ) {
      throw new Error(
        `${dependency} lock source ${sha} does not equal required ${expectedRev}`,
      );
    }
  }
}

function occurrences(source, needle) {
  return source.split(needle).length - 1;
}

export function verifyUpstreamAuthlessSource(checkoutRoot) {
  const exchangePath = join(
    checkoutRoot,
    "lore-transport",
    "src",
    "auth",
    "exchange.rs",
  );
  const userInfoPath = join(
    checkoutRoot,
    "lore-revision",
    "src",
    "auth",
    "userinfo.rs",
  );
  const exchange = readFileSync(exchangePath, "utf8");
  const userInfo = readFileSync(userInfoPath, "utf8");
  const operation = 'operation: "No authentication configured on server".to_string()';
  if (occurrences(exchange, operation) !== 2) {
    throw new Error(
      "upstream checkout must contain two typed NotSupported authless exchange branches",
    );
  }
  const forwarder =
    '.forward::<UserInfoError>("Failed authorization token exchange")?;';
  if (occurrences(userInfo, forwarder) !== 3) {
    throw new Error(
      "upstream checkout must contain three forwarded user-info exchange errors",
    );
  }
  if (userInfo.includes("debug_map_err(UserInfoError::from(NotAuthenticated))")) {
    throw new Error("upstream checkout retains a legacy NotAuthenticated remap");
  }
}

export function locateExactCheckout(
  expectedRev = EXPECTED_LORE_REV,
  cargoHome = process.env.CARGO_HOME || join(homedir(), ".cargo"),
) {
  const checkouts = join(cargoHome, "git", "checkouts");
  if (!existsSync(checkouts)) {
    throw new Error(`Cargo git checkout directory is missing: ${checkouts}`);
  }
  for (const repository of readdirSync(checkouts)) {
    if (!repository.startsWith("lore-")) continue;
    const repositoryPath = join(checkouts, repository);
    for (const candidate of readdirSync(repositoryPath)) {
      const checkout = join(repositoryPath, candidate);
      if (!statSync(checkout).isDirectory()) continue;
      try {
        const head = execFileSync("git", ["-C", checkout, "rev-parse", "HEAD"], {
          encoding: "utf8",
          stdio: ["ignore", "pipe", "ignore"],
        }).trim();
        if (head === expectedRev) return checkout;
      } catch {
        // Ignore unrelated or incomplete Cargo checkout directories.
      }
    }
  }
  throw new Error(
    `exact pinned Lore checkout ${expectedRev} is missing; run cargo fetch first`,
  );
}

export function verifyExactPin(
  repoRoot,
  expectedRev = EXPECTED_LORE_REV,
  expectedHost = EXPECTED_LORE_HOST,
) {
  verifyManifestAndLock(repoRoot, expectedRev, expectedHost);
  const checkout = locateExactCheckout(expectedRev);
  verifyUpstreamAuthlessSource(checkout);
  return checkout;
}

function argValue(name) {
  const index = process.argv.indexOf(name);
  return index === -1 ? null : process.argv[index + 1];
}

const invokedAsScript =
  process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href;
if (invokedAsScript) {
  const here = dirname(fileURLToPath(import.meta.url));
  const repoRoot = resolve(argValue("--repo-root") ?? join(here, ".."));
  const expectedRev = argValue("--expected") ?? EXPECTED_LORE_REV;
  try {
    const checkout = verifyExactPin(repoRoot, expectedRev);
    console.log(`exact Epic Lore authless contract verified at ${expectedRev}`);
    console.log(`checkout: ${checkout}`);
  } catch (error) {
    console.error(`FATAL: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  }
}
