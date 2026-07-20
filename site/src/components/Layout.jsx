import { Outlet } from 'react-router-dom'
import NavBar from './NavBar.jsx'
import Footer from './Footer.jsx'

// Shared shell for every route: nav + footer stay put, <Outlet/> swaps
// in whichever page matched. Add new routes in App.jsx and they get
// the nav/theme-toggle/footer for free.
export default function Layout() {
  return (
    <div className="app-shell">
      <NavBar />
      <Outlet />
      <Footer />
    </div>
  )
}
