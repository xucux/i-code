#!/usr/bin/env node

/**
 * generate-latest-json.mjs — 生成 latest.json（Tauri 自动更新用）
 *
 * 功能：
 *   1. 从 CHANGELOG.md 提取指定版本的 notes（与 release body 同源）
 *   2. 扫描 assets 目录，组装各平台安装包 URL 与签名
 *   3. 输出 latest.json 或仅输出 notes（用于 release body）
 *
 * 用法：
 *   # 生成完整 latest.json
 *   node .github/scripts/generate-latest-json.mjs \
 *     --version=0.0.1 \
 *     --tag=v0.0.1 \
 *     --repo=user/repo \
 *     --assets-dir=./dl
 *
 *   # 仅输出 notes（用于 release body）
 *   node .github/scripts/generate-latest-json.mjs \
 *     --version=0.0.1 \
 *     --notes-only
 *
 *   # Beta 版本
 *   node .github/scripts/generate-latest-json.mjs \
 *     --version=0.0.1-beta1 \
 *     --tag=v0.0.1-beta1 \
 *     --repo=user/repo \
 *     --assets-dir=./dl \
 *     --beta
 *
 * 参数：
 *   --version      版本号（不含 v 前缀）
 *   --tag          完整 Git tag（含 v 前缀）
 *   --repo         GitHub 仓库，格式 owner/repo
 *   --assets-dir   包含已下载安装包与 .sig 文件的目录
 *   --changelog    CHANGELOG.md 路径，默认 ./CHANGELOG.md
 *   --beta         标记为预览版，notes 会添加「预览版」说明
 *   --notes-only   仅输出 notes 内容到 stdout
 *   --output       latest.json 输出路径，默认 ./latest.json
 */

import { readFileSync, writeFileSync, readdirSync, existsSync } from 'fs';
import { resolve } from 'path';

/**
 * 从版本号中自动判断是否为 beta，并提取正式版本号
 *
 * 规则：
 *   - 自动剔除开头的 `v` 前缀
 *   - 版本号中包含 `-beta` 则视为预览版，正式版本号为 `-beta` 前的部分
 * 示例：
 *   `v0.0.1-beta20260725`  → { isBeta: true,  baseVersion: '0.0.1' }
 *   `0.0.1-beta20260725`   → { isBeta: true,  baseVersion: '0.0.1' }
 *   `v0.0.1`               → { isBeta: false, baseVersion: '0.0.1' }
 *   `0.0.1`                → { isBeta: false, baseVersion: '0.0.1' }
 *
 * @param {string} version  原始版本号（可能带 v 前缀）
 * @returns {{ isBeta: boolean, baseVersion: string }}
 */
function parseVersion(version) {
  // 剔除开头的 v 前缀
  const stripped = version.replace(/^v/i, '');
  console.warn(`[parseVersion] input: "${version}", stripped: "${stripped}"`);

  const betaMatch = stripped.match(/^(.+?)-beta/i);
  if (betaMatch) {
    return { isBeta: true, baseVersion: betaMatch[1] };
  }
  return { isBeta: false, baseVersion: stripped };
}

function parseArgs() {
  const args = {};
  for (const arg of process.argv.slice(2)) {
    if (arg.startsWith('--')) {
      const eqIdx = arg.indexOf('=');
      if (eqIdx !== -1) {
        args[arg.slice(2, eqIdx)] = arg.slice(eqIdx + 1);
      } else {
        args[arg.slice(2)] = true;
      }
    }
  }
  return args;
}

/**
 * 从 CHANGELOG.md 提取指定版本的 notes 内容
 *
 * @param {string} changelogPath  CHANGELOG.md 绝对路径
 * @param {string} version        版本号（不含 v 前缀）
 * @param {boolean} isBeta       是否为预览版
 * @returns {string} 提取的 notes 文本
 */
function extractNotes(changelogPath, version, isBeta) {
  console.warn(`[extractNotes] changelog: ${changelogPath}`);
  console.warn(`[extractNotes] lookup version: "${version}", isBeta: ${isBeta}`);

  const content = readFileSync(changelogPath, 'utf-8');
  const lines = content.split('\n');
  let found = false;
  const notes = [];
  // Beta 版本在标题前添加预览版说明
  if (isBeta) {
    notes.push('> 🚨该版本为预览版（Beta），可能包含未完善的功能🚧。');
    notes.push('');
  }

  for (const line of lines) {
    // 遇到下一个 ## 标题时停止（已找到目标节后）
    if (found && line.startsWith('## ')) break;

    // 匹配目标版本行：## [version] ...
    if (!found && line.startsWith('## ') && line.includes(`[${version}]`)) {
      found = true;
      console.warn(`[extractNotes] found section: "${line.trim()}"`);
      
    }

    if (found) {
      notes.push(line);
    }
  }

  const result = notes.join('\n').trim();
  if (!result) {
    console.error(`[extractNotes] ⚠ No section found for version "${version}" in CHANGELOG.md`);
  } else {
    console.warn(`[extractNotes] extracted ${result.split('\n').length} lines of notes`);
  }
  return result;
}

/**
 * 判断安装包所属的基础平台键名
 *
 * 匹配规则（与 Tauri 产物命名对齐）：
 *   - *aarch64|arm64*.app.tar.gz  → darwin-aarch64
 *   - *x64|x86_64*.app.tar.gz     → darwin-x86_64
 *   - *aarch64|arm64*.dmg         → darwin-aarch64
 *   - *x64|x86_64*.dmg            → darwin-x86_64
 *   - *.AppImage|*.appimage       → linux-x86_64
 *   - *-setup.exe                 → windows-x86_64
 *   - *.msi                       → windows-x86_64
 *   - *.deb / *.rpm               → linux-x86_64（默认）
 *
 * @param {string} file  文件名
 * @returns {string|null} 基础平台键名，无法匹配时返回 null
 */
function getBasePlatform(file) {
  if (/aarch64|arm64/.test(file) && file.endsWith('.app.tar.gz')) return 'darwin-aarch64';
  if (/x64|x86_64/.test(file) && file.endsWith('.app.tar.gz'))    return 'darwin-x86_64';
  if (/aarch64|arm64/.test(file) && /\.dmg$/i.test(file))          return 'darwin-aarch64';
  if (/x64|x86_64/.test(file) && /\.dmg$/i.test(file))            return 'darwin-x86_64';
  if (/\.AppImage$/i.test(file) || /\.appimage$/i.test(file))      return 'linux-x86_64';
  if (file.endsWith('-setup.exe'))                                  return 'windows-x86_64';
  if (file.endsWith('.msi'))                                        return 'windows-x86_64';
  if (file.endsWith('.deb'))                                        return 'linux-x86_64';
  if (file.endsWith('.rpm'))                                        return 'linux-x86_64';
  return null;
}

/**
 * 获取安装包格式后缀，用于生成带格式标识的平台键名
 *
 * 规则：
 *   - .app.tar.gz → app
 *   - .AppImage   → appimage
 *   - .deb        → deb
 *   - .rpm        → rpm
 *   - .dmg        → dmg
 *   - .msi        → msi
 *   - -setup.exe  → nsis
 *
 * @param {string} file  文件名
 * @returns {string|null} 格式后缀，无法识别时返回 null
 */
function getInstallerSuffix(file) {
  if (file.endsWith('.app.tar.gz')) return 'app';
  if (/\.AppImage$/i.test(file))    return 'appimage';
  if (file.endsWith('.deb'))        return 'deb';
  if (file.endsWith('.rpm'))        return 'rpm';
  if (/\.dmg$/i.test(file))        return 'dmg';
  if (file.endsWith('.msi'))        return 'msi';
  if (file.endsWith('-setup.exe'))  return 'nsis';
  return null;
}

/**
 * 扫描 assets 目录，按平台归类安装包 URL 与签名
 *
 * 生成两类平台条目（与 Tauri updater 社区惯例对齐）：
 *   1. 基础平台键 — 如 `linux-x86_64`、`darwin-aarch64`、`windows-x86_64`
 *      （每种平台取一种安装包：AppImage / .app.tar.gz / -setup.exe）
 *   2. 格式后缀键 — 如 `linux-x86_64-deb`、`linux-x86_64-rpm`、`windows-x86_64-msi`
 *      （每种安装包都有独立条目，供客户端选择特定格式）
 *
 * 注意：如果同目录下存在对应的 .sig 文件，签名会被读取并写入 platforms；
 *       若不存在 .sig 文件，signature 为空字符串，不影响平台匹配。
 *
 * @param {string|null} assetsDir  assets 目录绝对路径
 * @param {string} baseUrl         下载基础 URL
 * @returns {Record<string, {signature: string, url: string}>}
 */
function scanAssets(assetsDir, baseUrl) {
  const platforms = {};

  if (!assetsDir || !existsSync(assetsDir)) {
    console.error(`[scanAssets] ⚠ assets directory not found: ${assetsDir}`);
    return platforms;
  }

  const allFiles = readdirSync(assetsDir);
  console.warn(`[scanAssets] scanning ${allFiles.length} files in: ${assetsDir}`);

  // 第一步：构建签名映射 { baseName → signature }
  const sigMap = {};
  for (const file of allFiles) {
    if (file.endsWith('.sig')) {
      const baseName = file.slice(0, -4);
      const sigPath = resolve(assetsDir, file);
      sigMap[baseName] = readFileSync(sigPath, 'utf-8').trim();
      console.warn(`[scanAssets]   found signature: ${file} → ${baseName}`);
    }
  }

  // 第二步：遍历所有非 .sig 文件，匹配安装包并归类
  const installers = allFiles.filter(f => !f.endsWith('.sig'));
  console.warn(`[scanAssets] matching ${installers.length} installer files...`);

  // 基础平台条目优先级（越小越优先）：
  //   Linux: AppImage(0) > deb(1) > rpm(2)
  //   macOS: .app.tar.gz(0) > .dmg(1)
  //   Windows: -setup.exe(0) > .msi(1)
  const BASE_PRIORITY = { appimage: 0, app: 0, nsis: 0, deb: 1, dmg: 1, msi: 1, rpm: 2 };
  // 记录每个基础平台当前最高优先级条目
  const basePriority = {};

  for (const file of installers) {
    const basePlatform = getBasePlatform(file);
    const suffix = getInstallerSuffix(file);
    const url = `${baseUrl}/${encodeURIComponent(file)}`;
    const signature = sigMap[file] || '';
    const sigInfo = signature ? `(sig: ${signature.slice(0, 20)}…)` : '(no sig)';

    if (!basePlatform || !suffix) {
      console.warn(`[scanAssets]   - skipped: ${file}`);
      continue;
    }

    // 带格式后缀的条目（如 linux-x86_64-deb、windows-x86_64-msi）
    const suffixKey = `${basePlatform}-${suffix}`;
    platforms[suffixKey] = { signature, url };
    console.warn(`[scanAssets]   ✓ ${suffixKey} ← ${file} ${sigInfo}`);

    // 基础平台条目（按优先级选择，不依赖文件系统顺序）
    const priority = BASE_PRIORITY[suffix] ?? 99;
    if (basePriority[basePlatform] === undefined || priority < basePriority[basePlatform]) {
      platforms[basePlatform] = { signature, url };
      basePriority[basePlatform] = priority;
      console.warn(`[scanAssets]   ✓ ${basePlatform}  ← ${file} ${sigInfo} (base, priority=${priority})`);
    }
  }

  console.warn(`[scanAssets] done. platforms: ${Object.keys(platforms).join(', ') || '(none)'}`);
  return platforms;
}

function main() {
  const args = parseArgs();

  if (!args.version) {
    console.error('Usage: node .github/scripts/generate-latest-json.mjs --version=0.0.1 [options]');
    console.error('');
    console.error('Required:');
    console.error('  --version=0.0.1         版本号（不含 v 前缀）');
    console.error('');
    console.error('Options:');
    console.error('  --tag=v0.0.1            完整 Git tag（默认 v{version}）');
    console.error('  --repo=owner/repo       GitHub 仓库');
    console.error('  --assets-dir=./dl       已下载安装包与 .sig 的目录');
    console.error('  --changelog=CHANGELOG.md CHANGELOG.md 路径');
    console.error('  --beta                  标记为预览版');
    console.error('  --notes-only            仅输出 notes 到 stdout');
    console.error('  --output=latest.json    latest.json 输出路径');
    process.exit(1);
  }

  // 自动解析版本号：检测 beta 并提取正式版本号，同时剔除可能的 v 前缀
  const { isBeta: autoBeta, baseVersion } = parseVersion(args.version);
  // --beta 显式传入时也视为 beta（兼容手动指定）
  const isBeta = autoBeta || !!args.beta;
  // 无 v 前缀的完整版本号（如 0.0.1-beta20260725）
  const strippedVersion = args.version.replace(/^v/i, '');

  console.warn(`[main] raw version:      "${args.version}"`);
  console.warn(`[main] stripped version: "${strippedVersion}"`);
  console.warn(`[main] base version:     "${baseVersion}"`);
  console.warn(`[main] is beta:          ${isBeta}`);
  console.warn(`[main] beta source:      ${autoBeta ? 'auto-detected from version' : args.beta ? '--beta flag' : 'N/A'}`);

  const changelogPath = resolve(process.cwd(), args.changelog || 'CHANGELOG.md');
  console.warn(`[main] changelog path: ${changelogPath}`);

  // 1. 用正式版本号（不含 -beta 后缀）查询 changelog
  const notes = extractNotes(changelogPath, baseVersion, isBeta);

  // 2. --notes-only 模式：仅输出 notes 到 stdout，用于 release body
  if (args['notes-only']) {
    console.warn(`[main] mode: --notes-only, writing notes to stdout (${notes.length} chars)`);
    // 确保输出以换行结尾，适配 GitHub Actions body_path
    process.stdout.write(notes + '\n');
    return;
  }

  // 3. 完整 latest.json 模式
  const tag = args.tag || `v${args.version}`;
  const repo = args.repo || '';
  const baseUrl = `https://github.com/${repo}/releases/download/${tag}`;
  const assetsDir = args['assets-dir'] ? resolve(process.cwd(), args['assets-dir']) : null;

  console.warn(`[main] tag:            ${tag}`);
  console.warn(`[main] repo:           ${repo}`);
  console.warn(`[main] assets dir:     ${assetsDir || '(not provided)'}`);

  const platforms = scanAssets(assetsDir, baseUrl);
  console.warn(`[main] platforms found: ${Object.keys(platforms).join(', ') || '(none)'}`);

  const pubDate = new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');

  const latestJson = {
    version: strippedVersion,
    notes,
    pub_date: pubDate,
    platforms,
  };

  const output = JSON.stringify(latestJson, null, 2);

  const outputPath = resolve(process.cwd(), args.output || 'latest.json');
  writeFileSync(outputPath, output, 'utf-8');

  console.warn(`[main] ✅ latest.json generated → ${outputPath}`);
  console.warn(`[main] version: ${strippedVersion}, platforms: ${Object.keys(platforms).join(', ') || '(none)'}`);
}

main();