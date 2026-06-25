import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";

/** Extract a human-readable message from a thrown value (a LoreError is
 * serialized as `{ kind, message }`; plain strings and Errors pass through).
 * Mirrors App.tsx's `errText` so a thrown command error never reaches the user
 * as raw `{"kind":...}` JSON (loregui #331). */
function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  if (e && typeof e === "object") {
    const o = e as { message?: unknown; kind?: unknown };
    if (typeof o.message === "string") return o.message;
    if (typeof o.kind === "string") return o.kind;
  }
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  error: unknown;
}

/**
 * App-shell error boundary (loregui #331).
 *
 * Before this existed, ANY thrown error on the first-run path — most acutely a
 * `lore status` "Repository not found" surfacing as an uncaught `CommandFailed`
 * on a fresh install where the working dir is the app-data folder, not a repo —
 * would unmount the whole React tree to a blank window, leaving the user no way
 * to reach onboarding, settings, or server config. The app "flashed then closed."
 *
 * This boundary catches that throw and renders a calm, themeable recovery state
 * instead of a crash-close, so the window stays usable. It uses the semantic
 * `--surface-*` tokens (DESIGN-SYSTEM) so it re-themes with the rest of the app.
 */
export default class ErrorBoundary extends Component<
  ErrorBoundaryProps,
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: unknown, info: ErrorInfo): void {
    // Surface to the console for diagnostics; never re-throw.
    console.error("LoreGUI app shell error:", error, info.componentStack);
  }

  private reset = (): void => {
    this.setState({ error: null });
  };

  render(): ReactNode {
    if (this.state.error == null) return this.props.children;

    return (
      <div className="onboarding" role="alert">
        <div className="onboarding-card">
          <h2>Something went wrong</h2>
          <p className="onboarding-description">
            LoreGUI hit an unexpected error and paused to keep the window open.
            You can retry, or open a repository / configure a server from the
            main view.
          </p>
          <pre
            style={{
              margin: "0 0 16px",
              padding: "12px 14px",
              borderRadius: 8,
              whiteSpace: "pre-wrap",
              wordBreak: "break-word",
              background: "var(--surface-error-bg, rgba(248,81,73,0.12))",
              color: "var(--surface-error-text, var(--red))",
              border:
                "1px solid var(--surface-error-border, var(--border))",
              fontSize: 13,
            }}
          >
            {errText(this.state.error)}
          </pre>
          <div className="onboarding-nav">
            <button
              className="onboarding-button onboarding-button--primary"
              onClick={this.reset}
            >
              Try again
            </button>
          </div>
        </div>
      </div>
    );
  }
}
