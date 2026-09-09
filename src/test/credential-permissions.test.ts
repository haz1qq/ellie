import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const read = (path: string) => readFileSync(resolve(process.cwd(), path), "utf8");
const manifest = read("src-tauri/build.rs");
const handler = read("src-tauri/src/lib.rs");
const desktop = read("src/lib/desktop.ts");
const capability = JSON.parse(read("src-tauri/capabilities/main.json"));
const config = JSON.parse(read("src-tauri/tauri.conf.json"));

// Mocked invoke calls cannot catch missing native permissions. Keep all
// parts of each native IPC route aligned, without accessing any secrets.
describe("credential IPC permissions", () => {
  it.each([
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

  it("keeps access scoped to the local main window", () => {
    expect(config.app.security.capabilities).toEqual(["main"]);
    expect(capability.identifier).toBe("main");
    expect(capability.windows).toEqual(["main"]);
    expect(capability.remote).toBeUndefined();
    expect(capability.local).not.toBe(false);
    expect(capability.permissions).toEqual([
      "core:event:allow-listen",
      "core:event:allow-unlisten",
      "allow-get-bootstrap",
      "allow-get-analytics",
      "allow-save-settings",
      "allow-hide-to-tray",
      "allow-refresh-all",
      "allow-refresh-provider",
      "allow-save-provider-key",
      "allow-delete-provider-key",
      "allow-provider-key-status",
    ]);
  });
});
