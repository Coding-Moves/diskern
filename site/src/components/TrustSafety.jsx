const steps = [
  {
    title: 'Scan',
    description: 'Diskern reads your filesystem without changing anything.',
    icon: (
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z" />
        <circle cx="12" cy="12" r="3" />
      </svg>
    ),
  },
  {
    title: 'Review',
    description: 'You see what Diskern found before anything is removed.',
    icon: (
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <path d="m9 11 3 3L22 4" />
        <path d="M21 12v7a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h11" />
      </svg>
    ),
  },
  {
    title: 'Quarantine',
    description: 'Files can be quarantined and restored if you change your mind.',
    icon: (
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <path d="M3 12a9 9 0 1 0 3-6.7" />
        <path d="M3 4v6h6" />
        <path d="M12 7v5l3 2" />
      </svg>
    ),
  },
]
export default function TrustSafety() {
  return (
    <section className="trust-safety">
      <div className="container">
        <h2 className="section-title">Trust &amp; Safety</h2>

        <p className="section-subtitle">
          You stay in control at every step.
        </p>

        <div className="safety-flow">
          {steps.map((step, index) => (
            <div className="safety-step-wrapper" key={step.title}>
              <article className="safety-step">
                <div className="safety-step-icon" aria-hidden="true">
                  {step.icon}
                </div>

                <h3>{step.title}</h3>

                <p>{step.description}</p>
              </article>

              {index < steps.length - 1 && (
                <div className="safety-arrow" aria-hidden="true">
                  →
                </div>
              )}
            </div>
          ))}
        </div>

        <div className="safety-rules">
          <div className="safety-rules-icon" aria-hidden="true">
            <svg
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path d="M12 3 4 6v5c0 5 3.4 8.8 8 10 4.6-1.2 8-5 8-10V6l-8-3Z" />
              <path d="m9 12 2 2 4-4" />
            </svg>
          </div>

          <div className="safety-rules-content">
            <h3>Safety is rule-based</h3>
            <p>
              Safety verdicts come from explicit, auditable rules — not AI
              guesses.
            </p>
          </div>
        </div>
      </div>
    </section>
  )
}