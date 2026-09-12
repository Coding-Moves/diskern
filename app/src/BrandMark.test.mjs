import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const src = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "BrandMark.jsx"),
  "utf8"
);

test("BrandMark is the one component every surface reuses", () => {
  assert.match(
    src,
    /export default function BrandMark\(\{ className = "" \}\)/,
    "a single exported component taking a className variant"
  );
});

test("every render carries the shared brand-mark hook plus the caller's variant", () => {
  assert.match(src, /`brand-mark \$\{className\}`/);
});

test("the mark is always decorative", () => {
  // The copy next to it carries the meaning; screen readers should skip it.
  assert.match(src, /aria-hidden="true"/);
});

test("the glyph is stroke-only in currentColor — no hardcoded fills", () => {
  assert.match(src, /fill="none"/);
  assert.match(src, /stroke="currentColor"/);
  assert.doesNotMatch(
    src,
    /(?:fill|stroke)="#[0-9a-fA-F]{3,8}"/,
    "hardcoded colours would break on light or dark surfaces"
  );
});

test("it is still the shield-check glyph on the same viewBox", () => {
  assert.match(src, /viewBox="0 0 24 24"/);
  assert.match(
    src,
    /<path d="M12 3l7 2\.6V11c0 4\.6-3 7\.9-7 9-4-1\.1-7-4\.4-7-9V5\.6L12 3z" \/>/
  );
  assert.match(src, /<path d="M8\.8 12\.2l2\.2 2\.2 4\.4-4\.8" \/>/);
});
