-- V010: 媒体生成（文生图 / 文生视频）迭代
-- 1) providers 表新增 is_media_generation 列：标识「视觉生成」供应商，
--    该类供应商不进入原网关转发逻辑与虚拟供应商逻辑，其模型不进入 /v1/models。
--    运行时隔离判定以 MEDIA_GENERATION_FAMILY 协议族常量为准，本列用于展示与预设填充。
-- 2) 新增 media_generations 表：图像生成历史（prompt / 参数 / 本地产物路径 / 状态）。
--    产物在生成成功后立即下载到应用数据目录（供应商 URL 存在过期机制，如 SenseNova 固定 1 小时）。
-- 3) 新增 media_video_tasks 表：视频生成任务状态机
--    （submitted → running → succeeded / failed，提交 → 轮询 → 下载产物）。
-- 本迭代的所有 schema 变更合并为一个迁移文件。

-- 1. 供应商视觉生成标识
ALTER TABLE providers ADD COLUMN is_media_generation INTEGER NOT NULL DEFAULT 0;

-- 2. 图像生成历史表
CREATE TABLE IF NOT EXISTS media_generations (
  id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  provider_slug TEXT NOT NULL,
  model_id TEXT NOT NULL,
  prompt TEXT NOT NULL,
  params_json TEXT,
  status TEXT NOT NULL DEFAULT 'succeeded',
  asset_paths_json TEXT,
  source_urls_json TEXT,
  error_message TEXT,
  duration_ms INTEGER,
  created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_media_generations_created_at
  ON media_generations (created_at DESC);

-- 3. 视频生成任务表
CREATE TABLE IF NOT EXISTS media_video_tasks (
  id TEXT PRIMARY KEY,
  provider_id TEXT NOT NULL,
  provider_slug TEXT NOT NULL,
  model_id TEXT NOT NULL,
  prompt TEXT NOT NULL,
  params_json TEXT,
  status TEXT NOT NULL DEFAULT 'submitted',
  remote_task_id TEXT,
  asset_paths_json TEXT,
  source_urls_json TEXT,
  error_message TEXT,
  submitted_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  finished_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_media_video_tasks_submitted_at
  ON media_video_tasks (submitted_at DESC);
