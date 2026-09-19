/* eslint-disable react-refresh/only-export-components */
import { useEffect, useState } from "react";
import { AlertTriangle, ExternalLink, FolderPlus, Loader2 } from "lucide-react";
import {
  desktop,
  type RepositoryCreationReview,
} from "../../lib/desktop";
import { githubErrorCopyOf, githubErrorText } from "../../lib/github";
import { Button } from "../ui/Button";
import { Checkbox } from "../ui/Checkbox";
import { Modal, ModalActions } from "../ui/Modal";
import { Field } from "../ui/Select";

export interface NewRepositoryDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  native: boolean;
  refreshRepositories: () => void;
}

type Visibility = "private" | "public";
type Phase = "form" | "review" | "confirming" | "done" | "error";

/**
 * Rust-owned prepare → review → confirm repository creation. The dialog never
 * sees or constructs an API URL; it only reviews non-secret fields returned by
 * `github_prepare_repository_creation` and confirms the opaque review ID.
 */
export function NewRepositoryDialog({
  open,
  onOpenChange,
  native,
  refreshRepositories,
}: NewRepositoryDialogProps) {
  const [phase, setPhase] = useState<Phase>("form");
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [visibility, setVisibility] = useState<Visibility>("private");
  const [initializeReadme, setInitializeReadme] = useState(true);
  const [review, setReview] = useState<RepositoryCreationReview | null>(null);
  const [error, setError] = useState("");
  const [createdUrl, setCreatedUrl] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setPhase("form");
      setName("");
      setDescription("");
      setVisibility("private");
      setInitializeReadme(true);
      setReview(null);
      setError("");
      setCreatedUrl(null);
    }
  }, [open]);

  async function prepare() {
    if (!native || !name.trim() || phase === "confirming") return;
    setError("");
    setPhase("confirming");
    try {
      const result = await desktop.githubPrepareRepositoryCreation({
        name: name.trim(),
        description: description.trim() ? description : null,
        private: visibility === "private",
        initializeReadme,
      });
      setReview(result);
      setPhase("review");
    } catch (reason) {
      setError(githubErrorText(reason));
      setPhase("error");
    }
  }

  async function confirm() {
    if (!review || phase === "confirming") return;
    setError("");
    setPhase("confirming");
    try {
      const created = await desktop.githubConfirmRepositoryCreation(review.reviewId);
      setCreatedUrl(created.htmlUrl);
      setPhase("done");
      refreshRepositories();
    } catch (reason) {
      const category = categoryOf(reason);
      if (category === "creation_outcome_unknown") {
        // Dispatch reached GitHub but the response was lost. Offer inspection
        // without implying success or retrying the create automatically.
        setCreatedUrl(`https://github.com/${review.owner}?tab=repositories`);
        setError(
          "GitHub received the request, but the response was lost, so the outcome is unknown. Check whether the repository exists before trying again.",
        );
      } else {
        setError(githubErrorText(reason));
      }
      setPhase("error");
    }
  }

  function categoryOf(reason: unknown): string | null {
    if (typeof reason !== "object" || reason === null) return null;
    const category = (reason as { category?: unknown }).category;
    return typeof category === "string" ? category : null;
  }

  const canPrepare = Boolean(name.trim()) && phase !== "confirming" && native;
  const pending = phase === "confirming";

  const title =
    phase === "done"
      ? "Repository created"
      : phase === "error"
        ? "Repository not confirmed"
        : phase === "review"
          ? "Review before creating"
          : "Create a repository";

  return (
    <Modal
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      description={
        phase === "done"
          ? "Created on GitHub for your personal account."
          : phase === "error"
            ? "Nothing was changed unless the outcome banner says otherwise."
            : "Private by default. Creation happens only after you review the exact details."
      }
      width="lg"
      footer={
        <>
          {error && (
            <p className="form-error" role="alert">
              {error}
            </p>
          )}
          <ModalActions>
            {phase === "done" ? (
              <Button variant="primary" onClick={() => onOpenChange(false)}>
                Done
              </Button>
            ) : phase === "review" ? (
              <>
                <Button variant="ghost" onClick={() => setPhase("form")} disabled={pending}>
                  Back
                </Button>
                <Button
                  variant="primary"
                  onClick={() => void confirm()}
                  disabled={pending}
                  className="confirm-create"
                >
                  {pending ? (
                    <>
                      <Loader2 size={14} className="spin" /> Creating…
                    </>
                  ) : review?.private ? (
                    "Create private repository"
                  ) : (
                    "Create public repository"
                  )}
                </Button>
              </>
            ) : phase === "error" ? (
              <>
                <Button variant="ghost" onClick={() => onOpenChange(false)}>
                  Close
                </Button>
                {!error.toLowerCase().includes("unknown") && (
                  <Button variant="primary" onClick={() => void prepare()} disabled={pending}>
                    Retry
                  </Button>
                )}
              </>
            ) : (
              <Button variant="primary" onClick={() => void prepare()} disabled={!canPrepare}>
                {pending ? "Preparing…" : "Continue to review"}
              </Button>
            )}
          </ModalActions>
        </>
      }
    >
      {phase === "form" && (
        <form
          className="task-editor-form"
          onSubmit={(event) => {
            event.preventDefault();
            void prepare();
          }}
        >
          <Field label="Repository name">
            <input
              className="input"
              value={name}
              onChange={(event) => setName(event.target.value)}
              placeholder="e.g. personal-notes"
              autoFocus
              maxLength={100}
              required
            />
          </Field>
          <Field label="Description">
            <textarea
              className="input textarea"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              placeholder="Optional short description"
              rows={2}
            />
          </Field>
          <div className="task-editor-grid">
            <Field label="Visibility">
              <div className="segmented" role="group" aria-label="Visibility">
                <button
                  type="button"
                  className={visibility === "private" ? "segmented-active" : ""}
                  aria-pressed={visibility === "private"}
                  onClick={() => setVisibility("private")}
                >
                  Private
                </button>
                <button
                  type="button"
                  className={visibility === "public" ? "segmented-active" : ""}
                  aria-pressed={visibility === "public"}
                  onClick={() => setVisibility("public")}
                >
                  Public
                </button>
              </div>
            </Field>
            <Field label="Initialization">
              <label className="inline-checkbox">
                <Checkbox
                  checked={initializeReadme}
                  onCheckedChange={setInitializeReadme}
                  label="Initialize with a README"
                />
                <span>Add a README on creation</span>
              </label>
            </Field>
          </div>
        </form>
      )}

      {phase === "review" && review && (
        <div className="creation-review">
          {!review.private && (
            <div className="public-warning" role="alert">
              <AlertTriangle size={15} />
              <span>
                <strong>Public repository</strong> — anyone can see this repository
                and its name and description once created. There is no undo from
                Ellie.
              </span>
            </div>
          )}
          <dl className="review-grid">
            <div>
              <dt>Owner</dt>
              <dd>{review.owner}</dd>
            </div>
            <div>
              <dt>Name</dt>
              <dd>{review.name}</dd>
            </div>
            <div>
              <dt>Visibility</dt>
              <dd>{review.private ? "Private" : "Public"}</dd>
            </div>
            <div>
              <dt>README</dt>
              <dd>{review.initializeReadme ? "Yes" : "No"}</dd>
            </div>
            {review.description && (
              <div className="review-grid-wide">
                <dt>Description</dt>
                <dd>{review.description}</dd>
              </div>
            )}
          </dl>
          <p className="modal-note">
            This review expires{" "}
            {new Date(review.expiresAt).toLocaleTimeString([], {
              hour: "numeric",
              minute: "2-digit",
            })}{" "}
            local time. Changing any detail above creates a fresh review.
          </p>
        </div>
      )}

      {phase === "done" && createdUrl && (
        <div className="creation-result">
          <p className="creation-result-icon" aria-hidden="true">
            <FolderPlus size={20} />
          </p>
          <p>
            <strong>{review?.owner}/{review?.name}</strong> is ready on GitHub.
            Ellie did not clone anything locally.
          </p>
          <a className="external-link" href={createdUrl} target="_blank" rel="noreferrer">
            Open repository on GitHub <ExternalLink size={12} />
          </a>
        </div>
      )}

      {phase === "error" && createdUrl && error.toLowerCase().includes("unknown") && (
        <div className="creation-result creation-result-unknown">
          <p role="alert">
            <AlertTriangle size={15} /> The request reached GitHub, but the result
            was lost. Inspect your repositories to confirm whether it was created
            before trying again. Ellie will not create it a second time on its own.
          </p>
          <a className="external-link" href={createdUrl} target="_blank" rel="noreferrer">
            Inspect repositories on GitHub <ExternalLink size={12} />
          </a>
        </div>
      )}
    </Modal>
  );
}

/** Friendly single-line hint used by the GitHub page banner. */
export function creationHint(): string {
  return githubErrorCopyOf("creation_outcome_unknown");
}