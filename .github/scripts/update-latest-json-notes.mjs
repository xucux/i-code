#!/usr/bin/env node

/**
 * update-latest-json-notes.mjs — 仅更新 latest.json 的 notes 字段（Tauri 自动更新用）
 *
 * 功能：
 *   1. 读取已存在于发布资产中的 latest.json（必须先下载到本地）
 *   2. 仅更新其中的 `notes` 字段，其余字段（version / pub_date / platforms 等）原样保留
 *   3. 写回 latest.json，由调用方（GH Actions workflow）上传覆盖到指定的 release tag
 *
 * 用法：
 *   # 手动指定 notes 内容
 *   node .github/scripts/update-latest-json-notes.mjs \
 *     --repo=user/repo \
 *     --tag=v0.1.7 \
 *     --notes="修复了若干 bug"
 *
 *   # 未传 --notes 时，从 CHANGELOG.md 自动提取该版本对应章节作为 notes（与 release body 同源）
 *   node .github/scripts/update-latest-json-notes.mjs \
 *     --repo=user/repo \
 *     --tag=v0.1.7
 *
 * 参数：
 *   --tag        完整 Git tag（含 v 前缀），用于定位最新版本号
 *   --repo       GitHub 仓库，格式 owner/repo（仅用于日志，不参与改字段）
 *   --notes      手动指定的 notes 内容；若省略，则从 CHANGELOG.md 提取
 *   --changelog  CHANGELOG.md 路径，默认 ./CHANGELOG.md
 *   --output     latest.json 路径，默认 ./latest.json
 *
 * 注意：本脚本只改 notes 字段，绝不重写 version / pub_date / platforms，
 *       因此必须基于已下载到的现有 latest.json 进行最小修改。
 */

import { readFileSync, writeFileSync, existsSync } from 'fs';
import { resolve } from 'path';

/**
 * 从版本号中自动判断是否为 beta，并提取正式版本号（与 generate-latest-json.mjs 保持一致）
 *
 * @param {string} version 原始版本号（可能带 v 前缀）
 * @returns {{ isBeta: boolean, baseVersion: string }}
 */
function parseVersion(version) {
  const stripped = version.replace(/^v/i, '');
  const betaMatch = stripped.match(/^(.+?)-beta/i);
  if (betaMatch) {
    return { isBeta: true, baseVersion: betaMatch[1] };
  }
  return { isBeta: false, baseVersion: stripped };
}

/**
 * 从 CHANGELOG.md 提取指定版本的 notes 内容（与 generate-latest-json.mjs 同源）
 *
 * @param {string} changelogPath CHANGELOG.md 路径
 * @param {string} version       正式版本号（不含 -beta 后缀）
 * @param {boolean} isBeta       是否为预览版
 * @returns {string} 提取的 notes 文本
 */
function extractNotes(changelogPath, version, isBeta) {
  const content = readFileSync(changelogPath, 'utf-8');
  const lines = content.split('\n');
  let found = false;
  const notes = [];
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
    }

    if (found) {
      notes.push(line);
    }
  }

  const result = notes.join('\n').trim();
  if (!result) {
    console.error(`[extractNotes] ⚠ No section found for version "${version}" in CHANGELOG.md`);
  }
  return result;
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

function main() {
  const args = parseArgs();

  const outputPath = resolve(process.cwd(), args.output || 'latest.json');

  if (!existsSync(outputPath)) {
    console.error(`[main] ✗ latest.json not found at: ${outputPath}`);
    console.error('       请先在 workflow 中通过 `gh release download` 把最新 latest.json 下载到本地。');
    process.exit(1);
  }

  // 1. 读取现有 latest.json
  const existing = JSON.parse(readFileSync(outputPath, 'utf-8'));
  console.warn(`[main] loaded existing latest.json: version=${existing.version}, keys=${Object.keys(existing).join(',')}`);

  // 读取 --notes 内容（多行文本经 workflow 传参）
  const manualNotes = args.notes || '';

  // 2. 计算新的 notes：显式传入优先，否则从 CHANGELOG.md 提取
  let newNotes;
  if (manualNotes) {
    newNotes = manualNotes.trim();
    console.warn(`[main] using manually provided notes (${newNotes.length} chars)`);
  } else {
    const changelogPath = resolve(process.cwd(), args.changelog || 'CHANGELOG.md');
    const { isBeta, baseVersion } = parseVersion(existing.version);
    console.warn(`[main] extracting notes from ${changelogPath} for version "${baseVersion}" (isBeta=${isBeta})`);
    newNotes = extractNotes(changelogPath, baseVersion, isBeta);
    if (!newNotes) {
      console.error('[main] ✗ no notes provided via --notes and CHANGELOG.md had no matching section.');
      process.exit(1);
    }
  }

  // 3. 仅更新 notes 字段，其余字段原样保留
  const before = JSON.stringify(existing);
  existing.notes = newNotes;

  const output = JSON.stringify(existing, null, 2);
  writeFileSync(outputPath, output, 'utf-8');

  const after = JSON.stringify(existing);
  const changed = before !== after;
  console.warn(`[main] notes updated: ${changed}`);
  console.warn(`[main] wrote ${outputPath}`);
  console.log(`[main] ${output}`);
}

main();
