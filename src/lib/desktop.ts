import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type View =
  | "dashboard"
  | "ai-usage"
  | "github"
  | "todos"
  | "history"
  | "settings";

export function isView(value: unknown): value is View {
  return (
    value === "dashboard" ||
    value === "ai-usage" ||
    value === "github" ||
    value === "todos" ||
    value === "history" ||
    value === "settings"
  );
}
export interface Settings {
  closeToTray: boolean;
  showMascot: boolean;
  friendlyMessages: boolean;
  notificationsEnabled: boolean;
  notificationThresholds: [number, number, number];
  hiddenProviderIds: string[];
  miniBarEnabled: boolean;
  miniBarOpacity: number;
  miniBarX: number | null;
  miniBarY: number | null;
  miniBarShowGitHub: boolean;
  miniBarShowTask: boolean;
  miniBarWidth: number | null;
  miniBarHeight: number | null;
}
export interface Bootstrap {
  settings: Settings;
  view: View;
  providers: ProviderOverview[];
}
export interface MiniBootstrap {
  settings: Settings;
  providers: ProviderOverview[];
  /** Present only when the HUD task section is enabled. */
  task: MiniTaskProjection | null;
  /** Present only when the HUD GitHub section is enabled. */
  github: MiniGitHubProjection | null;
}

export interface MiniTaskProjection {
  taskId: number;
  title: string;
  kind: TaskKind;
  /** True when this is the pinned task; false for the next-task fallback. */
  pinned: boolean;
}

/** Matches the Rust GitHubConnectionState verbatim serialization. */
export type MiniGitHubState = "Connected" | "Authorizing" | "Disconnected";

export interface MiniGitHubProjection {
  state: MiniGitHubState;
  accountLogin: string | null;
  summary: MiniCommitSummary | null;
}

export interface MiniCommitSummary {
  totalLoaded: number;
  attributed: number;
  repositoriesChecked: number;
  fetchedAt: string;
  ageSeconds: number;
}

export type MetricSource = "provider_reported" | "locally_calculated";
export type DataKind = "live" | "mock";
export interface ProviderCapabilities {
  quotaWindows: boolean;
  tokenUsage: boolean;
  accountBalance: boolean;
  credits: boolean;
  costTracking: boolean;
  localHistory: boolean;
}
export interface UsageWindow {
  id: string;
  label: string;
  usedPercent: number | null;
  remainingPercent: number | null;
  resetAt: string | null;
  source: MetricSource;
}
export interface TokenUsage {
  totalTokens: number | null;
  inputTokens?: number | null;
  outputTokens?: number | null;
  cachedInputTokens?: number | null;
  requestCount: number | null;
  estimatedCostUsd: number | null;
  source: MetricSource;
}
export interface SpendEstimate {
  amount: number;
  currency: string;
  windowDays: number;
}
export interface UsageSnapshot {
  providerId: string;
  displayName: string;
  accountLabel: string | null;
  plan: string | null;
  /** null = unknown or not subscription-based; false = explicitly unsubscribed (hidden) */
  hasSubscription: boolean | null;
  capabilities: ProviderCapabilities;
  authState: string;
  dataKind: DataKind;
  windows: UsageWindow[];
  tokenUsage: TokenUsage | null;
  balance: number | null;
  /** ISO-4217 code for `balance` (e.g. "USD", "CNY"); null when no balance is reported */
  balanceCurrency: string | null;
  /** Ellie's own estimate of spend from balance history, when computable */
  spendEstimate: SpendEstimate | null;
  /** Model in use or dominant alias reported by the provider, when available */
  model: string | null;
  fetchedAt: string;
}
export interface ProviderOverview {
  providerId: string;
  displayName: string;
  snapshot: UsageSnapshot | null;
  error:
    | "invalid_snapshot"
    | "authentication_required"
    | "authentication_expired"
    | "unavailable"
    | null;
  stale?: boolean;
  lastSuccessfulRefresh?: string | null;
  lastAttemptAt?: string | null;
  nextRetryAt?: string | null;
}

export type AnalyticsRange = "today" | "sevenDays" | "thirtyDays" | "ninetyDays";
export type AnalyticsSource =
  | "provider_reported"
  | "locally_calculated"
  | "mixed";
export interface AnalyticsSpend {
  currency: string;
  amount: number;
  source: AnalyticsSource;
}
export interface AnalyticsProvider {
  providerId: string;
  displayName: string;
  model: string | null;
  latestAt: string;
  totalTokens: number | null;
  requestCount: number | null;
  tokenSource: "provider_reported" | "locally_calculated" | null;
}
export interface TokenPoint {
  date: string;
  totalTokens: number;
}
export interface QuotaPoint {
  providerId: string;
  displayName: string;
  windowId: string;
  windowLabel: string;
  usedPercent: number | null;
  remainingPercent: number | null;
  observedAt: string;
}
export interface AnalyticsResponse {
  range: AnalyticsRange;
  startAt: string;
  endAt: string;
  snapshotCount: number;
  providerCount: number;
  latestTotalTokens: number | null;
  latestRequestCount: number | null;
  tokenSource: AnalyticsSource | null;
  estimatedSpend: AnalyticsSpend[];
  providers: AnalyticsProvider[];
  tokenSeries: TokenPoint[];
  quotaWindows: QuotaPoint[];
}

export type ProviderKeySource =
  | "credential_manager"
  | "environment"
  | "none";
export interface RefreshResponse {
  providers: ProviderOverview[];
  refreshed: boolean;
  busy: boolean;
}

export interface ProviderKeyStatus {
  providerId: string;
  source: ProviderKeySource;
}

export type LocalApiAction = "enable" | "disable" | "rotate";
export type LocalApiFailure = "credentialStore" | "invalidOverride" | "missingToken" | "persistence" | "bind" | "server" | "overrideActive" | "disabled" | "controlsUnavailable";
export interface LocalApiStatus {
  enabled: boolean;
  listening: boolean;
  tokenSource: "none" | "environment" | "credentialManager";
  error: LocalApiFailure | null;
}

/**
 * GitHub workspace integration (additive). Wire values match the Rust
 * `GitHubConnectionState` enum, whose unit variants serialize verbatim
 * (`"Disconnected"` / `"Authorizing"` / `"Connected"`).
 */
export type GitHubConnectionState = "Disconnected" | "Authorizing" | "Connected";
export interface GitHubAccount {
  id: number;
  login: string;
}
/** redacted snake_case categories returned by the Rust GitHub service */
export type GitHubErrorCategory =
  | "window_denied"
  | "invalid_input"
  | "busy"
  | "authorization_state_mismatch"
  | "authorization_denied"
  | "app_credentials_invalid"
  | "token_expiration_required"
  | "token_response_invalid"
  | "account_response_invalid"
  | "authentication_required"
  | "authentication_expired"
  | "rate_limited"
  | "permission_denied"
  | "not_found"
  | "validation_failed"
  | "network_unavailable"
  | "provider_unavailable"
  | "malformed_response"
  | "credential_store"
  | "cancelled"
  | "conflict"
  | "creation_outcome_unknown";
export interface GitHubConnectionStatus {
  state: GitHubConnectionState;
  account: GitHubAccount | null;
  lastError: GitHubErrorCategory | null;
  tokenPresent: boolean;
  clientIdConfigured: boolean;
  clientSecretConfigured: boolean;
}
export interface GitHubRepositorySummary {
  id: number;
  name: string;
  fullName: string;
  private: boolean;
  defaultBranch: string;
  htmlUrl: string;
}
export interface GitHubCommitSummary {
  sha: string;
  subject: string;
  authorId: number | null;
  authorLogin: string | null;
  authoredAt: string;
  committedAt: string;
}
export interface GitHubContributionDay {
  date: string;
  contributionCount: number;
  level: 0 | 1 | 2 | 3 | 4;
  weekday: number;
}
export interface GitHubContributionWeek {
  firstDay: string;
  days: GitHubContributionDay[];
}
export interface GitHubContributionCalendar {
  totalContributions: number;
  startedOn: string;
  endedOn: string;
  weeks: GitHubContributionWeek[];
}
export interface GitHubContributionCalendarQuery {
  year?: number;
}

export interface RepositoryCreationInput {
  name: string;
  description: string | null;
  private: boolean;
  initializeReadme: boolean;
}
export interface RepositoryCreationReview {
  reviewId: string;
  owner: string;
  name: string;
  description: string | null;
  private: boolean;
  initializeReadme: boolean;
  expiresAt: string;
}
export type RepositoryCreationAttemptState = "outcome_unknown";
export type RepositoryCreationResolution = "exists" | "not_found";
export interface RepositoryCreationAttemptStatus {
  attemptId: string;
  owner: string;
  name: string;
  state: RepositoryCreationAttemptState;
  repositoryUrl: string;
  createdAt: string;
  updatedAt: string;
}

/** Local task workspace (additive, main-window-only Rust commands). */
export type TaskPriority = "none" | "low" | "medium" | "high";
export type TaskKind = "work" | "personal";
export type TaskCompletionFilter = "all" | "open" | "completed";
export interface TaskRepositoryInput {
  repositoryId: number;
  fullName: string;
}
export interface TaskRepositoryLink extends TaskRepositoryInput {
  htmlUrl: string;
}
export interface TaskList {
  id: number;
  name: string;
  taskCount: number;
  createdAt: string;
  updatedAt: string;
}
export interface TaskItem {
  id: number;
  listId: number;
  title: string;
  notes: string | null;
  kind: TaskKind;
  priority: TaskPriority;
  dueDate: string | null;
  repository: TaskRepositoryLink | null;
  completedAt: string | null;
  createdAt: string;
  updatedAt: string;
}
export interface TaskBootstrap {
  lists: TaskList[];
  tasks: TaskItem[];
  pinnedTaskId: number | null;
}
export interface TaskQuery {
  listId?: number | null;
  completion?: TaskCompletionFilter;
  priority?: TaskPriority | null;
}
export interface ListDeletePreview {
  listId: number;
  taskCount: number;
}
/** Redacted snake_case categories returned by the Rust task service. */
export type TaskErrorCategory =
  | "window_denied"
  | "invalid_input"
  | "not_found"
  | "conflict"
  | "count_changed"
  | "storage";

export const desktop = {
  localApiStatus: () => invoke<LocalApiStatus>("local_api_status"),
  configureLocalApi: (action: LocalApiAction) => invoke<LocalApiStatus>("configure_local_api", { action }),
  available: isTauri,
  bootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  miniBootstrap: () => invoke<MiniBootstrap>("get_mini_bootstrap"),
  openMainWindow: () => invoke<void>("open_main_window"),
  openMainSection: (view: View) =>
    invoke<void>("open_main_section", { view }),
  getAnalytics: (range: AnalyticsRange) =>
    invoke<AnalyticsResponse>("get_analytics", { range }),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),
  hide: () => invoke<void>("hide_to_tray"),
  onNavigate: (callback: (view: View) => void) =>
    listen<View>("navigate", (event) => callback(event.payload)),
  onProvidersUpdated: (callback: (providers: ProviderOverview[]) => void) =>
    listen<ProviderOverview[]>("providers-updated", (event) =>
      callback(event.payload),
    ),
  onMiniSettingsUpdated: (callback: (settings: Settings) => void) =>
    listen<Settings>("mini-settings-updated", (event) => callback(event.payload)),
  onMiniRefresh: (callback: () => void) =>
    listen<void>("mini-refresh", () => callback()),
  onTasksUpdated: (callback: () => void) =>
    listen<void>("tasks-updated", () => callback()),
  onTaskNoteUpdated: (callback: (task: TaskItem | null) => void) =>
    listen<TaskItem | null>("task-note-updated", (event) => callback(event.payload)),
  refreshAll: () => invoke<RefreshResponse>("refresh_all"),
  refreshProvider: (providerId: string) =>
    invoke<RefreshResponse>("refresh_provider", { providerId }),
  saveProviderKey: (providerId: string, key: string) =>
    invoke<void>("save_provider_key", { providerId, key }),
  deleteProviderKey: (providerId: string) =>
    invoke<void>("delete_provider_key", { providerId }),
  providerKeyStatus: () =>
    invoke<ProviderKeyStatus[]>("provider_key_status"),
  githubConnectionStatus: () =>
    invoke<GitHubConnectionStatus>("github_connection_status"),
  githubSaveClientId: (clientId: string) =>
    invoke<GitHubConnectionStatus>("github_save_client_id", { clientId }),
  githubSaveClientSecret: (clientSecret: string) =>
    invoke<GitHubConnectionStatus>("github_save_client_secret", {
      clientSecret,
    }),
  githubSignIn: () => invoke<GitHubConnectionStatus>("github_sign_in"),
  githubCancelSignIn: () =>
    invoke<GitHubConnectionStatus>("github_cancel_sign_in"),
  githubDisconnect: () => invoke<GitHubConnectionStatus>("github_disconnect"),
  githubListRepositories: () =>
    invoke<GitHubRepositorySummary[]>("github_list_repositories"),
  githubListCommits: (owner: string, repo: string, branch?: string, maxRows?: number) =>
    invoke<GitHubCommitSummary[]>("github_list_commits", {
      owner,
      repo,
      branch: branch || undefined,
      maxRows,
    }),
  githubContributionCalendar: (query?: GitHubContributionCalendarQuery) =>
    invoke<GitHubContributionCalendar>("github_contribution_calendar", { query: query ?? {} }),
  githubPrepareRepositoryCreation: (input: RepositoryCreationInput) =>
    invoke<RepositoryCreationReview>("github_prepare_repository_creation", {
      input,
    }),
  githubConfirmRepositoryCreation: (reviewId: string) =>
    invoke<GitHubRepositorySummary>("github_confirm_repository_creation", {
      reviewId,
    }),
  githubRepositoryCreationStatus: () =>
    invoke<RepositoryCreationAttemptStatus[]>(
      "github_repository_creation_status",
    ),
  githubResolveRepositoryCreation: (
    attemptId: string,
    resolution: RepositoryCreationResolution,
  ) =>
    invoke<void>("github_resolve_repository_creation", {
      attemptId,
      resolution,
    }),
  taskBootstrap: () => invoke<TaskBootstrap>("task_bootstrap"),
  taskList: (query: TaskQuery) => invoke<TaskItem[]>("task_list", { query }),
  taskCreateList: (name: string) =>
    invoke<TaskList>("task_create_list", { name }),
  taskRenameList: (listId: number, name: string) =>
    invoke<TaskList>("task_rename_list", { listId, name }),
  taskListDeletePreview: (listId: number) =>
    invoke<ListDeletePreview>("task_list_delete_preview", { listId }),
  taskDeleteList: (listId: number, expectedTaskCount: number) =>
    invoke<void>("task_delete_list", { listId, expectedTaskCount }),
  taskCreate: (input: TaskInput) => invoke<TaskItem>("task_create", { input }),
  taskUpdate: (taskId: number, input: TaskInput) =>
    invoke<TaskItem>("task_update", { taskId, input }),
  taskSetCompleted: (taskId: number, completed: boolean) =>
    invoke<TaskItem>("task_set_completed", { taskId, completed }),
  taskDelete: (taskId: number) => invoke<void>("task_delete", { taskId }),
  taskSetPinned: (taskId: number | null) =>
    invoke<number | null>("task_set_pinned", { taskId }),
  taskNoteBootstrap: () => invoke<TaskItem | null>("task_note_bootstrap"),
  taskNoteComplete: () => invoke<TaskItem>("task_note_complete"),
  taskNoteUnpin: () => invoke<void>("task_note_unpin"),
};

export interface TaskInput {
  listId: number;
  title: string;
  notes: string | null;
  kind: TaskKind;
  priority: TaskPriority;
  dueDate: string | null;
  repository: TaskRepositoryInput | null;
}
