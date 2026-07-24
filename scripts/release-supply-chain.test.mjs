import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";

function normalizeNewlines(text) {
  return text.replace(/\r\n/g, "\n");
}

const workflow = normalizeNewlines(
  readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8"),
);
const guard = normalizeNewlines(
  readFileSync(new URL("../.github/workflows/boundary-guard.yml", import.meta.url), "utf8"),
);
const helper = fileURLToPath(new URL("./release-supply-chain.mjs", import.meta.url));
const verifySidecar = fileURLToPath(new URL("./verify-sidecar.mjs", import.meta.url));

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

  assert.match(workflow, /path: "\$\{\{ env\.STAGE_DIR_NATIVE \}\}"/);
  assert.match(workflow, /output-file: "\$\{\{ env\.STAGE_DIR_NATIVE \}\}\/sbom-\$\{\{ matrix\.triple \}\}\.spdx\.json"/);
  assert.match(workflow, /upload-artifact: false/);
  assert.match(workflow, /upload-release-assets: false/);
  assert.match(workflow, /node scripts\/release-supply-chain\.mjs checksums "\$STAGE_DIR" "\$\{\{ matrix\.triple \}\}"/);
  assert.match(workflow, /gh release upload "\$TAG" "\$STAGE_DIR"\/\*/);
  assert.match(workflow, /subject-path: "\$\{\{ env\.STAGE_DIR_NATIVE \}\}\/\*"/);
  assert.doesNotMatch(workflow, /\n\s+path: \./);
  assert.doesNotMatch(workflow, /\bsha256sum\b|\bshasum\b/);
});

test("native actions receive a Windows-native view of the staged directory", () => {
  const stageBlock = workflow.slice(
    workflow.indexOf("name: Stage raw sidecar release subjects"),
    workflow.indexOf("name: Generate staged-subject SBOM"),
  );
  const sbomBlock = workflow.slice(
    workflow.indexOf("name: Generate staged-subject SBOM"),
    workflow.indexOf("name: Checksum and upload staged supply-chain assets"),
  );
  const checksumBlock = workflow.slice(
    workflow.indexOf("name: Checksum and upload staged supply-chain assets"),
    workflow.indexOf("name: Attest exact released subjects"),
  );
  const attestBlock = workflow.slice(workflow.indexOf("name: Attest exact released subjects"));

  assert.match(stageBlock, /if \[ "\$RUNNER_OS" = "Windows" \]; then/);
  assert.match(stageBlock, /STAGE_NATIVE="\$\(cygpath -m "\$STAGE"\)"/);
  assert.match(stageBlock, /echo "STAGE_DIR=\$STAGE" >> "\$GITHUB_ENV"/);
  assert.match(stageBlock, /echo "STAGE_DIR_NATIVE=\$STAGE_NATIVE" >> "\$GITHUB_ENV"/);
  assert.match(sbomBlock, /path: "\$\{\{ env\.STAGE_DIR_NATIVE \}\}"/);
  assert.match(
    sbomBlock,
    /output-file: "\$\{\{ env\.STAGE_DIR_NATIVE \}\}[\\/]sbom-\$\{\{ matrix\.triple \}\}\.spdx\.json"/,
  );
  assert.match(checksumBlock, /checksums "\$STAGE_DIR"/);
  assert.match(checksumBlock, /gh release upload "\$TAG" "\$STAGE_DIR"\/\*/);
  assert.match(attestBlock, /subject-path: "\$\{\{ env\.STAGE_DIR_NATIVE \}\}[\\/]\*"/);
});

test(
  "Git Bash stage conversion resolves in a native Windows process",
  { skip: process.platform !== "win32" },
  () => {
    const result = spawnSync(
      "bash",
      [
        "-lc",
        'set -euo pipefail; stage="$(mktemp -d)"; printf native-path-ok > "$stage/probe"; cygpath -m "$stage"',
      ],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    const nativeStage = result.stdout.trim();
    assert.match(nativeStage, /^[A-Za-z]:\//);
    assert.equal(readFileSync(join(nativeStage, "probe"), "utf8"), "native-path-ok");
  },
);

test("release workflow rejects an empty stage before exporting STAGE_DIR", () => {
  const stageBlock = workflow.slice(
    workflow.indexOf("name: Stage raw sidecar release subjects"),
    workflow.indexOf("name: Generate staged-subject SBOM"),
  );
  assert.match(stageBlock, /node scripts\/release-supply-chain\.mjs assert-nonempty "\$STAGE"/);
  assert.ok(stageBlock.indexOf("assert-nonempty") < stageBlock.indexOf('echo "STAGE_DIR=$STAGE"'));
  assert.doesNotMatch(stageBlock, /Nothing to upload.*exit 0/);
});

test("release workflow PE-verifies the sidecar before and after tauri build (SBAI-5560)", () => {
  const staged = workflow.indexOf("name: Verify staged sidecar exists");
  const build = workflow.indexOf("name: Build + publish (Tauri → installers → GitHub Release)");
  const bundled = workflow.indexOf("name: Verify bundled sidecar in produced bundle");
  const stage = workflow.indexOf("name: Stage raw sidecar release subjects");
  assert.ok(staged >= 0 && staged < build && build < bundled && bundled < stage);

  // Pre-build: the staged sidecar for every matrix leg is PE-verified, not
  // just existence-checked, with the machine field picked per target triple.
  const stagedBlock = workflow.slice(staged, build);
  assert.match(
    stagedBlock,
    /node scripts\/verify-sidecar\.mjs "\$STAGED" \$\{\{ matrix\.machine \}\}/,
  );
  assert.match(workflow, /triple: x86_64-pc-windows-msvc\n\s+sidecar_ext: "\.exe"\n\s+machine: x64/);
  assert.match(workflow, /triple: x86_64-unknown-linux-gnu\n\s+sidecar_ext: ""\n\s+machine: x64/);
  assert.match(workflow, /triple: aarch64-apple-darwin\n\s+sidecar_ext: ""\n\s+machine: arm64/);

  // Post-build: the bundled copy inside an unpacked bundle layout (the macOS
  // .app) is verified; packed-installer platforms fall back to re-verifying
  // the staged sidecar the bundler embedded.
  const bundledBlock = workflow.slice(bundled, stage);
  assert.match(bundledBlock, /bundle\/macos\/LoreGUI\.app\/Contents\/MacOS\/loreserver/);
  assert.match(
    bundledBlock,
    /node scripts\/verify-sidecar\.mjs "\$BUNDLED" \$\{\{ matrix\.machine \}\}/,
  );
  assert.match(
    bundledBlock,
    /node scripts\/verify-sidecar\.mjs "src-tauri\/binaries\/loreserver-\$\{\{ matrix\.triple \}\}\$\{\{ matrix\.sidecar_ext \}\}" \$\{\{ matrix\.machine \}\}/,
  );
});

// A minimal synthetic PE: MZ magic, e_lfanew at 0x80, PE signature, and the
// given machine field — padded past the script's 1 MB size floor.
function syntheticPe(machine) {
  const buffer = Buffer.alloc(1024 * 1024 + 1024);
  buffer.write("MZ", 0, "latin1");
  buffer.writeUInt32LE(0x80, 0x3c);
  buffer.write("PE\0\0", 0x80, "latin1");
  buffer.writeUInt16LE(machine, 0x84);
  return buffer;
}

test("verify-sidecar.mjs accepts a valid PE64 sidecar (SBAI-5560)", () => {
  const dir = mkdtempSync(join(tmpdir(), "loregui-verify-sidecar-"));
  try {
    const valid = join(dir, "loreserver.exe");
    writeFileSync(valid, syntheticPe(0x8664));
    const result = spawnSync(process.execPath, [verifySidecar, valid, "x64"], {
      encoding: "utf8",
    });
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /OK/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("verify-sidecar.mjs rejects corrupt sidecars (SBAI-5560)", () => {
  const dir = mkdtempSync(join(tmpdir(), "loregui-verify-sidecar-"));
  try {
    const cases = [
      ["empty.exe", Buffer.alloc(0), /only 0 bytes/],
      ["text.exe", Buffer.from("this is not an executable\n".repeat(50000)), /MZ magic/],
      ["wrong-machine.exe", syntheticPe(0xaa64), /machine 0xaa64.*expected 0x8664/],
    ];
    for (const [name, contents, pattern] of cases) {
      const path = join(dir, name);
      writeFileSync(path, contents);
      const result = spawnSync(process.execPath, [verifySidecar, path, "x64"], {
        encoding: "utf8",
      });
      assert.notEqual(result.status, 0, `${name} must be rejected`);
      assert.match(result.stderr, pattern, name);
    }

    const missing = spawnSync(
      process.execPath,
      [verifySidecar, join(dir, "absent.exe"), "x64"],
      { encoding: "utf8" },
    );
    assert.notEqual(missing.status, 0);
    assert.match(missing.stderr, /not found/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
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
  assert.equal(normalizeNewlines("shell: bash\r\n  run: contract\r\n"), "shell: bash\n  run: contract\n");
  assert.match(guard, /os: \[ubuntu-latest, macos-latest, windows-latest\]/);
  assert.match(guard, /runs-on: \$\{\{ matrix\.os \}\}/);
  assert.match(guard, /shell: bash\n\s+run: node --test scripts\/release-supply-chain\.test\.mjs/);
});
