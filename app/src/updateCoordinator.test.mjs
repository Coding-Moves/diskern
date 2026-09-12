import assert from "node:assert/strict";
import { test } from "node:test";
import {
  createAppState,
  createUpdateChecker,
  runAppOperation,
} from "./updateCoordinator.js";

function createPendingOperation(state) {
  const finish = state.beginOperation();
  assert.equal(typeof finish, "function");
  return finish;
}

test("update install is deferred while an operation is busy", async () => {
  const state = createAppState();
  const release = createPendingOperation(state);
  let downloaded = false;
  let installed = false;
  let relaunched = false;

  const checkForUpdates = createUpdateChecker({
    state,
    confirm: () => {
      throw new Error("confirm should not run while busy");
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

  assert.equal(await checkForUpdates(), "deferred");
  assert.equal(downloaded, true);
  assert.equal(installed, false);
  assert.equal(relaunched, false);

  release();
  assert.equal(state.busy, false);
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
