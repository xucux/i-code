import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const outDir = path.resolve(__dirname, '../src-tauri/icons')
const svgPath = path.resolve(__dirname, '../src-tauri/icon-source.svg')
const pngPath = path.resolve(__dirname, '../src-tauri/icon-source.png')

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024">
  <rect width="1024" height="1024" rx="240" fill="#1e293b"/>
  <circle cx="512" cy="320" r="100" fill="#f97316"/>
  <rect x="462" y="480" width="100" height="320" rx="20" fill="#f97316"/>
</svg>`

async function generateIcons() {
  fs.mkdirSync(path.dirname(svgPath), { recursive: true })
  fs.writeFileSync(svgPath, svg)

  const { default: sharp } = await import('sharp')
  await sharp(svgPath).png().toFile(pngPath)
  console.log(`Rendered SVG to ${pngPath}`)

  fs.mkdirSync(outDir, { recursive: true })
  await sharp(pngPath).resize(32, 32).png().toFile(path.join(outDir, '32x32.png'))
  await sharp(pngPath).resize(128, 128).png().toFile(path.join(outDir, '128x128.png'))
  await sharp(pngPath).resize(256, 256).png().toFile(path.join(outDir, '128x128@2x.png'))

  // Use Tauri CLI to generate proper ICO/ICNS if available; otherwise fall back to PNG copies.
  try {
    const { execSync } = await import('node:child_process')
    execSync(`pnpm tauri icon "${pngPath}" --output "${outDir}"`, { stdio: 'inherit' })
    console.log('Generated icons via tauri icon')
  } catch {
    fs.copyFileSync(path.join(outDir, '128x128@2x.png'), path.join(outDir, 'icon.ico'))
    fs.copyFileSync(path.join(outDir, '128x128@2x.png'), path.join(outDir, 'icon.icns'))
    console.error('Tauri icon generation failed; using PNG fallbacks')
  }
}

generateIcons().catch((err) => {
  console.error(err)
  process.exit(1)
})
