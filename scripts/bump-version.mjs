import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import readline from 'node:readline'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const rootDir = path.resolve(__dirname, '..')

const paths = {
  packageJson: path.join(rootDir, 'package.json'),
  tauriConf: path.join(rootDir, 'src-tauri', 'tauri.conf.json'),
  cargoToml: path.join(rootDir, 'src-tauri', 'Cargo.toml'),
  versionJson: path.join(rootDir, 'version.json'),
  titleBar: path.join(rootDir, 'src', 'components', 'ui', 'title-bar.tsx'),
  settingsPage: path.join(rootDir, 'src', 'routes', 'settings.tsx'),
  agentsMd: path.join(rootDir, 'AGENTS.md'),
}

/**
 * 读取 JSON 文件，保留原始缩进与换行风格。
 * @param {string} filePath
 * @returns {{ data: any, indent: string, trailing: string }}
 */
function readJson(filePath) {
  const raw = fs.readFileSync(filePath, 'utf-8')
  const indentMatch = raw.match(/^(\s+)/m)
  const indent = indentMatch ? indentMatch[1] : '  '
  const trailing = raw.endsWith('\n') ? '\n' : ''
  return { data: JSON.parse(raw), indent, trailing }
}

/**
 * 写入 JSON 文件，尽量保留原始格式。
 * @param {string} filePath
 * @param {any} data
 * @param {string} indent
 * @param {string} trailing
 */
function writeJson(filePath, data, indent, trailing) {
  fs.writeFileSync(filePath, JSON.stringify(data, null, indent) + trailing)
}

/**
 * 读取 Cargo.toml 并解析简单键值（仅支持 package.version 这种顶层键）。
 * @param {string} filePath
 * @returns {{ raw: string, version: string }}
 */
function readCargoToml(filePath) {
  const raw = fs.readFileSync(filePath, 'utf-8')
  const match = raw.match(/^version\s*=\s*"([^"]+)"/m)
  return { raw, version: match ? match[1] : '0.0.0' }
}

/**
 * 更新 Cargo.toml 中的 version 字段。
 * @param {string} filePath
 * @param {string} newVersion
 */
function writeCargoToml(filePath, newVersion) {
  const raw = fs.readFileSync(filePath, 'utf-8')
  const updated = raw.replace(/^(version\s*=\s*")([^"]+)(")/m, `$1${newVersion}$3`)
  fs.writeFileSync(filePath, updated)
}

/**
 * 同步散落在 TSX / Markdown 中的硬编码版本号，避免版本显示不一致。
 * @param {string} oldVersion
 * @param {string} newVersion
 */
function syncStaticVersionRefs(oldVersion, newVersion) {
  const updates = [
    {
      file: paths.titleBar,
      patterns: [
        {
          regex: new RegExp(`(const APP_VERSION = ')${oldVersion}(')`, 'g'),
          replacement: `$1${newVersion}$2`,
        },
      ],
    },
    {
      file: paths.settingsPage,
      patterns: [
        {
          regex: new RegExp(`(<span className="text-xs text-muted-foreground tabular-nums">)${oldVersion}(</span>)`, 'g'),
          replacement: `$1${newVersion}$2`,
        },
      ],
    },
    {
      file: paths.agentsMd,
      patterns: [
        {
          regex: new RegExp(`(版本：\`)${oldVersion}(\`)`, 'g'),
          replacement: `$1${newVersion}$2`,
        },
      ],
    },
  ]

  for (const { file, patterns } of updates) {
    if (!fs.existsSync(file)) continue
    let raw = fs.readFileSync(file, 'utf-8')
    let changed = false
    for (const { regex, replacement } of patterns) {
      if (regex.test(raw)) {
        raw = raw.replace(regex, replacement)
        changed = true
      }
    }
    if (changed) {
      fs.writeFileSync(file, raw)
      console.log(`✓ ${path.relative(rootDir, file)}: ${oldVersion} → ${newVersion}`)
    }
  }
}

/**
 * 解析命令行参数。
 * @param {string[]} argv
 * @returns {{ version?: string, notes?: string, date?: string, noHistory: boolean, help: boolean }}
 */
function parseArgs(argv) {
  const result = { noHistory: false, help: false }
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === '--help' || arg === '-h') {
      result.help = true
    } else if (arg === '--notes' || arg === '-n') {
      result.notes = argv[++i]
    } else if (arg === '--date' || arg === '-d') {
      result.date = argv[++i]
    } else if (arg === '--no-history') {
      result.noHistory = true
    } else if (!arg.startsWith('-') && !result.version) {
      result.version = arg
    }
  }
  return result
}

/**
 * 在终端提问并等待用户输入。
 * @param {readline.Interface} rl
 * @param {string} question
 * @returns {Promise<string>}
 */
function ask(rl, question) {
  return new Promise((resolve) => {
    rl.question(question, (answer) => resolve(answer.trim()))
  })
}

/**
 * 多行输入，用户输入空行结束。
 * @param {readline.Interface} rl
 * @param {string} prompt
 * @returns {Promise<string>}
 */
function askMultiline(rl, prompt) {
  return new Promise((resolve) => {
    console.log(prompt)
    console.log('（连续输入空行结束；每行会自动转为列表项，也可直接输入 HTML）')
    const lines = []
    const promptLine = () => {
      rl.question('> ', (line) => {
        if (line === '' && lines.length > 0 && lines[lines.length - 1] === '') {
          lines.pop()
          const raw = lines.join('\n').trim()
          resolve(raw)
          return
        }
        lines.push(line)
        promptLine()
      })
    }
    promptLine()
  })
}

/**
 * 将普通文本行转换为富文本 HTML；如果输入本身像 HTML，则原样返回。
 * @param {string} raw
 * @returns {string}
 */
function toRichNotes(raw) {
  const trimmed = raw.trim()
  if (!trimmed) return ''
  // 若用户已提供 HTML，直接保留
  if (/^<[a-z][\s\S]*>$/i.test(trimmed)) {
    return trimmed
  }
  // 否则按行拆分为无序列表
  const items = trimmed
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
  if (items.length === 0) return ''
  return `<ul>\n${items.map((item) => `  <li>${item}</li>`).join('\n')}\n</ul>`
}

function printUsage() {
  console.log(`用法：
  node scripts/bump-version.mjs [version] [选项]

示例：
  node scripts/bump-version.mjs 0.3.0
  node scripts/bump-version.mjs 0.3.0 --notes "修复若干 Bug"
  node scripts/bump-version.mjs 0.3.0 --notes "<p>重要更新</p>" --date 2026-07-23
  node scripts/bump-version.mjs --no-history 0.3.0
  pnpm version:bump

选项：
  --notes, -n     更新内容（支持 HTML 富文本；纯文本会自动转为 <ul><li>...）
  --date, -d      发布日期，默认今天
  --no-history    仅更新版本号，不追加历史记录
  --help, -h      显示帮助`)
}

async function main() {
  const args = parseArgs(process.argv.slice(2))

  if (args.help) {
    printUsage()
    return
  }

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  })

  try {
    // 读取当前版本
    const pkg = readJson(paths.packageJson)
    const currentVersion = pkg.data.version || '0.0.0'

    // 确定目标版本
    let targetVersion = args.version
    if (!targetVersion) {
      targetVersion = await ask(rl, `当前版本：${currentVersion}\n请输入新版本号（直接回车保持不变）：`)
      if (!targetVersion) {
        console.log('未提供新版本号，操作已取消。')
        return
      }
    }

    // 简单语义化版本校验
    if (!/^\d+\.\d+\.\d+(-[a-zA-Z0-9.]+)?(\+[a-zA-Z0-9.]+)?$/.test(targetVersion)) {
      console.error(`版本号格式不合法：${targetVersion}`)
      process.exit(1)
    }

    // 读取或创建 version.json
    let versionInfo = { version: currentVersion, history: [] }
    if (fs.existsSync(paths.versionJson)) {
      try {
        versionInfo = JSON.parse(fs.readFileSync(paths.versionJson, 'utf-8'))
      } catch (err) {
        console.warn('读取 version.json 失败，将重新创建。', err.message)
      }
    }

    // 收集发布信息
    let releaseDate = args.date || new Date().toISOString().slice(0, 10)
    let notes = args.notes || ''

    if (!args.noHistory) {
      if (!args.date) {
        const inputDate = await ask(rl, `发布日期（默认 ${releaseDate}）：`)
        if (inputDate) releaseDate = inputDate
      }
      if (!args.notes) {
        const inputNotes = await askMultiline(rl, `请输入 ${targetVersion} 的更新内容：`)
        notes = toRichNotes(inputNotes)
      } else {
        notes = toRichNotes(notes)
      }
    }

    // 更新 package.json
    pkg.data.version = targetVersion
    writeJson(paths.packageJson, pkg.data, pkg.indent, pkg.trailing)
    console.log(`✓ package.json: ${currentVersion} → ${targetVersion}`)

    // 更新 tauri.conf.json
    const tauri = readJson(paths.tauriConf)
    tauri.data.version = targetVersion
    writeJson(paths.tauriConf, tauri.data, tauri.indent, tauri.trailing)
    console.log(`✓ src-tauri/tauri.conf.json: ${currentVersion} → ${targetVersion}`)

    // 更新 Cargo.toml
    writeCargoToml(paths.cargoToml, targetVersion)
    console.log(`✓ src-tauri/Cargo.toml: ${currentVersion} → ${targetVersion}`)

    // 同步散落在 UI 与文档中的硬编码版本号
    syncStaticVersionRefs(currentVersion, targetVersion)

    // 更新 version.json
    versionInfo.version = targetVersion
    if (!args.noHistory) {
      // 若已有同版本记录则替换，否则追加到头部
      const existingIndex = versionInfo.history.findIndex((h) => h.version === targetVersion)
      const entry = { version: targetVersion, date: releaseDate, notes }
      if (existingIndex >= 0) {
        versionInfo.history[existingIndex] = entry
        console.log(`✓ version.json: 已替换 ${targetVersion} 的历史记录`)
      } else {
        versionInfo.history.unshift(entry)
        console.log(`✓ version.json: 已追加 ${targetVersion} 的历史记录`)
      }
    } else {
      console.log(`✓ version.json: 版本号已更新为 ${targetVersion}（未写入历史记录）`)
    }
    writeJson(paths.versionJson, versionInfo, '  ', '\n')

    console.log('\n版本更新完成。')
  } finally {
    rl.close()
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
