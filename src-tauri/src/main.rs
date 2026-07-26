// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod db;
mod error;
mod modules;

use std::sync::{Arc, Mutex};
use sysinfo::{get_current_pid, ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System};
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Listener, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_log::{Target, TargetKind};

/// 在系统默认浏览器中打开指定 URL
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    tauri_plugin_opener::open_url(&url, None::<&str>)
        .map_err(|e| format!("打开浏览器失败: {}", e))
}

// 供应商列表，用于托盘子菜单展示
const PROVIDERS: &[(&str, &str)] = &[
    ("openai", "OpenAI"),
    ("anthropic", "Anthropic"),
    ("gemini", "Google Gemini"),
    ("deepseek", "DeepSeek"),
];

/// 获取当前应用进程占用的物理内存（KB）
/// sysinfo 的 Process::memory() 返回字节数，此处除以 1024 转换为 KB
#[tauri::command]
fn get_memory_usage() -> u64 {
    let mut sys = System::new_with_specifics(
        RefreshKind::nothing().with_processes(ProcessRefreshKind::nothing().with_memory()),
    );
    let pid = get_current_pid().unwrap_or_else(|_| sysinfo::Pid::from(0));
    sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
    sys.process(pid).map(|p| p.memory() / 1024).unwrap_or(0)
}

/// 从额度快照中提取摘要文本（含百分比与用量/限额）
///
/// 解析 `BalanceSnapshot.items` 中的 percent 与 amount 类型指标，
/// 返回托盘菜单展示用的紧凑文本：
/// - 同时有周/月百分比 + 用量/限额：`周87% 月65% | $12.50/$100`
/// - 仅百分比：`周87% 月65%`
/// - 仅用量/限额：`$12.50/$100`
/// - 无指标：`暂无数据` / `已查询`
fn format_balance_summary(snapshot: &modules::balance::types::BalanceSnapshot) -> String {
    use modules::balance::types::{BalanceDirection, BalanceMetricType, BalancePeriod};

    let mut week_pct: Option<f64> = None;
    let mut month_pct: Option<f64> = None;
    let mut other_pct: Option<f64> = None;
    let mut used_amount: Option<(f64, Option<String>)> = None;
    let mut limit_amount: Option<(f64, Option<String>)> = None;

    for item in &snapshot.items {
        if item.metric_type == BalanceMetricType::Percent {
            let val = item
                .value
                .as_ref()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            match item.period {
                Some(BalancePeriod::Week) => week_pct = Some(val),
                Some(BalancePeriod::Month) => month_pct = Some(val),
                _ => other_pct = Some(val),
            }
        } else if item.metric_type == BalanceMetricType::Amount {
            let val = item
                .value
                .as_ref()
                .and_then(|v| v.as_f64());
            if let Some(v) = val {
                let currency = item.currency_symbol.clone();
                match item.direction {
                    Some(BalanceDirection::Used) => used_amount = Some((v, currency)),
                    Some(BalanceDirection::Limit) => limit_amount = Some((v, currency)),
                    _ => {}
                }
            }
        }
    }

    /// 格式化金额数值（保留最多 2 位小数，大数加千分位分隔）
    fn fmt_amount(val: f64) -> String {
        if val >= 1000.0 {
            let s = (val * 100.0).round() / 100.0;
            let whole = s.trunc() as i64;
            let dec = ((s.fract() * 100.0).round()) as u32;
            // 分组千分位
            let num_str = whole.to_string();
            let mut result = String::new();
            for (i, c) in num_str.chars().enumerate() {
                if i > 0 && (num_str.len() - i) % 3 == 0 {
                    result.push(',');
                }
                result.push(c);
            }
            if dec > 0 {
                result.push_str(&format!(".{:02}", dec));
            }
            result
        } else if val == val.trunc() {
            format!("{}", val as i64)
        } else {
            format!("{:.2}", val)
        }
    }

    /// 构建金额部分字符串
    fn build_amount_str(used: Option<(f64, Option<String>)>, limit: Option<(f64, Option<String>)>) -> Option<String> {
        match (used, limit) {
            (Some((u, _)), Some((l, _))) => {
                let u_str = fmt_amount(u);
                let l_str = fmt_amount(l);
                // 优先使用 used 的货币符号
                let sym = " ";
                Some(format!("{}{}/{}", sym, u_str, l_str))
            }
            (Some((u, _)), None) => Some(format!(" {}", fmt_amount(u))),
            (None, Some((l, _))) => Some(format!(" /{}", fmt_amount(l))),
            (None, None) => None,
        }
    }

    let pct_part = match (week_pct, month_pct) {
        (Some(w), Some(m)) => Some(format!("周{}% 月{}%", w as u32, m as u32)),
        (Some(w), None) => Some(format!("周{}%", w as u32)),
        (None, Some(m)) => Some(format!("月{}%", m as u32)),
        (None, None) => other_pct.map(|v| format!("{}%", v as u32)),
    };

    let amount_part = build_amount_str(used_amount, limit_amount);

    match (pct_part, amount_part) {
        (Some(p), Some(a)) => format!("{}{}", p, a),
        (Some(p), None) => p,
        (None, Some(a)) => a.trim().to_string(),
        (None, None) => {
            if snapshot.items.is_empty() {
                "暂无数据".to_string()
            } else {
                "已查询".to_string()
            }
        }
    }
}

/// 打开迷你面板窗口；若已存在则显示并聚焦
#[tauri::command]
async fn open_mini_panel(app: tauri::AppHandle) -> Result<(), String> {
    const LABEL: &str = "mini-panel";
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }

    let settings = load_mini_panel_settings(&app)?;
    // 将读取到的尺寸限制在合法范围内：
    //   下限 190 保证正常模式内容可读（最小化模式 52×180 由前端单独控制）
    //   上限 480 对应 max_inner_size，防止窗口超出设计范围
    let width = settings.width.clamp(190.0, 480.0);
    let height = settings.height.clamp(190.0, 480.0);

    tauri::WebviewWindowBuilder::new(
        &app,
        LABEL,
        tauri::WebviewUrl::App("mini-panel".into()),
    )
    .title("i-code Mini")
    .inner_size(width, height)           // 初始窗口尺寸，由持久化设置 + clamp 决定
    .min_inner_size(48.0, 80.0)          // 允许缩小到最小化竖长条尺寸
    .max_inner_size(480.0, 400.0)        // 正常模式最大尺寸
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(true)
    .center()
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 关闭迷你面板窗口，同时显示并聚焦主窗口
#[tauri::command]
async fn close_mini_panel(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("mini-panel") {
        window.close().map_err(|e| e.to_string())?;
    }
    // 显示并聚焦主窗口
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
    Ok(())
}

/// 迷你面板持久化设置（前端 localStorage 为主，此处仅作后备/默认值）
///
/// 首次打开迷你窗口时，前端将 DEFAULT_SETTINGS 写入 localStorage，
/// Rust 侧从 mini-panel-settings.json 读取；若该文件不存在才使用此处默认值。
/// 实际优先级：localStorage > mini-panel-settings.json > 下方的 default 函数
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct MiniPanelSettings {
    /// 窗口宽度（逻辑像素），首次默认取 default_mini_width()
    #[serde(default = "default_mini_width")]
    width: f64,
    /// 窗口高度（逻辑像素），首次默认取 default_mini_height()
    #[serde(default = "default_mini_height")]
    height: f64,
}

/// 迷你窗口后备默认宽度（逻辑像素）
/// 仅在 mini-panel-settings.json 不存在时生效；前端 DEFAULT_SETTINGS 优先级更高
fn default_mini_width() -> f64 {
    220.0
}

/// 迷你窗口后备默认高度（逻辑像素）
/// 仅在 mini-panel-settings.json 不存在时生效；前端 DEFAULT_SETTINGS 优先级更高
fn default_mini_height() -> f64 {
    210.0
}

fn load_mini_panel_settings(app: &tauri::AppHandle) -> Result<MiniPanelSettings, String> {
    if let Some(dir) = app.path().app_config_dir().ok() {
        let path = dir.join("mini-panel-settings.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            return serde_json::from_str(&content).map_err(|e| e.to_string());
        }
    }
    Ok(MiniPanelSettings {
        width: default_mini_width(),
        height: default_mini_height(),
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        // 系统浏览器/文件打开插件（替代已弃用的 Shell::open）
        .plugin(tauri_plugin_opener::init())
        // 进程管理插件：提供应用重启能力（备份恢复后使用）
        .plugin(tauri_plugin_process::init())
        // 开机自启插件：跨平台注册自启入口；传入 --autostart 参数用于启动时识别是否为自启调用
        .plugin(tauri_plugin_autostart::Builder::new()
            .args(["--autostart"])
            .build())
        // 日志插件：统一输出到终端、WebView 控制台、应用日志目录
        // level 设为 Trace，允许运行时通过 log::set_max_level 动态限制；
        // 实际过滤级别由 app_settings.log_level 决定，默认 Info。
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Webview),
                ])
                .level(log::LevelFilter::Trace)
                .max_file_size(1024 * 1024 * 20)
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepAll)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            modules::update_version::check_update,
            open_url,
            get_memory_usage,
            open_mini_panel,
            close_mini_panel,
            // ===== Secret 模块 =====
            modules::secret::commands::secret_save,
            modules::secret::commands::secret_update,
            modules::secret::commands::secret_delete,
            modules::secret::commands::secret_list,
            modules::secret::commands::secret_list_kinds,
            modules::secret::commands::secret_scan_references,
            modules::secret::commands::secret_cleanup_orphaned,
            modules::secret::commands::secret_decrypt_text,
            // ===== Settings 模块 =====
            modules::settings::commands::settings_get,
            modules::settings::commands::settings_update,
            modules::settings::commands::settings_log_dir,
            // ===== Balance 模块 =====
            modules::balance::commands::balance_refresh,
            modules::ai_gateway::commands::balance_refresh_provider,
            modules::ai_gateway::commands::balance_list_snapshots,
            modules::ai_gateway::commands::balance_delete_snapshot,
            // ===== Logger 模块 =====
            modules::logger::commands::log_list,
            modules::logger::commands::log_recent,
            modules::logger::commands::log_write,
            modules::logger::commands::log_clear,
            modules::logger::commands::log_count,
            modules::logger::commands::log_export,
            modules::logger::commands::log_message,
            modules::logger::commands::log_get_command_config,
            modules::logger::commands::log_set_command_config,
            modules::logger::commands::log_get_settings,
            modules::logger::commands::log_set_settings,
            // ===== Backup 模块 =====
            modules::backup::commands::backup_create,
            modules::backup::commands::backup_list_local,
            modules::backup::commands::backup_restore,
            modules::backup::commands::backup_delete,
            modules::backup::commands::backup_push_webdav,
            modules::backup::commands::backup_list_webdav,
            modules::backup::commands::backup_restore_webdav,
            modules::backup::commands::backup_delete_webdav,
            modules::backup::commands::backup_webdav_config_list,
            modules::backup::commands::backup_webdav_config_get,
            modules::backup::commands::backup_webdav_config_save,
            modules::backup::commands::backup_webdav_config_delete,
            // ===== AI Gateway 模块 =====
            modules::ai_gateway::commands::gateway_provider_list,
            modules::ai_gateway::commands::gateway_provider_get,
            modules::ai_gateway::commands::gateway_provider_create,
            modules::ai_gateway::commands::gateway_provider_update,
            modules::ai_gateway::commands::gateway_provider_delete,
            modules::ai_gateway::commands::gateway_provider_oauth_authorize,
            modules::ai_gateway::commands::gateway_provider_oauth_start,
            modules::ai_gateway::commands::gateway_provider_oauth_complete,
            modules::ai_gateway::commands::gateway_provider_oauth_device_code,
            modules::ai_gateway::commands::gateway_provider_oauth_poll_device_token,
            modules::ai_gateway::commands::gateway_provider_oauth_refresh_token,
            modules::ai_gateway::commands::gateway_provider_export,
            modules::ai_gateway::commands::gateway_provider_import,
            modules::ai_gateway::commands::gateway_model_config_create,
            modules::ai_gateway::commands::gateway_model_config_get,
            modules::ai_gateway::commands::gateway_model_config_list,
            modules::ai_gateway::commands::gateway_model_config_update,
            modules::ai_gateway::commands::gateway_model_config_delete,
            modules::ai_gateway::commands::gateway_model_create,
            modules::ai_gateway::commands::gateway_model_get,
            modules::ai_gateway::commands::gateway_model_list,
            modules::ai_gateway::commands::gateway_model_list_by_provider,
            modules::ai_gateway::commands::gateway_model_delete,
            modules::ai_gateway::commands::gateway_model_update,
            modules::ai_gateway::commands::gateway_exposed_models,
            modules::ai_gateway::commands::gateway_all_models,
            modules::ai_gateway::commands::gateway_builtin_providers_list,
            modules::ai_gateway::commands::gateway_builtin_models_list,
            modules::ai_gateway::commands::gateway_builtin_models_by_provider_type,
            modules::ai_gateway::commands::gateway_fetch_official_models,
            modules::ai_gateway::commands::gateway_fetch_models_by_protocol,
            modules::ai_gateway::commands::gateway_settings_get,
            modules::ai_gateway::commands::gateway_settings_update,
            modules::ai_gateway::commands::gateway_auth_key_create,
            modules::ai_gateway::commands::gateway_auth_key_update,
            modules::ai_gateway::commands::gateway_auth_key_delete,
            modules::ai_gateway::commands::gateway_auth_key_list,
            // ===== CLI Management 模块 =====
            modules::cli_management::commands::cli_profile_list,
            modules::cli_management::commands::cli_profile_ensure_defaults,
            modules::cli_management::commands::cli_config_inspect,
            modules::cli_management::commands::cli_profile_get,
            modules::cli_management::commands::cli_profile_create,
            modules::cli_management::commands::cli_profile_update,
            modules::cli_management::commands::cli_profile_delete,
            modules::cli_management::commands::cli_provider_list,
            modules::cli_management::commands::cli_provider_get,
            modules::cli_management::commands::cli_provider_create,
            modules::cli_management::commands::cli_provider_update,
            modules::cli_management::commands::cli_provider_delete,
            modules::cli_management::commands::cli_model_mapping_list,
            modules::cli_management::commands::cli_model_mapping_create,
            modules::cli_management::commands::cli_model_mapping_update,
            modules::cli_management::commands::cli_model_mapping_delete,
            modules::cli_management::commands::cli_config_read,
            modules::cli_management::commands::cli_config_save,
            modules::cli_management::commands::cli_client_check,
            // ===== Workspace 模块 =====
            modules::workspace::commands::workspace_list,
            modules::workspace::commands::workspace_get,
            modules::workspace::commands::workspace_get_active,
            modules::workspace::commands::workspace_create,
            modules::workspace::commands::workspace_update,
            modules::workspace::commands::workspace_delete,
            modules::workspace::commands::workspace_switch,
            modules::workspace::commands::workspace_cli_config_list,
            modules::workspace::commands::workspace_prompt_list,
            modules::workspace::commands::workspace_prompt_get,
            modules::workspace::commands::workspace_prompt_create,
            modules::workspace::commands::workspace_prompt_update,
            modules::workspace::commands::workspace_prompt_delete,
            modules::workspace::commands::workspace_mcp_server_list,
            modules::workspace::commands::workspace_mcp_server_get,
            modules::workspace::commands::workspace_mcp_server_create,
            modules::workspace::commands::workspace_mcp_server_update,
            modules::workspace::commands::workspace_mcp_server_delete,
            modules::workspace::commands::workspace_skill_list,
            modules::workspace::commands::workspace_skill_get,
            modules::workspace::commands::workspace_skill_create,
            modules::workspace::commands::workspace_skill_update,
            modules::workspace::commands::workspace_skill_delete,
            modules::workspace::commands::workspace_apply,
            modules::workspace::commands::workspace_aggregate,
            modules::workspace::commands::workspace_preview,
            modules::workspace::commands::workspace_apply_cli_config,
            // ===== Script Template 模块 =====
            modules::script_template::commands::script_template_list,
            modules::script_template::commands::script_template_get,
            modules::script_template::commands::script_template_create,
            modules::script_template::commands::script_template_update,
            modules::script_template::commands::script_template_delete,
            modules::script_template::commands::script_template_set_status,
            modules::script_template::commands::script_template_test,
            modules::script_template::commands::script_template_list_active_for_select,
            modules::script_template::commands::script_template_list_snippets,
            modules::script_template::commands::script_template_list_refs,
            // ===== Virtual Provider 模块 =====
            modules::virtual_provider::commands::virtual_provider_list,
            modules::virtual_provider::commands::virtual_provider_get,
            modules::virtual_provider::commands::virtual_provider_create,
            modules::virtual_provider::commands::virtual_model_save,
            modules::virtual_provider::commands::virtual_provider_update,
            modules::virtual_provider::commands::virtual_provider_delete,
            modules::virtual_provider::commands::virtual_provider_model_list,
            modules::virtual_provider::commands::virtual_provider_model_get,
            modules::virtual_provider::commands::virtual_provider_model_create,
            modules::virtual_provider::commands::virtual_provider_model_update,
            modules::virtual_provider::commands::virtual_provider_model_delete,
            modules::virtual_provider::commands::virtual_provider_route_list,
            modules::virtual_provider::commands::virtual_provider_routes_by_provider,
            modules::virtual_provider::commands::virtual_provider_route_get,
            modules::virtual_provider::commands::virtual_provider_route_create,
            modules::virtual_provider::commands::virtual_provider_route_update,
            modules::virtual_provider::commands::virtual_provider_route_delete,
            // ===== Call Records 模块 =====
            modules::call_records::commands::call_records_list,
            modules::call_records::commands::call_records_get,
            modules::call_records::commands::gateway_model_call_stats,
            modules::call_records::commands::call_stats_aggregated,
            modules::call_records::commands::call_records_clear_stats,
            modules::call_records::commands::call_records_today_tokens,
            // ===== Tokenizer 模块 =====
            modules::tokenizer::commands::tokenizer_list,
            modules::tokenizer::commands::tokenizer_count,
            modules::tokenizer::commands::tokenizer_count_messages,
            // ===== Chat 模块 =====
            modules::chat::commands::chat_session_list,
            modules::chat::commands::chat_session_get,
            modules::chat::commands::chat_session_create,
            modules::chat::commands::chat_session_update,
            modules::chat::commands::chat_session_delete,
            modules::chat::commands::chat_message_send,
            modules::chat::commands::chat_message_abort,
            // ===== Gateway Runtime 模块 =====
            modules::gateway_runtime::commands::gateway_start,
            modules::gateway_runtime::commands::gateway_stop,
            modules::gateway_runtime::commands::gateway_status,
            modules::gateway_runtime::commands::gateway_health,
            modules::gateway_runtime::commands::gateway_get_forward_log_config,
            modules::gateway_runtime::commands::gateway_set_forward_log_config,
        ])
        .on_window_event(|window, event| {
            // 主窗口关闭时隐藏到系统托盘，而非退出进程
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                    log::info!("主窗口关闭请求 → 隐藏到系统托盘");
                }
            }
        })
        .setup(|app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").expect("main window not found");
                window.open_devtools();
            }

            // ===== 初始化数据库 =====
            // 1. 解析数据库路径：使用 Tauri 应用配置目录（如 Windows %APPDATA%/com.icode.app）
            // 2. 在该目录下创建 i-code.db 文件
            // 3. 初始化 r2d2 连接池（含 WAL 模式、外键约束等 PRAGMA）
            // 4. 执行未应用的数据库迁移（按 schema_migrations 表版本判断）
            let app_config_dir = app
                .path()
                .app_config_dir()
                .expect("无法获取应用配置目录");
            let db_path = app_config_dir.join("i-code.db");
            db::init_db_pool(&db_path).expect("数据库连接池初始化失败");
            db::run_migrations().expect("数据库迁移执行失败");
            log::info!("数据库初始化完成：{}", db_path.display());

            // ===== 初始化 Settings 模块 =====
            // Settings 无需启动期加载缓存，每次调用直接查库。
            // 必须在 Secret 模块之前初始化，因为 Secret 需要从中读取通用密码派生 AES 密钥。
            let settings_handle = modules::settings::SettingsServiceHandle::new();
            // 应用用户设置的全局日志级别（默认 Info）到 tauri-plugin-log
            match settings_handle.service().get_settings() {
                Ok(settings) => {
                    let level_filter = settings.log_level.to_level_filter();
                    log::set_max_level(level_filter);
                    log::info!("Settings 模块初始化完成，日志级别：{}", settings.log_level.as_str());
                }
                Err(e) => {
                    log::set_max_level(log::LevelFilter::Info);
                    log::warn!("读取设置失败，使用默认日志级别 Info：{}", e.message);
                }
            }
            app.manage(settings_handle.clone());

            // ===== 初始化 Secret 模块 =====
            // 1. 通用密码来自 Settings.config_key，经 SHA-256 派生为 AES-256-GCM 主密钥。
            // 2. 构造 SecretServiceHandle 并注册为 Tauri State，供 Commands 访问。
            let secret_handle =
                modules::secret::SecretServiceHandle::new(settings_handle.clone())
                    .expect("Secret 服务初始化失败");
            log::info!("Secret 模块初始化完成");
            // 克隆句柄给 backup 与 ai_gateway 模块使用（原句柄注册为 Tauri State）
            let secret_handle_for_backup = secret_handle.clone();
            let secret_handle_for_ai_gateway = secret_handle.clone();
            app.manage(secret_handle);

            // ===== 初始化 Balance 模块 =====
            // Balance 无状态，直接构造 Handle 注册为 Tauri State。
            let balance_handle = modules::balance::BalanceServiceHandle::new();
            log::info!("Balance 模块初始化完成");
            app.manage(balance_handle);

            // ===== 初始化 Logger 模块 =====
            // Logger 使用内存环形缓冲区，默认保留 10000 条日志。
            let logger_handle = modules::logger::LoggerServiceHandle::with_default();
            // 注册到全局，供无法获取 AppHandle 的后端代码通过 Log::info 等写入自研 logger
            modules::logger::set_global_logger_handle(logger_handle.clone());
            log::info!("Logger 模块初始化完成");
            // 克隆句柄给 gateway_runtime 模块使用
            let logger_handle_for_gateway = logger_handle.clone();
            app.manage(logger_handle);

            // ===== 初始化 Backup 模块 =====
            // Backup 需要数据库路径、本地备份目录、安全备份目录、应用版本号、schema 版本号、
            // 以及 Secret 服务句柄（用于恢复后扫描缺失的 Secret 引用）。
            // 本地备份目录优先使用程序运行目录下的 backup/，取不到可执行路径时回退到应用配置目录。
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| app_config_dir.clone());
            let backups_dir = exe_dir.join("backup");
            let safety_backups_dir = app_config_dir.join("safety-backups");
            let backup_handle = modules::backup::BackupServiceHandle::new(
                db_path.clone(),
                backups_dir,
                safety_backups_dir,
                env!("CARGO_PKG_VERSION").to_string(),
                db::SCHEMA_VERSION,
                secret_handle_for_backup,
            );
            log::info!("Backup 模块初始化完成");
            app.manage(backup_handle);

            // ===== 初始化 AI Gateway 模块 =====
            // AI Gateway 依赖 Secret 模块加密 API Key 等敏感字段。
            // 同时再 clone 一份 Secret 句柄给 gateway_runtime（解析 Gateway Key 明文）。
            let secret_handle_for_gateway = secret_handle_for_ai_gateway.clone();
            let ai_gateway_handle =
                modules::ai_gateway::AiGatewayServiceHandle::new(secret_handle_for_ai_gateway);
            log::info!("AI Gateway 模块初始化完成");
            app.manage(ai_gateway_handle.clone());

            // ===== 初始化 CLI Management 模块 =====
            // CLI Management 依赖 AI Gateway 校验 provider_id 存在性。
            let cli_management_handle =
                modules::cli_management::CliManagementServiceHandle::new(ai_gateway_handle.clone());
            log::info!("CLI Management 模块初始化完成");
            // 克隆句柄给 workspace 模块使用（原句柄注册为 Tauri State）
            let cli_management_handle_for_workspace = cli_management_handle.clone();
            app.manage(cli_management_handle);

            // ===== 初始化 Workspace 模块 =====
            // Workspace 依赖 CLI Management 获取 CLI 档案列表与路径。
            let workspace_handle = modules::workspace::WorkspaceServiceHandle::new(
                cli_management_handle_for_workspace,
            );
            log::info!("Workspace 模块初始化完成");
            app.manage(workspace_handle);

            // ===== 初始化 Script Template 模块 =====
            // 依赖 AI Gateway 做试运行时的 Secret 解密与供应商加载。
            let script_template_handle =
                modules::script_template::ScriptTemplateHandle::new(ai_gateway_handle.clone());
            log::info!("Script Template 模块初始化完成");
            app.manage(script_template_handle);

            // ===== 初始化 Virtual Provider 模块 =====
            // Virtual Provider 无启动期依赖，直接构造句柄。
            let virtual_provider_handle = modules::virtual_provider::VirtualProviderHandle::new();
            log::info!("Virtual Provider 模块初始化完成");
            app.manage(virtual_provider_handle.clone());

            // ===== 初始化 Call Records 模块 =====
            // Call Records 直接操作数据库，无需额外依赖。
            let call_records_handle = modules::call_records::CallRecordsHandle::new();
            log::info!("Call Records 模块初始化完成");
            app.manage(call_records_handle.clone());

            // ===== 初始化 Gateway Runtime 模块 =====
            // Gateway Runtime 依赖 AI Gateway（供应商/模型列表、监听地址、Gateway Key）、
            // Secret（Gateway Key 解析）、Logger（请求日志）、Virtual Provider（故障转移）、
            // Call Records（调用记录持久化）五个模块。
            let gateway_runtime_handle = modules::gateway_runtime::GatewayRuntimeHandle::new(
                app.handle().clone(),
                ai_gateway_handle.clone(),
                secret_handle_for_gateway.clone(),
                logger_handle_for_gateway,
                virtual_provider_handle,
                call_records_handle,
            );
            log::info!("Gateway Runtime 模块初始化完成");
            app.manage(gateway_runtime_handle.clone());

            // ===== 初始化 Chat 模块 =====
            // 会话 JSONL 存放于程序运行目录下的 chat/；请求经本地网关转发。
            let chat_dir = exe_dir.join("chat");
            let chat_handle = modules::chat::ChatServiceHandle::new(
                chat_dir,
                app.handle().clone(),
                gateway_runtime_handle,
                ai_gateway_handle,
                secret_handle_for_gateway,
            )
            .expect("Chat 服务初始化失败");
            log::info!("Chat 模块初始化完成");
            app.manage(chat_handle);

            // 创建系统托盘菜单
            let show_i = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;

            // 供应商子菜单：当前仅作为组件库展示结构，选中事件可供后续业务逻辑订阅
            let provider_items: Vec<MenuItem<_>> = PROVIDERS
                .iter()
                .map(|(id, label)| {
                    MenuItem::with_id(app, format!("provider:{id}"), *label, true, None::<&str>)
                        .expect("failed to create provider menu item")
                })
                .collect();
            let provider_refs: Vec<&dyn IsMenuItem<_>> = provider_items
                .iter()
                .map(|item| item as &dyn IsMenuItem<_>)
                .collect();
            let provider_submenu = Submenu::with_items(app, "选择供应商", true, &provider_refs)?;

            // 额度子菜单：展示每个已配置额度监控的供应商的额度摘要
            // 菜单项创建后通过 Arc<Mutex> 持有引用，由定时线程刷新文字
            let balance_rows = modules::balance::repository::list_balance_snapshots().unwrap_or_default();
            let balance_items: Vec<MenuItem<_>> = if balance_rows.is_empty() {
                vec![MenuItem::with_id(app, "balance-empty", "暂无额度数据", false, None::<&str>)
                    .expect("failed to create balance-empty menu item")]
            } else {
                balance_rows
                    .iter()
                    .map(|row| {
                        let summary = format_balance_summary(&row.snapshot);
                        let text = format!("{}: {}", row.display_name, summary);
                        let item_id = format!("balance:{}", row.provider_id);
                        MenuItem::with_id(app, &item_id, &text, false, None::<&str>)
                            .expect("failed to create balance menu item")
                    })
                    .collect()
            };
            let balance_refs: Vec<&dyn IsMenuItem<_>> = balance_items
                .iter()
                .map(|item| item as &dyn IsMenuItem<_>)
                .collect();
            let balance_submenu = Submenu::with_items(app, "额度", true, &balance_refs)?;
            // 持有额度菜单项引用，供定时线程刷新文字（需可变：用 Arc<Mutex<Vec>>）
            let balance_items_handle: Arc<Mutex<Vec<MenuItem<_>>>> = Arc::new(Mutex::new(balance_items));

            // ===== 新增托盘菜单项 =====
            // 今日 token 消耗（只读）
            let today_tokens_i = MenuItem::with_id(app, "today-tokens", "今日 Tokens: —", false, None::<&str>)?;
            let today_tokens_item = Arc::new(Mutex::new(Some(today_tokens_i.clone())));

            // 开机自启开关
            let auto_start_settings = modules::settings::service::SettingsServiceHandle::new();
            let auto_start_enabled = auto_start_settings.service().get_settings().map(|s| s.auto_start_enabled).unwrap_or(false);
            let auto_start_i = MenuItem::with_id(app, "auto-start", if auto_start_enabled { "开机自启: ✓" } else { "开机自启: ✗" }, true, None::<&str>)?;
            let auto_start_item = Arc::new(Mutex::new(Some(auto_start_i.clone())));
            // 注册为 Tauri State（供 settings_update 时读取当前值）
            // 注意：settings_handle 已在前面注册，这里使用同一个实例
            // 我们额外存储 auto_start_item 的引用，供 settings_update 时更新菜单文字

            // 网关开关
            let gateway_runtime_state = {
                let handle = app.state::<modules::gateway_runtime::GatewayRuntimeHandle>();
                handle.service().status().unwrap_or_default()
            };
            let gateway_running = gateway_runtime_state.is_running;
            let gateway_toggle_i = MenuItem::with_id(
                app,
                "gateway-toggle",
                if gateway_running { "网关: 运行中 ✓" } else { "网关: 已关闭 ✗" },
                true,
                None::<&str>,
            )?;
            let gateway_toggle_item = Arc::new(Mutex::new(Some(gateway_toggle_i.clone())));

            // 内存信息菜单项，后续由后台线程刷新数值
            let memory_i = MenuItem::with_id(app, "memory", "内存: —", false, None::<&str>)?;
            let memory_item = Arc::new(Mutex::new(Some(memory_i.clone())));

            let separator2 = PredefinedMenuItem::separator(app)?;

            let menu = Menu::with_items(
                app,
                &[
                    &show_i,
                    &provider_submenu,
                    &balance_submenu,
                    &separator2,
                    &today_tokens_i,
                    &auto_start_i,
                    &gateway_toggle_i,
                    &memory_i,
                    &separator,
                    &quit_i,
                ],
            )?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("i-code")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, event| {
                    let auto_start_item = auto_start_item.clone();
                    match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                        "auto-start" => {
                            // 切换开机自启开关
                            let settings_handle = app.state::<modules::settings::service::SettingsServiceHandle>();
                            let current = settings_handle.service().get_settings().map(|s| s.auto_start_enabled).unwrap_or(false);
                            let new_val = !current;
                            let input = modules::settings::types::UpdateSettingsInput {
                                auto_start_enabled: Some(new_val),
                                ..Default::default()
                            };
                            if let Ok(_) = settings_handle.service().update_settings(input) {
                                let text = if new_val { "开机自启: ✓" } else { "开机自启: ✗" };
                                if let Ok(lock) = auto_start_item.lock() {
                                    if let Some(item) = lock.as_ref() {
                                        let _ = item.set_text(text);
                                    }
                                }
                                // 使用 tauri-plugin-autostart 跨平台注册/取消开机自启
                                let autolaunch = app.autolaunch();
                                if new_val {
                                    if let Err(e) = autolaunch.enable() {
                                        log::error!("开机自启启用失败：{}", e);
                                        app.state::<modules::logger::LoggerServiceHandle>().service().log_system(
                                            crate::modules::logger::types::LogLevel::Error,
                                            &format!("开机自启启用失败：{}", e),
                                            Some(file!()),
                                        );
                                    } else {
                                        log::info!("开机自启已启用");
                                        app.state::<modules::logger::LoggerServiceHandle>().service().log_system(
                                            crate::modules::logger::types::LogLevel::Info,
                                            "开机自启已启用",
                                            Some(file!()),
                                        );
                                    }
                                } else {
                                    if let Err(e) = autolaunch.disable() {
                                        log::error!("开机自启关闭失败：{}", e);
                                        app.state::<modules::logger::LoggerServiceHandle>().service().log_system(
                                            crate::modules::logger::types::LogLevel::Error,
                                            &format!("开机自启关闭失败：{}", e),
                                            Some(file!()),
                                        );
                                    } else {
                                        log::info!("开机自启已关闭");
                                        app.state::<modules::logger::LoggerServiceHandle>().service().log_system(
                                            crate::modules::logger::types::LogLevel::Info,
                                            "开机自启已关闭",
                                            Some(file!()),
                                        );
                                    }
                                }
                            }
                        }
                        "gateway-toggle" => {
                            // 通过事件机制触发网关启停，不再直接调用 Service
                            // 后端监听 gateway:toggle-request 事件执行实际操作
                            let _ = app.emit("gateway:toggle-request", ());
                        }
                        id if id.starts_with("provider:") => {
                            // 供应商切换事件占位：后续可通过 Tauri 事件通知前端
                            let _ = app.emit("provider-changed", id);
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            // ===== 注册后端事件监听器 =====
            // 托盘菜单通过事件机制驱动网关启停，而非直接调用 Service。
            // 事件流：
            //   托盘/前端 → emit("gateway:toggle-request") → 后端监听 → start/stop → emit("gateway:status-changed")
            //   → 前端 useGatewayStatus 自动刷新 + 托盘菜单文字更新

            // 1) 监听 gateway:toggle-request：执行网关启停 + 更新 settings 持久化状态
            let gateway_inner_for_toggle = app.state::<modules::gateway_runtime::GatewayRuntimeHandle>().inner.clone();
            let settings_inner_for_toggle = app.state::<modules::settings::service::SettingsServiceHandle>().inner.clone();
            app.listen("gateway:toggle-request", move |_| {
                // 在 Listener 回调中无法直接 async，需 spawn
                // 使用 tauri::async_runtime::spawn 确保在正确的 Tokio 运行时上下文中执行
                let gateway_inner = gateway_inner_for_toggle.clone();
                let settings_inner = settings_inner_for_toggle.clone();
                let current_running = gateway_inner.status().unwrap_or_default().is_running;
                tauri::async_runtime::spawn(async move {
                    if current_running {
                        // 关闭网关
                        let _ = gateway_inner.stop().await;
                        let _ = settings_inner.update_settings(modules::settings::types::UpdateSettingsInput {
                            gateway_last_running: Some(false),
                            ..Default::default()
                        });
                    } else {
                        // 启动网关
                        let _ = gateway_inner.start(modules::gateway_runtime::types::StartGatewayInput::default()).await;
                        let _ = settings_inner.update_settings(modules::settings::types::UpdateSettingsInput {
                            gateway_last_running: Some(true),
                            ..Default::default()
                        });
                    }
                    // 注意：菜单文字由 gateway:status-changed 监听器更新，这里不再手动设置
                });
            });

            // 2) 监听 gateway:status-changed：更新托盘菜单文字（同步前端与托盘状态）
            let gateway_toggle_item_for_status = gateway_toggle_item.clone();
            app.listen("gateway:status-changed", move |event| {
                // event.payload() 返回 JSON 字符串，解析为 GatewayRuntimeState
                if let Ok(state) = serde_json::from_str::<modules::gateway_runtime::types::GatewayRuntimeState>(event.payload()) {
                    let text = if state.is_running { "网关: 运行中 ✓" } else { "网关: 已关闭 ✗" };
                    if let Ok(lock) = gateway_toggle_item_for_status.lock() {
                        if let Some(item) = lock.as_ref() {
                            let _ = item.set_text(text);
                        }
                    }
                }
            });

            // ===== 开机自启时恢复网关 & 隐藏主窗口 =====
            // 如果启动参数含 --autostart（由 tauri-plugin-autostart 注入），说明本次为开机自启调用：
            //   1. 隐藏主窗口到托盘（开机自启时不需要显示窗口）
            //   2. 若上次网关处于运行状态，通过事件机制触发网关启动
            let args: Vec<String> = std::env::args().collect();
            let is_autostart = args.iter().any(|a| a == "--autostart");
            if is_autostart {
                log::info!("检测到 --autostart 参数：开机自启模式，隐藏主窗口到托盘");
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                    log::info!("主窗口已隐藏");
                }
            }
            if auto_start_enabled {
                if modules::settings::service::SettingsServiceHandle::new()
                    .service()
                    .get_settings()
                    .map(|s| s.gateway_last_running)
                    .unwrap_or(false)
                {
                    let app_handle_for_auto_start = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        // 通过事件机制触发网关启动，与托盘菜单逻辑统一
                        let _ = app_handle_for_auto_start.emit("gateway:toggle-request", ());
                        log::info!("开机自启：已触发网关启动");
                    });
                }
            }

            // 定时刷新托盘内存与今日 Tokens 显示（每 10 秒）并向前端广播事件
            // 同时刷新网关菜单文字作为兜底（事件监听已为主渠道）
            // 同时刷新额度子菜单文字（从已持久化的快照读取，不发起网络请求）
            let app_handle = app.handle().clone();
            let gateway_toggle_item_for_timer = gateway_toggle_item.clone();
            let balance_items_handle_for_timer = balance_items_handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                // 内存
                let usage = get_memory_usage();
                let text = format!("内存: {usage} KB");
                if let Ok(lock) = memory_item.lock() {
                    if let Some(item) = lock.as_ref() {
                        let _ = item.set_text(&text);
                    }
                }
                let _ = app_handle.emit("memory-usage", usage);

                // 今日 Tokens
                let tokens = modules::call_records::service::get_today_tokens().unwrap_or(0);
                let tokens_text = format!("今日 Tokens: {tokens}");
                if let Ok(lock) = today_tokens_item.lock() {
                    if let Some(item) = lock.as_ref() {
                        let _ = item.set_text(&tokens_text);
                    }
                }
                let _ = app_handle.emit("today-tokens", tokens);

                // 网关菜单文字兜底刷新（主渠道为 gateway:status-changed 事件监听）
                let gateway_handle = app_handle.state::<modules::gateway_runtime::GatewayRuntimeHandle>();
                let gateway_running = gateway_handle.service().status().unwrap_or_default().is_running;
                let gw_text = if gateway_running { "网关: 运行中 ✓" } else { "网关: 已关闭 ✗" };
                if let Ok(lock) = gateway_toggle_item_for_timer.lock() {
                    if let Some(item) = lock.as_ref() {
                        let _ = item.set_text(gw_text);
                    }
                }

                // 额度子菜单文字刷新：从已持久化的快照读取最新额度摘要
                // 注意：此处仅刷新展示文字，不发起网络请求；网络刷新由前端触发 balance_refresh_provider
                if let Ok(rows) = modules::balance::repository::list_balance_snapshots() {
                    if let Ok(lock) = balance_items_handle_for_timer.lock() {
                        for item in lock.iter() {
                            // 通过 item id 匹配 provider_id（id 格式：balance:{provider_id} 或 balance-empty）
                            let item_id: String = item.id().as_ref().to_string();
                            if let Some(provider_id) = item_id.strip_prefix("balance:") {
                                if let Some(row) = rows.iter().find(|r| r.provider_id == provider_id) {
                                    let summary = format_balance_summary(&row.snapshot);
                                    let _ = item.set_text(&format!("{}: {}", row.display_name, summary));
                                }
                            } else if item_id == "balance-empty" {
                                let _ = item.set_text(if rows.is_empty() { "暂无额度数据" } else { "—" });
                            }
                        }
                    }
                }
            });

            // ===== 启动期异步检查更新 =====
            // 应用启动后异步拉取一次 GitHub latest.json，无论是否有更新（或请求失败）
            // 都通过 `update-check-result` 事件推送给前端；前端据此决定是否在标题栏
            // 展示更新入口。延迟 3 秒发起，确保前端已完成事件监听注册。
            let app_handle_for_update = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                modules::update_version::run_update_check_and_emit(app_handle_for_update).await;
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
