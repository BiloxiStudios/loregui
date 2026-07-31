import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const LORE_URL = "https://github.com/EpicGames/lore.git";
const SHA_PATTERN = "[0-9a-f]{40}";

function packageBlock(lockfile, packageName) {
  return lockfile
    .split(/\n\[\[package\]\]/)
    .find((block) =>
      new RegExp(`(?:^|\\n)name = "${packageName}"(?:\\n|$)`).test(block),
    );
}

function lockfileRevision(lockfile, packageName) {
  const block = packageBlock(lockfile, packageName);
  if (!block) {
    throw new Error(`Cargo.lock has no ${packageName} package`);
  }
  const match = block.match(
    new RegExp(
      `source = "git\\+${LORE_URL.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")}\\?rev=(${SHA_PATTERN})#(${SHA_PATTERN})"`,
    ),
  );
  if (!match) {
    throw new Error(
      `Cargo.lock ${packageName} is not pinned to an exact lore revision`,
    );
  }
  if (match[1] !== match[2]) {
    throw new Error(`Cargo.lock ${packageName} requested and resolved revisions differ`);
  }
  return match[2];
}

function manifestRevision(manifest, dependencyName) {
  const match = manifest.match(
    new RegExp(
      `^${dependencyName}\\s*=\\s*\\{[^\\n]*git\\s*=\\s*"${LORE_URL.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")}"[^\\n]*rev\\s*=\\s*"(${SHA_PATTERN})"[^\\n]*\\}`,
      "m",
    ),
  );
  if (!match) {
    throw new Error(
      `Cargo.toml ${dependencyName} is not pinned to an exact lore revision`,
    );
  }
  return match[1];
}

/**
 * Read every product declaration that controls the shipped lore revision.
 * A single canonical revision is returned only when all declarations agree.
 */
export function readPinnedRevisions(repoRoot) {
  const manifest = readFileSync(join(repoRoot, "Cargo.toml"), "utf8");
  const lockfile = readFileSync(join(repoRoot, "Cargo.lock"), "utf8");
  const declarations = {
    manifestLore: manifestRevision(manifest, "lore"),
    manifestQuinnProto: manifestRevision(manifest, "quinn-proto"),
    lockLore: lockfileRevision(lockfile, "lore"),
    lockQuinnProto: lockfileRevision(lockfile, "quinn-proto"),
  };
  const revisions = new Set(Object.values(declarations));
  if (revisions.size !== 1) {
    throw new Error(
      `Lore revision mismatch: ${Object.entries(declarations)
        .map(([name, revision]) => `${name}=${revision}`)
        .join(", ")}`,
    );
  }
  return { revision: declarations.manifestLore, declarations };
}

function git(repoDir, args) {
  return execFileSync("git", ["-C", repoDir, ...args], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  }).trim();
}

function resolves(repoDir, sha) {
  if (!sha) return null;
  try {
    return git(repoDir, ["rev-parse", "--verify", `${sha}^{commit}`]);
  } catch {
    return null;
  }
}

function isAncestor(repoDir, older, newer) {
  try {
    execFileSync(
      "git",
      ["-C", repoDir, "merge-base", "--is-ancestor", older, newer],
      { stdio: "ignore" },
    );
    return true;
  } catch {
    return false;
  }
}

function count(repoDir, range) {
  return Number.parseInt(git(repoDir, ["rev-list", "--count", range]), 10);
}

/** Calculate a fail-closed relationship and distance between two commits. */
export function commitDistance(repoDir, fromSha, toSha) {
  const from = resolves(repoDir, fromSha);
  const to = resolves(repoDir, toSha);
  if (!from || !to) {
    return {
      status: "ERROR",
      detail: !from
        ? `commit ${fromSha || "<missing>"} is unavailable`
        : `commit ${toSha || "<missing>"} is unavailable`,
    };
  }
  if (from === to) {
    return {
      status: "LOCKSTEP",
      ahead: 0,
      behind: 0,
      fromSha: from,
      toSha: to,
    };
  }
  if (isAncestor(repoDir, from, to)) {
    return {
      status: "BEHIND",
      ahead: 0,
      behind: count(repoDir, `${from}..${to}`),
      ancestry: "from_is_ancestor",
      fromSha: from,
      toSha: to,
      compareUrl: `https://github.com/EpicGames/lore/compare/${from}...${to}`,
    };
  }
  if (isAncestor(repoDir, to, from)) {
    return {
      status: "AHEAD",
      ahead: count(repoDir, `${to}..${from}`),
      behind: 0,
      ancestry: "to_is_ancestor",
      fromSha: from,
      toSha: to,
      compareUrl: `https://github.com/EpicGames/lore/compare/${to}...${from}`,
    };
  }
  try {
    git(repoDir, ["merge-base", from, to]);
  } catch {
    return {
      status: "ERROR",
      detail: "commits have no common ancestor",
      fromSha: from,
      toSha: to,
    };
  }
  return {
    status: "DIVERGED",
    ahead: count(repoDir, `${to}..${from}`),
    behind: count(repoDir, `${from}..${to}`),
    ancestry: "diverged",
    fromSha: from,
    toSha: to,
    compareUrl: `https://github.com/EpicGames/lore/compare/${from}...${to}`,
  };
}

/** Keep shipped-product drift distinct from movement since the last watcher run. */
export function buildCommitDistanceReport(
  repoDir,
  productPin,
  upstreamHead,
  checkpoint,
) {
  return {
    productDrift: commitDistance(repoDir, productPin, upstreamHead),
    incrementalUpstreamMovement: checkpoint
      ? commitDistance(repoDir, checkpoint, upstreamHead)
      : {
          status: "NO_CHECKPOINT",
          detail: "first watcher run; checkpoint will be created",
        },
  };
}
