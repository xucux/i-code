//! # 社区本地状态数据访问层
//!
//! 社区本地状态（门禁开关 / base_url / 设备身份 / 昵称头像缓存）存入
//! `app_settings.community_json` 列（§7.3，沿用 settings 模块，不新建表）。
//! 该列由 `V008__community_local_state.sql` 迁移新增。

use chrono::Utc;

use crate::db::get_db_pool;
use crate::error::{IcodeError, IcodeResult};

use super::types::CommunityLocalState;

/// `app_settings.community_json` 列名
const COLUMN: &str = "community_json";

/// 读取社区本地状态
///
/// 列不存在或为空时返回 `None`（调用方回退到默认值）。
/// 解析失败视为数据损坏，返回 VALIDATION 错误。
pub fn get_local_state() -> IcodeResult<Option<CommunityLocalState>> {
    let conn = get_db_pool()?.get()?;
    let json: Option<String> = conn
        .query_row(
            &format!(
                "SELECT {COLUMN} FROM app_settings WHERE id = 'default'"
            ),
            [],
            |row| row.get(0),
        )
        .map_err(|e| IcodeError::database(format!("读取社区本地状态失败：{e}")))?;

    match json {
        Some(j) if !j.is_empty() => {
            let state = serde_json::from_str(&j)
                .map_err(|e| IcodeError::validation(format!("解析社区本地状态失败：{e}")))?;
            Ok(Some(state))
        }
        _ => Ok(None),
    }
}

/// 写入社区本地状态（整体覆盖）
pub fn set_local_state(state: &CommunityLocalState) -> IcodeResult<()> {
    let conn = get_db_pool()?.get()?;
    let json = serde_json::to_string(state)?;
    let now = Utc::now().to_rfc3339();
    let affected = conn
        .execute(
            &format!(
                "UPDATE app_settings SET {COLUMN} = ?1, updated_at = ?2 WHERE id = 'default'"
            ),
            rusqlite::params![json, now],
        )
        .map_err(|e| IcodeError::database(format!("写入社区本地状态失败：{e}")))?;
    if affected == 0 {
        return Err(IcodeError::internal(
            "app_settings 单例行不存在，无法写入社区状态；请检查 V001 迁移是否已执行",
        ));
    }
    Ok(())
}
