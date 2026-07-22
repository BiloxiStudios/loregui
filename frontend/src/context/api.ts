import { invoke } from "@tauri-apps/api/core";
import { api, type RepoStatus } from "../api";
import type { ContextSettings } from "./types";

export const contextApi = {
  get: () => invoke<ContextSettings>("context_get"),
  validate: (context: ContextSettings) =>
    invoke<ContextSettings>("context_validate", { context }),
  update: (context: ContextSettings) =>
    invoke<ContextSettings>("context_update", { context }),
  currentRepository: (): Promise<string | null> => api.currentRepository(),
  openRepository: (path: string): Promise<void> => api.openRepository(path),
  status: (): Promise<RepoStatus> => api.status(),
};
