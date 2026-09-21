import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { RecentCommitsCard } from "./RecentCommitsCard";
import { desktop, type GitHubCommitSummary } from "../../lib/desktop";

vi.mock("../../lib/desktop", () => ({
  desktop: {
    githubListCommits: vi.fn(),
  },
}));

const repositories = [
  { id: 1, fullName: "haz1qq/ellie", htmlUrl: "https://github.com/haz1qq/ellie" },
  { id: 2, fullName: "haz1qq/jomhadir", htmlUrl: "https://github.com/haz1qq/jomhadir" },
];

function commit(
  sha: string,
  subject: string,
  authoredAt: string,
): GitHubCommitSummary {
  return {
    sha,
    subject,
    authorId: 7,
    authorLogin: "haz1qq",
    authoredAt,
    committedAt: authoredAt,
  };
}

const ellieCommits = [
  commit("1111111111111111111111111111111111111111", "Ellie newest", "2025-09-20T12:00:00Z"),
  commit("3333333333333333333333333333333333333333", "Ellie older", "2025-09-20T10:00:00Z"),
];
const jomhadirCommits = [
  commit("2222222222222222222222222222222222222222", "Jomhadir middle", "2025-09-20T11:00:00Z"),
];

interface RenderOptions {
  native?: boolean;
  connected?: boolean;
  repositoryRows?: typeof repositories;
}

function renderCard(options: RenderOptions = {}) {
  const {
    native = true,
    connected = true,
    repositoryRows = repositories,
  } = options;
  return render(
    <RecentCommitsCard
      native={native}
      connected={connected}
      repositories={repositoryRows}
      onOpenGitHub={() => {}}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.githubListCommits).mockImplementation((_owner, repo) => {
    if (repo === "ellie") return Promise.resolve(ellieCommits);
    if (repo === "jomhadir") return Promise.resolve(jomhadirCommits);
    return Promise.resolve([]);
  });
});

describe("RecentCommitsCard multi-repository feed", () => {
  it("merges recent commits and labels every row with its repository", async () => {
    renderCard();

    expect(await screen.findByText("Ellie newest")).toBeInTheDocument();
    const rows = screen.getAllByRole("listitem");
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("Ellie newest");
    expect(rows[0]).toHaveTextContent("haz1qq/ellie");
    expect(rows[1]).toHaveTextContent("Jomhadir middle");
    expect(rows[1]).toHaveTextContent("haz1qq/jomhadir");
    expect(rows[2]).toHaveTextContent("Ellie older");
    expect(rows[2]).toHaveTextContent("haz1qq/ellie");
    expect(screen.getByText("Across 2 repositories")).toBeInTheDocument();
    expect(screen.getByText(/2 repositories checked/)).toBeInTheDocument();

    expect(desktop.githubListCommits).toHaveBeenCalledWith(
      "haz1qq",
      "ellie",
      undefined,
      3,
    );
    expect(desktop.githubListCommits).toHaveBeenCalledWith(
      "haz1qq",
      "jomhadir",
      undefined,
      3,
    );
  });

  it("keeps successful repository commits when another repository fails", async () => {
    vi.mocked(desktop.githubListCommits).mockImplementation((_owner, repo) => {
      if (repo === "jomhadir") return Promise.reject(new Error("unavailable"));
      return Promise.resolve(ellieCommits);
    });

    renderCard();

    expect(await screen.findByText("Ellie newest")).toBeInTheDocument();
    expect(screen.queryByText("Commits unavailable")).not.toBeInTheDocument();
    expect(screen.getByText(/1 repository checked · 1 unavailable/)).toBeInTheDocument();
  });

  it("shows the disconnected state without loading repository commits", () => {
    renderCard({ connected: false });

    expect(screen.getByText("GitHub not connected")).toBeInTheDocument();
    expect(screen.queryByText("Across 2 repositories")).not.toBeInTheDocument();
    expect(desktop.githubListCommits).not.toHaveBeenCalled();
  });

  it("shows an explicit empty state when no repositories are available", async () => {
    renderCard({ repositoryRows: [] });

    expect(await screen.findByText("No repositories loaded")).toBeInTheDocument();
    await waitFor(() => expect(desktop.githubListCommits).not.toHaveBeenCalled());
  });
});
