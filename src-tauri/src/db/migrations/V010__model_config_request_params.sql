-- V010: model_configs 表新增请求参数设置列
--
-- 背景：部分供应商（如 SenseNova 6.8 Flash Lite）支持 n / stop / seed / stream_options.include_usage 等请求参数。
-- 先做配置持久化（设置），网关注入请求体待后续迭代。
--
-- 新增列：
--   n              INTEGER  回复数量（范围 1–7）
--   stop_json      TEXT     停止序列 JSON（字符串或字符串数组）
--   seed           INTEGER  随机种子（Beta，范围 [0,9999999]）
--   include_usage  INTEGER  流式响应是否返回 usage（stream_options.include_usage）

ALTER TABLE model_configs ADD COLUMN n INTEGER;
ALTER TABLE model_configs ADD COLUMN stop_json TEXT;
ALTER TABLE model_configs ADD COLUMN seed INTEGER;
ALTER TABLE model_configs ADD COLUMN include_usage INTEGER;
