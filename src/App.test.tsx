import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { desktop, type ProviderOverview } from "./lib/desktop";

vi.mock("./lib/desktop", () => ({
  desktop: {
    available: vi.fn(),
    bootstrap: vi.fn(),
    saveSettings: vi.fn(),
    hide: vi.fn(),
    onNavigate: vi.fn(),
    saveProviderKey: vi.fn(),
    deleteProviderKey: vi.fn(),
    providerKeyStatus: vi.fn(),
  },
}));
const initial = { closeToTray: true, showMascot: true, friendlyMessages: true, hiddenProviderIds: [] as string[] };
const demoProviders: ProviderOverview[] = [
  {
    providerId: "ellie-demo",
    displayName: "Ellie Demo",
    snapshot: {
      providerId: "ellie-demo",
      displayName: "Ellie Demo",
      accountLabel: "Illustrative account",
      plan: "Demo",
      capabilities: {
        quotaWindows: true,
        tokenUsage: true,
        accountBalance: false,
        credits: false,
        costTracking: true,
        localHistory: false,
      },
      authState: "unsupported",
      hasSubscription: null,
      dataKind: "mock",
      windows: [
        {
          id: "sample-window",
          label: "Sample allowance",
          usedPercent: 41,
          remainingPercent: 59,
          resetAt: "2026-09-06T12:00:00Z",
          source: "provider_reported",
        },
      ],
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
  },
];

const liveProviders: ProviderOverview[] = [
  {
    providerId: "openai-codex",
    displayName: "OpenAI / Codex",
    snapshot: {
      providerId: "openai-codex",
      displayName: "OpenAI / Codex",
      accountLabel: "acct_…",
      plan: "plus",
      capabilities: {
        quotaWindows: true,
        tokenUsage: false,
        accountBalance: false,
        credits: true,
        costTracking: false,
        localHistory: false,
      },
      authState: "authenticated",
      hasSubscription: true,
      dataKind: "live",
      windows: [
        {
          id: "primary",
          label: "5-hour limit",
          usedPercent: 25,
          remainingPercent: 75,
          resetAt: "2026-09-06T21:00:00Z",
          source: "provider_reported",
        },
        {
          id: "secondary",
          label: "Weekly limit",
          usedPercent: 40,
          remainingPercent: 60,
          resetAt: "2026-09-07T21:00:00Z",
          source: "provider_reported",
        },
      ],
      tokenUsage: null,
      balance: null,
      balanceCurrency: null,
      spendEstimate: null,
      model: null,
      fetchedAt: "2026-09-06T14:00:00Z",
    },
    error: null,
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.saveSettings).mockImplementation(async (settings) => settings);
  vi.mocked(desktop.available).mockReturnValue(true);
  vi.mocked(desktop.providerKeyStatus).mockResolvedValue([]);
  vi.mocked(desktop.onNavigate).mockResolvedValue(() => {});
  vi.mocked(desktop.bootstrap).mockResolvedValue({
    settings: initial,
    view: "dashboard",
    providers: demoProviders,
  });
});

describe("bootstrap shell", () => {
  it("renders mock usage with clear provenance rather than live provider claims", async () => {
    render(<App />);
    await waitFor(() =>
      expect(
        screen.queryByText("Opening your local settings…"),
      ).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Ellie Demo")).toBeVisible();
    expect(screen.getAllByText("Mock data")).not.toHaveLength(0);
    expect(
      screen.getByRole("progressbar", { name: /illustrative data/ }),
    ).toHaveAttribute("aria-valuenow", "41");
    expect(
      screen.getByText(/Illustrative provider-reported sample/),
    ).toBeVisible();
    expect(screen.getByText(/Locally calculated sample/)).toBeVisible();
    expect(screen.queryByText("OpenAI / Codex")).not.toBeInTheDocument();
    expect(screen.getByText("Current usage")).toBeVisible();
    expect(
      screen.getByText(/Demo providers exercise Ellie's display/),
    ).toBeVisible();
  });

  it("renders live quota with provider provenance and no demo labels", async () => {
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial,
      view: "dashboard",
      providers: liveProviders,
    });
    render(<App />);
    await waitFor(() =>
      expect(screen.queryByText("Opening your local settings…")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("OpenAI / Codex")).toBeVisible();
    expect(screen.queryByText("Mock data")).not.toBeInTheDocument();
    expect(screen.getByText("5-hour limit")).toBeVisible();
    expect(screen.getByText("Weekly limit")).toBeVisible();
    const primary = screen.getByRole("progressbar", {
      name: "5-hour limit: 25% used, provider data",
    });
    expect(primary).toHaveAttribute("aria-valuenow", "25");
    expect(screen.getAllByText("Provider-reported").length).toBe(2);
    expect(screen.getAllByText(/Resets \d/).length).toBe(2);
    expect(
      screen.getByText(/Live data comes from your codex CLI login/),
    ).toBeVisible();
    expect(screen.queryByText("Sample reset")).not.toBeInTheDocument();
  });

  it("keeps browser preview separate from desktop settings", async () => {
    vi.mocked(desktop.available).mockReturnValue(false);
    render(<App />);
    expect(screen.getByText(/Browser preview/)).toBeVisible();
    expect(screen.getByRole("button", { name: /Hide to tray/ })).toBeDisabled();
  });

  it("saves preferences only after success and reports failures without raw errors", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.saveSettings)
      .mockRejectedValueOnce(new Error("sensitive backend detail"))
      .mockResolvedValueOnce({ ...initial, friendlyMessages: false });
    render(<App />);
    await waitFor(() =>
      expect(
        screen.queryByText("Opening your local settings…"),
      ).not.toBeInTheDocument(),
    );
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.click(
      screen.getByRole("checkbox", { name: /Friendly messages/ }),
    );
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Your settings were not saved",
    );
    expect(
      screen.queryByText("sensitive backend detail"),
    ).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(
      await screen.findByText("Settings saved on this device."),
    ).toBeVisible();
    expect(desktop.saveSettings).toHaveBeenLastCalledWith({
      ...initial,
      friendlyMessages: false,
    });
    await user.click(screen.getByRole("button", { name: "Overview" }));
    expect(
      screen.queryByText("There you are. I saved your spot."),
    ).not.toBeInTheDocument();
  });

  it("hides explicitly unsubscribed providers and brings them back on resubscribe", async () => {
    const unsubscribed: ProviderOverview[] = [
      {
        providerId: "openai-codex",
        displayName: "OpenAI / Codex",
        snapshot: {
          providerId: "openai-codex",
          displayName: "OpenAI / Codex",
          accountLabel: null,
          plan: "free",
          capabilities: {
            quotaWindows: true,
            tokenUsage: false,
            accountBalance: false,
            credits: false,
            costTracking: false,
            localHistory: false,
          },
          authState: "authenticated",
          hasSubscription: false,
          dataKind: "live",
          windows: [
            {
              id: "primary",
              label: "5-hour limit",
              usedPercent: 5,
              remainingPercent: 95,
              resetAt: null,
              source: "provider_reported",
            },
          ],
          tokenUsage: null,
          balance: null,
          balanceCurrency: null,
          spendEstimate: null,
          model: null,
          fetchedAt: "2026-09-06T14:00:00Z",
        },
        error: null,
      },
      ...demoProviders,
    ];
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial,
      view: "dashboard",
      providers: unsubscribed,
    });
    const first = render(<App />);
    await waitFor(() =>
      expect(screen.queryByText("Opening your local settings…")).not.toBeInTheDocument(),
    );
    // The unsubscribed card is hidden; the demo card stays.
    expect(screen.queryByText("OpenAI / Codex")).not.toBeInTheDocument();
    expect(screen.getByText("Ellie Demo")).toBeVisible();
    expect(screen.getByText("1 shown · 1 hidden")).toBeVisible();
    expect(screen.getByText(/Demo providers exercise Ellie's display/)).toBeVisible();

    // Resubscribing flips the flag on the next refresh and the card returns.
    const resubscribed: ProviderOverview[] = unsubscribed.map((provider) =>
      provider.snapshot
        ? {
            ...provider,
            snapshot: { ...provider.snapshot, plan: "plus", hasSubscription: true },
          }
        : provider,
    );
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial,
      view: "dashboard",
      providers: resubscribed,
    });
    first.unmount();
    render(<App />);
    await waitFor(() => expect(screen.getByText("OpenAI / Codex")).toBeVisible());
  });

  it("hides unconfigured providers but keeps transient errors visible", async () => {
    const mixed: ProviderOverview[] = [
      {
        providerId: "anthropic-claude",
        displayName: "Anthropic / Claude",
        snapshot: null,
        error: "authentication_required", // no ANTHROPIC_API_KEY
      },
      {
        providerId: "openai-codex",
        displayName: "OpenAI / Codex",
        snapshot: null,
        error: "unavailable", // transient codex failure stays visible
      },
      ...demoProviders,
    ];
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial,
      view: "dashboard",
      providers: mixed,
    });
    render(<App />);
    await waitFor(() =>
      expect(screen.queryByText("Opening your local settings…")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("2 shown · 1 hidden")).toBeVisible();
    // Only the transient error card renders; the unconfigured one is hidden.
    const unavailableCards = screen.getAllByText("Provider unavailable");
    expect(unavailableCards).toHaveLength(1);
    expect(screen.getByText("unavailable")).toBeVisible();
    expect(screen.queryByText("authentication_required")).not.toBeInTheDocument();
    expect(screen.getByText("Ellie Demo")).toBeVisible();
  });

  it("renders provider-reported account balance with its currency", async () => {
    const deepseek: ProviderOverview[] = [
      {
        providerId: "deepseek",
        displayName: "DeepSeek",
        snapshot: {
          providerId: "deepseek",
          displayName: "DeepSeek",
          accountLabel: null,
          plan: null,
          capabilities: {
            quotaWindows: false,
            tokenUsage: false,
            accountBalance: true,
            credits: false,
            costTracking: false,
            localHistory: false,
          },
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
      },
    ];
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial,
      view: "dashboard",
      providers: deepseek,
    });
    render(<App />);
    await waitFor(() =>
      expect(screen.queryByText("Opening your local settings…")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("DeepSeek")).toBeVisible();
    expect(screen.getByText("Account balance")).toBeVisible();
    expect(screen.getByText(/110/)).toBeVisible();
    expect(screen.queryByText("Mock data")).not.toBeInTheDocument();
  });

  it("renders live token breakdown, model, and spend estimate for balance providers", async () => {
    const anthropic: ProviderOverview[] = [
      {
        providerId: "anthropic-claude",
        displayName: "Anthropic / Claude",
        snapshot: {
          providerId: "anthropic-claude",
          displayName: "Anthropic / Claude",
          accountLabel: null,
          plan: null,
          capabilities: {
            quotaWindows: false,
            tokenUsage: true,
            accountBalance: false,
            credits: false,
            costTracking: true,
            localHistory: false,
          },
          authState: "authenticated",
          hasSubscription: null,
          dataKind: "live",
          windows: [],
          tokenUsage: {
            totalTokens: 8000,
            inputTokens: 7200,
            outputTokens: 800,
            cachedInputTokens: 1700,
            requestCount: null,
            estimatedCostUsd: 2.0,
            source: "locally_calculated",
          },
          balance: 100,
          balanceCurrency: "USD",
          spendEstimate: {
            amount: 20,
            currency: "USD",
            windowDays: 10,
          },
          model: "claude-opus-5",
          fetchedAt: "2026-09-06T14:00:00Z",
        },
        error: null,
      },
    ];
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial,
      view: "dashboard",
      providers: anthropic,
    });
    render(<App />);
    await waitFor(() =>
      expect(screen.queryByText("Opening your local settings…")).not.toBeInTheDocument(),
    );
    expect(screen.getByText("Model: claude-opus-5")).toBeVisible();
    expect(screen.getByText(/7,200 in \/ 800 out/)).toBeVisible();
    expect(screen.getByText(/1,700 cached input/)).toBeVisible();
    expect(screen.getByText("Token activity (last 30 days)")).toBeVisible();
    expect(screen.getByText(/≈ spent \(last 10 days\)/)).toBeVisible();
    expect(screen.queryByText("Mock data")).not.toBeInTheDocument();
  });

  it("saves and reports provider API keys in settings without echoing them", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.providerKeyStatus).mockResolvedValue([
      { providerId: "anthropic-claude", source: "none" },
      { providerId: "deepseek", source: "credential_manager" },
    ]);
    vi.mocked(desktop.saveProviderKey).mockResolvedValue(undefined);
    render(<App />);
    await user.click(screen.getByRole("button", { name: "Settings" }));
    await user.type(
      screen.getByLabelText("DeepSeek API key"),
      "sk-test-secret",
    );
    await user.click(
      screen.getByRole("button", { name: "Save DeepSeek API key" }),
    );
    await waitFor(() =>
      expect(desktop.saveProviderKey).toHaveBeenCalledWith(
        "deepseek",
        "sk-test-secret",
      ),
    );
    expect(screen.queryByText("sk-test-secret")).not.toBeInTheDocument();
    expect(screen.getByText(/Saved on this device/)).toBeVisible();
  });

  it("hides the demo only after saving and restores it from Settings", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: initial, view: "dashboard", providers: [...demoProviders, ...liveProviders],
    });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Hide Ellie Demo" }));
    await waitFor(() => expect(screen.queryByRole("heading", { name: "Ellie Demo" })).not.toBeInTheDocument());
    expect(screen.getByRole("heading", { name: "OpenAI / Codex" })).toBeVisible();
    expect(desktop.saveSettings).toHaveBeenLastCalledWith({ ...initial, hiddenProviderIds: ["ellie-demo"] });
    expect(screen.getByText("1 shown · 1 hidden")).toBeVisible();
    expect(desktop.bootstrap).toHaveBeenCalledTimes(1);
    expect(desktop.deleteProviderKey).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Settings" }));
    const toggle = screen.getByRole("checkbox", { name: /Show Ellie Demo on dashboard/ });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    await waitFor(() => expect(toggle).toBeChecked());
    expect(desktop.saveSettings).toHaveBeenLastCalledWith(initial);
    await user.click(screen.getByRole("button", { name: "Overview" }));
    expect(screen.getByRole("heading", { name: "Ellie Demo" })).toBeVisible();
  });

  it("loads persisted hidden cards and can hide a failed provider by its registry identity", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: { ...initial, hiddenProviderIds: ["ellie-demo"] }, view: "dashboard",
      providers: [...demoProviders, { providerId: "deepseek", displayName: "DeepSeek", snapshot: null, error: "unavailable" }],
    });
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Hide DeepSeek" }));
    await waitFor(() => expect(screen.queryByRole("heading", { name: "DeepSeek" })).not.toBeInTheDocument());
    expect(screen.queryByRole("heading", { name: "Ellie Demo" })).not.toBeInTheDocument();
    expect(screen.getByText("0 shown · 2 hidden")).toBeVisible();
    expect(screen.getByText(/No visible providers/)).toBeVisible();
    expect(desktop.saveSettings).toHaveBeenLastCalledWith({ ...initial, hiddenProviderIds: ["ellie-demo", "deepseek"] });
    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("checkbox", { name: /Show Ellie Demo/ })).not.toBeChecked();
    expect(screen.getByRole("checkbox", { name: /Show DeepSeek/ })).not.toBeChecked();
  });

  it("keeps the card on save failure and disables hide while saving", async () => {
    const user = userEvent.setup();
    let rejectSave!: (error: Error) => void;
    vi.mocked(desktop.saveSettings).mockReturnValueOnce(new Promise((_, reject) => { rejectSave = reject; }));
    render(<App />);
    const hide = await screen.findByRole("button", { name: "Hide Ellie Demo" });
    await user.click(hide);
    expect(hide).toBeDisabled();
    expect(screen.getByRole("heading", { name: "Ellie Demo" })).toBeVisible();
    rejectSave(new Error("sensitive storage failure"));
    expect(await screen.findByRole("alert")).toHaveTextContent("Provider visibility was not saved");
    expect(screen.queryByText("sensitive storage failure")).not.toBeInTheDocument();
    expect(hide).toBeEnabled();
    expect(screen.getByRole("heading", { name: "Ellie Demo" })).toBeVisible();
  });

  it("preserves unsaved appearance edits when restoring a provider", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: { ...initial, hiddenProviderIds: ["ellie-demo"] }, view: "settings", providers: demoProviders,
    });
    render(<App />);
    await user.click(await screen.findByRole("checkbox", { name: /Friendly messages/ }));
    await user.click(screen.getByRole("checkbox", { name: /Show Ellie Demo/ }));
    await waitFor(() => expect(screen.getByRole("checkbox", { name: /Show Ellie Demo/ })).toBeChecked());
    expect(desktop.saveSettings).toHaveBeenLastCalledWith(initial);
    expect(screen.getByRole("checkbox", { name: /Friendly messages/ })).not.toBeChecked();
    await user.click(screen.getByRole("button", { name: "Save settings" }));
    expect(desktop.saveSettings).toHaveBeenLastCalledWith({ ...initial, friendlyMessages: false });
  });

  it("keeps failed restores hidden and preserves automatic authentication hiding", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.bootstrap).mockResolvedValue({
      settings: { ...initial, hiddenProviderIds: ["ellie-demo", "deepseek"] }, view: "settings",
      providers: [...demoProviders, { providerId: "deepseek", displayName: "DeepSeek", snapshot: null, error: "authentication_required" }],
    });
    vi.mocked(desktop.saveSettings).mockRejectedValueOnce(new Error("secret"));
    render(<App />);
    const demo = await screen.findByRole("checkbox", { name: /Show Ellie Demo/ });
    await user.click(demo);
    expect(await screen.findByRole("alert")).toHaveTextContent("Provider visibility was not saved");
    expect(demo).not.toBeChecked();
    await user.click(screen.getByRole("checkbox", { name: /Show DeepSeek/ }));
    await waitFor(() => expect(screen.getByRole("checkbox", { name: /Show DeepSeek/ })).toBeChecked());
    await user.click(screen.getByRole("button", { name: "Overview" }));
    expect(screen.queryByRole("heading", { name: "DeepSeek" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Ellie Demo" })).not.toBeInTheDocument();
  });

  it("offers retry when settings cannot be loaded", async () => {
    vi.mocked(desktop.bootstrap).mockRejectedValueOnce(
      new Error("unavailable"),
    );
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("button", { name: "Retry" }));
    await waitFor(() =>
      expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
    );
  });
});
