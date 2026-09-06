//! imagebed 模块数据类型（DTO / 事件 payload）

use serde::Serialize;

/// 内置图床 provider 规格（含注入脚本，仅后端使用，不暴露给前端）
#[derive(Debug, Clone)]
pub struct ImagebedProviderSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
    /// document-start 注入到图床页面的增强脚本（篡改猴式）
    pub inject_script: &'static str,
}

/// 图床 provider 对外 DTO（安全字段，不含注入脚本）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagebedProvider {
    pub id: String,
    pub name: String,
    pub url: String,
}

impl ImagebedProvider {
    pub fn from_spec(spec: &ImagebedProviderSpec) -> Self {
        Self {
            id: spec.id.to_string(),
            name: spec.name.to_string(),
            url: spec.url.to_string(),
        }
    }
}

/// 图床外链就绪事件 payload（后端 `emit("imagebed:link-ready")` → 前端 listen）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImagebedLinkReady {
    pub provider_id: String,
    /// 图片直链 URL
    pub url: String,
    /// 可直接插入编辑器的 Markdown 片段（`![alt](url)`）
    pub markdown: String,
    /// 毫秒时间戳
    pub created_at: u64,
}