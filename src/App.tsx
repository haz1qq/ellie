import { useEffect, useState } from "react";
import { AppShell } from "./components/layout/AppShell";
import { WorkspaceDashboard } from "./components/dashboard/WorkspaceDashboard";
import { AiUsagePage } from "./components/providers/AiUsagePage";
import { GitHubPanel } from "./components/GitHubPanel";
import { TodoPage } from "./components/tasks/TodoPage";
import { AnalyticsPage } from "./components/history/AnalyticsPage";
import { SettingsPage } from "./components/settings/SettingsPage";
import {
  desktop,
  isView,
  type AnalyticsRange,
  type ProviderOverview,
  type Settings,
  type TaskItem,
  type View,
} from "./lib/desktop";
import { useGitHubConnection } from "./lib/github";
import { useRepositories, useTasks } from "./lib/tasks";

export default function App() {
  const [view, setView] = useState<View>("dashboard");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [refreshing, setRefreshing] = useState(false);
  const [refreshingProvider, setRefreshingProvider] = useState<string | null>(null);
  const [retry, setRetry] = useState(0);
  const [providers, setProviders] = useState<ProviderOverview[]>([]);
  const [analyticsRange, setAnalyticsRange] = useState<AnalyticsRange>("sevenDays");

  /** Shell commands: "add a task" / "create a repository" from the top bar. */
  const [newTaskRequest, setNewTaskRequest] = useState(0);
  const [newRepositoryRequest, setNewRepositoryRequest] = useState(0);
  const [editTaskId, setEditTaskId] = useState<number | null>(null);

  const native = desktop.available();
  const github = useGitHubConnection(native);
  const githubConnected = github.status?.state === "Connected";
  const tasks = useTasks(native);
  const repositories = useRepositories(native, githubConnected);

  const visibleProviders = providers.filter(
    (provider) =>
      !(settings?.hiddenProviderIds ?? []).includes(provider.providerId) &&
      (provider.snapshot
        ? provider.snapshot.hasSubscription !== false
        : provider.error !== "authentication_required"),
  );

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    let unlistenProviders: (() => void) | undefined;
    if (!native) {
      setLoading(false);
      return;
    }
    setLoading(true);
    setError("");
    void (async () => {
      try {
        const stop = await desktop.onNavigate((next) => {
          if (active) setView((current) => (isView(next) ? next : current));
        });
        if (!active) {
          stop();
          return;
        }
        unlisten = stop;
        const stopProviders = await desktop.onProvidersUpdated((next) => {
          if (active) setProviders(next);
        });
        if (!active) {
          stopProviders();
          return;
        }
        unlistenProviders = stopProviders;
        const result = await desktop.bootstrap();
        if (active) {
          setSettings(result.settings);
          setView(isView(result.view) ? result.view : "dashboard");
          setProviders(result.providers ?? []);
        }
      } catch {
        if (active)
          setError(
            "Ellie could not load local settings. Retry, or restart the app if the problem continues.",
          );
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
      unlisten?.();
      unlistenProviders?.();
    };
  }, [native, retry]);

  async function refreshAll() {
    if (!native || refreshing || refreshingProvider) return;
    setRefreshing(true);
    setError("");
    setNotice("");
    try {
      const result = await desktop.refreshAll();
      if (result.busy) {
        setNotice("Ellie is already refreshing. The current data is unchanged.");
      } else {
        setProviders(result.providers);
        setNotice("Provider data refreshed.");
      }
    } catch {
      setError("Ellie could not refresh provider data. Existing data is unchanged.");
    } finally {
      setRefreshing(false);
    }
  }

  async function refreshProvider(providerId: string) {
    if (!native || refreshing || refreshingProvider) return;
    setRefreshingProvider(providerId);
    setError("");
    setNotice("");
    try {
      const result = await desktop.refreshProvider(providerId);
      if (result.busy) {
        setNotice("Ellie is already refreshing. The current data is unchanged.");
      } else {
        setProviders(result.providers);
        setNotice("Provider data refreshed.");
      }
    } catch {
      setError("Ellie could not refresh this provider. Existing data is unchanged.");
    } finally {
      setRefreshingProvider(null);
    }
  }

  async function hide() {
    setError("");
    try {
      await desktop.hide();
    } catch {
      setError("Ellie could not hide the window. Try the minimize button.");
    }
  }

  function navigate(next: View) {
    setView(next);
    setNotice("");
  }

  function openNewTask() {
    setEditTaskId(null);
    navigate("todos");
    setNewTaskRequest((value) => value + 1);
  }

  function openTaskEditor(task: TaskItem) {
    navigate("todos");
    setEditTaskId(task.id);
  }

  function openNewRepository() {
    navigate("github");
    if (!githubConnected) return;
    setNewRepositoryRequest((value) => value + 1);
  }

  const friendly = settings?.friendlyMessages ?? true;
  const showMascot = settings?.showMascot ?? true;

  async function hideProvider(providerId: string) {
    if (!settings || !native) return;
    try {
      const saved = await desktop.saveSettings({
        ...settings,
        hiddenProviderIds: [...new Set([...(settings.hiddenProviderIds ?? []), providerId])],
      });
      setSettings(saved);
      setNotice("Provider hidden. Show it again in Settings → Provider visibility.");
    } catch {
      setError("Provider visibility was not saved. Your previous display settings are still active.");
    }
  }

  return (
    <AppShell
      view={view}
      onNavigate={navigate}
      friendly={friendly}
      native={native}
      refreshing={refreshing}
      githubConnected={githubConnected}
      onNewTask={openNewTask}
      onNewRepository={openNewRepository}
      onRefreshAll={() => void refreshAll()}
    >
      {error && (
        <div className="page-alert" role="alert">
          {error}
          {!settings && native && (
            <button className="btn btn-secondary btn-sm" onClick={() => setRetry((value) => value + 1)} disabled={loading}>
              Retry
            </button>
          )}
        </div>
      )}
      {notice && view === "dashboard" && (
        <p className="page-notice" role="status">
          {notice}
        </p>
      )}
      {loading && <p className="page-notice" role="status">Opening your local settings…</p>}

      {view === "dashboard" ? (
        <WorkspaceDashboard
          native={native}
          providers={providers}
          visibleProviders={visibleProviders}
          settings={settings}
          friendly={friendly}
          showMascot={showMascot}
          saving={false}
          refreshing={refreshing}
          refreshingProvider={refreshingProvider}
          github={github}
          tasks={tasks}
          repositories={repositories.repositories}
          onRefreshAll={() => void refreshAll()}
          onRefreshProvider={(providerId) => void refreshProvider(providerId)}
          onHideProvider={(providerId) => void hideProvider(providerId)}
          onHideToTray={() => void hide()}
          onNavigate={navigate}
          onOpenNewTask={openNewTask}
          onEditTask={openTaskEditor}
        />
      ) : view === "ai-usage" ? (
        <AiUsagePage
          providers={providers}
          visibleProviders={visibleProviders}
          native={native}
          saving={false}
          settingsReady={Boolean(settings)}
          refreshing={refreshing}
          refreshingProvider={refreshingProvider}
          onRefreshAll={() => void refreshAll()}
          onRefreshProvider={(providerId) => void refreshProvider(providerId)}
          onHideProvider={(providerId) => void hideProvider(providerId)}
          onHideToTray={() => void hide()}
        />
      ) : view === "github" ? (
        <GitHubPanel
          native={native}
          connection={github}
          onOpenSettings={() => navigate("settings")}
          createRequest={newRepositoryRequest}
          onConsumeCreateRequest={() => { /* counter consumed by local state */ }}
          onRepositoriesChanged={repositories.reload}
        />
      ) : view === "todos" ? (
        <TodoPage
          native={native}
          tasks={tasks}
          repositories={repositories.repositories}
          createRequest={newTaskRequest}
          onConsumeCreateRequest={() => { /* counter consumed by local state */ }}
          editTaskId={editTaskId}
          onConsumeEditRequest={() => setEditTaskId(null)}
        />
      ) : view === "history" ? (
        <AnalyticsPage
          native={native}
          range={analyticsRange}
          onRangeChange={setAnalyticsRange}
        />
      ) : (
        <SettingsPage
          native={native}
          settings={settings}
          loading={loading}
          providers={providers}
          onChangeSettings={setSettings}
          onRefreshAll={() => void refreshAll()}
          refreshing={refreshing}
        />
      )}
    </AppShell>
  );
}