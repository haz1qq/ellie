import { useEffect, useRef, useState } from "react";
import { desktop, type GitHubCommitSummary, type GitHubRepositorySummary } from "../lib/desktop";
import {
  githubErrorCopyOf,
  githubErrorText,
  ownerOf,
  sameScope,
  useGitHubConnection,
  type GitHubScope,
} from "../lib/github";
import { ConfirmDialog } from "./ConfirmDialog";

function formatLocalTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString([], {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

function scopeLabel(scope: GitHubScope): string {
  if (scope.branch) return `${scope.owner}/${scope.repo} @ ${scope.branch}`;
  return `${scope.owner}/${scope.repo} · default branch`;
}

export function GitHubPanel({ native }: { native: boolean }) {
  const connection = useGitHubConnection(native);
  const { status: connectionStatus, error: connectionError, busy, signInPending } = connection;
  const [repositories, setRepositories] = useState<GitHubRepositorySummary[] | null>(null);
  const [repositoriesLoading, setRepositoriesLoading] = useState(false);
  const [repositoriesError, setRepositoriesError] = useState("");
  const [repositoriesReload, setRepositoriesReload] = useState(0);
  const [selectedRepoId, setSelectedRepoId] = useState<number | null>(null);
  const [branch, setBranch] = useState("");
  const [committedBranch, setCommittedBranch] = useState("");
  const [commits, setCommits] = useState<GitHubCommitSummary[] | null>(null);
  const [commitsScope, setCommitsScope] = useState<GitHubScope | null>(null);
  const [commitsLoading, setCommitsLoading] = useState(false);
  const [commitsError, setCommitsError] = useState("");
  const [commitsReload, setCommitsReload] = useState(0);
  const [disconnectOpen, setDisconnectOpen] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const lastCommitsScope = useRef<GitHubScope | null>(null);

  const connected = connectionStatus?.state === "Connected";
  const authorizing = connectionStatus?.state === "Authorizing";
  const needsClientId = connectionStatus !== null && !connectionStatus.clientIdConfigured;
  const statusError = connectionStatus?.lastError
    ? githubErrorCopyOf(connectionStatus.lastError)
    : "";
  const visibleError = connectionError || statusError;
  const selectedRepository =
    (repositories ?? []).find((repo) => repo.id === selectedRepoId) ?? null;
  const selectedOwner = selectedRepository ? ownerOf(selectedRepository.fullName) : null;
  const selectedRepoName = selectedRepository?.name ?? null;

  // Drop GitHub content when the connection is not live: a disconnected,
  // authorizing, or failed connection must never masquerade as zero data.
  useEffect(() => {
    if (!native || connectionStatus === null) return;
    if (connectionStatus.state !== "Connected") {
      setRepositories(null);
      setRepositoriesError("");
      setSelectedRepoId(null);
      setBranch("");
      setCommittedBranch("");
      setCommits(null);
      setCommitsScope(null);
      setCommitsError("");
      setCommitsLoading(false);
      setRepositoriesLoading(false);
      lastCommitsScope.current = null;
    }
  }, [native, connectionStatus]);

  useEffect(() => {
    if (!native || !connected) return;
    let active = true;
    setRepositoriesLoading(true);
    setRepositoriesError("");
    desktop
      .githubListRepositories()
      .then((value) => {
        if (active) {
          setRepositories(value);
          setSelectedRepoId((current) =>
            value.some((repo) => repo.id === current) ? current : null,
          );
        }
      })
      .catch((reason: unknown) => {
        if (active) setRepositoriesError(githubErrorText(reason));
      })
      .finally(() => {
        if (active) setRepositoriesLoading(false);
      });
    return () => {
      active = false;
    };
  }, [native, connected, repositoriesReload]);

  useEffect(() => {
    if (!native || !connected || selectedOwner === null || selectedRepoName === null) {
      return;
    }
    let active = true;
    const trimmedBranch = committedBranch.trim() || null;
    const scope: GitHubScope = {
      owner: selectedOwner,
      repo: selectedRepoName,
      branch: trimmedBranch,
    };
    if (!sameScope(lastCommitsScope.current, scope)) {
      lastCommitsScope.current = scope;
      setCommits(null);
      setCommitsError("");
    }
    setCommitsLoading(true);
    desktop
      .githubListCommits(selectedOwner, selectedRepoName, trimmedBranch ?? undefined)
      .then((value) => {
        if (active) {
          setCommits(value);
          setCommitsScope(scope);
        }
      })
      .catch((reason: unknown) => {
        // A failed reload of the same scope keeps the prior list visible.
        if (active) setCommitsError(githubErrorText(reason));
      })
      .finally(() => {
        if (active) setCommitsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [native, connected, selectedOwner, selectedRepoName, committedBranch, commitsReload]);

  async function handleCancelSignIn() {
    setCancelling(true);
    try {
      await connection.cancelSignIn();
    } finally {
      setCancelling(false);
    }
  }

  const cancelDisabled = !native || cancelling;
  const waiting = signInPending || authorizing;

  const coverage =
    commits === null || commitsScope === null
      ? null
      : `${commits.length} loaded commit${commits.length === 1 ? "" : "s"} · ${scopeLabel(commitsScope)}`;

  return (
    <section className="github" aria-labelledby="github-title">
      <p className="eyebrow">A gentle look at your work</p>
      <h1 id="github-title">GitHub</h1>
      <p className="welcome-copy">
        Repositories and bounded commit history for the connected account. These
        are workspace records — never quota or allowance data.
      </p>

      <section
        className="connection-banner"
        aria-labelledby="github-connection-heading"
      >
        <div className="connection-copy">
          <h2 id="github-connection-heading" className="visually-hidden">
            Connection
          </h2>
          {!native ? (
            <>
              <span className="github-status-badge status-muted">Preview</span>
              <p>Open the desktop app to connect GitHub.</p>
            </>
          ) : waiting ? (
            <>
              <span className="github-status-badge status-busy">Waiting</span>
              <p>
                Waiting for GitHub… Complete the sign-in in the browser tab that
                opened. Ellie waits up to 15 minutes.
              </p>
            </>
          ) : connected ? (
            <>
              <span className="github-status-badge status-ok">Connected</span>
              <p>
                Connected as @{connectionStatus.account?.login ?? "your account"}.
                {connectionStatus.tokenPresent
                  ? " Refresh token saved on this device."
                  : " No saved refresh token yet."}
              </p>
            </>
          ) : (
            <>
              <span className="github-status-badge status-muted">
                Not connected
              </span>
              <p>
                Connect your GitHub account to browse repositories and recent
                commit history.
              </p>
            </>
          )}
          {needsClientId && (
            <p className="github-hint">
              Add your GitHub App Client ID in Settings → GitHub before connecting.
            </p>
          )}
          {visibleError && (
            <p className="github-alert" role="alert">
              {visibleError}
            </p>
          )}
        </div>
        <div className="connection-actions">
          {!native ? null : waiting ? (
            <button type="button" onClick={() => void handleCancelSignIn()} disabled={cancelDisabled}>
              Cancel sign-in
            </button>
          ) : connected ? (
            <button
              type="button"
              onClick={() => setDisconnectOpen(true)}
              disabled={busy}
            >
              Disconnect
            </button>
          ) : (
            <button
              type="button"
              disabled={busy || needsClientId || connectionStatus === null}
              onClick={() => void connection.signIn()}
            >
              Connect GitHub account
            </button>
          )}
        </div>
      </section>

      <div className="section-heading">
        <h2>Repositories</h2>
        <span>
          {repositories === null
            ? "Scope: connected GitHub account"
            : `${repositories.length} shown for the connected account`}
        </span>
      </div>
      {!native ? (
        <p className="github-empty">Open the desktop app to load repositories.</p>
      ) : !connected ? (
        <p className="github-empty">
          Connect your GitHub account to load repositories. Until then there is
          nothing to show — not an empty account.
        </p>
      ) : repositoriesLoading && repositories === null ? (
        <p className="github-empty" role="status">
          Loading repositories…
        </p>
      ) : repositoriesError !== "" && repositories === null ? (
        <div className="github-failed">
          <p role="alert">{repositoriesError}</p>
          <button
            type="button"
            onClick={() => setRepositoriesReload((value) => value + 1)}
          >
            Reload repositories
          </button>
        </div>
      ) : repositoriesError !== "" && repositories ? (
        <>
          <div className="github-failed">
            <p role="alert">
              {repositoriesError} Repositories couldn’t be refreshed; showing the
              previous list.
            </p>
            <button
              type="button"
              onClick={() => setRepositoriesReload((value) => value + 1)}
            >
              Reload repositories
            </button>
          </div>
          <div className="repository-list">
            {repositories.map((repo) => (
              <RepositoryCard key={repo.id} repo={repo} />
            ))}
          </div>
        </>
      ) : repositories && repositories.length === 0 ? (
        <p className="github-empty">
          No repositories found for this account. GitHub returned an empty list.
        </p>
      ) : repositories ? (
        <>
          {repositoriesLoading && (
            <p className="github-empty" role="status">
              Refreshing repositories… showing the previous list.
            </p>
          )}
          <div className="repository-list">
            {repositories.map((repo) => (
              <RepositoryCard key={repo.id} repo={repo} />
            ))}
          </div>
        </>
      ) : null}

      <div className="section-heading">
        <h2>Recent commits</h2>
        <span>Bounded per-scope history · never an account total</span>
      </div>
      {!native ? (
        <p className="github-empty">Open the desktop app to load commits.</p>
      ) : !connected ? (
        <p className="github-empty">
          Connect a GitHub account, then choose a repository to load commits.
        </p>
      ) : (
        <>
          <div className="github-controls">
            <div className="github-field">
              <label htmlFor="github-repo-picker">Repository</label>
              <select
                id="github-repo-picker"
                disabled={repositories === null || repositories.length === 0}
                value={selectedRepoId ?? ""}
                onChange={(event) => {
                  setSelectedRepoId(Number(event.target.value));
                  setBranch("");
                  setCommittedBranch("");
                  setCommits(null);
                  setCommitsError("");
                  lastCommitsScope.current = null;
                }}
              >
                <option value="" disabled>
                  {repositories === null
                    ? "Loading repositories…"
                    : repositories.length === 0
                      ? "No repositories loaded"
                      : "Choose a repository"}
                </option>
                {(repositories ?? []).map((repo) => (
                  <option key={repo.id} value={repo.id}>
                    {repo.fullName}
                  </option>
                ))}
              </select>
            </div>
            <div className="github-field">
              <label htmlFor="github-branch-input">Branch (optional)</label>
              <input
                id="github-branch-input"
                type="text"
                value={branch}
                placeholder={selectedRepository?.defaultBranch ?? "main"}
                disabled={!selectedRepository}
                onChange={(event) => setBranch(event.target.value)}
              />
            </div>
            <button
              type="button"
              disabled={!selectedRepository}
              onClick={() => {
                setCommittedBranch(branch.trim());
                setCommitsReload((value) => value + 1);
              }}
            >
              Load commits
            </button>
          </div>
          {commitsLoading && commits === null ? (
            <p className="github-empty" role="status">
              Loading commits…
            </p>
          ) : commitsError !== "" && commits === null ? (
            <div className="github-failed">
              <p role="alert">{commitsError}</p>
              <button
                type="button"
                onClick={() => setCommitsReload((value) => value + 1)}
              >
                Retry load
              </button>
            </div>
          ) : commits === null ? (
            <p className="github-empty">
              Choose a repository above to load its recent commit history.
            </p>
          ) : commits.length === 0 ? (
            <>
              <p className="coverage-label" role="status">
                {coverage}
              </p>
              <p className="github-empty">
                0 commits loaded for this scope. GitHub returned a completed
                empty list for it.
              </p>
            </>
          ) : (
            <>
              {commitsLoading && (
                <p className="github-empty" role="status">
                  Refreshing… showing the previous list.
                </p>
              )}
              {commitsError !== "" && (
                <div className="github-failed">
                  <p role="alert">
                    {commitsError} Showing previously loaded commits for this
                    scope.
                  </p>
                  <button
                    type="button"
                    onClick={() => setCommitsReload((value) => value + 1)}
                  >
                    Retry load
                  </button>
                </div>
              )}
              <p className="coverage-label" role="status">
                {coverage}
              </p>
              <div className="commit-list">
                {commits.map((commit) => (
                  <CommitRow key={commit.sha} commit={commit} />
                ))}
              </div>
            </>
          )}
          <p className="github-footnote">
            Loaded commits are a bounded, scoped read from GitHub — never an
            account-wide or pushed count, and commit times are commit times,
            not push times.
          </p>
        </>
      )}

      <ConfirmDialog
        open={disconnectOpen}
        title="Disconnect GitHub?"
        body="This removes the connected account and the saved refresh token from this device. Local AI usage data is unaffected, and you can reconnect at any time."
        confirmLabel="Disconnect"
        cancelLabel="Keep connected"
        busy={busy}
        onConfirm={() => {
          void connection.disconnect().finally(() => setDisconnectOpen(false));
        }}
        onCancel={() => setDisconnectOpen(false)}
      />
    </section>
  );
}

function RepositoryCard({ repo }: { repo: GitHubRepositorySummary }) {
  return (
    <article className="repo-card">
      <div className="repo-card-main">
        <h3>{repo.name}</h3>
        <p>{repo.fullName}</p>
      </div>
      <span
        className={`repo-visibility ${repo.private ? "repo-private" : "repo-public"}`}
      >
        {repo.private ? "Private" : "Public"}
      </span>
      <span className="repo-detail">Default branch: {repo.defaultBranch}</span>
      <span className="repo-url">{repo.htmlUrl}</span>
    </article>
  );
}

function CommitRow({ commit }: { commit: GitHubCommitSummary }) {
  return (
    <article className="commit-row">
      <span className="commit-sha">{commit.sha.slice(0, 7)}</span>
      <div className="commit-main">
        <strong className="commit-subject">{commit.subject}</strong>
        <span className="commit-meta">
          {commit.authorLogin ? `@${commit.authorLogin}` : "Unattributed"} ·{" "}
          {formatLocalTime(commit.committedAt)}
        </span>
      </div>
    </article>
  );
}