#!/usr/bin/env node

// SBAI-5560: release gate for the bundled `loreserver` sidecar. The shipped
// v0.1.3 installers surfaced Windows' "Unsupported 16 Bit Application" because
// a corrupt / AV-quarantined `loreserver.exe` reached the user's disk — the old
// CI step only checked that SOME file was staged. This script verifies the
// file is a real executable: it must exist, be non-trivially sized, and carry
// a valid PE header (MZ magic, PE signature at the MZ-header e_lfanew offset,
// and the expected machine field).
//
// Usage: verify-sidecar.mjs <path-to-loreserver[.exe]> <x64|arm64>

import { closeSync, openSync, readSync, statSync } from "node:fs";

// A real loreserver build is tens of MB (36,691,456 bytes in the v0.1.3
// installers); anything at or under 1 MB is a truncated download or a
// quarantine stub, never a runnable server.
const MIN_SIZE_BYTES = 1024 * 1024;

// IMAGE_FILE_MACHINE values from the PE spec.
const MACHINES = {
  x64: 0x8664, // IMAGE_FILE_MACHINE_AMD64
  arm64: 0xaa64, // IMAGE_FILE_MACHINE_ARM64
};

function fail(message) {
  console.error(`verify-sidecar: ${message}`);
  process.exit(1);
}

const [sidecarPath, machineArg] = process.argv.slice(2);
if (!sidecarPath || !machineArg) {
  fail("usage: verify-sidecar.mjs <path-to-loreserver[.exe]> <x64|arm64>");
}

const expectedMachine = MACHINES[machineArg.toLowerCase()];
if (expectedMachine === undefined) {
  fail(
    `unknown machine "${machineArg}" — expected one of: ${Object.keys(MACHINES).join(", ")}`,
  );
}

let stat;
try {
  stat = statSync(sidecarPath);
} catch (error) {
  fail(`sidecar not found at ${sidecarPath}: ${error.message}`);
}
if (!stat.isFile()) {
  fail(`${sidecarPath} is not a regular file`);
}
if (stat.size <= MIN_SIZE_BYTES) {
  fail(
    `${sidecarPath} is only ${stat.size} bytes (expected a >1 MB executable) — truncated or quarantined?`,
  );
}

const fd = openSync(sidecarPath, "r");
try {
  // DOS header: MZ magic + e_lfanew (PE header offset) at 0x3c.
  const dos = Buffer.alloc(64);
  readSync(fd, dos, 0, dos.length, 0);
  if (dos[0] !== 0x4d || dos[1] !== 0x5a) {
    fail(`${sidecarPath} is missing the MZ magic — not a PE executable`);
  }
  const peOffset = dos.readUInt32LE(0x3c);

  // PE signature + COFF machine field.
  const pe = Buffer.alloc(6);
  if (readSync(fd, pe, 0, pe.length, peOffset) !== pe.length) {
    fail(`${sidecarPath} is truncated before the PE header (offset ${peOffset})`);
  }
  if (pe.toString("latin1", 0, 4) !== "PE\0\0") {
    fail(`${sidecarPath} is missing the PE signature at offset ${peOffset}`);
  }
  const machine = pe.readUInt16LE(4);
  if (machine !== expectedMachine) {
    fail(
      `${sidecarPath} has PE machine 0x${machine.toString(16).padStart(4, "0")} — expected 0x${expectedMachine.toString(16).padStart(4, "0")} (${machineArg})`,
    );
  }
} finally {
  closeSync(fd);
}

console.log(
  `verify-sidecar: ${sidecarPath} OK (${stat.size} bytes, PE machine ${machineArg})`,
);
