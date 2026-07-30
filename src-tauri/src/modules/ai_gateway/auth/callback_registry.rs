//! # OAuth 回调服务器注册表
//!
//! 全局内存注册表，跟踪当前活跃的 OAuth 回调服务器实例。
//!
//! ## 设计要点
//!
//! - **仅内存存储**：不持久化，应用重启后清空
//! - **线程安全**：通过 `Mutex<Vec<...>>` 保护，写入开销极低
//! - **生命周期**：回调服务器启动时注册，回调完成/超时/强制关闭时注销
//! - **强制关闭**：通过持有的 `shutdown_tx` 触发 graceful shutdown

use std::sync::{Mutex, OnceLock};

use serde::Serialize;
use tokio::sync::oneshot;

/// 回调服务器条目（内部使用，持有 shutdown 信号）
pub struct CallbackServerEntry {
    /// 唯一标识（UUID）
    pub id: String,
    /// 供应商 ID
    pub provider_id: String,
    /// 供应商名称（用于前端展示）
    pub provider_name: String,
    /// 监听端口
    pub port: u16,
    /// 完整回调 URI
    pub redirect_uri: String,
    /// 是否为固定端口（供应商预设的 redirect_uri）
    pub is_fixed_port: bool,
    /// 启动时间戳（Unix 秒）
    pub started_at: i64,
    /// 服务器关闭信号发送端，强制关闭时调用
    pub shutdown_tx: std::sync::Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
}

/// 回调服务器信息（序列化给前端）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallbackServerInfo {
    /// 唯一标识
    pub id: String,
    /// 供应商 ID
    pub provider_id: String,
    /// 供应商名称
    pub provider_name: String,
    /// 监听端口
    pub port: u16,
    /// 完整回调 URI
    pub redirect_uri: String,
    /// 是否为固定端口
    pub is_fixed_port: bool,
    /// 启动时间戳（Unix 秒）
    pub started_at: i64,
}

impl From<&CallbackServerEntry> for CallbackServerInfo {
    fn from(entry: &CallbackServerEntry) -> Self {
        Self {
            id: entry.id.clone(),
            provider_id: entry.provider_id.clone(),
            provider_name: entry.provider_name.clone(),
            port: entry.port,
            redirect_uri: entry.redirect_uri.clone(),
            is_fixed_port: entry.is_fixed_port,
            started_at: entry.started_at,
        }
    }
}

/// 全局回调服务器注册表
pub struct CallbackRegistry {
    entries: Mutex<Vec<CallbackServerEntry>>,
}

impl CallbackRegistry {
    fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// 注册一个回调服务器条目
    pub fn register(&self, entry: CallbackServerEntry) {
        let mut entries = self.entries.lock().unwrap();
        log::info!(
            "注册 OAuth 回调服务器: id={}, port={}, provider={}",
            entry.id,
            entry.port,
            entry.provider_name
        );
        entries.push(entry);
    }

    /// 注销指定 ID 的回调服务器条目
    pub fn unregister(&self, id: &str) {
        let mut entries = self.entries.lock().unwrap();
        let before = entries.len();
        entries.retain(|e| e.id != id);
        let after = entries.len();
        if before != after {
            log::info!("注销 OAuth 回调服务器: id={}", id);
        }
    }

    /// 列出所有活跃的回调服务器信息（不含 shutdown_tx）
    pub fn list(&self) -> Vec<CallbackServerInfo> {
        let entries = self.entries.lock().unwrap();
        entries.iter().map(CallbackServerInfo::from).collect()
    }

    /// 强制关闭指定 ID 的回调服务器
    ///
    /// 发送 shutdown 信号触发 graceful shutdown，并从注册表中移除。
    /// 返回 `true` 表示找到并关闭了服务器，`false` 表示未找到。
    pub fn force_close(&self, id: &str) -> bool {
        let mut entries = self.entries.lock().unwrap();
        let pos = entries.iter().position(|e| e.id == id);
        if let Some(idx) = pos {
            let entry = entries.remove(idx);
            // 发送 shutdown 信号
            if let Some(tx) = entry.shutdown_tx.lock().unwrap().take() {
                let _ = tx.send(());
            }
            log::info!(
                "强制关闭 OAuth 回调服务器: id={}, port={}",
                entry.id,
                entry.port
            );
            true
        } else {
            false
        }
    }
}

/// 全局注册表实例
static GLOBAL_REGISTRY: OnceLock<CallbackRegistry> = OnceLock::new();

/// 获取全局回调服务器注册表
pub fn global_registry() -> &'static CallbackRegistry {
    GLOBAL_REGISTRY.get_or_init(CallbackRegistry::new)
}
