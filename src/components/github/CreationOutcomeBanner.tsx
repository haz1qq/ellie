import { AlertTriangle, ExternalLink } from "lucide-react";
import { useEffect, useState } from "react";
import {
  desktop,
  type RepositoryCreationAttemptStatus,
} from "../../lib/desktop";
import { githubErrorText } from "../../lib/github";
import { formatAge } from "../../lib/format";
import { Button } from "../ui/Button";

/**
 * Surfaces persisted `outcome_unknown` creation attempts after restart or
 * lost responses. Resolution is an explicit local acknowledgement and never
 * issues another create request.
 */
export function CreationOutcomeBanner({
  native,
  connected,
}: {
  native: boolean;
  connected: boolean;
}) {
  const [attempts, setAttempts] = useState<RepositoryCreationAttemptStatus[]>([]);
  const [error, setError] = useState("");
  const [resolving, setResolving] = useState<string | null>(null);

  useEffect(() => {
    if (!native) return;
    let active = true;
    desktop
      .githubRepositoryCreationStatus()
      .then((rows) => {
        if (active) setAttempts(rows);
      })
      .catch((reason: unknown) => {
        if (active) setError(githubErrorText(reason));
      });
    return () => {
      active = false;
    };
  }, [native]);

  async function resolve(
    attemptId: string,
    resolution: "exists" | "not_found",
  ) {
    setResolving(attemptId);
    setError("");
    try {
      await desktop.githubResolveRepositoryCreation(attemptId, resolution);
      setAttempts((current) =>
        current.filter((attempt) => attempt.attemptId !== attemptId),
      );
    } catch (reason) {
      setError(githubErrorText(reason));
    } finally {
      setResolving(null);
    }
  }

  if (attempts.length === 0) return null;
  return (
    <section className="outcome-banner" aria-label="Repository creation outcome unknown">
      <header className="outcome-banner-header">
        <AlertTriangle size={15} />
        <strong>Creation outcome unknown</strong>
      </header>
      <p>
        A previous request reached GitHub, but Ellie never saw the result. Check
        whether the repository exists before trying again — Ellie will not create
        it twice on its own.
      </p>
      <ul className="outcome-list">
        {attempts.map((attempt) => (
          <li key={attempt.attemptId}>
            <div className="outcome-attempt-copy">
              <span>
                {attempt.owner}/{attempt.name} · {formatAge(attempt.createdAt)} ago
              </span>
              <a href={attempt.repositoryUrl} target="_blank" rel="noreferrer">
                Inspect <ExternalLink size={11} />
              </a>
            </div>
            <div className="outcome-resolution" aria-label={`Resolve ${attempt.owner}/${attempt.name}`}>
              <Button
                size="sm"
                variant="ghost"
                disabled={!connected || resolving === attempt.attemptId}
                onClick={() => void resolve(attempt.attemptId, "exists")}
              >
                Repository exists
              </Button>
              <Button
                size="sm"
                variant="ghost"
                disabled={!connected || resolving === attempt.attemptId}
                onClick={() => void resolve(attempt.attemptId, "not_found")}
              >
                It does not exist
              </Button>
            </div>
          </li>
        ))}
      </ul>
      {!connected && (
        <p className="outcome-hint">
          Reconnect the same GitHub account to resolve this local warning.
        </p>
      )}
      {error && (
        <p className="outcome-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}