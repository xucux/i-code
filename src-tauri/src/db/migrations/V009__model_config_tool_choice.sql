-- V009: model_configs 表新增 tool_choice_json 列
--
-- 背景：部分供应商（如 SenseNova 6.8 Flash Lite）支持 tool_choice 工具选择策略，
-- 推荐枚举为 "auto" / "none" / "required"（或指定工具对象）。
-- 模型配置中持久化该值，供网关请求构造时透传给上游。
--
-- 使用单个 JSON 文本列存储：可存字符串（"auto" 等）或对象（{"type":"function",...}）。

ALTER TABLE model_configs ADD COLUMN tool_choice_json TEXT;
