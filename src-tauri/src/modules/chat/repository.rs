//! # 聊天模块 JSONL 存储
//!
//! ## 目录（程序运行目录 / 传入 root）
//!
//! ```text
//! chat/
//!   sessions.jsonl                 # 会话索引，每行一个 ChatSessionSummary
//!   messages/{session_id}.jsonl    # 会话消息，每行一个 ChatMessage
//! ```
//!
//! ## 逻辑描述
//!
//! - **索引更新**：`upsert_session_summary` 读全量 → 替换/追加 → 整文件重写（幂等、实现简单）。
//! - **消息追加**：`append_message` 行追加；流式结束后 `update_message` 全量重写该会话消息文件。
//! - **损坏行**：反序列化失败打 `log::warn` 并跳过，避免整库不可用。
//! - **删除**：去掉索引项并删除对应 `messages/{id}.jsonl`。
//!
//! 本层禁止调 Service、禁止发事件；仅文件 I/O。

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::error::{IcodeError, IcodeResult};

use super::types::{ChatMessage, ChatSession, ChatSessionSummary};

/// JSONL 仓储
pub struct ChatRepository {
    root: PathBuf,
}

impl ChatRepository {
    pub fn new(root: PathBuf) -> IcodeResult<Self> {
        let messages_dir = root.join("messages");
        fs::create_dir_all(&messages_dir).map_err(|e| {
            IcodeError::internal(format!("创建聊天目录失败: {}", e))
        })?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn sessions_index_path(&self) -> PathBuf {
        self.root.join("sessions.jsonl")
    }

    fn messages_path(&self, session_id: &str) -> PathBuf {
        self.root.join("messages").join(format!("{session_id}.jsonl"))
    }

    /// 读取全部会话摘要（按 updated_at 倒序）
    pub fn list_sessions(&self) -> IcodeResult<Vec<ChatSessionSummary>> {
        let path = self.sessions_index_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&path).map_err(|e| {
            IcodeError::internal(format!("读取会话索引失败: {}", e))
        })?;
        let reader = BufReader::new(file);
        let mut sessions = Vec::new();
        for (idx, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| {
                IcodeError::internal(format!("读取会话索引第 {} 行失败: {}", idx + 1, e))
            })?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<ChatSessionSummary>(line) {
                Ok(s) => sessions.push(s),
                Err(e) => {
                    log::warn!("跳过损坏的会话索引行 {}: {}", idx + 1, e);
                }
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// 按 ID 查找会话摘要
    pub fn find_session_summary(&self, id: &str) -> IcodeResult<Option<ChatSessionSummary>> {
        Ok(self.list_sessions()?.into_iter().find(|s| s.id == id))
    }

    /// 读取会话全部消息
    pub fn list_messages(&self, session_id: &str) -> IcodeResult<Vec<ChatMessage>> {
        let path = self.messages_path(session_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let file = File::open(&path).map_err(|e| {
            IcodeError::internal(format!("读取会话消息失败: {}", e))
        })?;
        let reader = BufReader::new(file);
        let mut messages = Vec::new();
        for (idx, line) in reader.lines().enumerate() {
            let line = line.map_err(|e| {
                IcodeError::internal(format!("读取消息第 {} 行失败: {}", idx + 1, e))
            })?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<ChatMessage>(line) {
                Ok(m) => messages.push(m),
                Err(e) => {
                    log::warn!("跳过损坏的消息行 {}: {}", idx + 1, e);
                }
            }
        }
        Ok(messages)
    }

    /// 获取完整会话
    pub fn get_session(&self, id: &str) -> IcodeResult<ChatSession> {
        let summary = self
            .find_session_summary(id)?
            .ok_or_else(|| IcodeError::not_found("ChatSession", Some(id)))?;
        let messages = self.list_messages(id)?;
        Ok(ChatSession {
            id: summary.id,
            title: summary.title,
            model: summary.model,
            transport_mode: summary.transport_mode,
            messages,
            created_at: summary.created_at,
            updated_at: summary.updated_at,
        })
    }

    /// 写入/更新会话摘要（全量重写索引，保证幂等）
    pub fn upsert_session_summary(&self, summary: &ChatSessionSummary) -> IcodeResult<()> {
        let mut sessions = self.list_sessions()?;
        if let Some(pos) = sessions.iter().position(|s| s.id == summary.id) {
            sessions[pos] = summary.clone();
        } else {
            sessions.push(summary.clone());
        }
        self.write_sessions_index(&sessions)
    }

    /// 删除会话摘要与消息文件
    pub fn delete_session(&self, id: &str) -> IcodeResult<()> {
        let mut sessions = self.list_sessions()?;
        let before = sessions.len();
        sessions.retain(|s| s.id != id);
        if sessions.len() == before {
            return Err(IcodeError::not_found("ChatSession", Some(id)));
        }
        self.write_sessions_index(&sessions)?;
        let msg_path = self.messages_path(id);
        if msg_path.exists() {
            fs::remove_file(&msg_path).map_err(|e| {
                IcodeError::internal(format!("删除会话消息文件失败: {}", e))
            })?;
        }
        Ok(())
    }

    /// 追加一条消息
    pub fn append_message(&self, message: &ChatMessage) -> IcodeResult<()> {
        let path = self.messages_path(&message.session_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                IcodeError::internal(format!("创建消息目录失败: {}", e))
            })?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| IcodeError::internal(format!("打开消息文件失败: {}", e)))?;
        let line = serde_json::to_string(message)
            .map_err(|e| IcodeError::internal(format!("序列化消息失败: {}", e)))?;
        writeln!(file, "{line}")
            .map_err(|e| IcodeError::internal(format!("写入消息失败: {}", e)))?;
        Ok(())
    }

    /// 更新已有消息（全量重写消息文件）
    pub fn update_message(&self, message: &ChatMessage) -> IcodeResult<()> {
        let mut messages = self.list_messages(&message.session_id)?;
        if let Some(pos) = messages.iter().position(|m| m.id == message.id) {
            messages[pos] = message.clone();
        } else {
            messages.push(message.clone());
        }
        self.write_messages(&message.session_id, &messages)
    }

    fn write_sessions_index(&self, sessions: &[ChatSessionSummary]) -> IcodeResult<()> {
        let path = self.sessions_index_path();
        let mut file = File::create(&path).map_err(|e| {
            IcodeError::internal(format!("写入会话索引失败: {}", e))
        })?;
        for s in sessions {
            let line = serde_json::to_string(s)
                .map_err(|e| IcodeError::internal(format!("序列化会话摘要失败: {}", e)))?;
            writeln!(file, "{line}")
                .map_err(|e| IcodeError::internal(format!("写入会话索引失败: {}", e)))?;
        }
        Ok(())
    }

    fn write_messages(&self, session_id: &str, messages: &[ChatMessage]) -> IcodeResult<()> {
        let path = self.messages_path(session_id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                IcodeError::internal(format!("创建消息目录失败: {}", e))
            })?;
        }
        let mut file = File::create(&path).map_err(|e| {
            IcodeError::internal(format!("写入消息文件失败: {}", e))
        })?;
        for m in messages {
            let line = serde_json::to_string(m)
                .map_err(|e| IcodeError::internal(format!("序列化消息失败: {}", e)))?;
            writeln!(file, "{line}")
                .map_err(|e| IcodeError::internal(format!("写入消息失败: {}", e)))?;
        }
        Ok(())
    }
}
