import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { desktop, type ProviderOverview } from "./lib/desktop";

vi.mock("./lib/desktop", () => ({
  isView: (value: unknown) =>
    value === "dashboard" ||
    value === "ai-usage" ||
    value === "github" ||
    value === "todos" ||
    value === "history" ||
    value === "settings",
  desktop: {
    available: vi.fn(),
    localApiStatus: vi.fn().mockResolvedValue({ enabled: false, listening: false, tokenSource: "none", error: null }),
    configureLocalApi: vi.fn(),
    bootstrap: vi.fn(),
    getAnalytics: vi.fn(),
    saveSettings: vi.fn(),
    hide: vi.fn(),
    onNavigate: vi.fn(),
    saveProviderKey: vi.fn(),
    deleteProviderKey: vi.fn(),
    providerKeyStatus: vi.fn(),
    onProvidersUpdated: vi.fn(),
    onTasksUpdated: vi.fn(),
    refreshAll: vi.fn(),
    refreshProvider: vi.fn(),
    githubConnectionStatus: vi.fn(),
    githubSaveClientId: vi.fn(),
    githubSaveClientSecret: vi.fn(),
    githubSignIn: vi.fn(),
    githubCancelSignIn: vi.fn(),
    githubDisconnect: vi.fn(),
    githubListRepositories: vi.fn(),
    githubListCommits: vi.fn(),
    githubContributionCalendar: vi.fn(),
    githubPrepareRepositoryCreation: vi.fn(),
    githubConfirmRepositoryCreation: vi.fn(),
    githubRepositoryCreationStatus: vi.fn(),
    githubResolveRepositoryCreation: vi.fn(),
    taskBootstrap: vi.fn(),
    taskList: vi.fn(),
    taskCreateList: vi.fn(),
    taskRenameList: vi.fn(),
    taskListDeletePreview: vi.fn(),
    taskDeleteList: vi.fn(),
    taskCreate: vi.fn(),
    taskUpdate: vi.fn(),
    taskSetCompleted: vi.fn(),
    taskDelete: vi.fn(),
    taskSetPinned: vi.fn(),
  },
}));

const initial = {
  closeToTray: true,
  showMascot: true,
  friendlyMessages: true,
  notificationsEnabled: true,
  notificationThresholds: [75, 90, 95] as [number, number, number],
  hiddenProviderIds: [] as string[],
  miniBarEnabled: false,
  miniBarOpacity: 0.9,
  miniBarX: null as number | null,
  miniBarY: null as number | null,
};

const initialAnalytics = {
  range: "sevenDays" as const,
  startAt: "2026-09-01T00:00:00Z",
  endAt: "2026-09-07T12:00:00Z",
  snapshotCount: 2,
  providerCount: 1,
  latestTotalTokens: 1_000,
  latestRequestCount: 3,
  tokenSource: "locally_calculated" as const,
  estimatedSpend: [{ currency: "USD", amount: 1.25, source: "locally_calculated" as const }],
  providers: [{
    providerId: "openai-codex",
    displayName: "OpenAI / Codex",
    model: "gpt-5",
    latestAt: "2026-09-07T12:00:00Z",
    totalTokens: 1_000,
    requestCount: 3,
    tokenSource: "locally_calculated" as const,
  }],
  tokenSeries: [{ date: "2026-09-07", totalTokens: 1_000 }],
  quotaWindows: [{
    providerId: "openai-codex",
    displayName: "OpenAI / Codex",
    windowId: "weekly",
    windowLabel: "Weekly limit",
    usedPercent: 42,
    remainingPercent: 58,
    observedAt: "2026-09-07T12:00:00Z",
  }],
};

const emptyAnalytics = {
  ...initialAnalytics,
  snapshotCount: 0,
  providerCount: 0,
  latestTotalTokens: null,
  latestRequestCount: null,
  tokenSource: null,
  estimatedSpend: [],
  providers: [],
  tokenSeries: [],
  quotaWindows: [],
};

const demoProviders: ProviderOverview[] = [{
  providerId: "ellie-demo",
  displayName: "Ellie Demo",
  snapshot: {
    providerId: "ellie-demo",
    displayName: "Ellie Demo",
    accountLabel: "Illustrative account",
    plan: "Demo",
    capabilities: { quotaWindows: true, tokenUsage: true, accountBalance: false, credits: false, costTracking: true, localHistory: false },
    authState: "unsupported",
    hasSubscription: null,
    dataKind: "mock",
    windows: [{
      id: "sample-window",
      label: "Sample allowance",
      usedPercent: 41,
      remainingPercent: 59,
      resetAt: "2026-09-06T12:00:00Z",
      source: "provider_reported",
    }],
    tokenUsage: {
      totalTokens: 145000,
      requestCount: 28,
      estimatedCostUsd: 0.42,
      source: "locally_calculated",
    },
    balance: null,
    balanceCurrency: null,
    spendEstimate: null,
    model: null,
    fetchedAt: "2026-09-06T08:00:00Z",
  },
  error: null,
}];

const liveProviders: ProviderOverview[] = [{
  providerId: "openai-codex",
  displayName: "OpenAI / Codex",
  snapshot: {
    providerId: "openai-codex",
    displayName: "OpenAI / Codex",
    accountLabel: "acct_…",
    plan: "plus",
    capabilities: { quotaWindows: true, tokenUsage: false, accountBalance: false, credits: true, costTracking: false, localHistory: false },
    authState: "authenticated",
    hasSubscription: true,
    dataKind: "live",
    windows: [
      { id: "primary", label: "5-hour limit", usedPercent: 25, remainingPercent: 75, resetAt: "2026-09-06T21:00:00Z", source: "provider_reported" },
      { id: "secondary", label: "Weekly limit", usedPercent: 40, remainingPercent: 60, resetAt: "2026-09-07T21:00:00Z", source: "provider_reported" },
    ],
    tokenUsage: null,
    balance: null,
    balanceCurrency: null,
    spendEstimate: null,
    model: null,
    fetchedAt: "2026-09-06T14:00:00Z",
  },
  error: null,
}];

const taskFixtures = {
  lists: [
    { id: 1, name: "Focus", taskCount: 1, createdAt: "2026-09-01T00:00:00Z", updatedAt: "2026-09-01T00:00:00Z" },
  ],
  tasks: [
    {
      id: 11,
      listId: 1,
      title: "Ship the dashboard",
      notes: null,
      kind: "work" as const,
      priority: "high" as const,
      dueDate: "2026-09-20",
      repository: null,
      completedAt: null,
      createdAt: "2026-09-01T00:00:00Z",
      updatedAt: "2026-09-01T00:00:00Z",
    },
  ],
  pinnedTaskId: 11,
};

const disconnectedStatus = {
  state: "Disconnected" as const,
  account: null,
  lastError: null,
  tokenPresent: false,
  clientIdConfigured: false,
  clientSecretConfigured: false,
};

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.localApiStatus).mockResolvedValue({ enabled: false, listening: false, tokenSource: "none", error: null });
  vi.mocked(desktop.saveSettings).mockImplementation(async (settings) => settings);
  vi.mocked(desktop.available).mockReturnValue(true);
  vi.mocked(desktop.providerKeyStatus).mockResolvedValue([]);
  vi.mocked(desktop.getAnalytics).mockResolvedValue(emptyAnalytics);
  vi.mocked(desktop.onNavigate).mockResolvedValue(() => {});
  vi.mocked(desktop.onProvidersUpdated).mockResolvedValue(() => {});
  vi.mocked(desktop.onTasksUpdated).mockResolvedValue(() => {});
  vi.mocked(desktop.refreshAll).mockResolvedValue({ providers: demoProviders, refreshed: true, busy: false });
  vi.mocked(desktop.refreshProvider).mockResolvedValue({ providers: demoProviders, refreshed: true, busy: false });
  vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(disconnectedStatus);
  vi.mocked(desktop.githubListRepositories).mockResolvedValue([]);
  vi.mocked(desktop.githubListCommits).mockResolvedValue([]);
  vi.mocked(desktop.githubContributionCalendar).mockResolvedValue({ totalContributions: 0, startedOn: "2026-01-01", endedOn: "2026-12-31", weeks: [] });
  vi.mocked(desktop.githubRepositoryCreationStatus).mockResolvedValue([]);
  vi.mocked(desktop.taskBootstrap).mockResolvedValue({ lists: [], tasks: [], pinnedTaskId: null });
  vi.mocked(desktop.taskList).mockResolvedValue([]);
  vi.mocked(desktop.taskListDeletePreview).mockResolvedValue({ listId: 1, taskCount: 1 });
  vi.mocked(desktop.saveProviderKey).mockResolvedValue(undefined);
  vi.mocked(desktop.deleteProviderKey).mockResolvedValue(undefined);
  vi.mocked(desktop.taskSetCompleted).mockImplementation(async (taskId, completed) => ({
    ...taskFixtures.tasks[0]!,
    completedAt: completed ? "2026-09-08T10:00:00Z" : null,
    id: taskId,
  }));
  vi.mocked(desktop.taskSetPinned).mockImplementation(async (taskId) => taskId);
  vi.mocked(desktop.bootstrap).mockResolvedValue({
    settings: initial,
    view: "dashboard",
    providers: demoProviders,
  });
});

async function renderApp() {
  render(<App />);
  await waitFor(() =>
    expect(screen.queryByText("Opening your local settings…")).not.toBeInTheDocument(),
  );
}

describe("shell and navigation", () => {
  it("renders the command-center shell with six destinations", async () => {
    await renderApp();
    expect(screen.getByRole("heading", { name: "Everything important, one glance." })).toBeVisible();
    for (const name of ["Overview", "AI Usage", "GitHub", "To-do", "History", "Settings"]) {
      expect(screen.getByRole("button", { name })).toBeVisible();
    }
    expect(screen.getByText("Local to this device")).toBeVisible();
  });

  it("navigates between all views and marks the active page", async () => {
    const user = userEvent.setup();
    await renderApp();
    for (const [name, heading] of [
      ["AI Usage", "AI Usage"],
      ["GitHub", "GitHub"],
      ["To-do", "My tasks"],
      ["History", "History"],
      ["Settings", "Settings"],
    ]) {
      const nav = screen.getByRole("button", { name });
      await user.click(nav);
      expect(nav).toHaveAttribute("aria-current", "page");
      expect(screen.getByRole("heading", { name: heading })).toBeVisible();
    }
    await user.click(screen.getByRole("button", { name: "Overview" }));
    expect(screen.getByRole("button", { name: "Overview" })).toHaveAttribute("aria-current", "page");
  });

  it("opens the command palette and jumps to a view", async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "Quick actions (Ctrl+K)" }));
    expect(
      await screen.findByRole("option", { name: /Create a repository/ }),
    ).toBeDisabled();
    await user.click(screen.getByRole("option", { name: /Open History/ }));
    expect(screen.getByRole("heading", { name: "History" })).toBeVisible();
  });
});

describe("dashboard derivation (no invented data)", () => {
  it("summarizes live allowance, health, github, and tasks from real data", async () => {
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial,
      view: "dashboard",
      providers: liveProviders,
    });
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue({
      ...disconnectedStatus,
      state: "Connected",
      account: { id: 7, login: "octocat" },
      tokenPresent: true,
      clientIdConfigured: true,
      clientSecretConfigured: true,
    });
    vi.mocked(desktop.taskBootstrap).mockResolvedValue(taskFixtures);
    render(<App />);
    // Lowest reported allowance KPI: 60% weekly / 75% 5-hour → 60%.
    expect(await screen.findByText("60%")).toBeVisible();
    // Provider health: 1 live of 1 visible.
    expect(screen.getByText("1/1")).toBeVisible();
    expect(screen.getByText("Connected")).toBeVisible();
    expect(screen.getByText("1")).toBeVisible(); // open tasks KPI
    // Pinned task appears on the Overview focus card.
    expect(await screen.findByText("Ship the dashboard")).toBeVisible();
    // A disconnected state would not show zero activity; it invites connection.
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(disconnectedStatus);
  });

  it("shows no invented data when disconnected and empty", async () => {
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial,
      view: "dashboard",
      providers: [],
    });
    await renderApp();
    expect(screen.getAllByText("GitHub not connected").length).toBeGreaterThan(0);
    expect(screen.getByText("No current task")).toBeVisible();
    expect(screen.getByText("No visible provider data")).toBeVisible();
  });
});

describe("provider provenance and detail renders", () => {
  it("renders mock usage with clear provenance rather than live provider claims", async () => {
    await renderApp();
    expect(screen.getAllByText("Ellie Demo").length).toBeGreaterThan(0);
    expect(screen.getByText("Mock data")).toBeVisible();
    expect(screen.getByRole("progressbar", { name: /illustrative data/ })).toHaveAttribute("aria-valuenow", "41");
    expect(screen.getByText(/Illustrative provider-reported sample/)).toBeVisible();
    expect(screen.getByText(/Locally calculated sample/)).toBeVisible();
    expect(screen.queryByText("OpenAI / Codex")).not.toBeInTheDocument();
  });

  it("renders live quota with provider provenance and no demo labels", async () => {
    vi.mocked(desktop.bootstrap).mockResolvedValue({ settings: initial, view: "dashboard", providers: liveProviders });
    render(<App />);
    expect(await screen.findByText("5-hour limit")).toBeVisible();
    expect(screen.getByText("Weekly limit")).toBeVisible();
    expect(screen.getByRole("progressbar", { name: "5-hour limit: 25% used, provider data" })).toHaveAttribute("aria-valuenow", "25");
    expect(await screen.findByText(/75% remaining/)).toBeVisible();
    expect(screen.queryByText("Mock data")).not.toBeInTheDocument();
    expect(await screen.findAllByText(/Resets/)).not.toHaveLength(0);
  });

  it("omits unavailable token breakdowns", async () => {
    const snapshot = liveProviders[0]!.snapshot!;
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial, view: "dashboard",
      providers: [{ ...liveProviders[0]!, snapshot: {
        ...snapshot, capabilities: { ...snapshot.capabilities, tokenUsage: true }, tokenUsage: {
          totalTokens: 149_655_123, inputTokens: null, outputTokens: null,
          cachedInputTokens: null, requestCount: null, estimatedCostUsd: null,
          source: "locally_calculated",
        },
      } }],
    });
    render(<App />);
    expect(await screen.findByText("149,655,123 tokens")).toBeVisible();
  });

  it("renders provider-reported account balance with its currency", async () => {
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial, view: "dashboard",
      providers: [{
        providerId: "deepseek",
        displayName: "DeepSeek",
        snapshot: {
          providerId: "deepseek",
          displayName: "DeepSeek",
          accountLabel: null,
          plan: null,
          capabilities: { quotaWindows: false, tokenUsage: false, accountBalance: true, credits: false, costTracking: false, localHistory: false },
          authState: "authenticated",
          hasSubscription: null,
          dataKind: "live",
          windows: [],
          tokenUsage: null,
          balance: 110,
          balanceCurrency: "CNY",
          spendEstimate: null,
          model: null,
          fetchedAt: "2026-09-06T14:00:00Z",
        },
        error: null,
      }],
    });
    render(<App />);
    expect(await screen.findByRole("heading", { name: "DeepSeek" })).toBeVisible();
    expect(screen.getByText("Balance")).toBeVisible();
    expect(screen.getByText(/110/)).toBeVisible();
    expect(screen.queryByText("Mock data")).not.toBeInTheDocument();
  });
});

describe("settings behavior regressions", () => {
  it("saves preferences from the settings form", async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("checkbox", { name: /Mini floating bar/ }));
    fireEvent.change(screen.getByRole("slider", { name: "Mini bar opacity" }), { target: { value: "0.7" } });
    expect(screen.getByText("70%")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    await waitFor(() => expect(desktop.saveSettings).toHaveBeenLastCalledWith({
      ...initial, miniBarEnabled: true, miniBarOpacity: 0.7,
    }));
  });

  it("keeps prior settings active after a failed save without raw errors", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.saveSettings).mockRejectedValue(new Error("sensitive backend detail"));
    await renderApp();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(screen.getByRole("checkbox", { name: /Friendly messages/ }));
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    await waitFor(() => expect(screen.getByText(/Your settings were not saved/)).toBeVisible());
    expect(screen.queryByText("sensitive backend detail")).not.toBeInTheDocument();
  });

  it("hides a provider from the dashboard and restores it in Settings", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.bootstrap).mockResolvedValue({ settings: initial, view: "dashboard", providers: [...demoProviders, ...liveProviders] });
    await renderApp();
    await user.click(await screen.findByRole("button", { name: "Hide Ellie Demo" }));
    await waitFor(() => expect(desktop.saveSettings).toHaveBeenLastCalledWith({ ...initial, hiddenProviderIds: ["ellie-demo"] }));
    expect(screen.queryByText("Ellie Demo")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    const toggle = screen.getByRole("checkbox", { name: /Show Ellie Demo on dashboard/ });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    await waitFor(() => expect(toggle).toBeChecked());
  });

  it("saves provider keys without echoing them", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.providerKeyStatus).mockResolvedValue([{ providerId: "deepseek", source: "none" }]);
    await renderApp();
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.type(screen.getByLabelText("DeepSeek API key"), "sk-test-secret");
    await user.click(screen.getByRole("button", { name: "Save DeepSeek key" }));
    await waitFor(() => expect(desktop.saveProviderKey).toHaveBeenCalledWith("deepseek", "sk-test-secret"));
    expect(screen.queryByText("sk-test-secret")).not.toBeInTheDocument();
  });

  it("offers GitHub connection controls inside Settings", async () => {
    await renderApp();
    await userEvent.setup().click(screen.getByRole("button", { name: "Settings" }));
    expect(await screen.findByText(/No GitHub App Client ID saved yet/)).toBeVisible();
    expect(screen.getByLabelText("GitHub App Client ID")).toBeEnabled();
    expect(screen.getByText(/stored in Windows Credential Manager and are never displayed/)).toBeVisible();
  });
});

describe("provider refresh and lifecycle", () => {
  it("refreshes all providers and keeps stale-data messaging", async () => {
    const user = userEvent.setup();
    const stale = { ...demoProviders[0]!, error: "unavailable" as const, stale: true, lastSuccessfulRefresh: new Date(Date.now() - 120_000).toISOString() };
    vi.mocked(desktop.refreshAll).mockResolvedValue({ providers: [stale], refreshed: true, busy: false });
    await renderApp();
    await user.click(await screen.findByRole("button", { name: "Refresh providers" }));
    await waitFor(() => expect(desktop.refreshAll).toHaveBeenCalledTimes(1));
    expect(await screen.findByText(/Showing data from .*refresh failed/)).toBeVisible();  });

  it("accepts background provider updates", async () => {
    let update!: (providers: ProviderOverview[]) => void;
    vi.mocked(desktop.onProvidersUpdated).mockImplementation(async (callback) => {
      update = callback;
      return () => {};
    });
    await renderApp();
    update([]);
    await waitFor(() => expect(screen.getByText("0 connected")).toBeVisible());
  });

  it("offers retry when settings cannot be loaded", async () => {
    vi.mocked(desktop.bootstrap).mockRejectedValueOnce(new Error("unavailable"));
    render(<App />);
    await userEvent.setup().click(await screen.findByRole("button", { name: "Retry" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("keeps browser preview separate", async () => {
    vi.mocked(desktop.available).mockReturnValue(false);
    render(<App />);
    expect(await screen.findByText(/Browser preview/)).toBeVisible();
    expect(screen.getByText(/Desktop settings, tray controls, GitHub sign-in/)).toBeVisible();
  });
});

describe("to-do workflows", () => {
  it("lists tasks, completes them, and pins a focus task", async () => {
    vi.mocked(desktop.taskBootstrap).mockResolvedValue(taskFixtures);
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "To-do" }));
    expect(await screen.findByText("Ship the dashboard")).toBeVisible();
    await user.click(screen.getByRole("checkbox", { name: /Mark "Ship the dashboard" complete/ }));
    await waitFor(() => expect(desktop.taskSetCompleted).toHaveBeenCalledWith(11, true));
  });

  it("pins an open task as the desktop sticky note", async () => {
    vi.mocked(desktop.taskBootstrap).mockResolvedValue({
      ...taskFixtures,
      pinnedTaskId: null,
    });
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "To-do" }));
    await user.click(await screen.findByRole("button", { name: "Pin Ship the dashboard as sticky note" }));
    await waitFor(() => expect(desktop.taskSetPinned).toHaveBeenCalledWith(11));
  });

  it("creates a task from the new-task dialog", async () => {
    vi.mocked(desktop.taskBootstrap).mockResolvedValue({ lists: taskFixtures.lists as never, tasks: [], pinnedTaskId: null });
    vi.mocked(desktop.taskCreate).mockResolvedValue({ ...taskFixtures.tasks[0]!, id: 12 });
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "To-do" }));
    await user.click(await screen.findByRole("button", { name: "New task" }));
    await user.type(await screen.findByLabelText("Task name"), "Write tests");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    await waitFor(() => expect(desktop.taskCreate).toHaveBeenCalled());
    expect(desktop.taskCreate).toHaveBeenCalledWith(expect.objectContaining({
      title: "Write tests",
      kind: "personal",
      listId: 1,
      repository: null,
    }));
  });

  it("stores an explicit work type without requiring a repository", async () => {
    vi.mocked(desktop.taskBootstrap).mockResolvedValue({ lists: taskFixtures.lists as never, tasks: [], pinnedTaskId: null });
    vi.mocked(desktop.taskCreate).mockResolvedValue({ ...taskFixtures.tasks[0]!, id: 12 });
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "To-do" }));
    await user.click(await screen.findByRole("button", { name: "New task" }));
    await user.click(screen.getByRole("radio", { name: /Work/ }));
    await user.type(screen.getByLabelText("Task name"), "Redesign Ellie");
    await user.click(screen.getByRole("button", { name: "Add task" }));
    await waitFor(() => expect(desktop.taskCreate).toHaveBeenCalledWith(expect.objectContaining({
      title: "Redesign Ellie",
      kind: "work",
      repository: null,
    })));
  });

  it("uses one Rust-created task board without list setup controls", async () => {
    vi.mocked(desktop.taskBootstrap).mockResolvedValue({ lists: taskFixtures.lists as never, tasks: [], pinnedTaskId: null });
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "To-do" }));
    expect(await screen.findByRole("heading", { name: "My tasks" })).toBeVisible();
    expect(screen.queryByText("Create a list")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Task lists")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "New task" }));
    expect(await screen.findByRole("heading", { name: "New task" })).toBeVisible();
    expect(screen.getByLabelText("Task name")).toBeEnabled();
    expect(screen.queryByLabelText("List")).not.toBeInTheDocument();
  });

  it("allows editing a task whose due date is already overdue", async () => {
    vi.mocked(desktop.taskBootstrap).mockResolvedValue({
      lists: taskFixtures.lists as never,
      tasks: [{ ...taskFixtures.tasks[0]!, dueDate: "2020-01-01" }] as never,
      pinnedTaskId: null,
    });
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "To-do" }));
    await user.click(await screen.findByRole("button", { name: "Ship the dashboard" }));
    expect(screen.getByLabelText("Due date")).not.toHaveAttribute("min");
  });
});

describe("GitHub page and repository creation", () => {
  it("invites connection instead of showing zero activity", async () => {
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "GitHub" }));
    expect(await screen.findByText("Connect to load repositories")).toBeVisible();
    expect(screen.getByRole("button", { name: "Connect GitHub" })).toBeDisabled();
    expect(screen.queryByText(/0 repositories/)).not.toBeInTheDocument();
  });

  it("walks the private repository creation flow with review confirmation", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue({
      ...disconnectedStatus, state: "Connected", account: { id: 7, login: "octocat" },
      tokenPresent: true, clientIdConfigured: true, clientSecretConfigured: true,
    });
    vi.mocked(desktop.githubListRepositories).mockResolvedValue([
      { id: 1, name: "ellie", fullName: "octocat/ellie", private: true, defaultBranch: "main", htmlUrl: "https://github.com/octocat/ellie" },
    ]);
    vi.mocked(desktop.githubPrepareRepositoryCreation).mockResolvedValue({
      reviewId: "review-1",
      owner: "octocat",
      name: "personal-notes",
      description: "My notes",
      private: true,
      initializeReadme: false,
      expiresAt: "2026-09-08T12:00:00Z",
    });
    vi.mocked(desktop.githubConfirmRepositoryCreation).mockResolvedValue({
      id: 99, name: "personal-notes", fullName: "octocat/personal-notes", private: true, defaultBranch: "main", htmlUrl: "https://github.com/octocat/personal-notes",
    });
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "GitHub" }));
    await user.click(await screen.findByRole("button", { name: "New repository" }));
    await user.type(await screen.findByLabelText("Repository name"), "personal-notes");
    await user.click(screen.getByRole("button", { name: "Continue to review" }));
    expect(await screen.findByText("Review before creating")).toBeVisible();
    expect(screen.getByText("octocat")).toBeVisible();
    expect(screen.getByText("personal-notes")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Create private repository" }));
    expect(await screen.findByText("Repository created")).toBeVisible();
    expect(screen.getByRole("link", { name: /Open repository on GitHub/ })).toHaveAttribute(
      "href",
      "https://github.com/octocat/personal-notes",
    );
  });
});

describe("history analytics", () => {
  it("shows local analytics with real charts and range switching", async () => {
    vi.mocked(desktop.getAnalytics).mockResolvedValue(initialAnalytics);
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "History" }));
    expect(await screen.findByText("Token activity")).toBeVisible();
    expect(screen.getByText("Quota utilization")).toBeVisible();
    await user.click(screen.getByRole("button", { name: "30 days" }));
    await waitFor(() => expect(desktop.getAnalytics).toHaveBeenLastCalledWith("thirtyDays"));
  });

  it("explains missing history metrics", async () => {
    vi.mocked(desktop.getAnalytics).mockResolvedValue({ ...initialAnalytics, latestRequestCount: null, estimatedSpend: [] });
    const user = userEvent.setup();
    await renderApp();
    await user.click(screen.getByRole("button", { name: "History" }));
    expect(await screen.findByText("No request counts in the selected history")).toBeVisible();
    expect(screen.getByText("No cost estimates in the selected history")).toBeVisible();
    expect(screen.getAllByText("Not available")).toHaveLength(2);
  });
});