export function createAppState() {
  let activeOperations = 0;
  let installingUpdate = false;
  const idleWaiters = new Set();

  // Wake every waiter once nothing is running. Called on each release —
  // most calls find no waiters and no drain, and do nothing.
  function settleIdleWaiters() {
    if (activeOperations > 0 || installingUpdate) return;
    for (const resolve of idleWaiters) resolve();
    idleWaiters.clear();
  }

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
        settleIdleWaiters();
      };
    },

    beginUpdateInstall() {
      if (installingUpdate || activeOperations > 0) return null;
      installingUpdate = true;
      return () => {
        installingUpdate = false;
        settleIdleWaiters();
      };
    },

    // Resolves once no operation or install is running — the updater's way
    // to wait out a busy spell instead of dropping a downloaded update.
    whenIdle() {
      if (activeOperations === 0 && !installingUpdate) {
        return Promise.resolve();
      }
      return new Promise((resolve) => idleWaiters.add(resolve));
    },

    resetForTests() {
      activeOperations = 0;
      installingUpdate = false;
      // Resolve rather than clear, so nothing awaiting whenIdle() can hang.
      settleIdleWaiters();
    },
  };
}

export const appState = createAppState();

// --- Update status surface ------------------------------------------------
// The checker runs outside React (main.jsx, a few seconds after launch), so
// the phase it publishes lives in a tiny observable store rather than
// component state. App subscribes with useSyncExternalStore; tests can read
// getUpdateStatus() or inject their own onStatus sink into the checker.
// `null` means there is nothing to show — before the first check, after a
// check finds no update, and after the user declines or dismisses one.
let updateStatus = null;
const updateStatusListeners = new Set();

export function getUpdateStatus() {
  return updateStatus;
}

export function subscribeUpdateStatus(listener) {
  updateStatusListeners.add(listener);
  return () => updateStatusListeners.delete(listener);
}

export function setUpdateStatus(status) {
  updateStatus = status;
  for (const listener of updateStatusListeners) listener(status);
}

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
  onStatus = setUpdateStatus,
  confirm = (message) => window.confirm(message),
  warn = console.warn,
}) {
  return async function checkForUpdates() {
    let update = null;
    try {
      onStatus({ phase: "checking" });
      update = await check();
      if (!update) {
        onStatus(null);
        return "none";
      }

      // Download now, install on user confirmation, never mid-operation.
      onStatus({ phase: "downloading", version: update.version });
      await update.download();

      // Work is running: say the update is waiting rather than dropping it
      // silently — the prompt lands as soon as the app goes quiet.
      while (state.busy) {
        onStatus({ phase: "deferred", version: update.version });
        await state.whenIdle();
      }

      onStatus({ phase: "ready", version: update.version });
      const ok = confirm(
        `Diskern ${update.version} is available.\n\n${update.body ?? ""}\n\nRestart to update?`
      );
      if (!ok) {
        onStatus(null);
        return "declined";
      }

      // A new operation may have started while the confirm dialog was open —
      // wait it out the same way.
      let releaseInstall = state.beginUpdateInstall();
      while (!releaseInstall) {
        onStatus({ phase: "deferred", version: update.version });
        await state.whenIdle();
        releaseInstall = state.beginUpdateInstall();
      }

      try {
        onStatus({ phase: "installing", version: update.version });
        await update.install();
        await relaunch();
      } catch (e) {
        releaseInstall();
        throw e;
      }
      return "installed";
    } catch (e) {
      // Updates are best-effort; never surface errors to the user on startup.
      // A failed *check* (offline, no release published yet) stays quiet and
      // the status just goes away — but once an update was found, a failed
      // download or install says so instead of looking like the app froze.
      warn("update check failed", e);
      onStatus(update ? { phase: "failed", version: update.version } : null);
      return "error";
    }
  };
}
