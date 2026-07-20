import { createContext, useContext, useMemo, useState } from 'react'

const ThemeContext = createContext(null)

// Plain React state, not localStorage: the choice lives only for the
// current page load and resets on refresh. That's a deliberate
// tradeoff — localStorage would persist the preference across visits,
// but it also silently fails in some embedded/private-browsing
// contexts, and this is a small marketing site where that edge case
// isn't worth the complexity. If you want persistence later, swap the
// useState below for a small useEffect-backed localStorage hook.
export function ThemeProvider({ children }) {
  const [theme, setTheme] = useState('dark')

  const value = useMemo(
    () => ({
      theme,
      toggleTheme: () => setTheme((t) => (t === 'dark' ? 'light' : 'dark')),
    }),
    [theme],
  )

  return (
    <ThemeContext.Provider value={value}>
      <div data-theme={theme}>{children}</div>
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  const ctx = useContext(ThemeContext)
  if (!ctx) throw new Error('useTheme must be used within a ThemeProvider')
  return ctx
}
