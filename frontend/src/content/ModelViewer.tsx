import { useEffect, useRef, useState } from "react";

/**
 * Lightweight three.js 3D model viewer for the Preview tab (SBAI-4083).
 *
 * Loaded lazily (React.lazy in PreviewView) so three.js (~600 KB) never lands
 * in the initial bundle — it ships in its own chunk fetched only when a user
 * previews a glTF/glb/fbx/obj. Renders the model in an orbit-able canvas with a
 * neutral studio light rig; fbx/obj are best-effort (no materials/animation
 * guarantees, per the ticket).
 *
 * The viewer is theme-aware: the canvas clear color reads the live
 * `--surface-elevated-bg` token so the 3D stage re-themes with the rest of the
 * app. All colors come from tokens — nothing hardcoded.
 */

type ModelExt = "gltf" | "glb" | "fbx" | "obj";

function readSurfaceColor(varName: string, fallback: number): number {
  const raw = getComputedStyle(document.documentElement)
    .getPropertyValue(varName)
    .trim();
  if (!raw) return fallback;
  // Accept #rgb / #rrggbb; fall back otherwise (three parses hex strings).
  try {
    const probe = document.createElement("div");
    probe.style.color = raw;
    document.body.appendChild(probe);
    const rgb = getComputedStyle(probe).color;
    document.body.removeChild(probe);
    const m = rgb.match(/\d+/g);
    if (m && m.length >= 3) {
      return (Number(m[0]) << 16) | (Number(m[1]) << 8) | Number(m[2]);
    }
  } catch {
    /* ignore */
  }
  return fallback;
}

export default function ModelViewer({
  url,
  ext,
}: {
  url: string;
  ext: ModelExt;
}) {
  const mountRef = useRef<HTMLDivElement | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let disposed = false;
    let cleanup: (() => void) | undefined;

    void (async () => {
      try {
        const THREE = await import("three");
        const { OrbitControls } = await import(
          "three/examples/jsm/controls/OrbitControls.js"
        );

        const mount = mountRef.current;
        if (!mount || disposed) return;

        const width = mount.clientWidth || 480;
        const height = mount.clientHeight || 360;

        const scene = new THREE.Scene();
        const clear = readSurfaceColor("--surface-elevated-bg", 0x161b22);
        scene.background = new THREE.Color(clear);

        const camera = new THREE.PerspectiveCamera(
          50,
          width / height,
          0.01,
          1000,
        );
        camera.position.set(0, 0, 5);

        const renderer = new THREE.WebGLRenderer({ antialias: true });
        renderer.setPixelRatio(window.devicePixelRatio);
        renderer.setSize(width, height);
        mount.appendChild(renderer.domElement);

        const controls = new OrbitControls(camera, renderer.domElement);
        controls.enableDamping = true;

        // Neutral studio rig.
        scene.add(new THREE.AmbientLight(0xffffff, 0.7));
        const key = new THREE.DirectionalLight(0xffffff, 1.0);
        key.position.set(3, 5, 4);
        scene.add(key);
        const fill = new THREE.DirectionalLight(0xffffff, 0.4);
        fill.position.set(-4, -2, -3);
        scene.add(fill);

        const frameObject = (obj: import("three").Object3D) => {
          const box = new THREE.Box3().setFromObject(obj);
          const size = box.getSize(new THREE.Vector3());
          const center = box.getCenter(new THREE.Vector3());
          obj.position.sub(center);
          const maxDim = Math.max(size.x, size.y, size.z) || 1;
          camera.position.set(0, maxDim * 0.4, maxDim * 2.2);
          camera.near = maxDim / 100;
          camera.far = maxDim * 100;
          camera.updateProjectionMatrix();
          controls.target.set(0, 0, 0);
          controls.update();
          scene.add(obj);
        };

        if (ext === "glb" || ext === "gltf") {
          const { GLTFLoader } = await import(
            "three/examples/jsm/loaders/GLTFLoader.js"
          );
          const loader = new GLTFLoader();
          const gltf = await loader.loadAsync(url);
          if (disposed) return;
          frameObject(gltf.scene);
        } else if (ext === "fbx") {
          const { FBXLoader } = await import(
            "three/examples/jsm/loaders/FBXLoader.js"
          );
          const obj = await new FBXLoader().loadAsync(url);
          if (disposed) return;
          obj.scale.setScalar(0.01); // FBX is usually in cm
          frameObject(obj);
        } else {
          const { OBJLoader } = await import(
            "three/examples/jsm/loaders/OBJLoader.js"
          );
          const obj = await new OBJLoader().loadAsync(url);
          if (disposed) return;
          obj.traverse((c) => {
            const mesh = c as import("three").Mesh;
            if (mesh.isMesh && !mesh.material) {
              mesh.material = new THREE.MeshStandardMaterial({
                color: 0x9aa4b2,
              });
            }
          });
          frameObject(obj);
        }

        setLoading(false);

        let raf = 0;
        const animate = () => {
          raf = requestAnimationFrame(animate);
          controls.update();
          renderer.render(scene, camera);
        };
        animate();

        const onResize = () => {
          const w = mount.clientWidth || width;
          const h = mount.clientHeight || height;
          camera.aspect = w / h;
          camera.updateProjectionMatrix();
          renderer.setSize(w, h);
        };
        window.addEventListener("resize", onResize);

        cleanup = () => {
          cancelAnimationFrame(raf);
          window.removeEventListener("resize", onResize);
          controls.dispose();
          renderer.dispose();
          if (renderer.domElement.parentNode === mount) {
            mount.removeChild(renderer.domElement);
          }
        };
      } catch (e) {
        if (!disposed) {
          setLoading(false);
          setError(e instanceof Error ? e.message : String(e));
        }
      }
    })();

    return () => {
      disposed = true;
      cleanup?.();
    };
  }, [url, ext]);

  return (
    <div className="cw-model-wrap">
      <div ref={mountRef} className="cw-model-canvas" />
      {loading && !error && <p className="cw-status">Loading 3D model…</p>}
      {error && (
        <p className="cw-error" role="alert">
          Could not render this model: {error}
        </p>
      )}
      {!loading && !error && (
        <p className="cw-hint">drag to orbit · scroll to zoom</p>
      )}
    </div>
  );
}
