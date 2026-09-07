import { readdir, readFile, stat } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'
import { gzipSync } from 'node:zlib'
import assert from 'node:assert/strict'

// Payload budgets catch growth; they do not measure native RAM or old-hardware speed.
const root = fileURLToPath(new URL('../', import.meta.url))
const assets = join(root, 'apps/desktop/dist')
const totals = { js: 0, css: 0, all: 0 }
const limits = { js: 100 * 1024, css: 20 * 1024, all: 1024 * 1024 }

async function inspect(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) await inspect(path)
    else {
      totals.all += (await stat(path)).size
      if (entry.name.endsWith('.js')) totals.js += gzipSync(await readFile(path)).length
      if (entry.name.endsWith('.css')) totals.css += gzipSync(await readFile(path)).length
    }
  }
}

await inspect(assets)
assert(totals.js > 0 && totals.css > 0, 'Build the desktop frontend before checking budgets')
for (const [key, bytes] of Object.entries(totals)) {
  console.log(`${key}: ${bytes} bytes / ${limits[key]} byte budget${key === 'all' ? ' (raw assets)' : ' (gzip)'}`)
  assert(bytes <= limits[key], `${key} exceeds its resource budget; investigate before raising the limit`)
}
const config = JSON.parse(await readFile(join(root, 'apps/desktop/src-tauri/tauri.conf.json'), 'utf8'))
assert.equal(config.bundle?.windows?.webviewInstallMode?.type, 'offlineInstaller',
  'Windows distribution must include WebView2 provisioning for offline installation')
console.log('PASS: frontend payload budgets and Windows offline runtime configuration')
