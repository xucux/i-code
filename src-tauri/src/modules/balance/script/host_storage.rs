//! # 脚本公共存储 Host Functions
//!
//! `storage::get/set/delete/keys/has/clear/incr/set_ns/get_ns/delete_ns/keys_ns`：
//! 读写应用数据目录下的 `script-storage.json`，作为所有脚本模板共享的键值存储。
//!
//! 设计要点：
//!
//! - **文件位置**：与 `i-code.db` 同目录（Tauri `app_config_dir`），文件名 `script-storage.json`
//! - **自动创建**：文件不存在时后端自动创建（初始内容 `{}`）
//! - **明文存储**：非敏感数据，**无需脱敏 / 加密**（与 Secret 体系区分）
//! - **并发安全**：进程内全局单例 `Arc<Mutex<...>>`，跨脚本共享，防止并发写互相覆盖
//! - **原子写**：写入采用「临时文件 + rename」避免写一半损坏
//! - **值类型**：任意可 JSON 序列化的 Rhai 值（字符串 / 数字 / 布尔 / map / 数组）
//! - **TTL 过期**：`set(key, value, ttl_ms)` 可设置过期时间；读取时惰性清理，启动时批量清理
//! - **大小上限**：单值 ≤ [`MAX_VALUE_BYTES`]，总量 ≤ [`MAX_TOTAL_BYTES`]，超出报错
//! - **命名空间**：`set_ns/get_ns/delete_ns/keys_ns` 以 `ns:key` 前缀隔离，避免不同模板 key 冲突
//!
//! 调用记法（与其它模块一致）：`storage::get(...)` 等，禁止 `storage.get(...)`；
//! 扁平别名 `storage_get(key)` / `storage_set(key, value)` / `storage_delete(key)` 等。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rhai::{Dynamic, Engine, EvalAltResult};

use crate::error::{IcodeError, IcodeResult};

/// 公共存储文件名（位于应用配置目录，与 `i-code.db` 同目录）
pub const STORAGE_FILE_NAME: &str = "script-storage.json";

/// TTL 元数据保留键：`data[TTL_KEY] = { key: expires_at_ms }`
const TTL_KEY: &str = "__ttl__";

/// 单值大小上限（序列化后字节数）：64 KiB
pub const MAX_VALUE_BYTES: usize = 64 * 1024;

/// 存储总量上限（序列化后字节数）：1 MiB
pub const MAX_TOTAL_BYTES: usize = 1024 * 1024;

/// 全局存储单例：跨脚本执行共享，保证并发读写一致
static SCRIPT_STORAGE: Mutex<Option<Arc<Mutex<ScriptStorageInner>>>> = Mutex::new(None);

/// 存储内部状态：文件路径 + 内存缓存（含 `__ttl__` 元数据）
struct ScriptStorageInner {
    path: PathBuf,
    data: HashMap<String, serde_json::Value>,
}

impl ScriptStorageInner {
    /// 读取 TTL 元数据 map（不存在时为 `None`）
    fn ttl_map(&self) -> Option<&serde_json::Map<String, serde_json::Value>> {
        self.data.get(TTL_KEY).and_then(|v| v.as_object())
    }

    fn ttl_map_mut(&mut self) -> &mut serde_json::Map<String, serde_json::Value> {
        if !self.data.contains_key(TTL_KEY) {
            self.data
                .insert(TTL_KEY.to_string(), serde_json::json!({}));
        }
        self.data
            .get_mut(TTL_KEY)
            .and_then(|v| v.as_object_mut())
            .expect("__ttl__ 必须是对象")
    }

    /// 惰性过期：若 key 已过期则删除并返回 true（需随后落盘）
    fn expire_key_if_needed(&mut self, key: &str, now: i64) -> bool {
        let expired = self
            .ttl_map()
            .and_then(|m| m.get(key))
            .and_then(|v| v.as_i64())
            .map(|exp| exp <= now)
            .unwrap_or(false);
        if expired {
            self.data.remove(key);
            self.ttl_map_mut().remove(key);
            return true;
        }
        false
    }

    /// 批量清理所有已过期 key；返回是否删除了任何项
    fn sweep_expired(&mut self, now: i64) -> bool {
        let expired: Vec<String> = self
            .ttl_map()
            .map(|m| {
                m.iter()
                    .filter(|(_, v)| v.as_i64().map(|exp| exp <= now).unwrap_or(false))
                    .map(|(k, _)| k.clone())
                    .collect()
            })
            .unwrap_or_default();
        if expired.is_empty() {
            return false;
        }
        for k in &expired {
            self.data.remove(k);
        }
        self.ttl_map_mut()
            .retain(|k, _| !expired.contains(k));
        true
    }

    /// 估算当前数据总字节数（序列化后）
    fn total_bytes(&self) -> usize {
        serde_json::to_vec(&self.data).map(|b| b.len()).unwrap_or(0)
    }
}

/// 初始化脚本公共存储
///
/// - 目录不存在时自动创建
/// - 文件不存在时自动创建（初始 `{}`）
/// - 加载已有内容到内存缓存，并批量清理已过期项
///
/// 在 `main.rs` setup 中与数据库同目录初始化一次；备份恢复后也会重新调用。
pub fn init_script_storage(config_dir: &Path) -> IcodeResult<()> {
    let path = config_dir.join(STORAGE_FILE_NAME);

    // 确保目录存在
    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir)?;
    }
    // 文件不存在 → 自动创建
    if !path.exists() {
        std::fs::write(&path, "{}").map_err(|e| {
            IcodeError::internal(format!("创建脚本公共存储失败（{}）: {e}", path.display()))
        })?;
    }

    // 读取并解析（容错：损坏时按空存储处理，不阻塞脚本运行）
    let data: HashMap<String, serde_json::Value> = match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            tracing::warn!("读取脚本公共存储失败，按空存储处理（{}）: {e}", path.display());
            HashMap::new()
        }
    };

    // 启动清理：删除已过期项并落盘
    let mut inner = ScriptStorageInner { path, data };
    let now = now_ms();
    let expired: Vec<String> = inner
        .ttl_map()
        .map(|m| {
            m.iter()
                .filter(|(_, v)| v.as_i64().map(|exp| exp <= now).unwrap_or(false))
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();
    if !expired.is_empty() {
        for k in &expired {
            inner.data.remove(k);
        }
        inner.ttl_map_mut().retain(|k, _| !expired.contains(k));
        if let Err(e) = flush(&inner) {
            tracing::warn!("清理过期公共存储项后落盘失败: {e}");
        } else {
            tracing::info!("脚本公共存储启动清理完成，清理 {} 个过期项", expired.len());
        }
    }

    let mut guard = SCRIPT_STORAGE
        .lock()
        .map_err(|e| IcodeError::internal(format!("获取脚本公共存储锁失败：{e}")))?;
    *guard = Some(Arc::new(Mutex::new(inner)));
    Ok(())
}

/// 获取全局存储句柄
fn storage_handle() -> IcodeResult<Arc<Mutex<ScriptStorageInner>>> {
    SCRIPT_STORAGE
        .lock()
        .map_err(|e| IcodeError::internal(format!("获取脚本公共存储锁失败：{e}")))?
        .clone()
        .ok_or_else(|| IcodeError::internal("脚本公共存储尚未初始化"))
}

/// 原子写盘：先写临时文件再 rename，避免写一半损坏
fn flush(inner: &ScriptStorageInner) -> IcodeResult<()> {
    let content = serde_json::to_string_pretty(&inner.data)
        .map_err(|e| IcodeError::internal(format!("序列化脚本公共存储失败：{e}")))?;
    let tmp_path = inner.path.with_extension("json.tmp");
    std::fs::write(&tmp_path, &content).map_err(|e| {
        IcodeError::internal(format!("写入脚本公共存储失败（{}）: {e}", tmp_path.display()))
    })?;
    std::fs::rename(&tmp_path, &inner.path).map_err(|e| {
        IcodeError::internal(format!("替换脚本公共存储失败（{}）: {e}", inner.path.display()))
    })?;
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// key 是否为系统保留键（禁止用户使用）
fn is_reserved_key(key: &str) -> bool {
    key == TTL_KEY
}

/// 校验 key 合法性（非空、非保留）
fn validate_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("存储 key 不能为空".to_string());
    }
    if is_reserved_key(key) {
        return Err(format!("key `{key}` 为系统保留键，不能使用"));
    }
    Ok(())
}

/// 检查写入大小是否超限（单值 + 总量）
fn check_size(
    inner: &ScriptStorageInner,
    key: &str,
    new_value: &serde_json::Value,
) -> Result<(), String> {
    let new_bytes = serde_json::to_vec(new_value)
        .map(|b| b.len())
        .map_err(|e| format!("序列化存储值失败: {e}"))?;
    if new_bytes > MAX_VALUE_BYTES {
        return Err(format!(
            "存储值过大（{new_bytes} 字节），单值上限 {MAX_VALUE_BYTES} 字节"
        ));
    }
    let old_bytes = inner
        .data
        .get(key)
        .map(|v| serde_json::to_vec(v).map(|b| b.len()).unwrap_or(0))
        .unwrap_or(0);
    let total = inner.total_bytes().saturating_sub(old_bytes) + new_bytes;
    if total > MAX_TOTAL_BYTES {
        return Err(format!(
            "公共存储总大小超限（约 {total} 字节），上限 {MAX_TOTAL_BYTES} 字节"
        ));
    }
    Ok(())
}

/// `storage::get(key)`：读取公共存储中的值
///
/// - key 存在：返回存储的值（字符串 / 数字 / 布尔 / map / 数组）
/// - key 不存在或已过期：返回 `()`（unit），用 `if v == ()` 判断
fn storage_get_impl(key: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    let handle = storage_handle().map_err(|e| e.message)?;
    let mut inner = handle
        .lock()
        .map_err(|e| format!("脚本公共存储锁中毒：{e}"))?;
    // 惰性过期：已过期则删除并落盘
    if inner.expire_key_if_needed(key, now_ms()) {
        let _ = flush(&inner);
    }
    match inner.data.get(key) {
        Some(v) => Ok(super::host_json::serde_to_dynamic(v)),
        None => Ok(Dynamic::UNIT),
    }
}

/// `storage::set(key, value)`：写入公共存储（无 TTL，永不过期）
fn storage_set_impl(key: &str, value: Dynamic) -> Result<(), Box<EvalAltResult>> {
    storage_set_ttl_impl(key, value, None)
}

/// `storage::set(key, value, ttl_ms)`：写入公共存储并设置过期时间（毫秒）
///
/// - ttl_ms 必须 > 0
/// - 2 参调用等价于 ttl_ms = None（永不过期，并清除已有 TTL）
fn storage_set_ttl_impl(
    key: &str,
    value: Dynamic,
    ttl_ms: Option<i64>,
) -> Result<(), Box<EvalAltResult>> {
    validate_key(key).map_err(|e| format!("脚本公共存储：{e}"))?;
    if let Some(ttl) = ttl_ms {
        if ttl <= 0 {
            return Err(format!("ttl_ms 必须大于 0，收到 {ttl}").into());
        }
    }
    let handle = storage_handle().map_err(|e| e.message)?;
    let mut inner = handle
        .lock()
        .map_err(|e| format!("脚本公共存储锁中毒：{e}"))?;
    let json = super::host_json::dynamic_to_serde(&value)?;
    check_size(&inner, key, &json).map_err(|e| format!("脚本公共存储：{e}"))?;
    inner.data.insert(key.to_string(), json);
    match ttl_ms {
        Some(ttl) => {
            inner
                .ttl_map_mut()
                .insert(key.to_string(), serde_json::json!(now_ms() + ttl));
        }
        None => {
            inner.ttl_map_mut().remove(key);
        }
    }
    flush(&inner).map_err(|e| e.message)?;
    Ok(())
}

/// `storage::delete(key)`：删除公共存储中的 key（幂等，key 不存在不报错）
///
/// 立即落盘（原子写）；同时清理 TTL 记录。
fn storage_delete_impl(key: &str) -> Result<(), Box<EvalAltResult>> {
    let handle = storage_handle().map_err(|e| e.message)?;
    let mut inner = handle
        .lock()
        .map_err(|e| format!("脚本公共存储锁中毒：{e}"))?;
    let removed = inner.data.remove(key).is_some();
    if removed || inner.ttl_map().map(|m| m.contains_key(key)).unwrap_or(false) {
        inner.ttl_map_mut().remove(key);
        flush(&inner).map_err(|e| e.message)?;
    }
    Ok(())
}

/// `storage::has(key)`：key 是否存在（未过期）
fn storage_has_impl(key: &str) -> Result<bool, Box<EvalAltResult>> {
    let handle = storage_handle().map_err(|e| e.message)?;
    let mut inner = handle
        .lock()
        .map_err(|e| format!("脚本公共存储锁中毒：{e}"))?;
    if inner.expire_key_if_needed(key, now_ms()) {
        let _ = flush(&inner);
    }
    Ok(inner.data.contains_key(key))
}

/// `storage::keys()`：列出全部 key（不含系统保留键；已过期项自动清理）
fn storage_keys_impl() -> Result<rhai::Array, Box<EvalAltResult>> {
    let handle = storage_handle().map_err(|e| e.message)?;
    let mut inner = handle
        .lock()
        .map_err(|e| format!("脚本公共存储锁中毒：{e}"))?;
    if inner.sweep_expired(now_ms()) {
        let _ = flush(&inner);
    }
    let mut keys: Vec<String> = inner
        .data
        .keys()
        .filter(|k| !is_reserved_key(k))
        .cloned()
        .collect();
    keys.sort();
    Ok(keys.into_iter().map(Dynamic::from).collect())
}

/// `storage::clear()`：清空全部数据（含 TTL 记录）
fn storage_clear_impl() -> Result<(), Box<EvalAltResult>> {
    let handle = storage_handle().map_err(|e| e.message)?;
    let mut inner = handle
        .lock()
        .map_err(|e| format!("脚本公共存储锁中毒：{e}"))?;
    inner.data.clear();
    flush(&inner).map_err(|e| e.message)?;
    Ok(())
}

/// `storage::incr(key, delta)`：原子自增/自减（整数计数器）
///
/// - key 不存在视为 0；已有值必须是整数，否则报错
/// - 返回新值；保留已有 TTL
fn storage_incr_impl(key: &str, delta: i64) -> Result<i64, Box<EvalAltResult>> {
    validate_key(key).map_err(|e| format!("脚本公共存储：{e}"))?;
    let handle = storage_handle().map_err(|e| e.message)?;
    let mut inner = handle
        .lock()
        .map_err(|e| format!("脚本公共存储锁中毒：{e}"))?;
    // 先惰性过期
    if inner.expire_key_if_needed(key, now_ms()) {
        let _ = flush(&inner);
    }
    let current = match inner.data.get(key) {
        None => 0_i64,
        Some(v) => v
            .as_i64()
            .ok_or_else(|| format!("脚本公共存储：key `{key}` 不是整数，无法自增"))?,
    };
    let next = current + delta;
    inner
        .data
        .insert(key.to_string(), serde_json::Value::from(next));
    flush(&inner).map_err(|e| e.message)?;
    Ok(next)
}

// ===== 命名空间辅助 =====

/// 命名空间拼接：`ns:key`
fn ns_key(ns: &str, key: &str) -> String {
    format!("{ns}:{key}")
}

/// `storage::get_ns(ns, key)`：读取命名空间下的值
fn storage_get_ns_impl(ns: &str, key: &str) -> Result<Dynamic, Box<EvalAltResult>> {
    storage_get_impl(&ns_key(ns, key))
}

/// `storage::set_ns(ns, key, value)`：写入命名空间（等价 `set("ns:key", value)`）
fn storage_set_ns_impl(ns: &str, key: &str, value: Dynamic) -> Result<(), Box<EvalAltResult>> {
    storage_set_impl(&ns_key(ns, key), value)
}

/// `storage::delete_ns(ns, key)`：删除命名空间下的 key
fn storage_delete_ns_impl(ns: &str, key: &str) -> Result<(), Box<EvalAltResult>> {
    storage_delete_impl(&ns_key(ns, key))
}

/// `storage::keys_ns(ns)`：列出命名空间下的全部 key（去掉 `ns:` 前缀）
fn storage_keys_ns_impl(ns: &str) -> Result<rhai::Array, Box<EvalAltResult>> {
    let handle = storage_handle().map_err(|e| e.message)?;
    let mut inner = handle
        .lock()
        .map_err(|e| format!("脚本公共存储锁中毒：{e}"))?;
    if inner.sweep_expired(now_ms()) {
        let _ = flush(&inner);
    }
    let prefix = format!("{ns}:");
    let mut keys: Vec<String> = inner
        .data
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .map(|k| k[prefix.len()..].to_string())
        .collect();
    keys.sort();
    Ok(keys.into_iter().map(Dynamic::from).collect())
}

// ===== 公开 Rust API（供 UI 命令 / 其它模块使用） =====

/// 公共存储条目（UI 浏览用）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ScriptStorageEntry {
    pub key: String,
    pub value: serde_json::Value,
    /// 过期时间戳（毫秒）；无 TTL 时为 `None`
    pub expires_at: Option<i64>,
}

/// 浏览全部存储条目（排除保留键；清理已过期项）
pub fn storage_snapshot() -> IcodeResult<Vec<ScriptStorageEntry>> {
    let handle = storage_handle()?;
    let mut inner = handle
        .lock()
        .map_err(|e| IcodeError::internal(format!("脚本公共存储锁中毒：{e}")))?;
    if inner.sweep_expired(now_ms()) {
        let _ = flush(&inner);
    }
    let mut entries: Vec<ScriptStorageEntry> = inner
        .data
        .iter()
        .filter(|(k, _)| !is_reserved_key(k))
        .map(|(k, v)| ScriptStorageEntry {
            key: k.clone(),
            value: v.clone(),
            expires_at: inner
                .ttl_map()
                .and_then(|m| m.get(k))
                .and_then(|e| e.as_i64()),
        })
        .collect();
    entries.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(entries)
}

/// 写入存储条目（UI 编辑用；`ttl_ms` 为 `Some` 时设置过期）
pub fn storage_set_value(
    key: &str,
    value: serde_json::Value,
    ttl_ms: Option<i64>,
) -> IcodeResult<()> {
    validate_key(key).map_err(IcodeError::validation)?;
    if let Some(ttl) = ttl_ms {
        if ttl <= 0 {
            return Err(IcodeError::validation("ttl_ms 必须大于 0"));
        }
    }
    let handle = storage_handle()?;
    let mut inner = handle
        .lock()
        .map_err(|e| IcodeError::internal(format!("脚本公共存储锁中毒：{e}")))?;
    check_size(&inner, key, &value).map_err(IcodeError::validation)?;
    inner.data.insert(key.to_string(), value);
    match ttl_ms {
        Some(ttl) => {
            inner
                .ttl_map_mut()
                .insert(key.to_string(), serde_json::json!(now_ms() + ttl));
        }
        None => {
            inner.ttl_map_mut().remove(key);
        }
    }
    flush(&inner)
}

/// 删除存储条目（UI 编辑用）
pub fn storage_delete_value(key: &str) -> IcodeResult<()> {
    let handle = storage_handle()?;
    let mut inner = handle
        .lock()
        .map_err(|e| IcodeError::internal(format!("脚本公共存储锁中毒：{e}")))?;
    let removed = inner.data.remove(key).is_some();
    if removed || inner.ttl_map().map(|m| m.contains_key(key)).unwrap_or(false) {
        inner.ttl_map_mut().remove(key);
        flush(&inner)?;
    }
    Ok(())
}

/// 清空全部条目（UI 编辑用）
pub fn storage_clear_all() -> IcodeResult<()> {
    let handle = storage_handle()?;
    let mut inner = handle
        .lock()
        .map_err(|e| IcodeError::internal(format!("脚本公共存储锁中毒：{e}")))?;
    inner.data.clear();
    flush(&inner)
}

/// 注册公共存储 host 函数
///
/// - 扁平名：`storage_get/storage_set/storage_delete/storage_has/storage_keys/storage_clear/storage_incr/storage_set_ns/storage_get_ns/storage_delete_ns/storage_keys_ns`
/// - 静态模块：`storage::get/set/delete/has/keys/clear/incr/set_ns/get_ns/delete_ns/keys_ns`
///
/// 注意：模块调用请用 `storage::get(...)`，不要写 `storage.get(...)`。
pub fn register(engine: &mut Engine) {
    // 扁平别名
    engine.register_fn("storage_get", storage_get_impl);
    engine.register_fn("storage_set", storage_set_impl);
    engine.register_fn("storage_set", storage_set_ttl_impl);
    engine.register_fn("storage_delete", storage_delete_impl);
    engine.register_fn("storage_has", storage_has_impl);
    engine.register_fn("storage_keys", storage_keys_impl);
    engine.register_fn("storage_clear", storage_clear_impl);
    engine.register_fn("storage_incr", storage_incr_impl);
    engine.register_fn("storage_set_ns", storage_set_ns_impl);
    engine.register_fn("storage_get_ns", storage_get_ns_impl);
    engine.register_fn("storage_delete_ns", storage_delete_ns_impl);
    engine.register_fn("storage_keys_ns", storage_keys_ns_impl);

    // 静态模块（重载：set 2 参 / 3 参）
    let mut module = rhai::Module::new();
    module.set_native_fn("get", |key: &str| storage_get_impl(key));
    module.set_native_fn("set", |key: &str, value: Dynamic| storage_set_impl(key, value));
    module.set_native_fn("set", |key: &str, value: Dynamic, ttl_ms: i64| {
        storage_set_ttl_impl(key, value, Some(ttl_ms))
    });
    module.set_native_fn("delete", |key: &str| storage_delete_impl(key));
    module.set_native_fn("has", |key: &str| storage_has_impl(key));
    module.set_native_fn("keys", || storage_keys_impl());
    module.set_native_fn("clear", || storage_clear_impl());
    module.set_native_fn("incr", |key: &str, delta: i64| storage_incr_impl(key, delta));
    module.set_native_fn("get_ns", |ns: &str, key: &str| storage_get_ns_impl(ns, key));
    module.set_native_fn("set_ns", |ns: &str, key: &str, value: Dynamic| {
        storage_set_ns_impl(ns, key, value)
    });
    module.set_native_fn("delete_ns", |ns: &str, key: &str| {
        storage_delete_ns_impl(ns, key)
    });
    module.set_native_fn("keys_ns", |ns: &str| storage_keys_ns_impl(ns));
    engine.register_static_module("storage", module.into());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 全局存储为单例，测试必须串行执行（并行时各测试的 tempdir 会互相覆盖句柄）
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// 测试用：临时目录初始化存储
    fn init_tmp() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        init_script_storage(tmp.path()).expect("初始化脚本公共存储失败");
        tmp
    }

    #[test]
    fn init_creates_file() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = init_tmp();
        let path = tmp.path().join(STORAGE_FILE_NAME);
        assert!(path.exists(), "script-storage.json 应自动创建");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("{}") || content == "{}", "初始内容应为空对象");
    }

    #[test]
    fn set_then_get_roundtrip() {
        let _g = TEST_LOCK.lock().unwrap();
        // 必须持有 TempDir，否则目录被立即删除导致写入失败
        let _tmp = init_tmp();
        storage_set_impl("balance", Dynamic::from("12.34")).expect("set 失败");
        let v = storage_get_impl("balance").expect("get 失败");
        assert_eq!(v.as_string().ok(), Some("12.34".to_string()));

        // 数字
        storage_set_impl("count", Dynamic::from(42_i64)).expect("set 失败");
        let v = storage_get_impl("count").expect("get 失败");
        assert_eq!(v.as_int(), Ok(42));

        // map
        let mut m = rhai::Map::new();
        m.insert("used".into(), Dynamic::from(100_i64));
        storage_set_impl("quota", Dynamic::from_map(m)).expect("set 失败");
        let v = storage_get_impl("quota").expect("get 失败");
        let map = v.try_cast::<rhai::Map>().expect("应为 map");
        assert!(map.contains_key("used"));
    }

    #[test]
    fn missing_key_returns_unit() {
        let _g = TEST_LOCK.lock().unwrap();
        init_tmp();
        let v = storage_get_impl("not-exist").expect("get 失败");
        assert!(v.is_unit(), "不存在的 key 应返回 ()");
    }

    #[test]
    fn persist_across_reload() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = init_tmp();
        storage_set_impl("keep", Dynamic::from("value")).expect("set 失败");

        // 重新初始化（模拟重启）后数据仍在
        init_script_storage(tmp.path()).expect("重新初始化失败");
        let v = storage_get_impl("keep").expect("get 失败");
        assert_eq!(v.as_string().ok(), Some("value".to_string()));
    }

    #[test]
    fn overwrite_same_key() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_impl("k", Dynamic::from(1_i64)).expect("set 失败");
        storage_set_impl("k", Dynamic::from(2_i64)).expect("覆盖失败");
        let v = storage_get_impl("k").expect("get 失败");
        assert_eq!(v.as_int(), Ok(2));
    }

    #[test]
    fn delete_removes_key() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_impl("temp", Dynamic::from("value")).expect("set 失败");
        storage_delete_impl("temp").expect("delete 失败");
        let v = storage_get_impl("temp").expect("get 失败");
        assert!(v.is_unit(), "删除后 get 应返回 ()");

        // 删除不存在的 key：幂等，不报错
        storage_delete_impl("not-exist").expect("删除不存在的 key 不应报错");
    }

    #[test]
    fn delete_persists() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = init_tmp();
        storage_set_impl("a", Dynamic::from(1_i64)).expect("set 失败");
        storage_set_impl("b", Dynamic::from(2_i64)).expect("set 失败");
        storage_delete_impl("a").expect("delete 失败");

        // 重新初始化（模拟重启）后删除结果仍在
        init_script_storage(tmp.path()).expect("重新初始化失败");
        assert!(storage_get_impl("a").expect("get 失败").is_unit());
        assert_eq!(storage_get_impl("b").expect("get 失败").as_int(), Ok(2));
    }

    // ===== A1：keys / has / clear =====

    #[test]
    fn has_and_keys() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_impl("a", Dynamic::from(1_i64)).expect("set 失败");
        storage_set_impl("b", Dynamic::from("x")).expect("set 失败");

        assert!(storage_has_impl("a").expect("has 失败"));
        assert!(!storage_has_impl("not-exist").expect("has 失败"));

        let keys = storage_keys_impl().expect("keys 失败");
        let keys: Vec<String> = keys
            .iter()
            .filter_map(|k| k.clone().try_cast::<String>())
            .collect();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn keys_excludes_reserved() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_impl("a", Dynamic::from(1_i64)).expect("set 失败");
        // 保留键 __ttl__ 不应出现在 keys 中
        let keys = storage_keys_impl().expect("keys 失败");
        let keys: Vec<String> = keys
            .iter()
            .filter_map(|k| k.clone().try_cast::<String>())
            .collect();
        assert!(!keys.iter().any(|k| k == TTL_KEY));
        assert_eq!(keys, vec!["a".to_string()]);
    }

    #[test]
    fn clear_wipes_all() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_impl("a", Dynamic::from(1_i64)).expect("set 失败");
        storage_set_impl("b", Dynamic::from(2_i64)).expect("set 失败");
        storage_clear_impl().expect("clear 失败");
        assert!(!storage_has_impl("a").expect("has 失败"));
        assert!(!storage_has_impl("b").expect("has 失败"));
        assert!(storage_keys_impl().expect("keys 失败").is_empty());
    }

    // ===== A2：命名空间 =====

    #[test]
    fn namespace_isolated() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_ns_impl("tpl1", "count", Dynamic::from(1_i64)).expect("set_ns 失败");
        storage_set_ns_impl("tpl2", "count", Dynamic::from(99_i64)).expect("set_ns 失败");

        // 同名 key 在不同命名空间互不干扰
        let v1 = storage_get_ns_impl("tpl1", "count").expect("get_ns 失败");
        assert_eq!(v1.as_int(), Ok(1));
        let v2 = storage_get_ns_impl("tpl2", "count").expect("get_ns 失败");
        assert_eq!(v2.as_int(), Ok(99));

        // keys_ns 只返回本命名空间的 key
        let keys = storage_keys_ns_impl("tpl1").expect("keys_ns 失败");
        let keys: Vec<String> = keys
            .iter()
            .filter_map(|k| k.clone().try_cast::<String>())
            .collect();
        assert_eq!(keys, vec!["count".to_string()]);

        // delete_ns 只删本命名空间
        storage_delete_ns_impl("tpl1", "count").expect("delete_ns 失败");
        assert!(storage_get_ns_impl("tpl1", "count").expect("get_ns 失败").is_unit());
        assert_eq!(
            storage_get_ns_impl("tpl2", "count").expect("get_ns 失败").as_int(),
            Ok(99)
        );
    }

    // ===== A3：TTL 过期 =====

    #[test]
    fn ttl_expires_after_deadline() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        // 立即读取：先用较长 TTL，避免并行负载下线程被调度导致偶发「提前过期」
        storage_set_ttl_impl("temp", Dynamic::from("value"), Some(60_000)).expect("set ttl 失败");
        assert_eq!(
            storage_get_impl("temp").expect("get 失败").as_string().ok(),
            Some("value".to_string())
        );
        assert!(storage_has_impl("temp").expect("has 失败"));

        // 重设 10ms 过期，等待后应已移除且不可见
        storage_set_ttl_impl("temp", Dynamic::from("value"), Some(10)).expect("set ttl 失败");
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(storage_get_impl("temp").expect("get 失败").is_unit());
        assert!(!storage_has_impl("temp").expect("has 失败"));
        assert!(storage_keys_impl().expect("keys 失败").is_empty());
    }

    #[test]
    fn ttl_removed_when_set_without_ttl() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_ttl_impl("k", Dynamic::from(1_i64), Some(10)).expect("set ttl 失败");
        // 2 参 set 清除 TTL（永不过期）
        storage_set_impl("k", Dynamic::from(2_i64)).expect("set 失败");
        std::thread::sleep(std::time::Duration::from_millis(30));
        assert_eq!(storage_get_impl("k").expect("get 失败").as_int(), Ok(2));
    }

    #[test]
    fn ttl_swept_on_reload() {
        let _g = TEST_LOCK.lock().unwrap();
        let tmp = init_tmp();
        storage_set_ttl_impl("short", Dynamic::from("x"), Some(5)).expect("set ttl 失败");
        storage_set_impl("keep", Dynamic::from("y")).expect("set 失败");
        std::thread::sleep(std::time::Duration::from_millis(20));

        // 重新初始化：启动清理删除过期项
        init_script_storage(tmp.path()).expect("重新初始化失败");
        assert!(storage_get_impl("short").expect("get 失败").is_unit());
        assert_eq!(
            storage_get_impl("keep").expect("get 失败").as_string().ok(),
            Some("y".to_string())
        );
    }

    #[test]
    fn ttl_zero_rejected() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        let r = storage_set_ttl_impl("k", Dynamic::from(1_i64), Some(0));
        assert!(r.is_err(), "ttl_ms=0 应报错");
    }

    // ===== A4：原子计数器 =====

    #[test]
    fn incr_works() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        // 不存在 → 从 0 开始
        assert_eq!(storage_incr_impl("n", 1).expect("incr 失败"), 1);
        assert_eq!(storage_incr_impl("n", 5).expect("incr 失败"), 6);
        assert_eq!(storage_incr_impl("n", -2).expect("incr 失败"), 4);
        // 持久化
        assert_eq!(storage_get_impl("n").expect("get 失败").as_int(), Ok(4));
    }

    #[test]
    fn incr_rejects_non_integer() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_impl("s", Dynamic::from("abc")).expect("set 失败");
        assert!(storage_incr_impl("s", 1).is_err(), "非整数 key 自增应报错");
    }

    // ===== A5：大小上限 =====

    #[test]
    fn value_size_limit() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        let big = "x".repeat(MAX_VALUE_BYTES + 1);
        let r = storage_set_impl("big", Dynamic::from(big));
        assert!(r.is_err(), "超过单值上限应报错");
        assert!(!storage_has_impl("big").expect("has 失败"));
    }

    #[test]
    fn total_size_limit() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        // 直接向全局句柄注入接近总量上限的数据（绕过单值上限，验证总量校验）
        {
            let handle = storage_handle().expect("句柄失败");
            let mut inner = handle.lock().expect("锁失败");
            inner.data.insert(
                "fill".to_string(),
                serde_json::Value::String("x".repeat(MAX_TOTAL_BYTES)),
            );
        }
        // 再写一个值应超限
        let r = storage_set_impl("c", Dynamic::from("y"));
        assert!(r.is_err(), "超过总量上限应报错");
        assert!(!storage_has_impl("c").expect("has 失败"));
    }

    // ===== 公开 Rust API =====

    #[test]
    fn snapshot_and_public_api() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        storage_set_impl("a", Dynamic::from(1_i64)).expect("set 失败");
        storage_set_ttl_impl("b", Dynamic::from("x"), Some(60_000)).expect("set ttl 失败");

        let entries = storage_snapshot().expect("snapshot 失败");
        assert_eq!(entries.len(), 2);
        let a = entries.iter().find(|e| e.key == "a").expect("缺 a");
        assert!(a.expires_at.is_none());
        let b = entries.iter().find(|e| e.key == "b").expect("缺 b");
        assert!(b.expires_at.is_some(), "b 应有 TTL");

        // UI 写入/删除/清空
        storage_set_value("c", serde_json::json!({"k": 1}), None).expect("set_value 失败");
        assert!(storage_has_impl("c").expect("has 失败"));
        storage_delete_value("c").expect("delete_value 失败");
        assert!(!storage_has_impl("c").expect("has 失败"));
        storage_clear_all().expect("clear_all 失败");
        assert!(storage_snapshot().expect("snapshot 失败").is_empty());
    }

    #[test]
    fn reserved_key_rejected() {
        let _g = TEST_LOCK.lock().unwrap();
        let _tmp = init_tmp();
        let r = storage_set_impl(TTL_KEY, Dynamic::from(1_i64));
        assert!(r.is_err(), "保留键应禁止写入");
    }
}
