export function createAppState() {
  let activeOperations = 0;
  let installingUpdate = false;

  return {
    get busy() {
      return activeOperations > 0;
    },

    get installingUpdate() {
      return installingUpdate;
    },

    beginOperation() {
      if (installingUpdate) return null;

      activeOperations += 1;
      let released = false;
      return () => {
        if (released) return;
        released = true;
        activeOperations = Math.max(0, activeOperations - 1);
      };
    },

    beginUpdateInstall() {
      if (installingUpdate || activeOperations > 0) return false;
      installingUpdate = true;
      return true;
    },

    resetForTests() {
      activeOperations = 0;
      installingUpdate = false;
    },
  };
}

export const appState = createAppState();

export async function runAppOperation(work, { state = appState } = {}) {
  const finish = state.beginOperation();
  if (!finish) {
    throw new Error("An update is installing; try again after Diskern restarts.");
  }

  try {
    return await work();
  } finally {
    finish();
  }
}

export function createUpdateChecker({
  check,
  relaunch,
  state = appState,
  confirm = (message) => window.confirm(message),
  warn = console.warn,
}) {
  return async function checkForUpdates() {
    try {
      const update = await check();
      if (!update) return "none";

      // Download now, install on user confirmation, never mid-operation.
      await update.download();
      if (state.busy) return "deferred";

      const ok = confirm(
        `Diskern ${update.version} is available.\n\n${update.body ?? ""}\n\nRestart to update?`
      );
      if (!ok) return "declined";
      if (!state.beginUpdateInstall()) return "deferred";

      await update.install();
      await relaunch();
      return "installed";
    } catch (e) {
      // Updates are best-effort; never surface errors to the user on startup.
      warn("update check failed", e);
      return "error";
    }
  };
}
