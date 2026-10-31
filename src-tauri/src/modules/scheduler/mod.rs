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
//!   且（is_healthy=0 OR consecutive_failures>0）的路由，冷却期内的路由跳过；
//!   其余对目标模型发起**模型级最小请求**（OpenAI 兼容 `chat/completions`、
//!   Anthropic `messages`，`max_tokens=1`），不支持的协议退化为 `/v1/models` 存在性校验。
//!   连续成功 2 次后置 is_healthy=1；命中额度/鉴权/模型/限流/5xx 时按判定写冷却。
//! - **虚拟路由尝试历史清理**：每 24 小时清理 `virtual_route_attempts` 表中
//!   超过保留天数（默认 3 天，可在虚拟供应商「配置」中调整）的历史记录。
//! - **模型调用聚合统计窗口重算**：每 10 分钟从 `model_call_logs` 明细表
//!   重新计算「当前时刻 − 10 分钟」所在小时/天桶，覆盖式写回
//!   `model_call_stats_hourly` / `model_call_stats_daily`（聚合与明细严格同源）。

use std::time::{Duration, Instant};

use tauri::async_runtime::{self, JoinHandle};

use crate::error::IcodeResult;
use crate::modules::ai_gateway::types::AuthMethod;
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::logger::Log;
use crate::modules::virtual_provider::probe::{probe_route, ProbeOutcome};
use crate::modules::virtual_provider::settings;
use crate::modules::virtual_provider::VirtualProviderHandle;

/// OAuth 续期扫描间隔（秒）
const OAUTH_REFRESH_INTERVAL_SECS: u64 = 600; // 10 分钟

/// OAuth 续期阈值：过期前 2 小时内触发续期
const OAUTH_REFRESH_THRESHOLD_SECS: i64 = 2 * 3600;

/// 单次探活请求超时（秒）
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 5;

/// 虚拟路由尝试历史清理间隔（秒）：每 24 小时清理一次
const ATTEMPTS_CLEANUP_INTERVAL_SECS: u64 = 24 * 3600;

/// 模型调用聚合统计窗口重算间隔（秒）
///
/// 与 `call_records::repository::AGG_STATS_REBUILD_INTERVAL_SECS` 保持一致：
/// 每次任务重算「当前时刻 − 该间隔」所在的小时/天桶。
const STATS_REBUILD_INTERVAL_SECS: u64 = 600;

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
        tasks.push(async_runtime::spawn(stats_rebuild_loop()));
        tracing::info!(
            "Scheduler 模块初始化完成（OAuth 续期、虚拟路由健康检查、尝试历史清理、聚合统计重算任务已启动）"
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
/// 冷却期内的路由跳过；其余对目标模型发起模型级探活，首次成功即恢复健康。
/// 扫描间隔从 `virtual_provider.settings` 动态读取（默认 1h），修改后下一轮生效。
async fn health_check_loop(
    ai_gateway: AiGatewayServiceHandle,
    virtual_provider: VirtualProviderHandle,
) {
    tracing::info!(
        "虚拟路由健康检查任务启动，扫描间隔动态读取（默认 {}s），超时 {}s",
        settings::DEFAULT_HEALTH_CHECK_INTERVAL_SECS,
        HEALTH_CHECK_TIMEOUT_SECS,
    );
    loop {
        // 每个周期重新读取间隔，支持运行时修改
        let interval = Duration::from_secs(settings::health_check_interval_secs());
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
        // 冷却期内不探活：避免"探活失败 → 冷却顺延"导致永久无法恢复
        let now = chrono::Utc::now().to_rfc3339();
        if route
            .cooldown_until
            .as_deref()
            .map_or(false, |until| until > now.as_str())
        {
            continue;
        }

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
        let result = probe_route(
            &client,
            &provider,
            &route.target_model_id,
            &auth_config,
            &extra_headers,
        )
        .await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            ProbeOutcome::Success => {
                // 首次探活成功即恢复健康（探活间隔较长，连续成功会显著拖延恢复）
                let _ = virtual_provider
                    .service()
                    .mark_route_healthy(&route.id, duration_ms);
                Log::info(&format!(
                    "[Scheduler] 虚拟路由探活恢复：route_id={}, provider={}, model={}, duration={}ms",
                    route.id, provider.slug, route.target_model_id, duration_ms
                ));
            }
            ProbeOutcome::Failure(failure) => {
                let _ = virtual_provider.service().degrade_route_health(
                    &route.id,
                    failure.verdict.kind,
                    failure.verdict.penalty,
                    &failure.message,
                );
                Log::warn(&format!(
                    "[Scheduler] 虚拟路由探活失败，已按判定降级：route_id={}, provider={}, model={}, status={:?}, kind={}, err={}",
                    route.id,
                    provider.slug,
                    route.target_model_id,
                    failure.status,
                    failure.verdict.kind.as_str(),
                    failure.message
                ));
            }
            ProbeOutcome::Inconclusive(err) => {
                let _ = virtual_provider.service().mark_route_check_failed(
                    &route.id,
                    &err,
                    duration_ms,
                    false,
                );
                tracing::warn!(
                    "虚拟路由探活结果无法判定（不降级）：route_id={}, provider={}, err={}",
                    route.id,
                    provider.slug,
                    err
                );
            }
            ProbeOutcome::Unsupported => {
                // 上游不支持可用的探活端点：跳过，不更新健康状态
                tracing::debug!(
                    "虚拟路由探活跳过（上游不支持探活端点）：route_id={}, provider={}",
                    route.id,
                    provider.slug
                );
            }
        }
    }
    Ok(())
}

/// 虚拟路由尝试历史清理循环
///
/// 每 24 小时执行一次 `cleanup_old_attempts`，清理超过保留天数的历史记录。
/// 清理失败仅记录日志，不中断循环。
async fn attempts_cleanup_loop() {
    let interval = Duration::from_secs(ATTEMPTS_CLEANUP_INTERVAL_SECS);
    tracing::info!(
        "虚拟路由尝试历史清理任务启动，扫描间隔 {}s，保留天数取配置（默认 {} 天）",
        ATTEMPTS_CLEANUP_INTERVAL_SECS,
        settings::DEFAULT_ATTEMPTS_KEEP_DAYS
    );
    loop {
        // 先等待一个周期（启动后延迟首次清理，避免与初始化竞争）
        tokio::time::sleep(interval).await;
        // 每轮动态读取保留天数，配置修改后无需重启即生效
        let keep_days = settings::attempts_keep_days();
        match crate::modules::virtual_provider::repository::cleanup_old_attempts(keep_days) {
            Ok(affected) => {
                if affected > 0 {
                    tracing::info!(
                        "虚拟路由尝试历史清理完成：删除 {} 条记录（保留 {} 天）",
                        affected,
                        keep_days
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

/// 模型调用聚合统计窗口重算循环
///
/// 每 [`STATS_REBUILD_INTERVAL_SECS`] 秒从 `model_call_logs` 明细表重新计算
/// 「当前时刻 − 该间隔」所在的小时/天桶并覆盖写回聚合表，保证「聚合汇总」
/// 与「明细汇总」严格同源一致（以明细为准，无增量累加导致的计数偏差）。
/// 重算失败仅记录日志，不中断循环。
async fn stats_rebuild_loop() {
    let interval = Duration::from_secs(STATS_REBUILD_INTERVAL_SECS);
    tracing::info!(
        "模型调用聚合统计重算任务启动，扫描间隔 {}s",
        STATS_REBUILD_INTERVAL_SECS
    );
    loop {
        // 先等待一个周期（启动后延迟首次重算，配合「当前时刻 − 周期」窗口，
        // 保证首个周期内完成的流式请求 usage 已全部落库）
        tokio::time::sleep(interval).await;
        match crate::modules::call_records::repository::rebuild_windowed_stats() {
            Ok(()) => {
                tracing::debug!("模型调用聚合统计窗口重算完成");
            }
            Err(e) => {
                tracing::error!("模型调用聚合统计窗口重算失败: {}", e.message);
                Log::error(&format!(
                    "[Scheduler] 模型调用聚合统计窗口重算失败：{}",
                    e.message
                ));
            }
        }
    }
}
