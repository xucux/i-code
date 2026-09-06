//! 图床浏览器窗口生命周期与标题桥接轮询
//!
//! ## 数据回传机制
//!
//! 注入脚本在用户点击「复制 Markdown / 直链」成功后，把外链写入
//! `document.title = "ICODE_IMGBED:<markdown>"`（超长时只写 `LONG` 标记）；
//! 本模块的轮询线程周期性读取当前窗口标题，解析前缀后通过 Tauri 事件
//! `imagebed:link-ready` 推送给主界面前端（社区编辑器自动插入光标处）。
//!
//! ## 为什么用 document.title 桥接
//!
//! 远程图床页面无法调用 Tauri IPC（无 `__TAURI__`），且 `Webview::eval` 无返回值，
//! 无法从页面取数据；文档标题是 WebView 侧唯一能被 Rust 读到的页面状态
//! （`WebviewWindow::title()`），配合剪贴板兜底形成双向冗余。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{Emitter, Manager};

use crate::error::{IcodeError, IcodeResult};

use super::types::{ImagebedLinkReady, ImagebedProviderSpec};

/// 图床浏览器窗口 label（固定，便于复用与查找）
pub const IMAGEBED_LABEL: &str = "imagebed-browser";
/// 注入脚本与主程序约定的 title 桥接前缀
pub const BRIDGE_PREFIX: &str = "ICODE_IMGBED:";
/// 超长外链的降级标记（此时外链仅进剪贴板，不解析为直链事件）
pub const BRIDGE_LONG_MARKER: &str = "LONG";
/// 标题桥接轮询间隔
const POLL_INTERVAL: Duration = Duration::from_millis(800);
/// 前端 BACKEND_EVENTS.IMAGEBED_LINK_READY 对应的 Tauri 事件名
pub const EVENT_LINK_READY: &str = "imagebed:link-ready";

/// imagebed 模块句柄（setup 阶段注入 Tauri State）
pub struct ImagebedHandle {
    app_handle: tauri::AppHandle,
    /// 当前打开的图床 provider id（窗口打开时写入，供轮询线程组装事件）
    active_provider: Mutex<Option<String>>,
    /// 轮询线程运行守卫：保证同时只有一个轮询线程
    poll_running: Arc<AtomicBool>,
}

impl ImagebedHandle {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            app_handle,
            active_provider: Mutex::new(None),
            poll_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 打开图床浏览器窗口；窗口已存在则直接显示并聚焦（复用 open_mini_panel 模式）
    pub fn open(&self, provider: &ImagebedProviderSpec) -> IcodeResult<()> {
        if let Some(window) = self.app_handle.get_webview_window(IMAGEBED_LABEL) {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
            return Ok(());
        }

        let url =
            tauri::Url::parse(provider.url).map_err(|e| IcodeError::validation(format!("图床地址非法：{e}")))?;

        // 通过 initialization_script 在 document-start 注入增强脚本
        //（Tauri 对每次 top-level 导航注入，远程页面同样生效）
        tauri::WebviewWindowBuilder::new(
            &self.app_handle,
            IMAGEBED_LABEL,
            tauri::WebviewUrl::External(url),
        )
        .title(format!("i-code 图床 · {}", provider.name))
        .inner_size(1100.0, 780.0)
        .min_inner_size(620.0, 480.0)
        .resizable(true)
        .center()
        // 覆盖默认 WebView2 UA，规避图床站点的 WebView/爬虫 UA 检测导致白屏
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .initialization_script(provider.inject_script)
        .build()
        .map_err(|e| IcodeError::internal(format!("创建图床窗口失败：{e}")))?;

        *self
            .active_provider
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(provider.id.to_string());

        self.spawn_polling();
        Ok(())
    }

    /// 关闭图床窗口（轮询线程因窗口销毁自动退出）
    pub fn close(&self) -> IcodeResult<()> {
        if let Some(window) = self.app_handle.get_webview_window(IMAGEBED_LABEL) {
            window
                .close()
                .map_err(|e| IcodeError::internal(format!("关闭图床窗口失败：{e}")))?;
        }
        Ok(())
    }

    /// 启动标题桥接轮询线程（幂等：仅第一次真正创建线程）
    fn spawn_polling(&self) {
        if self.poll_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let app = self.app_handle.clone();
        let running = self.poll_running.clone();
        let active_provider = Arc::new(
            self.active_provider
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone(),
        );
        std::thread::spawn(move || {
            // 上次已上报的 markdown，避免对同一直链重复推送
            let mut last_seen: Option<String> = None;
            loop {
                std::thread::sleep(POLL_INTERVAL);
                // 窗口已被销毁/关闭 → 退出轮询
                let Some(window) = app.get_webview_window(IMAGEBED_LABEL) else {
                    break;
                };
                let title = match window.title() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let Some(markdown) = title.strip_prefix(BRIDGE_PREFIX) else {
                    continue;
                };
                let markdown = markdown.to_string();
                // 站点覆盖标题导致重复出现等场景下，同值只推送一次
                if last_seen.as_deref() == Some(markdown.as_str()) {
                    continue;
                }
                last_seen = Some(markdown.clone());

                // LONG 标记仅剪贴板兜底（脚本已写剪贴板），此处不构造直链事件
                let Some(url) = extract_markdown_url(&markdown) else {
                    continue;
                };
                let provider_id = active_provider
                    .as_ref() // &Option<String>（Arc 解引用）
                    .as_ref() // Option<&String>
                    .cloned() // Option<String>
                    .unwrap_or_else(|| "remit".to_string());
                let payload = ImagebedLinkReady {
                    provider_id,
                    url,
                    markdown,
                    created_at: now_millis(),
                };
                let app2 = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = app2.emit(EVENT_LINK_READY, &payload);
                });
            }
            running.store(false, Ordering::SeqCst);
        });
    }
}

/// 从 `![alt](url)` 提取直链 URL；LONG 标记或非 markdown 形态返回 None
fn extract_markdown_url(markdown: &str) -> Option<String> {
    if markdown == BRIDGE_LONG_MARKER {
        return None;
    }
    // 取最后一个 "](" 到末尾 ")" 之间的内容（alt 内即使含括号也不影响尾部提取）
    let start = markdown.rfind("](")?;
    let inner = markdown.get(start + 2..)?;
    let end = inner.rfind(')')?;
    let url = &inner[..end];
    if url.is_empty() {
        return None;
    }
    Some(url.to_string())
}

/// 当前毫秒时间戳（事件 payload 用）
fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}