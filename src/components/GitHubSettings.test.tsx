import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { GitHubSettings } from "./GitHubSettings";
import { desktop, type GitHubConnectionStatus } from "../lib/desktop";

vi.mock("../lib/desktop", () => ({
  desktop: {
    githubConnectionStatus: vi.fn(),
    githubSaveClientId: vi.fn(),
    githubSaveClientSecret: vi.fn(),
    githubSignIn: vi.fn(),
    githubCancelSignIn: vi.fn(),
    githubDisconnect: vi.fn(),
    githubListRepositories: vi.fn(),
    githubListCommits: vi.fn(),
  },
}));

const disconnected: GitHubConnectionStatus = {
  state: "Disconnected",
  account: null,
  lastError: null,
  tokenPresent: false,
  clientIdConfigured: false,
  clientSecretConfigured: false,
};
const idConfigured: GitHubConnectionStatus = {
  ...disconnected,
  clientIdConfigured: true,
};
const configured: GitHubConnectionStatus = {
  ...idConfigured,
  clientSecretConfigured: true,
};
const authorizing: GitHubConnectionStatus = {
  ...configured,
  state: "Authorizing",
};
const connected: GitHubConnectionStatus = {
  state: "Connected",
  account: { id: 42, login: "octo-cat" },
  lastError: null,
  tokenPresent: true,
  clientIdConfigured: true,
  clientSecretConfigured: true,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((accept, fail) => {
    resolve = accept;
    reject = fail;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(disconnected);
  vi.mocked(desktop.githubSaveClientId).mockResolvedValue(idConfigured);
  vi.mocked(desktop.githubSaveClientSecret).mockResolvedValue(configured);
  vi.mocked(desktop.githubCancelSignIn).mockResolvedValue(disconnected);
  vi.mocked(desktop.githubDisconnect).mockResolvedValue(disconnected);
});

describe("GitHub Settings section", () => {
  it("saves the trimmed Client ID and reports success without echoing secrets", async () => {
    const user = userEvent.setup();
    render(<GitHubSettings native />);
    await screen.findByText("Not connected to GitHub.");
    await user.type(
      screen.getByLabelText("GitHub App Client ID"),
      "  Iv1.sanitized-client  ",
    );
    await user.click(screen.getByRole("button", { name: "Save Client ID" }));
    expect(desktop.githubSaveClientId).toHaveBeenCalledWith("Iv1.sanitized-client");
    expect(
      await screen.findByText("GitHub App Client ID saved on this device."),
    ).toBeVisible();
    expect(screen.getByText("GitHub App Client ID is set.")).toBeVisible();
  });

  it("saves the trimmed Client Secret, clears the input, and never displays it", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(idConfigured);
    render(<GitHubSettings native />);
    await screen.findByText("Not connected to GitHub.");
    const secretInput = screen.getByLabelText("GitHub App Client Secret");
    await user.type(secretInput, "  sanitized-test-client-secret  ");
    await user.click(screen.getByRole("button", { name: "Save Client Secret" }));
    expect(desktop.githubSaveClientSecret).toHaveBeenCalledWith(
      "sanitized-test-client-secret",
    );
    expect(
      await screen.findByText("GitHub App Client Secret saved securely."),
    ).toBeVisible();
    expect(secretInput).toHaveValue("");
    expect(screen.queryByText("sanitized-test-client-secret")).not.toBeInTheDocument();
    expect(
      screen.getByText("GitHub App Client Secret is stored securely."),
    ).toBeVisible();
  });

  it("explains the disconnect status and gates Connect on both App credentials", async () => {
    render(<GitHubSettings native />);
    await screen.findByText("Not connected to GitHub.");
    expect(
      screen.getByText(/No GitHub App Client ID saved yet — add yours above/),
    ).toBeVisible();
    expect(
      screen.getByText(/No GitHub App Client Secret saved yet/),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Connect GitHub" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save Client ID" })).toBeDisabled();
  });

  it("shows a pending sign-in with an enabled Cancel and completes through cancel", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(configured);
    const signIn = deferred<GitHubConnectionStatus>();
    vi.mocked(desktop.githubSignIn).mockReturnValue(signIn.promise);
    render(<GitHubSettings native />);
    await screen.findByText("Not connected to GitHub.");
    await user.click(screen.getByRole("button", { name: "Connect GitHub" }));
    expect(desktop.githubSignIn).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText(
        "Waiting for GitHub… Complete the sign-in in the browser tab that opened.",
      ),
    ).toBeVisible();
    expect(screen.getByRole("button", { name: "Connect GitHub" })).toBeDisabled();
    const cancel = screen.getByRole("button", { name: "Cancel sign-in" });
    expect(cancel).toBeEnabled();
    const cancelOutcome = deferred<GitHubConnectionStatus>();
    vi.mocked(desktop.githubCancelSignIn).mockReturnValue(cancelOutcome.promise);
    await user.click(cancel);
    expect(desktop.githubCancelSignIn).toHaveBeenCalledTimes(1);
    await act(async () => {
      cancelOutcome.resolve(configured);
      signIn.reject(new Error("superseded"));
    });
    expect(await screen.findByText("Not connected to GitHub.")).toBeVisible();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(screen.queryByText(/superseded/)).not.toBeInTheDocument();
  });

  it("transitions through the authorizing state and shows the cancel affordance", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(authorizing);
    render(<GitHubSettings native />);
    expect(await screen.findByText(/Waiting for GitHub…/)).toBeVisible();
    expect(screen.getByRole("button", { name: "Cancel sign-in" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Connect GitHub" })).toBeDisabled();
  });

  it("confirms before disconnecting and only calls the backend on confirmation", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(connected);
    render(<GitHubSettings native />);
    await screen.findByText("Connected as @octo-cat.");
    await user.click(screen.getByRole("button", { name: "Disconnect GitHub" }));
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAccessibleName("Disconnect GitHub?");
    expect(screen.getByText(/removes the connected account/)).toBeVisible();
    await user.click(screen.getByRole("button", { name: "Keep connected" }));
    expect(desktop.githubDisconnect).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Disconnect GitHub" }));
    const outcome = deferred<GitHubConnectionStatus>();
    vi.mocked(desktop.githubDisconnect).mockReturnValue(outcome.promise);
    await user.click(screen.getByRole("button", { name: "Disconnect" }));
    expect(desktop.githubDisconnect).toHaveBeenCalledTimes(1);
    await act(async () => outcome.resolve(disconnected));
    expect(await screen.findByText("Not connected to GitHub.")).toBeVisible();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("shows token presence and the privacy note without leaking values", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue({
      ...connected,
      tokenPresent: false,
    });
    render(<GitHubSettings native />);
    await screen.findByText(/Connected as @octo-cat/);
    expect(screen.getByText("No refresh token saved yet.")).toBeVisible();
    expect(screen.getByText(/Windows Credential Manager/)).toBeVisible();
    expect(screen.getByText(/never\s+displayed/)).toBeVisible();
    expect(screen.getByText(/revoke the app in your GitHub account settings/)).toBeVisible();
    expect(screen.getByText(/fetched on demand and are not stored by Ellie/)).toBeVisible();
  });

  it.each([
    ["token_expiration_required", /non-expiring user token without a refresh token/],
    ["token_response_invalid", /token metadata Ellie cannot safely use/],
    ["account_response_invalid", /account response was not usable/],
  ] as const)(
    "shows actionable redacted auth diagnostic for %s",
    async (category, expected) => {
      vi.mocked(desktop.githubConnectionStatus).mockResolvedValue({
        ...configured,
        lastError: category,
      });
      render(<GitHubSettings native />);
      expect(await screen.findByRole("alert")).toHaveTextContent(expected);
      expect(screen.queryByText(category)).not.toBeInTheDocument();
    },
  );

  it("maps command failures to friendly copy and never shows raw categories", async () => {
    const user = userEvent.setup();
    vi.mocked(desktop.githubConnectionStatus).mockResolvedValue(configured);
    vi.mocked(desktop.githubSaveClientId).mockRejectedValue({
      category: "credential_store",
    });
    render(<GitHubSettings native />);
    await screen.findByText("Not connected to GitHub.");
    await user.type(screen.getByLabelText("GitHub App Client ID"), "Iv1.test");
    await user.click(screen.getByRole("button", { name: "Save Client ID" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "couldn’t be saved securely",
    );
    expect(screen.queryByText("credential_store")).not.toBeInTheDocument();
  });

  it("redacts a failed status read into a generic message", async () => {
    vi.mocked(desktop.githubConnectionStatus).mockRejectedValue(
      new Error("inner secret detail"),
    );
    render(<GitHubSettings native />);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "GitHub couldn’t complete the request",
    );
    expect(screen.queryByText(/inner secret detail/)).not.toBeInTheDocument();
  });

  it("keeps the browser preview inert", () => {
    render(<GitHubSettings native={false} />);
    expect(desktop.githubConnectionStatus).not.toHaveBeenCalled();
    expect(
      screen.getByText("Open the desktop app to connect GitHub."),
    ).toBeVisible();
    for (const name of ["Connect GitHub", "Save Client ID", "Save Client Secret"]) {
      expect(screen.getByRole("button", { name })).toBeDisabled();
    }
  });

  it.each(["success", "error"] as const)(
    "ignores a stale mount %s after a newer mutation",
    async (outcome) => {
      const user = userEvent.setup();
      const mount = deferred<GitHubConnectionStatus>();
      vi.mocked(desktop.githubConnectionStatus)
        .mockReturnValueOnce(mount.promise)
        .mockResolvedValue(configured);
      vi.mocked(desktop.githubSaveClientId).mockResolvedValue({
        ...configured,
        clientIdConfigured: true,
      });
      render(<GitHubSettings native />);
      expect(
        screen.getByText("GitHub connection status unavailable."),
      ).toBeVisible();
      await user.type(screen.getByLabelText("GitHub App Client ID"), "Iv1.x");
      await user.click(screen.getByRole("button", { name: "Save Client ID" }));
      expect(
        await screen.findByText("GitHub App Client ID saved on this device."),
      ).toBeVisible();
      await act(async () => {
        if (outcome === "success") mount.resolve(disconnected);
        else mount.reject(new Error("test-only-stale-error"));
      });
      expect(screen.getByText("GitHub App Client ID is set.")).toBeVisible();
      expect(screen.getByText("Not connected to GitHub.")).toBeVisible();
      expect(screen.queryByRole("alert")).not.toBeInTheDocument();
      expect(screen.queryByText(/test-only-stale-error/)).not.toBeInTheDocument();
    },
  );
});