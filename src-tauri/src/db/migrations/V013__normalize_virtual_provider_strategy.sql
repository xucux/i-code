-- 移除已废弃的故障转移策略（load_balance / on_all）
-- 历史数据统一归一为 fallback：新建 / 更新由 Service 层拒绝写入其它值
UPDATE virtual_providers SET strategy = 'fallback' WHERE strategy <> 'fallback';
