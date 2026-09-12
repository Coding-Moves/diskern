import assert from "node:assert/strict";
import { test } from "node:test";
import { visibleDuplicateSets } from "./duplicates.js";

const twoCopySet = {
  hash: "two",
  size: 10,
  paths: ["/scan/a.bin", "/scan/b.bin"],
  wasted: 10,
};

const threeCopySet = {
  hash: "three",
  size: 4,
  paths: ["/scan/one.tmp", "/scan/two.tmp", "/scan/three.tmp"],
  wasted: 8,
};

test("hides a two-copy duplicate set after one copy is quarantined", () => {
  const sets = visibleDuplicateSets([twoCopySet], new Set(["/scan/a.bin"]));

  assert.deepEqual(sets, []);
});

test("recalculates wasted bytes for a three-copy set", () => {
  const sets = visibleDuplicateSets([threeCopySet], new Set(["/scan/two.tmp"]));

  assert.equal(sets.length, 1);
  assert.deepEqual(sets[0].paths, ["/scan/one.tmp", "/scan/three.tmp"]);
  assert.equal(sets[0].wasted, 4);
});

test("restored paths make duplicate sets visible again", () => {
  const hidden = new Set(["/scan/a.bin"]);
  assert.deepEqual(visibleDuplicateSets([twoCopySet], hidden), []);

  hidden.delete("/scan/a.bin");
  const sets = visibleDuplicateSets([twoCopySet], hidden);

  assert.equal(sets.length, 1);
  assert.deepEqual(sets[0], twoCopySet);
});

test("does not produce negative wasted totals when every copy is hidden", () => {
  const sets = visibleDuplicateSets(
    [threeCopySet],
    new Set(["/scan/one.tmp", "/scan/two.tmp", "/scan/three.tmp"])
  );

  assert.deepEqual(sets, []);
});
