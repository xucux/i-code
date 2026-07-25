//! # 版本更新检查
//!
//! 通过 GitHub Releases 的 `latest.json` 检测是否有新版本，
//! 并将结果返回给前端展示更新弹窗。

use std::collections::HashMap;

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

/// 检查 GitHub Releases 是否有新版本
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<CheckUpdateResult, String> {
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
