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
  },
}));
const initial = { closeToTray: true, showMascot: true, friendlyMessages: true };
const demoProviders: ProviderOverview[] = [
  {
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
      fetchedAt: "2026-09-06T08:00:00Z",
    },
    error: null,
  },
];

const liveProviders: ProviderOverview[] = [
  {
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
      fetchedAt: "2026-09-06T14:00:00Z",
    },
    error: null,
  },
];

beforeEach(() => {
  vi.mocked(desktop.available).mockReturnValue(true);
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
        snapshot: null,
        error: "authentication_required", // no ANTHROPIC_API_KEY
      },
      {
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
