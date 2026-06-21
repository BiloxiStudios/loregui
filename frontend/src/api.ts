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
export interface StorageBackendConfig {
  kind: "local" | "s3" | "minio" | "garage";
  /** local packfiles path (kind === "local") */
  path?: string;
  /** object-storage connection (kind !== "local") */
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
