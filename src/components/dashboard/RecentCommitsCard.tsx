import { GitBranch, GitCommitHorizontal, GitFork } from "lucide-react";
import { useEffect, useState } from "react";
import { desktop, type GitHubCommitSummary } from "../../lib/desktop";
import { githubErrorText } from "../../lib/github";
import { formatRelativeAge, shortSha } from "../../lib/format";
import { Button } from "../ui/Button";
import { EmptyState, Spinner } from "../ui/Panel";

const MAX_OVERVIEW_REPOSITORIES = 12;
const MAX_COMMITS_PER_REPOSITORY = 3;
const MAX_OVERVIEW_COMMITS = 6;
const REPOSITORY_FETCH_CONCURRENCY = 4;

export interface RecentCommitsCardProps {
  native: boolean;
  connected: boolean;
  repositories: Array<{ id: number; fullName: string; htmlUrl: string }>;
  onOpenGitHub: () => void;
}

interface ScopedRepository {
  owner: string;
  repo: string;
  fullName: string;
}

interface RepositoryCommit extends GitHubCommitSummary {
  repository: string;
}

/**
 * Overview commit feed: newest authored commits merged from a bounded set of
 * loaded repositories. Each row retains repository attribution and failures
 * remain isolated so one unavailable repository does not hide good results.
 */
export function RecentCommitsCard({
  native,
  connected,
  repositories,
  onOpenGitHub,
}: RecentCommitsCardProps) {
  const [commits, setCommits] = useState<RepositoryCommit[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [loadedRepositoryCount, setLoadedRepositoryCount] = useState(0);
  const [failedRepositoryCount, setFailedRepositoryCount] = useState(0);

  const scopedRepositoryCount = Math.min(
    repositories.length,
    MAX_OVERVIEW_REPOSITORIES,
  );
  const omittedRepositoryCount = Math.max(
    0,
    repositories.length - MAX_OVERVIEW_REPOSITORIES,
  );

  useEffect(() => {
    if (!native || !connected || repositories.length === 0) {
      setCommits([]);
      setLoading(false);
      setError("");
      setLoadedRepositoryCount(0);
      setFailedRepositoryCount(0);
      return;
    }

    const selectedFullNames = repositories
      .slice(0, MAX_OVERVIEW_REPOSITORIES)
      .map((repository) => repository.fullName);
    const selectedRepositories = selectedFullNames
      .map(parseRepository)
      .filter(
        (repository): repository is ScopedRepository => repository !== null,
      );

    let active = true;
    setCommits([]);
    setLoading(true);
    setError("");
    setLoadedRepositoryCount(0);
    setFailedRepositoryCount(0);

    async function loadCommits() {
      const loadedCommits: RepositoryCommit[] = [];
      let successfulRepositories = 0;
      let failedRepositories = selectedFullNames.length - selectedRepositories.length;
      let firstError = "";

      for (
        let index = 0;
        index < selectedRepositories.length;
        index += REPOSITORY_FETCH_CONCURRENCY
      ) {
        const batch = selectedRepositories.slice(
          index,
          index + REPOSITORY_FETCH_CONCURRENCY,
        );
        const results = await Promise.allSettled(
          batch.map(async (repository) => {
            const rows = await desktop.githubListCommits(
              repository.owner,
              repository.repo,
              undefined,
              MAX_COMMITS_PER_REPOSITORY,
            );
            return rows.map((commit) => ({
              ...commit,
              repository: repository.fullName,
            }));
          }),
        );

        if (!active) return;
        for (const result of results) {
          if (result.status === "fulfilled") {
            successfulRepositories += 1;
            loadedCommits.push(...result.value);
          } else {
            failedRepositories += 1;
            if (!firstError) firstError = githubErrorText(result.reason);
          }
        }
      }

      if (!active) return;
      loadedCommits.sort(
        (left, right) => timestampOf(right.authoredAt) - timestampOf(left.authoredAt),
      );
      setCommits(loadedCommits.slice(0, MAX_OVERVIEW_COMMITS));
      setLoadedRepositoryCount(successfulRepositories);
      setFailedRepositoryCount(failedRepositories);
      if (successfulRepositories === 0 && failedRepositories > 0) {
        setError(firstError || "Ellie could not load commits from the selected repositories.");
      }
    }

    void loadCommits().finally(() => {
      if (active) setLoading(false);
    });

    return () => {
      active = false;
    };
  }, [native, connected, repositories]);

  return (
    <section className="panel panel-commits" aria-label="Recent GitHub commits">
      <header className="panel-header">
        <div className="panel-title">
          <span className="panel-kicker">Connected work</span>
          <h2 className="panel-heading">
            <GitFork size={14} className="panel-heading-icon" />
            Recent commits
          </h2>
        </div>
        <Button size="sm" variant="ghost" onClick={onOpenGitHub}>
          Open GitHub
        </Button>
      </header>
      {connected && scopedRepositoryCount > 0 && (
        <p className="commit-repo" title="Repositories checked for this commit feed">
          <GitBranch size={12} className="commit-repo-icon" aria-hidden="true" />
          <span className="commit-repo-name">
            {omittedRepositoryCount > 0
              ? `${scopedRepositoryCount} of ${repositories.length} repositories`
              : `Across ${scopedRepositoryCount} ${
                  scopedRepositoryCount === 1 ? "repository" : "repositories"
                }`}
          </span>
        </p>
      )}
      {!connected ? (
        <EmptyState icon={<GitCommitHorizontal size={18} />} title="GitHub not connected">
          <p className="empty-state-text">
            Connect GitHub to see your latest commits here. No activity is shown
            rather than inventing a zero.
          </p>
          <Button size="sm" onClick={onOpenGitHub} className="empty-state-action">
            Connect on the GitHub page
          </Button>
        </EmptyState>
      ) : loading ? (
        <Spinner label="Loading commits…" />
      ) : error ? (
        <EmptyState title="Commits unavailable" className="empty-state-error">
          <p className="empty-state-text" role="alert">
            {error}
          </p>
        </EmptyState>
      ) : repositories.length === 0 ? (
        <EmptyState title="No repositories loaded">
          <p className="empty-state-text">
            Ellie did not receive any repositories to check for recent work.
          </p>
        </EmptyState>
      ) : commits.length === 0 ? (
        <EmptyState title="No commits loaded">
          <p className="empty-state-text">
            The checked repositories returned no commits yet.
          </p>
        </EmptyState>
      ) : (
        <>
          <ul className="commit-feed">
            {commits.map((commit) => (
              <li className="commit-feed-item" key={`${commit.repository}:${commit.sha}`}>
                <span className="commit-avatar" aria-hidden="true">
                  {commit.authorLogin ? commit.authorLogin.slice(0, 1).toUpperCase() : "?"}
                </span>
                <div className="commit-feed-copy">
                  <p className="commit-feed-subject">{commit.subject}</p>
                  <p className="commit-feed-meta">
                    <span className="commit-feed-repository">{commit.repository}</span> ·{" "}
                    <code>{shortSha(commit.sha)}</code> ·{" "}
                    {commit.authorLogin ?? (commit.authorId ? "linked author" : "unattributed")} ·{" "}
                    {formatRelativeAge(commit.authoredAt)}
                  </p>
                </div>
              </li>
            ))}
          </ul>
          <p className="commit-scope">
            {commits.length} shown · {loadedRepositoryCount} {loadedRepositoryCount === 1 ? "repository" : "repositories"} checked
            {failedRepositoryCount > 0 ? ` · ${failedRepositoryCount} unavailable` : ""}
            {omittedRepositoryCount > 0 ? ` · ${omittedRepositoryCount} not checked` : ""} · bounded overview
          </p>
        </>
      )}
    </section>
  );
}

function parseRepository(fullName: string): ScopedRepository | null {
  const separator = fullName.indexOf("/");
  if (separator <= 0 || separator === fullName.length - 1) return null;
  return {
    owner: fullName.slice(0, separator),
    repo: fullName.slice(separator + 1),
    fullName,
  };
}

function timestampOf(value: string): number {
  const timestamp = Date.parse(value);
  return Number.isNaN(timestamp) ? 0 : timestamp;
}
