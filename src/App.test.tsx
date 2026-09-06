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
