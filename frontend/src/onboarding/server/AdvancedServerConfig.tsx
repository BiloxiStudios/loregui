import { useCallback, useMemo, useState } from "react";
import type {
  HostAdvancedOptions,
  HostPeerOption,
  HostTopologyOptions,
} from "../../api";

/**
 * Advanced ("Expert mode") server configuration surface for the host flow
 * (SBAI-4075). Renders every lore-server option the wizard exposes, grouped into
 * collapsible sections. Fully controlled: the parent owns the
 * {@link HostAdvancedOptions} value (plus the shared bind host) and receives
 * granular updates.
 *
 * Defaults handling mirrors the backend: every field is optional, an empty field
 * means "use lore's compiled-in default", and the placeholder/help text shows
 * exactly what that default is. So leaving the whole surface untouched produces
 * the same minimal local config the simple first-run flow always did.
 */

export interface AdvancedServerConfigProps {
  /** Current advanced config (controlled). */
  value: HostAdvancedOptions;
  /** Bind host for every endpoint (controlled, shared with Basic mode). */
  bindHost: string;
  /** Emit a new advanced config. */
  onChange: (next: HostAdvancedOptions) => void;
  /** Emit a new bind host. */
  onBindHostChange: (next: string) => void;
  /** Disable all inputs (e.g. while the server is starting). */
  disabled?: boolean;
  /** Per-section validation errors keyed by stable field id (see validate()). */
  errors?: Record<string, string>;
}

/* ------------------------------------------------------------------ */
/* Generic field building blocks                                      */
/* ------------------------------------------------------------------ */

interface NumFieldProps {
  id: string;
  label: string;
  /** lore default, shown as placeholder + in help. */
  placeholder: string;
  help?: string;
  value: number | undefined;
  onChange: (v: number | undefined) => void;
  disabled?: boolean;
  min?: number;
  max?: number;
  step?: number;
  error?: string;
}

function NumField({
  id,
  label,
  placeholder,
  help,
  value,
  onChange,
  disabled,
  min,
  max,
  step,
  error,
}: NumFieldProps) {
  return (
    <div
      className={`onboarding-field${error ? " server-config-field--invalid" : ""}`}
    >
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type="number"
        inputMode="numeric"
        placeholder={placeholder}
        value={value ?? ""}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        aria-invalid={error ? true : undefined}
        aria-describedby={help ? `${id}-help` : undefined}
        onChange={(e) => {
          const raw = e.target.value.trim();
          onChange(raw === "" ? undefined : Number(raw));
        }}
      />
      {help && (
        <p id={`${id}-help`} className="onboarding-field-hint">
          {help}
        </p>
      )}
      {error && <p className="server-config-field-error">{error}</p>}
    </div>
  );
}

interface TextFieldProps {
  id: string;
  label: string;
  placeholder?: string;
  help?: string;
  value: string | undefined;
  onChange: (v: string | undefined) => void;
  disabled?: boolean;
  error?: string;
}

function TextField({
  id,
  label,
  placeholder,
  help,
  value,
  onChange,
  disabled,
  error,
}: TextFieldProps) {
  return (
    <div
      className={`onboarding-field${error ? " server-config-field--invalid" : ""}`}
    >
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type="text"
        placeholder={placeholder}
        value={value ?? ""}
        disabled={disabled}
        aria-invalid={error ? true : undefined}
        aria-describedby={help ? `${id}-help` : undefined}
        onChange={(e) => {
          const raw = e.target.value;
          onChange(raw === "" ? undefined : raw);
        }}
      />
      {help && (
        <p id={`${id}-help`} className="onboarding-field-hint">
          {help}
        </p>
      )}
      {error && <p className="server-config-field-error">{error}</p>}
    </div>
  );
}

interface SelectFieldProps {
  id: string;
  label: string;
  help?: string;
  value: string | undefined;
  options: { value: string; label: string }[];
  /** Label for the "use lore default" empty option. */
  defaultLabel: string;
  onChange: (v: string | undefined) => void;
  disabled?: boolean;
}

function SelectField({
  id,
  label,
  help,
  value,
  options,
  defaultLabel,
  onChange,
  disabled,
}: SelectFieldProps) {
  return (
    <div className="onboarding-field">
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={value ?? ""}
        disabled={disabled}
        aria-describedby={help ? `${id}-help` : undefined}
        onChange={(e) => {
          const raw = e.target.value;
          onChange(raw === "" ? undefined : raw);
        }}
      >
        <option value="">{defaultLabel}</option>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      {help && (
        <p id={`${id}-help`} className="onboarding-field-hint">
          {help}
        </p>
      )}
    </div>
  );
}

interface BoolFieldProps {
  id: string;
  label: string;
  /** Shown next to the value selector to explain the lore default. */
  help: string;
  value: boolean | undefined;
  onChange: (v: boolean | undefined) => void;
  disabled?: boolean;
}

/**
 * A tri-state boolean: "Default" (omit → lore default), "On", "Off". Using a
 * select rather than a checkbox preserves the unset state, which matters because
 * omitting a key means "use lore's default" (which may itself be true).
 */
function BoolField({ id, label, help, value, onChange, disabled }: BoolFieldProps) {
  const sel = value === undefined ? "" : value ? "true" : "false";
  return (
    <div className="onboarding-field">
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={sel}
        disabled={disabled}
        aria-describedby={`${id}-help`}
        onChange={(e) => {
          const raw = e.target.value;
          onChange(raw === "" ? undefined : raw === "true");
        }}
      >
        <option value="">Default</option>
        <option value="true">On</option>
        <option value="false">Off</option>
      </select>
      <p id={`${id}-help`} className="onboarding-field-hint">
        {help}
      </p>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Collapsible section                                                */
/* ------------------------------------------------------------------ */

interface SectionProps {
  id: string;
  title: string;
  /** A short summary shown on the right of the header (e.g. "default"). */
  summary?: string;
  defaultOpen?: boolean;
  children: React.ReactNode;
}

function Section({ id, title, summary, defaultOpen, children }: SectionProps) {
  const [open, setOpen] = useState(Boolean(defaultOpen));
  const bodyId = `${id}-body`;
  return (
    <div className="server-config-section">
      <button
        type="button"
        className="server-config-section-toggle"
        aria-expanded={open}
        aria-controls={bodyId}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="server-config-section-caret" aria-hidden="true">
          ▶
        </span>
        <span>{title}</span>
        {summary && <span className="server-config-section-summary">{summary}</span>}
      </button>
      {open && (
        <div id={bodyId} className="server-config-section-body">
          {children}
        </div>
      )}
    </div>
  );
}

/* ------------------------------------------------------------------ */
/* Main component                                                     */
/* ------------------------------------------------------------------ */

export default function AdvancedServerConfig({
  value,
  bindHost,
  onChange,
  onBindHostChange,
  disabled,
  errors = {},
}: AdvancedServerConfigProps) {
  // Section-update helpers. Each returns an updater that merges a partial patch
  // into the named section and drops the section entirely when it becomes empty,
  // so an all-default config serializes to nothing.
  const patchSection = useCallback(
    <K extends keyof HostAdvancedOptions>(
      key: K,
      patch: Partial<NonNullable<HostAdvancedOptions[K]>>,
    ) => {
      const current = (value[key] ?? {}) as Record<string, unknown>;
      const merged: Record<string, unknown> = { ...current, ...patch };
      // Strip undefined / empty so we don't ship empty sub-objects.
      for (const k of Object.keys(merged)) {
        const v = merged[k];
        if (v === undefined || v === "" || (Array.isArray(v) && v.length === 0)) {
          delete merged[k];
        }
      }
      const next: HostAdvancedOptions = { ...value };
      if (Object.keys(merged).length === 0) {
        delete next[key];
      } else {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        next[key] = merged as any;
      }
      onChange(next);
    },
    [value, onChange],
  );

  const quic = value.quic ?? {};
  const grpc = value.grpc ?? {};
  const http = value.http ?? {};
  const localStore = value.localStore ?? {};
  const telemetry = value.telemetry ?? {};
  const runtime = value.runtime ?? {};
  const features = value.features ?? {};
  const timeouts = value.timeouts ?? {};
  const quicInternal = value.quicInternal ?? {};
  const replication = value.replicationEndpoint ?? {};
  const topology = value.topology ?? {};
  const notification = value.notification ?? {};

  const topologyProvider = topology.provider ?? "none";
  const peers = topology.peers ?? [];

  const setTopology = useCallback(
    (patch: Partial<HostTopologyOptions>) => {
      const merged: HostTopologyOptions = { ...topology, ...patch };
      // Normalise: provider "none"/empty with no peers → drop the whole section.
      const prov = merged.provider ?? "none";
      const next: HostAdvancedOptions = { ...value };
      const isDefault =
        (prov === "none" || prov === undefined) &&
        (merged.peers ?? []).length === 0 &&
        merged.rotationIntervalSeconds === undefined;
      if (isDefault) {
        delete next.topology;
      } else {
        next.topology = merged;
      }
      onChange(next);
    },
    [topology, value, onChange],
  );

  const updatePeer = useCallback(
    (index: number, patch: Partial<HostPeerOption>) => {
      const nextPeers = peers.map((p, i) => (i === index ? { ...p, ...patch } : p));
      setTopology({ peers: nextPeers });
    },
    [peers, setTopology],
  );

  const addPeer = useCallback(() => {
    setTopology({
      peers: [...peers, { address: "", port: 41337, locality: "SameRegion" }],
    });
  }, [peers, setTopology]);

  const removePeer = useCallback(
    (index: number) => {
      setTopology({ peers: peers.filter((_, i) => i !== index) });
    },
    [peers, setTopology],
  );

  // Per-section "is anything set?" summaries for the collapsed header.
  const summary = useCallback(
    (obj: object) =>
      Object.values(obj).some(
        (v) => v !== undefined && v !== "" && !(Array.isArray(v) && v.length === 0),
      )
        ? "customized"
        : "default",
    [],
  );

  const topologySummary = useMemo(
    () => (topologyProvider === "none" ? "single node" : topologyProvider),
    [topologyProvider],
  );

  return (
    <div className="server-config-advanced">
      {/* Network / QUIC / gRPC / HTTP -------------------------------- */}
      <Section
        id="sc-network"
        title="Network: bind host, QUIC, gRPC, HTTP"
        summary={summary({ bindHost: bindHost || undefined, ...quic, ...grpc, ...http })}
      >
        <TextField
          id="sc-bind-host"
          label="Bind host"
          placeholder="127.0.0.1"
          help="Address every endpoint binds to. Default 127.0.0.1 (loopback only). Use 0.0.0.0 to expose on the network — you then own firewalling and certs."
          value={bindHost || undefined}
          onChange={(v) => onBindHostChange(v ?? "")}
          disabled={disabled}
        />

        <h3>QUIC transport</h3>
        <div className="server-config-grid">
          <BoolField
            id="sc-quic-verify"
            label="Verify client certs (mTLS)"
            help="Default off — clients are verified by auth token, not a cert."
            value={quic.verifyClientCerts}
            onChange={(v) => patchSection("quic", { verifyClientCerts: v })}
            disabled={disabled}
          />
          <NumField
            id="sc-quic-idle"
            label="Idle timeout (ms)"
            placeholder="30000"
            value={quic.idleTimeout}
            onChange={(v) => patchSection("quic", { idleTimeout: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-quic-keepalive"
            label="Keep-alive (ms)"
            placeholder="500"
            value={quic.keepAlive}
            onChange={(v) => patchSection("quic", { keepAlive: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-quic-bidi"
            label="Max bidi streams"
            placeholder="8"
            value={quic.maxBidiStreams}
            onChange={(v) => patchSection("quic", { maxBidiStreams: v })}
            disabled={disabled}
            min={1}
          />
          <NumField
            id="sc-quic-listeners"
            label="Listeners"
            placeholder="10"
            value={quic.numListeners}
            onChange={(v) => patchSection("quic", { numListeners: v })}
            disabled={disabled}
            min={1}
            max={255}
            error={errors["quic.numListeners"]}
          />
          <NumField
            id="sc-quic-bps"
            label="Bandwidth cap (bits/s)"
            placeholder="1073741824"
            help="1073741824 = 1 Gbit/s."
            value={quic.transportBitsPerSecond}
            onChange={(v) => patchSection("quic", { transportBitsPerSecond: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-quic-rtt"
            label="Expected RTT (ms)"
            placeholder="100"
            value={quic.transportRtt}
            onChange={(v) => patchSection("quic", { transportRtt: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-quic-handler"
            label="Handler timeout (s)"
            placeholder="50"
            value={quic.handlerTimeoutSeconds}
            onChange={(v) => patchSection("quic", { handlerTimeoutSeconds: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-quic-msglimit"
            label="Connection msg limit"
            placeholder="unbounded"
            value={quic.connectionMessageLimit}
            onChange={(v) => patchSection("quic", { connectionMessageLimit: v })}
            disabled={disabled}
            min={1}
          />
        </div>

        <h3>gRPC</h3>
        <div className="server-config-grid">
          <BoolField
            id="sc-grpc-verify"
            label="Verify client certs (mTLS)"
            help="Default on for the gRPC endpoint."
            value={grpc.verifyClientCerts}
            onChange={(v) => patchSection("grpc", { verifyClientCerts: v })}
            disabled={disabled}
          />
          <NumField
            id="sc-grpc-handler"
            label="Handler timeout (s)"
            placeholder="50"
            value={grpc.requestHandlerTimeoutSeconds}
            onChange={(v) =>
              patchSection("grpc", { requestHandlerTimeoutSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-grpc-ka-int"
            label="HTTP/2 keepalive (s)"
            placeholder="unset"
            value={grpc.http2KeepaliveIntervalSeconds}
            onChange={(v) =>
              patchSection("grpc", { http2KeepaliveIntervalSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-grpc-ka-to"
            label="HTTP/2 keepalive timeout (s)"
            placeholder="unset"
            value={grpc.http2KeepaliveTimeoutSeconds}
            onChange={(v) =>
              patchSection("grpc", { http2KeepaliveTimeoutSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
        </div>

        <h3>HTTP</h3>
        <div className="server-config-grid">
          <NumField
            id="sc-http-maxfile"
            label="Max file size (bytes)"
            placeholder="10485760"
            help="10485760 = 10 MB."
            value={http.maxFileSize}
            onChange={(v) => patchSection("http", { maxFileSize: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-http-reqto"
            label="Request timeout (s)"
            placeholder="300"
            value={http.requestTimeoutSeconds}
            onChange={(v) => patchSection("http", { requestTimeoutSeconds: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-http-bodyto"
            label="Body timeout (s)"
            placeholder="3600"
            value={http.requestBodyTimeoutSeconds}
            onChange={(v) =>
              patchSection("http", { requestBodyTimeoutSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-http-availint"
            label="Availability interval (s)"
            placeholder="30"
            value={http.availableIntervalSeconds}
            onChange={(v) =>
              patchSection("http", { availableIntervalSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-http-availto"
            label="Availability timeout (s)"
            placeholder="5"
            value={http.availableTimeoutSeconds}
            onChange={(v) =>
              patchSection("http", { availableTimeoutSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
          <BoolField
            id="sc-http-health"
            label="Store health check"
            help="Default off."
            value={http.storeHealthCheck}
            onChange={(v) => patchSection("http", { storeHealthCheck: v })}
            disabled={disabled}
          />
        </div>
      </Section>

      {/* Storage ----------------------------------------------------- */}
      <Section
        id="sc-storage"
        title="Storage: local store tuning & lock store"
        summary={summary({ ...localStore, lockStoreMode: value.lockStoreMode })}
      >
        <p className="onboarding-field-hint">
          These tune the local filesystem store. S3 (object storage) is set in
          Basic mode. Compaction/eviction/capacity apply to a local immutable
          store only.
        </p>
        <div className="server-config-grid">
          <NumField
            id="sc-store-flush"
            label="Flush delay (s)"
            placeholder="10"
            value={localStore.flushDelaySeconds}
            onChange={(v) => patchSection("localStore", { flushDelaySeconds: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-store-compact"
            label="Compaction delay"
            placeholder="unset"
            value={localStore.compactionDelay}
            onChange={(v) => patchSection("localStore", { compactionDelay: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-store-evict"
            label="Eviction delay"
            placeholder="unset"
            value={localStore.evictionDelay}
            onChange={(v) => patchSection("localStore", { evictionDelay: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-store-maxcap"
            label="Max capacity (entries)"
            placeholder="unbounded"
            value={localStore.maxCapacity}
            onChange={(v) => patchSection("localStore", { maxCapacity: v })}
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-store-maxsize"
            label="Max size (bytes)"
            placeholder="unbounded"
            value={localStore.maxSize}
            onChange={(v) => patchSection("localStore", { maxSize: v })}
            disabled={disabled}
            min={0}
          />
        </div>
        <SelectField
          id="sc-lock-mode"
          label="Lock store mode"
          help="Default: local (in-memory). Other modes need a matching plugin."
          value={value.lockStoreMode}
          defaultLabel="Default (local)"
          options={[{ value: "local", label: "local (in-memory)" }]}
          onChange={(v) => onChange({ ...value, lockStoreMode: v })}
          disabled={disabled}
        />
      </Section>

      {/* Topology & Replication ------------------------------------- */}
      <Section
        id="sc-topology"
        title="Topology & replication"
        summary={topologySummary}
      >
        <SelectField
          id="sc-topo-provider"
          label="Topology provider"
          help="Default: none (single node). 'fixed' / 'rotating_id_fixed' need a peer list."
          value={topologyProvider === "none" ? undefined : topologyProvider}
          defaultLabel="None (single node)"
          options={[
            { value: "fixed", label: "Fixed peer list" },
            { value: "rotating_id_fixed", label: "Fixed peers, rotating IDs" },
          ]}
          onChange={(v) => setTopology({ provider: v ?? "none" })}
          disabled={disabled}
        />

        {topologyProvider === "rotating_id_fixed" && (
          <NumField
            id="sc-topo-rotation"
            label="Rotation interval (s)"
            placeholder="required"
            help="Required for rotating_id_fixed: how often peer IDs rotate."
            value={topology.rotationIntervalSeconds}
            onChange={(v) => setTopology({ rotationIntervalSeconds: v })}
            disabled={disabled}
            min={1}
            error={errors["topology.rotationIntervalSeconds"]}
          />
        )}

        {topologyProvider !== "none" && (
          <div className="server-config-peers">
            <h3>Peers</h3>
            {peers.length === 0 && (
              <p className="onboarding-field-hint">
                Add at least one peer for this provider.
              </p>
            )}
            {peers.map((peer, i) => (
              <div className="server-config-grid" key={i}>
                <TextField
                  id={`sc-peer-addr-${i}`}
                  label={`Peer ${i + 1} address`}
                  placeholder="10.0.0.2"
                  value={peer.address || undefined}
                  onChange={(v) => updatePeer(i, { address: v ?? "" })}
                  disabled={disabled}
                  error={errors[`topology.peers.${i}.address`]}
                />
                <NumField
                  id={`sc-peer-port-${i}`}
                  label="Port"
                  placeholder="41337"
                  value={peer.port}
                  onChange={(v) => updatePeer(i, { port: v ?? 0 })}
                  disabled={disabled}
                  min={1}
                  max={65535}
                />
                <SelectField
                  id={`sc-peer-loc-${i}`}
                  label="Locality"
                  value={peer.locality}
                  defaultLabel="SameRegion"
                  options={[
                    { value: "SameRegion", label: "SameRegion" },
                    { value: "OtherRegion", label: "OtherRegion" },
                  ]}
                  onChange={(v) => updatePeer(i, { locality: v })}
                  disabled={disabled}
                />
                <div className="onboarding-field">
                  <label aria-hidden="true">&nbsp;</label>
                  <button
                    type="button"
                    className="onboarding-button onboarding-button--danger"
                    onClick={() => removePeer(i)}
                    disabled={disabled}
                  >
                    Remove peer {i + 1}
                  </button>
                </div>
              </div>
            ))}
            <button
              type="button"
              className="onboarding-button"
              onClick={addPeer}
              disabled={disabled}
            >
              Add peer
            </button>
          </div>
        )}

        <h3>Internal replication endpoints</h3>
        <p className="onboarding-field-hint">
          Opt-in, mTLS-only server-to-server endpoints (off by default).
        </p>
        {(
          [
            ["quicInternal", quicInternal, "QUIC internal"] as const,
            ["replicationEndpoint", replication, "gRPC replication"] as const,
          ] satisfies ReadonlyArray<
            readonly [
              "quicInternal" | "replicationEndpoint",
              {
                enabled?: boolean;
                port?: number;
                certFile?: string;
                pkeyFile?: string;
                certChain?: string;
              },
              string,
            ]
          >
        ).map(([key, ep, title]) => (
          <div key={key}>
            <h3>{title}</h3>
            <div className="server-config-grid">
              <BoolField
                id={`sc-${key}-enabled`}
                label="Enabled"
                help="Default off. Requires the cert + key below when on."
                value={ep.enabled}
                onChange={(v) => patchSection(key, { enabled: v })}
                disabled={disabled}
              />
              <NumField
                id={`sc-${key}-port`}
                label="Port"
                placeholder="41340"
                value={ep.port}
                onChange={(v) => patchSection(key, { port: v })}
                disabled={disabled}
                min={1}
                max={65535}
              />
              <TextField
                id={`sc-${key}-cert`}
                label="mTLS cert file"
                placeholder="/path/cert.pem"
                value={ep.certFile}
                onChange={(v) => patchSection(key, { certFile: v })}
                disabled={disabled}
                error={errors[`${key}.certFile`]}
              />
              <TextField
                id={`sc-${key}-key`}
                label="mTLS key file"
                placeholder="/path/key.pem"
                value={ep.pkeyFile}
                onChange={(v) => patchSection(key, { pkeyFile: v })}
                disabled={disabled}
                error={errors[`${key}.pkeyFile`]}
              />
              <TextField
                id={`sc-${key}-chain`}
                label="CA chain (optional)"
                placeholder="/path/ca.pem"
                value={ep.certChain}
                onChange={(v) => patchSection(key, { certChain: v })}
                disabled={disabled}
              />
            </div>
          </div>
        ))}
      </Section>

      {/* Telemetry --------------------------------------------------- */}
      <Section id="sc-telemetry" title="Telemetry" summary={summary(telemetry)}>
        <div className="server-config-grid">
          <SelectField
            id="sc-tel-format"
            label="Log format"
            value={telemetry.logFormat}
            defaultLabel="Default (text)"
            options={[
              { value: "text", label: "text" },
              { value: "ansi", label: "ansi" },
              { value: "json", label: "json" },
            ]}
            onChange={(v) => patchSection("telemetry", { logFormat: v })}
            disabled={disabled}
          />
          <SelectField
            id="sc-tel-output"
            label="Log output"
            value={telemetry.logOutput}
            defaultLabel="Default (stdout)"
            options={[
              { value: "stdout", label: "stdout" },
              { value: "stderr", label: "stderr" },
              { value: "file", label: "file" },
            ]}
            onChange={(v) => patchSection("telemetry", { logOutput: v })}
            disabled={disabled}
          />
          {telemetry.logOutput === "file" && (
            <TextField
              id="sc-tel-file"
              label="Log file path"
              placeholder="/var/log/lore-server.log"
              value={telemetry.logFile}
              onChange={(v) => patchSection("telemetry", { logFile: v })}
              disabled={disabled}
              error={errors["telemetry.logFile"]}
            />
          )}
          <BoolField
            id="sc-tel-otlp"
            label="OTLP export"
            help="Default off."
            value={telemetry.enableOtlp}
            onChange={(v) => patchSection("telemetry", { enableOtlp: v })}
            disabled={disabled}
          />
          <NumField
            id="sc-tel-mexp"
            label="Metrics export (ms)"
            placeholder="30000"
            value={telemetry.metricsExportIntervalMillis}
            onChange={(v) =>
              patchSection("telemetry", { metricsExportIntervalMillis: v })
            }
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-tel-msam"
            label="Metrics sample (ms)"
            placeholder="10000"
            value={telemetry.metricsSampleIntervalMillis}
            onChange={(v) =>
              patchSection("telemetry", { metricsSampleIntervalMillis: v })
            }
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-tel-trace"
            label="Trace sample rate"
            placeholder="0.05"
            help="0.0–1.0."
            value={telemetry.traceSampleRate}
            onChange={(v) => patchSection("telemetry", { traceSampleRate: v })}
            disabled={disabled}
            min={0}
            max={1}
            step={0.01}
            error={errors["telemetry.traceSampleRate"]}
          />
          <NumField
            id="sc-tel-tracelow"
            label="Trace rate (low tier)"
            placeholder="0.001"
            help="0.0–1.0."
            value={telemetry.traceSampleRateLowTier}
            onChange={(v) =>
              patchSection("telemetry", { traceSampleRateLowTier: v })
            }
            disabled={disabled}
            min={0}
            max={1}
            step={0.001}
            error={errors["telemetry.traceSampleRateLowTier"]}
          />
        </div>
      </Section>

      {/* Runtime ----------------------------------------------------- */}
      <Section id="sc-runtime" title="Runtime (Tokio)" summary={summary(runtime)}>
        <div className="server-config-grid">
          <NumField
            id="sc-rt-worker"
            label="Worker threads"
            placeholder="CPU cores"
            value={runtime.workerThreads}
            onChange={(v) => patchSection("runtime", { workerThreads: v })}
            disabled={disabled}
            min={1}
          />
          <NumField
            id="sc-rt-blocking"
            label="Max blocking threads"
            placeholder="512"
            value={runtime.maxBlockingThreads}
            onChange={(v) => patchSection("runtime", { maxBlockingThreads: v })}
            disabled={disabled}
            min={1}
          />
          <NumField
            id="sc-rt-keepalive"
            label="Thread keep-alive (s)"
            placeholder="default"
            value={runtime.threadKeepAliveSeconds}
            onChange={(v) =>
              patchSection("runtime", { threadKeepAliveSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
        </div>
      </Section>

      {/* Notifications ----------------------------------------------- */}
      <Section
        id="sc-notify"
        title="Notifications"
        summary={notification.mode ?? "default"}
      >
        <SelectField
          id="sc-notify-mode"
          label="Notification mode"
          help="Default: local (in-process). Other modes need a matching plugin."
          value={notification.mode}
          defaultLabel="Default (local)"
          options={[{ value: "local", label: "local (in-process)" }]}
          onChange={(v) => patchSection("notification", { mode: v })}
          disabled={disabled}
        />
      </Section>

      {/* Features ---------------------------------------------------- */}
      <Section id="sc-features" title="Features" summary={summary(features)}>
        <div className="server-config-grid">
          <NumField
            id="sc-feat-step"
            label="History step size"
            placeholder="100"
            help="Larger speeds up history lookups but the cached blob must fit the fragment threshold."
            value={features.historyStepSize}
            onChange={(v) => patchSection("features", { historyStepSize: v })}
            disabled={disabled}
            min={1}
          />
          <BoolField
            id="sc-feat-stepkeys"
            label="Revision step keys"
            help="Default on. Skip-pointer acceleration for revision lists."
            value={features.revisionStepKeys}
            onChange={(v) => patchSection("features", { revisionStepKeys: v })}
            disabled={disabled}
          />
          <BoolField
            id="sc-feat-listcache"
            label="Revision list cache"
            help="Default on. Per-segment cache of revision items."
            value={features.revisionListCache}
            onChange={(v) => patchSection("features", { revisionListCache: v })}
            disabled={disabled}
          />
          <NumField
            id="sc-feat-diffcap"
            label="Diff source cap"
            placeholder="100000"
            value={features.revisionDiffSourceCap}
            onChange={(v) =>
              patchSection("features", { revisionDiffSourceCap: v })
            }
            disabled={disabled}
            min={1}
          />
          <NumField
            id="sc-feat-diffwalk"
            label="Diff history-walk concurrency"
            placeholder="24"
            value={features.revisionDiffHistoryWalkConcurrency}
            onChange={(v) =>
              patchSection("features", {
                revisionDiffHistoryWalkConcurrency: v,
              })
            }
            disabled={disabled}
            min={1}
          />
        </div>
      </Section>

      {/* Timeouts ---------------------------------------------------- */}
      <Section id="sc-timeouts" title="Shutdown timeouts" summary={summary(timeouts)}>
        <div className="server-config-grid">
          <NumField
            id="sc-to-conn"
            label="Connection close (s)"
            placeholder="5"
            help="Seconds to wait for connections to drain on shutdown."
            value={timeouts.connectionCloseTimeoutSeconds}
            onChange={(v) =>
              patchSection("timeouts", { connectionCloseTimeoutSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
          <NumField
            id="sc-to-runtime"
            label="Runtime shutdown (s)"
            placeholder="25"
            help="Seconds to wait for the runtime to stop after draining."
            value={timeouts.runtimeShutdownTimeoutSeconds}
            onChange={(v) =>
              patchSection("timeouts", { runtimeShutdownTimeoutSeconds: v })
            }
            disabled={disabled}
            min={0}
          />
        </div>
      </Section>
    </div>
  );
}
