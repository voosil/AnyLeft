import { Panel } from "./screens/Panel";
import { Settings } from "./screens/Settings";

/**
 * Both windows load the same bundle. The settings window is opened with
 * `?window=settings` (see `tauri.conf.json`); everything else is the panel.
 * In a browser, append `?window=settings` to preview the settings screen.
 */
function currentScreen(): "panel" | "settings" {
  const param = new URLSearchParams(window.location.search).get("window");
  return param === "settings" ? "settings" : "panel";
}

export function App() {
  return currentScreen() === "settings" ? <Settings /> : <Panel />;
}
