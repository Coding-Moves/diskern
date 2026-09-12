import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const css = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "styles.css"),
  "utf8"
);

// The body of the `@media (prefers-reduced-motion: reduce)` block, found
// by walking braces from its opening '{' — a lazy regex would stop at the
// first inner rule's close.
function reducedMotionBlock() {
  const marker = /@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)\s*\{/;
  const match = marker.exec(css);
  assert.ok(match, "styles.css must handle prefers-reduced-motion: reduce");

  const start = match.index + match[0].length;
  let depth = 1;
  let end = start;
  for (; end < css.length && depth > 0; end++) {
    if (css[end] === "{") depth++;
    if (css[end] === "}") depth--;
  }
  return css.slice(start, end - 1);
}

test("the stylesheet handles prefers-reduced-motion", () => {
  assert.match(css, /@media\s*\(\s*prefers-reduced-motion\s*:\s*reduce\s*\)/);
});

test("reduced motion collapses animations and transitions", () => {
  const block = reducedMotionBlock();
  assert.match(block, /animation-duration\s*:\s*0(\.\d+)?m?s\b/);
  assert.match(block, /animation-iteration-count\s*:\s*1\b/);
  assert.match(block, /transition-duration\s*:\s*0(\.\d+)?m?s\b/);
});

test("the indeterminate scan bar stops sweeping under reduced motion", () => {
  const block = reducedMotionBlock();
  assert.match(
    block,
    /\.progress-fill-indeterminate\s*\{[^}]*animation\s*:\s*none\b/s,
    "the looping scan animation must be switched off, not just shortened"
  );
});
