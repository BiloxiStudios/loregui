/**
 * Auth-domain Tauri IPC wrappers.
 *
 * These functions invoke the corresponding `#[tauri::command]` handlers
 * in the loregui Rust backend. Import and use from React components,
 * panel action handlers, or command-palette entries.
 */

import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// local_user_info
// ---------------------------------------------------------------------------

export interface ResolvedUserInfo {
  id: string;
  name: string;
  token?: string;
  preferred_username?: string;
  is_service_account?: boolean;
  expires?: number;
}

export interface LocalUserInfoArgs {
  /** Auth service remote URL; empty resolves from the repo's remote env. */
  auth_endpoint?: string;
  /** User identities to resolve; empty resolves the current user. */
  user_ids?: string[];
  /** Include cached token details for identities with a local token. */
  with_token?: boolean;
}

/**
 * Resolve user identities from locally stored JWT tokens.
 *
 * Does not require network access or a repository context.
 * Decodes locally cached JWT tokens to extract display names.
 *
 * @example
 * // Get current user info
 * const result = await getLocalUserInfo();
 * console.log(result.identities[0].name);
 *
 * @example
 * // Get current user info with token details
 * const result = await getLocalUserInfo({ with_token: true });
 * console.log(result.identities[0].token);
 *
 * @example
 * // Resolve specific user IDs
 * const result = await getLocalUserInfo({ user_ids: ["user-123", "user-456"] });
 */
export async function getLocalUserInfo(
  args: LocalUserInfoArgs = {},
): Promise<{ identities: ResolvedUserInfo[] }> {
  return invoke<{ identities: ResolvedUserInfo[] }>("local_user_info", {
    authEndpoint: args.auth_endpoint ?? "",
    userIds: args.user_ids ?? [],
    withToken: args.with_token ?? false,
  });
}
