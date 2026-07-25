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
 * @returns {{ version?: string, help: boolean }}
 */
function parseArgs(argv) {
  const result = { help: false }
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i]
    if (arg === '--help' || arg === '-h') {
      result.help = true
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

function printUsage() {
  console.log(`用法：
  node scripts/bump-version.mjs [version]

示例：
  node scripts/bump-version.mjs 0.3.0
  pnpm version:bump

选项：
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

    // 更新到目标版本
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

    console.log('\n版本更新完成。')
  } finally {
    rl.close()
  }
}

main().catch((err) => {
  console.error(err)
  process.exit(1)
})
