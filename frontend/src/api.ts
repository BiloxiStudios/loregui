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
};
