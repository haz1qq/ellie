import { useEffect, useState, type CSSProperties, type PointerEvent } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ChevronRight } from "lucide-react";
import {
  desktop,
  type MiniGitHubProjection,
  type MiniTaskProjection,
  type ProviderOverview,
  type Settings,
  type View,
} from "../lib/desktop";
import { selectMiniQuotaMetrics } from "../lib/miniQuota";

const STALE_AFTER_SECONDS = 30 * 60;

export default function MiniBar() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [providers, setProviders] = useState<ProviderOverview[]>([]);
  const [task, setTask] = useState<MiniTaskProjection | null>(null);
  const [github, setGithub] = useState<MiniGitHubProjection | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const native = desktop.available();

  useEffect(() => {
    let active = true;
    let stopProviders: (() => void) | undefined;
    let stopSettings: (() => void) | undefined;
    let stopRefresh: (() => void) | undefined;
    let stopTasks: (() => void) | undefined;
    if (!native) {
      setLoading(false);
      setError("Quota unavailable");
      return;
    }
    void (async () => {
      let providersAdvanced = false;
      let settingsAdvanced = false;
      try {
        const unlistenProviders = await desktop.onProvidersUpdated((next) => {
          providersAdvanced = true;
          if (active) setProviders(next);
        });
        if (!active) {
          unlistenProviders();
          return;
        }
        stopProviders = unlistenProviders;
        const unlistenSettings = await desktop.onMiniSettingsUpdated((next) => {
          settingsAdvanced = true;
          if (active) setSettings(next);
        });
        if (!active) {
          unlistenSettings();
          return;
        }
        stopSettings = unlistenSettings;
        // The HUD reflects shared cached data: re-read it whenever the window
        // is shown, settings change, or tasks change. All reads are cached.
        stopRefresh = await desktop.onMiniRefresh(() => {
          if (active) void loadSnapshot(setSettings, setProviders, setTask, setGithub);
        });
        stopTasks = await desktop.onTasksUpdated(() => {
          if (active) void loadSnapshot(setSettings, setProviders, setTask, setGithub);
        });
        const bootstrap = await desktop.miniBootstrap();
        if (active) {
          if (!settingsAdvanced) setSettings(bootstrap.settings);
          if (!providersAdvanced) setProviders(bootstrap.providers);
          setTask(bootstrap.task);
          setGithub(bootstrap.github);
        }
      } catch {
        if (active) setError("Quota unavailable");
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
      stopProviders?.();
      stopSettings?.();
      stopRefresh?.();
      stopTasks?.();
    };
  }, [native]);

  const showTask = Boolean(settings?.miniBarShowTask);
  const showGithub = Boolean(settings?.miniBarShowGitHub);

  const metrics = selectMiniQuotaMetrics(
    providers,
    settings?.hiddenProviderIds ?? [],
  );
  const emptyState = miniEmptyState(providers, settings?.hiddenProviderIds ?? []);
  const style = {
    "--mini-opacity": settings?.miniBarOpacity ?? 0.9,
  } as CSSProperties;
  const accessibleStatus = miniAccessibleStatus(
    loading,
    error,
    metrics,
    emptyState,
    showTask ? task : undefined,
    showGithub ? github : undefined,
  );

  async function openMain() {
    setError("");
    try {
      await desktop.openMainWindow();
    } catch {
      setError("Ellie could not open the dashboard");
    }
  }

  async function openSection(view: View) {
    setError("");
    try {
      await desktop.openMainSection(view);
    } catch {
      setError("Ellie could not open that page");
    }
  }

  function startDragging(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    event.preventDefault();
    void getCurrentWindow().startDragging().catch(() => {
      setError("Mini bar could not be moved");
    });
  }

  return (
    <main
      className="mini-bar"
      style={style}
      aria-label="Ellie quota mini bar"
    >
      <span
        className="visually-hidden"
        role={error ? "alert" : "status"}
        aria-live={error ? "assertive" : "polite"}
      >
        Mini bar status: {accessibleStatus}
      </span>
      <div
        className="mini-drag-handle"
        data-tauri-drag-region
        title="Drag mini bar"
        onPointerDown={startDragging}
      >
        <span aria-hidden="true">⠿</span>
      </div>
      <div className="mini-bands">
        <div className="mini-band mini-band-quota">
          <button
            type="button"
            className="mini-open"
            onClick={() => void openMain()}
            aria-label={`${accessibleStatus}. Open Ellie dashboard`}
          >
            <span className="mini-wordmark" aria-hidden="true">ellie<span>.</span></span>
            <span className="mini-content">
              {loading ? (
                <span className="mini-state">Loading quota…</span>
              ) : error ? (
                <span className="mini-state mini-error">{error}</span>
              ) : metrics.length === 0 ? (
                <span className="mini-state">{emptyState}</span>
              ) : (
                metrics.map((metric) => (
                  <span
                    className="mini-metric"
                    key={`${metric.providerId}:${metric.windowId}`}
                  >
                    <span className="mini-provider">{metric.providerName}</span>
                    <span>{metric.windowLabel}</span>
                    <strong>{formatPercent(metric.remainingPercent)} remaining</strong>
                    {metric.stale && (
                      <small>
                        Showing data from {formatAge(metric.lastSuccessfulRefresh)} · refresh failed
                      </small>
                    )}
                  </span>
                ))
              )}
            </span>
          </button>
        </div>
        {showTask && (
          <section className="mini-band mini-band-task" aria-label="Current task">
            <button
              type="button"
              className="mini-section-button"
              onClick={() => void openSection("todos")}
            >
              <span className="mini-section-title">
                Current task <ChevronRight size={12} className="mini-section-chevron" aria-hidden="true" />
              </span>
              <span className="mini-section-value" title={task?.title ?? undefined}>
                {task ? (
                  <>
                    {task.kind === "work" ? "Work · " : "Personal · "}
                    <span className="mini-task-title">{task.title}</span>
                  </>
                ) : (
                  "No current task"
                )}
              </span>
            </button>
          </section>
        )}
        {showGithub && (
          <section className="mini-band mini-band-github" aria-label="GitHub activity">
            <button
              type="button"
              className="mini-section-button"
              onClick={() => void openSection("github")}
            >
              <span className="mini-section-title">
                GitHub activity <ChevronRight size={12} className="mini-section-chevron" aria-hidden="true" />
              </span>
              <span className="mini-section-value">
                {githubSummaryCopy(github)}
              </span>
            </button>
          </section>
        )}
      </div>
    </main>
  );
}

async function loadSnapshot(
  setSettings: (settings: Settings) => void,
  setProviders: (providers: ProviderOverview[]) => void,
  setTask: (task: MiniTaskProjection | null) => void,
  setGithub: (github: MiniGitHubProjection | null) => void,
) {
  try {
    const bootstrap = await desktop.miniBootstrap();
    setSettings(bootstrap.settings);
    setProviders(bootstrap.providers);
    setTask(bootstrap.task);
    setGithub(bootstrap.github);
  } catch {
    // The mini keeps its last cached snapshot on refresh failures; bootstrap
    // errors are shown once at mount by the owning effect.
  }
}

function githubSummaryCopy(github: MiniGitHubProjection | null): string {
  if (!github) return "Not connected";
  if (github.state === "Disconnected") return "Connect GitHub in Ellie";
  if (github.state === "Authorizing") return "Completing sign-in…";
  const summary = github.summary;
  if (!summary) return "Open Ellie to load commits";
  const age = summaryAgeCopy(summary.fetchedAt, summary.ageSeconds);
  const scope = `${summary.totalLoaded} ${summary.totalLoaded === 1 ? "commit" : "commits"} · ${
    summary.repositoriesChecked
  } ${summary.repositoriesChecked === 1 ? "repo" : "repos"}`;
  const attributed =
    summary.attributed > 0
      ? ` · ${summary.attributed} ${summary.attributed === 1 ? "yours" : "yours"}`
      : "";
  const stale =
    summary.ageSeconds > STALE_AFTER_SECONDS
      ? ` · cached ${age} — open Ellie to refresh`
      : "";
  return `${scope}${attributed}${stale}`;
}

function summaryAgeCopy(fetchedAt: string, ageSeconds: number): string {
  const timestamp = Date.parse(fetchedAt);
  if (!Number.isFinite(timestamp) || ageSeconds > 900) {
    return formatAge(fetchedAt);
  }
  if (ageSeconds < 60) return `${ageSeconds}s ago`;
  const minutes = Math.floor(ageSeconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  return `${Math.floor(minutes / 60)}h ago`;
}

function miniEmptyState(
  providers: ProviderOverview[],
  hiddenProviderIds: string[],
) {
  const visible = providers.filter(
    (provider) => !hiddenProviderIds.includes(provider.providerId),
  );
  if (providers.length === 0) return "Waiting for quota data";
  if (visible.length === 0) return "No providers selected";
  if (visible.some((provider) => provider.error !== null)) {
    return "Quota unavailable";
  }
  if (visible.every((provider) => provider.snapshot === null)) {
    return "Waiting for quota data";
  }
  const eligible = visible.filter((provider) => {
    const snapshot = provider.snapshot;
    return snapshot?.dataKind === "live"
      && snapshot.hasSubscription !== false
      && snapshot.capabilities.quotaWindows;
  });
  if (eligible.some((provider) => provider.snapshot?.windows.some(
    (window) => window.source === "provider_reported" && window.remainingPercent === null,
  ))) {
    return "Quota remaining not reported";
  }
  return "No provider-reported quota windows";
}

function miniAccessibleStatus(
  loading: boolean,
  error: string,
  metrics: ReturnType<typeof selectMiniQuotaMetrics>,
  emptyState: string,
  task: MiniTaskProjection | null | undefined,
  github: MiniGitHubProjection | null | undefined,
) {
  const parts: string[] = [];
  if (loading) {
    parts.push("Loading quota");
  } else if (error) {
    parts.push(error);
  } else if (metrics.length === 0) {
    parts.push(emptyState);
  } else {
    parts.push(metrics.map((metric) => {
      const stale = metric.stale
        ? `, showing data from ${formatAge(metric.lastSuccessfulRefresh)}, refresh failed`
        : "";
      return `${metric.providerName}, ${metric.windowLabel}, ${formatPercent(metric.remainingPercent)} remaining${stale}`;
    }).join(". "));
  }
  if (task !== undefined) {
    parts.push(task === null ? "No current task" : `Current task, ${task.title}`);
  }
  if (github !== undefined) {
    const summary = github?.summary;
    const loaded = summary
      ? `, ${summary.totalLoaded} commits loaded across ${summary.repositoriesChecked} repositories`
      : "";
    parts.push(`${githubSummaryCopy(github)}${loaded}`);
  }
  return parts.join(". ");
}

function formatPercent(value: number) {
  return `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(value)}%`;
}

function formatAge(value: string | null) {
  if (!value) return "an earlier refresh";
  const timestamp = Date.parse(value);
  if (!Number.isFinite(timestamp)) return "an earlier refresh";
  const seconds = Math.max(0, Math.floor((Date.now() - timestamp) / 1_000));
  if (seconds < 60) return `${seconds} seconds ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} minute${minutes === 1 ? "" : "s"} ago`;
  const hours = Math.floor(minutes / 60);
  return `${hours} hour${hours === 1 ? "" : "s"} ago`;
}