//! # 媒体生成服务层
//!
//! 图像生成的业务编排：
//! 1. 校验供应商为「视觉生成」供应商（媒体生成协议族）
//! 2. 经 ai_gateway Service 解析认证（`$SECRET:` 解密）与附加请求头
//! 3. 调用上游 `POST {base_url}/images/generations`（协议适配器按 provider_type 分发）
//! 4. 产物立即下载到本地（asset_store），写入生成历史与调用统计（call-records）
//!
//! 日志约定：同时写入两套日志——
//! - `log::info!`（tauri-plugin-log）：完整请求参数与上游响应概要，供终端/文件追踪
//! - `Log::info`（自研内存 logger）：应用内「日志」页面可见的运行时诊断

use chrono::Utc;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::ai_gateway::service::AiGatewayService;
use crate::modules::ai_gateway::types::{is_media_generation_provider_type, AuthConfig};
use crate::modules::call_records::service::CallRecordsService;
use crate::modules::call_records::types::{CreateModelCallLogInput, RouteMode};

use super::asset_store;
use super::repository;
use super::types::{GenerateImageInput, MediaGeneration, UpstreamImageResponse};

/// 图像生成上游请求超时（秒）
///
/// 生图为非流式同步请求，2K 分辨率下单次可达数十秒，取宽裕值。
const GENERATE_TIMEOUT_SECS: u64 = 180;

/// 媒体生成服务
pub struct MediaGenerationService;

impl MediaGenerationService {
    /// 创建 Service 实例
    pub fn new() -> Self {
        Self
    }

    /// 生成图像（供应商直连，不经网关转发管道）
    ///
    /// 成功：产物下载到本地并写入历史（status = succeeded）。
    /// 上游失败：同样写入失败历史后返回 `Err`（前端以 toast 呈现）。
    pub async fn generate_image(
        &self,
        ai_gateway: &AiGatewayService,
        input: GenerateImageInput,
    ) -> IcodeResult<MediaGeneration> {
        if input.prompt.trim().is_empty() {
            return Err(IcodeError::validation("图像描述文本 prompt 不能为空"));
        }

        // 1. 供应商校验：必须存在、启用且属于媒体生成协议族
        let provider = ai_gateway.get_provider(&input.provider_id)?;
        if !provider.is_enabled {
            return Err(IcodeError::validation(format!(
                "供应商 '{}' 已禁用",
                provider.slug
            )));
        }
        if !is_media_generation_provider_type(&provider.provider_type) {
            return Err(IcodeError::validation(format!(
                "供应商 '{}'（{}）不是视觉生成供应商，请在视觉生成供应商上调用图像生成",
                provider.slug, provider.provider_type
            )));
        }

        // 2. 解析认证与附加请求头（$SECRET: 引用在此处解密为明文，仅存在于后端）
        let auth = ai_gateway.resolve_auth_for_request(&provider)?;
        let api_key = extract_api_key_plaintext(&auth).ok_or_else(|| {
            IcodeError::validation(format!("供应商 '{}' 未配置 API Key 认证", provider.slug))
        })?;
        let extra_headers = ai_gateway.resolve_extra_headers_for_request(&provider.id)?;

        // 3. 构造上游请求
        let url = format!(
            "{}/images/generations",
            provider.base_url.trim_end_matches('/')
        );
        let mut body = serde_json::json!({
            "model": input.model_id,
            "prompt": input.prompt,
        });
        if let Some(size) = &input.size {
            body["size"] = serde_json::Value::String(size.clone());
        }
        if let Some(n) = input.n {
            body["n"] = serde_json::json!(n);
        }
        if let Some(watermark) = input.watermark {
            body["watermark"] = serde_json::json!(watermark);
        }

        let request_id = crate::core::id::generate_id();
        let record_id = crate::core::id::generate_id();
        let started = std::time::Instant::now();

        log::info!(
            "媒体生成开始：provider={} model={} url={} prompt_len={} n={:?} size={:?} watermark={:?}",
            provider.slug,
            input.model_id,
            url,
            input.prompt.chars().count(),
            input.n,
            input.size,
            input.watermark
        );
        crate::modules::logger::Log::info(&format!(
            "媒体生成开始：{}/{}",
            provider.slug, input.model_id
        ));

        // 写入调用统计初始记录（来源 internal：应用内部直连调用）
        // 统计为辅助数据：写入失败不阻断生成，仅记录日志
        let call_log_id: Option<String> = match CallRecordsService::new().start_call(
            CreateModelCallLogInput {
                provider_id: provider.id.clone(),
                gateway_model_id: None,
                model_id: input.model_id.clone(),
                request_id: Some(request_id.clone()),
                route_mode: RouteMode::Direct,
                source: "internal".to_string(),
                api_key_secret_id: None,
            },
        ) {
            Ok(l) => Some(l.id),
            Err(e) => {
                log::error!("写入调用统计初始记录失败：{}", e.message);
                None
            }
        };

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(GENERATE_TIMEOUT_SECS))
            .build()
            .map_err(|e| IcodeError::internal(format!("构建 HTTP 客户端失败：{e}")))?;

        let mut request = client
            .post(&url)
            .bearer_auth(&api_key)
            .header("Content-Type", "application/json")
            .json(&body);
        // 供应商级附加请求头（后写覆盖先写）
        for (key, value) in &extra_headers {
            request = request.header(key, value);
        }

        let response = request.send().await;
        let duration_ms = started.elapsed().as_millis() as i64;

        let response = match response {
            Ok(resp) => resp,
            Err(e) => {
                let message = format!("上游图像生成请求失败：{e}");
                self.record_failure(
                    &provider,
                    &input,
                    &record_id,
                    duration_ms,
                    &message,
                    call_log_id.as_deref(),
                );
                return Err(IcodeError::internal(message));
            }
        };

        let status = response.status();
        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            let message = format!(
                "上游图像生成返回 HTTP {}：{}",
                status,
                truncate(&body_text, 500)
            );
            self.record_failure(
                &provider,
                &input,
                &record_id,
                duration_ms,
                &message,
                call_log_id.as_deref(),
            );
            return Err(IcodeError::internal(message));
        }

        // 4. 解析响应并本地化产物
        let payload: UpstreamImageResponse = match response.json().await {
            Ok(p) => p,
            Err(e) => {
                let message = format!("上游响应解析失败：{e}");
                self.record_failure(
                    &provider,
                    &input,
                    &record_id,
                    duration_ms,
                    &message,
                    call_log_id.as_deref(),
                );
                return Err(IcodeError::internal(message));
            }
        };

        let urls: Vec<String> = payload
            .data
            .iter()
            .filter_map(|item| item.url.clone())
            .collect();
        let asset_paths = match asset_store::download_images(&record_id, &urls).await {
            Ok(paths) => paths,
            Err(e) => {
                let message = format!("产物本地化失败：{}", e.message);
                self.record_failure(
                    &provider,
                    &input,
                    &record_id,
                    duration_ms,
                    &message,
                    call_log_id.as_deref(),
                );
                return Err(e);
            }
        };

        let duration_ms = started.elapsed().as_millis() as i64;

        // 5. 完成调用统计（生图无 token 概念，不记录 token 数）
        if let Some(call_log_id) = &call_log_id {
            let _ = CallRecordsService::new().finish_call_with_duration_and_tokens(
                call_log_id,
                duration_ms,
                Some(status.as_u16() as i64),
                None,
                None,
            );
        }

        // 6. 写入成功历史
        let record = MediaGeneration {
            id: record_id,
            provider_id: provider.id.clone(),
            provider_slug: provider.slug.clone(),
            model_id: input.model_id.clone(),
            prompt: input.prompt.clone(),
            params: Some(build_params_snapshot(&input)),
            status: "succeeded".to_string(),
            asset_paths,
            source_urls: urls,
            error_message: None,
            duration_ms: Some(duration_ms),
            created_at: Utc::now().to_rfc3339(),
        };
        repository::insert_generation(&record)?;

        log::info!(
            "媒体生成完成：id={} 产物数={} 耗时={}ms",
            record.id,
            record.asset_paths.len(),
            duration_ms
        );
        crate::modules::logger::Log::info(&format!(
            "媒体生成完成：{}/{} 产物 {} 张，耗时 {}ms",
            provider.slug,
            input.model_id,
            record.asset_paths.len(),
            duration_ms
        ));

        Ok(record)
    }

    /// 列出图像生成历史（按创建时间倒序）
    pub fn list_history(&self, limit: Option<i64>) -> IcodeResult<Vec<MediaGeneration>> {
        repository::list_generations(limit)
    }

    /// 删除一条生成历史（同时清理本地产物文件）
    pub fn delete_history(&self, id: &str) -> IcodeResult<()> {
        let record = repository::delete_generation(id)?;
        asset_store::delete_assets(&record.asset_paths);
        Ok(())
    }

    /// 记录失败历史 + 完成调用统计
    ///
    /// 上游调用失败时历史同样留痕（status = failed），便于「日志」页面之外的排查。
    fn record_failure(
        &self,
        provider: &crate::modules::ai_gateway::types::Provider,
        input: &GenerateImageInput,
        record_id: &str,
        duration_ms: i64,
        message: &str,
        call_log_id: Option<&str>,
    ) {
        log::error!("媒体生成失败：id={} {}", record_id, message);
        crate::modules::logger::Log::error_with_loc(
            &format!(
                "媒体生成失败：{}/{} {}",
                provider.slug, input.model_id, message
            ),
            file!(),
            line!(),
        );
        let record = MediaGeneration {
            id: record_id.to_string(),
            provider_id: provider.id.clone(),
            provider_slug: provider.slug.clone(),
            model_id: input.model_id.clone(),
            prompt: input.prompt.clone(),
            params: Some(build_params_snapshot(input)),
            status: "failed".to_string(),
            asset_paths: Vec::new(),
            source_urls: Vec::new(),
            error_message: Some(message.to_string()),
            duration_ms: Some(duration_ms),
            created_at: Utc::now().to_rfc3339(),
        };
        if let Err(e) = repository::insert_generation(&record) {
            log::error!("写入失败历史时出错：{}", e.message);
        }
        if let Some(call_log_id) = call_log_id {
            let _ = CallRecordsService::new().finish_call_with_duration_and_tokens(
                call_log_id,
                duration_ms,
                None,
                Some(message.to_string()),
                None,
            );
        }
    }
}

/// 从已解密的 AuthConfig 中提取 API Key 明文
///
/// 仅支持 API Key 认证（视觉生成供应商当前均为 api-key 方式）。
fn extract_api_key_plaintext(auth: &Option<AuthConfig>) -> Option<String> {
    match auth {
        Some(AuthConfig::ApiKey { api_key, .. }) => api_key
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// 构造生成参数快照（仅包含显式传入的参数）
fn build_params_snapshot(input: &GenerateImageInput) -> serde_json::Value {
    let mut params = serde_json::Map::new();
    if let Some(size) = &input.size {
        params.insert("size".to_string(), serde_json::json!(size));
    }
    if let Some(n) = input.n {
        params.insert("n".to_string(), serde_json::json!(n));
    }
    if let Some(watermark) = input.watermark {
        params.insert("watermark".to_string(), serde_json::json!(watermark));
    }
    serde_json::Value::Object(params)
}

/// 截断文本（错误体透出前限制长度，避免巨型响应刷屏）
fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}
