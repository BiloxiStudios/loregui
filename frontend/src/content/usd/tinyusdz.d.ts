/**
 * Minimal declarations for the `tinyusdz` package surface used by
 * ./usdAdapter.ts. The package ships no types of its own (v0.9.1);
 * everything typed here is exercised by the adapter's tests.
 */

declare module "tinyusdz/tinyusdz.js" {
  /** Emscripten MODULARIZE entry. We only ever pass locateFile. */
  export default function initTinyUSDZNative(options?: {
    locateFile?: (path: string, prefix: string) => string;
    wasmBinary?: ArrayBuffer | Uint8Array;
  }): Promise<Record<string, unknown>>;
}

declare module "tinyusdz/TinyUSDZLoader.js" {
  export interface UsdScene {
    getDefaultRootNode(): unknown;
  }
  export class TinyUSDZLoader {
    /** Assigned by the adapter after explicit WASM init (see usdAdapter). */
    native_: unknown;
    init(options?: { useZstdCompressedWasm?: boolean }): Promise<this>;
    loadAsync(url: string, onProgress?: (e: ProgressEvent) => void): Promise<UsdScene>;
  }
}

declare module "tinyusdz/TinyUSDZLoaderUtils.js" {
  import type * as THREE from "three";
  import type { UsdScene } from "tinyusdz/TinyUSDZLoader.js";
  export const TinyUSDZLoaderUtils: {
    createDefaultMaterial(): THREE.Material;
    buildThreeNode(
      rootNode: unknown,
      defaultMaterial: THREE.Material,
      usdScene: UsdScene,
      options?: { overrideMaterial?: boolean },
    ): THREE.Object3D | null;
  };
}

declare module "tinyusdz/tinyusdz.wasm?url" {
  const url: string;
  export default url;
}
