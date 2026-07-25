//! # 敏感凭据业务服务层
//!
//! 提供 Secret 的保存、读取、引用解析、清理等高层操作。
//! Service 层封装加密/解密与 Repository 调用，对 Commands 层提供简洁接口。
//!
//! ## v0.2 变更
//!
//! - 主密钥不再来自 `master.key` 文件，而是由用户在「设置」中配置的 1-20 位
//!   通用密码经 SHA-256 派生。
//! - 该密码同时用于 `backup` 模块的远端备份文件加密/解密。
//! - 修改通用密码后，旧密码加密的 Secret 与备份将无法解密（初始化版本不处理历史数据迁移）。

use std::sync::Arc;

use crate::core::id::{generate_id, is_snowflake_id};
use crate::error::{IcodeError, IcodeResult};
use crate::modules::settings::SettingsServiceHandle;

use super::crypto::{self, MasterKey};
use super::repository;
#[cfg(test)]
use super::types::build_secret_ref;
use super::types::{
    parse_secret_ref, SecretKind, SecretMask, SecretReferenceScanResult,
};

/// Secret 引用前缀字节数（`$SECRET:` 的长度）
const SECRET_PREFIX_LEN: usize = 8;

/// 通用密码最小长度
const CONFIG_KEY_MIN_LEN: usize = 1;

/// 通用密码最大长度
const CONFIG_KEY_MAX_LEN: usize = 20;

/// Secret Service 在 Tauri State 中的句柄
///
/// 使用 `Arc` 包装以便在多线程间共享。
/// 主密钥不在启动期固定加载，而是每次加密/解密时从 Settings 读取最新密码派生，
/// 以支持用户在运行期间修改密码。
#[derive(Clone)]
pub struct SecretServiceHandle {
    inner: Arc<SecretService>,
}

impl SecretServiceHandle {
    /// 创建 Secret Service
    ///
    /// # 参数
    /// - `settings_handle`：Settings 服务句柄，用于动态读取 `config_key`
    pub fn new(settings_handle: SettingsServiceHandle) -> IcodeResult<Self> {
        Ok(Self {
            inner: Arc::new(SecretService { settings_handle }),
        })
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &SecretService {
        &self.inner
    }
}

/// Secret Service 业务逻辑
///
/// 通过 `SecretServiceHandle::service()` 获取引用，
/// 在 Tauri Command 中通过 `State<SecretServiceHandle>` 访问。
pub struct SecretService {
    /// Settings 服务句柄，用于读取 `config_key`
    settings_handle: SettingsServiceHandle,
}

impl SecretService {
    /// 从 Settings 读取通用密码并派生 AES-256 主密钥
    ///
    /// 若用户未设置密码，返回业务错误提示先配置密码。
    fn load_master_key(&self) -> IcodeResult<MasterKey> {
        let settings = self.settings_handle.service().get_settings()?;
        let password = settings
            .config_key
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                IcodeError::validation("通用密码未配置，请先前往「设置 → 安全」或「备份 → 备份设置」设置 1-20 位密码")
            })?;

        if password.len() < CONFIG_KEY_MIN_LEN || password.len() > CONFIG_KEY_MAX_LEN {
            return Err(IcodeError::validation(format!(
                "通用密码长度必须在 {CONFIG_KEY_MIN_LEN}-{CONFIG_KEY_MAX_LEN} 位之间"
            )));
        }

        Ok(crypto::derive_key_from_password(password))
    }

    /// 保存 Secret
    ///
    /// 生成雪花 ID，加密明文，写入数据库，返回掩码视图。
    /// 明文在加密后立即被丢弃，不再保留在内存中。
    pub fn save_secret(
        &self,
        kind: SecretKind,
        plaintext: &str,
        label: Option<&str>,
    ) -> IcodeResult<SecretMask> {
        let master_key = self.load_master_key()?;
        let id = generate_id();
        let encrypted = crypto::encrypt(&master_key, plaintext)?;
        repository::insert(&id, kind, &encrypted, label)?;

        // 读取刚插入的记录以返回完整掩码视图
        let row = repository::find_by_id(&id)?
            .ok_or_else(|| IcodeError::internal("Secret 保存后立即查询失败"))?;
        Ok(SecretMask {
            id: row.id,
            kind,
            label: row.label,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// 更新已有 Secret 的明文（保留 id 与 kind）
    ///
    /// 用于轮换 API Key 等场景。若指定 ID 不存在返回 NotFound。
    pub fn update_secret(
        &self,
        id: &str,
        plaintext: &str,
        label: Option<&str>,
    ) -> IcodeResult<SecretMask> {
        let master_key = self.load_master_key()?;
        let existing = repository::find_by_id(id)?
            .ok_or_else(|| IcodeError::not_found("Secret", Some(id)))?;
        let kind = SecretKind::from_str(&existing.kind)
            .ok_or_else(|| IcodeError::internal(format!("数据库中 kind 值非法: {}", existing.kind)))?;

        let encrypted = crypto::encrypt(&master_key, plaintext)?;
        repository::update(id, &encrypted, label)?;

        let row = repository::find_by_id(id)?
            .ok_or_else(|| IcodeError::internal("Secret 更新后立即查询失败"))?;
        Ok(SecretMask {
            id: row.id,
            kind,
            label: row.label,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }

    /// 读取 Secret 明文
    ///
    /// **仅在后端使用**，禁止将明文返回前端。
    /// 用于 `gateway-runtime` 转发请求前解析 `$SECRET:{snowflake_id}$` 引用。
    pub fn read_secret(&self, id: &str) -> IcodeResult<String> {
        let master_key = self.load_master_key()?;
        let row = repository::find_by_id(id)?
            .ok_or_else(|| IcodeError::not_found("Secret", Some(id)))?;
        crypto::decrypt(&master_key, &row.encrypted_value)
    }

    /// 删除 Secret
    pub fn delete_secret(&self, id: &str) -> IcodeResult<()> {
        repository::delete(id)
    }

    /// 列出所有 Secret（掩码视图）
    pub fn list_secrets(&self) -> IcodeResult<Vec<SecretMask>> {
        repository::list_all()
    }

    /// 解析单个 `$SECRET:{snowflake_id}$` 引用，返回明文
    ///
    /// 若字符串不是合法引用格式，原样返回。
    /// 若引用的 Secret 不存在或解密失败，返回错误。
    #[allow(dead_code)]
    pub fn resolve_ref(&self, value: &str) -> IcodeResult<String> {
        match parse_secret_ref(value) {
            Some(id) => self.read_secret(id),
            None => Ok(value.to_string()),
        }
    }

    /// 解析字符串中所有 `$SECRET:{snowflake_id}$` 引用并替换为明文
    ///
    /// 适用于单个配置字段值（如 `authJson` 中的 apiKey 字段）。
    /// 非引用格式的字符串保持不变。
    /// 若任一引用的 Secret 不存在，整个解析失败并返回错误。
    pub fn resolve_in_text(&self, text: &str) -> IcodeResult<String> {
        let master_key = self.load_master_key()?;
        let prefix = "$SECRET:";
        let mut result = String::with_capacity(text.len());
        let mut pos = 0;

        while pos < text.len() {
            // 查找下一个引用前缀
            match text[pos..].find(prefix) {
                None => {
                    // 没有更多引用，追加剩余文本
                    result.push_str(&text[pos..]);
                    break;
                }
                Some(rel) => {
                    let abs = pos + rel;
                    // 追加引用前的文本
                    result.push_str(&text[pos..abs]);

                    // 在前缀之后查找结束 $
                    let after_prefix = &text[abs + SECRET_PREFIX_LEN..];
                    match after_prefix.find('$') {
                        None => {
                            // 没有结束 $，原样追加剩余文本
                            result.push_str(&text[abs..]);
                            break;
                        }
                        Some(rel_end) => {
                            let id = &after_prefix[..rel_end];
                            if id.is_empty() {
                                // 空引用（`$SECRET:$`），原样追加并跳过
                                result.push_str("$SECRET:$");
                                pos = abs + SECRET_PREFIX_LEN + rel_end + 1;
                            } else {
                                // 解析并替换为明文
                                let row = repository::find_by_id(id)?
                                    .ok_or_else(|| IcodeError::not_found("Secret", Some(id)))?;
                                let plaintext = crypto::decrypt(&master_key, &row.encrypted_value)?;
                                result.push_str(&plaintext);
                                pos = abs + SECRET_PREFIX_LEN + rel_end + 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// 递归解析 JSON 值中所有字符串字段的 `$SECRET:{snowflake_id}$` 引用并替换为明文
    ///
    /// 用于网关转发前替换整个配置对象中的引用。
    /// 仅解析 String 类型字段，其他类型（数字、布尔、数组、对象）递归处理。
    pub fn resolve_in_json(
        &self,
        value: &serde_json::Value,
    ) -> IcodeResult<serde_json::Value> {
        let master_key = self.load_master_key()?;
        Self::resolve_in_json_with_key(value, &master_key)
    }

    fn resolve_in_json_with_key(
        value: &serde_json::Value,
        master_key: &MasterKey,
    ) -> IcodeResult<serde_json::Value> {
        match value {
            serde_json::Value::String(s) => {
                let resolved = Self::resolve_in_text_with_key(master_key, s)?;
                Ok(serde_json::Value::String(resolved))
            }
            serde_json::Value::Array(arr) => {
                let mut result = Vec::with_capacity(arr.len());
                for item in arr {
                    result.push(Self::resolve_in_json_with_key(item, master_key)?);
                }
                Ok(serde_json::Value::Array(result))
            }
            serde_json::Value::Object(obj) => {
                let mut result = serde_json::Map::with_capacity(obj.len());
                for (k, v) in obj {
                    result.insert(k.clone(), Self::resolve_in_json_with_key(v, master_key)?);
                }
                Ok(serde_json::Value::Object(result))
            }
            // 其他类型原样返回
            other => Ok(other.clone()),
        }
    }

    fn resolve_in_text_with_key(master_key: &MasterKey, text: &str) -> IcodeResult<String> {
        let prefix = "$SECRET:";
        let mut result = String::with_capacity(text.len());
        let mut pos = 0;

        while pos < text.len() {
            match text[pos..].find(prefix) {
                None => {
                    result.push_str(&text[pos..]);
                    break;
                }
                Some(rel) => {
                    let abs = pos + rel;
                    result.push_str(&text[pos..abs]);

                    let after_prefix = &text[abs + SECRET_PREFIX_LEN..];
                    match after_prefix.find('$') {
                        None => {
                            result.push_str(&text[abs..]);
                            break;
                        }
                        Some(rel_end) => {
                            let id = &after_prefix[..rel_end];
                            if id.is_empty() {
                                result.push_str("$SECRET:$");
                                pos = abs + SECRET_PREFIX_LEN + rel_end + 1;
                            } else {
                                let row = repository::find_by_id(id)?
                                    .ok_or_else(|| IcodeError::not_found("Secret", Some(id)))?;
                                let plaintext = crypto::decrypt(master_key, &row.encrypted_value)?;
                                result.push_str(&plaintext);
                                pos = abs + SECRET_PREFIX_LEN + rel_end + 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    /// 扫描 JSON 值中所有 `$SECRET:{snowflake_id}$` 引用，返回 ID 列表
    ///
    /// 用于：
    /// - 清理孤立 Secret 时判断哪些 Secret 仍被引用
    /// - 导出供应商配置时标记 `missingSecrets`
    pub fn scan_references(
        &self,
        value: &serde_json::Value,
    ) -> IcodeResult<SecretReferenceScanResult> {
        let mut secret_ids: Vec<String> = Vec::new();
        Self::collect_secret_ids_static(value, &mut secret_ids);

        // 去重
        secret_ids.sort();
        secret_ids.dedup();

        // 检查每个引用是否存在
        let mut missing = Vec::new();
        for id in &secret_ids {
            if repository::find_by_id(id)?.is_none() {
                missing.push(id.clone());
            }
        }

        Ok(SecretReferenceScanResult {
            secret_ids,
            missing,
        })
    }

    /// 递归收集 JSON 值中的所有 Secret 引用 ID（静态方法，无副作用）
    fn collect_secret_ids_static(value: &serde_json::Value, ids: &mut Vec<String>) {
        match value {
            serde_json::Value::String(s) => {
                if let Some(id) = parse_secret_ref(s) {
                    ids.push(id.to_string());
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    Self::collect_secret_ids_static(item, ids);
                }
            }
            serde_json::Value::Object(obj) => {
                for (_, v) in obj {
                    Self::collect_secret_ids_static(v, ids);
                }
            }
            _ => {}
        }
    }

    /// 清理未引用的孤立 Secret
    ///
    /// 扫描所有业务表（providers、cli_providers、gateway_settings 等）中的 Secret 引用，
    /// 删除不再被任何配置引用的 Secret 记录。
    ///
    /// 返回被清理的记录数。
    ///
    /// **注意**：v0.1 实现仅扫描已知字段；完整的扫描需要遍历所有包含
    /// `*_json` 列的表。后续迭代中通过 schema 元数据驱动扫描。
    pub fn cleanup_orphaned(&self) -> IcodeResult<usize> {
        let all_ids = repository::list_all_ids()?;
        if all_ids.is_empty() {
            return Ok(0);
        }

        let referenced = self.collect_all_referenced_ids()?;
        let orphaned: Vec<String> = all_ids
            .into_iter()
            .filter(|id| !referenced.contains(id))
            .collect();

        if orphaned.is_empty() {
            return Ok(0);
        }

        repository::delete_batch(&orphaned)
    }

    /// 收集所有业务表中被引用的 Secret ID
    ///
    /// v0.1 实现扫描以下表的 JSON 字段与直接引用字段：
    /// - `providers`: auth_json, balance_provider_json, proxy_json
    /// - `cli_providers`: auth_json, balance_json
    /// - `app_settings`: global_proxy_json, network_retry_json
    /// - `gateway_settings`: default_api_key_secret_id
    /// - `provider_extra_headers`, `model_config_extra_headers`: value
    ///
    /// 后续迭代可通过 `db/schema.rs` 中的元数据驱动扫描所有 `*_json` 列。
    fn collect_all_referenced_ids(&self) -> IcodeResult<Vec<String>> {
        let conn = crate::db::get_db_pool()?.get()?;
        let mut all_ids: Vec<String> = Vec::new();

        // 收集所有 JSON 字段，统一扫描其中的 $SECRET:uuid$ 引用
        let json_columns_query = [
            "SELECT auth_json FROM providers WHERE auth_json IS NOT NULL",
            "SELECT balance_provider_json FROM providers WHERE balance_provider_json IS NOT NULL",
            "SELECT proxy_json FROM providers WHERE proxy_json IS NOT NULL",
            "SELECT auth_json FROM cli_providers WHERE auth_json IS NOT NULL",
            "SELECT balance_json FROM cli_providers WHERE balance_json IS NOT NULL",
            "SELECT global_proxy_json FROM app_settings WHERE global_proxy_json IS NOT NULL",
            "SELECT network_retry_json FROM app_settings WHERE network_retry_json IS NOT NULL",
        ];
        for sql in json_columns_query {
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], |row| row.get::<_, Option<String>>(0))?;
            for row in rows {
                if let Some(json_str) = row? {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        Self::collect_secret_ids_static(&value, &mut all_ids);
                    }
                }
            }
        }

        // 直接文本字段（非 JSON），可能包含 $SECRET:uuid$
        let text_columns_query =
            "SELECT value FROM provider_extra_headers
             UNION ALL SELECT value FROM model_config_extra_headers";
        let mut stmt = conn.prepare(text_columns_query)?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let value = row?;
            if let Some(id) = parse_secret_ref(&value) {
                all_ids.push(id.to_string());
            }
        }

        // gateway_settings.default_api_key_secret_id 可能是 $SECRET:{id}$ 引用，
        // 也可能是用户直接填写的明文 key。仅当值看起来像 Secret 引用时才纳入扫描。
        let mut stmt = conn.prepare(
            "SELECT default_api_key_secret_id FROM gateway_settings
             WHERE default_api_key_secret_id IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            let value = row?;
            if let Some(id) = parse_secret_ref(&value) {
                all_ids.push(id.to_string());
            } else if is_snowflake_id(&value) {
                all_ids.push(value);
            }
        }

        Ok(all_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use serde_json::json;

    /// 不依赖数据库的引用解析测试
    /// 使用 mock 实现，仅测试 `resolve_in_text` 与 `resolve_in_json` 的字符串处理逻辑
    /// 数据库相关测试见 `tests/secret_integration.rs`（待添加）

    /// 构造一个测试用的 SecretService（密钥随机）
    /// 注意：由于不连接数据库，read_secret 会失败；仅用于测试字符串处理逻辑
    fn make_test_service() -> SecretService {
        SecretService {
            settings_handle: SettingsServiceHandle::new(),
        }
    }

    #[test]
    fn test_resolve_in_text_no_refs() {
        let svc = make_test_service();
        // 不包含任何引用的字符串应原样返回
        let text = "Bearer sk-12345";
        let resolved = svc.resolve_in_text(text);
        assert!(resolved.is_err(), "未配置密码时应返回错误");
    }

    #[test]
    fn test_resolve_in_text_unclosed_prefix() {
        let svc = make_test_service();
        let text = "Bearer $SECRET:abc no closing";
        let resolved = svc.resolve_in_text(text);
        assert!(resolved.is_err(), "未配置密码时应返回错误");
    }

    #[test]
    fn test_resolve_in_text_empty_ref() {
        let svc = make_test_service();
        let text = "key=$SECRET:$";
        let resolved = svc.resolve_in_text(text);
        assert!(resolved.is_err(), "未配置密码时应返回错误");
    }

    #[test]
    fn test_collect_secret_ids_static() {
        let json = json!({
            "key1": "$SECRET:id-1$",
            "key2": "$SECRET:id-2$",
            "key3": "$SECRET:id-1$",  // 重复
            "plain": "not-a-ref",
            "nested": {
                "deep": "$SECRET:id-3$"
            },
            "list": ["$SECRET:id-4$", "plain"],
            "number": 42,
            "bool": true
        });
        let mut ids = Vec::new();
        SecretService::collect_secret_ids_static(&json, &mut ids);
        // 不去重的原始结果
        assert_eq!(ids.len(), 5);
        assert!(ids.contains(&"id-1".to_string()));
        assert!(ids.contains(&"id-2".to_string()));
        assert!(ids.contains(&"id-3".to_string()));
        assert!(ids.contains(&"id-4".to_string()));
    }

    #[test]
    fn test_build_and_parse_ref_roundtrip() {
        let id = "550e8400-e29b-41d4-a716-446655440000";
        let r = build_secret_ref(id);
        assert_eq!(r, "$SECRET:550e8400-e29b-41d4-a716-446655440000$");
        assert_eq!(parse_secret_ref(&r), Some(id));
    }
}
