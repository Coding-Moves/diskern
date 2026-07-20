import { Link } from 'react-router-dom'
import { useTheme } from '../context/ThemeContext.jsx'
import { SunIcon, MoonIcon, GithubIcon } from './icons.jsx'

const REPO_URL = 'https://github.com/Coding-Moves/diskern'

export default function NavBar() {
  const { theme, toggleTheme } = useTheme()

  return (
    <header className="nav">
      <div className="nav-inner">
        <Link to="/" className="brand">
          <img
            src={`${import.meta.env.BASE_URL}favicon.svg`}
            alt=""
            className="brand-logo"
          />
          Diskern
        </Link>

        <div className="nav-actions">
          <button
            type="button"
            className="icon-button"
            onClick={toggleTheme}
            aria-label={
              theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'
            }
            title={theme === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'}
          >
            {theme === 'dark' ? <SunIcon /> : <MoonIcon />}
          </button>

          <a
            href={REPO_URL}
            target="_blank"
            rel="noopener noreferrer"
            className="icon-button"
            aria-label="View Diskern on GitHub"
            title="View on GitHub"
          >
            <GithubIcon />
          </a>
        </div>
      </div>
    </header>
  )
}
