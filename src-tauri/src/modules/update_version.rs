//! # 版本更新检查
//!
//! 通过 GitHub Releases 的 `latest.json` 检测是否有新版本，
//! 并将结果返回给前端展示更新弹窗。
//!
//! 启动期由 `main.rs` 调用 [`run_update_check_and_emit`] 异步拉取一次，
//! 无论是否有更新（或请求失败）都通过 `update-check-result` 事件推送给前端，
//! 前端据此控制标题栏更新图标的显示与自动关闭。

use std::collections::HashMap;
use tauri::Emitter;

/// 启动期检查完成后向后端推送的事件名，与前端 `BACKEND_EVENTS.UPDATE_CHECK_RESULT` 保持一致
const UPDATE_CHECK_RESULT_EVENT: &str = "update-check-result";

/// 更新检查响应（对应 GitHub Releases latest.json）
#[derive(serde::Serialize, serde::Deserialize)]
struct UpdateInfo {
    version: String,
    notes: String,
    #[serde(rename = "pub_date")]
    pub_date: String,
    platforms: HashMap<String, PlatformEntry>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PlatformEntry {
    signature: String,
    url: String,
}

/// 检查更新结果（返回给前端）
#[derive(serde::Serialize)]
pub struct CheckUpdateResult {
    /// 是否需要更新
    pub has_update: bool,
    /// 最新版本是否为预览版（含 -beta 后缀）
    pub is_beta: bool,
    /// 当前版本号
    pub current_version: String,
    /// 最新版本号
    pub latest_version: String,
    /// 更新日志
    pub notes: String,
    /// 发布日期
    pub pub_date: String,
    /// 各平台安装包信息
    pub platforms: HashMap<String, PlatformEntry>,
}

/// 去除版本号的 v 前缀
fn strip_v(v: &str) -> &str {
    v.strip_prefix('v').unwrap_or(v)
}

/// 版本号比较：返回 true 表示 remote > local
/// 自动处理 v 前缀（如 v0.0.1、0.0.1-beta1）
fn is_newer_version(remote: &str, local: &str) -> bool {
    fn parse(v: &str) -> (Vec<u32>, String) {
        let v = strip_v(v);
        if let Some(idx) = v.find('-') {
            let parts = v[..idx].split('.').filter_map(|s| s.parse().ok()).collect();
            let pre = v[idx + 1..].to_string();
            (parts, pre)
        } else {
            let parts = v.split('.').filter_map(|s| s.parse().ok()).collect();
            (parts, String::new())
        }
    }

    let (r_parts, r_pre) = parse(remote);
    let (l_parts, l_pre) = parse(local);

    for i in 0..r_parts.len().max(l_parts.len()) {
        let rp = r_parts.get(i).copied().unwrap_or(0);
        let lp = l_parts.get(i).copied().unwrap_or(0);
        if rp != lp {
            return rp > lp;
        }
    }

    // pre-release 版本低于正式版
    if r_pre.is_empty() && !l_pre.is_empty() {
        return true;
    }
    if !r_pre.is_empty() && l_pre.is_empty() {
        return false;
    }
    r_pre > l_pre
}

/// 检查版本号是否包含 beta 标识
fn is_beta_version(version: &str) -> bool {
    version.to_lowercase().contains("beta")
}

/// 读取全局代理配置并应用到 reqwest ClientBuilder
///
/// 从 `app_settings` 表读取 `global_proxy_enabled` 和 `global_proxy_json`，
/// 根据代理类型配置客户端：
/// - `custom`：使用自定义代理 URL
/// - `system` / `vscode`：使用系统环境变量代理（reqwest 默认行为）
/// - `direct`：显式禁用代理
fn apply_global_proxy(builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    let settings = match crate::modules::settings::repository::find() {
        Ok(s) => s,
        Err(_) => return builder,
    };
    if !settings.global_proxy_enabled {
        return builder;
    }
    let Some(json) = settings.global_proxy_json.as_deref() else {
        return builder;
    };
    let cfg: crate::modules::shared::ProxyConfig = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(_) => return builder,
    };
    match cfg.proxy_type {
        crate::modules::shared::ProxyType::Direct => builder.no_proxy(),
        crate::modules::shared::ProxyType::Custom => {
            if let Some(url) = cfg.url.as_deref().filter(|s| !s.is_empty()) {
                if let Ok(proxy) = reqwest::Proxy::all(url) {
                    builder.proxy(proxy)
                } else {
                    builder
                }
            } else {
                builder
            }
        }
        // `system` / `vscode`：沿用 reqwest 默认行为（读取系统环境变量代理）
        _ => builder,
    }
}

/// 执行一次更新检查的核心逻辑（供 Command 与启动期事件推送复用）
///
/// 拉取 GitHub Releases 的 `latest.json`，与当前版本比较，返回结构化结果。
async fn check_update_internal(app: &tauri::AppHandle) -> Result<CheckUpdateResult, String> {
    let url = "https://github.com/xucux/i-code/releases/latest/download/latest.json";
    let current_version = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| "0.0.1".into());

    log::info!("[check_update] current version: {}", current_version);
    log::info!("[check_update] fetching: {}", url);

    let client = apply_global_proxy(
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(15)),
    )
    .build()
    .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("请求 latest.json 失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let update: UpdateInfo = resp
        .json()
        .await
        .map_err(|e| format!("解析 latest.json 失败: {}", e))?;

    let has_update = is_newer_version(&update.version, &current_version);
    let is_beta = is_beta_version(&update.version);

    log::info!(
        "[check_update] remote: {}, current: {}, has_update: {}, is_beta: {}",
        update.version,
        current_version,
        has_update,
        is_beta
    );

    Ok(CheckUpdateResult {
        has_update,
        is_beta,
        current_version,
        latest_version: update.version,
        notes: update.notes,
        pub_date: update.pub_date,
        platforms: update.platforms,
    })
}

/// 检查 GitHub Releases 是否有新版本（前端手动触发）
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<CheckUpdateResult, String> {
    check_update_internal(&app).await
}

/// 启动期异步检查更新并通过事件把结果推送给前端
///
/// 无论是否有更新（甚至请求失败）都会推送 `update-check-result` 事件，
/// 前端依据 `has_update` / `is_beta` 决定是否在标题栏展示更新入口：
/// - `has_update=true` 且 `is_beta=false`：展示更新 icon；
/// - `has_update=false` 或 `is_beta=true`：关闭更新 icon（实现自动关闭）。
///
/// 请求失败时推送一个「无更新」结果，确保前端能可靠关闭更新图标。
pub async fn run_update_check_and_emit(app: tauri::AppHandle) {
    let current_version = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| "0.0.1".into());
    match check_update_internal(&app).await {
        Ok(result) => {
            log::info!(
                "[update_check] 推送结果：has_update={}, is_beta={}",
                result.has_update,
                result.is_beta
            );
            let _ = app.emit(UPDATE_CHECK_RESULT_EVENT, &result);
        }
        Err(e) => {
            log::warn!("[update_check] 启动期检查失败：{}", e);
            let fallback = CheckUpdateResult {
                has_update: false,
                is_beta: false,
                current_version: current_version.clone(),
                latest_version: current_version,
                notes: String::new(),
                pub_date: String::new(),
                platforms: HashMap::new(),
            };
            let _ = app.emit(UPDATE_CHECK_RESULT_EVENT, &fallback);
        }
    }
}
