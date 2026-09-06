import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type View = "dashboard" | "settings";
export interface Settings {
  closeToTray: boolean;
  showMascot: boolean;
  friendlyMessages: boolean;
}
export interface Bootstrap {
  settings: Settings;
  view: View;
}

export const desktop = {
  available: isTauri,
  bootstrap: () => invoke<Bootstrap>("get_bootstrap"),
  saveSettings: (settings: Settings) =>
    invoke<Settings>("save_settings", { settings }),
  hide: () => invoke<void>("hide_to_tray"),
  onNavigate: (callback: (view: View) => void) =>
    listen<View>("navigate", (event) => callback(event.payload)),
};
