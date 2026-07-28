-- V002: 为 providers 表新增 script_variables_json 列（供应商扩展模板变量）
-- 存储结构见 docs/proposals/provider-script-variables.md §2.2
ALTER TABLE providers ADD COLUMN script_variables_json TEXT;
