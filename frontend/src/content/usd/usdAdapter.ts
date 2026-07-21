/**
 * Quarantined USD → three.js adapter (SBAI-5433, public issue #387).
 *
 * The ONLY module in the app that touches the `tinyusdz` package (TinyUSDZ
 * compiled to WASM, Apache-2.0/MIT — the same engine the studio's
 * open-3d-viewer uses). Everything fragile about the dependency lives here:
 *
 *  1. **WASM asset seam.** The loader's own `init()` resolves `tinyusdz.wasm`
 *     relative to its module URL, which does not exist once Vite bundles the
 *     app (and deep-import pre-bundling breaks it in dev). v0.9.1 offers no
 *     public injection point for a bundled WASM URL, so the adapter passes an
 *     explicit `locateFile` to the emscripten glue and assigns the resulting
 *     native module onto the loader. If tinyusdz is ever upgraded, re-check
 *     this seam first.
 *  2. **No types.** The package ships no TypeScript declarations; the minimal
 *     surface we use is declared in `./tinyusdz.d.ts`.
 *  3. **Failure honesty.** TinyUSDZ cannot resolve external references from a
 *     single in-memory file (bare .usd/.usda/.usdc scenes that reference
 *     payloads/textures by path). Those fail here and the viewer surfaces the
 *     "package as .usdz" guidance — we never pretend a partial parse is a
 *     render.
 *
 * No globals, no eval, no code copied from the reference app — the call
 * pattern (loader → scene → default root → buildThreeNode) follows the
 * package's documented usage.
 */

import type * as THREE_NS from "three";
import initTinyUSDZNative from "tinyusdz/tinyusdz.js";
import { TinyUSDZLoader } from "tinyusdz/TinyUSDZLoader.js";
import { TinyUSDZLoaderUtils } from "tinyusdz/TinyUSDZLoaderUtils.js";
import tinyusdzWasmUrl from "tinyusdz/tinyusdz.wasm?url";

/** Error message fragment the viewer maps to USD-specific guidance. */
export const USD_EXTERNAL_REFS_HINT =
  "this USD file references external assets — package it as a self-contained .usdz to preview";

export interface UsdLoadResult {
  object: THREE_NS.Object3D;
}

type NativeModule = Awaited<ReturnType<typeof initTinyUSDZNative>>;

let loaderPromise: Promise<TinyUSDZLoader> | null = null;

/**
 * Shared, lazily-initialized TinyUSDZ loader. The 2.4 MB WASM is fetched from
 * the Vite-emitted asset URL exactly once per app session.
 */
async function getLoader(): Promise<TinyUSDZLoader> {
  if (!loaderPromise) {
    loaderPromise = (async () => {
      const native: NativeModule = await initTinyUSDZNative({
        // Quarantine seam (see header): serve the Vite-emitted WASM asset
        // instead of the glue's default relative resolution, which breaks
        // under bundling/pre-bundling.
        locateFile: () => tinyusdzWasmUrl,
      } as Parameters<typeof initTinyUSDZNative>[0]);
      const loader = new TinyUSDZLoader();
      // Skip loader.init(): its only WASM resolution paths are the broken
      // relative default and the fzstd-compressed variant we don't ship.
      loader.native_ = native;
      return loader;
    })();
    // A failed init must not poison the singleton — retry next time.
    loaderPromise.catch(() => {
      loaderPromise = null;
    });
  }
  return loaderPromise;
}

/**
 * Load a USD/USDZ file from an object URL and convert it to a three.js
 * Object3D ready for the scene graph. Throws with USD_EXTERNAL_REFS_HINT in
 * the message when the failure looks like an external-reference miss.
 */
export async function loadUsdToThree(url: string): Promise<UsdLoadResult> {
  const loader = await getLoader();
  let usdScene;
  try {
    usdScene = await loader.loadAsync(url);
  } catch (e) {
    throw withExternalRefsHint(e);
  }
  const rootNode = usdScene.getDefaultRootNode();
  if (!rootNode) {
    throw new Error("USD file contains no renderable scene");
  }
  const defaultMtl = TinyUSDZLoaderUtils.createDefaultMaterial();
  const object = TinyUSDZLoaderUtils.buildThreeNode(
    rootNode,
    defaultMtl,
    usdScene,
    { overrideMaterial: false },
  );
  if (!object) {
    throw new Error("USD parse produced no renderable geometry");
  }
  return { object };
}

/** Attach the external-refs guidance when the failure smells like one. */
function withExternalRefsHint(e: unknown): Error {
  const msg = e instanceof Error ? e.message : String(e);
  if (/asset|resolve|fetch|referenc|payload|texture|not found|failed/i.test(msg)) {
    return new Error(`${msg} — ${USD_EXTERNAL_REFS_HINT}`);
  }
  return e instanceof Error ? e : new Error(msg);
}
