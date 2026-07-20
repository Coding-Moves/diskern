import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// `base` must match the repo name because GitHub Pages serves project
// sites from https://<user>.github.io/<repo>/ — every asset URL Vite
// generates gets this prefix. Wrong value = blank page with 404s in
// the browser console.
export default defineConfig({
  base: '/diskern/',
  plugins: [react()],
})
