import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  api,
  authLocalUserInfoApi,
  isNoAuthConfigured,
  isNoAuthIdentityUnavailable,
  type LocalUserInfo,
  type UserInfo,
} from "./api";

/**
 * Account / identity panel (top-bar identity surface) — the curated home for the
 * **auth** domain, per `docs/INFORMATION-ARCHITECTURE.md` (auth → top-bar identity
 * menu + onboarding). A panel is a curated view; the command palette remains the
 * exhaustive surface.
 *
 * This desktop app READS identity (current user, remote vs local device account)
 * and lets the user connect to a server. It deliberately implements NO billing,
 * PII, or SSO configuration — those live in the security-isolated accounts surface
 * and are never embedded here (see the accounts security boundary).
 *
 * Surfaced auth ops (each has a registered `#[tauri::command]`):
 *   - `auth_user_info`         — current server-verified identity (centerpiece)
 *   - `auth_local_user_info`   — local device identities cached on this machine
 *   - `auth_login_interactive` — connect to a server via the browser flow
 *
 * Not surfaced: `auth::logout` / `auth::clear` exist in lore-vm but have no
 * registered Tauri command yet, so they cannot be invoked from the GUI; the
 * Sign out section explains this rather than offering a button that can't work.
 *
 * SECURITY (SBAI-5910): pasted-token sign-in is **gone from this panel**, and
 * from the whole GUI. `auth_login_with_token` reached upstream with an empty
 * auth URL, so the pasted bearer was delivered to whatever authentication
 * endpoint the *user-supplied, untrusted* remote advertised — before any JWT
 * audience validation. The backend command now denies at its first line with a
 * constant message, and the UI no longer offers the affordance at all, so a
 * token can neither be typed here nor collected on the server's behalf.
 * Interactive browser sign-in is the only path. `auth_local_user_info` is
 * always called with `withToken=false` so cached tokens never enter the UI.
 * Themed entirely via `--surface-*` tokens; no new styles.
 */

type ConnectStep = "idle" | "authenticating" | "error";

export default function AccountPanel({ onClose }: { onClose: () => void }) {
  // --- current (server-verified) identity, loaded on open ---
  const [remoteUser, setRemoteUser] = useState<UserInfo | null>(null);
  const [remoteLoading, setRemoteLoading] = useState(true);
  const [remoteError, setRemoteError] = useState<string | null>(null);

  // --- local device identities (cached on this machine) ---
  const [localUsers, setLocalUsers] = useState<LocalUserInfo[]>([]);
  const [localLoading, setLocalLoading] = useState(true);
  const [localError, setLocalError] = useState<string | null>(null);

  // --- connect form (interactive browser sign-in is the only method) ---
  const [remoteUrl, setRemoteUrl] = useState("");
  const [connectStep, setConnectStep] = useState<ConnectStep>("idle");
  const [connectError, setConnectError] = useState<string | null>(null);
  const [connectedWithoutAuth, setConnectedWithoutAuth] = useState(false);

  const loadRemote = useCallback(async () => {
    setRemoteLoading(true);
    setRemoteError(null);
    try {
      setRemoteUser(await api.authUserInfo());
    } catch (e) {
      setRemoteUser(null);
      // SBAI-5478: auth-disabled / no-endpoint is "not signed in", not a red error.
      if (isNoAuthIdentityUnavailable(e)) {
        setRemoteError(null);
      } else {
        setRemoteError(typeof e === "string" ? e : JSON.stringify(e));
      }
    } finally {
      setRemoteLoading(false);
    }
  }, []);

  const loadLocal = useCallback(async () => {
    setLocalLoading(true);
    setLocalError(null);
    try {
      // withToken=false — never pull cached tokens into the UI.
      const result = await authLocalUserInfoApi.localUserInfo("", [], false);
      setLocalUsers(result.users);
    } catch (e) {
      setLocalUsers([]);
      // SBAI-5478: no local auth endpoint → empty device list, not red JSON.
      if (isNoAuthIdentityUnavailable(e)) {
        setLocalError(null);
      } else {
        setLocalError(typeof e === "string" ? e : JSON.stringify(e));
      }
    } finally {
      setLocalLoading(false);
    }
  }, []);

  // --- sign out (auth_clear: clear this device's cached sessions) ---
  const [confirmSignOut, setConfirmSignOut] = useState(false);
  const [signingOut, setSigningOut] = useState(false);
  const [signOutError, setSignOutError] = useState<string | null>(null);

  const signOut = useCallback(async () => {
    setSigningOut(true);
    setSignOutError(null);
    try {
      await invoke("auth_clear");
      setConfirmSignOut(false);
      // Reload identities — they should now reflect the cleared session.
      await loadRemote();
      await loadLocal();
    } catch (e) {
      setSignOutError(typeof e === "string" ? e : JSON.stringify(e));
    } finally {
      setSigningOut(false);
    }
  }, [loadRemote, loadLocal]);

  // Load both identities on open (the panel's centerpiece).
  useEffect(() => {
    void loadRemote();
    void loadLocal();
  }, [loadRemote, loadLocal]);

  // Esc closes the panel (DESIGN-SYSTEM: overlays dismiss on Esc).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const connect = useCallback(async () => {
    const url = remoteUrl.trim();
    if (!url) return;
    setConnectStep("authenticating");
    setConnectError(null);
    setConnectedWithoutAuth(false);
    try {
      // Interactive browser sign-in only — the pasted-token path is removed
      // from the GUI and denied by the backend (SBAI-5910).
      const user = await api.authLoginInteractive(url);
      setRemoteUser(user);
      setConnectedWithoutAuth(false);
      setConnectStep("idle");
      // Refresh the local identity list — a successful login caches a token.
      void loadLocal();
    } catch (e) {
      if (isNoAuthConfigured(e)) {
        setRemoteUser(null);
        setRemoteError(null);
        setLocalError(null);
        setConnectedWithoutAuth(true);
        setConnectStep("idle");
        return;
      }
      setConnectError(typeof e === "string" ? e : JSON.stringify(e));
      setConnectStep("error");
    }
  }, [remoteUrl, loadLocal]);

  const signedIn = remoteUser != null;
  const busy = connectStep === "authenticating";

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Account and identity"
      className="storage-scrim"
      onClick={onClose}
    >
      <div className="storage-panel" onClick={(e) => e.stopPropagation()}>
        <header className="storage-panel-header">
          <h2>Account</h2>
          <button onClick={onClose} title="Close (Esc)">
            Close
          </button>
        </header>

        {/* --- Current identity (centerpiece) --- */}
        <section className="storage-section">
          <h3>Signed in</h3>
          {remoteLoading && <p className="storage-help">Loading…</p>}

          {!remoteLoading && remoteError && (
            <>
              <div className="error storage-inline-error">{remoteError}</div>
              <button onClick={() => void loadRemote()}>Retry</button>
            </>
          )}

          {!remoteLoading && !remoteError && !signedIn && (
            <div className="storage-empty">
              <p className="empty">Not signed in — connect to a server.</p>
            </div>
          )}

          {!remoteLoading && !remoteError && signedIn && remoteUser && (
            <div className="onboarding-success">
              <div className="success-message">
                <span className="success-icon">&#10003;</span>
                <span>Signed in (server-verified)</span>
              </div>
              <div className="user-info">
                <div className="user-info-field">
                  <span className="user-info-label">Name</span>
                  <span className="user-info-value">{remoteUser.name}</span>
                </div>
                <div className="user-info-field">
                  <span className="user-info-label">ID</span>
                  <span className="user-info-value code">{remoteUser.id}</span>
                </div>
              </div>
            </div>
          )}
        </section>

        {/* --- Local device identities --- */}
        <section className="storage-section">
          <h3>This device</h3>
          <p className="storage-help">
            Identities cached locally on this machine. Local accounts are not
            server-verified until you connect.
          </p>
          {localLoading && <p className="storage-help">Loading…</p>}
          {!localLoading && localError && (
            <>
              <div className="error storage-inline-error">{localError}</div>
              <button onClick={() => void loadLocal()}>Retry</button>
            </>
          )}
          {!localLoading && !localError && localUsers.length === 0 && (
            <p className="empty">No local identities on this device.</p>
          )}
          {!localLoading && !localError && localUsers.length > 0 && (
            <ul className="storage-list">
              {localUsers.map((u) => (
                <li key={u.user_id}>
                  <span className="storage-backend-label">
                    {u.display_name || "(unnamed)"}
                  </span>
                  <code>{u.user_id}</code>
                </li>
              ))}
            </ul>
          )}
          {!localLoading && (
            <button onClick={() => void loadLocal()}>
              Refresh
            </button>
          )}
        </section>

        {/* --- Connect to a server --- */}
        <section className="storage-section">
          <h3>Connect to a server</h3>
          <p className="storage-help">
            Connect to a remote lore server. Sign in through your browser — the
            only method this app offers.
          </p>
          <p className="storage-help">
            Pasted-token sign-in is disabled in this build: it could deliver
            your token to an authentication endpoint advertised by an untrusted
            server. Use browser sign-in instead.
          </p>

          <div className="onboarding-field">
            <label htmlFor="account-url">Server URL</label>
            <input
              id="account-url"
              type="text"
              value={remoteUrl}
              disabled={busy}
              placeholder="https://api.studiobrain.ai"
              onChange={(e) => setRemoteUrl(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") void connect();
              }}
            />
          </div>

          {connectStep === "error" && connectError && (
            <div className="error storage-inline-error">{connectError}</div>
          )}

          {connectedWithoutAuth && (
            <div className="onboarding-success">
              <div className="success-message">
                <span className="success-icon">&#10003;</span>
                <span>Connected without authentication</span>
              </div>
            </div>
          )}

          <button
            className="storage-primary"
            disabled={busy || !remoteUrl.trim()}
            onClick={() => void connect()}
          >
            {busy
              ? "Connecting…"
              : connectStep === "error"
                ? "Retry connect"
                : signedIn || connectedWithoutAuth
                  ? "Connect to another server"
                  : "Connect"}
          </button>
        </section>

        {/* --- Sign out (auth_clear: clears this device's cached sessions) --- */}
        <section className="storage-section storage-danger">
          <h3>Sign out</h3>
          <p className="storage-help">
            Clears all auth sessions cached on this device. Account, billing,
            team, and SSO settings live in the StudioBrain accounts area, not in
            this app.
          </p>
          {signOutError && <div className="error">{signOutError}</div>}
          {!confirmSignOut ? (
            <button onClick={() => setConfirmSignOut(true)} disabled={signingOut}>
              Sign out
            </button>
          ) : (
            <div className="storage-confirm">
              <span>Clear all sessions on this device?</span>
              <button
                className="storage-primary"
                onClick={() => void signOut()}
                disabled={signingOut}
              >
                {signingOut ? "Signing out…" : "Confirm sign out"}
              </button>
              <button onClick={() => setConfirmSignOut(false)} disabled={signingOut}>
                Cancel
              </button>
            </div>
          )}
        </section>
      </div>
    </div>
  );
}
