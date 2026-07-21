import { useEffect, useState } from 'react'
import { RELEASES_API } from '../lib/releases.js'

// Build-time snapshot written by the deploy workflow (see
// .github/workflows/deploy-pages.yml). Same-origin, so no CORS and no
// rate limit — this is the primary source. BASE_URL is "/diskern/" in
// production, "/" in local dev.
const STATIC_RELEASE_URL = `${import.meta.env.BASE_URL}latest-release.json`

// Fetches the latest release, preferring the static snapshot baked into
// the deployed site and falling back to GitHub's live API.
//
// Why the snapshot first: GitHub's REST API allows only 60 unauthenticated
// requests/hour PER IP, shared across everyone behind the same NAT. Calling
// it on every page load meant a handful of visitors (or refreshes) from one
// network exhausted the budget and everyone got the error fallback. The
// deploy workflow fetches the release server-side with GITHUB_TOKEN (a much
// higher, authenticated limit) and writes it to a static file, so normal
// visitors never touch the rate-limited API at all.
//
// The live API remains a fallback for the cases the snapshot can't cover:
// local dev (no file generated) and any deploy where generation failed.
export function useLatestRelease() {
  const [status, setStatus] = useState('loading') // 'loading' | 'success' | 'error'
  const [release, setRelease] = useState(null)
  const [error, setError] = useState(null)

  useEffect(() => {
    let cancelled = false

    async function load() {
      // 1. Static snapshot (same-origin, no rate limit).
      try {
        const res = await fetch(STATIC_RELEASE_URL, { cache: 'no-cache' })
        // A missing file on GitHub Pages returns the SPA 404 shell, so
        // guard on both res.ok and the content actually being release JSON.
        if (res.ok) {
          const data = await res.json()
          if (data && (data.tag_name || data.assets)) {
            if (!cancelled) {
              setRelease(data)
              setStatus('success')
            }
            return
          }
        }
      } catch {
        // Not JSON / not present — fall through to the live API.
      }

      // 2. Live GitHub API fallback (may be rate-limited).
      try {
        const res = await fetch(RELEASES_API, {
          headers: { Accept: 'application/vnd.github+json' },
        })
        if (!res.ok) {
          const limited = res.status === 403 || res.status === 429
          throw new Error(
            `GitHub API responded ${res.status}${limited ? ' (rate limit — 60 requests/hour per IP)' : ''}`,
          )
        }
        const data = await res.json()
        if (!cancelled) {
          setRelease(data)
          setStatus('success')
        }
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e))
          setStatus('error')
        }
      }
    }

    load()

    return () => {
      cancelled = true
    }
  }, [])

  return { status, release, error }
}
