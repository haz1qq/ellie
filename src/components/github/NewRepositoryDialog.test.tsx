import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { desktop } from "../../lib/desktop";
import { NewRepositoryDialog } from "./NewRepositoryDialog";

vi.mock("../../lib/desktop", () => ({
  desktop: {
    githubPrepareRepositoryCreation: vi.fn(),
    githubConfirmRepositoryCreation: vi.fn(),
    githubListRepositories: vi.fn(),
  },
}));

function openDialog() {
  const refresh = vi.fn();
  render(<NewRepositoryDialog open onOpenChange={() => {}} native refreshRepositories={refresh} />);
  return refresh;
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.githubListRepositories).mockResolvedValue([]);
});

describe("NewRepositoryDialog", () => {
  it("is private by default and asks for review before creating", async () => {
    vi.mocked(desktop.githubPrepareRepositoryCreation).mockResolvedValue({
      reviewId: "review-1", owner: "octocat", name: "personal-notes", description: null,
      private: true, initializeReadme: true, expiresAt: "2026-09-08T12:00:00Z",
    });
    const user = userEvent.setup();
    openDialog();
    await user.type(screen.getByLabelText("Repository name"), "personal-notes");
    expect(screen.getByRole("button", { name: "Continue to review" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "Continue to review" }));
    expect(await screen.findByText("Review before creating")).toBeVisible();
    expect(desktop.githubPrepareRepositoryCreation).toHaveBeenCalledWith({
      name: "personal-notes", description: null, private: true, initializeReadme: true,
    });
    expect(screen.queryByText(/Public repository/)).not.toBeInTheDocument();
  });

  it("demands explicit public disclosure on the review step", async () => {
    vi.mocked(desktop.githubPrepareRepositoryCreation).mockResolvedValue({
      reviewId: "review-2", owner: "octocat", name: "open-notes", description: null,
      private: false, initializeReadme: false, expiresAt: "2026-09-08T12:00:00Z",
    });
    const user = userEvent.setup();
    openDialog();
    await user.type(screen.getByLabelText("Repository name"), "open-notes");
    await user.click(screen.getByRole("button", { name: "Public" }));
    await user.click(screen.getByRole("button", { name: "Continue to review" }));
    expect(await screen.findByText("Review before creating")).toBeVisible();
    const warning = await screen.findByRole("alert");
    expect(warning).toHaveTextContent("Public repository");
    expect(desktop.githubPrepareRepositoryCreation).toHaveBeenCalledWith(
      expect.objectContaining({ private: false }),
    );
  });

  it("reports an outcome-unknown result without retrying the remote create", async () => {
    vi.mocked(desktop.githubPrepareRepositoryCreation).mockResolvedValue({
      reviewId: "review-3", owner: "octocat", name: "mystery", description: null,
      private: false, initializeReadme: false, expiresAt: "2026-09-08T12:00:00Z",
    });
    vi.mocked(desktop.githubConfirmRepositoryCreation).mockRejectedValue({
      category: "creation_outcome_unknown",
    });
    const user = userEvent.setup();
    const refresh = openDialog();
    await user.type(screen.getByLabelText("Repository name"), "mystery");
    await user.click(screen.getByRole("button", { name: "Public" }));
    await user.click(screen.getByRole("button", { name: "Continue to review" }));
    await screen.findByText("Review before creating");
    await user.click(screen.getByRole("button", { name: "Create public repository" }));
    expect(await screen.findByText(/outcome is unknown/)).toBeVisible();
    expect(screen.getByRole("link", { name: /Inspect repositories on GitHub/ })).toHaveAttribute(
      "href",
      "https://github.com/octocat?tab=repositories",
    );
    // The dialog must not offer an automatic retry of the remote side effect.
    expect(screen.queryByRole("button", { name: "Retry" })).not.toBeInTheDocument();
    expect(desktop.githubConfirmRepositoryCreation).toHaveBeenCalledTimes(1);
    expect(refresh).not.toHaveBeenCalled();
  });

  it("reports success with a safe GitHub link and refreshes the repo list", async () => {
    vi.mocked(desktop.githubPrepareRepositoryCreation).mockResolvedValue({
      reviewId: "review-4", owner: "octocat", name: "safe-repo", description: null,
      private: true, initializeReadme: true, expiresAt: "2026-09-08T12:00:00Z",
    });
    vi.mocked(desktop.githubConfirmRepositoryCreation).mockResolvedValue({
      id: 9, name: "safe-repo", fullName: "octocat/safe-repo", private: true, defaultBranch: "main",
      htmlUrl: "https://github.com/octocat/safe-repo",
    });
    const user = userEvent.setup();
    const refresh = openDialog();
    await user.type(screen.getByLabelText("Repository name"), "safe-repo");
    await user.click(screen.getByRole("button", { name: "Continue to review" }));
    await screen.findByText("Review before creating");
    await user.click(screen.getByRole("button", { name: "Create private repository" }));
    expect(await screen.findByText("Repository created")).toBeVisible();
    expect(screen.getByRole("link", { name: /Open repository on GitHub/ })).toHaveAttribute(
      "href",
      "https://github.com/octocat/safe-repo",
    );
    await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
  });
});
