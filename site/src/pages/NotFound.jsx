import { Link } from 'react-router-dom'

export default function NotFound() {
  return (
    <main className="not-found">
      <h1>Page not found</h1>
      <p>
        <Link to="/">Back to the Diskern homepage</Link>
      </p>
    </main>
  )
}
