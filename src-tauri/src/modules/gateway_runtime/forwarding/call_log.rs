//! # 调用记录读写封装
//!
//! 封装 `call_records` 模块的 start / finish 调用，避免转发器重复样板代码。

use std::time::Instant;

use tauri::Emitter;

use crate::error::IcodeResult;
use crate::modules::call_records::types::{CreateModelCallLogInput, RouteMode, UpdateModelCallLogInput};
use crate::modules::gateway_runtime::service::GatewaySharedState;

use super::context::ForwardContext;

/// 调用记录更新事件名
const EVENT_CALL_RECORD_UPDATED: &str = "call-record:updated";

/// 写入调用记录初始数据
///
/// 返回调用记录 ID，供后续 `finish_*` 更新使用。
pub fn start_call_log(
    shared: &GatewaySharedState,
    ctx: &ForwardContext,
    api_key_secret_id: Option<String>,
) -> IcodeResult<String> {
    let route_mode = if ctx.kind.is_virtual() {
        RouteMode::VirtualFallback
    } else {
        RouteMode::Direct
    };

    let log = shared.call_records_handle.service().start_call(CreateModelCallLogInput {
        provider_id: ctx.upstream.provider.id.clone(),
        gateway_model_id: ctx.upstream.gateway_model_id.clone(),
        model_id: ctx.upstream.upstream_model_id.clone(),
        request_id: Some(ctx.upstream.request_id.clone()),
        route_mode,
        source: "gateway".to_string(),
        api_key_secret_id,
    })?;

    Ok(log.id)
}

/// 更新调用记录完成信息（基础字段）
pub fn finish_call_log(
    shared: &GatewaySharedState,
    log_id: &str,
    start_time: Instant,
    status_code: Option<i64>,
    error_message: Option<String>,
    prompt_tokens: Option<i64>,
) -> IcodeResult<()> {
    finish_call_log_full(
        shared, log_id, start_time, status_code, error_message, prompt_tokens,
        None, None, None, None, None,
    )
}

/// 更新调用记录完成信息（含完整 token 与费用数据）
#[allow(clippy::too_many_arguments)]
pub fn finish_call_log_full(
    shared: &GatewaySharedState,
    log_id: &str,
    start_time: Instant,
    status_code: Option<i64>,
    error_message: Option<String>,
    prompt_tokens: Option<i64>,
    completion_tokens: Option<i64>,
    total_tokens: Option<i64>,
    cached_tokens: Option<i64>,
    cache_hit: Option<bool>,
    price_per_1m_tokens: Option<f64>,
) -> IcodeResult<()> {
    let duration_ms = start_time.elapsed().as_millis() as i64;

    let mut update = UpdateModelCallLogInput {
        completed_at: Some(chrono::Utc::now().to_rfc3339()),
        duration_ms: Some(duration_ms),
        status_code,
        error_message,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cached_tokens,
        cache_hit,
        time_to_first_token_ms: None,
        price_per_1m_tokens,
    };

    if update.total_tokens.is_none() && update.prompt_tokens.is_some() {
        let pt = update.prompt_tokens.unwrap();
        let ct = update.completion_tokens.unwrap_or(0);
        update.total_tokens = Some(pt + ct);
    }

    let log = shared.call_records_handle.service().finish_call_with_duration_and_tokens_full(
        log_id, duration_ms, &update,
    )?;

    let _ = shared.app_handle.emit(EVENT_CALL_RECORD_UPDATED, &log);
    Ok(())
}

/// 在 SSE 流式响应结束后更新 usage 数据
///
/// 由流中间件在流完成回调中调用，使用 accumulator 中的 usage 数据。
pub fn finish_streaming_usage(
    shared: &GatewaySharedState,
    log_id: &str,
    duration_ms: i64,
    usage: &crate::modules::gateway_runtime::client::SseUsageData,
) {
    let update = UpdateModelCallLogInput {
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cached_tokens: usage.cached_tokens,
        cache_hit: usage.cache_hit,
        prompt_tokens: usage.prompt_tokens,
        completed_at: None,
        duration_ms: None,
        status_code: None,
        error_message: None,
        time_to_first_token_ms: None,
        price_per_1m_tokens: None,
    };
    let _ = shared
        .call_records_handle
        .service()
        .finish_call_with_duration_and_tokens_full(log_id, duration_ms, &update);
}

/// 模型价格查询（占位，未实现）
pub fn lookup_model_price(
    _shared: &GatewaySharedState,
    _provider_id: &str,
    _model_id: &str,
) -> Option<f64> {
    None
}
