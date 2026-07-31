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

/// 下载进度事件名，前端监听此事件获取下载字节数与总大小
const DOWNLOAD_PROGRESS_EVENT: &str = "update-download-progress";

/// 下载进度数据
#[derive(serde::Serialize, Clone)]
pub struct DownloadProgress {
    /// 已下载字节数
    pub downloaded: u64,
    /// 总字节数（未知时为 0）
    pub total: u64,
}

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

    tracing::info!("[check_update] current version: {}", current_version);
    tracing::info!("[check_update] fetching: {}", url);

    let client = crate::modules::shared::apply_global_proxy(
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

    tracing::info!(
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

/// 下载更新安装包并触发安装
///
/// 流程：
///   1. 通过 reqwest 流式下载安装包到系统临时目录
///   2. 下载过程中通过 `update-download-progress` 事件推送进度
///   3. 下载完成后根据平台打开/执行安装包
///
/// 参数：
///   - `url`：安装包下载地址（来自 latest.json platforms）
///   - `file_name`：保存的文件名（如 `i-code_0.0.3_x64-setup.exe`）
#[tauri::command]
pub async fn download_and_install_update(
    app: tauri::AppHandle,
    url: String,
    file_name: String,
) -> Result<(), String> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    tracing::info!("[download_update] 开始下载: {}", url);

    let client = crate::modules::shared::apply_global_proxy(
        reqwest::Client::builder().timeout(std::time::Duration::from_secs(300)),
    )
    .build()
    .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("请求安装包失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("下载失败: HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);

    // 保存到系统临时目录
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join(&file_name);

    tracing::info!("[download_update] 保存到: {:?}", file_path);

    let mut file = tokio::fs::File::create(&file_path)
        .await
        .map_err(|e| format!("创建文件失败: {}", e))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载中断: {}", e))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("写入文件失败: {}", e))?;
        downloaded += chunk.len() as u64;

        // 每 256KB 或下载完成时推送进度，避免事件过于频繁
        if total == 0 || downloaded % (256 * 1024) < chunk.len() as u64 || downloaded == total {
            let _ = app.emit(
                DOWNLOAD_PROGRESS_EVENT,
                DownloadProgress {
                    downloaded,
                    total,
                },
            );
        }
    }

    file.flush()
        .await
        .map_err(|e| format!("刷新文件缓冲失败: {}", e))?;
    drop(file);

    tracing::info!("[download_update] 下载完成，触发安装: {:?}", file_path);

    // 根据平台触发安装
    #[cfg(target_os = "windows")]
    {
        // Windows：NSIS exe 或 MSI，直接启动安装程序
        // NSIS 安装器会自动处理正在运行的实例
        std::process::Command::new(&file_path)
            .spawn()
            .map_err(|e| format!("启动安装程序失败: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        // macOS：.app.tar.gz → 解压后打开 .app
        // 先解压到临时目录
        let extract_dir = temp_dir.join("i-code-update");
        let _ = std::fs::remove_dir_all(&extract_dir);
        std::fs::create_dir_all(&extract_dir)
            .map_err(|e| format!("创建解压目录失败: {}", e))?;

        // 使用系统 tar 解压
        let status = std::process::Command::new("tar")
            .args(["xzf", file_path.to_str().unwrap(), "-C", extract_dir.to_str().unwrap()])
            .status()
            .map_err(|e| format!("解压失败: {}", e))?;

        if !status.success() {
            return Err("解压安装包失败".into());
        }

        // 查找 .app bundle 并打开
        let entries: Vec<_> = std::fs::read_dir(&extract_dir)
            .map_err(|e| format!("读取解压目录失败: {}", e))?
            .filter_map(|e| e.ok())
            .collect();

        let app_entry = entries
            .iter()
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .ends_with(".app")
            })
            .ok_or("未找到 .app 安装包")?;

        std::process::Command::new("open")
            .arg(app_entry.path())
            .spawn()
            .map_err(|e| format!("打开应用失败: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        // Linux：AppImage → 添加执行权限后运行
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&file_path)
            .map_err(|e| format!("读取文件权限失败: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&file_path, perms)
            .map_err(|e| format!("设置执行权限失败: {}", e))?;

        std::process::Command::new(&file_path)
            .spawn()
            .map_err(|e| format!("启动安装程序失败: {}", e))?;
    }

    Ok(())
}
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
            tracing::info!(
                "[update_check] 推送结果：has_update={}, is_beta={}",
                result.has_update,
                result.is_beta
            );
            let _ = app.emit(UPDATE_CHECK_RESULT_EVENT, &result);
        }
        Err(e) => {
            tracing::warn!("[update_check] 启动期检查失败：{}", e);
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
