//! # Tokenizer 模块
//!
//! LLM token 计数估算器，从参考项目 `vscode-unify-chat-provider/src/tokenizer/` 移植。
//!
//! ## 模块职责
//!
//! 为网关预校验、调用记录统计、虚拟供应商选路等场景提供 token 估算能力。
//!
//! ## 分词策略
//!
//! | 策略 | 精度 | 速度 | 依赖 | 适用场景 |
//! |------|------|------|------|---------|
//! | `default`/`char4` | 低 | 最快 | 无 | 通用 fallback |
//! | `conservative` | 中（偏高估） | 快 | 无 | 安全边界校验 |
//! | `openai` | 高（OpenAI 模型） | 中 | tiktoken | OpenAI 系列精确计数 |
//! | `deepseek` | 高（DeepSeek 模型） | 中 | tiktoken | DeepSeek 系列精确计数 |
//!
//! ## 模块组成
//!
//! - [`types`]：TokenizerId / ChatMessage / TokenCountResult / Command 入参等 DTO
//! - [`char4`]：~4 字符/token 近似算法
//! - [`conservative`]：3 UTF-8 字节/token 保守估算
//! - [`content`]：从 ChatMessage 提取纯文本与额外 token 数
//! - [`openai`]：tiktoken BPE 精确分词（按模型自动选择编码）
//! - [`deepseek`]：tiktoken deepseek_v3 编码精确分词
//! - [`service`]：分词器注册表与统一调度（含自动推断、乘数、降级）
//! - [`commands`]：Tauri Command 声明
//!
//! ## 调用链
//!
//! ```text
//! 前端 → tokenizer_count / tokenizer_count_messages Command
//!   → service::provide_token_count / provide_message_token_count
//!   → 解析 TokenizerId → 按策略分派
//!   → 失败 fallback char4 → 应用乘数 → 返回 TokenCountResult
//! ```

pub mod char4;
pub mod commands;
pub mod conservative;
pub mod content;
pub mod deepseek;
pub mod openai;
pub mod service;
pub mod types;
