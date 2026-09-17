import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const read = (path: string) => readFileSync(resolve(process.cwd(), path), "utf8");
const manifest = read("src-tauri/build.rs");
const handler = read("src-tauri/src/lib.rs");
const desktop = read("src/lib/desktop.ts");
const capability = JSON.parse(read("src-tauri/capabilities/main.json"));
const miniCapability = JSON.parse(read("src-tauri/capabilities/mini.json"));
const config = JSON.parse(read("src-tauri/tauri.conf.json"));
const githubCommands = [
  "github_connection_status",
  "github_save_client_id",
  "github_sign_in",
  "github_cancel_sign_in",
  "github_connect_start",
  "github_connect_complete",
  "github_disconnect",
  "github_list_repositories",
  "github_list_commits",
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
    expect(config.app.security.capabilities).toEqual(["main", "mini"]);
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
      "allow-github-connect-start",
      "allow-github-connect-complete",
      "allow-github-disconnect",
      "allow-github-list-repositories",
      "allow-github-list-commits",
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
