// Small hand-rolled icon set so the site doesn't need an icon-library
// dependency for half a dozen glyphs. Each icon inherits color from
// its parent via `currentColor` and sizes via the CSS on its wrapper.

export function SunIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <circle cx="12" cy="12" r="4.5" fill="currentColor" />
      <g stroke="currentColor" strokeWidth="1.8" strokeLinecap="round">
        <path d="M12 2.5v2.5M12 19v2.5M4.4 4.4l1.77 1.77M17.83 17.83l1.77 1.77M2.5 12H5M19 12h2.5M4.4 19.6l1.77-1.77M17.83 6.17l1.77-1.77" />
      </g>
    </svg>
  )
}

export function MoonIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        d="M20.5 14.5A8.5 8.5 0 1 1 9.5 3.5a7 7 0 0 0 11 11Z"
        fill="currentColor"
      />
    </svg>
  )
}

export function GithubIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        fill="currentColor"
        d="M12 2C6.48 2 2 6.58 2 12.21c0 4.51 2.87 8.33 6.84 9.68.5.1.68-.22.68-.49 0-.24-.01-.87-.01-1.71-2.78.62-3.37-1.37-3.37-1.37-.46-1.18-1.11-1.5-1.11-1.5-.91-.63.07-.62.07-.62 1 .07 1.53 1.05 1.53 1.05.89 1.56 2.34 1.11 2.91.85.09-.66.35-1.11.63-1.37-2.22-.26-4.56-1.14-4.56-5.05 0-1.12.39-2.03 1.03-2.74-.1-.26-.45-1.31.1-2.73 0 0 .84-.28 2.75 1.05a9.3 9.3 0 0 1 5 0c1.91-1.33 2.75-1.05 2.75-1.05.55 1.42.2 2.47.1 2.73.64.71 1.03 1.62 1.03 2.74 0 3.92-2.34 4.78-4.57 5.04.36.32.68.94.68 1.9 0 1.37-.01 2.47-.01 2.81 0 .27.18.6.69.49A10.02 10.02 0 0 0 22 12.21C22 6.58 17.52 2 12 2Z"
      />
    </svg>
  )
}

export function WindowsIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        fill="currentColor"
        d="M3 5.5 10.4 4.4v7.1H3V5.5Zm8.4-1.24L21 3v8.4h-9.6V4.26ZM3 12.6h7.4v7.1L3 18.5v-5.9Zm8.4 0H21V21l-9.6-1.4v-7Z"
      />
    </svg>
  )
}

export function LinuxIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        fill="currentColor"
        d="M12 2c-1.66 0-3 1.79-3 4 0 1.36.47 2.24.9 3.02.3.55.6 1.08.6 1.73 0 .53-.28.9-.85 1.51-.86.93-2.15 2.34-2.15 4.74 0 2.62 2.24 5 6.5 5s6.5-2.38 6.5-5c0-2.4-1.29-3.81-2.15-4.74-.57-.61-.85-.98-.85-1.51 0-.65.3-1.18.6-1.73.43-.78.9-1.66.9-3.02 0-2.21-1.34-4-3-4-1 0-1.62.53-2 1.03-.38-.5-1-1.03-2-1.03Z"
      />
      <ellipse cx="9.6" cy="9.4" rx="0.9" ry="1.1" fill="#12161c" />
      <ellipse cx="14.4" cy="9.4" rx="0.9" ry="1.1" fill="#12161c" />
    </svg>
  )
}

export function DownloadIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M12 3v12m0 0 4.5-4.5M12 15 7.5 10.5M4 19.5h16"
      />
    </svg>
  )
}

export function EyeIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z"
      />
      <circle cx="12" cy="12" r="3" fill="currentColor" />
    </svg>
  )
}

export function CopyIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <rect
        x="8.5"
        y="8.5"
        width="12"
        height="12"
        rx="2"
        stroke="currentColor"
        strokeWidth="1.8"
      />
      <path
        stroke="currentColor"
        strokeWidth="1.8"
        d="M15.5 8.5V5.5A2 2 0 0 0 13.5 3.5H5.5a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h3"
      />
    </svg>
  )
}

export function ShieldIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinejoin="round"
        d="M12 3.5 19 6v5.5c0 4.5-3 7.5-7 9-4-1.5-7-4.5-7-9V6l7-2.5Z"
      />
      <path
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
        d="m9 12 2.2 2.2L15.5 10"
      />
    </svg>
  )
}

export function NoTrashIcon(props) {
  return (
    <svg viewBox="0 0 24 24" fill="none" aria-hidden="true" {...props}>
      <path
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        strokeLinejoin="round"
        d="M5 7h14M9.5 7V5a1.5 1.5 0 0 1 1.5-1.5h2A1.5 1.5 0 0 1 14.5 5v2M7 7l.7 11.2A2 2 0 0 0 9.7 20h4.6a2 2 0 0 0 2-1.8L17 7"
      />
      <path
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
        d="m3.5 3.5 17 17"
      />
    </svg>
  )
}
