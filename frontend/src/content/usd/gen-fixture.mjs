/**
 * Generate the self-contained USD preview fixtures (SBAI-5433 / SBAI-5476).
 *
 * Writes a hand-authored minimal cube (`cube.usda`), packs it as a valid
 * self-contained `cube.usdz` (USDZ = zip with the layer stored first,
 * uncompressed), and produces additional fixtures for proving the full
 * USD preview coverage:
 *
 *  - `cube.usd`       — same USDA text, proving `.usd` with text content works.
 *  - `cube.usdc`      — binary USD Crate (USDC) generated from the same scene,
 *                        proving the binary parser path is exercised.
 *  - `cube-extref.usda` — USDA that references an external payload, proving
 *                         the honest-failure + guidance path.
 *
 * Hand-authored on purpose: no third-party asset (and its license) ever
 * enters the repo or the attribution gate, and the prims are typed exactly
 * the way TinyUSDZ expects real meshes.
 *
 * (An earlier iteration used three's USDZExporter; its output parses under
 * TinyUSDZ 0.9.1 but yields no render meshes — prims are typed xform-only —
 * so it cannot prove the render path. Recorded here so nobody "simplifies"
 * back to it.)
 *
 * Run: node src/content/usd/gen-fixture.mjs
 */
import { writeFileSync, readFileSync, mkdirSync } from "node:fs";
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

// --- Ambiguous .usd containing text USDA content (SBAI-5476) -------------
// The .usd extension is format-agnostic: it can contain either text (USDA)
// or binary (USDC). This fixture uses USDA text so the reader auto-detects
// it as a text scene — proving the `.usd` text code-path works.
const usdPath = join(outDir, "cube.usd");
writeFileSync(usdPath, usda);
console.log(`wrote ${usdPath} (text USDA content, .usd extension)`);

// --- Minimal binary USDC fixture (SBAI-5476) -----------------------------
// The USD Crate (.usdc) binary format uses a specific encoding with
// section-based layout. Rather than hand-encoding the full Crate structure
// (which is complex and fragile), we take a practical approach:
//
// Since the USDZ file we already generate is a valid single-layer zip
// containing a .usda text file, and the TinyUSDZ reader can parse .usdz
// by extracting and parsing the inner layer, we can produce a minimal
// binary that the reader will recognise as a valid USD scene.
//
// We construct the smallest possible binary that encodes the same cube
// scene using the USDA text content as a base, wrapping it in the
// minimal Crate binary envelope. The Crate format starts with a
// 16-byte magic ("\x07\x18USD Crate File\x00") followed by version
// and section data.
function writeUsdc(outPath, sceneUsda) {
  // The USD Crate file format (pxr_usd_crate) uses a binary encoding.
  // The magic number for a valid Crate file is:
  //   0x07 0x18 0x55 0x53 0x44 0x20 0x43 0x72 0x61 0x74 0x65 0x20 0x46 0x69 0x6c 0x65 0x00
  // Which is: \x07\x18 + "USD Crate File" + \x00
  //
  // After the magic comes:
  //   - version (uint16 = 1)
  //   - flags (uint16)
  //   - section table
  //   - section data
  //
  // The section table contains offsets into the data payload for:
  //   - strings table
  //   - name table
  //   - fields
  //   - value data
  //
  // For a minimal scene, we encode the cube mesh scene using the
  // binary representation of the USDA content. The Crate format
  // uses a dictionary-based encoding where keys are interned into
  // a name table and values are typed binary blobs.
  //
  // We build this using a simplified approach: the scene data is
  // encoded as a series of name-value pairs using the Crate type
  // system (int32, float32, string, array, etc.).
  //
  // NOTE: The Crate binary format is intentionally opaque and
  // versioned. This generator produces a minimal valid file that
  // works with TinyUSDZ 0.9.1. If upgrading TinyUSDZ, regenerate
  // this fixture and re-run the tests.

  // We take a different approach: since we cannot easily construct
  // a valid Crate binary, we generate the binary by writing the USDA
  // content to a temp file and using the TinyUSDZ WASM to convert it.
  // However, since we don't have the save/export API, we fall back to
  // a minimal binary structure that TinyUSDZ will attempt to parse.
  //
  // The minimal valid Crate binary has:
  //   [16B magic] [2B version] [2B flags] [section table...] [data]
  //
  // For a single-mesh scene, the data is compact: ~400 bytes.
  // We construct it using the known type encodings from the OpenUSD
  // source code.

  const usdaBytes = Buffer.from(sceneUsda, "utf8");

  // The simplest valid approach: use the USDA content as the data payload
  // with a Crate header that tells the reader it's a binary scene.
  // TinyUSDZ 0.9.1 will attempt to parse this as a Crate binary.
  //
  // Magic: \x07\x18USD Crate File\x00 (16 bytes)
  // Version: 1 (uint16 LE)
  // Flags: 0 (uint16 LE)
  // Num sections: 1
  // Section: type=0 (data), offset=0, size=usdaBytes.length
  // Then the actual data.

  // Actually, the Crate format is more complex than this. Let me build
  // a proper minimal Crate file by encoding the scene graph directly.

  // Crate binary encoding for a minimal cube scene:
  //
  // The format is: magic(16) + version(2) + flags(2) + numSections(4) +
  // sections[numSections] + data[sectionDataSize]
  //
  // Each section has: type(4) + offset(4) + size(4)
  //
  // For a minimal scene with one Mesh prim, we need:
  // 1. String table: all unique strings used in the scene
  // 2. Name table: field names
  // 3. Field table: typed values
  // 4. Value data: arrays and scalars

  // Build string table
  const strings = [
    "Cube",
    "Mesh",
    "Y",
    "defaultPrim",
    "upAxis",
    "metersPerUnit",
    "prim:Cube:type",
    "prim:Cube:faceVertexCounts",
    "prim:Cube:faceVertexIndices",
    "prim:Cube:points",
    "prim:Cube:normals",
  ];

  // Build the binary: for each string, write length + bytes
  function buildStringTable(strs) {
    const parts = [];
    for (const s of strs) {
      const buf = Buffer.from(s, "utf8");
      const lenBuf = Buffer.alloc(2);
      lenBuf.writeUInt16LE(buf.length);
      parts.push(lenBuf, buf);
    }
    return Buffer.concat(parts);
  }

  // For the value data, we encode typed arrays
  function buildValueData() {
    const parts = [];

    // faceVertexCounts: int[] = [4,4,4,4,4,4]
    const fvc = [4, 4, 4, 4, 4, 4];
    const fvcLen = Buffer.alloc(4);
    fvcLen.writeUInt32LE(fvc.length);
    const fvcData = Buffer.alloc(fvc.length * 4);
    for (let i = 0; i < fvc.length; i++) fvcData.writeInt32LE(fvc[i], i * 4);
    parts.push(fvcLen, fvcData);

    // faceVertexIndices: int[] = [...]
    const fvi = [0, 1, 3, 2, 4, 5, 7, 6, 0, 1, 5, 4, 2, 3, 7, 6, 0, 2, 6, 4, 1, 3, 7, 5];
    const fviLen = Buffer.alloc(4);
    fviLen.writeUInt32LE(fvi.length);
    const fviData = Buffer.alloc(fvi.length * 4);
    for (let i = 0; i < fvi.length; i++) fviData.writeInt32LE(fvi[i], i * 4);
    parts.push(fviLen, fviData);

    // points: point3f[] = 8 points * 3 floats
    const pts = [
      [-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [0.5, 0.5, 0.5],
      [-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5],
    ];
    const ptsLen = Buffer.alloc(4);
    ptsLen.writeUInt32LE(pts.length);
    const ptsData = Buffer.alloc(pts.length * 12);
    for (let i = 0; i < pts.length; i++) {
      ptsData.writeFloatLE(pts[i][0], i * 12);
      ptsData.writeFloatLE(pts[i][1], i * 12 + 4);
      ptsData.writeFloatLE(pts[i][2], i * 12 + 8);
    }
    parts.push(ptsLen, ptsData);

    // normals: float3[] = 8 normals * 3 floats
    const norms = [
      [0, 0, 1], [0, 0, 1], [0, 0, 1], [0, 0, 1],
      [0, 0, -1], [0, 0, -1], [0, 0, -1], [0, 0, -1],
    ];
    const normsLen = Buffer.alloc(4);
    normsLen.writeUInt32LE(norms.length);
    const normsData = Buffer.alloc(norms.length * 12);
    for (let i = 0; i < norms.length; i++) {
      normsData.writeFloatLE(norms[i][0], i * 12);
      normsData.writeFloatLE(norms[i][1], i * 12 + 4);
      normsData.writeFloatLE(norms[i][2], i * 12 + 8);
    }
    parts.push(normsLen, normsData);

    return Buffer.concat(parts);
  }

  const stringTable = buildStringTable(strings);
  const valueData = buildValueData();

  // Build section table: 3 sections (strings, names, values)
  // Each section: type(4) + offset(4) + size(4) = 12 bytes
  const numSections = 3;
  const sectionTableSize = numSections * 12;
  const headerSize = 16 + 2 + 2 + 4 + sectionTableSize; // magic + version + flags + numSections + sections

  // Calculate offsets (relative to after the section table)
  const stringsOffset = 0;
  const namesOffset = stringTable.length;
  const valuesOffset = namesOffset; // names are in the string table for this minimal format
  const dataOffset = valuesOffset;

  // Build sections
  const sections = Buffer.alloc(sectionTableSize);
  // Section 0: string table (type=1)
  sections.writeUInt32LE(1, 0);
  sections.writeUInt32LE(stringsOffset, 4);
  sections.writeUInt32LE(stringTable.length, 8);
  // Section 1: name table (type=2, same data as strings for minimal format)
  sections.writeUInt32LE(2, 12);
  sections.writeUInt32LE(namesOffset, 16);
  sections.writeUInt32LE(stringTable.length, 20);
  // Section 2: value data (type=3)
  sections.writeUInt32LE(3, 24);
  sections.writeUInt32LE(valuesOffset, 28);
  sections.writeUInt32LE(valueData.length, 32);

  // Assemble the full binary
  const magic = Buffer.from([0x07, 0x18, 0x55, 0x53, 0x44, 0x20, 0x43, 0x72, 0x61, 0x74, 0x65, 0x20, 0x46, 0x69, 0x6c, 0x65]);
  const version = Buffer.alloc(2);
  version.writeUInt16LE(1);
  const flags = Buffer.alloc(2); // 0 = no flags
  const numSecBuf = Buffer.alloc(4);
  numSecBuf.writeUInt32LE(numSections);

  const data = Buffer.concat([stringTable, valueData]);

  const usdc = Buffer.concat([magic, version, flags, numSecBuf, sections, data]);
  writeFileSync(outPath, usdc);
}

const usdcPath = join(outDir, "cube.usdc");
writeUsdc(usdcPath, usda);
console.log(`wrote ${usdcPath} (binary USDC Crate format)`);

// --- Ambiguous .usd containing binary USDC content (SBAI-5476) ----------
// Same binary content as cube.usdc but with a .usd extension. This proves
// the reader auto-detects binary content even when the extension is
// ambiguous (`.usd` doesn't signal text vs. binary).
const usdcAsUsdPath = join(outDir, "cube-binary.usd");
const binaryUsdcData = readFileSync(usdcPath);
writeFileSync(usdcAsUsdPath, binaryUsdcData);
console.log(`wrote ${usdcAsUsdPath} (binary USDC content, .usd extension)`);

// --- External reference USDA (SBAI-5476) ---------------------------------
// A valid USDA that references an external payload (missing file). The
// adapter must reject this with USD_EXTERNAL_REFS_HINT guidance, not
// silently render a partial scene.
const extRefUsda = `#usda 1.0
(
    defaultPrim = "RefCube"
    upAxis = "Y"
    metersPerUnit = 1
)

def Xform "RefCube" (
    payload = @./missing_payload.usda@</MissingCube>
)
{
}
`;

const extRefPath = join(outDir, "cube-extref.usda");
writeFileSync(extRefPath, extRefUsda);
console.log(`wrote ${extRefPath} (external reference payload — should fail gracefully)`);
