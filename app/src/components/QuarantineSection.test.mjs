import assert from "node:assert/strict";
import { after, afterEach, before, beforeEach, test } from "node:test";
import { fileURLToPath } from "node:url";
import React, { act } from "react";
import { createRoot } from "react-dom/client";
import { JSDOM } from "jsdom";
import { createServer } from "vite";

let server;
let QuarantineSection;
let dom;
let root;
let records;
let calls;
let restored;

before(async () => {
  server = await createServer({
    root: fileURLToPath(new URL("../../", import.meta.url)),
    configFile: false,
    server: { middlewareMode: true, hmr: false, ws: false, watch: null },
    optimizeDeps: { noDiscovery: true },
  });
  ({ default: QuarantineSection } = await server.ssrLoadModule(
    "/src/components/QuarantineSection.jsx"
  ));
});

after(async () => { await server?.close(); });

beforeEach(() => {
  dom = new JSDOM("<!doctype html><div id='root'></div>");
  globalThis.window = dom.window;
  globalThis.document = dom.window.document;
  globalThis.IS_REACT_ACT_ENVIRONMENT = true;
  records = [];
  calls = [];
  restored = [];
  window.__TAURI_INTERNALS__ = {
    async invoke(command, args) {
      calls.push({ command, args });
      if (command === "list_quarantine") return records;
      if (command === "restore_quarantined") {
        records = records.filter((record) => record.quarantined_to !== args.quarantinedTo);
        return;
      }
      if (command === "purge_quarantine") {
        const count = records.length;
        records = [];
        return { files_removed: count, bytes_removed: count, failed: [] };
      }
      throw new Error(`Unexpected command: ${command}`);
    },
  };
  root = createRoot(document.getElementById("root"));
});

afterEach(async () => {
  await act(async () => root.unmount());
  dom.window.close();
  delete globalThis.window;
  delete globalThis.document;
  delete globalThis.IS_REACT_ACT_ENVIRONMENT;
});

function fixture(count) {
  return Array.from({ length: count }, (_, index) => ({
    original: `/source/file-${index}`,
    quarantined_to: `/quarantine/item-${index}`,
    at_epoch: 1_700_000_000 + index,
  }));
}

async function mount(count) {
  records = fixture(count);
  await act(async () => root.render(React.createElement(QuarantineSection, {
    quarantineDir: "/quarantine",
    refreshKey: 0,
    onRestored: (record) => restored.push(record),
  })));
}

function rows() { return [...document.querySelectorAll(".quarantine li.finding")]; }

test("10,000 records mount at most 50 quarantine rows while collapsed", async (t) => {
  await mount(10_000);
  const mounted = rows().length;
  t.diagnostic(`fixture records: ${records.length}; mounted rows: ${mounted}`);
  assert.ok(mounted > 0 && mounted <= 50, `expected at most 50 rows, got ${mounted}`);
  assert.equal(document.querySelector(".group-body").classList.contains("open"), false);
  assert.match(document.querySelector(".group-meta").textContent, /10000 files/);
});

function button(label) {
  const found = [...document.querySelectorAll("button")].find((item) =>
    item.getAttribute("aria-label") === label || item.textContent.trim() === label
  );
  assert.ok(found, `missing button: ${label}`);
  return found;
}

async function click(label) {
  await act(async () => button(label).click());
}

function rowPaths() { return rows().map((row) => row.querySelector(".path").textContent); }

test("all 10,000 records are reachable while every page mounts at most 50 rows", async (t) => {
  await mount(10_000);
  await act(async () => document.querySelector(".group-header").click());
  assert.equal(document.querySelector(".group-body").classList.contains("open"), true);
  const seen = new Set();
  let maximum = 0;
  let pages = 0;
  while (true) {
    const visible = rowPaths();
    maximum = Math.max(maximum, visible.length);
    assert.ok(visible.length > 0 && visible.length <= 50);
    visible.forEach((path) => { assert.ok(!seen.has(path)); seen.add(path); });
    pages += 1;
    assert.ok(pages <= 200, "navigation must stop at the final page");
    if (button("Next quarantine page").disabled) break;
    await click("Next quarantine page");
  }
  assert.equal(pages, 200);
  assert.deepEqual([...seen], fixture(10_000).map((record) => record.original));
  assert.match(document.querySelector('[role="status"]').textContent, /Page 200 of 200/);
  assert.equal(button("Last quarantine page").disabled, true);
  await click("Previous quarantine page");
  assert.equal(rowPaths()[0], "/source/file-9900");
  await click("First quarantine page");
  assert.equal(rowPaths()[0], "/source/file-0");
  assert.equal(button("Previous quarantine page").disabled, true);
  await click("Last quarantine page");
  assert.equal(rowPaths()[0], "/source/file-9950");
  t.diagnostic(`fixture records: 10000; pages visited: ${pages}; maximum mounted rows: ${maximum}`);
});

test("restore on a later page uses the exact record and keeps surviving row identities", async () => {
  await mount(102);
  await click("Next quarantine page");
  const expected = records[50];
  const survivingRow = rows()[1];
  await act(async () => rows()[0].querySelector("button").click());
  assert.deepEqual(calls.filter((call) => call.command === "restore_quarantined"), [{
    command: "restore_quarantined",
    args: { quarantineDir: "/quarantine", quarantinedTo: expected.quarantined_to },
  }]);
  assert.deepEqual(restored, [expected]);
  assert.equal(rows()[0], survivingRow, "quarantined paths must remain the React keys");
  assert.equal(rowPaths()[0], "/source/file-51");
  assert.equal(rows().length, 50);
  assert.match(document.querySelector(".group-meta").textContent, /101 files/);
  assert.match(document.querySelector('[role="status"]').textContent, /Files 51–100 of 101/);
});

test("restoring the only row on the last page returns to a populated page", async () => {
  await mount(51);
  await click("Last quarantine page");
  assert.deepEqual(rowPaths(), ["/source/file-50"]);
  await click("Restore");
  assert.equal(rows().length, 50);
  assert.equal(rowPaths()[0], "/source/file-0");
  assert.equal(document.querySelector(".quarantine-pagination"), null);
  assert.match(document.querySelector(".group-meta").textContent, /50 files/);
});

test("a manifest refresh clamps the current page after records disappear", async () => {
  await mount(10_000);
  await click("Last quarantine page");
  records = records.slice(0, 75);
  await act(async () => root.render(React.createElement(QuarantineSection, {
    quarantineDir: "/quarantine", refreshKey: 1, onRestored: (record) => restored.push(record),
  })));
  assert.equal(rows().length, 25);
  assert.equal(rowPaths()[0], "/source/file-50");
  assert.match(document.querySelector('[role="status"]').textContent, /Page 2 of 2/);
  assert.equal(button("Next quarantine page").disabled, true);
});

test("purge confirmation and deletion cover all records, including other pages", async () => {
  await mount(103);
  await click("Last quarantine page");
  assert.equal(rows().length, 3);
  assert.match(document.querySelector(".quarantine-note").textContent, /other pages/);
  await click("Purge quarantine");
  assert.match(document.querySelector(".confirm-q").textContent, /Delete all 103 files for good/);
  assert.equal(calls.some((call) => call.command === "purge_quarantine"), false);
  await click("Cancel");
  assert.equal(calls.some((call) => call.command === "purge_quarantine"), false);
  await click("Purge quarantine");
  await click("Delete");
  assert.deepEqual(calls.filter((call) => call.command === "purge_quarantine"), [{
    command: "purge_quarantine", args: { quarantineDir: "/quarantine" },
  }]);
  assert.equal(records.length, 0);
  assert.equal(rows().length, 0);
  assert.equal(document.querySelector(".quarantine-pagination"), null);
  assert.match(document.querySelector(".notice").textContent, /Deleted 103 files/);
});

test("an empty history hides the quarantine section", async () => {
  await mount(0);
  assert.equal(document.querySelector(".quarantine"), null);
});

test("short histories show every row without pagination", async () => {
  await mount(20);
  assert.equal(rows().length, 20);
  assert.equal(document.querySelector(".quarantine-pagination"), null);
});

test("a pending restore disables pagination and competing actions until reload finishes", async () => {
  await mount(102);
  await click("Next quarantine page");
  const invoke = window.__TAURI_INTERNALS__.invoke;
  let finishRestore;
  window.__TAURI_INTERNALS__.invoke = (command, args) => {
    if (command !== "restore_quarantined") return invoke(command, args);
    return new Promise((resolve) => {
      finishRestore = async () => { await invoke(command, args); resolve(); };
    });
  };
  await click("Restore");
  assert.ok([...document.querySelectorAll(".quarantine-pagination button")].every((item) => item.disabled));
  assert.ok(rows().slice(1).every((row) => row.querySelector("button").disabled));
  assert.equal(button("Purge quarantine").disabled, true);
  assert.match(rows()[0].textContent, /Restoring/);
  await act(async () => finishRestore());
  assert.equal(button("Next quarantine page").disabled, false);
  assert.equal(rowPaths()[0], "/source/file-51");
});

test("a failed restore keeps the current page and restores its controls", async () => {
  await mount(102);
  await click("Next quarantine page");
  const invoke = window.__TAURI_INTERNALS__.invoke;
  window.__TAURI_INTERNALS__.invoke = async (command, args) => {
    if (command === "restore_quarantined") throw new Error("Restore denied");
    return invoke(command, args);
  };
  await click("Restore");
  assert.equal(rowPaths()[0], "/source/file-50");
  assert.equal(rows().length, 50);
  assert.equal(records.length, 102);
  assert.match(document.querySelector(".error").textContent, /Restore denied/);
  assert.equal(button("Next quarantine page").disabled, false);
});

test("a pending purge disables page controls and restores until the manifest reloads", async () => {
  await mount(103);
  await click("Last quarantine page");
  const invoke = window.__TAURI_INTERNALS__.invoke;
  let finishPurge;
  window.__TAURI_INTERNALS__.invoke = (command, args) => {
    if (command !== "purge_quarantine") return invoke(command, args);
    return new Promise((resolve) => {
      finishPurge = async () => resolve(await invoke(command, args));
    });
  };
  await click("Purge quarantine");
  await click("Delete");
  assert.ok([...document.querySelectorAll(".quarantine-pagination button")].every((item) => item.disabled));
  assert.ok(rows().every((row) => row.querySelector("button").disabled));
  assert.match(document.querySelector(".purge").textContent, /Deleting/);
  await act(async () => finishPurge());
  assert.equal(rows().length, 0);
  assert.match(document.querySelector(".notice").textContent, /Deleted 103 files/);
});

for (const staleOutcome of ["success", "failure"]) {
  test(`an older refresh ${staleOutcome} cannot overwrite a later restore refresh`, async () => {
    await mount(51);
    await click("Last quarantine page");
    const oldRecords = records;
    const originalInvoke = window.__TAURI_INTERNALS__.invoke;
    let finishOld;
    let delayed = false;
    window.__TAURI_INTERNALS__.invoke = (command, args) => {
      if (command === "list_quarantine" && !delayed) {
        delayed = true;
        return new Promise((resolve, reject) => {
          finishOld = () => staleOutcome === "success"
            ? resolve(oldRecords) : reject(new Error("stale refresh failure"));
        });
      }
      return originalInvoke(command, args);
    };
    await act(async () => root.render(React.createElement(QuarantineSection, {
      quarantineDir: "/quarantine",
      refreshKey: 1,
      onRestored: (record) => restored.push(record),
    })));
    await click("Restore");
    assert.equal(records.length, 50);
    await act(async () => finishOld());
    assert.ok(!document.querySelector('[aria-label="Quarantine pages"]'), "stale records must not restore the pager");
    assert.match(document.querySelector(".group-meta").textContent, /50 files/);
    assert.ok(!document.querySelector(".error"), "stale errors must not replace a successful refresh");
    assert.equal(rowPaths().includes("/source/file-50"), false);
  });
}
