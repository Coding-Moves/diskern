import { useEffect, useState } from 'react'
import { RELEASES_API } from '../lib/releases.js'

// Fetches the latest GitHub release once on mount. GitHub's REST API
// allows 60 unauthenticated requests per hour per IP — plenty for a
// visitor loading this page a few times, but easy to exhaust while
// developing locally with fast refresh. When that happens (or the
// network fails, or the repo somehow has no releases yet) `status`
// becomes 'error' and the caller should fall back to linking straight
// at the GitHub Releases page instead of trying to render assets.
export function useLatestRelease() {
  const [status, setStatus] = useState('loading') // 'loading' | 'success' | 'error'
  const [release, setRelease] = useState(null)

  useEffect(() => {
    let cancelled = false

    fetch(RELEASES_API, {
      headers: { Accept: 'application/vnd.github+json' },
    })
      .then((res) => {
        if (!res.ok) throw new Error(`GitHub API responded ${res.status}`)
        return res.json()
      })
      .then((data) => {
        if (cancelled) return
        setRelease(data)
        setStatus('success')
      })
      .catch(() => {
        if (cancelled) return
        setStatus('error')
      })

    return () => {
      cancelled = true
    }
  }, [])

  return { status, release }
}
