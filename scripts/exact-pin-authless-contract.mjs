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

export const EXPECTED_LORE_REV =
  "826ad5d20ff4f5814101c946df127cef8253ada3";

function manifestPin(manifest, dependency) {
  const escaped = dependency.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const line = manifest.match(new RegExp(`^${escaped}\\s*=\\s*\\{[^\\n]+$`, "m"));
  if (!line) throw new Error(`${dependency} manifest pin is missing`);
  const rev = line[0].match(/\brev\s*=\s*"([0-9a-f]{40})"/);
  if (!rev) throw new Error(`${dependency} manifest pin is missing a full 40-character rev`);
  return rev[1];
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

export function verifyManifestAndLock(repoRoot, expectedRev = EXPECTED_LORE_REV) {
  const manifest = readFileSync(join(repoRoot, "Cargo.toml"), "utf8");
  const lock = readFileSync(join(repoRoot, "Cargo.lock"), "utf8");

  for (const dependency of ["lore", "quinn-proto"]) {
    const pin = manifestPin(manifest, dependency);
    if (pin !== expectedRev) {
      throw new Error(
        `${dependency} manifest pin ${pin} does not equal required ${expectedRev}`,
      );
    }
    const source = lockSource(lock, dependency);
    const sha = resolvedSha(source);
    if (
      !source.includes("git+https://github.com/EpicGames/lore.git") ||
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
    `exact Epic Lore checkout ${expectedRev} is missing; run cargo fetch first`,
  );
}

export function verifyExactPin(repoRoot, expectedRev = EXPECTED_LORE_REV) {
  verifyManifestAndLock(repoRoot, expectedRev);
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
