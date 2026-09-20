import { Cat } from "../Cat";
import { ContributionCalendarCard } from "../github/ContributionCalendarCard";
import { AttentionCard, UpcomingTasksCard } from "./AttentionCard";
import { FocusTaskCard } from "./FocusTaskCard";
import { buildKpis, KpiGrid } from "./KpiGrid";
import { ProviderSummaryPanel } from "./ProviderSummaryPanel";
import { RecentCommitsCard } from "./RecentCommitsCard";
import { TokenTrendCard } from "./TokenTrendCard";
import {
  dashboardAttentionItems,
  latestProviderUpdate,
  lowestReportedAllowance,
  recentProviders,
  upcomingTasks,
} from "../../lib/dashboard";
import type {
  GitHubConnectionApi,
} from "../../lib/github";
import {
  githubErrorCopyOf,
} from "../../lib/github";
import type {
  ProviderOverview,
  Settings,
  TaskItem,
  View,
} from "../../lib/desktop";
import type { TaskController } from "../../lib/tasks";
import { useAnalytics } from "../../lib/useAnalytics";
import { useMemo } from "react";

export interface WorkspaceDashboardProps {
  native: boolean;
  providers: ProviderOverview[];
  visibleProviders: ProviderOverview[];
  settings: Settings | null;
  friendly: boolean;
  showMascot: boolean;
  saving: boolean;
  refreshing: boolean;
  refreshingProvider: string | null;
  github: GitHubConnectionApi;
  tasks: TaskController;
  /** First repository for the commit feed; null when disconnected. */
  defaultRepository: { owner: string; repo: string; branch: string | null } | null;
  onRefreshAll: () => void;
  onRefreshProvider: (providerId: string) => void;
  onHideProvider: (providerId: string) => void;
  onHideToTray: () => void;
  onNavigate: (view: View) => void;
  onOpenNewTask: () => void;
  onEditTask: (task: TaskItem) => void;
}

export function WorkspaceDashboard(props: WorkspaceDashboardProps) {
  const {
    native,
    providers,
    visibleProviders,
    friendly,
    showMascot,
    saving,
    refreshing,
    refreshingProvider,
    github,
    tasks,
    defaultRepository,
    onRefreshAll,
    onRefreshProvider,
    onHideProvider,
    onHideToTray,
    onNavigate,
    onOpenNewTask,
    onEditTask,
  } = props;
  const analytics = useAnalytics(native, "sevenDays", true);

  const githubConnected = github.status?.state === "Connected";
  const githubAttention =
    github.error ||
    (github.status?.lastError ? githubErrorCopyOf(github.status.lastError) : "");

  const attentionItems = useMemo(
    () => dashboardAttentionItems(visibleProviders, githubAttention),
    [visibleProviders, githubAttention],
  );

  const lowestAllowance = useMemo(
    () => lowestReportedAllowance(visibleProviders),
    [visibleProviders],
  );
  const latestUpdate = useMemo(
    () => latestProviderUpdate(visibleProviders),
    [visibleProviders],
  );

  const githubKpi = !native
    ? { label: "Desktop only", connected: false, detail: "GitHub is available in the Windows app." }
    : github.status === null
      ? { label: "Checking", connected: false, detail: "Reading the saved connection…" }
      : github.status.state === "Connected"
        ? {
            label: "Connected",
            connected: true,
            detail: `@${github.status.account?.login ?? "connected account"} · repositories loaded in GitHub`,
          }
        : github.status.state === "Authorizing"
          ? { label: "Signing in", connected: false, detail: "Complete the browser authorization to continue." }
          : {
              label: "Not connected",
              connected: false,
              detail:
                github.status?.clientIdConfigured && github.status.clientSecretConfigured
                  ? "App credentials ready — connect your account."
                  : "Set up GitHub in Settings, then connect.",
            };

  const kpis = buildKpis(
    visibleProviders,
    githubKpi,
    tasks.tasks,
    latestUpdate,
    lowestAllowance,
  );

  const pinnedTask =
    tasks.tasks.find((task) => task.id === tasks.pinnedTaskId) ?? null;

  const upcoming = upcomingTasks(tasks.tasks);

  const githubLogin = github.status?.account?.login ?? null;

  const greeting = friendly ? friendlyGreeting() : null;

  return (
    <div className="dashboard">
      <section className="dashboard-hero" aria-label="Workspace greeting">
        <div className="hero-copy">
          <p className="eyebrow">Workspace command center</p>
          <h1 className="hero-title">Everything important, one glance.</h1>
          <p className="hero-sub">
            {greeting ? (
              <>
                <strong className="hero-greeting">{greeting}.</strong>{" "}
              </>
            ) : null}
            Live AI allowances, connected GitHub work, and your local task list —
            nothing here is invented.
          </p>
          <div className="hero-actions">
            <button
              type="button"
              className="btn btn-primary"
              onClick={onRefreshAll}
              disabled={!native || refreshing || refreshingProvider !== null}
            >
              {refreshing ? "Refreshing…" : "Refresh providers"}
            </button>
            <button type="button" className="btn btn-secondary" onClick={onOpenNewTask} disabled={!native}>
              Add a task
            </button>
            <button type="button" className="btn btn-secondary" onClick={onHideToTray} disabled={!native}>
              Hide to tray
            </button>
          </div>
        </div>
        {showMascot && (
          <div className="hero-mascot" aria-hidden="true">
            <Cat />
          </div>
        )}
      </section>

      <KpiGrid kpis={kpis} onNavigate={onNavigate} />

      <div className="bento">
        <div className="bento-main">
          <ProviderSummaryPanel
            providers={providers}
            visibleProviders={visibleProviders}
            native={native}
            refreshing={refreshing}
            refreshingProvider={refreshingProvider}
            onRefreshAll={onRefreshAll}
            onRefreshProvider={onRefreshProvider}
            onHideProvider={onHideProvider}
            onOpenDetails={() => onNavigate("ai-usage")}
            saving={saving}
            settingsReady={Boolean(props.settings)}
          />
          <TokenTrendCard analytics={analytics.analytics} />
        </div>
        <div className="bento-side">
          <ContributionCalendarCard
            native={native}
            connected={githubConnected}
            login={githubLogin}
            onOpenGitHub={() => onNavigate("github")}
          />
          <RecentCommitsCard
            native={native}
            connected={githubConnected}
            defaultRepository={defaultRepository}
            onOpenGitHub={() => onNavigate("github")}
          />
        </div>
      </div>

      <FocusTaskCard
        pinnedTask={pinnedTask}
        tasks={tasks.tasks}
        busy={tasks.busy}
        native={native}
        onOpenTodo={() => onNavigate("todos")}
        onEdit={onEditTask}
        onComplete={(taskId) => void tasks.setCompleted(taskId, true)}
        onShowNote={(taskId) => void tasks.setPinned(taskId)}
        onUnpin={() => void tasks.setPinned(null)}
      />

      <div className="dashboard-lower">
        <AttentionCard items={attentionItems} />
        <UpcomingTasksCard upcoming={upcoming} onOpenTodo={() => onNavigate("todos")} />
        <section className="panel panel-activity" aria-label="Recent workspace activity">
          <header className="panel-header">
            <div className="panel-title">
              <span className="panel-kicker">Local pulse</span>
              <h2 className="panel-heading">Recent activity</h2>
            </div>
          </header>
          <ul className="activity-feed">
            {recentProviders(visibleProviders, 3).map((provider) => (
              <li className="activity-item" key={provider.providerId}>
                <span className="activity-mark" aria-hidden="true">
                  AI
                </span>
                <div className="activity-copy">
                  <strong>{provider.displayName}</strong>
                  <p>
                    {provider.stale ? "Cached snapshot" : "Updated"} ·{" "}
                    {provider.snapshot ? formatAgeShort(provider.snapshot.fetchedAt) : ""}
                  </p>
                </div>
              </li>
            ))}
            <li className="activity-item">
              <span className="activity-mark" aria-hidden="true">
                GH
              </span>
              <div className="activity-copy">
                <strong>GitHub</strong>
                <p>
                  {githubConnected
                    ? `Connected as ${github.status?.account?.login ?? "your account"}`
                    : "Not connected"}
                </p>
              </div>
            </li>
            {attentionItems.length > 0 && (
              <li className="activity-item">
                <span className="activity-mark" aria-hidden="true">
                  !
                </span>
                <div className="activity-copy">
                  <strong>Needs attention</strong>
                  <p>{attentionItems.length} item{attentionItems.length === 1 ? "" : "s"} on the board.</p>
                </div>
              </li>
            )}
          </ul>
        </section>
      </div>
    </div>
  );
}

function friendlyGreeting(): string {
  const hour = new Date().getHours();
  if (hour < 5) return "Still up, I see";
  if (hour < 12) return "Good morning";
  if (hour < 18) return "Good afternoon";
  return "Good evening";
}

function formatAgeShort(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const elapsed = Math.max(0, Date.now() - date.getTime());
  const minutes = Math.floor(elapsed / 60_000);
  if (minutes < 1) return "just now";
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.floor(hours / 24)}d ago`;
}