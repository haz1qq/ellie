import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MiniBar from "./MiniBar";
import { selectMiniQuotaMetrics } from "../lib/miniQuota";
import {
  desktop,
  type ProviderOverview,
  type Settings,
} from "../lib/desktop";

const windowMocks = vi.hoisted(() => ({
  startDragging: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ startDragging: windowMocks.startDragging }),
}));

vi.mock("../lib/desktop", () => ({
  desktop: {
    available: vi.fn(),
    miniBootstrap: vi.fn(),
    openMainWindow: vi.fn(),
    onProvidersUpdated: vi.fn(),
    onMiniSettingsUpdated: vi.fn(),
  },
}));

const settings: Settings = {
  closeToTray: true,
  showMascot: true,
  friendlyMessages: true,
  notificationsEnabled: true,
  notificationThresholds: [75, 90, 95],
  hiddenProviderIds: [],
  miniBarEnabled: true,
  miniBarOpacity: 0.65,
  miniBarX: null,
  miniBarY: null,
};

function provider(
  remainingPercent: number | null = 75,
  overrides: Partial<ProviderOverview> = {},
): ProviderOverview {
  return {
    providerId: "openai-codex",
    displayName: "OpenAI / Codex",
    snapshot: {
      providerId: "openai-codex",
      displayName: "OpenAI / Codex",
      accountLabel: null,
      plan: "plus",
      hasSubscription: true,
      capabilities: {
        quotaWindows: true,
        tokenUsage: false,
        accountBalance: false,
        credits: false,
        costTracking: false,
        localHistory: true,
      },
      authState: "authenticated",
      dataKind: "live",
      windows: [{
        id: "five-hour",
        label: "5-hour limit",
        usedPercent: remainingPercent === null ? null : 100 - remainingPercent,
        remainingPercent,
        resetAt: null,
        source: "provider_reported",
      }],
      tokenUsage: null,
      balance: null,
      balanceCurrency: null,
      spendEstimate: null,
      model: null,
      fetchedAt: "2026-01-01T00:00:00Z",
    },
    error: null,
    ...overrides,
  };
}

beforeEach(() => {
  vi.clearAllMocks();
  windowMocks.startDragging.mockResolvedValue(undefined);
  vi.mocked(desktop.available).mockReturnValue(true);
  vi.mocked(desktop.miniBootstrap).mockResolvedValue({
    settings,
    providers: [provider()],
  });
  vi.mocked(desktop.openMainWindow).mockResolvedValue(undefined);
  vi.mocked(desktop.onProvidersUpdated).mockResolvedValue(() => {});
  vi.mocked(desktop.onMiniSettingsUpdated).mockResolvedValue(() => {});
});

describe("mini quota selection", () => {
  it("selects only live provider-reported remaining quota and keeps zero", () => {
    const liveZero = provider(0);
    const local = provider(50);
    local.snapshot!.windows[0]!.source = "locally_calculated";
    const mock = provider(40);
    mock.providerId = "demo";
    mock.snapshot!.dataKind = "mock";
    const unsubscribed = provider(30);
    unsubscribed.providerId = "free";
    unsubscribed.snapshot!.hasSubscription = false;
    const noQuota = provider(20);
    noQuota.providerId = "deepseek";
    noQuota.snapshot!.capabilities.quotaWindows = false;

    expect(selectMiniQuotaMetrics(
      [liveZero, local, mock, unsubscribed, noQuota],
      [],
    )).toEqual([
      expect.objectContaining({
        providerId: "openai-codex",
        windowLabel: "5-hour limit",
        remainingPercent: 0,
      }),
    ]);
    expect(selectMiniQuotaMetrics([liveZero], ["openai-codex"])).toEqual([]);
  });

  it("retains provider and full window identities for multiple qualifying rows", () => {
    const openai = provider(75);
    openai.snapshot!.windows.push({
      id: "weekly",
      label: "Weekly limit",
      usedPercent: 40,
      remainingPercent: 60,
      resetAt: null,
      source: "provider_reported",
    });
    const claude = provider(25);
    claude.providerId = "claude";
    claude.displayName = "Anthropic / Claude";
    claude.snapshot!.providerId = "claude";
    claude.snapshot!.displayName = "Anthropic / Claude";

    expect(selectMiniQuotaMetrics([openai, claude], [])).toEqual([
      expect.objectContaining({ providerName: "OpenAI / Codex", windowLabel: "5-hour limit", remainingPercent: 75 }),
      expect.objectContaining({ providerName: "OpenAI / Codex", windowLabel: "Weekly limit", remainingPercent: 60 }),
      expect.objectContaining({ providerName: "Anthropic / Claude", windowLabel: "5-hour limit", remainingPercent: 25 }),
    ]);
  });
});

describe("mini bar", () => {
  it("renders explicit quota labels, exact opacity, and opens the main window", async () => {
    const user = userEvent.setup();
    render(<MiniBar />);
    expect(await screen.findByText("OpenAI / Codex")).toBeVisible();
    expect(screen.getByText("5-hour limit")).toBeVisible();
    expect(screen.getByText("75% remaining")).toBeVisible();
    expect(screen.getByLabelText("Ellie quota mini bar")).toHaveStyle({
      "--mini-opacity": "0.65",
    });
    const open = screen.getByRole("button", {
      name: "OpenAI / Codex, 5-hour limit, 75% remaining. Open Ellie dashboard",
    });
    expect(screen.getByRole("status")).not.toBe(open);
    expect(open).not.toContainElement(screen.getByRole("status"));
    open.focus();
    await user.keyboard("{Enter}");
    expect(desktop.openMainWindow).toHaveBeenCalledOnce();
  });

  it("marks cached quota stale without removing its value", async () => {
    vi.mocked(desktop.miniBootstrap).mockResolvedValue({
      settings,
      providers: [provider(60, {
        stale: true,
        error: "unavailable",
        lastSuccessfulRefresh: "2026-01-01T00:00:00Z",
      })],
    });
    render(<MiniBar />);
    expect(await screen.findByText("60% remaining")).toBeVisible();
    expect(screen.getByText(/Showing data from .* · refresh failed/)).toBeVisible();
  });

  it.each([
    [[], settings, "Waiting for quota data"],
    [[{ providerId: "openai-codex", displayName: "OpenAI / Codex", snapshot: null, error: null }], settings, "Waiting for quota data"],
    [[{ providerId: "openai-codex", displayName: "OpenAI / Codex", snapshot: null, error: "unavailable" }], settings, "Quota unavailable"],
    [[provider()], { ...settings, hiddenProviderIds: ["openai-codex"] }, "No providers selected"],
    [[provider(null)], settings, "Quota remaining not reported"],
    [[{ ...provider(20), snapshot: { ...provider(20).snapshot!, capabilities: { ...provider(20).snapshot!.capabilities, quotaWindows: false } } }], settings, "No provider-reported quota windows"],
  ] as [ProviderOverview[], Settings, string][])("renders the distinct empty state %#", async (providers, bootstrapSettings, message) => {
    vi.mocked(desktop.miniBootstrap).mockResolvedValue({ settings: bootstrapSettings, providers });
    render(<MiniBar />);
    expect(await screen.findByText(message)).toBeVisible();
  });

  it("starts dragging only for the primary pointer without opening the dashboard", async () => {
    render(<MiniBar />);
    await screen.findByText("75% remaining");
    const handle = screen.getByTitle("Drag mini bar");

    fireEvent(handle, new MouseEvent("pointerdown", { bubbles: true, button: 2 }));
    expect(windowMocks.startDragging).not.toHaveBeenCalled();
    fireEvent(handle, new MouseEvent("pointerdown", { bubbles: true, button: 0 }));
    await waitFor(() => expect(windowMocks.startDragging).toHaveBeenCalledOnce());
    expect(desktop.openMainWindow).not.toHaveBeenCalled();
  });

  it("does not let late bootstrap state overwrite newer provider or settings events", async () => {
    let updateProviders: ((providers: ProviderOverview[]) => void) | undefined;
    let updateSettings: ((settings: Settings) => void) | undefined;
    let resolveBootstrap: ((value: { settings: Settings; providers: ProviderOverview[] }) => void) | undefined;
    vi.mocked(desktop.onProvidersUpdated).mockImplementation(async (callback) => {
      updateProviders = callback;
      return () => {};
    });
    vi.mocked(desktop.onMiniSettingsUpdated).mockImplementation(async (callback) => {
      updateSettings = callback;
      return () => {};
    });
    vi.mocked(desktop.miniBootstrap).mockReturnValue(new Promise((resolve) => {
      resolveBootstrap = resolve;
    }));

    render(<MiniBar />);
    await waitFor(() => {
      expect(updateProviders).toBeDefined();
      expect(updateSettings).toBeDefined();
    });
    act(() => {
      updateProviders?.([provider(42)]);
      updateSettings?.({ ...settings, miniBarOpacity: 0.8 });
    });
    act(() => resolveBootstrap?.({ settings, providers: [provider(75)] }));

    expect(await screen.findByText("42% remaining")).toBeVisible();
    expect(screen.queryByText("75% remaining")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Ellie quota mini bar")).toHaveStyle({
      "--mini-opacity": "0.8",
    });
  });

  it("updates from the shared provider event", async () => {
    let update: ((providers: ProviderOverview[]) => void) | undefined;
    vi.mocked(desktop.onProvidersUpdated).mockImplementation(async (callback) => {
      update = callback;
      return () => {};
    });
    render(<MiniBar />);
    expect(await screen.findByText("75% remaining")).toBeVisible();
    act(() => update?.([provider(42)]));
    expect(screen.getByText("42% remaining")).toBeVisible();
  });

  it("shows a redacted unavailable state when cached bootstrap fails", async () => {
    vi.mocked(desktop.miniBootstrap).mockRejectedValue(new Error("database path"));
    render(<MiniBar />);
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Quota unavailable"));
    expect(screen.queryByText("database path")).not.toBeInTheDocument();
  });
});
