-- 社区本地状态：app_settings 表新增 community_json 列
-- 存储社区门禁开关 / base_url / 设备身份 / 昵称头像缓存（docs/proposals/community.md §7.3）
ALTER TABLE app_settings ADD COLUMN community_json TEXT;
