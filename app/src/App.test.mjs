import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const jsx = readFileSync(
  join(dirname(fileURLToPath(import.meta.url)), "App.jsx"),
  "utf8"
);

// Just the ScanningIndicator component, from its declaration to the App
// component that follows it — the scan panel's whole markup lives there.
const scan = jsx.slice(
  jsx.indexOf("function ScanningIndicator"),
  jsx.indexOf("export default function App")
);
assert.ok(scan.length > 0, "App.jsx must define ScanningIndicator");

test("the scan panel keeps the same props contract", () => {
  assert.match(
    scan,
    /function ScanningIndicator\(\{ filesSeen, bytesSeen, phase, onCancel, cancelling \}\)/,
    "filesSeen / bytesSeen / onCancel / cancelling must keep working as today"
  );
});

test("the scan panel carries the shared brand mark", () => {
  assert.match(
    scan,
    /<BrandMark\s+className="scan-mark"\s*\/>/,
    "the shield glyph lives in BrandMark — one source, reused everywhere"
  );
});

test("the copy says plainly that scanning changes nothing", () => {
  assert.match(scan, /Scanning safely… nothing is being changed/);
});

test("the live counters still render from the same props", () => {
  assert.match(scan, /\{filesSeen\.toLocaleString\(\)\} files found/);
  assert.match(
    scan,
    /bytesSeen > 0 && <> · \{humanBytes\(bytesSeen\)\} so far<\/>/,
    "the byte counter still goes through humanBytes"
  );
});

test("cancelling gets its own calm state", () => {
  assert.match(
    scan,
    /cancelling \? " cancelling" : ""/,
    "the panel needs a cancelling modifier class"
  );
  assert.match(
    scan,
    /nothing has been changed/,
    "the cancelling copy should confirm nothing was touched"
  );
});

test("cancel still works the same way", () => {
  assert.match(scan, /onClick=\{onCancel\}/);
  assert.match(scan, /disabled=\{cancelling\}/);
  assert.match(scan, /cancelling \? "Stopping…" : "Cancel scan"/);
});

test("App imports the shared BrandMark component", () => {
  assert.match(jsx, /import BrandMark from "\.\/BrandMark\.jsx";/);
});

// The header lives inside App's JSX, not in the ScanningIndicator slice.
const header = jsx.slice(jsx.indexOf("<header>"), jsx.indexOf("</header>"));
assert.ok(header.length > 0, "App.jsx must render a <header>");

test("the header pairs the brand mark with the wordmark", () => {
  assert.match(
    header,
    /<BrandMark\s+className="header-mark"\s*\/>/,
    "the header reuses the same mark component, not a copy of the SVG"
  );
  assert.match(header, /<h1>Diskern<\/h1>/);
  assert.ok(
    header.indexOf("<BrandMark") < header.indexOf("<h1>"),
    "the mark leads the wordmark in the brand row"
  );
});
