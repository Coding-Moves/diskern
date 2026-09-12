import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { appState, createUpdateChecker } from "./updateCoordinator.js";

export { appState };

export const checkForUpdates = createUpdateChecker({ check, relaunch });
