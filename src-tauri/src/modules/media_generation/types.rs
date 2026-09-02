//! # 媒体生成模块类型定义
//!
//! 与前端 `src/modules/media-generation/types.ts` 对齐。
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。

use serde::{Deserialize, Serialize};

/// 图像生成请求输入
///
/// `size` / `n` / `watermark` 为可选参数，未提供时不携带到上游
/// （由供应商使用其默认值，如 SenseNova 默认 `2752x1536` / `n=1` / `watermark=true`）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateImageInput {
    /// 视觉生成供应商 ID（providers 表主键）
    pub provider_id: String,
    /// 模型 ID（如 `sensenova-u1-fast`）
    pub model_id: String,
    /// 图像描述文本
    pub prompt: String,
    /// 图像尺寸，如 `2752x1536`（供应商默认值时可不传）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// 生成图片数量
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<i64>,
    /// 是否添加水印（None 表示使用供应商默认值）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watermark: Option<bool>,
}

/// 图像生成历史记录
///
/// 对应 `media_generations` 表。`asset_paths` 为应用数据目录下的相对路径
/// （相对媒体产物根目录），`source_urls` 为供应商返回的原始 URL（可能已过期）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaGeneration {
    pub id: String,
    pub provider_id: String,
    pub provider_slug: String,
    pub model_id: String,
    pub prompt: String,
    /// 生成参数快照（size / n / watermark 等 JSON）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// 状态：`succeeded` / `failed`
    pub status: String,
    /// 本地产物相对路径（相对媒体产物根目录）
    #[serde(default)]
    pub asset_paths: Vec<String>,
    /// 供应商返回的原始 URL（可能已过期，仅作追溯）
    #[serde(default)]
    pub source_urls: Vec<String>,
    /// 失败原因（status = failed 时存在）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// 生成耗时（毫秒，从发起上游请求到产物下载完成）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    pub created_at: String,
}

/// 上游图像生成接口的响应结构（OpenAI Images 兼容）
///
/// `{ "created": 1713167890, "data": [ { "url": "..." } ] }`
#[derive(Debug, Deserialize)]
pub struct UpstreamImageResponse {
    #[allow(dead_code)]
    #[serde(default)]
    pub created: Option<i64>,
    #[serde(default)]
    pub data: Vec<UpstreamImageItem>,
}

/// 上游图像生成响应中的单张图片项
#[derive(Debug, Deserialize)]
pub struct UpstreamImageItem {
    /// 图片 URL（临时链接，存在过期机制）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Base64 图片内容（OpenAI `response_format: "b64_json"` 场景，预留）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[allow(dead_code)]
    pub b64_json: Option<String>,
}
