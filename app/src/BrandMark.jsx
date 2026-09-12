import React from "react";

/**
 * The Diskern mark: a shield-check drawn once and reused wherever the UI
 * needs the brand — the header today, the scanning panel, and any future
 * empty or update states.
 *
 * The glyph is stroke-only in `currentColor` with no fill of its own, so
 * it stays crisp on light and dark surfaces and picks up whatever colour
 * the surface's rule sets. It is decorative by contract — the copy next
 * to it always carries the meaning — so it is permanently aria-hidden.
 * Callers size and colour it through `className`; `brand-mark` is the
 * shared hook and gives a sane default size.
 */
export default function BrandMark({ className = "" }) {
  return (
    <svg
      className={className ? `brand-mark ${className}` : "brand-mark"}
      viewBox="0 0 24 24"
      aria-hidden="true"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M12 3l7 2.6V11c0 4.6-3 7.9-7 9-4-1.1-7-4.4-7-9V5.6L12 3z" />
      <path d="M8.8 12.2l2.2 2.2 4.4-4.8" />
    </svg>
  );
}
