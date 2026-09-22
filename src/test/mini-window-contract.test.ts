import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const read = (path: string) => readFileSync(resolve(process.cwd(), path), "utf8");
const commands = read("src-tauri/src/commands.rs");
const miniBar = read("src-tauri/src/mini_bar.rs");
const storage = read("src-tauri/src/storage.rs");
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
      // Base 96px quota band plus 6px transparent padding and 1px borders.
      height: 110,
      minWidth: 320,
      minHeight: 96,
      resizable: true,
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

  it("exposes the expanded HUD navigation and section sizing contract", () => {
    // Bounded main-window navigation from the mini window.
    expect(commands).toContain("pub fn open_main_section");
    expect(commands).toContain("\"dashboard\",");
    expect(commands).toContain("\"history\",");
    expect(commands).toContain("\"settings\",");
    const mini = read("src-tauri/capabilities/mini.json");
    expect(mini).toContain("allow-open-main-section");
    // The quota band absorbs extra height when the user resizes the bar; the
    // section bands keep their height so text never clips.
    expect(styles).toMatch(/\.mini-band-quota[^{]*\{[^}]*min-height:\s*96px;/s);
    expect(styles).toMatch(/\.mini-band-task[^{]*\{[^}]*flex:\s*0 0 58px;/s);
    expect(styles).toMatch(/\.mini-band-github[^{]*\{[^}]*flex:\s*0 0 76px;/s);
    // Rust derives the logical pixel size from the enabled bands, the webview
    // padding and borders, and any user-chosen size, then persists resizes.
    expect(miniBar).toContain("CHROME_HEIGHT");
    expect(miniBar).toContain("fn desired_size");
    expect(miniBar).toContain("LogicalSize");
    expect(miniBar).toContain("set_min_size");
    expect(storage).toContain("pub fn save_mini_bar_size");
  });
});
