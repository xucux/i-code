//! # Tokenizer 模块类型定义
//!
//! 从参考项目 `vscode-unify-chat-provider/src/tokenizer/` 移植，
//! 适配 i-code 后端的 OpenAI 消息格式与 Tauri Command 交互。
//!
//! ## 与参考项目的差异
//!
//! - 输入消息使用 OpenAI ChatCompletion 格式（`role` + `content`），
//!   而非 VS Code `LanguageModelChatRequestMessage`
//! - 分词策略函数签名为纯 Rust，不依赖 VS Code API
//! - 增加 `TokenizerInfo` 用于前端列表展示

use serde::{Deserialize, Serialize};

// ===== 分词器标识 =====

/// 分词器策略标识
///
/// 与参考项目 `tokenizers.ts` 的 `TokenizerId` 对齐。
/// - `default` / `char4`：~4 字符/token 近似算法（VS Code 官方近似）
/// - `conservative`：3 UTF-8 字节/token 保守估算
/// - `openai`：tiktoken BPE 精确分词（按模型自动选择编码）
/// - `deepseek`：HuggingFace tokenizer 精确分词
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenizerId {
    Default,
    Char4,
    Conservative,
    Openai,
    Deepseek,
}

impl TokenizerId {
    /// 转换为字符串字面量
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Char4 => "char4",
            Self::Conservative => "conservative",
            Self::Openai => "openai",
            Self::Deepseek => "deepseek",
        }
    }

    /// 从字符串解析，无效值回退到 Default
    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim() {
            "default" => Self::Default,
            "char4" => Self::Char4,
            "conservative" => Self::Conservative,
            "openai" => Self::Openai,
            "deepseek" => Self::Deepseek,
            _ => Self::Default,
        }
    }

    /// 获取所有分词器 ID 列表
    #[allow(dead_code)]
    pub fn all() -> &'static [TokenizerId] {
        &[
            Self::Default,
            Self::Char4,
            Self::Conservative,
            Self::Openai,
            Self::Deepseek,
        ]
    }
}

impl std::fmt::Display for TokenizerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 默认分词器 ID
pub const DEFAULT_TOKENIZER_ID: TokenizerId = TokenizerId::Default;

/// 默认 token 计数乘数
pub const DEFAULT_TOKEN_COUNT_MULTIPLIER: f64 = 1.0;

// ===== 消息内容类型 =====

/// 聊天消息角色
///
/// 对齐 OpenAI ChatCompletion API 的 role 字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 消息内容部分（对标 OpenAI ChatCompletion 的 content 字段）
///
/// - `string` 类型：纯文本（OpenAI 格式中 content 为 string 的简写）
/// - `text` 类型：结构化文本部分
/// - `image_url` 类型：图片 URL 部分，估算时按固定 token 数计入
/// - `image_data` 类型：Base64 图片数据部分
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// 纯文本部分
    Text {
        text: String,
    },
    /// 图片 URL 部分
    ImageUrl {
        /// 图片 URL 或 base64 data URI
        image_url: ImageUrl,
    },
}

/// 图片 URL 结构
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageUrl {
    /// 图片 URL 或 data URI
    pub url: String,
    /// 图片细节级别：low / high / auto
    #[serde(default = "default_image_detail")]
    pub detail: String,
}

fn default_image_detail() -> String {
    "auto".to_string()
}

/// 聊天消息（对齐 OpenAI ChatCompletion API 格式）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    /// 消息角色
    pub role: ChatRole,
    /// 消息内容：字符串或结构化内容部分数组
    pub content: MessageContent,
    /// 工具调用 ID（role=tool 时必填）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 工具调用列表（role=assistant 时可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// 消息内容：字符串或内容部分数组
///
/// OpenAI API 中 content 可以是 string 或 ContentPart[]。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 纯文本字符串
    Text(String),
    /// 结构化内容部分数组
    Parts(Vec<ContentPart>),
}

impl MessageContent {
    /// 判断是否为空内容
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Text(s) => s.is_empty(),
            Self::Parts(parts) => parts.is_empty(),
        }
    }
}

/// 工具调用（assistant 消息中的 function/tool 调用）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    /// 工具调用 ID
    pub id: String,
    /// 工具类型，通常为 "function"
    #[serde(rename = "type")]
    pub call_type: String,
    /// 函数调用信息
    pub function: FunctionCall,
}

/// 函数调用
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// 函数名
    pub name: String,
    /// 函数参数（JSON 字符串）
    pub arguments: String,
}

// ===== 分词结果 =====

/// 消息提取结果
///
/// 由 `content` 模块从 `ChatMessage` 中提取，
/// 供 BPE 分词器（openai/deepseek）使用。
#[derive(Debug, Clone, Default)]
pub struct TokenizedInput {
    /// 所有文本部分拼接的纯文本
    pub text_content: String,
    /// 非文本部分折算的 token 数（图片 512、二进制 byteLength）
    pub extra_tokens: usize,
}

/// Token 计数结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCountResult {
    /// 估算的 token 数
    pub token_count: usize,
    /// 使用的分词器 ID
    pub tokenizer_id: String,
    /// 应用的乘数
    pub multiplier: f64,
}

// ===== 分词器信息（前端列表展示用） =====

/// 分词器描述信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenizerInfo {
    /// 分词器 ID
    pub id: String,
    /// 显示标签
    pub label: String,
    /// 描述说明
    pub description: String,
}

// ===== Command 入参 =====

/// Token 计数 Command 入参
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenCountInput {
    /// 模型 ID（格式：provider_slug/model_id），用于选择对应分词器与编码
    pub model_id: String,
    /// 要计算 token 的文本内容
    pub text: String,
    /// 指定分词器（覆盖模型配置中的 tokenizer）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<String>,
    /// 指定乘数（覆盖模型配置中的 tokenCountMultiplier）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f64>,
}

/// 消息列表 Token 计数 Command 入参
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageTokenCountInput {
    /// 模型 ID（格式：provider_slug/model_id）
    pub model_id: String,
    /// 聊天消息列表
    pub messages: Vec<ChatMessage>,
    /// 指定分词器（覆盖模型配置中的 tokenizer）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer: Option<String>,
    /// 指定乘数（覆盖模型配置中的 tokenCountMultiplier）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f64>,
}

// ===== 保守估算常量 =====

/// 每条消息的额外 token 开销（对齐 OpenAI 消息格式开销）
pub const MESSAGE_OVERHEAD_TOKENS: usize = 4;

/// 每个图片部分固定 token 数（保守估算）
pub const IMAGE_PART_TOKENS: usize = 512;

/// 保守估算：每 N 个 UTF-8 字节算 1 token
pub const CONSERVATIVE_BYTES_PER_TOKEN: usize = 3;

/// char4 估算：每 N 个 Unicode 字符算 1 token
pub const CHAR4_CHARS_PER_TOKEN: usize = 4;
