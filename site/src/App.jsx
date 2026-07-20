import { Routes, Route } from 'react-router-dom'
import Layout from './components/Layout.jsx'
import Home from './pages/Home.jsx'
import NotFound from './pages/NotFound.jsx'

// All routes share <Layout/> (nav + footer). To add a new page later —
// e.g. /docs or /changelog — create src/pages/Docs.jsx and add
// <Route path="docs" element={<Docs />} /> below.
export default function App() {
  return (
    <Routes>
      <Route element={<Layout />}>
        <Route index element={<Home />} />
        <Route path="*" element={<NotFound />} />
      </Route>
    </Routes>
  )
}
