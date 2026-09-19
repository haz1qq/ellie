import { GitFork, GitCommitHorizontal } from "lucide-react";
import { useEffect, useState } from "react";
import { desktop, type GitHubCommitSummary } from "../../lib/desktop";
import { githubErrorText } from "../../lib/github";
import { formatRelativeAge, shortSha } from "../../lib/format";
import { Button } from "../ui/Button";
import { EmptyState, Spinner } from "../ui/Panel";

export interface RecentCommitsCardProps {
  native: boolean;
  connected: boolean;
  defaultRepository: { owner: string; repo: string; branch: string | null } | null;
  onOpenGitHub: () => void;
}

/**
 * Overview commit feed: bounded list for one repository's default branch,
 * with an explicit scope label. Disconnected GitHub never pretends to be zero.
 */
export function RecentCommitsCard({
  native,
  connected,
  defaultRepository,
  onOpenGitHub,
}: RecentCommitsCardProps) {
  const [commits, setCommits] = useState<GitHubCommitSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!native || !connected || !defaultRepository) {
      setCommits([]);
      setLoading(false);
      setError("");
      return;
    }
    let active = true;
    setLoading(true);
    setError("");
    void desktop
      .githubListCommits(
        defaultRepository.owner,
        defaultRepository.repo,
        defaultRepository.branch ?? undefined,
      )
      .then((rows) => {
        if (active) setCommits(rows.slice(0, 6));
      })
      .catch((reason: unknown) => {
        if (active) setError(githubErrorText(reason));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [native, connected, defaultRepository]);

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
      ) : commits.length === 0 ? (
        <EmptyState title="No commits loaded">
          <p className="empty-state-text">
            The requested scope returned nothing yet — or the repository is new.
          </p>
        </EmptyState>
      ) : (
        <>
          <ul className="commit-feed">
            {commits.map((commit) => (
              <li className="commit-feed-item" key={commit.sha}>
                <span className="commit-avatar" aria-hidden="true">
                  {commit.authorLogin ? commit.authorLogin.slice(0, 1).toUpperCase() : "?"}
                </span>
                <div className="commit-feed-copy">
                  <p className="commit-feed-subject">{commit.subject}</p>
                  <p className="commit-feed-meta">
                    <code>{shortSha(commit.sha)}</code> ·{" "}
                    {commit.authorLogin ?? (commit.authorId ? "linked author" : "unattributed")} ·{" "}
                    {formatRelativeAge(commit.authoredAt)}
                  </p>
                </div>
              </li>
            ))}
          </ul>
          <p className="commit-scope">
            {defaultRepository
              ? `${commits.length} loaded · ${defaultRepository.owner}/${defaultRepository.repo}${
                  defaultRepository.branch ? ` · ${defaultRepository.branch}` : ""
                } · not an account-wide total`
              : "Loaded from the connected account"}
          </p>
        </>
      )}
    </section>
  );
}