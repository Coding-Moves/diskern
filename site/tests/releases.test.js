import assert from 'node:assert/strict'
import test from 'node:test'

import { matchDownloads } from '../src/lib/releases.js'

function windowsDownload(assets) {
  return matchDownloads(assets).find((download) => download.id === 'windows')
}

const cases = [
  {
    name: 'labels an MSI-only Windows download as an MSI installer',
    assets: [
      {
        name: 'Diskern.msi',
        browser_download_url: 'https://example.com/Diskern.msi',
      },
    ],
    url: 'https://example.com/Diskern.msi',
    sublabel: '.msi installer',
  },
  {
    name: 'labels an EXE-only Windows download as an EXE installer',
    assets: [
      {
        name: 'Diskern-setup.exe',
        browser_download_url: 'https://example.com/Diskern-setup.exe',
      },
    ],
    url: 'https://example.com/Diskern-setup.exe',
    sublabel: '.exe installer',
  },
  {
    name: 'uses the EXE label when an EXE is the first Windows asset',
    assets: [
      {
        name: 'Diskern-setup.exe',
        browser_download_url: 'https://example.com/Diskern-setup.exe',
      },
      {
        name: 'Diskern.msi',
        browser_download_url: 'https://example.com/Diskern.msi',
      },
    ],
    url: 'https://example.com/Diskern-setup.exe',
    sublabel: '.exe installer',
  },
  {
    name: 'uses the MSI label when an MSI is the first Windows asset',
    assets: [
      {
        name: 'Diskern.msi',
        browser_download_url: 'https://example.com/Diskern.msi',
      },
      {
        name: 'Diskern-setup.exe',
        browser_download_url: 'https://example.com/Diskern-setup.exe',
      },
    ],
    url: 'https://example.com/Diskern.msi',
    sublabel: '.msi installer',
  },
]

for (const { name, assets, url, sublabel } of cases) {
  test(name, () => {
    const download = windowsDownload(assets)

    assert.equal(download.url, url)
    assert.equal(download.sublabel, sublabel)
  })
}
