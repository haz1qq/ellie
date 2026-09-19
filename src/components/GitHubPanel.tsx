import {
  BookLock,
  BookOpen,
  ChevronLeft,
  ChevronRight,
  ExternalLink,
  FolderPlus,
  GitCommitHorizontal,
  Lock,
  RefreshCw,
  Unplug,
  Link2,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import {
  desktop,
  type GitHubCommitSummary,
  type GitHubRepositorySummary,
} from "../lib/desktop";
import { githubErrorText, type GitHubConnectionApi } from "../lib/github";
import { formatAge, formatRelativeAge, shortSha } from "../lib/format";
import { Badge } from "./ui/Badge";
import { Button } from "./ui/Button";
import { EmptyState, Spinner } from "./ui/Panel";
import { Select } from "./ui/Select";
import { CreationOutcomeBanner } from "./github/CreationOutcomeBanner";
import { NewRepositoryDialog } from "./github/NewRepositoryDialog";

export interface GitHubPanelProps {
  native: boolean;
  connection: GitHubConnectionApi;
  onOpenSettings: () => void;
  /** Commands the creation dialog (from the shell "New repository"). */
  createRequest: number;
  onConsumeCreateRequest: () => void;
  onRepositoriesChanged: () => void;
}

const COMMITS_PER_PAGE = 10;

const BRANCH_OPTIONS = [
  { value: "default", label: "Default branch" },
  { value: "main", label: "main" },
  { value: "master", label: "master" },
];

/**
 * Polished GitHub workspace page: connection state, repository grid, scoped
 * commit list, creation flow, and outcome-unknown banner.
 */
export function GitHubPanel({
  native,
  connection,
  onOpenSettings,
  createRequest,
  onConsumeCreateRequest,
  onRepositoriesChanged,
}: GitHubPanelProps) {
  const [repositories, setRepositories] = useState<GitHubRepositorySummary[]>([]);
  const [repositoriesLoading, setRepositoriesLoading] = useState(true);
  const [repositoriesError, setRepositoriesError] = useState("");
  const [selected, setSelected] = useState<GitHubRepositorySummary | null>(null);
  const [branch, setBranch] = useState<string>("default");
  const [commits, setCommits] = useState<GitHubCommitSummary[]>([]);
  const [commitsLoading, setCommitsLoading] = useState(false);
  const [commitsError, setCommitsError] = useState("");
  const [commitPage, setCommitPage] = useState(1);
  const [creationOpen, setCreationOpen] = useState(false);

  const connected = connection.status?.state === "Connected";
  const [lastCreateRequest, setLastCreateRequest] = useState(0);

  useEffect(() => {
    if (createRequest !== lastCreateRequest) {
      setLastCreateRequest(createRequest);
      if (connected) setCreationOpen(true);
      onConsumeCreateRequest();
    }
  }, [createRequest, lastCreateRequest, onConsumeCreateRequest, connected]);

  function loadRepositories() {
    if (!native || !connected) {
      setRepositoriesLoading(false);
      return;
    }
    setRepositoriesLoading(true);
    setRepositoriesError("");
    desktop
      .githubListRepositories()
      .then((rows) => {
        setRepositories(rows);
        onRepositoriesChanged();
        setSelected((current) => {
          if (current) {
            const match = rows.find((row) => row.id === current.id);
            if (match) return match;
          }
          return rows[0] ?? null;
        });
      })
      .catch((reason: unknown) => {
        setRepositoriesError(githubErrorText(reason));
      })
      .finally(() => setRepositoriesLoading(false));
  }

  useEffect(() => {
    loadRepositories();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [native, connected]);

  // Re-derive commits when the selection or branch changes.
  useEffect(() => {
    setCommitPage(1);
    if (!native || !connected || !selected) {
      setCommits([]);
      setCommitsLoading(false);
      setCommitsError("");
      return;
    }
    let active = true;
    setCommitsLoading(true);
    setCommitsError("");
    desktop
      .githubListCommits(
        selected.fullName.split("/")[0] ?? selected.fullName,
        selected.name,
        branch === "default" ? undefined : branch,
      )
      .then((rows) => {
        if (active) setCommits(rows);
      })
      .catch((reason: unknown) => {
        if (active) setCommitsError(githubErrorText(reason));
      })
      .finally(() => {
        if (active) setCommitsLoading(false);
      });
    return () => {
      active = false;
    };
  }, [native, connected, selected, branch]);

  const connectedAccountId = connection.status?.account?.id ?? null;
  const attribution = useMemo(() => {
    const linked = commits.filter(
      (commit) => connectedAccountId !== null && commit.authorId === connectedAccountId,
    ).length;
    return { linked };
  }, [commits, connectedAccountId]);
  const commitPageCount = Math.max(1, Math.ceil(commits.length / COMMITS_PER_PAGE));
  const visibleCommits = commits.slice(
    (commitPage - 1) * COMMITS_PER_PAGE,
    commitPage * COMMITS_PER_PAGE,
  );

  const stateLabel = !native
    ? "Desktop only"
    : connection.status === null
      ? "Checking"
      : connection.status.state === "Connected"
        ? "Connected"
        : connection.status.state === "Authorizing"
          ? "Signing in"
          : "Not connected";

  return (
    <div className="github-page">
      <div className="github-toolbar">
        <div className="todo-toolbar-title">
          <p className="eyebrow">Connected work</p>
          <h1 className="page-title">GitHub</h1>
          <p className="page-sub">
            Track repositories and pushed commits on the connected account.
          </p>
        </div>
        <div className="todo-toolbar-actions">
          <Button
            variant="primary"
            disabled={!native || !connected || connection.busy}
            onClick={() => setCreationOpen(true)}
          >
            <FolderPlus size={15} />
            New repository
          </Button>
          <Button
            variant="ghost"
            disabled={!native || !connected || connection.busy}
            onClick={() => void connection.refresh()}
          >
            <RefreshCw size={14} />
            Refresh
          </Button>
        </div>
      </div>

      <section className="connection-banner" aria-label="GitHub connection">
        <div className="connection-copy">
          <Badge tone={connected ? "good" : stateLabel === "Signing in" ? "warning" : "neutral"}>
            {stateLabel}
          </Badge>
          <p>
            {connected
              ? `Signed in as @${connection.status?.account?.login ?? "your account"} — read and create access for your personal repositories.`
              : connection.status?.clientIdConfigured && connection.status.clientSecretConfigured
                ? "App credentials are ready. Connect your GitHub account to continue."
                : "Set up the GitHub App credentials in Settings, then connect."}
          </p>
        </div>
        <div className="connection-actions">
          {connected ? (
            <>
              <Button
                size="sm"
                variant="danger"
                onClick={() => void connection.disconnect()}
                disabled={connection.busy}
              >
                <Unplug size={13} />
                Disconnect
              </Button>
              <Button size="sm" variant="ghost" onClick={onOpenSettings}>
                Manage credentials
              </Button>
            </>
          ) : (
            <Button
              size="sm"
              variant="primary"
              onClick={() => void connection.signIn()}
              disabled={
                !native ||
                connection.busy ||
                connection.signInPending ||
                !connection.status?.clientIdConfigured ||
                !connection.status?.clientSecretConfigured
              }
            >
              <Link2 size={13} />
              {connection.signInPending ? "Signing in…" : "Connect GitHub"}
            </Button>
          )}
        </div>
      </section>

      {connection.error && (
        <p className="page-alert" role="alert">
          {connection.error}
        </p>
      )}

      <CreationOutcomeBanner native={native} connected={connected} />

      <div className="github-layout">
        <section className="panel" aria-label="Repositories">
          <header className="panel-header">
            <div className="panel-title">
              <span className="panel-kicker">Tracked scope</span>
              <h2 className="panel-heading">Repositories</h2>
            </div>
            <span className="panel-count">
              {!connected
                ? "—"
                : repositoriesLoading
                  ? "loading…"
                  : `${repositories.length} loaded`}
            </span>
          </header>
          {!native ? (
            <EmptyState title="Repositories need the desktop app">
              <p className="empty-state-text">
                Browser preview does not call GitHub.
              </p>
            </EmptyState>
          ) : !connected ? (
            <EmptyState icon={<Lock size={17} />} title="Connect to load repositories">
              <p className="empty-state-text">
                No repositories are guessed while disconnected.
              </p>
            </EmptyState>
          ) : repositoriesLoading ? (
            <Spinner label="Loading repositories…" />
          ) : repositoriesError ? (
            <EmptyState title="Repositories unavailable" className="empty-state-error">
              <p className="empty-state-text" role="alert">
                {repositoriesError}
              </p>
            </EmptyState>
          ) : repositories.length === 0 ? (
            <EmptyState title="No repositories loaded">
              <p className="empty-state-text">
                The connected account returned no repositories in the bounded read.
              </p>
            </EmptyState>
          ) : (
            <ul className="repo-grid">
              {repositories.map((repo) => (
                <li
                  key={repo.id}
                  className={
                    selected?.id === repo.id
                      ? "repo-card repo-card-active"
                      : "repo-card"
                  }
                >
                  <button
                    type="button"
                    className="repo-card-select"
                    aria-pressed={selected?.id === repo.id}
                    onClick={() => setSelected(repo)}
                  >
                    <span className="repo-card-icon" aria-hidden="true">
                      {repo.private ? <BookLock size={15} /> : <BookOpen size={15} />}
                    </span>
                    <span className="repo-card-copy">
                      <strong>{repo.fullName}</strong>
                      <span className="repo-card-meta">
                        {repo.private ? "Private" : "Public"} · {repo.defaultBranch}
                      </span>
                    </span>
                  </button>
                  <a
                    href={repo.htmlUrl}
                    target="_blank"
                    rel="noreferrer"
                    aria-label={`Open ${repo.fullName} on GitHub`}
                  >
                    <ExternalLink size={12} />
                  </a>
                </li>
              ))}
            </ul>
          )}
        </section>

        <section className="panel" aria-label="Commits">
          <header className="panel-header">
            <div className="panel-title">
              <span className="panel-kicker">Bounded history</span>
              <h2 className="panel-heading">Commits</h2>
            </div>
            <Select
              label="Branch"
              value={branch}
              onValueChange={setBranch}
              options={BRANCH_OPTIONS}
              disabled={!connected || !native}
            />
          </header>
          {!connected || !native ? (
            <EmptyState title="Commits appear after connecting">
              <p className="empty-state-text">
                Select a loaded repository to browse its commit history.
              </p>
            </EmptyState>
          ) : commitsLoading ? (
            <Spinner label="Loading commits…" />
          ) : commitsError ? (
            <EmptyState title="Commits unavailable" className="empty-state-error">
              <p className="empty-state-text" role="alert">
                {commitsError}
              </p>
            </EmptyState>
          ) : commits.length === 0 ? (
            <EmptyState icon={<GitCommitHorizontal size={17} />} title="No commits loaded">
              <p className="empty-state-text">
                {selected
                  ? "This scope returned nothing yet — or the repository has no commits."
                  : "Choose a repository from the list."}
              </p>
            </EmptyState>
          ) : (
            <>
              <ul className="commit-list">
                {visibleCommits.map((commit) => (
                  <li className="commit-row" key={commit.sha}>
                    <span className="commit-avatar" aria-hidden="true">
                      {commit.authorLogin ? commit.authorLogin.slice(0, 1).toUpperCase() : "?"}
                    </span>
                    <div className="commit-copy">
                      <p className="commit-subject">{commit.subject}</p>
                      <p className="commit-meta">
                        <code>{shortSha(commit.sha)}</code> ·{" "}
                        {commit.authorLogin ?? (commit.authorId ? "linked author" : "unattributed")} ·{" "}
                        committed {formatRelativeAge(commit.committedAt)}
                        {commit.authoredAt !== commit.committedAt
                          ? ` · authored ${formatAge(commit.authoredAt)} ago`
                          : ""}
                      </p>
                    </div>
                  </li>
                ))}
              </ul>
              {commitPageCount > 1 && (
                <nav className="commit-pagination" aria-label="Commit pages">
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={commitPage === 1}
                    onClick={() => setCommitPage((page) => Math.max(1, page - 1))}
                  >
                    <ChevronLeft size={13} /> Previous
                  </Button>
                  <span>
                    Page {commitPage} of {commitPageCount} · {commits.length} loaded
                  </span>
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={commitPage === commitPageCount}
                    onClick={() =>
                      setCommitPage((page) => Math.min(commitPageCount, page + 1))
                    }
                  >
                    Next <ChevronRight size={13} />
                  </Button>
                </nav>
              )}
              <p className="commit-scope">
                {commits.length} loaded · {selected?.fullName ?? "selected repository"}
                {branch !== "default" ? ` · ${branch}` : ` · ${selected?.defaultBranch ?? "default branch"}`} · locally
                counted, not an account-wide total
              </p>
              <p className="commit-attribution">
                {attribution.linked} of {commits.length} commits attributed to{" "}
                {connection.status?.account?.login ?? "the connected account"} by GitHub.
              </p>
            </>
          )}
        </section>
      </div>

      <NewRepositoryDialog
        open={creationOpen}
        onOpenChange={setCreationOpen}
        native={native}
        refreshRepositories={loadRepositories}
      />
    </div>
  );
}
