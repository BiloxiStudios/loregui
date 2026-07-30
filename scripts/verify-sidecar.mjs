#!/usr/bin/env node

// SBAI-5560 / SBAI-5808: release gate for the bundled `loreserver` sidecar.
// The shipped v0.1.3 installers surfaced Windows' "Unsupported 16 Bit
// Application" because a corrupt / AV-quarantined `loreserver.exe` reached the
// user's disk. The old CI step only checked that SOME file was staged; the
// first header gate then assumed every release target was Windows PE. Verify
// existence, useful size, native executable format, and target architecture.
//
// Usage: verify-sidecar.mjs <path-to-loreserver[.exe]> <target-triple>

import { closeSync, openSync, readSync, statSync } from "node:fs";

// A real loreserver build is tens of MB (36,691,456 bytes in the v0.1.3
// installers); anything at or under 1 MB is a truncated download or a
// quarantine stub, never a runnable server.
const MIN_SIZE_BYTES = 1024 * 1024;

function fail(message) {
  console.error(`verify-sidecar: ${message}`);
  process.exit(1);
}

function hex(value, width) {
  return `0x${value.toString(16).padStart(width, "0")}`;
}

function readExactly(fd, length, position, description) {
  const buffer = Buffer.alloc(length);
  if (readSync(fd, buffer, 0, length, position) !== length) {
    fail(`sidecar is truncated before ${description} (offset ${position})`);
  }
  return buffer;
}

function verifyPe(fd, sidecarPath, target) {
  const dos = readExactly(fd, 64, 0, "the PE DOS header");
  if (dos[0] !== 0x4d || dos[1] !== 0x5a) {
    fail(`${sidecarPath} is missing the PE MZ magic — not a PE executable`);
  }
  const peOffset = dos.readUInt32LE(0x3c);
  const pe = readExactly(fd, 26, peOffset, "the PE and COFF headers");
  if (pe.toString("latin1", 0, 4) !== "PE\0\0") {
    fail(`${sidecarPath} is missing the PE signature at offset ${peOffset}`);
  }
  const machine = pe.readUInt16LE(4);
  if (machine !== target.machine) {
    fail(
      `${sidecarPath} has PE machine ${hex(machine, 4)} — expected ${hex(target.machine, 4)} (${target.arch})`,
    );
  }
  const characteristics = pe.readUInt16LE(22);
  if ((characteristics & 0x0002) === 0) {
    fail(
      `${sidecarPath} PE header is missing the executable image characteristic 0x0002`,
    );
  }
  const optionalMagic = pe.readUInt16LE(24);
  if (optionalMagic !== 0x020b) {
    fail(`${sidecarPath} has PE optional-header magic ${hex(optionalMagic, 4)} — expected 0x020b (PE32+)`);
  }
  return `PE machine ${target.arch}`;
}

function verifyElf(fd, sidecarPath, target) {
  const elf = readExactly(fd, 24, 0, "the ELF header");
  if (
    elf[0] !== 0x7f ||
    elf[1] !== 0x45 ||
    elf[2] !== 0x4c ||
    elf[3] !== 0x46
  ) {
    fail(`${sidecarPath} is missing the ELF magic — not an ELF executable`);
  }
  if (elf[4] !== 2) {
    fail(`${sidecarPath} is ELF class ${elf[4]} — expected ELF64 class 2`);
  }
  if (elf[5] !== 1) {
    fail(`${sidecarPath} is ELF data encoding ${elf[5]} — expected little-endian encoding 1`);
  }
  if (elf[6] !== 1) {
    fail(`${sidecarPath} is ELF identification version ${elf[6]} — expected version 1`);
  }
  const fileType = elf.readUInt16LE(16);
  if (fileType !== 2 && fileType !== 3) {
    fail(`${sidecarPath} has ELF type ${fileType} — expected ET_EXEC (2) or ET_DYN (3)`);
  }
  const machine = elf.readUInt16LE(18);
  if (machine !== target.machine) {
    fail(
      `${sidecarPath} has ELF machine ${hex(machine, 4)} — expected ${hex(target.machine, 4)} (${target.arch})`,
    );
  }
  const version = elf.readUInt32LE(20);
  if (version !== 1) {
    fail(`${sidecarPath} is ELF header version ${version} — expected version 1`);
  }
  return `ELF64 machine ${target.arch}`;
}

function verifyMachO(fd, sidecarPath, target) {
  const mach = readExactly(fd, 16, 0, "the Mach-O header");
  const magic = mach.readUInt32LE(0);
  if (magic === 0xcffaedfe) {
    fail(`${sidecarPath} has byte-swapped Mach-O magic — not executable on macOS`);
  }
  if (magic !== 0xfeedfacf) {
    fail(`${sidecarPath} is missing the Mach-O 64-bit magic — not a Mach-O executable`);
  }
  const cpuType = mach.readUInt32LE(4);
  if (cpuType !== target.machine) {
    fail(
      `${sidecarPath} has Mach-O CPU ${hex(cpuType, 8)} — expected ${hex(target.machine, 8)} (${target.arch})`,
    );
  }
  const fileType = mach.readUInt32LE(12);
  if (fileType !== 2) {
    fail(`${sidecarPath} has Mach-O file type ${fileType} — expected MH_EXECUTE (2)`);
  }
  return `Mach-O64 CPU ${target.arch}, little-endian`;
}

const TARGETS = {
  "x86_64-pc-windows-msvc": {
    arch: "x86_64",
    machine: 0x8664, // IMAGE_FILE_MACHINE_AMD64
    verify: verifyPe,
  },
  "x86_64-unknown-linux-gnu": {
    arch: "x86_64",
    machine: 0x003e, // EM_X86_64
    verify: verifyElf,
  },
  "aarch64-apple-darwin": {
    arch: "arm64",
    machine: 0x0100000c, // CPU_TYPE_ARM64
    verify: verifyMachO,
  },
};

const [sidecarPath, targetTriple] = process.argv.slice(2);
if (!sidecarPath || !targetTriple) {
  fail("usage: verify-sidecar.mjs <path-to-loreserver[.exe]> <target-triple>");
}

const target = TARGETS[targetTriple];
if (!target) {
  fail(
    `unknown target triple "${targetTriple}" — expected one of: ${Object.keys(TARGETS).join(", ")}`,
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
let formatDescription;
try {
  formatDescription = target.verify(fd, sidecarPath, target);
} finally {
  closeSync(fd);
}

console.log(
  `verify-sidecar: ${sidecarPath} OK (${stat.size} bytes, ${formatDescription}, target ${targetTriple})`,
);
