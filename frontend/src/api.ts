// Thin typed wrappers over the Tauri commands exposed by src-tauri.
// These mirror the lore-vm view-model types (serde-serialized).
import { invoke } from "@tauri-apps/api/core";

export type ChangeKind =
  | "added"
  | "modified"
  | "deleted"
  | "renamed"
  | "untracked";

export interface FileChange {
  path: string;
  kind: ChangeKind;
  staged: boolean;
}

export interface RepoStatus {
  repo_id: string;
  branch: string;
  revision: string;
  changes: FileChange[];
  ahead: number;
  behind: number;
}

export interface Branch {
  name: string;
  id: string;
  latest_revision: string;
  is_current: boolean;
}

export interface Revision {
  hash: string;
  message: string;
  author: string;
  timestamp: string;
  parent: string | null;
}

/// Storage backend configuration captured by the server-setup onboarding wizard.
///
/// lore has exactly two real store backends: a `local` filesystem store and an
/// `s3` (S3-compatible object store, lore's `aws` mode). AWS S3, MinIO, Garage,
/// Ceph/RGW, Backblaze B2, etc. are all the *same* `s3` backend differing only
/// by endpoint URL — they are surfaced as non-binding presets, not separate
/// `kind`s. (lore also has advanced composite/replicated immutable stores and a
/// DynamoDB lock/mutable store at scale; those are enterprise modes not exposed
/// by this wizard yet — see `docs/domains/storage.md`.)
export interface StorageBackendConfig {
  kind: "local" | "s3";
  /** local packfiles path (kind === "local") */
  path?: string;
  /** S3-compatible object-storage connection (kind === "s3") */
  endpoint?: string;
  bucket?: string;
  region?: string;
  accessKeyId?: string;
  secretAccessKey?: string;
  /** mutable KV store location (branch pointers / bookkeeping) */
  mutableStore?: string;
}

export interface UserInfo {
  id: string;
  name: string;
}

export interface ServiceStopResult {
  log_messages: string[];
}

export type TrayStatusKind = "clean" | "dirty" | "syncing" | "conflict";

export interface TraySnapshot {
  branch: string;
  dirtyCount: number;
  status: TrayStatusKind;
}

/// Options for hosting a real `loreserver` from the GUI (SBAI-4065 / SBAI-4075).
export interface HostServerOptions {
  /** Store directory to serve — MUST be the host flow's shared-store path. */
  storeDir: string;
  /** QUIC/gRPC port. Defaults to 41337 when omitted. */
  port?: number;
  /** Repository name embedded in the advertised lore:// URL clients clone. */
  repositoryName?: string;
  /** Reserved hook for a future authed mode. Local host flow is no-auth. */
  auth?: boolean;
  /**
   * Bind host for every endpoint. Defaults to `127.0.0.1` (loopback only).
   * Set to `0.0.0.0` to expose the server on the LAN/WAN — a deliberate choice
   * that makes firewalling + real certs the operator's responsibility.
   */
  bindHost?: string;
  /**
   * Optional S3-compatible object-storage backing for the hosted server's
   * immutable store (lore's `aws` mode). When omitted, the server uses a local
   * filesystem store under `storeDir`. The mutable (branch-pointer) store stays
   * local in both cases — lore's `aws` mutable store requires DynamoDB, which
   * this wizard does not provision.
   */
  s3?: HostS3Options;
  /**
   * Expert-mode advanced configuration (SBAI-4075). Every section/field is
   * optional; whatever is left unset falls through to lore's own compiled-in
   * default, so omitting this whole bag reproduces the simple local config.
   */
  advanced?: HostAdvancedOptions;
}

/// QUIC transport tuning (`[server.quic]`). lore default shown in `()`.
export interface HostQuicOptions {
  /** Override the QUIC port (default: the server port). */
  port?: number;
  /** Require client certificates / mTLS (default: false). */
  verifyClientCerts?: boolean;
  /** Idle timeout in milliseconds (default: 30000). */
  idleTimeout?: number;
  /** Keep-alive interval in milliseconds (default: 500). */
  keepAlive?: number;
  /** Max concurrent bidirectional streams per connection (default: 8). */
  maxBidiStreams?: number;
  /** Number of QUIC listener tasks (default: 10). */
  numListeners?: number;
  /** Transport bandwidth cap in bits/second (default: 1073741824 = 1 Gbit/s). */
  transportBitsPerSecond?: number;
  /** Expected round-trip time in milliseconds (default: 100). */
  transportRtt?: number;
  /** Per-request handler timeout in seconds (default: 50). */
  handlerTimeoutSeconds?: number;
  /** Max inflight messages per connection (default: unbounded). */
  connectionMessageLimit?: number;
}

/// gRPC endpoint tuning (`[server.grpc]`).
export interface HostGrpcOptions {
  /** Override the gRPC port (default: the server port). */
  port?: number;
  /** Require client certificates / mTLS (default: true). */
  verifyClientCerts?: boolean;
  /** HTTP/2 keepalive ping interval in seconds (default: unset). */
  http2KeepaliveIntervalSeconds?: number;
  /** HTTP/2 keepalive ping timeout in seconds (default: unset). */
  http2KeepaliveTimeoutSeconds?: number;
  /** Per-request handler timeout in seconds (default: 50). */
  requestHandlerTimeoutSeconds?: number;
}

/// HTTP endpoint tuning (`[server.http]`).
export interface HostHttpOptions {
  /** Override the HTTP port (default: server port + 2). */
  port?: number;
  /** Max upload size in bytes (default: 10485760 = 10 MB). */
  maxFileSize?: number;
  /** Whole-request timeout in seconds (default: 300). */
  requestTimeoutSeconds?: number;
  /** Request-body read timeout in seconds (default: 3600). */
  requestBodyTimeoutSeconds?: number;
  /** Store-availability poll interval in seconds (default: 30). */
  availableIntervalSeconds?: number;
  /** Store-availability check timeout in seconds (default: 5). */
  availableTimeoutSeconds?: number;
  /** Run an active store health check (default: false). */
  storeHealthCheck?: boolean;
}

/// Local filesystem store tuning.
export interface HostLocalStoreOptions {
  /** Flush interval in seconds (default: 10). */
  flushDelaySeconds?: number;
  /** Immutable-store compaction delay (default: unset). */
  compactionDelay?: number;
  /** Immutable-store eviction delay (default: unset). */
  evictionDelay?: number;
  /** Immutable-store max capacity in entries (default: unbounded). */
  maxCapacity?: number;
  /** Immutable-store max on-disk size in bytes (default: unbounded). */
  maxSize?: number;
}

/// A topology peer.
export interface HostPeerOption {
  /** Peer address (host or IP). */
  address: string;
  /** Peer port. */
  port: number;
  /** "SameRegion" (default) or "OtherRegion". */
  locality?: string;
}

/// Topology + replication (`[topology]`). Built-in providers only.
export interface HostTopologyOptions {
  /** "none" (single node, default), "fixed", or "rotating_id_fixed". */
  provider?: string;
  /** Peers for fixed / rotating_id_fixed providers. */
  peers?: HostPeerOption[];
  /** Rotation interval (seconds) — required for rotating_id_fixed. */
  rotationIntervalSeconds?: number;
}

/// Telemetry (`[telemetry]`).
export interface HostTelemetryOptions {
  /** Logger format: "text" (default), "ansi", or "json". */
  logFormat?: string;
  /** Logger output: "stdout" (default), "stderr", or "file". */
  logOutput?: string;
  /** File path when logOutput is "file". */
  logFile?: string;
  /** Emit logs over OTLP (default: false). */
  enableOtlp?: boolean;
  /** Metrics export interval in milliseconds (default: 30000). */
  metricsExportIntervalMillis?: number;
  /** Metrics sample interval in milliseconds (default: 10000). */
  metricsSampleIntervalMillis?: number;
  /** Trace sample rate in [0.0, 1.0] (default: 0.05). */
  traceSampleRate?: number;
  /** Low-tier trace sample rate in [0.0, 1.0] (default: 0.001). */
  traceSampleRateLowTier?: number;
}

/// Tokio runtime (`[tokio]`).
export interface HostRuntimeOptions {
  /** Worker (async) threads (default: number of CPU cores). */
  workerThreads?: number;
  /** Max blocking threads (default: 512). */
  maxBlockingThreads?: number;
  /** Idle blocking-thread keep-alive in seconds. */
  threadKeepAliveSeconds?: number;
}

/// Notification backend (`[notification]`).
export interface HostNotificationOptions {
  /** "local" (default, in-process) or a plugin name. */
  mode?: string;
}

/// Revision/history feature flags (`[feature]`).
export interface HostFeatureOptions {
  /** Revision-history step size (default: 100). */
  historyStepSize?: number;
  /** Persist revision_step_key skip pointers (default: true). */
  revisionStepKeys?: boolean;
  /** Persist the per-segment revision-list cache (default: true). */
  revisionListCache?: boolean;
  /** Max source-side changes for v1 3-way RevisionDiff (default: 100000). */
  revisionDiffSourceCap?: number;
  /** Parallel history-walk permits for diff3 (default: 24). */
  revisionDiffHistoryWalkConcurrency?: number;
}

/// Graceful-shutdown timeouts (`[server]`).
export interface HostTimeoutOptions {
  /** Seconds to wait for connections to drain on shutdown (default: 5). */
  connectionCloseTimeoutSeconds?: number;
  /** Seconds to wait for the runtime to stop after draining (default: 25). */
  runtimeShutdownTimeoutSeconds?: number;
}

/// An opt-in internal endpoint (quic_internal / replication). mTLS required.
export interface HostInternalEndpointOptions {
  /** Enable the endpoint (default: false). */
  enabled?: boolean;
  /** Bind port (default: 41340). */
  port?: number;
  /** mTLS certificate chain file. */
  certChain?: string;
  /** mTLS certificate file (required when enabled). */
  certFile?: string;
  /** mTLS private-key file (required when enabled). */
  pkeyFile?: string;
}

/// The full Expert-mode configuration surface (SBAI-4075). One optional bag of
/// optional sections; anything unset uses lore's compiled-in default.
export interface HostAdvancedOptions {
  quic?: HostQuicOptions;
  grpc?: HostGrpcOptions;
  http?: HostHttpOptions;
  localStore?: HostLocalStoreOptions;
  topology?: HostTopologyOptions;
  telemetry?: HostTelemetryOptions;
  runtime?: HostRuntimeOptions;
  notification?: HostNotificationOptions;
  features?: HostFeatureOptions;
  timeouts?: HostTimeoutOptions;
  quicInternal?: HostInternalEndpointOptions;
  replicationEndpoint?: HostInternalEndpointOptions;
  /** Lock-store mode (`[lock_store]`). Defaults to lore's "local". */
  lockStoreMode?: string;
}

/// S3-compatible object-storage options for a hosted server's immutable store.
export interface HostS3Options {
  /** S3 endpoint URL. Empty/omitted = real AWS S3 (no custom endpoint). */
  endpoint?: string;
  /** Bucket name (required). */
  bucket: string;
  /** Region (e.g. "us-east-1"). Required by most S3 providers. */
  region?: string;
  /** Access key id. Passed to the server as AWS_ACCESS_KEY_ID. */
  accessKeyId?: string;
  /** Secret access key. Passed to the server as AWS_SECRET_ACCESS_KEY. */
  secretAccessKey?: string;
  /**
   * Force path-style addressing (`endpoint/bucket/key` rather than
   * `bucket.endpoint/key`). Required by MinIO/Garage and most non-AWS providers.
   */
  forcePathStyle?: boolean;
  /**
   * Optional DynamoDB-compatible endpoint URL for the immutable store's
   * fragment-association + metadata tables. Omit to use real AWS DynamoDB in the
   * chosen region; set it for DynamoDB Local / LocalStack / ScyllaDB Alternator.
   */
  dynamodbEndpoint?: string;
}

/// Status of the hosted `loreserver` process.
export interface HostStatus {
  running: boolean;
  pid?: number;
  port?: number;
  httpPort?: number;
  /** The `lore://host:port/<repo>` URL clients connect to. */
  url?: string;
  configPath?: string;
  storeDir?: string;
  /**
   * An externally-registered, publicly-reachable URL that supersedes {@link url}
   * for display (SBAI-4072). Set by the proprietary cross-network *relay*
   * overlay via {@link api.hostServerSetAdvertisedUrl}; absent in the open core.
   * When present, the host UI shows this to clients instead of the loopback
   * `url`. The open core knows nothing about how it is produced.
   */
  advertisedUrl?: string;
}

/**
 * A lore server discovered on the LAN via mDNS (SBAI-4073). Mirrors the Rust
 * `lan_discovery::DiscoveredServer` (camelCase via serde). Open-core, not gated.
 */
export interface DiscoveredServer {
  /** Stable id for this browse session (the mDNS service fullname). */
  id: string;
  /** Friendly host label, e.g. "BRAINZ" or "Brian's Mac · world-bible". */
  name: string;
  /** Repository name the host advertised (may be empty). */
  repo: string;
  /** The `lore://host:port/<repo>` URL one-click "Connect" prefills. */
  url: string;
  /** Resolved host (display-only; the connect target is `url`). */
  host: string;
  /** Advertised lore port. */
  port: number;
}

/**
 * Tauri event carrying the live discovered-server list (SBAI-4073). The connect
 * flow `listen`s on this to refresh as servers appear/leave, without polling.
 * Payload: `DiscoveredServer[]`.
 */
export const LAN_DISCOVERED_EVENT = "lan/discovered";

export const api = {
  currentRepository: () => invoke<string>("current_repository"),
  openRepository: (path: string) => invoke<void>("open_repository", { path }),
  status: () => invoke<RepoStatus>("status"),
  log: (limit: number) => invoke<Revision[]>("log", { limit }),
  branches: () => invoke<Branch[]>("branches"),
  stage: (paths: string[]) => invoke<void>("stage", { paths }),
  unstage: (paths: string[]) => invoke<void>("unstage", { paths }),
  commit: (message: string) => invoke<string>("commit", { message }),
  createBranch: (name: string) => invoke<void>("create_branch", { name }),
  switchBranch: (name: string) => invoke<void>("switch_branch", { name }),
  mergeBranch: (name: string) => invoke<void>("merge_branch", { name }),
  push: () => invoke<void>("push"),
  sync: () => invoke<void>("sync"),

  // --- onboarding / deployment (client + server setup) ---
  // NOTE: these map to src-tauri commands wired by the integration manager as the
  // underlying lore-vm ops land. Declared here so onboarding components (SBAI-3841..3848)
  // can build against a stable typed surface.
  authLoginInteractive: (remoteUrl: string) =>
    invoke<UserInfo>("auth_login_interactive", { remoteUrl }),
  authLoginWithToken: (remoteUrl: string, token: string) =>
    invoke<UserInfo>("auth_login_with_token", { remoteUrl, token }),
  authUserInfo: () => invoke<UserInfo | null>("auth_user_info"),
  repositoryClone: (url: string, dest: string) =>
    invoke<void>("repository_clone", { url, dest }),
  // The wizard supplies a filesystem path + a repo name. The underlying
  // `repository_create` op is addressed by a repository URL; we derive a
  // local `lore://localhost/<name>` URL from the name and pass the target
  // path so the command opens the repo there. Returns the created repo id.
  repositoryCreate: (path: string, name: string) =>
    invoke<RepositoryCreateResult>("repository_create", {
      path,
      repositoryUrl: `lore://localhost/${name}`,
      description: "",
      id: "",
      useSharedStore: false,
      sharedStorePath: "",
    }).then((r) => r.id),
  storageOpen: (config: StorageBackendConfig) =>
    invoke<void>("storage_open", { config }),
  storagePut: (key: string, data: number[]) =>
    invoke<void>("storage_put", { key, data }),
  storageGet: (key: string) => invoke<number[]>("storage_get", { key }),
  storageObliterate: (key: string) =>
    invoke<void>("storage_obliterate", { key }),
  sharedStoreCreate: (path: string) =>
    invoke<string>("shared_store_create", { path }),
  serviceStart: (installAutorun: boolean) =>
    invoke<void>("service_start", { installAutorun }),
  serviceStop: (all: boolean = false) =>
    invoke<ServiceStopResult>("service_stop", { all }),

  // --- host a real loreserver (SBAI-4065) ---
  // `serviceStart` maps to an upstream STUB that hosts nothing. These launch and
  // manage the genuine standalone `loreserver` binary over the host flow's
  // local stores. `hostServerStart` returns the lore:// URL to give to clients.
  hostServerStart: (opts: HostServerOptions) =>
    invoke<HostStatus>("host_server_start", {
      storeDir: opts.storeDir,
      port: opts.port ?? null,
      repositoryName: opts.repositoryName ?? null,
      auth: opts.auth ?? false,
      bindHost: opts.bindHost ?? null,
      // S3-compatible immutable store (lore `aws` mode) when a bucket is given;
      // otherwise the server uses a local filesystem store under storeDir.
      s3Bucket: opts.s3?.bucket ?? null,
      s3Endpoint: opts.s3?.endpoint ?? null,
      s3Region: opts.s3?.region ?? null,
      s3AccessKeyId: opts.s3?.accessKeyId ?? null,
      s3SecretAccessKey: opts.s3?.secretAccessKey ?? null,
      s3ForcePathStyle: opts.s3?.forcePathStyle ?? null,
      s3DynamodbEndpoint: opts.s3?.dynamodbEndpoint ?? null,
      // Expert-mode advanced sections (SBAI-4075); omitted = lore defaults.
      advanced: opts.advanced ?? null,
    }),
  /**
   * Render the loreserver config TOML for `opts` WITHOUT writing anything to
   * disk or starting a server (SBAI-4075). Backs the host flow's "View
   * generated config" preview; also surfaces validation errors as a rejection.
   */
  hostServerRenderConfig: (opts: HostServerOptions) =>
    invoke<string>("host_server_render_config", { opts }),
  hostServerStop: () => invoke<HostStatus>("host_server_stop"),
  hostServerStatus: () => invoke<HostStatus>("host_server_status"),

  // --- advertised-URL seam for the cross-network relay overlay (SBAI-4072) ---
  // Generic open-core hooks: a premium relay overlay opens a tunnel to the
  // hosted loreserver and registers the resulting public URL here, which
  // `hostServerStatus` then surfaces as `advertisedUrl`. The open core ships
  // these wrappers but never calls them (no relay UI in core).
  /**
   * Register a publicly-reachable URL for the hosted server (SBAI-4072). A blank
   * string clears it. Surfaced by `hostServerStatus` as `advertisedUrl` while
   * the server is running. The core stores the string verbatim — it has no
   * knowledge of tunnels/bore.
   */
  hostServerSetAdvertisedUrl: (url: string) =>
    invoke<void>("host_server_set_advertised_url", { url }),
  /** Clear any advertised-URL override (SBAI-4072). */
  hostServerClearAdvertisedUrl: () =>
    invoke<void>("host_server_clear_advertised_url"),

  // --- system tray live status (SBAI-4042) ---
  traySyncState: (snapshot: TraySnapshot) =>
    invoke<void>("tray_sync_state", { snapshot }),

  // --- LAN auto-discovery of lore servers (SBAI-4073) ---
  // Open-core, NOT gated: dynamic mDNS discovery, same pattern as
  // studiobrain-model-manager. `lanDiscoverBrowse` starts (or reuses) a live
  // browse and returns what's been seen so far; the `lan/discovered` event keeps
  // the list live. `lanDiscoverRefresh` re-reads the running browse without
  // restarting it; `lanDiscoverStop` tears the browse down on flow exit.
  /** Start/reuse a LAN browse and return the servers seen so far. */
  lanDiscoverBrowse: () =>
    invoke<DiscoveredServer[]>("lan_discover_browse"),
  /** Re-read the running browse's current snapshot (no restart). */
  lanDiscoverRefresh: () =>
    invoke<DiscoveredServer[]>("lan_discover_refresh"),
  /** Stop the LAN browse session (call on connect-flow unmount). */
  lanDiscoverStop: () => invoke<void>("lan_discover_stop"),
};

// --- repository create (ops-layer) ---

export interface RepositoryCreateResult {
  id: string;
  name: string;
  path: string;
}

export const repositoryCreateApi = {
  create: (
    repositoryUrl: string,
    description: string = "",
    id: string = "",
    useSharedStore: boolean = false,
    sharedStorePath: string = "",
  ) =>
    invoke<RepositoryCreateResult>("repository_create", {
      repositoryUrl,
      description,
      id,
      useSharedStore,
      sharedStorePath,
    }),
};

// --- repository dump ---

export interface DumpStateSummary {
  revision_number: number;
  revision: string;
  tree_hash: string;
  tree_size: number;
}

export interface DumpNode {
  name: string;
  id: number;
  parent: number;
  sibling: number;
  mode: number;
  size: number;
  flags: number;
  type_data: string;
}

export interface RepositoryDumpResult {
  repository: string;
  begin_revision: string;
  state: DumpStateSummary | null;
  nodes: DumpNode[];
  log_messages: string[];
}

export const repositoryDumpApi = {
  dump: (
    revision: string = "",
    path: string = "",
    maxDepth: number = 0,
  ) =>
    invoke<RepositoryDumpResult>("repository_dump", {
      revision,
      path,
      maxDepth,
    }),
};

// --- repository delete ---

export interface DeleteResult {
  log_messages: string[];
}

export const repositoryDeleteApi = {
  delete: (repositoryUrl: string) =>
    invoke<DeleteResult>("repository_delete", { repositoryUrl }),
};

// --- repository list ---

export interface RepositoryEntry {
  id: string;
  name: string;
}

export interface RepositoryListResult {
  url: string;
  entries: RepositoryEntry[];
}

export const repositoryListApi = {
  list: (url: string) =>
    invoke<RepositoryListResult>("repository_list", { url }),
};

// --- repository instance_list ---

export interface InstanceEntry {
  instance_id: string;
  path: string;
  branch_name: string;
  branch: string;
  revision: string;
  stale: boolean;
}

export interface InstanceListResult {
  instance_count: number;
  instances: InstanceEntry[];
}

export const repositoryInstanceListApi = {
  instanceList: () =>
    invoke<InstanceListResult>("repository_instance_list"),
};

// --- repository flush ---

export interface FlushResult {
  log_messages: string[];
}

export const repositoryFlushApi = {
  flush: () => invoke<FlushResult>("repository_flush"),
};

// --- repository gc ---

export interface GcResult {
  log_messages: string[];
}

export const repositoryGcApi = {
  gc: () => invoke<GcResult>("repository_gc"),
};

// --- repository instance_prune ---

export interface PrunedInstance {
  instance_id: string;
  path: string;
  branch_name: string;
}

export interface InstancePruneResult {
  pruned_count: number;
  pruned: PrunedInstance[];
}

export const repositoryInstancePruneApi = {
  instancePrune: () =>
    invoke<InstancePruneResult>("repository_instance_prune"),
};

// --- repository verify_state ---

export interface VerifiedFragment {
  hash: string;
  match_count: number;
  error: string;
}

export interface VerifiedRemoteFragment {
  address_hash: string;
  corrupted: boolean;
  healed: boolean;
  error: string;
}

export interface VerifyStateResult {
  healed_staged_state: string;
  fragments: VerifiedFragment[];
  remote_fragments: VerifiedRemoteFragment[];
  error_count: number;
  corrupted_count: number;
}

export const repositoryVerifyStateApi = {
  verifyState: (path: string = "", heal: boolean = false) =>
    invoke<VerifyStateResult>("repository_verify_state", { path, heal }),
};

export interface BranchInfoResult {
  id: string;
  name: string;
  category: string;
  latest: string;
  latest_remote: string;
  parent: string;
  branch_point: string;
  creator: string;
  created: number;
  archived: boolean;
}

export const branchInfoApi = {
  info: (branch: string) =>
    invoke<BranchInfoResult>("branch_info", { branch }),
};

export interface BranchProtectResult {
  branch: string;
}

export const branchProtectApi = {
  protect: (branch: string) =>
    invoke<BranchProtectResult>("branch_protect", { branch }),
};

// --- branch unprotect ---

export interface BranchUnprotectResult {
  branch: string;
}

export const branchUnprotectApi = {
  unprotect: (branch: string) =>
    invoke<BranchUnprotectResult>("branch_unprotect", { branch }),
};

// --- branch archive ---

export interface BranchArchiveResult {
  branch: string;
}

export const branchArchiveApi = {
  archive: (branch: string) =>
    invoke<BranchArchiveResult>("branch_archive", { branch }),
};

// --- branch metadata_get ---

export interface BranchMetadataEntry {
  key: string;
  value: string;
  value_type: string;
}

export interface BranchMetadataGetResult {
  branch: string;
  entries: BranchMetadataEntry[];
}

export const branchMetadataGetApi = {
  metadataGet: (branch: string = "", key: string = "") =>
    invoke<BranchMetadataGetResult>("branch_metadata_get", { branch, key }),
};

// --- branch merge_abort ---

export interface BranchMergeAbortResult {
  staged_revision: string;
  current_revision: string;
}

export const branchMergeAbortApi = {
  mergeAbort: (link: string = "", ignoreLinks: boolean = false) =>
    invoke<BranchMergeAbortResult>("branch_merge_abort", { link, ignoreLinks }),
};

// --- branch merge_unresolve ---

export interface BranchMergeUnresolveResult {
  unresolved_paths: string[];
}

export const branchMergeUnresolveApi = {
  mergeUnresolve: (paths: string[] = []) =>
    invoke<BranchMergeUnresolveResult>("branch_merge_unresolve", { paths }),
};

// --- branch merge_into ---

export interface BranchMergeIntoResult {
  revision: string;
  revision_number: number;
}

export const branchMergeIntoApi = {
  mergeInto: (
    branch: string,
    message: string = "",
    branchId: string = "",
    link: string = "",
    ignoreLinks: boolean = false,
  ) =>
    invoke<BranchMergeIntoResult>("branch_merge_into", {
      branch,
      branchId,
      message,
      link,
      ignoreLinks,
    }),
};

// --- file stage ---

export type FileStageAction = "keep" | "add" | "delete" | "move" | "copy";
export type CaseChange = "error" | "keep" | "rename";

export interface FileStageEntry {
  path: string;
  from_path: string;
  action: FileStageAction;
}

export interface FileStageResult {
  files: FileStageEntry[];
  revision: string;
}

export const fileStageApi = {
  stage: (
    paths: string[],
    caseChange?: CaseChange,
    scan?: boolean,
  ) =>
    invoke<FileStageResult>("file_stage", { paths, caseChange, scan }),
};

// --- file dirty ---

export interface FileDirtyResult {
  paths: string[];
}

export const fileDirtyApi = {
  dirty: (paths: string[]) =>
    invoke<FileDirtyResult>("file_dirty", { paths }),
};

// --- file dirty_copy ---

export interface FileDirtyCopyResult {
  from_path: string;
  to_path: string;
}

export const fileDirtyCopyApi = {
  dirtyCopy: (fromPath: string, toPath: string) =>
    invoke<FileDirtyCopyResult>("file_dirty_copy", { fromPath, toPath }),
};

// --- file dirty_move ---

export interface FileDirtyMoveResult {
  from_path: string;
  to_path: string;
}

export const fileDirtyMoveApi = {
  dirtyMove: (fromPath: string, toPath: string) =>
    invoke<FileDirtyMoveResult>("file_dirty_move", { fromPath, toPath }),
};

// --- file reset_to_last_merged ---

export interface FileResetToLastMergedEntry {
  path: string;
  action: string;
  from_path: string;
}

export interface FileResetToLastMergedCounts {
  directory_reset_count: number;
  directory_delete_count: number;
  file_reset_count: number;
  file_delete_count: number;
}

export interface FileResetToLastMergedResult {
  files: FileResetToLastMergedEntry[];
  counts: FileResetToLastMergedCounts;
}

export const fileResetToLastMergedApi = {
  resetToLastMerged: (paths: string[], branch: string, purge: boolean) =>
    invoke<FileResetToLastMergedResult>("file_reset_to_last_merged", {
      paths,
      branch,
      purge,
    }),
};

// --- file obliterate ---

export interface FileObliterateEntry {
  address: string;
  num_fragments: number;
  num_payloads: number;
}

export interface FileObliterateResult {
  obliterated: FileObliterateEntry[];
}

export const fileObliterateApi = {
  obliterate: (path: string = "", address: string = "") =>
    invoke<FileObliterateResult>("file_obliterate", { path, address }),
};

// --- file write ---

export interface FileWriteResult {
  path: string;
}

export const fileWriteApi = {
  write: (
    output: string,
    path: string = "",
    revision: string = "",
    address: string = "",
  ) =>
    invoke<FileWriteResult>("file_write", {
      path,
      revision,
      output,
      address,
    }),
};

// --- file info ---

export interface FileInfoEntry {
  path: string;
  context: string;
  hash: string;
  is_file: boolean;
  is_dir: boolean;
  flag_modified: boolean;
  flag_deleted: boolean;
  flag_added: boolean;
  flag_conflict: boolean;
  mode: number;
  size: number;
  local_size: number;
  local_hash: string;
  filter_size: number;
}

export interface FileInfoResult {
  entries: FileInfoEntry[];
}

export const fileInfoApi = {
  info: (
    paths: string[],
    revision: string = "",
    local: boolean = false,
    filtered: boolean = false,
  ) =>
    invoke<FileInfoResult>("file_info", {
      paths,
      revision,
      local,
      filtered,
    }),
};

// --- file dump ---

export interface FileDumpEntry {
  address: string;
  flags: number;
  size_payload: number;
  size_content: number;
  match_made: boolean;
}

export interface FileDumpResult {
  entries: FileDumpEntry[];
}

export const fileDumpApi = {
  dump: (address: string = "", path: string = "") =>
    invoke<FileDumpResult>("file_dump", { address, path }),
};

// --- file diff ---

export type FileDiffAction = "keep" | "add" | "delete" | "move" | "copy";

export interface FileDiffEntry {
  path: string;
  patch: string;
  action: FileDiffAction;
}

export const fileDiffApi = {
  diff: (
    paths: string[] = [],
    sourceRevision: string = "",
    targetRevision: string = "",
    diff3: boolean = false,
    contextLines: number = 3,
    ignoreWhitespaceEol: boolean = false,
    ignoreWhitespaceInline: boolean = false,
  ) =>
    invoke<FileDiffEntry[]>("file_diff", {
      paths,
      sourceRevision,
      targetRevision,
      diff3,
      contextLines,
      ignoreWhitespaceEol,
      ignoreWhitespaceInline,
    }),
};

// --- repository metadata_get ---

export interface MetadataEntry {
  key: string;
  value: string;
  value_type: string;
}

export interface RepositoryMetadataGetResult {
  entries: MetadataEntry[];
}

export const repositoryMetadataGetApi = {
  metadataGet: (key: string = "") =>
    invoke<RepositoryMetadataGetResult>("repository_metadata_get", { key }),
};

// --- repository metadata_set ---

export type MetadataFormat = "binary" | "numeric" | "string";

export interface RepositoryMetadataSetResult {
  keys: string[];
  values: string[];
}

export const repositoryMetadataSetApi = {
  metadataSet: (
    keys: string[],
    values: string[],
    formats: MetadataFormat[] = [],
  ) =>
    invoke<RepositoryMetadataSetResult>("repository_metadata_set", {
      keys,
      values,
      formats,
    }),
};

// --- revision diff ---

export type DiffFileAction = "keep" | "add" | "delete" | "move" | "copy";

export interface RevisionDiffFile {
  path: string;
  action: DiffFileAction;
  action_short: string;
  old_is_file: boolean;
  new_is_file: boolean;
  old_address: string;
  new_address: string;
}

export interface RevisionDiffResult {
  files: RevisionDiffFile[];
}

export const revisionDiffApi = {
  diff: (
    revisionSource: string,
    revisionTarget: string = "",
    paths: string[] = [],
  ) =>
    invoke<RevisionDiffResult>("revision_diff", {
      revisionSource,
      revisionTarget,
      paths,
    }),
};

// --- revision find ---

export interface RevisionFindEntry {
  signature: string;
}

export interface RevisionFindResult {
  revisions: RevisionFindEntry[];
}

export const revisionFindApi = {
  find: (
    key: string = "",
    value: string = "",
    number: number = 0,
  ) =>
    invoke<RevisionFindResult>("revision_find", {
      key,
      value,
      number,
    }),
};

// --- revision find_local ---

export interface RevisionFound {
  signature: string;
}

export interface RevisionFindLocalResult {
  revisions: RevisionFound[];
}

export const revisionFindLocalApi = {
  findLocal: (
    key: string = "",
    value: string = "",
    number: number = 0,
  ) =>
    invoke<RevisionFindLocalResult>("revision_find_local", {
      key,
      value,
      number,
    }),
};

// --- revision history ---

export interface RevisionHistoryEntry {
  revision: string;
  revision_number: number;
  parents: string[];
}

export interface RevisionHistoryResult {
  entries: RevisionHistoryEntry[];
}

export const revisionHistoryApi = {
  history: (
    revision: string = "",
    branch: string = "",
    date: number = 0,
    length: number = 0,
    onlyBranch: boolean = false,
  ) =>
    invoke<RevisionHistoryResult>("revision_history", {
      revision,
      branch,
      date,
      length,
      onlyBranch,
    }),
};

// --- revision info ---

export interface RevisionInfoData {
  repository: string;
  revision: string;
  revision_number: number;
  parents: string[];
}

export interface RevisionInfoDelta {
  path: string;
  size: number;
  action: string;
  flag_modify: boolean;
  flag_merged: boolean;
  flag_file: boolean;
}

export interface RevisionMetadataEntry {
  key: string;
  value: string;
}

export interface RevisionInfoResult {
  info: RevisionInfoData | null;
  deltas: RevisionInfoDelta[];
  metadata: RevisionMetadataEntry[];
}

export const revisionInfoApi = {
  info: (
    revision: string = "",
    delta: boolean = false,
    metadata: boolean = false,
  ) =>
    invoke<RevisionInfoResult>("revision_info", {
      revision,
      delta,
      metadata,
    }),
};

// --- revision amend ---

export interface AmendResult {
  revision: string;
  revision_number: number;
  branch: string;
}

export const revisionAmendApi = {
  amend: (message: string) =>
    invoke<AmendResult>("revision_amend", { message }),
};

// --- revision commit (ops-layer) ---

export interface RevisionCommitResult {
  revision: string;
  revision_number: number;
  branch: string;
}

export const revisionCommitApi = {
  commit: (message: string) =>
    invoke<RevisionCommitResult>("revision_commit", { message }),
};

// --- revision revert_local ---

export interface RevertConflictFile {
  path: string;
}

export interface RevertLocalResult {
  has_conflicts: boolean;
  conflict_files: RevertConflictFile[];
  committed_revision: string | null;
}

export const revisionRevertLocalApi = {
  revertLocal: (
    revision: string,
    message: string = "",
    noCommit: boolean = false,
  ) =>
    invoke<RevertLocalResult>("revision_revert_local", {
      revision,
      message,
      noCommit,
    }),
};

// --- revision sync ---

export interface SyncFileEntry {
  path: string;
  size: number;
  action: string;
  is_file: boolean;
}

export interface SyncRevisionInfo {
  branch: string;
  revision: string;
  revision_number: number;
  is_merge: boolean;
  has_conflicts: boolean;
}

export interface RevisionSyncResult {
  files: SyncFileEntry[];
  revisions: SyncRevisionInfo[];
  files_updated: number;
  files_deleted: number;
}

export const revisionSyncApi = {
  sync: (
    revision: string = "",
    forwardChanges: boolean = false,
    reset: boolean = false,
    rootFiles: string[] = [],
    dependencyTags: string[] = [],
    dependencyRecursive: boolean = false,
    dependencyDepthLimit: number = 0,
  ) =>
    invoke<RevisionSyncResult>("revision_sync", {
      revision,
      forwardChanges,
      reset,
      rootFiles,
      dependencyTags,
      dependencyRecursive,
      dependencyDepthLimit,
    }),
};

// --- revision revert_resolve ---

export interface RevertResolveResult {
  paths: string[];
}

export const revisionRevertResolveApi = {
  revertResolve: (paths: string[]) =>
    invoke<RevertResolveResult>("revision_revert_resolve", { paths }),
};

// --- dependency add ---

export interface DependencyAddEntry {
  dependency: string;
  tags?: string[];
}

export interface DependencyAddSource {
  path: string;
  dependencies: DependencyAddEntry[];
}

export interface DependencyAddResult {
  added_count: number;
}

export const dependencyAddApi = {
  add: (sources: DependencyAddSource[], force: boolean = false) =>
    invoke<DependencyAddResult>("dependency_add", { sources, force }),
};

// --- dependency list ---

export interface DependencyEntry {
  path: string;
  tags: string[];
  depth: number;
}

export interface FileDependencies {
  path: string;
  entries: DependencyEntry[];
}

export interface DependencyListResult {
  file_count: number;
  files: FileDependencies[];
  total_entry_count: number;
}

export const dependencyListApi = {
  list: (
    paths: string[],
    revision: string = "",
    recursive: boolean = false,
    reverse: boolean = false,
    tags: string[] = [],
    depthLimit: number = 0,
  ) =>
    invoke<DependencyListResult>("dependency_list", {
      paths,
      revision,
      recursive,
      reverse,
      tags,
      depthLimit,
    }),
};

// --- dependency remove ---

export interface DependencyRemoveEntry {
  dependency: string;
  tags?: string[];
}

export interface DependencyRemoveSource {
  path: string;
  dependencies: DependencyRemoveEntry[];
}

export interface DependencyRemoveResult {
  removed_count: number;
}

export const dependencyRemoveApi = {
  remove: (sources: DependencyRemoveSource[]) =>
    invoke<DependencyRemoveResult>("dependency_remove", { sources }),
};

// --- link add ---

export interface LinkAddResult {
  link_path: string;
}

export const linkAddApi = {
  add: (
    link: string,
    linkPath: string,
    sourcePath: string = "/",
    pin: string = "",
    disableBranching: boolean = false,
  ) =>
    invoke<LinkAddResult>("link_add", {
      link,
      linkPath,
      sourcePath,
      pin,
      disableBranching,
    }),
};

// --- link remove ---

export interface LinkRemoveResult {
  link_path: string;
}

export const linkRemoveApi = {
  remove: (linkPath: string) =>
    invoke<LinkRemoveResult>("link_remove", { linkPath }),
};

// --- lock file_release ---

export interface FileReleaseResult {
  released: string[];
  not_found: boolean;
}

export const lockFileReleaseApi = {
  fileRelease: (
    paths: string[],
    branch: string,
    owner: string,
    ownerId: string,
  ) =>
    invoke<FileReleaseResult>("lock_file_release", {
      paths,
      branch,
      owner,
      ownerId,
    }),
};

// --- lock file_acquire_as_owner ---

export interface FileAcquireAsOwnerResult {
  acquired: string[];
  ignored: string[];
}

export const lockFileAcquireAsOwnerApi = {
  fileAcquireAsOwner: (paths: string[], branch: string, owner: string) =>
    invoke<FileAcquireAsOwnerResult>("lock_file_acquire_as_owner", {
      paths,
      branch,
      owner,
    }),
};

// --- lock file_query ---

export interface LockEntry {
  branch: string;
  path: string;
  owner: string;
  locked_at: number;
}

export interface FileQueryResult {
  count: number;
  locks: LockEntry[];
}

export const lockFileQueryApi = {
  fileQuery: (branch: string, owner: string, path: string) =>
    invoke<FileQueryResult>("lock_file_query", { branch, owner, path }),
};

// --- lock file_acquire ---

export interface FileAcquireResult {
  acquired: string[];
  ignored: string[];
}

export const lockFileAcquireApi = {
  fileAcquire: (paths: string[], branch: string) =>
    invoke<FileAcquireResult>("lock_file_acquire", { paths, branch }),
};

// --- lock file_status ---

export interface LockStatus {
  path: string;
  owner: string;
  locked_at: number;
}

export interface FileStatusResult {
  locks: LockStatus[];
}

export const lockFileStatusApi = {
  fileStatus: (paths: string[], branch: string) =>
    invoke<FileStatusResult>("lock_file_status", { paths, branch }),
};

// --- lock messaging: request check-in from a holder (SBAI-4044) ---
//
// Transport note (see docs/lock-messaging-spike.md): lore's notification channel
// can't carry arbitrary user→user messages in-band, so the core build delivers
// requests **locally** (same process / single machine). Cross-network delivery
// is the premium relay (SBAI-4072). The shape here is what the relay would carry.

/** Tauri event emitted when a check-in request lands in this client's inbox. */
export const LOCK_REQUEST_EVENT = "lock/request";

/** An incoming "please check in <file>" request. */
export interface LockRequest {
  /** Stable id for inbox dedupe / dismissal. */
  id: string;
  /** File the lock applies to. */
  path: string;
  /** Branch the lock is on (empty = current). */
  branch: string;
  /** Display name of the requester (who is asking). */
  from: string;
  /** User id of the holder this request is addressed to. */
  toUserId: string;
  /** Display name of the holder. */
  holder: string;
  /** Optional free-text note from the requester. */
  note: string;
  /** Unix epoch millis when the request was created. */
  createdAt: number;
}

export const lockMessagingApi = {
  /** Ask the holder of a file's lock to check it in / release it. */
  requestCheckin: (args: {
    path: string;
    branch: string;
    from: string;
    toUserId: string;
    holder: string;
    note?: string;
  }) =>
    invoke<LockRequest>("lock_request_checkin", {
      path: args.path,
      branch: args.branch,
      from: args.from,
      toUserId: args.toUserId,
      holder: args.holder,
      note: args.note ?? "",
    }),
  /** Read the incoming check-in request inbox. */
  inboxList: () => invoke<LockRequest[]>("lock_inbox_list"),
  /** Remove a request from the inbox (Dismiss, or after Release). */
  inboxDismiss: (id: string) => invoke<boolean>("lock_inbox_dismiss", { id }),
};

// --- branch reset ---

export interface BranchResetResult {
  branch: string;
  revision: string;
}

export const branchResetApi = {
  reset: (revision: string, branch: string = "") =>
    invoke<BranchResetResult>("branch_reset", { revision, branch }),
};

// --- branch merge_start ---

export interface BranchMergeStartResult {
  source_branch: string;
  source_revision: string;
  source_revision_number: number;
  has_conflicts: boolean;
  conflict_files: string[];
  merge_revision: string;
}

export const branchMergeStartApi = {
  mergeStart: (
    branch: string,
    message: string = "",
    noCommit: boolean = false,
    link: string = "",
    ignoreLinks: boolean = false,
  ) =>
    invoke<BranchMergeStartResult>("branch_merge_start", {
      branch,
      message,
      noCommit,
      link,
      ignoreLinks,
    }),
};

// --- branch merge_restart ---

export interface MergeRestartSyncedFile {
  path: string;
  size: number;
  action: string;
  is_file: boolean;
}

export interface BranchMergeRestartResult {
  conflict_files: string[];
  synced_files: MergeRestartSyncedFile[];
}

export const branchMergeRestartApi = {
  mergeRestart: (paths: string[] = []) =>
    invoke<BranchMergeRestartResult>("branch_merge_restart", { paths }),
};

// --- branch merge_resolve_theirs ---

export interface BranchMergeResolveTheirsResult {
  resolved_paths: string[];
  revision: string;
}

export const branchMergeResolveTheirsApi = {
  mergeResolveTheirs: (paths: string[] = []) =>
    invoke<BranchMergeResolveTheirsResult>("branch_merge_resolve_theirs", { paths }),
};

// --- branch merge_resolve_mine ---

export interface BranchMergeResolveMineResult {
  resolved_paths: string[];
  revision: string;
}

export const branchMergeResolveMineApi = {
  mergeResolveMine: (paths: string[] = []) =>
    invoke<BranchMergeResolveMineResult>("branch_merge_resolve_mine", { paths }),
};

// --- branch merge_resolve ---

export interface BranchMergeResolveResult {
  resolved_paths: string[];
  revision: string;
}

export const branchMergeResolveApi = {
  mergeResolve: (paths: string[] = []) =>
    invoke<BranchMergeResolveResult>("branch_merge_resolve", { paths }),
};

// --- branch latest_list ---

export interface BranchLatestListEntry {
  branch: string;
  revision: string;
}

export interface BranchLatestListResult {
  entries: BranchLatestListEntry[];
}

export const branchLatestListApi = {
  latestList: (branch: string = "", limit: number = 0) =>
    invoke<BranchLatestListResult>("branch_latest_list", { branch, limit }),
};

// --- branch create (ops-layer) ---

export interface BranchCreateResult {
  name: string;
  latest: string;
  is_commit: boolean;
}

export const branchCreateApi = {
  create: (
    branch: string,
    category: string = "",
    id: string = "",
  ) =>
    invoke<BranchCreateResult>("branch_create", { branch, category, id }),
};

// --- branch list ---

export interface BranchPointEntry {
  branch: string;
  revision: string;
}

export interface BranchListEntry {
  location: string;
  id: string;
  name: string;
  category: string;
  latest: string;
  stack: BranchPointEntry[];
  creator: string;
  created: number;
  is_current: boolean;
  archived: boolean;
}

export interface BranchListResult {
  entries: BranchListEntry[];
  count: number;
}

export const branchListApi = {
  list: (archived: boolean = false) =>
    invoke<BranchListResult>("branch_list", { archived }),
};

// --- auth local_user_info ---

export interface LocalUserInfo {
  user_id: string;
  display_name: string;
}

export interface LocalUserTokenInfo {
  user_id: string;
  display_name: string;
  token: string;
  preferred_username: string;
  is_service_account: boolean;
  expires: number;
}

export interface LocalUserInfoResult {
  users: LocalUserInfo[];
  tokens: LocalUserTokenInfo[];
}

export const authLocalUserInfoApi = {
  localUserInfo: (
    authEndpoint: string = "",
    userIds: string[] = [],
    withToken: boolean = false,
  ) =>
    invoke<LocalUserInfoResult>("auth_local_user_info", {
      authEndpoint,
      userIds,
      withToken,
    }),
};

// --- storage: full content-addressed ops (flat, palette/panel-friendly) ---
//
// These wrap the *full* lore-vm storage ops (handle + partition + address),
// distinct from the onboarding key-based `api.storage*` helpers above. The
// Storage panel and the command palette both drive these.

export interface StorageCloseResult {
  log_messages: string[];
}

export interface StorageFlushResult {
  log_messages: string[];
}

export interface FragmentMetadata {
  flags: number;
  size_payload: number;
  size_content: number;
}

export interface StorageGetMetadataItemResult {
  id: number;
  address: string;
  fragment?: FragmentMetadata;
  ok: boolean;
  error?: string;
}

export interface StorageGetMetadataResult {
  items: StorageGetMetadataItemResult[];
}

export interface StoragePutFileItemResult {
  id: number;
  address: string;
  ok: boolean;
  error: string;
}

export interface StoragePutFileResult {
  items: StoragePutFileItemResult[];
}

export interface StorageCopyItemResult {
  id: number;
  source_partition: string;
  target_partition: string;
  source_address: string;
  target_context: string;
  ok: boolean;
  error: string;
}

export interface StorageCopyResult {
  items: StorageCopyItemResult[];
}

export interface StorageUploadItemResult {
  id: number;
  address: string;
  already_durable: boolean;
  ok: boolean;
  error: string;
}

export interface StorageUploadResult {
  items: StorageUploadItemResult[];
}

export const storageApi = {
  /** Open a store and return its handle id (also recorded in the session). */
  open: (repositoryPath = "", remoteUrl = "", inMemory = false) =>
    invoke<number>("storage_open_handle", {
      repositoryPath,
      remoteUrl,
      inMemory,
    }),
  close: (handle: number) =>
    invoke<StorageCloseResult>("storage_close", { handle }),
  flush: (handle: number) =>
    invoke<StorageFlushResult>("storage_flush", { handle }),
  getMetadata: (handle: number, partition: string, address: string) =>
    invoke<StorageGetMetadataResult>("storage_get_metadata", {
      handle,
      partition,
      address,
    }),
  putFile: (
    handle: number,
    partition: string,
    path: string,
    context = "",
    remoteWrite = false,
    localCache = false,
  ) =>
    invoke<StoragePutFileResult>("storage_put_file", {
      handle,
      partition,
      path,
      context,
      remoteWrite,
      localCache,
    }),
  copy: (
    handle: number,
    sourcePartition: string,
    targetPartition: string,
    sourceAddress: string,
    targetContext = "",
  ) =>
    invoke<StorageCopyResult>("storage_copy", {
      handle,
      sourcePartition,
      targetPartition,
      sourceAddress,
      targetContext,
    }),
  upload: (handle: number, partition: string, address: string) =>
    invoke<StorageUploadResult>("storage_upload", { handle, partition, address }),
};

// --- revision cherry_pick_restart ---

export interface CherryPickRestartResult {
  paths: string[];
}

export const revisionCherryPickRestartApi = {
  cherryPickRestart: (paths: string[]) =>
    invoke<CherryPickRestartResult>("revision_cherry_pick_restart", { paths }),
};

// --- shared_store: info + auto-use ---

export interface SharedStoreEntry {
  remote_url: string;
  path: string;
  exists: boolean;
}

export interface SharedStoreInfoResult {
  use_automatically: boolean;
  stores: SharedStoreEntry[];
}

export const sharedStoreApi = {
  info: () => invoke<SharedStoreInfoResult>("shared_store_info"),
  create: (path: string) => invoke<string>("shared_store_create", { path }),
  setUseAutomatically: (enabled: boolean) =>
    invoke<void>("shared_store_set_use_automatically", { enabled }),
};

// --- layer add (SBAI-4038) ---

export interface LayerAddResult {
  target_path: string;
  source_repository: string;
  source_path: string;
  metadata: string;
  revision: string;
}

export const layerAddApi = {
  add: (
    targetPath: string,
    sourceRepository: string,
    sourcePath: string = "",
    metadata: string = "",
  ) =>
    invoke<LayerAddResult>("layer_add", {
      targetPath,
      sourceRepository,
      sourcePath,
      metadata,
    }),
};

// --- revision activity_report (SBAI-4061; commercial Reporting add-on, SBAI-4068) ---
//
// "Who did what when": an aggregated rollup over a revision chain — per-author
// commit counts, files changed, and the revision timeline. This is just the
// typed transport; entitlement gating happens in the UI layer (the Reporting
// panel is hidden/locked unless `isEntitled("reporting")`). See
// frontend/src/commercial/entitlement.ts and docs/COMMERCIAL-ADDONS.md.

export interface ActivityFileChange {
  path: string;
  size: number;
  action: string;
}

export interface ActivityEntry {
  revision: string;
  revision_number: number;
  parents: string[];
  message: string;
  author: string;
  /** Commit Unix timestamp (seconds since epoch). */
  timestamp: number;
  files_changed: ActivityFileChange[];
}

export interface ActivityReportResult {
  /** Entries, newest first. */
  entries: ActivityEntry[];
  /** Total revisions walked before filtering. */
  total_walked: number;
  /** Entries remaining after filters. */
  total_after_filter: number;
}

export const revisionActivityReportApi = {
  report: (
    revision: string = "",
    branch: string = "",
    length: number = 0,
    author: string = "",
    dateFrom: number = 0,
    dateTo: number = 0,
    filePath: string = "",
  ) =>
    invoke<ActivityReportResult>("revision_activity_report", {
      revision,
      branch,
      length,
      author,
      dateFrom,
      dateTo,
      filePath,
    }),
};

// --- desktop settings (autostart + close-to-tray, SBAI-4043) ---

export interface DesktopSettingsResult {
  autostart_enabled: boolean;
  close_to_tray: boolean;
}

export const desktopSettingsApi = {
  get: () => invoke<DesktopSettingsResult>("get_desktop_settings"),
  setAutostart: (enabled: boolean) =>
    invoke<void>("set_autostart", { enabled }),
  setCloseToTray: (enabled: boolean) =>
    invoke<void>("set_close_to_tray", { enabled }),
};

// --- working-tree file I/O (content workspace: Preview / Diff / Edit) ---
//
// Plain filesystem helpers scoped to the open repo's working tree (SBAI-4083/
// 4084/4085). Distinct from `fileWriteApi` (which materialises lore *content*
// from a revision/address); these read & write the bytes on disk so the content
// workspace can preview media, show working-tree diffs, and save edits.

export interface ReadTextFileResult {
  path: string;
  content: string;
  size: number;
  too_large: boolean;
}

export interface ReadFileBytesResult {
  path: string;
  /** Standard base64 of the file bytes; empty when too_large. */
  base64: string;
  size: number;
  /** Best-effort MIME type derived from the extension. */
  mime: string;
  too_large: boolean;
}

export interface WorkingFileMeta {
  path: string;
  size: number;
  is_dir: boolean;
  exists: boolean;
}

export const workingFileApi = {
  /** Read a working-tree file as UTF-8 text (lossy). */
  readText: (path: string) =>
    invoke<ReadTextFileResult>("read_text_file", { path }),
  /** Read a working-tree file as base64 bytes (+ derived MIME). */
  readBytes: (path: string) =>
    invoke<ReadFileBytesResult>("read_file_bytes", { path }),
  /** Write UTF-8 content to a working-tree file (the editor's Save). */
  writeText: (path: string, content: string) =>
    invoke<WorkingFileMeta>("write_text_file", { path, content }),
};
