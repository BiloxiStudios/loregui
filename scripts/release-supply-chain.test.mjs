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
const autoReleaseWorkflow = normalizeNewlines(
  readFileSync(new URL("../.github/workflows/auto-release.yml", import.meta.url), "utf8"),
);
const guard = normalizeNewlines(
  readFileSync(new URL("../.github/workflows/release-supply-chain.yml", import.meta.url), "utf8"),
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

test("auto-release fetches the exact locked graph before offline version stamping", () => {
  const bumpStep = autoReleaseWorkflow.slice(
    autoReleaseWorkflow.indexOf("name: Bump workspace version + tag"),
    autoReleaseWorkflow.indexOf("name: Dispatch release build on the tag"),
  );
  const lockedFetch = bumpStep.match(/^\s*cargo fetch --locked\s*$/m);
  const manifestMutation = bumpStep.indexOf('sed -i -E "0,/^version =');
  const tauriMutation = bumpStep.indexOf('sed -i -E "s/\\"version\\":');
  const offlineStamp = bumpStep.indexOf("cargo update -w --offline");

  assert.ok(lockedFetch, "the fresh runner must populate the exact locked graph");
  const lockedFetchOffset = bumpStep.indexOf(lockedFetch[0]);
  assert.ok(lockedFetchOffset < manifestMutation, "fetch must run before Cargo.toml changes");
  assert.ok(lockedFetchOffset < tauriMutation, "fetch must run before tauri.conf.json changes");
  assert.ok(lockedFetchOffset < offlineStamp, "fetch must run before the offline version stamp");
  assert.equal(bumpStep.match(/^\s*cargo fetch --locked\s*$/gm)?.length, 1);
  assert.equal(bumpStep.match(/^\s*cargo update -w --offline\s*$/gm)?.length, 1);
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

test("release workflow validates each target format and proves corrupt sidecars are rejected", () => {
  const staged = workflow.indexOf("name: Verify staged sidecar exists");
  const corrupt = workflow.indexOf("name: Prove corrupt sidecar gate rejects target format");
  const build = workflow.indexOf("name: Build + publish (Tauri → installers → GitHub Release)");
  const bundled = workflow.indexOf("name: Verify bundled sidecar in produced bundle");
  const stage = workflow.indexOf("name: Stage raw sidecar release subjects");
  assert.ok(
    staged >= 0 &&
      staged < corrupt &&
      corrupt < build &&
      build < bundled &&
      bundled < stage,
  );

  // Pre-build: every matrix leg validates the real staged binary against its
  // target triple, then corrupts a copy and proves the same validator rejects it.
  const stagedBlock = workflow.slice(staged, corrupt);
  assert.match(
    stagedBlock,
    /node scripts\/verify-sidecar\.mjs "\$STAGED" \$\{\{ matrix\.triple \}\}/,
  );
  const corruptBlock = workflow.slice(corrupt, build);
  assert.match(corruptBlock, /cp "\$STAGED" "\$CORRUPT"/);
  assert.match(corruptBlock, /Buffer\.alloc\(4\)/);
  assert.match(corruptBlock, /x86_64-pc-windows-msvc\) EXPECTED_FAILURE="PE MZ magic"/);
  assert.match(corruptBlock, /x86_64-unknown-linux-gnu\) EXPECTED_FAILURE="ELF magic"/);
  assert.match(corruptBlock, /aarch64-apple-darwin\) EXPECTED_FAILURE="Mach-O 64-bit magic"/);
  assert.match(
    corruptBlock,
    /if VERIFY_OUTPUT="\$\(node scripts\/verify-sidecar\.mjs "\$CORRUPT" \$\{\{ matrix\.triple \}\} 2>&1\)"; then/,
  );
  assert.match(corruptBlock, /grep -Fq "\$EXPECTED_FAILURE"/);
  assert.match(corruptBlock, /Rule 40 enforcement proved/);

  // Post-build: the bundled copy inside an unpacked bundle layout (the macOS
  // .app) is verified; packed-installer platforms fall back to re-verifying
  // the staged sidecar the bundler embedded.
  const bundledBlock = workflow.slice(bundled, stage);
  assert.match(bundledBlock, /bundle\/macos\/LoreGUI\.app\/Contents\/MacOS\/loreserver/);
  assert.match(
    bundledBlock,
    /node scripts\/verify-sidecar\.mjs "\$BUNDLED" \$\{\{ matrix\.triple \}\}/,
  );
  assert.match(
    bundledBlock,
    /node scripts\/verify-sidecar\.mjs "src-tauri\/binaries\/loreserver-\$\{\{ matrix\.triple \}\}\$\{\{ matrix\.sidecar_ext \}\}" \$\{\{ matrix\.triple \}\}/,
  );
});

const SYNTHETIC_EXECUTABLE_SIZE = 1024 * 1024 + 1024;

function syntheticPe(machine, { executable = true } = {}) {
  const buffer = Buffer.alloc(SYNTHETIC_EXECUTABLE_SIZE);
  buffer.write("MZ", 0, "latin1");
  buffer.writeUInt32LE(0x80, 0x3c);
  buffer.write("PE\0\0", 0x80, "latin1");
  buffer.writeUInt16LE(machine, 0x84);
  buffer.writeUInt16LE(1, 0x86); // NumberOfSections
  buffer.writeUInt16LE(0xf0, 0x94); // SizeOfOptionalHeader (PE32+)
  buffer.writeUInt16LE(executable ? 0x0022 : 0x0020, 0x96); // Characteristics
  buffer.writeUInt16LE(0x020b, 0x98); // PE32+ optional-header magic
  return buffer;
}

function syntheticElf(machine, { fileType = 3 } = {}) {
  const buffer = Buffer.alloc(SYNTHETIC_EXECUTABLE_SIZE);
  buffer.set([0x7f, 0x45, 0x4c, 0x46], 0);
  buffer[4] = 2; // ELFCLASS64
  buffer[5] = 1; // ELFDATA2LSB
  buffer[6] = 1; // EV_CURRENT
  buffer.writeUInt16LE(fileType, 16); // ET_DYN (PIE) by default
  buffer.writeUInt16LE(machine, 18);
  buffer.writeUInt32LE(1, 20); // EV_CURRENT
  return buffer;
}

function syntheticMachO(cpuType, { littleEndian = true, fileType = 2 } = {}) {
  const buffer = Buffer.alloc(SYNTHETIC_EXECUTABLE_SIZE);
  if (littleEndian) {
    buffer.writeUInt32LE(0xfeedfacf, 0); // MH_MAGIC_64
    buffer.writeUInt32LE(cpuType, 4);
    buffer.writeUInt32LE(fileType, 12);
  } else {
    buffer.writeUInt32BE(0xfeedfacf, 0); // MH_CIGAM_64 byte order
    buffer.writeUInt32BE(cpuType, 4);
    buffer.writeUInt32BE(fileType, 12);
  }
  return buffer;
}

const targetFixtures = [
  {
    name: "Windows PE64 x86_64",
    filename: "loreserver.exe",
    triple: "x86_64-pc-windows-msvc",
    format: /PE/,
    contents: syntheticPe(0x8664),
  },
  {
    name: "Linux ELF64 x86_64",
    filename: "loreserver-linux",
    triple: "x86_64-unknown-linux-gnu",
    format: /ELF/,
    contents: syntheticElf(0x3e),
  },
  {
    name: "macOS Mach-O64 arm64",
    filename: "loreserver-macos",
    triple: "aarch64-apple-darwin",
    format: /Mach-O/,
    contents: syntheticMachO(0x0100000c),
  },
];

for (const fixture of targetFixtures) {
  test(`verify-sidecar.mjs accepts ${fixture.name}`, () => {
    const dir = mkdtempSync(join(tmpdir(), "loregui-verify-sidecar-"));
    try {
      const valid = join(dir, fixture.filename);
      writeFileSync(valid, fixture.contents);
      const result = spawnSync(process.execPath, [verifySidecar, valid, fixture.triple], {
        encoding: "utf8",
      });
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, fixture.format);
      assert.match(result.stdout, /OK/);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}

test("verify-sidecar.mjs rejects corrupt magic for every release target", () => {
  const dir = mkdtempSync(join(tmpdir(), "loregui-verify-sidecar-"));
  try {
    for (const fixture of targetFixtures.slice(0, 3)) {
      const corrupt = Buffer.from(fixture.contents);
      corrupt.fill(0, 0, 4);
      const path = join(dir, `corrupt-${fixture.filename}`);
      writeFileSync(path, corrupt);
      const result = spawnSync(process.execPath, [verifySidecar, path, fixture.triple], {
        encoding: "utf8",
      });
      assert.notEqual(result.status, 0, `${fixture.name} corrupt magic must be rejected`);
      assert.match(result.stderr, fixture.format, fixture.name);
      assert.match(result.stderr, /magic/, fixture.name);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("verify-sidecar.mjs rejects non-executable and byte-swapped native headers", () => {
  const dir = mkdtempSync(join(tmpdir(), "loregui-verify-sidecar-"));
  try {
    const cases = [
      [
        "pe-without-executable-flag.exe",
        syntheticPe(0x8664, { executable: false }),
        "x86_64-pc-windows-msvc",
        /PE.*executable image/,
      ],
      [
        "elf-et-none",
        syntheticElf(0x3e, { fileType: 0 }),
        "x86_64-unknown-linux-gnu",
        /ELF type 0.*ET_EXEC.*ET_DYN/,
      ],
      [
        "macho-object",
        syntheticMachO(0x0100000c, { fileType: 1 }),
        "aarch64-apple-darwin",
        /Mach-O file type 1.*MH_EXECUTE/,
      ],
      [
        "macho-byte-swapped",
        syntheticMachO(0x0100000c, { littleEndian: false }),
        "aarch64-apple-darwin",
        /byte-swapped Mach-O/,
      ],
    ];
    for (const [name, contents, triple, pattern] of cases) {
      const path = join(dir, name);
      writeFileSync(path, contents);
      const result = spawnSync(process.execPath, [verifySidecar, path, triple], {
        encoding: "utf8",
      });
      assert.notEqual(result.status, 0, `${name} must be rejected`);
      assert.match(result.stderr, pattern, name);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("verify-sidecar.mjs rejects wrong architectures, truncated files, and unknown triples", () => {
  const dir = mkdtempSync(join(tmpdir(), "loregui-verify-sidecar-"));
  try {
    const cases = [
      [
        "wrong-pe-machine.exe",
        syntheticPe(0xaa64),
        "x86_64-pc-windows-msvc",
        /PE machine 0xaa64.*expected 0x8664/,
      ],
      [
        "wrong-elf-machine",
        syntheticElf(0xb7),
        "x86_64-unknown-linux-gnu",
        /ELF machine 0x00b7.*expected 0x003e/,
      ],
      [
        "wrong-macho-cpu",
        syntheticMachO(0x01000007),
        "aarch64-apple-darwin",
        /Mach-O CPU 0x01000007.*expected 0x0100000c/,
      ],
      ["empty.exe", Buffer.alloc(0), "x86_64-pc-windows-msvc", /only 0 bytes/],
    ];
    for (const [name, contents, triple, pattern] of cases) {
      const path = join(dir, name);
      writeFileSync(path, contents);
      const result = spawnSync(process.execPath, [verifySidecar, path, triple], {
        encoding: "utf8",
      });
      assert.notEqual(result.status, 0, `${name} must be rejected`);
      assert.match(result.stderr, pattern, name);
    }

    const missing = spawnSync(
      process.execPath,
      [verifySidecar, join(dir, "absent.exe"), "x86_64-pc-windows-msvc"],
      { encoding: "utf8" },
    );
    assert.notEqual(missing.status, 0);
    assert.match(missing.stderr, /not found/);

    const unknown = join(dir, "unknown-target");
    writeFileSync(unknown, syntheticElf(0x3e));
    const unknownResult = spawnSync(
      process.execPath,
      [verifySidecar, unknown, "x86_64-unknown-freebsd"],
      { encoding: "utf8" },
    );
    assert.notEqual(unknownResult.status, 0);
    assert.match(unknownResult.stderr, /unknown target triple/);
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
