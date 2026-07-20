export default function Hero() {
  return (
    <section className="hero">
      <img
        src={`${import.meta.env.BASE_URL}favicon.svg`}
        alt="Diskern logo"
        className="hero-logo"
      />
      <h1>Diskern</h1>
      <p className="hero-tagline">Understand your disk before you clean it.</p>
      <p className="hero-description">
        Diskern scans your drives and shows you exactly where the space went —
        biggest folders, duplicate files, old caches — before you delete
        anything. It's a read-only analyzer first: nothing gets removed until
        you tell it to.
      </p>
    </section>
  )
}
