/**
 * Stub for the optional `fzstd` peer of the tinyusdz package (SBAI-5433).
 *
 * TinyUSDZLoader.js contains a dynamic `import('fzstd')` used ONLY on the
 * zstd-compressed-WASM path (`useZstdCompressedWasm: true`). loregui never
 * takes that path — the adapter initializes the uncompressed WASM via an
 * explicit asset URL — so the package is not shipped. This stub exists so the
 * bundler can resolve the import; if the compressed path is ever enabled by
 * mistake it fails loudly here instead of shipping a silent behavior change.
 */
export function decompress(): never {
  throw new Error(
    "fzstd is not shipped in loregui: the TinyUSDZ zstd-compressed WASM path " +
      "(useZstdCompressedWasm) is disabled — see src/content/usd/usdAdapter.ts",
  );
}
export function compress(): never {
  return decompress();
}
