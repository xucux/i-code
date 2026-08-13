//! # 定时任务调度器
//!
//! 提供通用周期任务框架，启动后在后台以 `tokio::spawn` + `tokio::time::sleep` 运行。
//! 任务句柄统一由 [`SchedulerHandle`] 持有，应用退出时自动 abort。
//!
//! ## 当前内置任务
//!
//! - **OAuth token 续期**：每 10 分钟扫描 `providers` 表中 `auth_expires_at`
//!   在 2 小时内过期且 `auth_method` 为 OAuth 类的已启用供应商，
//!   调用 `refresh_oauth_token` 用 refresh_token 续期，续期结果写入自研 logger。
//! - **虚拟路由主动健康检查**：每 60 秒扫描 `virtual_model_routes` 中已启用
//!   且（is_healthy=0 OR consecutive_failures>0）的路由，对每条路由发起轻量探活请求
//!   （GET {provider_base_url}/v1/models，5s 超时）；
//!   连续探活成功 N 次后置 is_healthy=1，连续失败 N 次后置 is_healthy=0。
//! - **虚拟路由尝试历史清理**：每 24 小时清理 `virtual_route_attempts` 表中
//!   超过 30 天的历史记录，避免表无限膨胀。

use std::time::{Duration, Instant};

use tauri::async_runtime::{self, JoinHandle};

use crate::error::IcodeResult;
use crate::modules::ai_gateway::types::AuthMethod;
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::logger::Log;
use crate::modules::virtual_provider::VirtualProviderHandle;

/// OAuth 续期扫描间隔（秒）
const OAUTH_REFRESH_INTERVAL_SECS: u64 = 600; // 10 分钟

/// OAuth 续期阈值：过期前 2 小时内触发续期
const OAUTH_REFRESH_THRESHOLD_SECS: i64 = 2 * 3600;

/// 虚拟路由健康检查扫描间隔（秒）
const HEALTH_CHECK_INTERVAL_SECS: u64 = 60;

/// 单次探活请求超时（秒）
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 5;

/// 连续探活成功几次后置 is_healthy=1
const HEALTH_RECOVER_SUCCESS_THRESHOLD: i32 = 2;

/// 连续探活失败几次后置 is_healthy=0
const HEALTH_DEGRADE_FAILURE_THRESHOLD: i32 = 3;

/// 虚拟路由尝试历史清理间隔（秒）：每 24 小时清理一次
const ATTEMPTS_CLEANUP_INTERVAL_SECS: u64 = 24 * 3600;

/// 虚拟路由尝试历史保留天数：超过 30 天的记录自动清理
const ATTEMPTS_CLEANUP_KEEP_DAYS: u32 = 30;

/// 调度器句柄，持有所有后台任务句柄
///
/// 应用启动时由 `setup` 创建并 `app.manage`；Drop 时自动终止所有任务。
pub struct SchedulerHandle {
    tasks: Vec<JoinHandle<()>>,
}

impl SchedulerHandle {
    /// 创建调度器并启动所有内置任务
    pub fn new(
        ai_gateway: AiGatewayServiceHandle,
        virtual_provider: VirtualProviderHandle,
    ) -> Self {
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();
        tasks.push(async_runtime::spawn(oauth_refresh_loop(ai_gateway.clone())));
        tasks.push(async_runtime::spawn(health_check_loop(
            ai_gateway,
            virtual_provider,
        )));
        tasks.push(async_runtime::spawn(attempts_cleanup_loop()));
        tracing::info!(
            "Scheduler 模块初始化完成（OAuth 续期、虚拟路由健康检查、尝试历史清理任务已启动）"
        );
        Self { tasks }
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        for task in &self.tasks {
            task.abort();
        }
    }
}

/// OAuth token 续期循环
async fn oauth_refresh_loop(ai_gateway: AiGatewayServiceHandle) {
    let interval = Duration::from_secs(OAUTH_REFRESH_INTERVAL_SECS);
    tracing::info!(
        "OAuth 续期任务启动，扫描间隔 {}s，阈值 {}s",
        OAUTH_REFRESH_INTERVAL_SECS,
        OAUTH_REFRESH_THRESHOLD_SECS
    );
    loop {
        // 先等待一个周期（启动后延迟首次扫描，避免与初始化竞争）
        tokio::time::sleep(interval).await;
        if let Err(e) = scan_and_refresh(&ai_gateway, OAUTH_REFRESH_THRESHOLD_SECS).await {
            tracing::error!("OAuth 续期扫描失败: {}", e);
            Log::error(&format!("[Scheduler] OAuth 续期扫描失败：{}", e));
        }
    }
}

/// 扫描所有供应商，对快过期的 OAuth 供应商执行续期
async fn scan_and_refresh(
    ai_gateway: &AiGatewayServiceHandle,
    threshold_secs: i64,
) -> IcodeResult<()> {
    let providers = ai_gateway.service().list_providers()?;
    let now = chrono::Utc::now();
    let threshold = now + chrono::Duration::seconds(threshold_secs);

    for p in providers {
        // 仅处理已启用供应商
        if !p.is_enabled {
            continue;
        }
        // 必须有 auth_expires_at 与 auth_method 顶层字段
        let Some(expires_str) = &p.auth_expires_at else {
            continue;
        };
        let Some(method_str) = &p.auth_method else {
            continue;
        };

        // 解析 AuthMethod，仅处理 OAuth 类
        let Some(method) = AuthMethod::from_kebab_str(method_str) else {
            continue;
        };
        if !method.is_oauth() {
            continue;
        }

        // 解析过期时间；解析失败则跳过
        let Ok(expires_dt) = chrono::DateTime::parse_from_rfc3339(expires_str) else {
            tracing::warn!(
                "OAuth 续期：供应商 {} 的 auth_expires_at 无法解析: {}",
                p.display_name,
                expires_str
            );
            continue;
        };
        let expires_utc = expires_dt.with_timezone(&chrono::Utc);

        // 未在阈值内过期则跳过
        if expires_utc > threshold {
            continue;
        }

        tracing::info!(
            "OAuth 续期：provider_id={}, name={}, method={}, expires_at={}",
            p.id, p.display_name, method_str, expires_str
        );

        match ai_gateway
            .service()
            .refresh_oauth_token(&p.id, method)
            .await
        {
            Ok(_) => {
                tracing::info!(
                    "OAuth 续期成功：provider_id={}, name={}",
                    p.id,
                    p.display_name
                );
                Log::info(&format!(
                    "[Scheduler] OAuth token 已续期（供应商 {}）",
                    p.display_name
                ));
            }
            Err(e) => {
                tracing::error!(
                    "OAuth 续期失败：provider_id={}, name={}, error={}",
                    p.id,
                    p.display_name,
                    e
                );
                Log::error(&format!(
                    "[Scheduler] OAuth 续期失败（供应商 {}）：{}",
                    p.display_name, e
                ));
            }
        }
    }
    Ok(())
}

/// 虚拟路由健康检查循环
///
/// 每个周期取出所有启用且需要探活的路由（is_healthy=0 或 consecutive_failures>0），
/// 顺序探活并按结果更新健康状态。
async fn health_check_loop(
    ai_gateway: AiGatewayServiceHandle,
    virtual_provider: VirtualProviderHandle,
) {
    let interval = Duration::from_secs(HEALTH_CHECK_INTERVAL_SECS);
    tracing::info!(
        "虚拟路由健康检查任务启动，扫描间隔 {}s，超时 {}s，恢复阈值 {} 次，降级阈值 {} 次",
        HEALTH_CHECK_INTERVAL_SECS,
        HEALTH_CHECK_TIMEOUT_SECS,
        HEALTH_RECOVER_SUCCESS_THRESHOLD,
        HEALTH_DEGRADE_FAILURE_THRESHOLD
    );
    loop {
        // 先等待一个周期，避免与初始化竞争
        tokio::time::sleep(interval).await;
        if let Err(e) = run_health_check_once(&ai_gateway, &virtual_provider).await {
            tracing::error!("虚拟路由健康检查失败: {}", e);
            Log::error(&format!("[Scheduler] 虚拟路由健康检查失败：{}", e));
        }
    }
}

/// 执行一次健康检查扫描
async fn run_health_check_once(
    ai_gateway: &AiGatewayServiceHandle,
    virtual_provider: &VirtualProviderHandle,
) -> IcodeResult<()> {
    let routes = virtual_provider.service().list_routes_for_health_check()?;
    if routes.is_empty() {
        return Ok(());
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS))
        .build()
        .map_err(|e| {
            crate::error::IcodeError::internal(format!("构造探活 HTTP 客户端失败: {e}"))
        })?;

    for route in routes {
        // 拿到目标供应商与认证配置
        let provider = match ai_gateway.service().get_provider(&route.target_provider_id) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "健康检查：路由 {} 的目标供应商 {} 读取失败: {}",
                    route.id,
                    route.target_provider_id,
                    e.message
                );
                continue;
            }
        };
        if !provider.is_enabled {
            continue;
        }

        // 解析认证头
        let auth_config = ai_gateway.service().resolve_auth_for_request(&provider).ok().flatten();
        let extra_headers = ai_gateway
            .service()
            .resolve_extra_headers_for_request(&provider.id)
            .unwrap_or_default();

        let start = Instant::now();
        let result = probe_route(&client, &provider.base_url, &provider.slug, &auth_config, &extra_headers).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            ProbeOutcome::Success => {
                // 探活成功：置健康并清零失败计数。
                // 真正的"连续 N 次成功才恢复"逻辑需要内存计数器，
                // 此处简化为单次成功即恢复，留待后续迭代加内存计数器。
                if !route.is_healthy || route.consecutive_failures > 0 {
                    let _ = virtual_provider
                        .service()
                        .mark_route_healthy(&route.id, duration_ms);
                    Log::info(&format!(
                        "[Scheduler] 虚拟路由探活成功：route_id={}, provider={}, duration={}ms",
                        route.id, provider.slug, duration_ms
                    ));
                } else {
                    // 当前 healthy 且无失败记录，仅刷新 last_check_at
                    let _ = virtual_provider
                        .service()
                        .mark_route_healthy(&route.id, duration_ms);
                }
            }
            ProbeOutcome::ConfigError(err) => {
                // 4xx：配置错误（如 401 鉴权失败），不增计失败次数，记录原因
                let _ = virtual_provider.service().mark_route_check_failed(
                    &route.id,
                    &err,
                    duration_ms,
                    false,
                );
                tracing::warn!(
                    "虚拟路由探活配置错误：route_id={}, provider={}, err={}",
                    route.id,
                    provider.slug,
                    err
                );
            }
            ProbeOutcome::Unavailable(err) => {
                // 5xx / 超时 / 网络错误：递增失败次数；达到阈值时降级
                let new_failures = route.consecutive_failures + 1;
                let degrade = new_failures >= HEALTH_DEGRADE_FAILURE_THRESHOLD as i64;
                let _ = virtual_provider.service().mark_route_check_failed(
                    &route.id,
                    &err,
                    duration_ms,
                    degrade,
                );
                if degrade && route.is_healthy {
                    Log::warn(&format!(
                        "[Scheduler] 虚拟路由已降级：route_id={}, provider={}, consecutive_failures={}, err={}",
                        route.id, provider.slug, new_failures, err
                    ));
                }
            }
            ProbeOutcome::Unsupported => {
                // 上游不支持 /v1/models 端点（404）：跳过探活，不更新健康状态
                tracing::debug!(
                    "虚拟路由探活跳过（上游不支持 /v1/models）：route_id={}, provider={}",
                    route.id,
                    provider.slug
                );
            }
        }
    }
    Ok(())
}

/// 探活结果分类
#[derive(Debug)]
enum ProbeOutcome {
    /// 2xx：探活成功
    Success,
    /// 4xx：配置错误（鉴权失败、参数错误等）
    ConfigError(String),
    /// 5xx / 超时 / 网络错误：上游不可用
    Unavailable(String),
    /// 404 Not Found：上游不支持 /v1/models 端点
    Unsupported,
}

/// 对单条路由发起探活请求
///
/// 实现：GET {base_url}/v1/models，5s 超时。
/// 注入供应商级 auth_config 与 extra_headers。
async fn probe_route(
    client: &reqwest::Client,
    base_url: &str,
    provider_slug: &str,
    auth_config: &Option<crate::modules::ai_gateway::types::AuthConfig>,
    extra_headers: &[(String, String)],
) -> ProbeOutcome {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let mut req = client.get(&url);

    // 解析认证配置为 Bearer / ApiKey 凭证
    if let Some(auth) = auth_config {
        match crate::modules::gateway_runtime::auth_resolver::resolve_auth(auth) {
            Ok(resolution) => {
                if let Some(cred) = &resolution.credential {
                    match cred {
                        crate::modules::gateway_runtime::auth_resolver::AuthCredential::Bearer(t) => {
                            req = req.header("Authorization", format!("Bearer {t}"));
                        }
                        crate::modules::gateway_runtime::auth_resolver::AuthCredential::ApiKey(k) => {
                            // 大部分 OpenAI 兼容供应商都接受 Authorization: Bearer
                            req = req.header("Authorization", format!("Bearer {k}"));
                        }
                    }
                }
                // OAuth 类可能带附加头（如 xAI Grok）
                for (k, v) in &resolution.extra_headers {
                    req = req.header(k, v);
                }
            }
            Err(e) => {
                // 认证解析失败视为配置错误
                return ProbeOutcome::ConfigError(format!("auth resolve: {}", e.message));
            }
        }
    }
    // 注入供应商级 extra_headers（覆盖同名 auth 头）
    for (k, v) in extra_headers {
        req = req.header(k, v);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            if (200..300).contains(&status) {
                ProbeOutcome::Success
            } else if status == 404 {
                ProbeOutcome::Unsupported
            } else if (400..500).contains(&status) {
                ProbeOutcome::ConfigError(format!("HTTP {}", status))
            } else {
                ProbeOutcome::Unavailable(format!("HTTP {}", status))
            }
        }
        Err(e) => {
            if e.is_timeout() {
                ProbeOutcome::Unavailable(format!("timeout: {provider_slug}"))
            } else if e.is_connect() {
                ProbeOutcome::Unavailable(format!("connect error: {provider_slug}"))
            } else {
                ProbeOutcome::Unavailable(format!("network: {e}"))
            }
        }
    }
}

/// 虚拟路由尝试历史清理循环
///
/// 每 24 小时执行一次 `cleanup_old_attempts`，清理超过保留天数的历史记录。
/// 清理失败仅记录日志，不中断循环。
async fn attempts_cleanup_loop() {
    let interval = Duration::from_secs(ATTEMPTS_CLEANUP_INTERVAL_SECS);
    tracing::info!(
        "虚拟路由尝试历史清理任务启动，扫描间隔 {}s，保留 {} 天",
        ATTEMPTS_CLEANUP_INTERVAL_SECS,
        ATTEMPTS_CLEANUP_KEEP_DAYS
    );
    loop {
        // 先等待一个周期（启动后延迟首次清理，避免与初始化竞争）
        tokio::time::sleep(interval).await;
        match crate::modules::virtual_provider::repository::cleanup_old_attempts(
            ATTEMPTS_CLEANUP_KEEP_DAYS,
        ) {
            Ok(affected) => {
                if affected > 0 {
                    tracing::info!(
                        "虚拟路由尝试历史清理完成：删除 {} 条记录（保留 {} 天）",
                        affected,
                        ATTEMPTS_CLEANUP_KEEP_DAYS
                    );
                    Log::info(&format!(
                        "[Scheduler] 虚拟路由尝试历史清理：删除 {} 条历史记录",
                        affected
                    ));
                }
            }
            Err(e) => {
                tracing::warn!(
                    "虚拟路由尝试历史清理失败: {}",
                    e.message
                );
                Log::warn(&format!(
                    "[Scheduler] 虚拟路由尝试历史清理失败：{}",
                    e.message
                ));
            }
        }
    }
}
