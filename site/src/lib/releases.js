export const REPO = 'Coding-Moves/diskern'
export const RELEASES_API = `https://api.github.com/repos/${REPO}/releases/latest`
export const RELEASES_PAGE = `https://github.com/${REPO}/releases`

// One entry per download button we're able to offer. `match` looks at
// the release asset's filename (lowercased) and returns true if this
// button should be built from it. Order matters: first match wins,
// so more specific extensions (.appimage) should stay ahead of
// anything that could double-match.
const ASSET_TYPES = [
  {
    id: 'windows',
    os: 'windows',
    label: 'Download for Windows',
    sublabel: '.msi installer',
    match: (name) => name.endsWith('.msi') || name.endsWith('.exe'),
  },
  {
    id: 'debian',
    os: 'linux',
    label: 'Download for Debian/Ubuntu',
    sublabel: '.deb package',
    match: (name) => name.endsWith('.deb'),
  },
  {
    id: 'fedora',
    os: 'linux',
    label: 'Download for Fedora/RHEL',
    sublabel: '.rpm package',
    match: (name) => name.endsWith('.rpm'),
  },
  {
    id: 'appimage',
    os: 'linux',
    label: 'Download for Linux (AppImage)',
    sublabel: '.AppImage — runs on most distros',
    match: (name) => name.endsWith('.appimage'),
  },
]

// Builds the list of download buttons to render from a GitHub release's
// `assets` array, matching each known asset type against the first
// release asset whose filename fits.
export function matchDownloads(assets = []) {
  return ASSET_TYPES.map((type) => {
    const asset = assets.find((a) => type.match(a.name.toLowerCase()))
    if (!asset) return null
    return {
      id: type.id,
      os: type.os,
      label: type.label,
      sublabel: type.sublabel,
      url: asset.browser_download_url,
      size: asset.size,
    }
  }).filter(Boolean)
}

// Best-effort OS guess from the browser, used only to highlight the
// most likely download for the visitor — never to hide the others.
export function detectOS() {
  if (typeof navigator === 'undefined') return 'unknown'
  const ua = `${navigator.userAgent} ${navigator.platform ?? ''}`.toLowerCase()
  if (ua.includes('win')) return 'windows'
  if (ua.includes('mac')) return 'mac'
  if (ua.includes('linux') || ua.includes('x11')) return 'linux'
  return 'unknown'
}

export function formatBytes(bytes) {
  if (!bytes) return ''
  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unitIndex = 0
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024
    unitIndex += 1
  }
  return `${value.toFixed(value >= 10 || unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`
}

export function formatDate(iso) {
  if (!iso) return ''
  return new Date(iso).toLocaleDateString(undefined, {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
  })
}
