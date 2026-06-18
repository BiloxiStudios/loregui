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

export type FileAction = "keep" | "add" | "delete" | "move" | "copy";

export interface FileDiffEntry {
  path: string;
  patch: string;
  action: FileAction;
}

export interface FileDiffOptions {
  paths?: string[];
  sourceRevision?: string;
  targetRevision?: string;
  diff3?: boolean;
  contextLines?: number;
  ignoreWhitespaceEol?: boolean;
  ignoreWhitespaceInline?: boolean;
}

export const fileDiffApi = {
  diff: (opts: FileDiffOptions = {}) =>
    invoke<FileDiffEntry[]>("file_diff", {
      paths: opts.paths ?? [],
      sourceRevision: opts.sourceRevision ?? "",
      targetRevision: opts.targetRevision ?? "",
      diff3: opts.diff3 ?? false,
      contextLines: opts.contextLines ?? 3,
      ignoreWhitespaceEol: opts.ignoreWhitespaceEol ?? false,
      ignoreWhitespaceInline: opts.ignoreWhitespaceInline ?? false,
    }),
};
