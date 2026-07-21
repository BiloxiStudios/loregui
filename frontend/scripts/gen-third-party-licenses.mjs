#!/usr/bin/env node
// Generates THIRD-PARTY-LICENSES-FRONTEND.md — the npm dependency attribution
// bundle for the JavaScript that ships inside the LoreGUI desktop binary
// (Vite builds frontend/dist, which Tauri embeds).
//
// Production deps only: devDependencies (vite, typescript, playwright, type
// stubs) are build-time tooling and are NOT in the shipped bundle, so they are
// excluded. The first-party `loregui-frontend` package itself is skipped (it is
// MIT under the repo root LICENSE).
//
// Uses `license-checker-rseidelsohn` (the maintained fork of license-checker).
// Run from the repo root:
//   node frontend/scripts/gen-third-party-licenses.mjs
// CI re-runs this and diffs the result (.github/workflows/licenses.yml).
//
// Exits non-zero if a non-permissive (GPL/AGPL/LGPL/SSPL/copyleft) license is
// found in the production dependency tree — a distribution tripwire.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const frontendDir = resolve(__dirname, "..");
const repoRoot = resolve(frontendDir, "..");
const outPath = join(repoRoot, "THIRD-PARTY-LICENSES-FRONTEND.md");

// SPDX ids we accept for a redistributable desktop bundle. Anything outside
// this set fails the run (the copyleft tripwire).
const ALLOWED = new Set([
  "MIT",
  "Apache-2.0",
  "BSD-2-Clause",
  "BSD-3-Clause",
  "ISC",
  "0BSD",
  "BlueOak-1.0.0",
  "CC0-1.0",
  "CC-BY-4.0",
  "Unlicense",
  "Python-2.0",
  "Zlib",
  "MPL-2.0",
]);

// The self-package (private, first-party, MIT under root LICENSE).
const SELF = "loregui-frontend";

// Packages whose license metadata license-checker cannot normalize (it
// reports e.g. "MIT*"), but whose LICENSE file we have manually read and
// verified as permissive. Keyed by bare package name; the value replaces the
// unparseable id in the generated doc so it stays truthful. Keep this list
// short — anything here has been verified by reading the package's LICENSE.
const VERIFIED_LICENSES = new Map([
  // tinyusdz@0.9.1 declares "license": "Apache 2.0 and MIT" in package.json
  // (lowercase "and"); license-checker can't parse the expression and reports
  // "MIT*". The bundled LICENSE file carries the Apache-2.0 + MIT dual text.
  ["tinyusdz", "Apache-2.0 AND MIT"],
]);

function runChecker() {
  // --production: only deps that ship; --json: machine-readable.
  const out = execFileSync(
    "npx",
    [
      "--yes",
      "license-checker-rseidelsohn",
      "--production",
      "--json",
      "--start",
      ".",
    ],
    { cwd: frontendDir, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  return JSON.parse(out);
}

function splitSpdx(expr) {
  // "MIT OR Apache-2.0" / "(MIT AND BSD-3-Clause)" -> individual ids.
  return String(expr)
    .replace(/[()]/g, " ")
    .split(/\s+(?:OR|AND)\s+/i)
    .map((s) => s.trim())
    .filter(Boolean);
}

function isAllowed(expr) {
  const ids = splitSpdx(expr);
  // An "OR" expression is fine if ANY branch is permissive; we approximate by
  // requiring every listed id to be allowed (conservative). Custom strings like
  // "Custom: <url>" are flagged for manual review.
  return ids.length > 0 && ids.every((id) => ALLOWED.has(id));
}

function main() {
  const data = runChecker();
  const entries = Object.entries(data)
    .filter(([name]) => !name.startsWith(`${SELF}@`))
    .sort(([a], [b]) => a.localeCompare(b));

  const offenders = entries.filter(
    ([name, v]) =>
      !isAllowed(v.licenses) && !VERIFIED_LICENSES.has(name.replace(/@[^@]*$/, "")),
  );
  if (offenders.length > 0) {
    console.error("Non-permissive / unrecognized license(s) found:");
    for (const [name, v] of offenders) {
      console.error(`  - ${name}: ${v.licenses}`);
    }
    console.error(
      "Add the id to ALLOWED only if it is genuinely permissive, " +
        "otherwise the dependency is a distribution problem.",
    );
    process.exit(1);
  }

  // Normalize manually verified ids into the report (see VERIFIED_LICENSES).
  for (const [name, v] of entries) {
    const verified = VERIFIED_LICENSES.get(name.replace(/@[^@]*$/, ""));
    if (verified && !isAllowed(v.licenses)) v.licenses = verified;
  }

  // License summary counts.
  const counts = new Map();
  for (const [, v] of entries) {
    const k = String(v.licenses);
    counts.set(k, (counts.get(k) ?? 0) + 1);
  }
  const summary = [...counts.entries()].sort((a, b) => b[1] - a[1]);

  let md = `# Third-Party Frontend (npm) Licenses — LoreGUI

This file is **generated**. Do not edit by hand. Regenerate with:

\`\`\`
node frontend/scripts/gen-third-party-licenses.mjs
\`\`\`

LoreGUI's desktop binary embeds the Vite-built frontend (\`frontend/dist\`). The
**production** npm dependencies bundled into that frontend are listed below with
their license and notice text. Build-time devDependencies (Vite, TypeScript,
Playwright, type stubs) are excluded — they are not shipped. All are permissive;
there are no GPL/AGPL/LGPL copyleft dependencies.

## License summary

`;
  for (const [lic, n] of summary) {
    md += `- **${lic}** — ${n} package(s)\n`;
  }
  md += `\n---\n\n`;

  for (const [name, v] of entries) {
    md += `## ${name}\n\n`;
    md += `- License: **${v.licenses}**\n`;
    if (v.repository) md += `- Repository: ${v.repository}\n`;
    if (v.publisher) md += `- Publisher: ${v.publisher}\n`;
    md += `\n`;
    let text = "";
    if (v.licenseFile) {
      try {
        text = readFileSync(v.licenseFile, "utf8").trim();
      } catch {
        /* ignore */
      }
    }
    if (text) {
      md += `<details><summary>License text</summary>\n\n\`\`\`\n${text}\n\`\`\`\n\n</details>\n\n`;
    }
    md += `---\n\n`;
  }

  writeFileSync(outPath, md);

  // Also stage a copy inside the frontend bundle so the in-app
  // Settings → About → Third-party licenses view can fetch it offline. Vite
  // copies frontend/public/* into dist/, which Tauri embeds in the binary.
  const publicCopy = join(frontendDir, "public", "licenses", "frontend.md");
  mkdirSync(dirname(publicCopy), { recursive: true });
  writeFileSync(publicCopy, md);

  console.log(
    `Wrote ${outPath} (${entries.length} production packages, all permissive).`,
  );
}

main();
