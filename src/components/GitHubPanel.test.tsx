import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GitHubPanel } from "./GitHubPanel";
import { desktop } from "../lib/desktop";
import type { GitHubConnectionApi } from "../lib/github";

vi.mock("../lib/desktop", () => ({
  desktop: {
    githubListRepositories: vi.fn(),
    githubListCommits: vi.fn(),
    githubRepositoryCreationStatus: vi.fn(),
    githubResolveRepositoryCreation: vi.fn(),
    githubPrepareRepositoryCreation: vi.fn(),
    githubConfirmRepositoryCreation: vi.fn(),
  },
}));

const connection: GitHubConnectionApi = {
  status: null,
  error: "",
  busy: false,
  signInPending: false,
  refresh: vi.fn(async () => true),
  saveClientId: vi.fn(async () => true),
  saveClientSecret: vi.fn(async () => true),
  signIn: vi.fn(async () => true),
  cancelSignIn: vi.fn(async () => true),
  disconnect: vi.fn(async () => true),
};

const connectedStatus = {
  state: "Connected" as const,
  account: { id: 7, login: "octocat" },
  lastError: null,
  tokenPresent: true,
  clientIdConfigured: true,
  clientSecretConfigured: true,
};

function renderPanel(overrides: Partial<typeof connection> = {}, status: null | { state: "Disconnected" | "Connected"; account: { id: number; login: string } | null; lastError: null; tokenPresent: boolean; clientIdConfigured: boolean; clientSecretConfigured: boolean } = null) {
  const merged = { ...connection, ...overrides };
  return render(
    <GitHubPanel
      native
      connection={{ ...merged, status }}
      onOpenSettings={() => {}}
      createRequest={0}
      onConsumeCreateRequest={() => {}}
      onRepositoriesChanged={() => {}}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.githubListRepositories).mockResolvedValue([]);
  vi.mocked(desktop.githubListCommits).mockResolvedValue([]);
  vi.mocked(desktop.githubRepositoryCreationStatus).mockResolvedValue([]);
  vi.mocked(desktop.githubResolveRepositoryCreation).mockResolvedValue(undefined);
});

describe("GitHubPanel", () => {
  it("shows an invitation with the New repository action disabled while disconnected", () => {
    renderPanel({}, {
      state: "Disconnected",
      account: null,
      lastError: null,
      tokenPresent: false,
      clientIdConfigured: false,
      clientSecretConfigured: false,
    });
    expect(screen.getByText("Not connected")).toBeVisible();
    expect(screen.getByRole("button", { name: "Connect GitHub" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "New repository" })).toBeDisabled();
    expect(screen.getByText("Connect to load repositories")).toBeVisible();
  });

  it("loads and selects repositories for a connected account", async () => {
    vi.mocked(desktop.githubListRepositories).mockResolvedValue([
      { id: 1, name: "ellie", fullName: "octocat/ellie", private: true, defaultBranch: "main", htmlUrl: "https://github.com/octocat/ellie" },
      { id: 2, name: "notes", fullName: "octocat/notes", private: false, defaultBranch: "main", htmlUrl: "https://github.com/octocat/notes" },
    ]);
    vi.mocked(desktop.githubListCommits).mockResolvedValue([
      { sha: "abc1234def5678ab", subject: "Add the command center", authorId: 7, authorLogin: "octocat", authoredAt: new Date().toISOString(), committedAt: new Date().toISOString() },
    ]);
    renderPanel({}, connectedStatus);
    expect(await screen.findByText("octocat/ellie")).toBeVisible();
    expect(screen.getByText("2 loaded")).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button", { name: /octocat\/notes/ }));
    await waitFor(() =>
      expect(desktop.githubListCommits).toHaveBeenCalledWith("octocat", "notes", undefined),
    );
    expect(await screen.findByText("Add the command center")).toBeVisible();
    expect(screen.getByText(/1 loaded · octocat\/notes · main · locally counted, not an account-wide total/)).toBeVisible();
    expect(screen.getByText(/1 of 1 commits attributed to octocat by GitHub/)).toBeVisible();
  });

  it("counts only commits linked to the connected GitHub account", async () => {
    vi.mocked(desktop.githubListRepositories).mockResolvedValue([
      { id: 1, name: "ellie", fullName: "octocat/ellie", private: true, defaultBranch: "main", htmlUrl: "https://github.com/octocat/ellie" },
    ]);
    vi.mocked(desktop.githubListCommits).mockResolvedValue([
      { sha: "a".repeat(40), subject: "Mine", authorId: 7, authorLogin: "octocat", authoredAt: new Date().toISOString(), committedAt: new Date().toISOString() },
      { sha: "b".repeat(40), subject: "Collaborator", authorId: 8, authorLogin: "friend", authoredAt: new Date().toISOString(), committedAt: new Date().toISOString() },
    ]);

    renderPanel({}, connectedStatus);

    expect(await screen.findByText("Collaborator")).toBeVisible();
    expect(screen.getByText(/1 of 2 commits attributed to octocat by GitHub/)).toBeVisible();
  });

  it("paginates bounded commits", async () => {
    vi.mocked(desktop.githubListRepositories).mockResolvedValue([
      { id: 1, name: "ellie", fullName: "octocat/ellie", private: true, defaultBranch: "main", htmlUrl: "https://github.com/octocat/ellie" },
    ]);
    vi.mocked(desktop.githubListCommits).mockResolvedValue(
      Array.from({ length: 11 }, (_, index) => ({
        sha: (index + 1).toString(16).padStart(40, "0"),
        subject: `Commit ${index + 1}`,
        authorId: 7,
        authorLogin: "octocat",
        authoredAt: new Date().toISOString(),
        committedAt: new Date().toISOString(),
      })),
    );

    renderPanel({}, connectedStatus);

    expect(await screen.findByText("Commit 10")).toBeVisible();
    expect(screen.queryByText("Commit 11")).not.toBeInTheDocument();
    expect(screen.getByText("Page 1 of 2 · 11 loaded")).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button", { name: /Next/ }));
    expect(await screen.findByText("Commit 11")).toBeVisible();
    expect(screen.queryByText("Commit 1")).not.toBeInTheDocument();
  });

  it("labels the loaded scope instead of claiming an account-wide total", async () => {
    vi.mocked(desktop.githubListRepositories).mockResolvedValue([
      { id: 1, name: "ellie", fullName: "octocat/ellie", private: true, defaultBranch: "main", htmlUrl: "https://github.com/octocat/ellie" },
    ]);
    vi.mocked(desktop.githubListCommits).mockResolvedValue([
      { sha: "abc1234def5678ab", subject: "Fix reset labels", authorId: null, authorLogin: null, authoredAt: new Date().toISOString(), committedAt: new Date().toISOString() },
    ]);
    renderPanel({}, connectedStatus);
    expect(await screen.findByText("Fix reset labels")).toBeVisible();
    expect(await screen.findByText(/unattributed/)).toBeVisible();
    expect(screen.getByText(/1 loaded · octocat\/ellie · main · locally counted, not an account-wide total/)).toBeVisible();
  });

  it("keeps commit history separate from a disconnected state", () => {
    renderPanel({}, {
      state: "Disconnected",
      account: null,
      lastError: null,
      tokenPresent: true,
      clientIdConfigured: true,
      clientSecretConfigured: true,
    });
    expect(screen.getByText("Commits appear after connecting")).toBeVisible();
    expect(screen.queryByText("0 loaded")).not.toBeInTheDocument();
  });

  it("surfaces a creation outcome banner from persisted attempts", async () => {
    vi.mocked(desktop.githubRepositoryCreationStatus).mockResolvedValue([
      {
        attemptId: "attempt-1",
        owner: "octocat",
        name: "mystery-repo",
        state: "outcome_unknown",
        repositoryUrl: "https://github.com/octocat",
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    ]);
    renderPanel({}, connectedStatus);
    expect(await screen.findByText(/Creation outcome unknown/)).toBeVisible();
    expect(screen.getByRole("link", { name: /Inspect/ })).toHaveAttribute("href", "https://github.com/octocat");
    await userEvent.setup().click(screen.getByRole("button", { name: "Repository exists" }));
    await waitFor(() =>
      expect(desktop.githubResolveRepositoryCreation).toHaveBeenCalledWith(
        "attempt-1",
        "exists",
      ),
    );
    expect(screen.queryByText("mystery-repo")).not.toBeInTheDocument();
  });
});