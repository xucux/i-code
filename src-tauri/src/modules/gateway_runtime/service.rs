//! # Gateway Runtime 业务服务层
//!
//! 提供 HTTP Server 生命周期管理：start / stop / status / health。
//!
//! ## 架构
//!
//! - `GatewayRuntimeHandle`：Tauri State 句柄，持有所有依赖模块的 Service Handle
//! - `GatewayRuntimeService`：业务逻辑，管理 axum Server 的启动与停止
//! - `GatewaySharedState`：传递给 axum Router 的共享状态（包含所有 Service Handle）
//!
//! ## 生命周期
//!
//! 1. `start()`：解析监听地址 → 构建 Router → spawn axum Server → 保存 shutdown 信号
//! 2. `stop()`：发送 shutdown 信号 → 等待 Server 退出 → 清理状态
//! 3. `status()`：读取当前运行时状态
//!
//! ## 线程模型
//!
//! - axum Server 在独立的 tokio task 中运行
//! - shutdown 信号通过 `tokio::sync::oneshot` 传递
//! - 运行时状态通过 `Mutex<GatewayRuntimeState>` 保护

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::call_records::CallRecordsHandle;
use crate::modules::logger::types::{ForwardLogConfig, CommandLogConfig};
use crate::modules::logger::LoggerServiceHandle;
use crate::modules::secret::SecretServiceHandle;
use crate::modules::virtual_provider::VirtualProviderHandle;

use super::router;
use super::types::{
    CatalogModel, CatalogProvider, GatewayRuntimeState, HealthCheckResult, StartGatewayInput,
    StartGatewayResult,
};

/// Gateway Runtime Service 在 Tauri State 中的句柄
///
/// 使用 `Arc` 包装以便在多线程间共享。
/// 持有所有依赖模块的 Service Handle，启动时传递给 axum Router。
pub struct GatewayRuntimeHandle {
    pub inner: Arc<GatewayRuntimeService>,
}

impl Clone for GatewayRuntimeHandle {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

impl GatewayRuntimeHandle {
    /// 创建 Gateway Runtime 句柄
    ///
    /// # 参数
    /// - `app_handle`：Tauri AppHandle，用于向所有窗口广播事件
    /// - `ai_gateway_handle`：AI Gateway 服务句柄（获取供应商/模型列表、网关监听地址、Gateway Key）
    /// - `secret_handle`：Secret 服务句柄（解析 Gateway Key 明文）
    /// - `logger_handle`：Logger 服务句柄（记录请求日志）
    /// - `virtual_provider_handle`：Virtual Provider 服务句柄（故障转移路由）
    /// - `call_records_handle`：Call Records 服务句柄（调用记录持久化）
    pub fn new(
        app_handle: AppHandle,
        ai_gateway_handle: AiGatewayServiceHandle,
        secret_handle: SecretServiceHandle,
        logger_handle: LoggerServiceHandle,
        virtual_provider_handle: VirtualProviderHandle,
        call_records_handle: CallRecordsHandle,
    ) -> Self {
        Self {
            inner: Arc::new(GatewayRuntimeService::new(
                app_handle,
                ai_gateway_handle,
                secret_handle,
                logger_handle,
                virtual_provider_handle,
                call_records_handle,
            )),
        }
    }

    /// 获取内部 Service 引用
    pub fn service(&self) -> &GatewayRuntimeService {
        &self.inner
    }
}

/// 传递给 axum Router 的共享状态
///
/// 包含所有依赖模块的 Service Handle 与 AppHandle，可在 handler 中通过
/// `State<GatewaySharedState>` 提取。
#[derive(Clone)]
pub struct GatewaySharedState {
    pub app_handle: AppHandle,
    pub ai_gateway_handle: AiGatewayServiceHandle,
    pub secret_handle: SecretServiceHandle,
    pub logger_handle: LoggerServiceHandle,
    pub virtual_provider_handle: VirtualProviderHandle,
    pub call_records_handle: CallRecordsHandle,
    /// 内部 CLI 请求豁免认证用的全局密钥
    ///
    /// 项目启动时随机生成，仅用于本进程内 CLI 与 Gateway 之间的可信通信。
    pub inner_cli_api_key: String,
}

/// Gateway Runtime 业务逻辑
pub struct GatewayRuntimeService {
    /// Tauri AppHandle，用于向后端广播事件
    app_handle: AppHandle,
    /// 运行时状态（运行中状态、监听地址、统计计数等）
    state: Mutex<GatewayRuntimeState>,
    /// 共享给 axum Router 的依赖 Service Handle 集合
    shared: GatewaySharedState,
    /// shutdown 信号发送端；Some 表示 Server 正在运行
    shutdown_tx: Mutex<Option<oneshot::Sender<()>>>,
    /// axum Server 的 JoinHandle；Some 表示 Server 正在运行
    server_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl GatewayRuntimeService {
    fn new(
        app_handle: AppHandle,
        ai_gateway_handle: AiGatewayServiceHandle,
        secret_handle: SecretServiceHandle,
        logger_handle: LoggerServiceHandle,
        virtual_provider_handle: VirtualProviderHandle,
        call_records_handle: CallRecordsHandle,
    ) -> Self {
        Self {
            app_handle: app_handle.clone(),
            state: Mutex::new(GatewayRuntimeState::default()),
            shared: GatewaySharedState {
                app_handle,
                ai_gateway_handle,
                secret_handle,
                logger_handle,
                virtual_provider_handle,
                call_records_handle,
                inner_cli_api_key: uuid::Uuid::new_v4().to_string(),
            },
            shutdown_tx: Mutex::new(None),
            server_handle: Mutex::new(None),
        }
    }

    /// 获取共享状态（用于手动构建 Router 的场景，如测试）
    #[allow(dead_code)]
    pub fn shared_state(&self) -> &GatewaySharedState {
        &self.shared
    }

    /// 获取内部 CLI 请求豁免认证用的全局密钥
    ///
    /// 该密钥在 Gateway Runtime 创建时随机生成，供 CLI 管理模块写入 CLI 配置文件。
    #[allow(dead_code)]
    pub fn inner_cli_api_key(&self) -> &str {
        &self.shared.inner_cli_api_key
    }

    // ===== 目录（真实 + 虚拟合并）=====

    /// 合并真实暴露模型与生效虚拟模型，构造统一目录
    ///
    /// 供聊天、CLI 配置管理等前端拉取「内部供应商/模型列表」时使用。
    /// - 真实模型：`ai_gateway.list_exposed_models()`（供应商已启用且模型已暴露）
    /// - 虚拟模型：`virtual_provider.list_exposed_virtual_models()`（虚拟供应商与虚拟模型均启用）
    ///   对外 ID 为 `{alias}/{model_id}`，`provider_slug` 取虚拟供应商 alias。
    pub fn catalog_models(&self) -> IcodeResult<Vec<CatalogModel>> {
        let real = self
            .shared
            .ai_gateway_handle
            .service()
            .list_exposed_models()?;
        let virtual_models = self
            .shared
            .virtual_provider_handle
            .service()
            .list_exposed_virtual_models()?;

        let mut out: Vec<CatalogModel> = real
            .into_iter()
            .map(|m| CatalogModel {
                id: m.id,
                provider_slug: m.provider_slug,
                model_id: m.model_id,
                display_name: m.display_name,
                is_virtual: false,
                thinking_json: m.thinking_json,
            })
            .collect();
        for vm in virtual_models {
            out.push(CatalogModel {
                id: vm.id,
                provider_slug: vm.alias,
                model_id: vm.model_id,
                display_name: vm.display_name,
                is_virtual: true,
                thinking_json: None,
            });
        }
        Ok(out)
    }

    /// 合并真实供应商与生效虚拟供应商，构造统一目录
    ///
    /// 供 CLI 配置管理「添加供应商」绑定下拉使用。
    /// 虚拟供应商条目 `id` 使用 `virtual:{virtual_provider_id}` 前缀，
    /// `slug` 取虚拟供应商 alias，且仅返回已启用的虚拟供应商。
    pub fn catalog_providers(&self) -> IcodeResult<Vec<CatalogProvider>> {
        let real = self
            .shared
            .ai_gateway_handle
            .service()
            .list_providers()?;
        let virtual_providers = self
            .shared
            .virtual_provider_handle
            .service()
            .list_providers()?;

        let mut out: Vec<CatalogProvider> = real
            .into_iter()
            .map(|p| CatalogProvider {
                id: p.id,
                slug: p.slug,
                display_name: p.display_name,
                is_enabled: p.is_enabled,
                is_virtual: false,
                base_url: Some(p.base_url),
                auth_json: p.auth_json,
            })
            .collect();
        for vp in virtual_providers {
            // 仅纳入生效虚拟供应商
            if !vp.is_enabled {
                continue;
            }
            out.push(CatalogProvider {
                id: format!("virtual:{}", vp.id),
                slug: vp.alias,
                display_name: vp.display_name.unwrap_or(vp.name),
                is_enabled: true,
                is_virtual: true,
                base_url: None,
                auth_json: None,
            });
        }
        Ok(out)
    }

    /// 解析网关默认授权 Key 明文
    ///
    /// 供虚拟供应商在 CLI 配置中预填 apiKey 使用（虚拟供应商无独立凭证，
    /// 统一使用网关默认授权 Key）。未配置默认 Key 时返回 `None`。
    pub fn resolve_default_gateway_key(&self) -> IcodeResult<Option<String>> {
        let settings = self
            .shared
            .ai_gateway_handle
            .service()
            .get_gateway_settings()?;
        let Some(id) = settings
            .default_api_key_secret_id
            .filter(|s| !s.trim().is_empty())
        else {
            return Ok(None);
        };
        let plain = super::auth::resolve_default_key(&id, &self.shared.secret_handle)
            .map_err(|code| IcodeError::internal(format!("解析网关默认 Key 失败: {code}")))?;
        Ok(Some(plain))
    }

    /// 广播网关状态变更事件
    ///
    /// 在 start / stop 后调用，将当前运行时状态推送给所有监听窗口。
    fn emit_status_changed(&self) {
        if let Ok(state) = self.state.lock() {
            let _ = self.app_handle.emit("gateway:status-changed", &*state);
        }
    }

    /// 读取当前转发详细日志配置
    pub fn get_forward_log_config(&self) -> ForwardLogConfig {
        self.shared.logger_handle.service().get_settings().to_forward_config()
    }

    /// 更新转发详细日志配置
    pub fn set_forward_log_config(&self, config: ForwardLogConfig) {
        let mut settings = self.shared.logger_handle.service().get_settings();
        settings.enable_request_log = config.enable_request_log;
        settings.enable_response_log = config.enable_response_log;
        settings.forward_max_body_length = config.max_body_length;
        let _ = self.shared.logger_handle.service().update_settings(&settings);
    }

    /// 读取当前 Command 交互日志配置
    #[allow(dead_code)]
    pub fn get_command_log_config(&self) -> CommandLogConfig {
        self.shared.logger_handle.service().get_settings().to_command_config()
    }

    /// 更新 Command 交互日志配置
    #[allow(dead_code)]
    pub fn set_command_log_config(&self, config: CommandLogConfig) {
        let mut settings = self.shared.logger_handle.service().get_settings();
        settings.enable_command_log = config.enable_command_log;
        settings.enable_command_request_log = config.enable_command_request_log;
        settings.enable_command_response_log = config.enable_command_response_log;
        settings.command_max_body_length = config.max_body_length;
        let _ = self.shared.logger_handle.service().update_settings(&settings);
    }

    /// 启动 HTTP Server
    ///
    /// 流程：
    /// 1. 检查是否已在运行
    /// 2. 解析监听地址（输入 > gateway_settings）
    /// 3. 构建 Router 并注入共享状态与认证中间件
    /// 4. spawn axum Server 到 tokio task
    /// 5. 保存 shutdown 信号发送端
    /// 6. 更新运行时状态
    pub async fn start(&self, input: StartGatewayInput) -> IcodeResult<StartGatewayResult> {
        // 1. 检查是否已在运行
        {
            let state = self.state.lock().map_err(|_| IcodeError::internal("状态锁 poisoned"))?;
            if state.is_running {
                return Ok(StartGatewayResult {
                    success: true,
                    host: state.bound_host.clone().unwrap_or_default(),
                    port: state.bound_port.unwrap_or(0),
                    error: Some("网关已在运行".to_string()),
                });
            }
        }

        // 2. 解析监听地址
        let settings = self.shared.ai_gateway_handle.service().get_gateway_settings()?;
        let host = input.host.unwrap_or(settings.gateway_host);
        let port = input.port.unwrap_or(settings.gateway_port as u16);

        // 解析 SocketAddr
        let addr: SocketAddr = format!("{host}:{port}")
            .parse()
            .map_err(|e: std::net::AddrParseError| {
                IcodeError::validation(format!("无效的监听地址 {host}:{port}: {e}"))
            })?;

        // 3. 构建 Router
        let router = router::build_router(self.shared.clone());

        // 4. spawn axum Server
        let (tx, rx) = oneshot::channel::<()>();
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| gateway_bind_error(&host, port, e))?;

        // 实际绑定的地址（端口为 0 时由 OS 分配）
        let bound_addr = listener.local_addr().map_err(|e| {
            IcodeError::internal(format!("获取绑定地址失败: {e}"))
        })?;

        let server = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            });

        let server_handle = tokio::spawn(async move {
            if let Err(e) = server.await {
                tracing::error!("axum Server 运行错误: {e}");
                // 注意：此处无法直接访问 self.shared.logger_handle，
                // 因为 tokio::spawn 的 async move 闭包不能捕获非 Send 的 self 引用。
                // 该错误已通过 log::error! 输出到标准输出，
                // 同时 start() 方法会在 Server 启动前通过 emit_status_changed() 广播状态。
            }
        });

        // 5. 保存 shutdown 信号发送端与 task handle
        {
            let mut tx_guard = self
                .shutdown_tx
                .lock()
                .map_err(|_| IcodeError::internal("shutdown_tx 锁 poisoned"))?;
            *tx_guard = Some(tx);
        }
        {
            let mut handle_guard = self
                .server_handle
                .lock()
                .map_err(|_| IcodeError::internal("server_handle 锁 poisoned"))?;
            *handle_guard = Some(server_handle);
        }

        // 6. 更新运行时状态
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| IcodeError::internal("状态锁 poisoned"))?;
            state.is_running = true;
            state.bound_host = Some(bound_addr.ip().to_string());
            state.bound_port = Some(bound_addr.port());
            state.started_at = Some(chrono::Utc::now().to_rfc3339());
            state.last_error = None;
            state.total_requests = 0;
            state.active_requests = 0;
        }

        tracing::info!("Gateway HTTP Server 已启动: http://{bound_addr}");

        // 写入系统日志
        self.shared.logger_handle.service().log_system(
            crate::modules::logger::types::LogLevel::Info,
            &format!("网关已启动: http://{}", bound_addr),
            Some(file!()),
        );

        // 广播网关状态变更
        self.emit_status_changed();

        Ok(StartGatewayResult {
            success: true,
            host: bound_addr.ip().to_string(),
            port: bound_addr.port(),
            error: None,
        })
    }

    /// 停止 HTTP Server
    ///
    /// 发送 shutdown 信号后等待 Server 优雅退出。
    pub async fn stop(&self) -> IcodeResult<()> {
        // 发送 shutdown 信号
        let tx = {
            let mut guard = self
                .shutdown_tx
                .lock()
                .map_err(|_| IcodeError::internal("shutdown_tx 锁 poisoned"))?;
            guard.take()
        };
        let tx = match tx {
            Some(tx) => tx,
            None => {
                // 不在运行，幂等返回
                return Ok(());
            }
        };

        // 发送 shutdown 信号（接收端已 drop 时忽略错误）
        let _ = tx.send(());

        // 等待 Server 退出（最长 5 秒）
        let handle = {
            let mut guard = self
                .server_handle
                .lock()
                .map_err(|_| IcodeError::internal("server_handle 锁 poisoned"))?;
            guard.take()
        };
        if let Some(handle) = handle {
            let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
        }

        // 更新运行时状态
        {
            let mut state = self
                .state
                .lock()
                .map_err(|_| IcodeError::internal("状态锁 poisoned"))?;
            state.is_running = false;
            state.bound_host = None;
            state.bound_port = None;
            state.started_at = None;
        }

        // 广播网关状态变更
        self.emit_status_changed();

        tracing::info!("Gateway HTTP Server 已停止");

        // 写入系统日志
        self.shared.logger_handle.service().log_system(
            crate::modules::logger::types::LogLevel::Info,
            "网关已停止",
            Some(file!()),
        );

        Ok(())
    }

    /// 获取当前运行时状态
    pub fn status(&self) -> IcodeResult<GatewayRuntimeState> {
        let state = self
            .state
            .lock()
            .map_err(|_| IcodeError::internal("状态锁 poisoned"))?;
        Ok(state.clone())
    }

    /// 健康检查
    ///
    /// v0.1 仅检查数据库连接，后续可扩展上游供应商可达性检查
    pub fn health(&self) -> IcodeResult<HealthCheckResult> {
        let database_ok = check_database_connection();
        Ok(HealthCheckResult {
            alive: true,
            ready: database_ok,
            database_ok,
            checked_at: chrono::Utc::now().timestamp_millis(),
        })
    }

    /// 更新统计计数（由 axum handler 调用）
    ///
    /// v0.1 暂未在 router 中调用，后续迭代接入拦截器链后使用
    #[allow(dead_code)]
    pub fn increment_total_requests(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.total_requests += 1;
        }
    }

    /// 增加活跃请求数
    #[allow(dead_code)]
    pub fn increment_active_requests(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.active_requests += 1;
        }
    }

    /// 减少活跃请求数
    #[allow(dead_code)]
    pub fn decrement_active_requests(&self) {
        if let Ok(mut state) = self.state.lock() {
            if state.active_requests > 0 {
                state.active_requests -= 1;
            }
        }
    }
}

/// 检查数据库连接是否正常
fn check_database_connection() -> bool {
    use crate::db::get_db_pool;
    match get_db_pool() {
        Ok(pool) => match pool.get() {
            Ok(conn) => conn
                .query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
                .map(|v| v == 1)
                .unwrap_or(false),
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// 将网关监听端口绑定错误转换为 IcodeError
///
/// 端口被占用、权限不足或地址不可用（Windows 上常见的"以一种访问权限不允许的方式做了
/// 一个访问套接字的尝试"，多为保留端口 / 端口被占用 / 权限不足所致）时，返回带
/// `reason` 与 `port` 的 details，前端据此复用端口帮助弹窗给用户排查指引；
/// 其他错误返回 `INTERNAL`。
fn gateway_bind_error(host: &str, port: u16, e: std::io::Error) -> IcodeError {
    let reason = match e.kind() {
        std::io::ErrorKind::AddrInUse => "port_in_use",
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::AddrNotAvailable => "addr_not_available",
        _ => return IcodeError::internal(format!("绑定 {host}:{port} 失败: {e}")),
    };
    let msg = format!("无法在 {host}:{port} 启动网关：{e}");
    IcodeError::conflict(msg)
        .with_details(serde_json::json!({ "port": port, "host": host, "reason": reason }))
}
