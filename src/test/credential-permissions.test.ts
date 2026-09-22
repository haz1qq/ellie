import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const read = (path: string) => readFileSync(resolve(process.cwd(), path), "utf8");
const manifest = read("src-tauri/build.rs");
const handler = read("src-tauri/src/lib.rs");
const desktop = read("src/lib/desktop.ts");
const capability = JSON.parse(read("src-tauri/capabilities/main.json"));
const miniCapability = JSON.parse(read("src-tauri/capabilities/mini.json"));
const taskNoteCapability = JSON.parse(read("src-tauri/capabilities/task-note.json"));
const config = JSON.parse(read("src-tauri/tauri.conf.json"));
const githubCommands = [
  "github_connection_status",
  "github_save_client_id",
  "github_sign_in",
  "github_cancel_sign_in",
  "github_disconnect",
  "github_list_repositories",
  "github_list_commits",
  "github_contribution_calendar",
  "github_prepare_repository_creation",
  "github_confirm_repository_creation",
  "github_repository_creation_status",
  "github_resolve_repository_creation",
];
const taskNoteCommands = [
  "task_note_bootstrap",
  "task_note_complete",
  "task_note_unpin",
];

// Mocked invoke calls cannot catch missing native permissions. Keep all
// parts of each native IPC route aligned, without accessing any secrets.
describe("credential IPC permissions", () => {
  it.each([
    "local_api_status",
    "configure_local_api",
    "save_provider_key",
    "delete_provider_key",
    "provider_key_status",
    "refresh_all",
    "refresh_provider",
  ])("allows %s through the native main-window boundary", (command) => {
    expect(desktop).toContain(`"${command}"`);
    expect(handler).toContain(`commands::${command}`);
    const commands = manifest.match(/\.commands\(&\[([\s\S]*?)\]\)/)?.[1];
    expect(commands).toContain(`"${command}"`);
    expect(capability.permissions).toContain(
      `allow-${command.replaceAll("_", "-")}`,
    );
  });

  it.each(githubCommands)(
    "allows backend-only %s through the native main-window boundary",
    (command) => {
      expect(handler).toContain(`commands::${command}`);
      const commands = manifest.match(/\.commands\(&\[([\s\S]*?)\]\)/)?.[1];
      expect(commands).toContain(`"${command}"`);
      expect(capability.permissions).toContain(
        `allow-${command.replaceAll("_", "-")}`,
      );
      expect(miniCapability.permissions).not.toContain(
        `allow-${command.replaceAll("_", "-")}`,
      );
    },
  );

  it("keeps access scoped to the local main window", () => {
    expect(config.app.security.capabilities).toEqual(["main", "mini", "task-note"]);
    expect(capability.identifier).toBe("main");
    expect(capability.windows).toEqual(["main"]);
    expect(capability.remote).toBeUndefined();
    expect(capability.local).not.toBe(false);
    expect(capability.permissions).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
      {
        identifier: "opener:allow-open-url",
        allow: [{ url: "https://github.com/login/oauth/authorize?*" }],
      },
      "allow-local-api-status",
      "allow-configure-local-api",
      "allow-get-bootstrap",
      "allow-get-analytics",
      "allow-save-settings",
      "allow-hide-to-tray",
      "allow-refresh-all",
      "allow-refresh-provider",
      "allow-save-provider-key",
      "allow-delete-provider-key",
      "allow-provider-key-status",
      "allow-github-connection-status",
      "allow-github-save-client-id",
      "allow-github-save-client-secret",
      "allow-github-sign-in",
      "allow-github-cancel-sign-in",
      "allow-github-disconnect",
      "allow-github-list-repositories",
      "allow-github-list-commits",
      "allow-github-contribution-calendar",
      "allow-github-prepare-repository-creation",
      "allow-github-confirm-repository-creation",
      "allow-github-repository-creation-status",
      "allow-github-resolve-repository-creation",
      "allow-task-bootstrap",
      "allow-task-list",
      "allow-task-create-list",
      "allow-task-rename-list",
      "allow-task-list-delete-preview",
      "allow-task-delete-list",
      "allow-task-create",
      "allow-task-update",
      "allow-task-set-completed",
      "allow-task-delete",
      "allow-task-set-pinned",
    ]);
  });

  it.each(["get_mini_bootstrap", "open_main_window"])(
    "aligns the mini-only %s command across IPC declarations",
    (command) => {
      expect(desktop).toContain(`"${command}"`);
      expect(handler).toContain(`commands::${command}`);
      const commands = manifest.match(/\.commands\(&\[([\s\S]*?)\]\)/)?.[1];
      expect(commands).toContain(`"${command}"`);
      expect(miniCapability.permissions).toContain(
        `allow-${command.replaceAll("_", "-")}`,
      );
      expect(capability.permissions).not.toContain(
        `allow-${command.replaceAll("_", "-")}`,
      );
    },
  );

  it.each(taskNoteCommands)(
    "aligns sticky-note-only %s across IPC declarations",
    (command) => {
      expect(desktop).toContain(`"${command}"`);
      expect(handler).toContain(`commands::${command}`);
      const commands = manifest.match(/\.commands\(&\[([\s\S]*?)\]\)/)?.[1];
      expect(commands).toContain(`"${command}"`);
      const permission = `allow-${command.replaceAll("_", "-")}`;
      expect(taskNoteCapability.permissions).toContain(permission);
      expect(capability.permissions).not.toContain(permission);
      expect(miniCapability.permissions).not.toContain(permission);
    },
  );

  it("keeps the sticky note on a least-privilege command boundary", () => {
    expect(taskNoteCapability.identifier).toBe("task-note");
    expect(taskNoteCapability.windows).toEqual(["task-note"]);
    expect(taskNoteCapability.remote).toBeUndefined();
    expect(taskNoteCapability.permissions).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
      "core:window:allow-start-dragging",
      "allow-open-main-window",
      "allow-task-note-bootstrap",
      "allow-task-note-complete",
      "allow-task-note-unpin",
    ]);
    for (const forbidden of [
      "allow-get-bootstrap",
      "allow-task-bootstrap",
      "allow-task-create",
      "allow-task-update",
      "allow-task-delete",
      "allow-task-set-pinned",
      "allow-github-connection-status",
      "allow-save-provider-key",
    ]) {
      expect(taskNoteCapability.permissions).not.toContain(forbidden);
    }
  });

  it("keeps the mini bar on a least-privilege command boundary", () => {
    expect(miniCapability.identifier).toBe("mini");
    expect(miniCapability.windows).toEqual(["mini"]);
    expect(miniCapability.remote).toBeUndefined();
    expect(miniCapability.permissions).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
      "core:window:allow-start-dragging",
      "allow-get-mini-bootstrap",
      "allow-open-main-window",
      "allow-open-main-section",
    ]);
    for (const forbidden of [
      "allow-local-api-status",
      "allow-configure-local-api",
      "allow-get-bootstrap",
      "allow-save-settings",
      "allow-refresh-all",
      "allow-refresh-provider",
      "allow-save-provider-key",
      "allow-delete-provider-key",
      "allow-provider-key-status",
      ...githubCommands.map(
        (command) => `allow-${command.replaceAll("_", "-")}`,
      ),
    ]) {
      expect(miniCapability.permissions).not.toContain(forbidden);
    }
  });
});
