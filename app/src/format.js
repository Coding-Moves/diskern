// Mirror of `human_bytes` in crates/diskern-core/src/lib.rs — same SI
// units and the same 999.95 carry, so the app and the CLI describe the
// same finding the same way.
export function humanBytes(n) {
  const UNITS = ["B", "KB", "MB", "GB", "TB"];
  let value = n;
  let unit = 0;
  // 999.95, not 1000: at one decimal place anything at or above that
  // rounds to "1000.0", which belongs in the next unit up. Choosing the
  // unit before rounding printed 999_999 as "1000.0 KB".
  while (value >= 999.95 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  if (unit === 0) {
    return `${Math.trunc(value)} B`;
  }
  return `${value.toFixed(1)} ${UNITS[unit]}`;
}
