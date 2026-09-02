//! # 聊天业务服务
//!
//! ## 职责
//!
//! - 会话 CRUD（委托 [`ChatRepository`] JSONL）
//! - 组装 OpenAI 兼容请求，经本地网关 `POST /v1/chat/completions`
//! - SSE 流式 / 普通 HTTP；解析正文与思考字段
//! - oneshot 中断；结果 `emit` 到前端
//!
//! ## 发送主流程
//!
//! ```text
//! send_message
//!   → 校验内容/附件 → 写用户消息 + 占位助手消息
//!   → 首条自动标题 → 注册 request_id + abort_tx
//!   → spawn run_chat_request
//!       → execute_gateway_chat（可 select 中断）
//!       → SSE: 每 chunk emit stream-chunk
//!       → 结束: 落库 finalize + emit done/error
//! ```
//!
//! ## 附件策略
//!
//! - `file`：`text_content` 拼入用户 content，名称写入展示与历史
//! - `image`：规范为 data URL，请求体用 `image_url` 多模态 part
//!
//! 日志：开发追踪用 `log::`（tauri-plugin-log）；不把 Secret 明文写入日志。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use futures::StreamExt;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::gateway_runtime::GatewayRuntimeHandle;
use crate::modules::secret::SecretServiceHandle;

use super::repository::ChatRepository;
use super::types::{
    AbortChatResult, ChatAttachment, ChatAttachmentInput, ChatAttachmentKind, ChatMessage,
    ChatProtocol, ChatPrompt, ChatPromptContent, ChatRole, ChatSession, ChatSessionSummary,
    ChatStreamChunkEvent, ChatStreamDoneEvent, ChatStreamErrorEvent, ChatTokenUsage,
    ChatTransportMode, CHAT_PROMPT_MAX_CHARS, CreateChatSessionInput, SendChatMessageInput,
    SendChatMessageResult, UpdateChatSessionInput,
};

/// 流式事件名（前端 `CHAT_EVENTS` / `listen` 与此一致）
pub const EVENT_CHAT_STREAM_CHUNK: &str = "chat:stream-chunk";
pub const EVENT_CHAT_STREAM_DONE: &str = "chat:stream-done";
pub const EVENT_CHAT_STREAM_ERROR: &str = "chat:stream-error";

/// 进行中请求的取消通道（`chat_message_abort` 发送后 select 退出）
struct ActiveRequest {
    abort_tx: oneshot::Sender<()>,
}

/// Chat Service 句柄（Tauri State 持有，Command 经此访问）
pub struct ChatServiceHandle {
    inner: Arc<ChatService>,
}

impl Clone for ChatServiceHandle {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl ChatServiceHandle {
    pub fn new(
        root: PathBuf,
        app_handle: AppHandle,
        gateway_runtime: GatewayRuntimeHandle,
        ai_gateway: AiGatewayServiceHandle,
        secret: SecretServiceHandle,
    ) -> IcodeResult<Self> {
        Ok(Self {
            inner: Arc::new(ChatService::new(
                root,
                app_handle,
                gateway_runtime,
                ai_gateway,
                secret,
            )?),
        })
    }

    pub fn service(&self) -> &ChatService {
        &self.inner
    }

    pub fn clone_inner(&self) -> Arc<ChatService> {
        self.inner.clone()
    }
}

pub struct ChatService {
    repo: Mutex<ChatRepository>,
    app_handle: AppHandle,
    gateway_runtime: GatewayRuntimeHandle,
    ai_gateway: AiGatewayServiceHandle,
    secret: SecretServiceHandle,
    active_requests: Mutex<HashMap<String, ActiveRequest>>,
    http_client: reqwest::Client,
}

impl ChatService {
    pub fn new(
        root: PathBuf,
        app_handle: AppHandle,
        gateway_runtime: GatewayRuntimeHandle,
        ai_gateway: AiGatewayServiceHandle,
        secret: SecretServiceHandle,
    ) -> IcodeResult<Self> {
        let repo = ChatRepository::new(root)?;
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| IcodeError::internal(format!("创建 HTTP 客户端失败: {}", e)))?;
        Ok(Self {
            repo: Mutex::new(repo),
            app_handle,
            gateway_runtime,
            ai_gateway,
            secret,
            active_requests: Mutex::new(HashMap::new()),
            http_client,
        })
    }

    // ===== 会话 CRUD =====

    pub fn list_sessions(&self) -> IcodeResult<Vec<ChatSessionSummary>> {
        self.repo
            .lock()
            .map_err(|_| IcodeError::internal("聊天仓储锁中毒"))?
            .list_sessions()
    }

    pub fn get_session(&self, id: &str) -> IcodeResult<ChatSession> {
        self.repo
            .lock()
            .map_err(|_| IcodeError::internal("聊天仓储锁中毒"))?
            .get_session(id)
    }

    pub fn create_session(&self, input: CreateChatSessionInput) -> IcodeResult<ChatSessionSummary> {
        if input.model.trim().is_empty() {
            return Err(IcodeError::validation("模型不能为空"));
        }
        let now = now_iso();
        let title = input
            .title
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "新会话".to_string());
        let summary = ChatSessionSummary {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            model: input.model.trim().to_string(),
            transport_mode: input.transport_mode.unwrap_or_default(),
            protocol: input.protocol.unwrap_or_default(),
            message_count: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        self.repo
            .lock()
            .map_err(|_| IcodeError::internal("聊天仓储锁中毒"))?
            .upsert_session_summary(&summary)?;
        Ok(summary)
    }

    pub fn update_session(
        &self,
        id: &str,
        input: UpdateChatSessionInput,
    ) -> IcodeResult<ChatSessionSummary> {
        let repo = self
            .repo
            .lock()
            .map_err(|_| IcodeError::internal("聊天仓储锁中毒"))?;
        let mut summary = repo
            .find_session_summary(id)?
            .ok_or_else(|| IcodeError::not_found("ChatSession", Some(id)))?;
        if let Some(title) = input.title {
            if !title.trim().is_empty() {
                summary.title = title.trim().to_string();
            }
        }
        if let Some(model) = input.model {
            if !model.trim().is_empty() {
                summary.model = model.trim().to_string();
            }
        }
        if let Some(mode) = input.transport_mode {
            summary.transport_mode = mode;
        }
        if let Some(p) = input.protocol {
            summary.protocol = p;
        }
        summary.updated_at = now_iso();
        repo.upsert_session_summary(&summary)?;
        Ok(summary)
    }

    pub fn delete_session(&self, id: &str) -> IcodeResult<()> {
        self.repo
            .lock()
            .map_err(|_| IcodeError::internal("聊天仓储锁中毒"))?
            .delete_session(id)
    }

    /// 删除单条消息：移除 JSONL 中该条并回写会话摘要计数
    ///
    /// 当前无子消息引用关系，直接删除即可；未来若引入分支/引用需联删（见 repository::delete_message 约束说明）
    pub fn delete_message(&self, session_id: &str, message_id: &str) -> IcodeResult<()> {
        let repo = self
            .repo
            .lock()
            .map_err(|_| IcodeError::internal("聊天仓储锁中毒"))?;
        let removed = repo.delete_message(session_id, message_id)?;
        if !removed {
            return Err(IcodeError::not_found("ChatMessage", Some(message_id)));
        }
        // 同步会话摘要：消息计数递减、更新时间刷新
        if let Some(mut summary) = repo.find_session_summary(session_id)? {
            summary.message_count = summary.message_count.saturating_sub(1);
            summary.updated_at = now_iso();
            repo.upsert_session_summary(&summary)?;
        }
        Ok(())
    }

    /// 导出 HTML 到应用配置目录 `exports/` 子目录，返回写入文件的绝对路径
    pub fn export_html(&self, html: &str, filename: &str) -> IcodeResult<String> {
        // 文件名安全化：仅保留文件名部分，禁止越界
        let safe_name = std::path::Path::new(filename)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| IcodeError::validation("无效的文件名"))?;
        let base = self
            .app_handle
            .path()
            .app_config_dir()
            .map_err(|e| IcodeError::internal(format!("无法获取应用配置目录: {e}")))?;
        let exports_dir = base.join("exports");
        std::fs::create_dir_all(&exports_dir)
            .map_err(|e| IcodeError::internal(format!("创建导出目录失败: {e}")))?;
        let target = exports_dir.join(safe_name);
        std::fs::write(&target, html)
            .map_err(|e| IcodeError::internal(format!("写入 HTML 文件失败: {e}")))?;
        Ok(target.to_string_lossy().into_owned())
    }

    // ===== 提示词库（prompt 目录下 *.md 文件） =====

    /// 解析提示词目录：`app_config_dir/prompt`，与数据库同目录。
    fn prompts_dir(&self) -> IcodeResult<PathBuf> {
        let base = self
            .app_handle
            .path()
            .app_config_dir()
            .map_err(|e| IcodeError::internal(format!("无法获取应用配置目录: {e}")))?;
        Ok(base.join("prompt"))
    }

    /// 从文件内容首个 `# ` 行提取标题；无则用文件名 stem。
    fn extract_title(content: &str, file_stem: &str) -> String {
        for line in content.lines() {
            let trimmed = line.trim_start_matches([' ', '\t', '\u{feff}']);
            if let Some(rest) = trimmed.strip_prefix("# ") {
                let title = rest.trim();
                if !title.is_empty() {
                    return title.to_string();
                }
            }
            // 跳过空行继续找
            if trimmed.is_empty() {
                continue;
            }
        }
        file_stem.to_string()
    }

    /// 列出所有提示词（标题取自首个 `# ` 行）
    pub fn list_prompts(&self) -> IcodeResult<Vec<ChatPrompt>> {
        let dir = self.prompts_dir()?;
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut items: Vec<ChatPrompt> = Vec::new();
        let read = std::fs::read_dir(&dir).map_err(|e| {
            IcodeError::internal(format!("读取提示词目录失败: {e}"))
        })?;
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let file_name = match path.file_name().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&file_name)
                .to_string();
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let title = Self::extract_title(&content, &stem);
            items.push(ChatPrompt {
                id: file_name,
                title,
            });
        }
        items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
        Ok(items)
    }

    /// 读取提示词正文，超过 [`CHAT_PROMPT_MAX_CHARS`] 字符截断并标记
    pub fn get_prompt(&self, id: &str) -> IcodeResult<ChatPromptContent> {
        // id 即文件名，禁止越界访问上级目录
        let safe_name = std::path::Path::new(id)
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| IcodeError::validation("无效的提示词 id"))?;
        let path = self.prompts_dir()?.join(safe_name);
        if !path.exists() {
            return Err(IcodeError::not_found("提示词", Some(id)));
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            IcodeError::internal(format!("读取提示词文件失败: {e}"))
        })?;
        let stem = std::path::Path::new(safe_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(safe_name)
            .to_string();
        let title = Self::extract_title(&content, &stem);

        let chars_count = content.chars().count();
        let (content, truncated) = if chars_count > CHAT_PROMPT_MAX_CHARS {
            let truncated_content: String = content.chars().take(CHAT_PROMPT_MAX_CHARS).collect();
            (truncated_content, true)
        } else {
            (content, false)
        };

        Ok(ChatPromptContent {
            id: id.to_string(),
            title,
            content,
            truncated,
        })
    }

    // ===== 发送 / 中断 =====

    /// 发送用户消息并启动网关请求
    ///
    /// # 界面侧时序
    ///
    /// 1. Command 同步返回 `user_message` + `assistant_message(streaming=true)` + `request_id`
    /// 2. 前端气泡立刻出现；随后 chunk/done/error 事件更新助手气泡与 token
    /// 3. 用户点「中断」→ `abort_request(request_id)` → 本请求 select 退出
    ///
    /// # 落库
    ///
    /// 先 append 用户与占位助手；结束后 `finalize_assistant_message` 写回 content/thinking/usage。
    pub fn send_message(
        self: &Arc<Self>,
        input: SendChatMessageInput,
    ) -> IcodeResult<SendChatMessageResult> {
        let content = input.content.trim().to_string();
        if content.is_empty() && input.attachments.is_empty() {
            return Err(IcodeError::validation("消息内容不能为空"));
        }

        let repo = self
            .repo
            .lock()
            .map_err(|_| IcodeError::internal("聊天仓储锁中毒"))?;

        let mut summary = repo
            .find_session_summary(&input.session_id)?
            .ok_or_else(|| IcodeError::not_found("ChatSession", Some(&input.session_id)))?;

        let transport_mode = input
            .transport_mode
            .unwrap_or(summary.transport_mode);
        let protocol = input.protocol.unwrap_or(summary.protocol);

        let now = now_iso();
        let attachments = convert_attachments(&input.attachments)?;

        // 用户展示文本：原文 + 附件名称提示
        let display_content = build_user_display_content(&content, &attachments);

        let user_message = ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: summary.id.clone(),
            role: ChatRole::User,
            content: display_content,
            thinking: None,
            attachments: attachments.clone(),
            usage: None,
            model: None,
            streaming: false,
            error: None,
            error_code: None,
            error_body: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        let assistant_message = ChatMessage {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: summary.id.clone(),
            role: ChatRole::Assistant,
            content: String::new(),
            thinking: None,
            attachments: Vec::new(),
            usage: None,
            model: None,
            streaming: true,
            error: None,
            error_code: None,
            error_body: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        repo.append_message(&user_message)?;
        repo.append_message(&assistant_message)?;

        // 标记是否为会话首条用户消息（用于后续注入 locale system 消息）
        let is_first_message = summary.message_count == 0;

        // 首条消息时用内容截断作为标题
        if is_first_message {
            let auto_title = auto_title_from_content(&content, &attachments);
            if !auto_title.is_empty() {
                summary.title = auto_title;
            }
        }
        summary.message_count = summary.message_count.saturating_add(2);
        summary.transport_mode = transport_mode;
        summary.protocol = protocol;
        summary.updated_at = now;
        repo.upsert_session_summary(&summary)?;

        // 历史消息用于构造上游 payload（含本轮用户消息）
        let history = repo.list_messages(&summary.id)?;
        drop(repo);

        let request_id = uuid::Uuid::new_v4().to_string();
        let (abort_tx, abort_rx) = oneshot::channel::<()>();
        {
            let mut map = self
                .active_requests
                .lock()
                .map_err(|_| IcodeError::internal("活跃请求锁中毒"))?;
            map.insert(
                request_id.clone(),
                ActiveRequest { abort_tx },
            );
        }

        let mut gateway_messages = build_openai_messages(&history, &user_message)?;

        // 首条用户消息且前端传了 locale 时，在请求 payload 开头注入 system 消息。
        // 该消息不写入 JSONL 历史，仅作为当前请求的上下文提示。
        if is_first_message {
            if let Some(locale) = input.locale.filter(|s| !s.trim().is_empty()) {
                let system_content = build_locale_system_content(&locale);
                gateway_messages.insert(
                    0,
                    json!({
                        "role": "system",
                        "content": system_content,
                    }),
                );
            }
        }
        let model = summary.model.clone();
        let stream = matches!(transport_mode, ChatTransportMode::Sse);
        let thinking_effort = input.thinking_effort.filter(|s| !s.trim().is_empty());

        let this = Arc::clone(self);
        let session_id = summary.id.clone();
        let assistant_id = assistant_message.id.clone();
        let req_id = request_id.clone();

        tauri::async_runtime::spawn(async move {
            this.run_chat_request(
                session_id,
                assistant_id,
                req_id,
                model,
                gateway_messages,
                protocol,
                stream,
                thinking_effort,
                abort_rx,
            )
            .await;
        });

        Ok(SendChatMessageResult {
            session: summary,
            user_message,
            assistant_message,
            request_id: Some(request_id),
        })
    }

    /// 中断进行中的请求
    pub fn abort_request(&self, request_id: &str) -> IcodeResult<AbortChatResult> {
        let mut map = self
            .active_requests
            .lock()
            .map_err(|_| IcodeError::internal("活跃请求锁中毒"))?;
        let aborted = if let Some(active) = map.remove(request_id) {
            let _ = active.abort_tx.send(());
            true
        } else {
            false
        };
        Ok(AbortChatResult {
            aborted,
            request_id: request_id.to_string(),
        })
    }

    // ===== 内部：网关请求 =====

    async fn run_chat_request(
        self: Arc<Self>,
        session_id: String,
        assistant_id: String,
        request_id: String,
        model: String,
        messages: Vec<Value>,
        protocol: ChatProtocol,
        stream: bool,
        thinking_effort: Option<String>,
        abort_rx: oneshot::Receiver<()>,
    ) {
        let result = self
            .execute_gateway_chat(
                &session_id,
                &assistant_id,
                &request_id,
                &model,
                messages,
                protocol,
                stream,
                thinking_effort,
                abort_rx,
            )
            .await;

        // 清理活跃请求
        if let Ok(mut map) = self.active_requests.lock() {
            map.remove(&request_id);
        }

        match result {
            Ok((content, thinking, usage, aborted)) => {
                let final_content = if aborted && content.is_empty() {
                    String::from("（已中断）")
                } else {
                    content
                };
                let thinking_opt = if thinking.trim().is_empty() {
                    None
                } else {
                    Some(thinking)
                };
                if let Err(e) = self.finalize_assistant_message(
                    &session_id,
                    &assistant_id,
                    &final_content,
                    thinking_opt.clone(),
                    usage.clone(),
                    None,
                    None,
                    None,
                    Some(&model),
                ) {
                    tracing::error!("更新助手消息失败: {}", e.message);
                }
                let _ = self.app_handle.emit(
                    EVENT_CHAT_STREAM_DONE,
                    ChatStreamDoneEvent {
                        session_id,
                        message_id: assistant_id,
                        request_id,
                        content: final_content,
                        thinking: thinking_opt,
                        usage,
                        model: Some(model),
                    },
                );
            }
            Err(err) => {
                let parsed = parse_chat_call_error(&err);
                // 气泡正文直接展示错误码 + body，便于会话内回看
                let bubble_content = format_error_bubble_content(&parsed);
                if let Err(e) = self.finalize_assistant_message(
                    &session_id,
                    &assistant_id,
                    &bubble_content,
                    None,
                    None,
                    Some(parsed.summary.clone()),
                    parsed.code.clone(),
                    parsed.body.clone(),
                    Some(&model),
                ) {
                    log::error!("更新助手错误消息失败: {}", e.message);
                }
                let _ = self.app_handle.emit(
                    EVENT_CHAT_STREAM_ERROR,
                    ChatStreamErrorEvent {
                        session_id,
                        message_id: assistant_id,
                        request_id,
                        error: parsed.summary,
                        error_code: parsed.code,
                        error_body: parsed.body,
                    },
                );
            }
        }
    }

    async fn execute_gateway_chat(
        &self,
        session_id: &str,
        assistant_id: &str,
        request_id: &str,
        model: &str,
        messages: Vec<Value>,
        protocol: ChatProtocol,
        stream: bool,
        thinking_effort: Option<String>,
        mut abort_rx: oneshot::Receiver<()>,
    ) -> IcodeResult<(String, String, Option<ChatTokenUsage>, bool)> {
        // 返回值：(content, thinking, usage, aborted)
        let base_url = self.resolve_gateway_base_url()?;
        let api_key = self.resolve_gateway_api_key()?;
        let endpoint = match protocol {
            ChatProtocol::Chat => "/v1/chat/completions",
            ChatProtocol::Messages => "/v1/messages",
            ChatProtocol::Responses => "/v1/responses",
        };
        let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint);

        // 注入推理力度：chat → reasoning_effort；responses → reasoning.effort；messages 协议不注入
        let reasoning_effort = thinking_effort.as_deref().filter(|s| !s.is_empty());
        let body = match protocol {
            ChatProtocol::Chat => {
                let mut body = json!({
                    "model": model,
                    "messages": messages,
                    "stream": stream,
                });
                if let Some(effort) = reasoning_effort {
                    body["reasoning_effort"] = json!(effort);
                }
                body
            }
            ChatProtocol::Responses => {
                let mut body = build_responses_request_body(model, &messages, stream);
                if let Some(effort) = reasoning_effort {
                    body["reasoning"] = json!({ "effort": effort });
                }
                body
            }
            ChatProtocol::Messages => build_anthropic_request_body(model, &messages, stream),
        };

        let mut builder = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("inner-cli-api", self.gateway_runtime.service().inner_cli_api_key());

        if let Some(key) = api_key {
            builder = builder.header("Authorization", format!("Bearer {key}"));
        }

        let request = builder
            .json(&body)
            .build()
            .map_err(|e| IcodeError::gateway(format!("构建请求失败: {}", e)))?;

        // 发送前检查是否已中断
        if abort_rx.try_recv().is_ok() {
            return Ok((String::new(), String::new(), None, true));
        }

        let response_fut = self.http_client.execute(request);
        tokio::pin!(response_fut);

        let response = tokio::select! {
            resp = &mut response_fut => {
                resp.map_err(|e| IcodeError::gateway(format!("请求网关失败: {}", e)))?
            }
            _ = &mut abort_rx => {
                return Ok((String::new(), String::new(), None, true));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| String::from("未知错误"));
            // 保留完整 body；错误码与 body 经 details 传给 UI 气泡
            let status_code = status.as_u16();
            let summary = extract_error_message(&text)
                .unwrap_or_else(|| format!("网关返回 HTTP {}", status_code));
            let protocol_code = extract_error_code(&text);
            let code_label = protocol_code
                .clone()
                .unwrap_or_else(|| status_code.to_string());
            return Err(
                IcodeError::gateway(format!("HTTP {} · {}", status_code, summary)).with_details(
                    json!({
                        "httpStatus": status_code,
                        "errorCode": code_label,
                        "errorBody": text,
                        "summary": summary,
                    }),
                ),
            );
        }

        if stream {
            self.consume_sse_stream(
                response,
                session_id,
                assistant_id,
                request_id,
                protocol,
                abort_rx,
            )
            .await
        } else {
            // 非流式：等待 body，同时可中断
            let text_fut = response.text();
            tokio::pin!(text_fut);
            let text = tokio::select! {
                t = &mut text_fut => {
                    t.map_err(|e| IcodeError::gateway(format!("读取响应失败: {}", e)))?
                }
                _ = abort_rx => {
                    return Ok((String::new(), String::new(), None, true));
                }
            };
            let (content, thinking, usage) = match protocol {
                ChatProtocol::Chat => parse_non_stream_response(&text)?,
                ChatProtocol::Messages => parse_anthropic_response(&text)?,
                ChatProtocol::Responses => parse_responses_response(&text)?,
            };
            // 推送完整内容作为一次 chunk，便于前端统一渲染
            let _ = self.app_handle.emit(
                EVENT_CHAT_STREAM_CHUNK,
                ChatStreamChunkEvent {
                    session_id: session_id.to_string(),
                    message_id: assistant_id.to_string(),
                    request_id: request_id.to_string(),
                    delta: content.clone(),
                    content: content.clone(),
                    thinking_delta: thinking.clone(),
                    thinking: thinking.clone(),
                },
            );
            Ok((content, thinking, usage, false))
        }
    }

    async fn consume_sse_stream(
        &self,
        response: reqwest::Response,
        session_id: &str,
        assistant_id: &str,
        request_id: &str,
        protocol: ChatProtocol,
        mut abort_rx: oneshot::Receiver<()>,
    ) -> IcodeResult<(String, String, Option<ChatTokenUsage>, bool)> {
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut content = String::new();
        let mut thinking = String::new();
        let mut usage: Option<ChatTokenUsage> = None;
        let mut pending = String::new();

        loop {
            tokio::select! {
                item = stream.next() => {
                    match item {
                        Some(Ok(bytes)) => {
                            pending.push_str(&String::from_utf8_lossy(&bytes));
                            // 按行处理 SSE
                            while let Some(pos) = pending.find('\n') {
                                let mut line = pending[..pos].to_string();
                                pending.drain(..=pos);
                                if line.ends_with('\r') {
                                    line.pop();
                                }
                                let line = line.trim_end();
                                if line.is_empty() {
                                    // 空行 = 事件边界，处理 buffer 中的 data
                                    if let Some(data) = take_sse_data(&mut buffer) {
                                        if data.trim() == "[DONE]" {
                                            return Ok((content, thinking, usage, false));
                                        }
                                        if let Ok(v) = serde_json::from_str::<Value>(&data) {
                                            let (content_delta, thinking_delta, usage_opt, is_done) =
                                                extract_stream_deltas(protocol, &v);
                                            // 先合并 usage 再判断结束：Responses 的 response.completed
                                            // 同时携带 is_done 与完整 usage，若先 return 会丢失用量
                                            merge_usage(&mut usage, usage_opt);
                                            if is_done {
                                                return Ok((content, thinking, usage, false));
                                            }
                                            let mut changed = false;
                                            if !content_delta.is_empty() {
                                                content.push_str(&content_delta);
                                                changed = true;
                                            }
                                            if !thinking_delta.is_empty() {
                                                thinking.push_str(&thinking_delta);
                                                changed = true;
                                            }
                                            if changed {
                                                let _ = self.app_handle.emit(
                                                    EVENT_CHAT_STREAM_CHUNK,
                                                    ChatStreamChunkEvent {
                                                        session_id: session_id.to_string(),
                                                        message_id: assistant_id.to_string(),
                                                        request_id: request_id.to_string(),
                                                        delta: content_delta,
                                                        content: content.clone(),
                                                        thinking_delta,
                                                        thinking: thinking.clone(),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                    continue;
                                }
                                if let Some(data) = line.strip_prefix("data:") {
                                    let data = data.trim_start();
                                    if !buffer.is_empty() {
                                        buffer.push('\n');
                                    }
                                    buffer.push_str(data);
                                }
                                // 忽略 event: / id: / : 注释
                            }
                        }
                        Some(Err(e)) => {
                            return Err(IcodeError::gateway(format!("读取 SSE 流失败: {}", e)));
                        }
                        None => {
                            // 流结束，处理剩余 buffer
                            if let Some(data) = take_sse_data(&mut buffer) {
                                if data.trim() != "[DONE]" {
                                    if let Ok(v) = serde_json::from_str::<Value>(&data) {
                                        let (cd, td, u_opt, _) = extract_stream_deltas(protocol, &v);
                                        content.push_str(&cd);
                                        thinking.push_str(&td);
                                        merge_usage(&mut usage, u_opt);
                                    }
                                }
                            }
                            return Ok((content, thinking, usage, false));
                        }
                    }
                }
                _ = &mut abort_rx => {
                    return Ok((content, thinking, usage, true));
                }
            }
        }
    }

    fn finalize_assistant_message(
        &self,
        session_id: &str,
        message_id: &str,
        content: &str,
        thinking: Option<String>,
        usage: Option<ChatTokenUsage>,
        error: Option<String>,
        error_code: Option<String>,
        error_body: Option<String>,
        model: Option<&str>,
    ) -> IcodeResult<()> {
        let repo = self
            .repo
            .lock()
            .map_err(|_| IcodeError::internal("聊天仓储锁中毒"))?;
        let mut messages = repo.list_messages(session_id)?;
        if let Some(msg) = messages.iter_mut().find(|m| m.id == message_id) {
            msg.content = content.to_string();
            msg.thinking = thinking;
            msg.usage = usage;
            msg.streaming = false;
            msg.error = error;
            msg.error_code = error_code;
            msg.error_body = error_body;
            // 记录该条助手消息实际使用的模型，避免会话内切换模型后历史气泡被改写
            if let Some(m) = model {
                msg.model = Some(m.to_string());
            }
            msg.updated_at = now_iso();
            let updated = msg.clone();
            repo.update_message(&updated)?;
        }
        if let Some(mut summary) = repo.find_session_summary(session_id)? {
            summary.updated_at = now_iso();
            repo.upsert_session_summary(&summary)?;
        }
        Ok(())
    }

    fn resolve_gateway_base_url(&self) -> IcodeResult<String> {
        let status = self.gateway_runtime.service().status().unwrap_or_default();
        if !status.is_running {
            return Err(IcodeError::gateway("本地网关未运行，请先启动网关"));
        }
        let host = status
            .bound_host
            .unwrap_or_else(|| "127.0.0.1".to_string());
        let port = status.bound_port.unwrap_or(54321);
        // 绑定 0.0.0.0 时客户端应连 127.0.0.1
        let host = if host == "0.0.0.0" {
            "127.0.0.1".to_string()
        } else {
            host
        };
        Ok(format!("http://{host}:{port}"))
    }

    fn resolve_gateway_api_key(&self) -> IcodeResult<Option<String>> {
        // 内部 CLI 头已足够鉴权；若配置了默认 key 也可附带
        let settings = self.ai_gateway.service().get_gateway_settings()?;
        let raw = settings.default_api_key_secret_id;
        let Some(raw) = raw.filter(|s| !s.trim().is_empty()) else {
            return Ok(None);
        };
        // 尝试解析 Secret 引用；失败则当作明文
        match self.secret.service().resolve_in_text(&raw) {
            Ok(resolved) => Ok(Some(resolved)),
            Err(_) => Ok(Some(raw)),
        }
    }
}

// ===== 工具函数 =====

fn now_iso() -> String {
    chrono::Local::now().to_rfc3339()
}

fn convert_attachments(inputs: &[ChatAttachmentInput]) -> IcodeResult<Vec<ChatAttachment>> {
    let mut out = Vec::with_capacity(inputs.len());
    for item in inputs {
        let id = uuid::Uuid::new_v4().to_string();
        match item.kind {
            ChatAttachmentKind::File => {
                out.push(ChatAttachment {
                    id,
                    name: item.name.clone(),
                    kind: ChatAttachmentKind::File,
                    mime_type: item.mime_type.clone(),
                    text_content: item.text_content.clone(),
                    data_url: None,
                    size: item.size,
                });
            }
            ChatAttachmentKind::Image => {
                let data_url = build_image_data_url(item)?;
                out.push(ChatAttachment {
                    id,
                    name: item.name.clone(),
                    kind: ChatAttachmentKind::Image,
                    mime_type: item.mime_type.clone(),
                    text_content: None,
                    data_url: Some(data_url),
                    size: item.size,
                });
            }
        }
    }
    Ok(out)
}

fn build_image_data_url(item: &ChatAttachmentInput) -> IcodeResult<String> {
    let raw = item
        .base64
        .as_ref()
        .ok_or_else(|| IcodeError::validation(format!("图片 {} 缺少 base64 数据", item.name)))?;
    if raw.starts_with("data:") {
        return Ok(raw.clone());
    }
    let mime = item
        .mime_type
        .clone()
        .unwrap_or_else(|| "image/png".to_string());
    Ok(format!("data:{mime};base64,{raw}"))
}

fn build_user_display_content(content: &str, attachments: &[ChatAttachment]) -> String {
    let mut parts = Vec::new();
    if !content.trim().is_empty() {
        parts.push(content.trim().to_string());
    }
    for att in attachments {
        match att.kind {
            ChatAttachmentKind::File => {
                parts.push(format!("[附件: {}]", att.name));
            }
            ChatAttachmentKind::Image => {
                parts.push(format!("[图片: {}]", att.name));
            }
        }
    }
    parts.join("\n")
}

fn auto_title_from_content(content: &str, attachments: &[ChatAttachment]) -> String {
    let base = if !content.trim().is_empty() {
        content.trim()
    } else if let Some(att) = attachments.first() {
        att.name.as_str()
    } else {
        return String::new();
    };
    let chars: Vec<char> = base.chars().collect();
    if chars.len() > 32 {
        format!("{}…", chars[..32].iter().collect::<String>())
    } else {
        base.to_string()
    }
}

/// 根据当前 locale 构造首条消息注入的 system 提示内容。
///
/// - `en`：使用英文提示，与 locale 语言一致；
/// - `ja`：使用日文提示；
/// - `zh-TW`：使用繁体中文提示；
/// - 其他（含 `zh-CN`）：使用简体中文提示。
fn build_locale_system_content(locale: &str) -> String {
    match locale {
        "en" => format!(
            "The current application locale is {}. Please respond in the corresponding language.",
            locale
        ),
        "ja" => format!(
            "現在のアプリの言語設定は {} です。この言語でユーザーに応答してください。",
            locale
        ),
        "zh-TW" => format!(
            "目前應用程式的語言設定為 {}，請使用該語言回覆使用者。",
            locale
        ),
        _ => format!(
            "当前应用语言环境为 {}，请根据该语言环境回应用户。",
            locale
        ),
    }
}

/// 构造发给网关的 OpenAI messages
///
/// - 用户消息：文本 content + 附件全文 + 图片 image_url
/// - 助手消息：纯文本
fn build_openai_messages(
    history: &[ChatMessage],
    current_user: &ChatMessage,
) -> IcodeResult<Vec<Value>> {
    let mut out = Vec::new();
    for msg in history {
        // 跳过尚未完成的助手占位消息
        if msg.role == ChatRole::Assistant && msg.streaming && msg.id != current_user.id {
            if msg.content.is_empty() && msg.error.is_none() {
                continue;
            }
        }
        // 当前用户消息用专门逻辑（含附件）
        if msg.id == current_user.id {
            out.push(build_user_openai_message(current_user));
            continue;
        }
        match msg.role {
            ChatRole::User => {
                // 历史用户消息：若有附件结构，重新拼；否则用 content
                if msg.attachments.is_empty() {
                    out.push(json!({
                        "role": "user",
                        "content": msg.content,
                    }));
                } else {
                    out.push(build_user_openai_message(msg));
                }
            }
            ChatRole::Assistant => {
                if msg.error.is_some() && msg.content.is_empty() {
                    continue;
                }
                // 构建助手消息；若有 thinking 内容则作为 reasoning_content 回传
                // 警告：DeepSeek V4 等模型要求在思考模式下多轮对话必须回传 reasoning_content，否者会报错，停止对话
                let mut assistant_msg = json!({
                    "role": "assistant",
                    "content": msg.content,
                });
                if let Some(thinking) = &msg.thinking {
                    if !thinking.is_empty() {
                        assistant_msg["reasoning_content"] = json!(thinking);
                    }
                }
                out.push(assistant_msg);
            }
            ChatRole::System => {
                out.push(json!({
                    "role": "system",
                    "content": msg.content,
                }));
            }
        }
    }
    // 若历史里还没包含当前用户消息（刚 append 后应已包含），兜底
    if !out.iter().any(|m| {
        m.get("role").and_then(|r| r.as_str()) == Some("user")
            && /* 粗略判断 */ true
    }) {
        // no-op: 上面循环已处理
    }
    // 确保至少有一条用户消息
    if out.is_empty() {
        out.push(build_user_openai_message(current_user));
    }
    Ok(out)
}

fn build_user_openai_message(msg: &ChatMessage) -> Value {
    let has_images = msg
        .attachments
        .iter()
        .any(|a| a.kind == ChatAttachmentKind::Image && a.data_url.is_some());

    // 组装文本部分：原文（去掉展示用附件标记）+ 文件附件全文
    let mut text_parts = Vec::new();
    // content 可能含 `[附件: x]` 展示行；协议侧用干净文本
    let clean = strip_attachment_markers(&msg.content);
    if !clean.is_empty() {
        text_parts.push(clean);
    }
    for att in &msg.attachments {
        match att.kind {
            ChatAttachmentKind::File => {
                let body = att.text_content.as_deref().unwrap_or("");
                text_parts.push(format!(
                    "【附件: {}】\n{}",
                    att.name,
                    body
                ));
            }
            ChatAttachmentKind::Image => {
                // 图片名称写入文本，便于模型感知
                text_parts.push(format!("【图片: {}】", att.name));
            }
        }
    }
    let text = text_parts.join("\n\n");

    if !has_images {
        return json!({
            "role": "user",
            "content": text,
        });
    }

    // 多模态 content 数组
    let mut parts: Vec<Value> = Vec::new();
    if !text.is_empty() {
        parts.push(json!({
            "type": "text",
            "text": text,
        }));
    }
    for att in &msg.attachments {
        if att.kind == ChatAttachmentKind::Image {
            if let Some(url) = &att.data_url {
                parts.push(json!({
                    "type": "image_url",
                    "image_url": {
                        "url": url
                    }
                }));
            }
        }
    }
    json!({
        "role": "user",
        "content": parts,
    })
}

fn strip_attachment_markers(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let t = line.trim();
            !(t.starts_with("[附件:") || t.starts_with("[图片:"))
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn take_sse_data(buffer: &mut String) -> Option<String> {
    if buffer.is_empty() {
        return None;
    }
    Some(std::mem::take(buffer))
}

fn extract_sse_delta(v: &Value) -> Option<String> {
    // OpenAI: choices[0].delta.content（字符串）
    if let Some(content) = v
        .pointer("/choices/0/delta/content")
        .and_then(|c| c.as_str())
    {
        return Some(content.to_string());
    }
    // content 为数组时：仅拼接 type=text / type=output_text 的文本
    if let Some(arr) = v.pointer("/choices/0/delta/content").and_then(|c| c.as_array()) {
        let text = extract_text_parts_from_content_array(arr);
        if !text.is_empty() {
            return Some(text);
        }
    }
    // 某些实现用 message.content
    if let Some(content) = v
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
    {
        return Some(content.to_string());
    }
    if let Some(arr) = v.pointer("/choices/0/message/content").and_then(|c| c.as_array()) {
        let text = extract_text_parts_from_content_array(arr);
        if !text.is_empty() {
            return Some(text);
        }
    }
    // text 字段
    if let Some(content) = v
        .pointer("/choices/0/text")
        .and_then(|c| c.as_str())
    {
        return Some(content.to_string());
    }
    None
}

/// 提取思考/推理增量
///
/// 兼容常见字段：
/// - `choices[0].delta.reasoning_content`（DeepSeek / 部分 OpenAI 兼容）
/// - `choices[0].delta.reasoning`
/// - `choices[0].delta.thinking`
/// - `choices[0].message.reasoning_content` / `reasoning` / `thinking`
/// - content 数组中的 `type=thinking|reasoning` 片段
fn extract_sse_thinking_delta(v: &Value) -> Option<String> {
    const PATHS: &[&str] = &[
        "/choices/0/delta/reasoning_content",
        "/choices/0/delta/reasoning",
        "/choices/0/delta/thinking",
        "/choices/0/message/reasoning_content",
        "/choices/0/message/reasoning",
        "/choices/0/message/thinking",
    ];
    for path in PATHS {
        if let Some(s) = v.pointer(path).and_then(|x| x.as_str()) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    // content 数组中的 thinking / reasoning 片段
    for base in ["/choices/0/delta/content", "/choices/0/message/content"] {
        if let Some(arr) = v.pointer(base).and_then(|c| c.as_array()) {
            let text = extract_thinking_parts_from_content_array(arr);
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

fn extract_text_parts_from_content_array(arr: &[Value]) -> String {
    let mut out = String::new();
    for part in arr {
        let ty = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if matches!(ty, "text" | "output_text" | "") {
            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                out.push_str(t);
            } else if let Some(t) = part
                .pointer("/text/value")
                .and_then(|t| t.as_str())
            {
                out.push_str(t);
            }
        }
    }
    out
}

fn extract_thinking_parts_from_content_array(arr: &[Value]) -> String {
    let mut out = String::new();
    for part in arr {
        let ty = part.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if matches!(ty, "thinking" | "reasoning" | "reasoning_content") {
            if let Some(t) = part.get("thinking").and_then(|t| t.as_str()) {
                out.push_str(t);
            } else if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                out.push_str(t);
            } else if let Some(t) = part.get("reasoning").and_then(|t| t.as_str()) {
                out.push_str(t);
            }
        }
    }
    out
}

fn extract_usage(v: &Value) -> Option<ChatTokenUsage> {
    let usage = v.get("usage")?;
    Some(ChatTokenUsage {
        prompt_tokens: usage.get("prompt_tokens").and_then(|x| x.as_i64()),
        completion_tokens: usage.get("completion_tokens").and_then(|x| x.as_i64()),
        total_tokens: usage.get("total_tokens").and_then(|x| x.as_i64()),
    })
}

fn parse_non_stream_response(text: &str) -> IcodeResult<(String, String, Option<ChatTokenUsage>)> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| IcodeError::gateway(format!("解析响应 JSON 失败: {}", e)))?;
    let content = if let Some(s) = v
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
    {
        s.to_string()
    } else if let Some(arr) = v.pointer("/choices/0/message/content").and_then(|c| c.as_array()) {
        extract_text_parts_from_content_array(arr)
    } else {
        v.pointer("/choices/0/text")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string()
    };
    let thinking = extract_sse_thinking_delta(&v).unwrap_or_default();
    // 非流式也可能只在 message 级字段给完整 reasoning
    let thinking = if thinking.is_empty() {
        const PATHS: &[&str] = &[
            "/choices/0/message/reasoning_content",
            "/choices/0/message/reasoning",
            "/choices/0/message/thinking",
        ];
        PATHS
            .iter()
            .find_map(|p| v.pointer(p).and_then(|x| x.as_str()).map(|s| s.to_string()))
            .unwrap_or_default()
    } else {
        thinking
    };
    let usage = extract_usage(&v);
    Ok((content, thinking, usage))
}

// ===== 协议适配：Anthropic Messages / OpenAI Responses =====

/// 按协议从单个 SSE 事件 JSON 提取增量
///
/// 返回 `(content_delta, thinking_delta, usage_opt, is_done)`：
/// - `is_done`：该事件标志流结束（Anthropic `message_stop` / Responses `response.completed`）
fn extract_stream_deltas(
    protocol: ChatProtocol,
    v: &Value,
) -> (String, String, Option<ChatTokenUsage>, bool) {
    match protocol {
        ChatProtocol::Chat => {
            let cd = extract_sse_delta(v).unwrap_or_default();
            let td = extract_sse_thinking_delta(v).unwrap_or_default();
            let u = extract_usage(v);
            (cd, td, u, false)
        }
        ChatProtocol::Messages => extract_anthropic_stream_deltas(v),
        ChatProtocol::Responses => extract_responses_stream_deltas(v),
    }
}

/// 合并 token 用量：Anthropic 的 input_tokens（message_start）与 output_tokens
/// （message_delta）分事件到达，需按字段累并。
fn merge_usage(usage: &mut Option<ChatTokenUsage>, new: Option<ChatTokenUsage>) {
    let Some(new) = new else {
        return;
    };
    let mut cur = usage.clone().unwrap_or_default();
    if new.prompt_tokens.is_some() {
        cur.prompt_tokens = new.prompt_tokens;
    }
    if new.completion_tokens.is_some() {
        cur.completion_tokens = new.completion_tokens;
    }
    if new.total_tokens.is_some() {
        cur.total_tokens = new.total_tokens;
    }
    *usage = Some(cur);
}

/// Anthropic Messages 流式事件提取
///
/// 事件类型（`type` 字段）：
/// - `message_start`：`message.usage.input_tokens`
/// - `content_block_delta`：`delta.type` = `text_delta` / `thinking_delta`
/// - `message_delta`：`usage.output_tokens`
/// - `message_stop`：流结束
fn extract_anthropic_stream_deltas(v: &Value) -> (String, String, Option<ChatTokenUsage>, bool) {
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let mut content_delta = String::new();
    let mut thinking_delta = String::new();
    let mut usage = None;
    let mut is_done = false;
    match ty {
        "message_start" => {
            if let Some(u) = v.pointer("/message/usage") {
                let input = u.get("input_tokens").and_then(|x| x.as_i64());
                let output = u.get("output_tokens").and_then(|x| x.as_i64());
                if input.is_some() || output.is_some() {
                    usage = Some(ChatTokenUsage {
                        prompt_tokens: input,
                        completion_tokens: output,
                        total_tokens: None,
                    });
                }
            }
        }
        "content_block_delta" => {
            if let Some(delta) = v.get("delta") {
                let dtype = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match dtype {
                    "text_delta" => {
                        if let Some(t) = delta.get("text").and_then(|t| t.as_str()) {
                            content_delta.push_str(t);
                        }
                    }
                    "thinking_delta" => {
                        if let Some(t) = delta.get("thinking").and_then(|t| t.as_str()) {
                            thinking_delta.push_str(t);
                        }
                    }
                    _ => {}
                }
            }
        }
        "message_delta" => {
            if let Some(u) = v.get("usage") {
                let output = u.get("output_tokens").and_then(|x| x.as_i64());
                if output.is_some() {
                    usage = Some(ChatTokenUsage {
                        prompt_tokens: None,
                        completion_tokens: output,
                        total_tokens: None,
                    });
                }
            }
        }
        "message_stop" => {
            is_done = true;
        }
        _ => {}
    }
    (content_delta, thinking_delta, usage, is_done)
}

/// OpenAI Responses 流式事件提取
///
/// 事件类型（`type` 字段）：
/// - `response.output_text.delta`：`delta` 为文本增量
/// - `response.completed`：流结束，`response.usage` 含完整用量
fn extract_responses_stream_deltas(v: &Value) -> (String, String, Option<ChatTokenUsage>, bool) {
    let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let mut content_delta = String::new();
    let mut usage = None;
    let mut is_done = false;
    match ty {
        "response.output_text.delta" => {
            if let Some(d) = v.get("delta").and_then(|d| d.as_str()) {
                content_delta.push_str(d);
            }
        }
        "response.completed" => {
            is_done = true;
            if let Some(u) = v.pointer("/response/usage") {
                usage = Some(ChatTokenUsage {
                    prompt_tokens: u.get("input_tokens").and_then(|x| x.as_i64()),
                    completion_tokens: u.get("output_tokens").and_then(|x| x.as_i64()),
                    total_tokens: u.get("total_tokens").and_then(|x| x.as_i64()),
                });
            }
        }
        _ => {}
    }
    (content_delta, String::new(), usage, is_done)
}

/// 构造 Anthropic Messages 请求体
///
/// - system 角色消息提取到顶层 `system` 字段
/// - 图片 `image_url` 转为 `source.base64`
/// - `max_tokens` 必填，默认 4096
fn build_anthropic_request_body(model: &str, messages: &[Value], stream: bool) -> Value {
    let mut system_parts: Vec<String> = Vec::new();
    let mut anthropic_messages: Vec<Value> = Vec::new();
    for msg in messages {
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("user");
        if role == "system" {
            if let Some(s) = msg.get("content").and_then(|c| c.as_str()) {
                system_parts.push(s.to_string());
            } else if let Some(arr) = msg.get("content").and_then(|c| c.as_array()) {
                for part in arr {
                    if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                        system_parts.push(t.to_string());
                    }
                }
            }
            continue;
        }
        let content = convert_openai_content_to_anthropic(msg.get("content"));
        anthropic_messages.push(json!({
            "role": role,
            "content": content,
        }));
    }
    let mut body = json!({
        "model": model,
        "messages": anthropic_messages,
        "max_tokens": 4096,
        "stream": stream,
    });
    if !system_parts.is_empty() {
        body["system"] = json!(system_parts.join("\n\n"));
    }
    body
}

/// 构造 OpenAI Responses 请求体：`input` 直接复用 OpenAI messages 数组
fn build_responses_request_body(model: &str, messages: &[Value], stream: bool) -> Value {
    json!({
        "model": model,
        "input": messages,
        "stream": stream,
    })
}

/// 将 OpenAI content（字符串或 parts 数组）转为 Anthropic content
///
/// - 字符串：原样返回
/// - 数组：`text` 保留；`image_url` 的 data URL 解析为 `source.base64`
fn convert_openai_content_to_anthropic(content: Option<&Value>) -> Value {
    let Some(content) = content else {
        return Value::Null;
    };
    if let Some(s) = content.as_str() {
        return Value::String(s.to_string());
    }
    let Some(arr) = content.as_array() else {
        return Value::Null;
    };
    let mut out: Vec<Value> = Vec::new();
    for part in arr {
        let ty = part.get("type").and_then(|t| t.as_str()).unwrap_or("text");
        match ty {
            "text" | "output_text" | "" => {
                if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                    out.push(json!({"type": "text", "text": t}));
                }
            }
            "image_url" => {
                if let Some(url) = part.pointer("/image_url/url").and_then(|u| u.as_str()) {
                    if let Some((media_type, data)) = parse_data_url(url) {
                        out.push(json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": media_type,
                                "data": data,
                            }
                        }));
                    }
                }
            }
            _ => {}
        }
    }
    Value::Array(out)
}

/// 解析 `data:{media_type};base64,{data}` 形式的 data URL
fn parse_data_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let semi = rest.find(';')?;
    let media_type = &rest[..semi];
    let after = &rest[semi + 1..];
    let data = after.strip_prefix("base64,")?;
    Some((media_type.to_string(), data.to_string()))
}

/// 解析 Anthropic Messages 非流式响应
///
/// `content` 数组中 `type=text` 拼为正文，`type=thinking` 拼为思考；
/// `usage.input_tokens` / `usage.output_tokens` 映射到 prompt/completion。
fn parse_anthropic_response(text: &str) -> IcodeResult<(String, String, Option<ChatTokenUsage>)> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| IcodeError::gateway(format!("解析 Anthropic 响应 JSON 失败: {}", e)))?;
    let mut content = String::new();
    let mut thinking = String::new();
    if let Some(arr) = v.get("content").and_then(|c| c.as_array()) {
        for block in arr {
            let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match ty {
                "text" => {
                    if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                        content.push_str(t);
                    }
                }
                "thinking" => {
                    if let Some(t) = block.get("thinking").and_then(|t| t.as_str()) {
                        thinking.push_str(t);
                    }
                }
                _ => {}
            }
        }
    }
    let usage = v.get("usage").map(|u| {
        let input = u.get("input_tokens").and_then(|x| x.as_i64());
        let output = u.get("output_tokens").and_then(|x| x.as_i64());
        ChatTokenUsage {
            prompt_tokens: input,
            completion_tokens: output,
            total_tokens: match (input, output) {
                (Some(i), Some(o)) => Some(i + o),
                _ => None,
            },
        }
    });
    Ok((content, thinking, usage))
}

/// 解析 OpenAI Responses 非流式响应
///
/// `output[*].content` 中 `output_text`/`text` 拼为正文；
/// `usage` 含 `input_tokens` / `output_tokens` / `total_tokens`。
fn parse_responses_response(text: &str) -> IcodeResult<(String, String, Option<ChatTokenUsage>)> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| IcodeError::gateway(format!("解析 Responses 响应 JSON 失败: {}", e)))?;
    let mut content = String::new();
    if let Some(arr) = v.get("output").and_then(|c| c.as_array()) {
        for item in arr {
            if let Some(blocks) = item.get("content").and_then(|c| c.as_array()) {
                for block in blocks {
                    let ty = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if matches!(ty, "output_text" | "text" | "") {
                        if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                            content.push_str(t);
                        }
                    }
                }
            }
        }
    }
    // 部分实现直接在顶层提供 output_text 字符串
    if content.is_empty() {
        if let Some(s) = v.get("output_text").and_then(|c| c.as_str()) {
            content.push_str(s);
        }
    }
    let usage = v.get("usage").map(|u| ChatTokenUsage {
        prompt_tokens: u.get("input_tokens").and_then(|x| x.as_i64()),
        completion_tokens: u.get("output_tokens").and_then(|x| x.as_i64()),
        total_tokens: u.get("total_tokens").and_then(|x| x.as_i64()),
    });
    Ok((content, String::new(), usage))
}

fn extract_error_message(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    v.pointer("/error/message")
        .and_then(|m| m.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            v.get("message")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string())
        })
}

/// 从 OpenAI 风格错误 JSON 提取 code（如 `invalid_api_key`）
fn extract_error_code(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    v.pointer("/error/code")
        .and_then(|c| {
            c.as_str()
                .map(|s| s.to_string())
                .or_else(|| c.as_i64().map(|n| n.to_string()))
        })
        .or_else(|| {
            v.get("code")
                .and_then(|c| {
                    c.as_str()
                        .map(|s| s.to_string())
                        .or_else(|| c.as_i64().map(|n| n.to_string()))
                })
        })
        .filter(|s| !s.trim().is_empty())
}

/// 聊天调用失败时解析出的结构化错误（供气泡展示）
struct ChatCallErrorInfo {
    /// 简短摘要
    summary: String,
    /// HTTP 状态或协议/业务错误码
    code: Option<String>,
    /// 完整 body（不截断）
    body: Option<String>,
}

/// 从 IcodeError 还原错误码与 body（优先 details）
fn parse_chat_call_error(err: &IcodeError) -> ChatCallErrorInfo {
    if let Some(details) = &err.details {
        let http_status = details
            .get("httpStatus")
            .and_then(|v| v.as_u64())
            .map(|n| n.to_string());
        let error_code = details
            .get("errorCode")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or(http_status.clone());
        let error_body = details
            .get("errorBody")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let summary = details
            .get("summary")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| err.message.clone());
        return ChatCallErrorInfo {
            summary,
            code: error_code.or(Some(err.code.clone())),
            body: error_body,
        };
    }

    // 无 details：尽量从 message 解析 "HTTP 502 · ..." 形态
    let summary = err.message.clone();
    let code = if let Some(rest) = summary.strip_prefix("HTTP ") {
        rest.split_whitespace()
            .next()
            .map(|s| s.trim_matches(|c: char| !c.is_ascii_digit()).to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
    .or_else(|| Some(err.code.clone()));

    ChatCallErrorInfo {
        summary,
        code,
        body: None,
    }
}

/// 组装写入气泡 content 的错误文本：错误码 + body（或摘要）
fn format_error_bubble_content(info: &ChatCallErrorInfo) -> String {
    let mut lines = Vec::new();
    if let Some(code) = info.code.as_ref().filter(|c| !c.trim().is_empty()) {
        lines.push(format!("错误码: {}", code));
    }
    if let Some(body) = info.body.as_ref().filter(|b| !b.trim().is_empty()) {
        lines.push(format!("响应 Body:\n{}", body.trim()));
    } else if !info.summary.trim().is_empty() {
        lines.push(info.summary.trim().to_string());
    } else {
        lines.push("调用失败".to_string());
    }
    lines.join("\n")
}
