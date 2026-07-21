/**
 * Generate the self-contained USD preview fixtures (SBAI-5433).
 *
 * Writes a hand-authored minimal cube (`cube.usda`) and packs it as a valid
 * self-contained `cube.usdz` (USDZ = zip with the layer stored first,
 * uncompressed). Hand-authored on purpose: no third-party asset (and its
 * license) ever enters the repo or the attribution gate, and the prims are
 * typed exactly the way TinyUSDZ expects real meshes.
 *
 * (An earlier iteration used three's USDZExporter; its output parses under
 * TinyUSDZ 0.9.1 but yields no render meshes — prims are typed xform-only —
 * so it cannot prove the render path. Recorded here so nobody "simplifies"
 * back to it.)
 *
 * Run: node src/content/usd/gen-fixture.mjs
 */
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "__fixtures__");
mkdirSync(outDir, { recursive: true });

// --- minimal CRC32 (zip entries need it) -----------------------------------
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xffffffff;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/**
 * Write a single-file USDZ: a zip whose first (only) entry is the USD layer,
 * STORED (no compression) with its data offset padded to a 64-byte boundary
 * via the extra field — the layout the USDZ spec mandates for direct
 * memory-mapped reads.
 */
function writeUsdz(outPath, layerName, layerBytes) {
  const name = Buffer.from(layerName, "utf8");
  const crc = crc32(layerBytes);
  const size = layerBytes.length;

  // Local header is 30 bytes + name + extra. Data must start at offset % 64.
  // Extra field: one padding record (header id 0xFFFF is ignored by readers,
  // but the USD convention is 0x0001 with the rest zeroed; both work — use
  // the simplest ignored id).
  const base = 30 + name.length + 4; // +4 for the extra-field header
  const pad = (64 - (base % 64)) % 64;
  const extraLen = 4 + pad;

  const local = Buffer.alloc(30 + name.length + extraLen);
  local.writeUInt32LE(0x04034b50, 0);
  local.writeUInt16LE(20, 4); // version needed
  local.writeUInt16LE(0, 6); // flags
  local.writeUInt16LE(0, 8); // method: stored
  local.writeUInt16LE(0, 10); // mod time
  local.writeUInt16LE(0x21, 12); // mod date (1980-01-01)
  local.writeUInt32LE(crc, 14);
  local.writeUInt32LE(size, 18);
  local.writeUInt32LE(size, 22);
  local.writeUInt16LE(name.length, 26);
  local.writeUInt16LE(extraLen, 28);
  name.copy(local, 30);
  local.writeUInt16LE(0xffff, 30 + name.length); // ignored padding id
  local.writeUInt16LE(pad, 30 + name.length + 2);

  const centralOffset = local.length + size;
  const central = Buffer.alloc(46 + name.length);
  central.writeUInt32LE(0x02014b50, 0);
  central.writeUInt16LE(20, 4); // version made by
  central.writeUInt16LE(20, 6); // version needed
  central.writeUInt16LE(0, 8); // flags
  central.writeUInt16LE(0, 10); // method: stored
  central.writeUInt16LE(0, 12); // mod time
  central.writeUInt16LE(0x21, 14); // mod date
  central.writeUInt32LE(crc, 16);
  central.writeUInt32LE(size, 20);
  central.writeUInt32LE(size, 24);
  central.writeUInt16LE(name.length, 28);
  central.writeUInt16LE(0, 30); // extra len (padding only needed locally)
  central.writeUInt16LE(0, 32); // comment len
  central.writeUInt16LE(0, 34); // disk number
  central.writeUInt16LE(0, 36); // internal attrs
  central.writeUInt32LE(0, 38); // external attrs
  central.writeUInt32LE(0, 42); // local header offset
  name.copy(central, 46);

  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(1, 8); // entries on this disk
  end.writeUInt16LE(1, 10); // total entries
  end.writeUInt32LE(central.length, 12);
  end.writeUInt32LE(centralOffset, 16);

  writeFileSync(outPath, Buffer.concat([local, layerBytes, central, end]));
}

const usda = `#usda 1.0
(
    defaultPrim = "Cube"
    upAxis = "Y"
    metersPerUnit = 1
)

def Mesh "Cube"
{
    int[] faceVertexCounts = [4, 4, 4, 4, 4, 4]
    int[] faceVertexIndices = [
        0, 1, 3, 2,
        4, 5, 7, 6,
        0, 1, 5, 4,
        2, 3, 7, 6,
        0, 2, 6, 4,
        1, 3, 7, 5
    ]
    point3f[] points = [
        (-0.5, -0.5, 0.5),
        (0.5, -0.5, 0.5),
        (-0.5, 0.5, 0.5),
        (0.5, 0.5, 0.5),
        (-0.5, -0.5, -0.5),
        (0.5, -0.5, -0.5),
        (-0.5, 0.5, -0.5),
        (0.5, 0.5, -0.5)
    ]
    float3[] normals = [
        (0, 0, 1), (0, 0, 1), (0, 0, 1), (0, 0, 1),
        (0, 0, -1), (0, 0, -1), (0, 0, -1), (0, 0, -1)
    ]
}
`;

const usdaPath = join(outDir, "cube.usda");
writeFileSync(usdaPath, usda);
console.log(`wrote ${usdaPath} (${usda.length} bytes)`);

// USDZ = single-file zip, layer first, STORED, 64-byte-aligned (see writeUsdz).
const usdzPath = join(outDir, "cube.usdz");
writeUsdz(usdzPath, "cube.usda", Buffer.from(usda, "utf8"));
console.log(`wrote ${usdzPath} (self-contained, aligned usdz)`);
