import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// Set to true while a scan or quarantine action runs (see App.jsx).
export const appState = { busy: false };

export async function checkForUpdates() {
  try {
    const update = await check();
    if (!update) return;

    // Download now, install on user confirmation, never mid-operation.
    await update.download();
    if (appState.busy) return; // try again next launch

    const ok = window.confirm(
      `Diskern ${update.version} is available.\n\n${update.body ?? ""}\n\nRestart to update?`
    );
    if (ok) {
      await update.install();
      await relaunch();
    }
  } catch (e) {
    // Updates are best-effort; never surface errors to the user on startup.
    console.warn("update check failed", e);
  }
}
