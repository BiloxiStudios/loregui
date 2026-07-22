import type { RepoStatus } from "../api";

export type AuthMode = "not_required" | "required" | "unknown";
export type ServerSource = "manual" | "lan" | "hosted" | "studio_brain";

export interface ServerProfile {
  id: string;
  alias: string;
  url: string;
  source: ServerSource;
  favorite: boolean;
  auth_mode: AuthMode;
  credential_ref: string | null;
  last_seen_at: string | null;
}

export interface RepositoryBookmark {
  id: string;
  server_id: string | null;
  display_name: string;
  url: string | null;
  favorite: boolean;
}

export interface LocalProject {
  id: string;
  repository_id: string;
  display_name: string;
  local_path: string;
  branch: string | null;
  favorite: boolean;
  last_opened_at: string;
}

export interface HostedServerProfile {
  id: string;
  display_name: string;
  store_path: string;
  advertised_url: string;
  last_configured_at: string;
}

export interface ActiveContext {
  project_id: string | null;
  server_id: string | null;
  identity_ref: string | null;
}

export interface ContextSettings {
  schema_version: number;
  servers: ServerProfile[];
  repositories: RepositoryBookmark[];
  projects: LocalProject[];
  hosted_servers: HostedServerProfile[];
  active: ActiveContext;
}

export interface ActiveContextSnapshot {
  server: ServerProfile | null;
  repository: RepositoryBookmark | null;
  project: LocalProject | null;
  branch: string | null;
  authMode: AuthMode;
  connection:
    | "local"
    | "connected"
    | "reconnecting"
    | "offline"
    | "auth_required";
}

export interface ProjectRecords {
  project: LocalProject;
  repository: RepositoryBookmark;
  server: ServerProfile | null;
}

export function recordsForProject(
  context: ContextSettings,
  projectId: string,
): ProjectRecords | null {
  const project = context.projects.find((item) => item.id === projectId);
  if (!project) return null;
  const repository = context.repositories.find(
    (item) => item.id === project.repository_id,
  );
  if (!repository) return null;
  const server = repository.server_id
    ? context.servers.find((item) => item.id === repository.server_id) ?? null
    : null;
  if (repository.server_id && !server) return null;
  return { project, repository, server };
}

export function snapshotForProject(
  records: ProjectRecords,
  status: RepoStatus,
): ActiveContextSnapshot {
  return {
    server: records.server,
    repository: records.repository,
    project: records.project,
    branch: status.branch,
    authMode: records.server?.auth_mode ?? "not_required",
    // Repository status proves the local working tree, not remote reachability.
    connection: records.server ? "offline" : "local",
  };
}

export const EMPTY_CONTEXT_SNAPSHOT: ActiveContextSnapshot = {
  server: null,
  repository: null,
  project: null,
  branch: null,
  authMode: "unknown",
  connection: "offline",
};
