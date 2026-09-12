import assert from "node:assert/strict";
import { test } from "node:test";
import {
  createAppState,
  createUpdateChecker,
  getUpdateStatus,
  runAppOperation,
  setUpdateStatus,
  subscribeUpdateStatus,
} from "./updateCoordinator.js";

function createPendingOperation(state) {
  const finish = state.beginOperation();
  assert.equal(typeof finish, "function");
  return finish;
}

// Let the checker run until it publishes `phase` (or gives up waiting).
// check() → download() → whenIdle() all resolve on the microtask queue, so
// a handful of macrotask ticks is plenty.
async function waitForPhase(statuses, phase) {
  for (let i = 0; i < 20 && statuses.at(-1)?.phase !== phase; i++) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
}

function phases(statuses) {
  return statuses.map((s) => s?.phase ?? null);
}

test("a downloaded update waits for a running operation, then prompts", async () => {
  const state = createAppState();
  const release = createPendingOperation(state);
  const statuses = [];
  let downloaded = false;
  let installed = false;
  let relaunched = false;
  let confirmCalls = 0;

  const checkForUpdates = createUpdateChecker({
    state,
    onStatus: (s) => statuses.push(s),
    confirm: () => {
      confirmCalls += 1;
      assert.equal(state.busy, false, "confirm must not run while busy");
      return true;
    },
    warn: () => {},
    relaunch: async () => {
      relaunched = true;
    },
    check: async () => ({
      version: "0.4.0",
      body: "test",
      download: async () => {
        downloaded = true;
      },
      install: async () => {
        installed = true;
      },
    }),
  });

  const result = checkForUpdates();
  await waitForPhase(statuses, "deferred");

  // Downloaded but parked: nothing prompts or installs while work runs.
  assert.equal(downloaded, true);
  assert.equal(installed, false);
  assert.equal(relaunched, false);
  assert.equal(confirmCalls, 0);
  assert.deepEqual(phases(statuses), ["checking", "downloading", "deferred"]);

  release();
  assert.equal(await result, "installed");
  assert.equal(installed, true);
  assert.equal(relaunched, true);
  assert.equal(confirmCalls, 1);
  assert.deepEqual(phases(statuses), [
    "checking",
    "downloading",
    "deferred",
    "ready",
    "installing",
  ]);
});

test("overlapping operations keep the app busy until both finish", () => {
  const state = createAppState();
  const first = createPendingOperation(state);
  const second = createPendingOperation(state);

  assert.equal(state.busy, true);
  first();
  assert.equal(state.busy, true);
  second();
  assert.equal(state.busy, false);
});

test("rejected operations release their busy state", async () => {
  const state = createAppState();

  await assert.rejects(
    runAppOperation(
      async () => {
        throw new Error("backend refused");
      },
      { state }
    ),
    /backend refused/
  );

  assert.equal(state.busy, false);
});

test("starting update installation prevents new operations", async () => {
  const state = createAppState();
  let installed = false;

  const checkForUpdates = createUpdateChecker({
    state,
    confirm: () => true,
    warn: () => {},
    relaunch: async () => {},
    check: async () => ({
      version: "0.4.0",
      download: async () => {},
      install: async () => {
        installed = true;
      },
    }),
  });

  assert.equal(await checkForUpdates(), "installed");
  assert.equal(installed, true);
  assert.equal(state.installingUpdate, true);

  await assert.rejects(
    runAppOperation(async () => {}, { state }),
    /update is installing/i
  );
});

test("failed update installation allows operations again", async () => {
  const state = createAppState();
  const warnings = [];

  const checkForUpdates = createUpdateChecker({
    state,
    confirm: () => true,
    warn: (...args) => warnings.push(args),
    relaunch: async () => {
      throw new Error("relaunch failed");
    },
    check: async () => ({
      version: "0.4.0",
      download: async () => {},
      install: async () => {},
    }),
  });

  assert.equal(await checkForUpdates(), "error");
  assert.equal(state.installingUpdate, false);
  assert.equal(warnings.length, 1);

  await runAppOperation(async () => "ok", { state });
  assert.equal(state.busy, false);
});

test("an operation started during confirmation defers the install, not the update", async () => {
  const state = createAppState();
  const statuses = [];
  let releaseLateOperation = null;
  let installed = false;

  const checkForUpdates = createUpdateChecker({
    state,
    onStatus: (s) => statuses.push(s),
    confirm: () => {
      // The user says yes, but a scan slipped in while the dialog was open.
      releaseLateOperation = state.beginOperation();
      return true;
    },
    warn: () => {},
    relaunch: async () => {},
    check: async () => ({
      version: "0.4.0",
      download: async () => {},
      install: async () => {
        installed = true;
      },
    }),
  });

  const result = checkForUpdates();
  await waitForPhase(statuses, "deferred");
  assert.equal(installed, false);
  assert.equal(statuses.at(-1).version, "0.4.0");

  releaseLateOperation();
  assert.equal(await result, "installed");
  assert.equal(installed, true);
});

test("each updater phase publishes a status in order", async () => {
  const state = createAppState();
  const statuses = [];

  const checkForUpdates = createUpdateChecker({
    state,
    onStatus: (s) => statuses.push(s),
    confirm: () => true,
    warn: () => {},
    relaunch: async () => {},
    check: async () => ({
      version: "0.4.0",
      download: async () => {},
      install: async () => {},
    }),
  });

  assert.equal(await checkForUpdates(), "installed");
  assert.deepEqual(phases(statuses), [
    "checking",
    "downloading",
    "ready",
    "installing",
  ]);
  assert.equal(statuses.at(-1).version, "0.4.0");
});

test("a check that finds no update clears the status", async () => {
  const statuses = [];
  const checkForUpdates = createUpdateChecker({
    state: createAppState(),
    onStatus: (s) => statuses.push(s),
    warn: () => {},
    relaunch: async () => {},
    check: async () => null,
  });

  assert.equal(await checkForUpdates(), "none");
  assert.deepEqual(phases(statuses), ["checking", null]);
});

test("declining an update clears the status", async () => {
  const statuses = [];
  const checkForUpdates = createUpdateChecker({
    state: createAppState(),
    onStatus: (s) => statuses.push(s),
    confirm: () => false,
    warn: () => {},
    relaunch: async () => {},
    check: async () => ({
      version: "0.4.0",
      download: async () => {},
      install: async () => {},
    }),
  });

  assert.equal(await checkForUpdates(), "declined");
  assert.equal(statuses.at(-1), null);
});

test("a failed download reports the failed phase", async () => {
  const statuses = [];
  const checkForUpdates = createUpdateChecker({
    state: createAppState(),
    onStatus: (s) => statuses.push(s),
    confirm: () => true,
    warn: () => {},
    relaunch: async () => {},
    check: async () => ({
      version: "0.4.0",
      download: async () => {
        throw new Error("network gone");
      },
      install: async () => {},
    }),
  });

  assert.equal(await checkForUpdates(), "error");
  // The user sees a calm failure note, not a frozen "Downloading…".
  assert.deepEqual(statuses.at(-1), { phase: "failed", version: "0.4.0" });
});

test("a failed check stays quiet and clears the status", async () => {
  const statuses = [];
  const warnings = [];
  const checkForUpdates = createUpdateChecker({
    state: createAppState(),
    onStatus: (s) => statuses.push(s),
    warn: (...args) => warnings.push(args),
    relaunch: async () => {},
    check: async () => {
      throw new Error("offline");
    },
  });

  // Being offline is not an update failure worth a banner on every launch.
  assert.equal(await checkForUpdates(), "error");
  assert.deepEqual(phases(statuses), ["checking", null]);
  assert.equal(warnings.length, 1);
});

test("whenIdle resolves immediately without work and after a release", async () => {
  const state = createAppState();
  await state.whenIdle(); // nothing running: no wait at all

  const release = createPendingOperation(state);
  let idleObserved = false;
  const waiting = state.whenIdle().then(() => {
    idleObserved = true;
  });

  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.equal(idleObserved, false, "must keep waiting while an op runs");

  release();
  await waiting;
  assert.equal(idleObserved, true);
});

test("the status store notifies subscribers and forgets them", () => {
  const seen = [];
  const unsubscribe = subscribeUpdateStatus((s) => seen.push(s));

  setUpdateStatus({ phase: "downloading", version: "0.4.0" });
  assert.deepEqual(getUpdateStatus(), { phase: "downloading", version: "0.4.0" });

  unsubscribe();
  setUpdateStatus(null);
  assert.equal(getUpdateStatus(), null);
  assert.deepEqual(seen, [{ phase: "downloading", version: "0.4.0" }]);
});
