import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { LocalApiSettings } from "./LocalApiSettings";
import { desktop, type LocalApiStatus } from "../lib/desktop";

vi.mock("../lib/desktop", () => ({ desktop: { localApiStatus: vi.fn(), configureLocalApi: vi.fn() } }));
const disabled: LocalApiStatus = { enabled: false, listening: false, tokenSource: "none", error: null };
const enabled: LocalApiStatus = { enabled: true, listening: true, tokenSource: "credentialManager", error: null };

beforeEach(() => {
  vi.mocked(desktop.localApiStatus).mockResolvedValue(disabled);
});

it("shows opt-in off and enables without passing any secret or generic settings", async () => {
  const user = userEvent.setup();
  vi.mocked(desktop.configureLocalApi).mockResolvedValue(enabled);
  render(<LocalApiSettings native />);
  expect(await screen.findByText("Disabled · Not listening")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Rotate API token" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Enable Local API" }));
  expect(desktop.configureLocalApi).toHaveBeenCalledWith("enable");
  expect(await screen.findByText("Listening on 127.0.0.1:9876")).toBeInTheDocument();
  expect(screen.getByText(/Token source: Windows Credential Manager/)).toBeInTheDocument();
  expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
});

it("disables under an override and makes rotation unavailable", async () => {
  const user = userEvent.setup();
  vi.mocked(desktop.localApiStatus).mockResolvedValue({ ...enabled, tokenSource: "environment" });
  vi.mocked(desktop.configureLocalApi).mockResolvedValue({ ...disabled, tokenSource: "environment" });
  render(<LocalApiSettings native />);
  await screen.findByText(/Token source: ELLIE_API_TOKEN override/);
  expect(screen.getByRole("button", { name: "Rotate API token" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Disable Local API" }));
  expect(desktop.configureLocalApi).toHaveBeenCalledWith("disable");
  expect(await screen.findByText("Disabled · Not listening")).toBeInTheDocument();
});

it("serializes UI controls while rotation is pending and preserves valid status on failed rotation", async () => {
  const user = userEvent.setup();
  let complete!: (status: LocalApiStatus) => void;
  vi.mocked(desktop.localApiStatus).mockResolvedValue(enabled);
  vi.mocked(desktop.configureLocalApi).mockReturnValue(new Promise((resolve) => { complete = resolve; }));
  render(<LocalApiSettings native />);
  await screen.findByText("Listening on 127.0.0.1:9876");
  await user.click(screen.getByRole("button", { name: "Rotate API token" }));
  for (const button of screen.getAllByRole("button")) expect(button).toBeDisabled();
  await act(async () => complete({ ...enabled, error: "credentialStore" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("previous token");
  expect(screen.getByText("Listening on 127.0.0.1:9876")).toBeInTheDocument();
  expect(desktop.configureLocalApi).toHaveBeenCalledWith("rotate");
});

it("does not claim listening from preference alone and can refresh status", async () => {
  const user = userEvent.setup();
  vi.mocked(desktop.localApiStatus).mockResolvedValueOnce({ ...enabled, listening: false, error: "bind" }).mockResolvedValue(enabled);
  render(<LocalApiSettings native />);
  expect(await screen.findByText("Enabled preference · Not listening")).toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent("Close the conflicting listener");
  await user.click(screen.getByRole("button", { name: "Refresh API status" }));
  expect(await screen.findByText("Listening on 127.0.0.1:9876")).toBeInTheDocument();
});

it("redacts IPC failures and stops showing stale operational status", async () => {
  const user = userEvent.setup();
  vi.mocked(desktop.localApiStatus).mockResolvedValue(enabled);
  vi.mocked(desktop.configureLocalApi).mockRejectedValue(new Error("test-only-sensitive-error"));
  render(<LocalApiSettings native />);
  await screen.findByText("Listening on 127.0.0.1:9876");
  await user.click(screen.getByRole("button", { name: "Disable Local API" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("status is unknown");
  expect(screen.queryByText(/test-only-sensitive-error/)).not.toBeInTheDocument();
  expect(screen.queryByText("Listening on 127.0.0.1:9876")).not.toBeInTheDocument();
});

it("keeps browser preview inert and provides retry for a failed status read", async () => {
  const { unmount } = render(<LocalApiSettings native={false} />);
  expect(desktop.localApiStatus).not.toHaveBeenCalled();
  for (const button of screen.getAllByRole("button")) expect(button).toBeDisabled();
  unmount();
  vi.mocked(desktop.localApiStatus).mockRejectedValueOnce(new Error("test-only-sensitive-error"));
  render(<LocalApiSettings native />);
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Retry status"));
  expect(screen.queryByText(/test-only-sensitive-error/)).not.toBeInTheDocument();
});

function deferredStatus() {
  let resolve!: (value: LocalApiStatus) => void;
  let reject!: (reason: Error) => void;
  const promise = new Promise<LocalApiStatus>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

it.each(["success", "error"] as const)("ignores stale mount %s after a newer refresh and enable", async (outcome) => {
  const user = userEvent.setup();
  const mount = deferredStatus();
  vi.mocked(desktop.localApiStatus).mockReturnValueOnce(mount.promise).mockResolvedValue(disabled);
  vi.mocked(desktop.configureLocalApi).mockResolvedValue(enabled);
  render(<LocalApiSettings native />);
  await user.click(screen.getByRole("button", { name: "Refresh API status" }));
  await screen.findByText("Disabled · Not listening");
  await user.click(screen.getByRole("button", { name: "Enable Local API" }));
  await screen.findByText("Listening on 127.0.0.1:9876");
  await act(async () => {
    if (outcome === "success") mount.resolve(disabled);
    else mount.reject(new Error("test-only-stale-error"));
  });
  expect(screen.getByText("Listening on 127.0.0.1:9876")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Disable Local API" })).toBeEnabled();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

it("a stale mount result cannot release busy controls belonging to a newer mutation", async () => {
  const user = userEvent.setup();
  const mount = deferredStatus();
  const configure = deferredStatus();
  vi.mocked(desktop.localApiStatus).mockReturnValueOnce(mount.promise).mockResolvedValue(disabled);
  vi.mocked(desktop.configureLocalApi).mockReturnValue(configure.promise);
  render(<LocalApiSettings native />);
  await user.click(screen.getByRole("button", { name: "Refresh API status" }));
  await screen.findByText("Disabled · Not listening");
  await user.click(screen.getByRole("button", { name: "Enable Local API" }));
  await act(async () => mount.resolve(disabled));
  for (const button of screen.getAllByRole("button")) expect(button).toBeDisabled();
  await act(async () => configure.resolve(enabled));
  expect(screen.getByText("Listening on 127.0.0.1:9876")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Disable Local API" })).toBeEnabled();
});

it.each(["success", "error"] as const)("cleanup supersedes a pending configure %s with the next mount read", async (outcome) => {
  const user = userEvent.setup();
  const configure = deferredStatus();
  vi.mocked(desktop.configureLocalApi).mockReturnValue(configure.promise);
  const { rerender } = render(<LocalApiSettings native />);
  await screen.findByText("Disabled · Not listening");
  await user.click(screen.getByRole("button", { name: "Enable Local API" }));
  rerender(<LocalApiSettings native={false} />);
  rerender(<LocalApiSettings native />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Enable Local API" })).toBeEnabled());
  await act(async () => {
    if (outcome === "success") configure.resolve(enabled);
    else configure.reject(new Error("test-only-stale-error"));
  });
  expect(screen.getByText("Disabled · Not listening")).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Enable Local API" })).toBeEnabled();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

it("explains non-owner controls while still allowing a preference status refresh", async () => {
  vi.mocked(desktop.localApiStatus).mockResolvedValue({ ...enabled, listening: false, error: "controlsUnavailable" });
  render(<LocalApiSettings native />);
  expect(await screen.findByRole("alert")).toHaveTextContent("Use the owning Ellie instance");
  for (const name of ["Enable Local API", "Disable Local API", "Rotate API token"]) {
    expect(screen.getByRole("button", { name })).toBeDisabled();
  }
  expect(screen.getByRole("button", { name: "Refresh API status" })).toBeEnabled();
});
