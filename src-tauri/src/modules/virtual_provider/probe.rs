//! # 虚拟路由探活（模型级）
//!
//! 供调度器（被动恢复）与「测试路由」按钮共用。优先对目标模型发起**最小真实请求**
//! （`max_tokens = 1`），只有 2xx 才视为健康；额度耗尽 / 鉴权 / 模型不存在等会被
//! 错误判定器识别为路由级故障。对不支持对话接口的供应商类型退化为
//! `GET /v1/models` + 目标模型存在性校验；对话探活返回“无法判定”（如上游拒绝
//! 最小请求）时也会回退到同一校验，避免已降级路由因探活方式不兼容而永久无法恢复。
//!
//! 注意：探活只针对已降级 / 冷却中的路由触发（调度器每 60s 扫描），不会对健康路由
//! 产生额外调用与费用。

use reqwest::Client;
use serde_json::json;

use crate::modules::ai_gateway::types::{AuthConfig, Provider};
use crate::modules::gateway_runtime::auth_resolver::{
    resolve_auth, AuthCredential, AuthResolution,
};
use crate::modules::gateway_runtime::forwarding::error_classifier::{
    classify, classify_transport_error, RouteVerdict,
};
use crate::modules::virtual_provider::types::{RouteFailureKind, RoutePenalty};

/// 探活失败信息
pub struct ProbeFailure {
    pub verdict: RouteVerdict,
    pub status: Option<u16>,
    pub message: String,
}

/// 探活结果
pub enum ProbeOutcome {
    /// 2xx（且模型存在）：健康
    Success,
    /// 明确的失败（额度 / 鉴权 / 模型 / 限流 / 5xx / 超时）：按判定降级
    Failure(ProbeFailure),
    /// 请求级错误或无法识别：不降级，仅记录失败原因
    Inconclusive(String),
    /// 上游不支持该探活方式：不改变健康状态
    Unsupported,
}

/// 探活方式
enum ProbeKind {
    /// OpenAI 兼容：`POST /v1/chat/completions`
    OpenAiChat,
    /// Anthropic：`POST /v1/messages`
    AnthropicMessages,
    /// 仅校验 `/v1/models` 中的模型存在性
    ModelsOnly,
}

fn probe_kind(provider_type: &str) -> ProbeKind {
    match provider_type {
        "anthropic" | "claude-code" => ProbeKind::AnthropicMessages,
        // opencode-free 上游强制要求 stream=true + 专用工具，最小对话请求必被拒绝，
        // 因此退化为模型存在性校验，避免探活永远返回 Inconclusive 而无法恢复。
        "openai-chat-completion" | "custom" | "ollama" | "xai-grok-build" | "github-copilot" => {
            ProbeKind::OpenAiChat
        }
        _ => ProbeKind::ModelsOnly,
    }
}

/// 对单条路由的目标模型发起探活
pub async fn probe_route(
    client: &Client,
    provider: &Provider,
    target_model_id: &str,
    auth_config: &Option<AuthConfig>,
    extra_headers: &[(String, String)],
) -> ProbeOutcome {
    let base = provider.base_url.trim_end_matches('/');

    if target_model_id.trim().is_empty() {
        return ProbeOutcome::Unsupported;
    }

    // 解析认证（失败视为供应商配置错误）
    let resolution = match auth_config {
        Some(config) => match resolve_auth(config) {
            Ok(r) => Some(r),
            Err(e) => {
                return ProbeOutcome::Failure(ProbeFailure {
                    verdict: RouteVerdict::new(
                        RouteFailureKind::ProviderMisconfigured,
                        RoutePenalty::Long,
                    ),
                    status: None,
                    message: format!("认证解析失败: {}", e.message),
                });
            }
        },
        None => None,
    };

    match probe_kind(&provider.provider_type) {
        ProbeKind::OpenAiChat => {
            let url = format!("{base}/v1/chat/completions");
            let mut req = client.post(&url).header("Content-Type", "application/json");
            if let Some(res) = &resolution {
                if let Some(cred) = &res.credential {
                    let token = match cred {
                        AuthCredential::Bearer(t) | AuthCredential::ApiKey(t) => t,
                    };
                    req = req.header("Authorization", format!("Bearer {token}"));
                }
                for (k, v) in &res.extra_headers {
                    req = req.header(k, v);
                }
            }
            for (k, v) in extra_headers {
                req = req.header(k, v);
            }
            let body = json!({
                "model": target_model_id,
                "messages": [{ "role": "user", "content": "ping" }],
                "max_tokens": 1,
                "stream": false,
            });
            let outcome = send_and_classify(req.json(&body)).await;
            fallback_on_inconclusive(
                outcome,
                client,
                base,
                target_model_id,
                resolution.as_ref(),
                extra_headers,
            )
            .await
        }
        ProbeKind::AnthropicMessages => {
            let url = format!("{base}/v1/messages");
            let mut req = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("anthropic-version", "2023-06-01");
            if let Some(res) = &resolution {
                if let Some(cred) = &res.credential {
                    match cred {
                        AuthCredential::ApiKey(key) => {
                            req = req.header("x-api-key", key.as_str());
                            req = req.header("Authorization", format!("Bearer {key}"));
                        }
                        AuthCredential::Bearer(token) => {
                            req = req.header("Authorization", format!("Bearer {token}"));
                        }
                    }
                }
                for (k, v) in &res.extra_headers {
                    req = req.header(k, v);
                }
            }
            for (k, v) in extra_headers {
                req = req.header(k, v);
            }
            let body = json!({
                "model": target_model_id,
                "max_tokens": 1,
                "messages": [{ "role": "user", "content": "ping" }],
            });
            let outcome = send_and_classify(req.json(&body)).await;
            fallback_on_inconclusive(
                outcome,
                client,
                base,
                target_model_id,
                resolution.as_ref(),
                extra_headers,
            )
            .await
        }
        ProbeKind::ModelsOnly => {
            probe_models_existence(
                client,
                base,
                target_model_id,
                resolution.as_ref(),
                extra_headers,
            )
            .await
        }
    }
}

/// 对话探活返回“无法判定”时，回退到 `/v1/models` 存在性校验
///
/// 某些模型会拒绝我们构造的最小请求（如只接受 `max_completion_tokens`），
/// 若因此一直 Inconclusive，已降级路由将永远无法恢复。回退校验只在
/// “模型存在”时按成功处理；端点不可用/无法识别时仍保留原始 Inconclusive。
async fn fallback_on_inconclusive(
    outcome: ProbeOutcome,
    client: &Client,
    base: &str,
    target_model_id: &str,
    resolution: Option<&AuthResolution>,
    extra_headers: &[(String, String)],
) -> ProbeOutcome {
    if !matches!(outcome, ProbeOutcome::Inconclusive(_)) {
        return outcome;
    }
    match probe_models_existence(client, base, target_model_id, resolution, extra_headers).await {
        ProbeOutcome::Success => ProbeOutcome::Success,
        _ => outcome,
    }
}

/// 校验目标模型是否存在于上游 `/v1/models`（Key 级可用性 + 模型存在性）
async fn probe_models_existence(
    client: &Client,
    base: &str,
    target_model_id: &str,
    resolution: Option<&AuthResolution>,
    extra_headers: &[(String, String)],
) -> ProbeOutcome {
    let url = format!("{base}/v1/models");
    let mut req = client.get(&url);
    if let Some(res) = resolution {
        if let Some(cred) = &res.credential {
            let token = match cred {
                AuthCredential::Bearer(t) | AuthCredential::ApiKey(t) => t,
            };
            req = req.header("Authorization", format!("Bearer {token}"));
        }
        for (k, v) in &res.extra_headers {
            req = req.header(k, v);
        }
    }
    for (k, v) in extra_headers {
        req = req.header(k, v);
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => return transport_failure(&e),
    };
    let status = resp.status().as_u16();
    if status == 404 {
        return ProbeOutcome::Unsupported;
    }
    if !(200..300).contains(&status) {
        return classify_response(status, resp).await;
    }
    let body = match resp.text().await {
        Ok(t) => t,
        Err(_) => return ProbeOutcome::Success,
    };
    match model_in_list(&body, target_model_id) {
        Some(false) => ProbeOutcome::Failure(ProbeFailure {
            verdict: RouteVerdict::new(RouteFailureKind::ModelUnavailable, RoutePenalty::Long),
            status: Some(status),
            message: format!("目标模型 {target_model_id} 不在上游模型列表中"),
        }),
        _ => ProbeOutcome::Success,
    }
}

/// 发送请求并按错误判定器分类
async fn send_and_classify(req: reqwest::RequestBuilder) -> ProbeOutcome {
    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => return transport_failure(&e),
    };
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        return ProbeOutcome::Success;
    }
    classify_response(status, resp).await
}

/// 读取错误体并分类
async fn classify_response(status: u16, resp: reqwest::Response) -> ProbeOutcome {
    let body = resp.text().await.unwrap_or_default();
    match classify(status, Some(body.as_bytes())) {
        Some(c) if c.verdict.failover => ProbeOutcome::Failure(ProbeFailure {
            verdict: c.verdict,
            status: Some(status),
            message: c.message,
        }),
        Some(c) => ProbeOutcome::Inconclusive(if c.message.is_empty() {
            format!("HTTP {status}")
        } else {
            c.message
        }),
        None => ProbeOutcome::Inconclusive(format!("HTTP {status}")),
    }
}

/// 网络层错误 → 路由级故障（超时/连接失败均视为短冷却）
fn transport_failure(e: &reqwest::Error) -> ProbeOutcome {
    let message = if e.is_timeout() {
        "探活请求超时".to_string()
    } else if e.is_connect() {
        "探活连接失败".to_string()
    } else {
        format!("探活网络错误: {e}")
    };
    ProbeOutcome::Failure(ProbeFailure {
        verdict: classify_transport_error(&message),
        status: None,
        message,
    })
}

/// 判断目标模型是否出现在 `/v1/models` 响应中
///
/// 兼容 OpenAI（`data[].id`）、Gemini（`models[].name` = `models/xxx`）与裸数组格式。
/// 返回 `None` 表示响应结构无法识别（不阻断探活）。
fn model_in_list(body: &str, target_model_id: &str) -> Option<bool> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let arr = v
        .get("data")
        .and_then(|d| d.as_array())
        .or_else(|| v.get("models").and_then(|m| m.as_array()))
        .or_else(|| v.as_array())?;

    let found = arr.iter().any(|item| {
        let id = item
            .get("id")
            .and_then(|x| x.as_str())
            .or_else(|| item.get("name").and_then(|x| x.as_str()))
            .or_else(|| item.as_str());
        match id {
            Some(id) => {
                id == target_model_id || id.strip_prefix("models/") == Some(target_model_id)
            }
            None => false,
        }
    });
    Some(found)
}
