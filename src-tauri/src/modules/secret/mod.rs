//! # 敏感凭据模块
//!
//! 安全存储 API Key、OAuth Token、代理认证、Gateway Key 等敏感数据。
//!
//! ## 架构约束
//!
//! 所有加密/解密、`$SECRET:{snowflake_id}$` 引用解析**仅在后端进行**。
//! 前端只有一个密码输入框组件，将用户输入的明文传给后端 Command 后立即丢弃，从不保留明文。
//!
//! ## 模块组成
//!
//! - [`crypto`]：AES-GCM 加密原语封装与主密钥管理
//! - [`repository`]：`secrets` 表的 CRUD
//! - [`service`]：业务逻辑（保存/读取/解析引用/清理孤立记录）
//! - [`commands`]：Tauri Command 声明
//! - [`types`]：DTO 与枚举定义
//!
//! ## v0.1 实现方案
//!
//! 当前实现为「本地 AES-GCM 加密」模式（`store_secrets_in_keychain = 0`）：
//! - 主密钥为随机 32 字节，存储在应用数据目录的 `master.key` 文件中
//! - 每条 Secret 使用独立的 12 字节随机 nonce 加密
//! - `encrypted_value` 列存储格式：`nonce(12B) || ciphertext_with_tag`
//!
//! 系统密钥链模式（`store_secrets_in_keychain = 1`）暂未实现，需引入
//! `tauri-plugin-stronghold` 或 `keytar` crate。详见 `docs/proposals/secret-storage.md`。

pub mod commands;
pub mod crypto;
pub mod repository;
pub mod service;
pub mod types;

pub use service::SecretServiceHandle;
pub use types::{build_secret_ref, parse_secret_ref, SecretKind};
