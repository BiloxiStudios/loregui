import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

const workflow = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
const guard = readFileSync(new URL("../.github/workflows/boundary-guard.yml", import.meta.url), "utf8");
const helper = fileURLToPath(new URL("./release-supply-chain.mjs", import.meta.url));

test("release workflow pins reviewed supply-chain actions and keeps OIDC off pull requests", () => {
  const triggers = workflow.slice(workflow.indexOf("on:"), workflow.indexOf("permissions:"));
  assert.match(triggers, /\n  push:/);
  assert.match(triggers, /\n  workflow_dispatch:/);
  assert.doesNotMatch(triggers, /pull_request/);
  assert.match(workflow, /id-token: write/);
  assert.match(
    workflow,
    /uses: anchore\/sbom-action@e22c389904149dbc22b58101806040fa8d37a610 # v0\.24\.0/,
  );
  assert.match(
    workflow,
    /uses: actions\/attest-build-provenance@ef244123eb79f2f7a7e75d99086184180e6d0018 # v1\.4\.4/,
  );
});

test("release workflow uses one staged raw-sidecar trust boundary", () => {
  const stage = workflow.indexOf("name: Stage raw sidecar release subjects");
  const sbom = workflow.indexOf("name: Generate staged-subject SBOM");
  const upload = workflow.indexOf("name: Checksum and upload staged supply-chain assets");
  const attest = workflow.indexOf("name: Attest exact released subjects");
  assert.ok(stage >= 0 && stage < sbom && sbom < upload && upload < attest);

  assert.match(workflow, /path: "\$\{\{ env\.STAGE_DIR \}\}"/);
  assert.match(workflow, /output-file: "\$\{\{ env\.STAGE_DIR \}\}\/sbom-\$\{\{ matrix\.triple \}\}\.spdx\.json"/);
  assert.match(workflow, /upload-artifact: false/);
  assert.match(workflow, /upload-release-assets: false/);
  assert.match(workflow, /node scripts\/release-supply-chain\.mjs checksums "\$STAGE_DIR" "\$\{\{ matrix\.triple \}\}"/);
  assert.match(workflow, /gh release upload "\$TAG" "\$STAGE_DIR"\/\*/);
  assert.match(workflow, /subject-path: "\$\{\{ env\.STAGE_DIR \}\}\/\*"/);
  assert.doesNotMatch(workflow, /\n\s+path: \./);
  assert.doesNotMatch(workflow, /\bsha256sum\b|\bshasum\b/);
});

test("release workflow rejects an empty stage before exporting STAGE_DIR", () => {
  const stageBlock = workflow.slice(
    workflow.indexOf("name: Stage raw sidecar release subjects"),
    workflow.indexOf("name: Generate staged-subject SBOM"),
  );
  assert.match(stageBlock, /node scripts\/release-supply-chain\.mjs assert-nonempty "\$STAGE"/);
  assert.ok(stageBlock.indexOf("assert-nonempty") < stageBlock.indexOf('echo "STAGE_DIR=$STAGE"'));
  assert.doesNotMatch(stageBlock, /Nothing to upload.*exit 0/);
});

test("empty staged subjects fail closed", () => {
  const stage = mkdtempSync(join(tmpdir(), "loregui-empty-stage-"));
  try {
    const result = spawnSync(process.execPath, [helper, "assert-nonempty", stage], {
      encoding: "utf8",
    });
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /no staged release subjects/);
  } finally {
    rmSync(stage, { recursive: true, force: true });
  }
});

test("checksum manifests are portable, deterministic, and unique per target triple", () => {
  const triples = [
    "x86_64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
  ];
  const names = new Set();

  for (const triple of triples) {
    const stage = mkdtempSync(join(tmpdir(), "loregui release stage-"));
    try {
      writeFileSync(join(stage, "z-server"), "server\n");
      writeFileSync(join(stage, "a-gui"), "gui\n");
      writeFileSync(join(stage, `sbom-${triple}.spdx.json`), "{}\n");
      const result = spawnSync(process.execPath, [helper, "checksums", stage, triple], {
        encoding: "utf8",
      });
      assert.equal(result.status, 0, result.stderr);

      const name = `SHA256SUMS-${triple}`;
      names.add(name);
      const manifest = readFileSync(join(stage, name), "utf8");
      const expected = ["a-gui", `sbom-${triple}.spdx.json`, "z-server"]
        .map((file) => `${createHash("sha256").update(readFileSync(join(stage, file))).digest("hex")}  ${file}`)
        .join("\n") + "\n";
      assert.equal(manifest, expected);
      assert.doesNotMatch(manifest, /SHA256SUMS/);
    } finally {
      rmSync(stage, { recursive: true, force: true });
    }
  }

  assert.equal(names.size, triples.length);
});

test("the static gate executes under bash on Linux, macOS, and Windows", () => {
  assert.match(guard, /os: \[ubuntu-latest, macos-latest, windows-latest\]/);
  assert.match(guard, /runs-on: \$\{\{ matrix\.os \}\}/);
  assert.match(guard, /shell: bash\n\s+run: node --test scripts\/release-supply-chain\.test\.mjs/);
});
