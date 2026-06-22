import { useCallback, useEffect, useState } from "react";
import {
  api,
  storageApi,
  sharedStoreApi,
  type SharedStoreInfoResult,
  type StorageBackendConfig,
} from "./api";
import BackendPicker from "./onboarding/server/BackendPicker";

/**
 * Storage panel (sidebar/topbar nav, daily domain) — the rich home for the
 * storage + shared_store domains, per `docs/INFORMATION-ARCHITECTURE.md` and
 * `docs/domains/storage.md`.
 *
 * Shows the configured backend + connection status, a real open→put→get→
 * obliterate connectivity round-trip (the same probe the onboarding
 * `ValidateConnectivity` step runs), `flush`, fragment metadata lookup
 * (`get_metadata`), and shared-store info — plus reconfiguring the backend via
 * the reused `BackendPicker`. Themed entirely via `--surface-*` tokens.
 */

type ConnStep = "idle" | "testing" | "pass" | "fail";

const TEST_KEY = "__lore_panel_connectivity_check__";
const TEST_DATA = [79, 75]; // "OK" as bytes

export default function StoragePanel({ onClose }: { onClose: () => void }) {
  // The backend the user has configured this session (from the picker or a
  // prior onboarding run). Null → empty state.
  const [config, setConfig] = useState<StorageBackendConfig | null>(null);
  const [reconfigure, setReconfigure] = useState(false);

  // Connectivity round-trip.
  const [conn, setConn] = useState<ConnStep>("idle");
  const [connError, setConnError] = useState<string | null>(null);

  // Open handle (for flush / metadata).
  const [handle, setHandle] = useState<number | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);

  // Flush.
  const [flushing, setFlushing] = useState(false);
  const [flushDone, setFlushDone] = useState(false);
  const [flushError, setFlushError] = useState<string | null>(null);

  // get_metadata.
  const [metaPartition, setMetaPartition] = useState(
    "00000000000000000000000000000001",
  );
  const [metaAddress, setMetaAddress] = useState("");
  const [metaResult, setMetaResult] = useState<string | null>(null);
  const [metaLoading, setMetaLoading] = useState(false);
  const [metaError, setMetaError] = useState<string | null>(null);

  // Shared-store info (usage overview).
  const [sharedInfo, setSharedInfo] = useState<SharedStoreInfoResult | null>(
    null,
  );
  const [sharedLoading, setSharedLoading] = useState(false);

  // Obliterate confirmation (destructive — clears the connectivity probe key).
  const [confirmObliterate, setConfirmObliterate] = useState(false);

  const backendLabel = config
    ? config.kind === "local"
      ? `Local · ${config.path || "(default path)"}`
      : `S3-compatible · ${config.endpoint || "(no endpoint)"}${
          config.bucket ? ` / ${config.bucket}` : ""
        }`
    : null;

  const loadSharedInfo = useCallback(async () => {
    setSharedLoading(true);
    try {
      setSharedInfo(await sharedStoreApi.info());
    } catch {
      setSharedInfo(null);
    } finally {
      setSharedLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadSharedInfo();
  }, [loadSharedInfo]);

  // Esc closes the panel (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Open a handle from the configured backend so flush/metadata have one.
  const ensureHandle = useCallback(async (): Promise<number> => {
    if (handle != null) return handle;
    setOpening(true);
    setOpenError(null);
    try {
      const repositoryPath = config?.kind === "local" ? config.path ?? "" : "";
      const remoteUrl = config?.kind !== "local" ? config?.endpoint ?? "" : "";
      const inMemory = !repositoryPath && !remoteUrl;
      const h = await storageApi.open(repositoryPath, remoteUrl, inMemory);
      setHandle(h);
      return h;
    } catch (e) {
      const msg = typeof e === "string" ? e : JSON.stringify(e);
      setOpenError(msg);
      throw e;
    } finally {
      setOpening(false);
    }
  }, [handle, config]);

  // Connectivity round-trip: open → put → get → obliterate, surfacing the real
  // error. Reuses the onboarding probe shape against the session API.
  const runConnectivity = useCallback(async () => {
    if (!config) return;
    setConn("testing");
    setConnError(null);
    try {
      await api.storageOpen(config);
      await api.storagePut(TEST_KEY, TEST_DATA);
      const got = await api.storageGet(TEST_KEY);
      const ok =
        got.length === TEST_DATA.length &&
        got.every((b, i) => b === TEST_DATA[i]);
      if (!ok) {
        throw new Error(`round-trip mismatch: wrote [${TEST_DATA}], got [${got}]`);
      }
      await api.storageObliterate(TEST_KEY);
      setConn("pass");
    } catch (e) {
      try {
        await api.storageObliterate(TEST_KEY);
      } catch {
        // ignore cleanup failure — key may not exist / store unreachable
      }
      setConnError(typeof e === "string" ? e : JSON.stringify(e));
      setConn("fail");
    }
  }, [config]);

  const runFlush = useCallback(async () => {
    setFlushing(true);
    setFlushDone(false);
    setFlushError(null);
    try {
      const h = await ensureHandle();
      await storageApi.flush(h);
      setFlushDone(true);
    } catch (e) {
      setFlushError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setFlushing(false);
    }
  }, [ensureHandle]);

  const runGetMetadata = useCallback(async () => {
    if (!metaAddress.trim()) return;
    setMetaLoading(true);
    setMetaError(null);
    setMetaResult(null);
    try {
      const h = await ensureHandle();
      const res = await storageApi.getMetadata(
        h,
        metaPartition.trim(),
        metaAddress.trim(),
      );
      const item = res.items[0];
      if (!item || !item.ok) {
        setMetaError(item?.error ?? "address not found");
      } else {
        setMetaResult(JSON.stringify(item, null, 2));
      }
    } catch (e) {
      setMetaError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setMetaLoading(false);
    }
  }, [ensureHandle, metaPartition, metaAddress]);

  // --- Empty state: no backend configured yet. ---
  const showPicker = !config || reconfigure;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Storage settings"
      className="storage-scrim"
      onClick={onClose}
    >
      <div
        className="storage-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <header className="storage-panel-header">
          <h2>Storage</h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        {/* --- Backend + connection status --- */}
        <section className="storage-section">
          <h3>Backend</h3>
          {!config && !reconfigure && (
            <div className="storage-empty">
              <p className="empty">No storage backend configured — choose one.</p>
            </div>
          )}
          {config && !reconfigure && (
            <div className="storage-backend-row">
              <div className="storage-backend-info">
                <span className="storage-backend-label">{backendLabel}</span>
                <span
                  className={`storage-status ${
                    conn === "pass"
                      ? "ok"
                      : conn === "fail"
                        ? "bad"
                        : "unknown"
                  }`}
                >
                  {conn === "pass"
                    ? "● connected"
                    : conn === "fail"
                      ? "● unreachable"
                      : "○ not tested"}
                </span>
              </div>
              <button onClick={() => setReconfigure(true)}>Reconfigure</button>
            </div>
          )}
          {showPicker && (
            <div className="storage-picker">
              <BackendPicker
                onConfigured={(c) => {
                  setConfig(c);
                  setReconfigure(false);
                  setConn("idle");
                  setHandle(null);
                  setConnError(null);
                }}
              />
              {reconfigure && config && (
                <button
                  className="storage-cancel"
                  onClick={() => setReconfigure(false)}
                >
                  Cancel
                </button>
              )}
            </div>
          )}
        </section>

        {/* --- Connectivity test --- */}
        {config && !reconfigure && (
          <section className="storage-section">
            <h3>Connectivity test</h3>
            <p className="storage-help">
              Runs a real round-trip (open → put → get → obliterate) against the
              configured backend.
            </p>
            {conn === "fail" && connError && (
              <div className="error storage-inline-error">{connError}</div>
            )}
            {conn === "pass" && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> Round-trip
                passed — backend is reachable.
              </div>
            )}
            <button
              className="storage-primary"
              disabled={conn === "testing"}
              onClick={() => void runConnectivity()}
            >
              {conn === "testing"
                ? "Testing…"
                : conn === "fail"
                  ? "Retry test"
                  : "Run connectivity test"}
            </button>
          </section>
        )}

        {/* --- Flush --- */}
        {config && !reconfigure && (
          <section className="storage-section">
            <h3>Flush pending writes</h3>
            <p className="storage-help">
              Force buffered writes to durable storage (fsync on disk-backed
              stores).
            </p>
            {openError && (
              <div className="error storage-inline-error">{openError}</div>
            )}
            {flushError && (
              <div className="error storage-inline-error">{flushError}</div>
            )}
            {flushDone && !flushing && (
              <div className="storage-ok">
                <span className="success-icon">&#10003;</span> Flushed.
              </div>
            )}
            <button
              disabled={flushing || opening}
              onClick={() => void runFlush()}
            >
              {flushing ? "Flushing…" : opening ? "Opening…" : "Flush"}
            </button>
          </section>
        )}

        {/* --- Fragment metadata / usage --- */}
        {config && !reconfigure && (
          <section className="storage-section">
            <h3>Fragment metadata</h3>
            <p className="storage-help">
              Look up a fragment's size and flags by partition + address (no
              bytes transferred).
            </p>
            <div className="onboarding-field">
              <label htmlFor="meta-partition">Partition</label>
              <input
                id="meta-partition"
                type="text"
                value={metaPartition}
                onChange={(e) => setMetaPartition(e.target.value)}
                placeholder="00000000000000000000000000000001"
              />
            </div>
            <div className="onboarding-field">
              <label htmlFor="meta-address">Address</label>
              <input
                id="meta-address"
                type="text"
                value={metaAddress}
                onChange={(e) => setMetaAddress(e.target.value)}
                placeholder="<hash>-<context>"
              />
            </div>
            {metaError && (
              <div className="error storage-inline-error">{metaError}</div>
            )}
            {metaResult && (
              <pre className="storage-pre">{metaResult}</pre>
            )}
            <button
              disabled={!metaAddress.trim() || metaLoading || opening}
              onClick={() => void runGetMetadata()}
            >
              {metaLoading ? "Looking up…" : "Get metadata"}
            </button>
          </section>
        )}

        {/* --- Shared stores --- */}
        <section className="storage-section">
          <h3>Shared stores</h3>
          {sharedLoading && <p className="storage-help">Loading…</p>}
          {!sharedLoading && sharedInfo && (
            <>
              <p className="storage-help">
                Used automatically:{" "}
                <strong>{sharedInfo.use_automatically ? "yes" : "no"}</strong>
              </p>
              {sharedInfo.stores.length === 0 ? (
                <p className="empty">No shared stores configured.</p>
              ) : (
                <ul className="storage-list">
                  {sharedInfo.stores.map((s, i) => (
                    <li key={i}>
                      <code>{s.path}</code>
                      <span
                        className={`storage-status ${s.exists ? "ok" : "bad"}`}
                      >
                        {s.exists ? "● exists" : "● missing"}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          {!sharedLoading && !sharedInfo && (
            <p className="empty">Shared-store info unavailable.</p>
          )}
          <button disabled={sharedLoading} onClick={() => void loadSharedInfo()}>
            {sharedLoading ? "Refreshing…" : "Refresh"}
          </button>
        </section>

        {/* --- Danger zone: obliterate the connectivity probe key --- */}
        {config && !reconfigure && (
          <section className="storage-section storage-danger">
            <h3>Danger zone</h3>
            <p className="storage-help">
              Obliterate permanently deletes the connectivity-probe fragment.
              This cannot be undone.
            </p>
            {!confirmObliterate ? (
              <button
                className="storage-danger-btn"
                onClick={() => setConfirmObliterate(true)}
              >
                Obliterate probe key
              </button>
            ) : (
              <div className="storage-confirm">
                <span>Permanently delete the probe fragment?</span>
                <button
                  className="storage-danger-btn"
                  onClick={() =>
                    void (async () => {
                      try {
                        await api.storageObliterate(TEST_KEY);
                      } catch {
                        // idempotent — already gone
                      } finally {
                        setConfirmObliterate(false);
                      }
                    })()
                  }
                >
                  Yes, obliterate
                </button>
                <button onClick={() => setConfirmObliterate(false)}>
                  Cancel
                </button>
              </div>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
