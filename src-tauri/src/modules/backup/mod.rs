//! # 备份与恢复模块
//!
//! 将应用 SQLite 数据库及相关配置压缩打包，支持备份到本地目录或远程 WebDAV。
//!
//! ## 模块组成
//!
//! - [`types`]：`BackupTarget` / `BackupFormat` / `BackupMeta` / `WebDavConfig` 等 DTO
//! - [`service`]：创建/推送/列出/恢复/删除备份
//! - [`commands`]：Tauri Command 声明
//! - [`crypto`]：远端备份 AES-256-GCM 加密
//! - [`webdav`]：WebDAV 客户端辅助
//!
//! ## v0.2 实现范围
//!
//! - **本地备份**：完整实现（创建 zip / 列出 / 恢复 / 删除）
//!   - 目录优先级：`BackupSettings.local_directory` > 程序运行目录下的 `backup/`
//!   - 创建后按 `BackupSettings.local_retention_count` 清理旧备份
//! - **WebDAV 备份**：完整实现（列出 / 上传 / 下载 / 删除）
//!   - 支持 AES 加密（密码 SHA-256 做密钥）
//!   - 上传后按 `BackupSettings.webdav_retention_count` 清理远程旧备份
//! - **安全备份**：恢复前自动创建紧急备份
//! - **校验和**：计算数据库文件 SHA-256
//! - **恢复后重启提示**：恢复成功后返回 `needs_restart: true`
//!
//! ## 覆盖原则
//!
//! 备份的「推送」与「恢复」均采用**覆盖式**同步——
//! 目标端只保留最新一份同名备份；恢复时当前数据库被备份文件完全覆盖。

pub mod commands;
pub mod crypto;
pub mod repository;
pub mod service;
pub mod types;
pub mod webdav;

pub use service::BackupServiceHandle;
