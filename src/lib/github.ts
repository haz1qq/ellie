import { useEffect, useRef, useState } from "react";
import { githubErrorCopy, githubErrorGeneric } from "../copy";
import {
  desktop,
  type GitHubConnectionStatus,
  type GitHubErrorCategory,
} from "./desktop";

const GITHUB_ERROR_CATEGORIES: readonly GitHubErrorCategory[] = [
  "window_denied",
  "invalid_input",
  "busy",
  "authorization_state_mismatch",
  "authorization_denied",
  "authentication_required",
  "authentication_expired",
  "rate_limited",
  "permission_denied",
  "not_found",
  "validation_failed",
  "network_unavailable",
  "provider_unavailable",
  "malformed_response",
  "credential_store",
  "cancelled",
];

/** Pulls the redacted category out of a Tauri IPC rejection, if present. */
export function githubCategoryOf(reason: unknown): GitHubErrorCategory | null {
  if (typeof reason !== "object" || reason === null) return null;
  const category = (reason as { category?: unknown }).category;
  if (typeof category !== "string") return null;
  return (GITHUB_ERROR_CATEGORIES as readonly string[]).includes(category)
    ? (category as GitHubErrorCategory)
    : null;
}

/** Friendly copy for a rejection; raw categories or traces are never shown. */
export function githubErrorText(reason: unknown): string {
  const category = githubCategoryOf(reason);
  return category ? githubErrorCopyOf(category) : githubErrorGeneric;
}

export function githubErrorCopyOf(category: GitHubErrorCategory): string {
  return githubErrorCopy[category] ?? githubErrorGeneric;
}

export type GitHubScope = {
  owner: string;
  repo: string;
  branch: string | null;
};

export function sameScope(left: GitHubScope | null, right: GitHubScope): boolean {
  return (
    left !== null &&
    left.owner === right.owner &&
    left.repo === right.repo &&
    left.branch === right.branch
  );
}

export function ownerOf(fullName: string): string {
  const slash = fullName.indexOf("/");
  return slash > 0 ? fullName.slice(0, slash) : fullName;
}

export interface GitHubConnectionApi {
  status: GitHubConnectionStatus | null;
  /** Friendly copy for the most recent failed operation; "" when none. */
  error: string;
  /** True while a connection mutation is outstanding. */
  busy: boolean;
  /**
   * True while a sign-in is outstanding. The Rust command waits for the whole
   * browser flow (up to 15 minutes), so this mirrors `Authorizing` without
   * depending on a status read.
   */
  signInPending: boolean;
  refresh: () => Promise<boolean>;
  saveClientId: (clientId: string) => Promise<boolean>;
  signIn: () => Promise<boolean>;
  cancelSignIn: () => Promise<boolean>;
  disconnect: () => Promise<boolean>;
}

/**
 * Shares the GitHub connection lifecycle between the GitHub view and the
 * Settings section, mirroring the LocalApiSettings ordering-boundary pattern:
 * each read and mutation snapshots a generation, and results from superseded
 * operations are ignored.
 */
export function useGitHubConnection(native: boolean): GitHubConnectionApi {
  const [status, setStatus] = useState<GitHubConnectionStatus | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [signInPending, setSignInPending] = useState(false);
  const generation = useRef(0);

  useEffect(() => {
    const generationAtMount = ++generation.current;
    if (!native) return;
    let active = true;
    desktop
      .githubConnectionStatus()
      .then((value) => {
        if (active && generationAtMount === generation.current) {
          setStatus(value);
          setError("");
        }
      })
      .catch((reason: unknown) => {
        if (active && generationAtMount === generation.current) {
          setError(githubErrorText(reason));
        }
      });
    return () => {
      active = false;
      generation.current += 1;
    };
  }, [native]);

  async function run(
    action: () => Promise<GitHubConnectionStatus>,
    markPending?: (pending: boolean) => void,
  ): Promise<boolean> {
    const current = ++generation.current;
    setBusy(true);
    setError("");
    markPending?.(true);
    try {
      const value = await action();
      if (current !== generation.current) return false;
      setStatus(value);
      setError("");
      return true;
    } catch (reason) {
      if (current !== generation.current) return false;
      setError(githubErrorText(reason));
      return false;
    } finally {
      // The pending flag must always settle, even when a newer operation
      // superseded this one (e.g. cancelling a pending sign-in).
      markPending?.(false);
      if (current === generation.current) setBusy(false);
    }
  }

  return {
    status,
    error,
    busy,
    signInPending,
    refresh: () => run(() => desktop.githubConnectionStatus()),
    saveClientId: (clientId: string) =>
      run(() => desktop.githubSaveClientId(clientId)),
    signIn: () => run(() => desktop.githubSignIn(), setSignInPending),
    cancelSignIn: () => run(() => desktop.githubCancelSignIn()),
    disconnect: () => run(() => desktop.githubDisconnect()),
  };
}