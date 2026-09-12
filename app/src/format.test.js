import { test } from "node:test";
import assert from "node:assert/strict";
import { humanBytes } from "./format.js";

// Same cases as the human_bytes tests in crates/diskern-core/src/lib.rs
// and crates/diskern-cli/src/main.rs — the two frontends should describe
// a finding identically.
test("scales to a readable unit", () => {
  assert.equal(humanBytes(0), "0 B");
  assert.equal(humanBytes(500), "500 B");
  assert.equal(humanBytes(999), "999 B");
  assert.equal(humanBytes(1_000), "1.0 KB");
  assert.equal(humanBytes(1_500_000), "1.5 MB");
  assert.equal(humanBytes(2_300_000_000), "2.3 GB");
  assert.equal(humanBytes(2_000_000_000_000), "2.0 TB");
});

test("rolls over instead of printing a four-digit mantissa", () => {
  assert.equal(humanBytes(999_949), "999.9 KB");
  assert.equal(humanBytes(999_950), "1.0 MB");
  assert.equal(humanBytes(999_999), "1.0 MB");
  assert.equal(humanBytes(999_999_999), "1.0 GB");
});

test("stops at the largest unit it knows", () => {
  assert.equal(humanBytes(Number.MAX_SAFE_INTEGER), "9007.2 TB");
});

// The rows from issue #83: every one of these printed as "0.00 GB" or a
// misleading fixed unit before.
test("findings no longer read 0.00 GB", () => {
  assert.equal(humanBytes(2_048), "2.0 KB");
  assert.equal(humanBytes(40_000_000), "40.0 MB");
});
