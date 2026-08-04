-- V005: providers 表新增 remark 列（供应商备注）
-- 前端「基础」页签展示/编辑；从内置预设创建时可自动填充 builtin 预设的 remark。
ALTER TABLE providers ADD COLUMN remark TEXT;
