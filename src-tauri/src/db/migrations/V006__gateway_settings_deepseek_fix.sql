-- V006: gateway_settings 表新增 DeepSeek 思考修复 JSON 配置列
--
-- 背景：DeepSeek V4 思考模式下，多轮对话中所有 assistant 消息必须携带 reasoning_content 字段。
-- 但模型有时仅返回 tool_calls 而不产生 reasoning_content，导致下一轮请求被上游 400 拒绝。
-- 开启后，网关自动为匹配模型的 assistant 消息补注入空 reasoning_content 兜底。
--
-- 使用单个 JSON 列存储配置，便于后续扩展字段而无需频繁迁移：
-- {"enabled": false, "keyword": "deepseek", "matchMode": "contains"}

ALTER TABLE gateway_settings ADD COLUMN deepseek_thinking_fix TEXT NOT NULL DEFAULT '{"enabled":false,"keyword":"deepseek","matchMode":"contains"}';
