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

use std::time::Duration;

use tauri::async_runtime::{self, JoinHandle};

use crate::error::IcodeResult;
use crate::modules::ai_gateway::types::AuthMethod;
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::logger::Log;

/// OAuth 续期扫描间隔（秒）
const OAUTH_REFRESH_INTERVAL_SECS: u64 = 600; // 10 分钟

/// OAuth 续期阈值：过期前 2 小时内触发续期
const OAUTH_REFRESH_THRESHOLD_SECS: i64 = 2 * 3600;

/// 调度器句柄，持有所有后台任务句柄
///
/// 应用启动时由 `setup` 创建并 `app.manage`；Drop 时自动终止所有任务。
pub struct SchedulerHandle {
    tasks: Vec<JoinHandle<()>>,
}

impl SchedulerHandle {
    /// 创建调度器并启动所有内置任务
    pub fn new(ai_gateway: AiGatewayServiceHandle) -> Self {
        let mut tasks: Vec<JoinHandle<()>> = Vec::new();
        tasks.push(async_runtime::spawn(oauth_refresh_loop(ai_gateway)));
        log::info!("Scheduler 模块初始化完成（OAuth 续期任务已启动）");
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
    log::info!(
        "OAuth 续期任务启动，扫描间隔 {}s，阈值 {}s",
        OAUTH_REFRESH_INTERVAL_SECS,
        OAUTH_REFRESH_THRESHOLD_SECS
    );
    loop {
        // 先等待一个周期（启动后延迟首次扫描，避免与初始化竞争）
        tokio::time::sleep(interval).await;
        if let Err(e) = scan_and_refresh(&ai_gateway, OAUTH_REFRESH_THRESHOLD_SECS).await {
            log::error!("OAuth 续期扫描失败: {}", e);
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
            log::warn!(
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

        log::info!(
            "OAuth 续期：provider_id={}, name={}, method={}, expires_at={}",
            p.id, p.display_name, method_str, expires_str
        );

        match ai_gateway
            .service()
            .refresh_oauth_token(&p.id, method)
            .await
        {
            Ok(_) => {
                log::info!(
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
                log::error!(
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
