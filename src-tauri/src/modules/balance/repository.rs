//! # 额度快照数据访问层
//!
//! 直接操作 `provider_balance_snapshots` 表，负责额度快照的持久化与查询。
//!
//! ## 表结构
//!
//! - `provider_balance_snapshots`：(provider_id, snapshot_json, updated_at)
//!   - provider_id 与 providers 表外键关联，删除供应商时级联清理
//!
//! ## 设计约定
//!
//! - 快照 JSON 在 Repository 层保持原始字符串形态，序列化由 Service / Command 层完成。
//! - 本模块不依赖 ai_gateway，仅操作本模块专属表，符合模块边界约束。

use chrono::Utc;

use crate::db::{get_db_pool, DbConn};
use crate::error::{IcodeError, IcodeResult};

use super::types::BalanceSnapshot;

/// 获取一个数据库连接
fn get_conn() -> IcodeResult<DbConn> {
    Ok(get_db_pool()?.get()?)
}

/// 供应商额度快照行（含供应商展示信息，供列表 / 托盘使用）
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderBalanceSnapshotRow {
    /// 供应商 ID
    pub provider_id: String,
    /// 供应商展示名
    pub display_name: String,
    /// 供应商 slug
    pub slug: String,
    /// 额度监控方法（如 "deepseek" / "openrouter"），无配置时为 None
    pub balance_method: Option<String>,
    /// 额度快照（解析后的 BalanceSnapshot）
    pub snapshot: BalanceSnapshot,
    /// 快照更新时间（ISO 8601）
    pub updated_at: String,
}

/// 写入或更新供应商额度快照（upsert）
///
/// 若该 provider_id 已存在快照则覆盖，否则插入新记录。
pub fn upsert_balance_snapshot(
    provider_id: &str,
    snapshot: &BalanceSnapshot,
) -> IcodeResult<()> {
    let conn = get_conn()?;
    let snapshot_json = serde_json::to_string(snapshot)?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO provider_balance_snapshots (provider_id, snapshot_json, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(provider_id) DO UPDATE SET
            snapshot_json = excluded.snapshot_json,
            updated_at = excluded.updated_at",
        rusqlite::params![provider_id, snapshot_json, now],
    )?;

    Ok(())
}

/// 查询单个供应商的额度快照
#[expect(dead_code)]
pub fn get_balance_snapshot(provider_id: &str) -> IcodeResult<Option<BalanceSnapshot>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT snapshot_json FROM provider_balance_snapshots WHERE provider_id = ?1",
    )?;
    let mut rows = stmt.query_map([provider_id], |row| row.get::<_, String>(0))?;
    if let Some(json) = rows.next() {
        let json = json?;
        let snapshot: BalanceSnapshot = serde_json::from_str(&json)
            .map_err(|e| IcodeError::database(format!("解析额度快照失败: {e}")))?;
        Ok(Some(snapshot))
    } else {
        Ok(None)
    }
}

/// 列出所有供应商的额度快照（关联 providers 表获取展示信息）
///
/// 仅返回**已启用额度监控**（`balance_provider_json` 非空且 `method != "none"`）
/// 且存在快照记录的供应商。
///
/// 过滤规则：
/// - `balance_provider_json` 为 None：供应商未配置额度监控，跳过
/// - `method == "none"`：供应商明确关闭额度监控，跳过（即使有历史快照也不返回）
///
/// 这样可保证：供应商关闭额度监控设置后，托盘与前端列表都立即不再展示其额度信息。
pub fn list_balance_snapshots() -> IcodeResult<Vec<ProviderBalanceSnapshotRow>> {
    let conn = get_conn()?;
    let mut stmt = conn.prepare(
        "SELECT s.provider_id, p.display_name, p.slug, p.balance_provider_json,
                s.snapshot_json, s.updated_at
         FROM provider_balance_snapshots s
         INNER JOIN providers p ON p.id = s.provider_id
         ORDER BY p.sort_order, p.created_at",
    )?;
    let rows = stmt.query_map([], |row| {
        let provider_id: String = row.get(0)?;
        let display_name: String = row.get(1)?;
        let slug: String = row.get(2)?;
        let balance_provider_json: Option<String> = row.get(3)?;
        let snapshot_json: String = row.get(4)?;
        let updated_at: String = row.get(5)?;
        Ok((provider_id, display_name, slug, balance_provider_json, snapshot_json, updated_at))
    })?;

    let mut result = Vec::new();
    for row in rows {
        let (provider_id, display_name, slug, balance_provider_json, snapshot_json, updated_at) = row?;

        // 从 balance_provider_json 提取 method 字段
        // 若 json 为空或解析失败，balance_method 为 None
        let balance_method: Option<String> = balance_provider_json
            .as_deref()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok())
            .and_then(|v| v.get("method").and_then(|m| m.as_str()).map(String::from));

        // 过滤：未配置额度监控（balance_provider_json 为 None）或 method=none 的供应商
        // 这是用户需求1的核心实现：关闭额度更新设置后，托盘与列表都不再展示
        if balance_provider_json.is_none() || balance_method.as_deref() == Some("none") {
            continue;
        }

        // 解析快照 JSON
        let snapshot: BalanceSnapshot = match serde_json::from_str(&snapshot_json) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("解析供应商 {} 额度快照失败: {}", provider_id, e);
                continue;
            }
        };

        result.push(ProviderBalanceSnapshotRow {
            provider_id,
            display_name,
            slug,
            balance_method,
            snapshot,
            updated_at,
        });
    }
    Ok(result)
}

/// 删除指定供应商的额度快照
pub fn delete_balance_snapshot(provider_id: &str) -> IcodeResult<()> {
    let conn = get_conn()?;
    conn.execute(
        "DELETE FROM provider_balance_snapshots WHERE provider_id = ?1",
        rusqlite::params![provider_id],
    )?;
    Ok(())
}
