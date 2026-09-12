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

test("section bodies animate expand and collapse through the grid row", () => {
  // 0fr→1fr animates to natural height — no fixed max-height that could
  // clip a long findings list.
  assert.match(css, /\.group-body\s*\{[^}]*grid-template-rows\s*:\s*0fr\b/s);
  assert.match(css, /\.group-body\.open\s*\{[^}]*grid-template-rows\s*:\s*1fr\b/s);
  assert.match(
    css,
    /\.group-body\s*\{[^}]*transition\s*:[^}]*grid-template-rows\b/s,
    "the row animation must be a transition, not a jump"
  );
  // The clip wrapper is what lets the row shrink below content height.
  assert.match(css, /\.group-body-inner\s*\{[^}]*min-height\s*:\s*0\b/s);
  assert.match(css, /\.group-body-inner\s*\{[^}]*overflow\s*:\s*hidden\b/s);
});

test("a closed section body leaves the tab order", () => {
  assert.match(
    css,
    /\.group-body\s*\{[^}]*visibility\s*:\s*hidden\b/s,
    "collapsed content must not stay focusable"
  );
});

test("row actions fade and slide in on each state change", () => {
  assert.match(css, /@keyframes\s+action-in\b/);
  assert.match(
    css,
    /\.row-action\s*>\s*\*[^}]*animation\s*:\s*action-in\b/s,
    "idle/confirm/working swaps need the entrance animation"
  );
  assert.match(
    css,
    /\.purge\s*>\s*\*[^}]*animation\s*:\s*action-in\b/s,
    "the purge idle/confirm/working swap needs it too"
  );
});

test("working labels carry a spinner", () => {
  assert.match(css, /@keyframes\s+spin\b/);
  assert.match(
    css,
    /\.working::before\s*\{[^}]*animation\s*:[^}]*\bspin\b/s,
    "Moving…/Restoring…/Deleting… should spin, not just sit there"
  );
});

test("the row-action column is pinned so buttons do not jump", () => {
  assert.match(
    css,
    /\.row-action\s*\{[^}]*min-width\s*:/s,
    "the auto column must out-size every state so swaps don't resize the row"
  );
});

test("the chevron rotates instead of swapping glyphs", () => {
  assert.match(css, /\.chevron\s*\{[^}]*transition\s*:[^}]*transform\b/s);
  assert.match(css, /\.chevron\.open\s*\{[^}]*transform\s*:\s*rotate\s*\(/s);
});
