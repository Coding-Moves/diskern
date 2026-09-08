import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'

const indexHtml = readFileSync(new URL('../index.html', import.meta.url), 'utf8')
const themeContext = readFileSync(
  new URL('../src/context/ThemeContext.jsx', import.meta.url),
  'utf8',
)
const styles = readFileSync(
  new URL('../src/styles/index.css', import.meta.url),
  'utf8',
)

test('the initial document paints in the default dark theme', () => {
  assert.match(indexHtml, /<html\b[^>]*\bdata-theme=["']dark["'][^>]*>/)
  assert.match(
    indexHtml,
    /<style>[\s\S]*?html\s*\{[^}]*background-color:\s*#0b0d10;[^}]*color-scheme:\s*dark;[^}]*}/,
  )
})

test('ThemeProvider applies the selected theme to the document root', () => {
  assert.match(
    themeContext,
    /document\.documentElement\.dataset\.theme\s*=\s*theme/,
  )
  assert.doesNotMatch(themeContext, /<div\b[^>]*\bdata-theme=/)
})

test('theme rules control native UI and paint the document canvas', () => {
  assert.match(
    styles,
    /\[data-theme='dark'\]\s*\{[^}]*color-scheme:\s*dark;/s,
  )
  assert.match(
    styles,
    /\[data-theme='light'\]\s*\{[^}]*color-scheme:\s*light;/s,
  )
  assert.match(
    styles,
    /html\s*,\s*body\s*\{[^}]*background-color:\s*var\(--bg\);/s,
  )
})
