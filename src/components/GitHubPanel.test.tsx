import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GitHubPanel } from "./GitHubPanel";
import { desktop, type GitHubConnectionStatus, type GitHubCommitSummary, type GitHubRepositorySummary } from "../lib/desktop";

vi.mock("../lib/desktop", () => ({
  desktop: {
    githubConnectionStatus: vi.fn(),
    githubSaveClientId: vi.fn(),
    githubSignIn: vi.fn(),
    githubCancelSignIn: vi.fn(),
    githubDisconnect: vi.fn(),
    githubListRepositories: vi.fn(),
    githubListCommits: vi.fn(),
  },
}));

const disconnected: GitHubConnectionStatus = {
  state: "Disconnected",
  account: null,
  lastError: null,
  tokenPresent: false,
  clientIdConfigured: true,
};
const connected: GitHubConnectionStatus = {
  state: "Connected",
  account: { id: 42, login: "octo-cat" },
  lastError: null,
  tokenPresent: true,
  clientIdConfigured: true,
};
const authorizing: GitHubConnectionStatus = {
  state: "Authorizing",
  account: null,
  lastError: null,
  tokenPresent: false,
  clientIdConfigured: true,
};
const repositories: GitHubRepositorySummary[] = [
  {
    id: 7,
    name: "ellie",
    fullName: "octo-cat/ellie",
    private: true,
    defaultBranch: "main",
    htmlUrl: "https://github.com/octo-cat/ellie",
  },
  {
    id: 9,
    name: "notes",
    fullName: "octo-cat/notes",
    private: false,
    defaultBranch: "trunk",
    htmlUrl: "https://github.com/octo-cat/notes",
  },
];
const commits: GitHubCommitSummary[] = [
  {
    sha: "0123456789abcdef0123456789abcdef01234567",
    subject: "Fix <b>display</b>",
    authorId: 42,
    authorLogin: "octo-cat",
    authoredAt: "2026-01-02T03:04:05Z",
    committedAt: "2026-01-02T03:04:05Z",
  },
  {
    sha: "fedcba9876543210fedcba9876543210fedcba98",
    subject: "Add tests",
    authorId: null,
    authorLogin: null,
    authoredAt: "2026-01-01T10:00:00Z",
    committedAt: "2026-01-01T11:00:00Z",
  },
];

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(disconnected);
  vi.mocked(desktop.githubListRepositories).mockResolvedValue([]);
  vi.mocked(desktop.githubListCommits).mockResolvedValue([]);
  vi.mocked(desktop.githubCancelSignIn).mockResolvedValue(disconnected);
});

describe("GitHub connection banner", () => {
  it("invites a connection without inventing any data", async () => {
    render(<GitHubPanel native />);
    expect(
      await screen.findByText(/Connect your GitHub account to browse repositories/),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Connect GitHub account" })).toBeEnabled();
    expect(screen.getByText(/not an empty account/)).toBeVisible();
    expect(desktop.githubListRepositories).not.toHaveBeenCalled();
    expect(desktop.githubListCommits).not.toHaveBeenCalled();
    expect(screen.queryByText(/Connected as @octo-cat/)).toBeNull(); // no invented login
  });

  it("gates connecting until a Client ID is configured and explains why", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue({
      ...disconnected,
      clientIdConfigured: false,
    });
    render(<GitHubPanel native />);
    const connect = await screen.findByRole("button", { name: "Connect GitHub account" });
    expect(connect).toBeDisabled();
    expect(screen.getByText(/Settings → GitHub before connecting/)).toBeVisible();
  });

  it("shows the authorizing state with a working cancel action", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(authorizing);
    const user = userEvent.setup();
    render(<GitHubPanel native />);
    expect(await screen.findByText(/Waiting for GitHub…/)).toBeVisible();
    const cancel = screen.getByRole("button", { name: "Cancel sign-in" });
    expect(cancel).toBeEnabled();
    await user.click(cancel);
    expect(desktop.githubCancelSignIn).toHaveBeenCalledTimes(1);
    expect(
      await screen.findByText(/Connect your GitHub account to browse repositories/),
    ).toBeVisible();
  });

  it("maps the persisted lastError to friendly copy and never shows the raw category", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue({
      ...disconnected,
      lastError: "rate_limited",
    });
    render(<GitHubPanel native />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "rate-limiting requests",
    );
    expect(screen.queryByText("rate_limited")).not.toBeInTheDocument();
  });

  it("keeps the browser preview inert with no backend read", () => {
    render(<GitHubPanel native={false} />);
    expect(screen.getByText("Open the desktop app to connect GitHub.")).toBeVisible();
    expect(desktop.githubConnectionStatus).not.toHaveBeenCalled();
    expect(desktop.githubListRepositories).not.toHaveBeenCalled();
    expect(screen.getAllByText(/Open the desktop app/).length).toBeGreaterThan(1);
  });
});

describe("GitHub repository list", () => {
  it("lists repositories with visibility, default branch, and URL after connecting", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    vi.mocked(desktop.githubListRepositories).mockResolvedValue(repositories);
    render(<GitHubPanel native />);
    expect(await screen.findByText(/Connected as @octo-cat/)).toBeVisible();
    expect(await screen.findByRole("heading", { name: "ellie" })).toBeVisible();
    expect(screen.getByText("Private")).toBeVisible();
    expect(screen.getByText("Public")).toBeVisible();
    expect(screen.getByText("Default branch: main")).toBeVisible();
    expect(screen.getByText("Default branch: trunk")).toBeVisible();
    expect(screen.getByText("https://github.com/octo-cat/ellie")).toBeVisible();
    expect(screen.getByText("2 shown for the connected account")).toBeVisible();
  });

  it("labels the loading state without substituting numbers", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    const repos = deferred<GitHubRepositorySummary[]>();
    vi.mocked(desktop.githubListRepositories).mockReturnValue(repos.promise);
    render(<GitHubPanel native />);
    expect(await screen.findByText("Loading repositories…")).toBeVisible();
    expect(screen.queryByText(/No repositories found/)).not.toBeInTheDocument();
    await act(async () => repos.resolve(repositories));
    expect(await screen.findByRole("heading", { name: "ellie" })).toBeVisible();
  });

  it("shows a friendly repository error with a retry and never claims a count", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    vi.mocked(desktop.githubListRepositories)
      .mockRejectedValueOnce({ category: "network_unavailable" })
      .mockResolvedValue(repositories);
    const user = userEvent.setup();
    render(<GitHubPanel native />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "GitHub isn’t reachable",
    );
    expect(screen.queryByText(/No repositories found/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Reload repositories" }));
    expect(await screen.findByRole("heading", { name: "ellie" })).toBeVisible();
  });

  it("shows an explicit zero only for a completed empty success", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    vi.mocked(desktop.githubListRepositories).mockResolvedValue([]);
    render(<GitHubPanel native />);
    expect(
      await screen.findByText(
        "No repositories found for this account. GitHub returned an empty list.",
      ),
    ).toBeVisible();
  });
});

describe("GitHub commit history", () => {
  async function renderConnected() {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    vi.mocked(desktop.githubListRepositories).mockResolvedValue(repositories);
    vi.mocked(desktop.githubListCommits).mockResolvedValue(commits);
    const user = userEvent.setup();
    const result = render(<GitHubPanel native />);
    await screen.findByRole("heading", { name: "ellie" });
    return { user, result };
  }

  it("loads commits for the picked repository with a scoped coverage label", async () => {
    const { user } = await renderConnected();
    await user.selectOptions(screen.getByLabelText("Repository"), "7");
    expect(desktop.githubListCommits).toHaveBeenCalledWith("octo-cat", "ellie", undefined);
    expect(
      await screen.findByText(
        /2 loaded commits · octo-cat\/ellie · default branch/,
      ),
    ).toBeVisible();
    expect(screen.getByText("Fix <b>display</b>")).toBeVisible();
    expect(screen.getByText("0123456")).toBeVisible();
    expect(screen.getByText(/@octo-cat · .*2026/)).toBeVisible();
    expect(screen.getByText(/Unattributed · .*2026/)).toBeVisible();
    // GitHub-provided strings are plain text, never markup.
    expect(document.querySelector("b")).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
  });

  it("sends an optional branch and labels the scope precisely", async () => {
    const { user } = await renderConnected();
    await user.selectOptions(screen.getByLabelText("Repository"), "7");
    await screen.findByText(/2 loaded commits/);
    await user.type(screen.getByLabelText("Branch (optional)"), "feat/w5");
    await user.click(screen.getByRole("button", { name: "Load commits" }));
    await waitFor(() =>
      expect(desktop.githubListCommits).toHaveBeenLastCalledWith(
        "octo-cat",
        "ellie",
        "feat/w5",
      ),
    );
    expect(
      await screen.findByText(/2 loaded commits · octo-cat\/ellie @ feat\/w5/),
    ).toBeVisible();
    expect(
      screen.getByText(/never an account-wide or pushed count/),
    ).toBeVisible();
  });

  it("shows zero commits only for a completed empty success", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    vi.mocked(desktop.githubListRepositories).mockResolvedValue(repositories);
    vi.mocked(desktop.githubListCommits).mockResolvedValue([]);
    const user = userEvent.setup();
    render(<GitHubPanel native />);
    await screen.findByRole("heading", { name: "ellie" });
    await user.selectOptions(screen.getByLabelText("Repository"), "7");
    expect(
      await screen.findByText(/0 loaded commits · octo-cat\/ellie · default branch/),
    ).toBeVisible();
    expect(
      screen.getByText(/0 commits loaded for this scope\./),
    ).toBeVisible();
  });

  it("keeps the previous list visible when the same-scope reload fails", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    vi.mocked(desktop.githubListRepositories).mockResolvedValue(repositories);
    const reload = deferred<GitHubCommitSummary[]>();
    vi.mocked(desktop.githubListCommits)
      .mockResolvedValueOnce(commits)
      .mockReturnValueOnce(reload.promise);
    const user = userEvent.setup();
    render(<GitHubPanel native />);
    await screen.findByRole("heading", { name: "ellie" });
    await user.selectOptions(screen.getByLabelText("Repository"), "7");
    await screen.findByText(/2 loaded commits/);
    await user.click(screen.getByRole("button", { name: "Load commits" }));
    expect(await screen.findByText(/Refreshing… showing the previous list/)).toBeVisible();
    await act(async () => reload.reject({ category: "rate_limited" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "rate-limiting requests",
    );
    expect(screen.getByText(/Showing previously loaded commits/)).toBeVisible();
    expect(screen.getByText("Fix <b>display</b>")).toBeVisible();
    expect(
      screen.getByText(/2 loaded commits · octo-cat\/ellie · default branch/),
    ).toBeVisible();
  });

  it("shows a friendly error and retry for a first load failure", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    vi.mocked(desktop.githubListRepositories).mockResolvedValue(repositories);
    vi.mocked(desktop.githubListCommits)
      .mockRejectedValueOnce({ category: "authentication_expired" })
      .mockResolvedValue(commits);
    const user = userEvent.setup();
    render(<GitHubPanel native />);
    await screen.findByRole("heading", { name: "ellie" });
    await user.selectOptions(screen.getByLabelText("Repository"), "7");
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your GitHub session expired",
    );
    expect(screen.queryByText(/0 commits loaded/)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry load" }));
    expect(await screen.findByText(/2 loaded commits/)).toBeVisible();
  });

  it("renders no provider or quota data representations", async () => {
    await renderConnected();
    expect(screen.getByRole("heading", { name: "GitHub" })).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Current usage" })).toBeNull();
    expect(screen.queryByRole("heading", { name: "Providers" })).toBeNull();
    expect(screen.queryByRole("progressbar")).toBeNull();
    expect(screen.queryByText(/\d+% (used|remaining)/)).toBeNull();
    expect(screen.queryByText(/Resets/)).toBeNull();
  });
});