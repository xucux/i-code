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
  const content = readFileSync(changelogPath, 'utf-8');
  const lines = content.split('\n');
  let found = false;
  const notes = [];

  for (const line of lines) {
    // 遇到下一个 ## 标题时停止（已找到目标节后）
    if (found && line.startsWith('## ')) break;

    // 匹配目标版本行：## [version] ...
    if (!found && line.startsWith('## ') && line.includes(`[${version}]`)) {
      found = true;
      // Beta 版本在标题前添加预览版说明
      if (isBeta) {
        notes.push('> 该版本为预览版（Beta），可能包含未完善的功能。');
        notes.push('');
      }
    }

    if (found) {
      notes.push(line);
    }
  }

  const result = notes.join('\n').trim();
  if (!result) {
    console.error(`Warning: No section found for version "${version}" in CHANGELOG.md`);
  }
  return result;
}

/**
 * 扫描 assets 目录，按平台归类安装包 URL 与签名
 *
 * 匹配规则（与 Tauri 产物命名对齐）：
 *   - *aarch64|arm64*.app.tar.gz  → darwin-aarch64
 *   - *x64|x86_64*.app.tar.gz     → darwin-x86_64
 *   - *.AppImage|*.appimage       → linux-x86_64
 *   - *-setup.exe                 → windows-x86_64（优先）
 *   - *.msi                       → windows-x86_64（后备）
 *
 * @param {string|null} assetsDir  assets 目录绝对路径
 * @param {string} baseUrl         下载基础 URL
 * @returns {Record<string, {signature: string, url: string}>}
 */
function scanAssets(assetsDir, baseUrl) {
  const platforms = {};

  if (!assetsDir || !existsSync(assetsDir)) {
    return platforms;
  }

  let winUrl = '';
  let winSig = '';

  const files = readdirSync(assetsDir);

  for (const file of files) {
    if (!file.endsWith('.sig')) continue;

    const baseName = file.slice(0, -4); // 去掉 .sig
    const sigPath = resolve(assetsDir, file);
    const sigContent = readFileSync(sigPath, 'utf-8').trim();
    const url = `${baseUrl}/${encodeURIComponent(baseName)}`;

    if (/aarch64|arm64/.test(baseName) && baseName.endsWith('.app.tar.gz')) {
      platforms['darwin-aarch64'] = { signature: sigContent, url };
    } else if (/x64|x86_64/.test(baseName) && baseName.endsWith('.app.tar.gz')) {
      platforms['darwin-x86_64'] = { signature: sigContent, url };
    } else if (/\.AppImage$/i.test(baseName) || /\.appimage$/i.test(baseName)) {
      platforms['linux-x86_64'] = { signature: sigContent, url };
    } else if (baseName.endsWith('-setup.exe')) {
      winUrl = url;
      winSig = sigContent;
    } else if (baseName.endsWith('.msi') && !winUrl) {
      winUrl = url;
      winSig = sigContent;
    }
  }

  if (winUrl && winSig) {
    platforms['windows-x86_64'] = { signature: winSig, url: winUrl };
  }

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

  const changelogPath = resolve(process.cwd(), args.changelog || 'CHANGELOG.md');
  const isBeta = !!args.beta;

  // 1. 提取 notes
  const notes = extractNotes(changelogPath, args.version, isBeta);

  // 2. --notes-only 模式：仅输出 notes 到 stdout，用于 release body
  if (args['notes-only']) {
    // 确保输出以换行结尾，适配 GitHub Actions body_path
    process.stdout.write(notes + '\n');
    return;
  }

  // 3. 完整 latest.json 模式
  const tag = args.tag || `v${args.version}`;
  const repo = args.repo || '';
  const baseUrl = `https://github.com/${repo}/releases/download/${tag}`;
  const assetsDir = args['assets-dir'] ? resolve(process.cwd(), args['assets-dir']) : null;

  const platforms = scanAssets(assetsDir, baseUrl);

  const pubDate = new Date().toISOString().replace(/\.\d{3}Z$/, 'Z');

  const latestJson = {
    version: args.version,
    notes,
    pub_date: pubDate,
    platforms,
  };

  const output = JSON.stringify(latestJson, null, 2);

  const outputPath = resolve(process.cwd(), args.output || 'latest.json');
  writeFileSync(outputPath, output, 'utf-8');

  console.log(`latest.json generated (version: ${args.version})`);
  console.log(`Platforms: ${Object.keys(platforms).join(', ') || '(none)'}`);
}

main();