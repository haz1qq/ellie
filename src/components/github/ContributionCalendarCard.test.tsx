import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { desktop } from "../../lib/desktop";
import {
  contributionQueryOf,
  yearOptions,
} from "../../lib/githubContributions";
import { ContributionCalendarCard } from "./ContributionCalendarCard";

vi.mock("../../lib/desktop", () => ({
  desktop: {
    githubContributionCalendar: vi.fn(),
  },
}));

const calendarFixture = {
  totalContributions: 3,
  startedOn: "2026-09-06",
  endedOn: "2026-09-07",
  weeks: [
    {
      firstDay: "2026-09-06",
      days: [
        { date: "2026-09-06", contributionCount: 0, level: 0 as const, weekday: 0 },
        { date: "2026-09-07", contributionCount: 3, level: 4 as const, weekday: 1 },
      ],
    },
  ],
};

function renderCard(overrides: {
  native?: boolean;
  connected?: boolean;
  login?: string | null;
} = {}) {
  return render(
    <ContributionCalendarCard
      native={overrides.native ?? true}
      connected={overrides.connected ?? true}
      login={overrides.login ?? "octocat"}
      onOpenGitHub={() => {}}
    />,
  );
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.githubContributionCalendar).mockResolvedValue(calendarFixture);
});

describe("ContributionCalendarCard", () => {
  it("loads and renders the account-wide GitHub profile calendar", async () => {
    renderCard();
    expect(await screen.findByText(/3 contributions in the last year/)).toBeVisible();
    expect(screen.getByText(/Account-wide activity GitHub counts for @octocat/)).toBeVisible();
    expect(
      screen.getByRole("img", {
        name: /3 GitHub profile contributions for octocat from 2026-09-06 through 2026-09-07/,
      }),
    ).toBeVisible();
    expect(desktop.githubContributionCalendar).toHaveBeenCalledTimes(1);
  });

  it("shows an invitation without inventing a zero while disconnected", () => {
    renderCard({ connected: false, login: null });
    expect(screen.getByText("GitHub not connected")).toBeVisible();
    expect(screen.queryByText(/0 contributions/)).not.toBeInTheDocument();
    expect(desktop.githubContributionCalendar).not.toHaveBeenCalled();
  });

  it("shows a typed redacted error and offers a retry", async () => {
    vi.mocked(desktop.githubContributionCalendar).mockRejectedValue({ category: "rate_limited" });
    renderCard();
    expect(await screen.findByText(/rate-limit/)).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button", { name: /Try again/ }));
    expect(desktop.githubContributionCalendar).toHaveBeenCalledTimes(2);
  });

  it("refetches when the user clicks Refresh", async () => {
    renderCard();
    await screen.findByText(/3 contributions in the last year/);
    await userEvent.setup().click(screen.getByRole("button", { name: /Refresh/ }));
    expect(desktop.githubContributionCalendar).toHaveBeenCalledTimes(2);
  });

  it("maps the yearly filter to the typed IPC query", async () => {
    renderCard();
    await screen.findByText(/3 contributions in the last year/);
    expect(desktop.githubContributionCalendar).toHaveBeenLastCalledWith({});
    expect(contributionQueryOf("rolling")).toEqual({});
    expect(contributionQueryOf("2025")).toEqual({ year: 2025 });
    expect(yearOptions().some((option) => option.value === "2025")).toBe(true);
    expect(yearOptions()[0]).toEqual({ value: "rolling", label: "Last 12 months" });
  });
});