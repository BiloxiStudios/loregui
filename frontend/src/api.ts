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
  repositoryCreate: (path: string, name: string) =>
    invoke<string>("repository_create", { path, name }),
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

// --- branch archive ---

export interface BranchArchiveResult {
  branch: string;
}

export const branchArchiveApi = {
  archive: (branch: string) =>
    invoke<BranchArchiveResult>("branch_archive", { branch }),
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
    invoke<FileInfoResult>("file_info", { paths, revision, local, filtered }),
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
