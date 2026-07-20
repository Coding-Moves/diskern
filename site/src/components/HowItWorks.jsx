import { EyeIcon, CopyIcon, ShieldIcon, NoTrashIcon } from './icons.jsx'

const FEATURES = [
  {
    icon: EyeIcon,
    title: 'Read-only scanning',
    description:
      "Diskern only reads your filesystem to build its picture of what's using space. It never touches a file during a scan.",
  },
  {
    icon: CopyIcon,
    title: 'Duplicate detection',
    description:
      'Finds exact duplicate files by content hash, not just name, so you can see what’s actually safe to consolidate.',
  },
  {
    icon: ShieldIcon,
    title: 'Rules-based safety',
    description:
      'System and application-critical paths are recognized and flagged, so you don’t accidentally target something the OS needs.',
  },
  {
    icon: NoTrashIcon,
    title: 'Never hard-deletes',
    description:
      'Anything you remove goes to your OS trash/recycle bin first — Diskern never permanently erases a file on your behalf.',
  },
]

export default function HowItWorks() {
  return (
    <section id="how-it-works">
      <h2 className="section-title">How it works</h2>
      <p className="section-subtitle">
        Diskern is built to be trustworthy first, fast second.
      </p>
      <div className="features-grid">
        {FEATURES.map(({ icon: Icon, title, description }) => (
          <div className="feature-card" key={title}>
            <Icon className="feature-icon" />
            <h3>{title}</h3>
            <p>{description}</p>
          </div>
        ))}
      </div>
    </section>
  )
}
