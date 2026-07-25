const fs = require('fs');
const path = require('path');

const file = path.resolve(__dirname, '..', 'src-tauri', 'src', 'modules', 'backup', 'service.rs');
let content = fs.readFileSync(file, 'utf-8');

// 保护宏定义行不被替换
const macros = ['backup_info', 'backup_warn', 'backup_error'];
const lines = content.split('\n');
const fixed = lines.map((line) => {
  let result = line;
  for (const name of macros) {
    // 跳过 macro_rules! 定义行
    if (line.includes(`macro_rules! ${name}`)) {
      continue;
    }
    // 把 backup_info( 替换为 backup_info!(
    const regex = new RegExp(`\\b${name}\\(`, 'g');
    result = result.replace(regex, `${name}!(`);
  }
  return result;
});

fs.writeFileSync(file, fixed.join('\n'), 'utf-8');
console.log('Added missing macro bangs.');
