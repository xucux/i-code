const fs = require('fs');
const path = require('path');

const file = path.resolve(__dirname, '..', 'src-tauri', 'src', 'modules', 'backup', 'service.rs');
let content = fs.readFileSync(file, 'utf-8');

// 在 secret import 后插入宏定义
const macroDef = `
/// 同时向 tauri-plugin-log 和自研内存 logger 输出日志
macro_rules! backup_info {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        log::info!("{}", msg);
        Log::info(&msg);
    }};
}
macro_rules! backup_warn {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        log::warn!("{}", msg);
        Log::warn(&msg);
    }};
}
macro_rules! backup_error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        log::error!("{}", msg);
        Log::error(&msg);
    }};
}
`;

const importAnchor = 'use crate::modules::secret::SecretServiceHandle;';
if (!content.includes('macro_rules! backup_info')) {
  content = content.replace(importAnchor, importAnchor + macroDef);
}

// 替换 log::info!/warn!/error! 为 backup_info!/backup_warn!/backup_error!
// 使用括号深度匹配
function replaceLogMacro(content, oldPrefix, newPrefix) {
  let result = '';
  let i = 0;
  while (i < content.length) {
    const idx = content.indexOf(oldPrefix, i);
    if (idx === -1) {
      result += content.slice(i);
      break;
    }
    result += content.slice(i, idx);
    let j = idx + oldPrefix.length;
    // 找到第一个 (
    while (j < content.length && content[j] !== '(') j++;
    if (j >= content.length) {
      result += content.slice(idx);
      break;
    }
    let depth = 1;
    let k = j + 1;
    while (k < content.length && depth > 0) {
      if (content[k] === '(') depth++;
      else if (content[k] === ')') depth--;
      // 跳过字符串字面量
      if (content[k] === '"' || content[k] === "'") {
        const quote = content[k];
        k++;
        while (k < content.length) {
          if (content[k] === '\\\\') {
            k += 2;
          } else if (content[k] === quote) {
            k++;
            break;
          } else {
            k++;
          }
        }
      } else {
        k++;
      }
    }
    if (depth !== 0) {
      result += content.slice(idx, k);
      i = k;
      continue;
    }
    result += newPrefix + content.slice(j, k);
    i = k;
  }
  return result;
}

content = replaceLogMacro(content, 'log::info!', 'backup_info!');
content = replaceLogMacro(content, 'log::warn!', 'backup_warn!');
content = replaceLogMacro(content, 'log::error!', 'backup_error!');

fs.writeFileSync(file, content, 'utf-8');
console.log('Replaced backup service logs with dual-channel macros.');
