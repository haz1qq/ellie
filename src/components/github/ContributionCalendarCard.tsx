import { Activity, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useState, type CSSProperties } from "react";
import {
  desktop,
  type GitHubContributionCalendar,
  type GitHubContributionWeek,
} from "../../lib/desktop";
import {
  contributionQueryOf,
  yearOptions,
  type ContributionYear,
} from "../../lib/githubContributions";
import { githubErrorText } from "../../lib/github";
import { Button } from "../ui/Button";
import { EmptyState, Spinner } from "../ui/Panel";
import { Select } from "../ui/Select";

function dateLabel(value: string): string {
  const date = new Date(`${value}T00:00:00Z`);
  return Number.isNaN(date.getTime())
    ? value
    : date.toLocaleDateString(undefined, {
        month: "short",
        day: "numeric",
        year: "numeric",
        timeZone: "UTC",
      });
}

function monthLabel(week: GitHubContributionWeek, index: number): string | null {
  const firstOfMonth = week.days.find((day) => day.date.endsWith("-01"));
  const value = firstOfMonth?.date ?? (index === 0 ? week.days[0]?.date : undefined);
  if (!value) return null;
  const date = new Date(`${value}T00:00:00Z`);
  return Number.isNaN(date.getTime())
    ? null
    : date.toLocaleDateString(undefined, { month: "short", timeZone: "UTC" });
}

export interface ContributionCalendarCardProps {
  native: boolean;
  connected: boolean;
  login: string | null;
  onOpenGitHub: () => void;
}

/** GitHub's account-wide profile contribution calendar, fetched by Rust. */
export function ContributionCalendarCard({
  native,
  connected,
  login,
  onOpenGitHub,
}: ContributionCalendarCardProps) {
  const [calendar, setCalendar] = useState<GitHubContributionCalendar | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [reload, setReload] = useState(0);
  const [year, setYear] = useState<ContributionYear>("rolling");

  useEffect(() => {
    if (!native || !connected) {
      setCalendar(null);
      setLoading(false);
      setError("");
      return;
    }
    let active = true;
    setLoading(true);
    setError("");
    const query = contributionQueryOf(year);
    void desktop
      .githubContributionCalendar(query)
      .then((value) => {
        if (active) setCalendar(value);
      })
      .catch((reason: unknown) => {
        if (active) {
          setCalendar(null);
          setError(githubErrorText(reason));
        }
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [native, connected, login, reload, year]);

  const weekCount = calendar?.weeks.length ?? 52;
  const gridStyle = useMemo<CSSProperties>(
    () => ({ "--weeks": weekCount } as CSSProperties),
    [weekCount],
  );
  const monthLabels = useMemo(
    () => calendar?.weeks.map(monthLabel) ?? [],
    [calendar],
  );
  const periodTitle = year === "rolling" ? "in the last year" : `in ${year}`;

  return (
    <section
      className="panel contribution-panel"
      aria-labelledby="contribution-calendar-title"
    >
      <header className="panel-header contribution-panel-header">
        <div className="panel-title">
          <span className="panel-kicker">GitHub profile</span>
          <h2 className="panel-heading" id="contribution-calendar-title">
            <Activity size={15} className="panel-heading-icon" />
            {calendar
              ? `${calendar.totalContributions.toLocaleString()} contributions ${periodTitle}`
              : "Contribution activity"}
          </h2>
          {connected && login ? (
            <p className="panel-description contribution-description">
              Account-wide activity GitHub counts for @{login} across accessible repositories.
            </p>
          ) : null}
        </div>
        <div className="panel-actions">
          <Select
            label="Contribution period"
            value={year}
            onValueChange={setYear}
            options={yearOptions()}
            disabled={!connected || !native}
          />
          {connected && native ? (
            <Button
              size="sm"
              variant="ghost"
              disabled={loading}
              onClick={() => setReload((value) => value + 1)}
            >
              <RefreshCw size={13} /> Refresh
            </Button>
          ) : null}
          <Button size="sm" variant="ghost" onClick={onOpenGitHub}>
            Open GitHub
          </Button>
        </div>
      </header>

      {!native ? (
        <EmptyState title="Available in the Windows app">
          <p className="empty-state-text">
            Connect GitHub in Ellie to load your profile activity.
          </p>
        </EmptyState>
      ) : !connected ? (
        <EmptyState icon={<Activity size={18} />} title="GitHub not connected">
          <p className="empty-state-text">
            Connect GitHub to load your real profile contribution calendar. Ellie does not substitute a zero.
          </p>
          <Button size="sm" onClick={onOpenGitHub} className="empty-state-action">
            Connect on the GitHub page
          </Button>
        </EmptyState>
      ) : loading ? (
        <Spinner label="Loading GitHub contributions…" />
      ) : error ? (
        <EmptyState title="Contributions unavailable" className="empty-state-error">
          <p className="empty-state-text" role="alert">{error}</p>
          <Button size="sm" onClick={() => setReload((value) => value + 1)} className="empty-state-action">
            Try again
          </Button>
        </EmptyState>
      ) : calendar ? (
        <>
          <div className="contribution-calendar-scroll">
            <div
              className="contribution-calendar-canvas"
              style={gridStyle}
              role="img"
              aria-label={`${calendar.totalContributions} GitHub profile contributions for ${login ?? "the connected account"} from ${calendar.startedOn} through ${calendar.endedOn}.`}
            >
              <div
                className="contribution-months"
                aria-hidden="true"
              >
                {monthLabels.map((label, index) =>
                  label ? (
                    <span
                      key={`${calendar.weeks[index]?.firstDay ?? index}-${label}`}
                      style={{ gridColumn: index + 1 }}
                    >
                      {label}
                    </span>
                  ) : null,
                )}
              </div>
              <div className="contribution-calendar-body">
                <div className="contribution-weekdays" aria-hidden="true">
                  <span style={{ gridRow: 2 }}>Mon</span>
                  <span style={{ gridRow: 4 }}>Wed</span>
                  <span style={{ gridRow: 6 }}>Fri</span>
                </div>
                <div className="contribution-weeks">
                  {calendar.weeks.map((week, weekIndex) => (
                    <div className="contribution-week" key={week.firstDay}>
                      {week.days.map((day, dayIndex) => (
                        <span
                          key={day.date}
                          className={`contribution-cell contribution-level-${day.level}`}
                          style={{
                            gridRow: day.weekday + 1,
                            gridColumn: weekIndex + 1,
                            "--i": Math.min(weekIndex * 7 + dayIndex, 60),
                          } as CSSProperties}
                          title={`${dateLabel(day.date)}: ${day.contributionCount} contribution${day.contributionCount === 1 ? "" : "s"}`}
                          aria-hidden="true"
                        />
                      ))}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
          <footer className="contribution-footer">
            <span>{dateLabel(calendar.startedOn)} – {dateLabel(calendar.endedOn)}</span>
            <span className="contribution-legend" aria-label="Contribution intensity from less to more">
              Less
              {[0, 1, 2, 3, 4].map((level) => (
                <i key={level} className={`contribution-cell contribution-level-${level}`} />
              ))}
              More
            </span>
          </footer>
          <p className="contribution-note">
            Includes commits, pull requests, issues, reviews, and other activity counted by GitHub. Private activity depends on the GitHub App&apos;s access.
          </p>
        </>
      ) : null}
    </section>
  );
}