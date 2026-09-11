import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const read = (path: string) => readFileSync(resolve(process.cwd(), path), "utf8");
const commands = read("src-tauri/src/commands.rs");
const entrypoint = read("src/main.tsx");
const styles = read("src/styles.css");
const config = JSON.parse(read("src-tauri/tauri.conf.json"));

describe("mini window native contract", () => {
  it("emits the completed main bootstrap refresh to already-open mini windows", () => {
    const bootstrap = commands.slice(
      commands.indexOf("pub async fn get_bootstrap"),
      commands.indexOf("pub async fn get_mini_bootstrap"),
    );
    const sharedRefresh = commands.slice(
      commands.indexOf("pub async fn refresh_all_from_app"),
      commands.indexOf("fn emit_refresh"),
    );

    expect(bootstrap).toContain("refresh_all_from_app(&app, true).await");
    expect(sharedRefresh).toContain("emit_refresh(app, &response)");
  });

  it("configures the native window and padded webview perimeter as transparent", () => {
    const miniWindow = config.app.windows.find(
      (window: { label: string }) => window.label === "mini",
    );
    expect(miniWindow).toMatchObject({
      width: 480,
      height: 96,
      transparent: true,
      backgroundColor: "#00000000",
    });
    expect(entrypoint).toContain(
      "document.documentElement.dataset.window = windowLabel",
    );
    expect(styles).toMatch(
      /html\[data-window="mini"\][^{]*\{[^}]*background: transparent;/s,
    );
    expect(styles).toMatch(
      /body\[data-window="mini"\][^{]*\{[^}]*background: transparent;/s,
    );
    expect(styles).toMatch(
      /body\[data-window="mini"\] #root[^{]*\{[^}]*background: transparent;/s,
    );
    expect(styles).toMatch(
      /\.mini-bar[^{]*\{[^}]*min-height:\s*0;[^}]*padding-top:\s*0;/s,
    );
  });
});
