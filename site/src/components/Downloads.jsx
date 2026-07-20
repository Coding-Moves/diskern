import { useMemo } from 'react'
import { useLatestRelease } from '../hooks/useLatestRelease.js'
import {
  RELEASES_PAGE,
  matchDownloads,
  detectOS,
  formatBytes,
  formatDate,
} from '../lib/releases.js'
import { WindowsIcon, LinuxIcon, DownloadIcon } from './icons.jsx'

const OS_ICONS = {
  windows: WindowsIcon,
  linux: LinuxIcon,
}

export default function Downloads() {
  const { status, release } = useLatestRelease()
  const visitorOS = useMemo(() => detectOS(), [])

  return (
    <section id="download">
      <h2 className="section-title">Download</h2>
      <p className="section-subtitle">
        Pick your platform. All builds are unsigned for now — see the release
        notes for details.
      </p>

      <div className="downloads-card">
        {status === 'loading' && <LoadingState />}
        {status === 'error' && <FallbackState />}
        {status === 'success' && (
          <ReleaseState release={release} visitorOS={visitorOS} />
        )}
      </div>
    </section>
  )
}

function LoadingState() {
  return (
    <div className="downloads-skeleton">
      <div className="spinner" role="status" aria-label="Loading latest release" />
      <span>Fetching the latest release…</span>
    </div>
  )
}

function FallbackState() {
  return (
    <div className="downloads-fallback">
      <p>
        Couldn't load the latest release automatically — GitHub's API limits
        anonymous requests, or your connection blipped.
      </p>
      <p>
        <a href={RELEASES_PAGE} target="_blank" rel="noopener noreferrer">
          <DownloadIcon style={{ width: 16, height: 16, display: 'inline', verticalAlign: '-3px' }} />
          {' '}View all releases on GitHub
        </a>
      </p>
    </div>
  )
}

function ReleaseState({ release, visitorOS }) {
  const downloads = matchDownloads(release.assets)
  const version = release.name || release.tag_name
  const notes = (release.body || '').trim()
  const truncatedNotes =
    notes.length > 420 ? `${notes.slice(0, 420).trimEnd()}…` : notes

  return (
    <>
      <div className="downloads-meta">
        <span className="downloads-version">{version}</span>
        <span className="downloads-date">
          Released {formatDate(release.published_at)}
        </span>
      </div>

      {downloads.length === 0 ? (
        <div className="downloads-fallback">
          <p>
            No installers are attached to this release yet.{' '}
            <a href={release.html_url} target="_blank" rel="noopener noreferrer">
              Check the release page
            </a>
            .
          </p>
        </div>
      ) : (
        <div className="downloads-grid">
          {downloads.map((d) => {
            const Icon = OS_ICONS[d.os] ?? DownloadIcon
            const recommended = d.os === visitorOS
            return (
              <a
                key={d.id}
                href={d.url}
                className={`download-button${recommended ? ' is-recommended' : ''}`}
              >
                <Icon className="os-icon" />
                <span className="download-label">
                  {d.label}
                  <span className="download-sub">
                    {d.sublabel}
                    {d.size ? ` · ${formatBytes(d.size)}` : ''}
                  </span>
                </span>
                {recommended && <span className="recommended-badge">You</span>}
              </a>
            )
          })}
        </div>
      )}

      {visitorOS === 'mac' && (
        <p className="downloads-notes-link" style={{ marginTop: 20 }}>
          A native macOS build isn't available yet — it's on the roadmap.
        </p>
      )}

      {notes && (
        <>
          <p className="downloads-notes">{truncatedNotes}</p>
          <a
            className="downloads-notes-link"
            href={release.html_url}
            target="_blank"
            rel="noopener noreferrer"
          >
            Read full release notes on GitHub →
          </a>
        </>
      )}
    </>
  )
}
