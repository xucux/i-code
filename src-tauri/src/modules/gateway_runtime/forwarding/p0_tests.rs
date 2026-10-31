//! P0 / P1 虚拟路由端到端验证
//!
//! 使用本地 mock 上游（axum）+ 临时 SQLite，跑通真实 `ForwardPipeline`：
//! 虚拟路由解析 → 上游错误判定 → 切换下一条路由 → 降级写冷却 → attempts 落库。
//!
//! P0 覆盖三个核心场景：
//! 1. 首条路由 402（额度耗尽）→ 同一次请求内切到第 2 条并返回 200，首条写入长冷却；
//! 2. 首条路由 400 + `context_length_exceeded` → 请求级错误，不切换、不降级；
//! 3. 首条路由 429（限流）→ 立即切换（不重试），首条写入短冷却。
//!
//! P1 覆盖：
//! 4. 删除供应商模型时联动清理指向它的虚拟路由；
//! 5. 保存虚拟模型时拒绝指向不存在模型的路由。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response as AxumResponse},
    routing::post,
    Json, Router,
};
use serde_json::{json, Value};

use crate::db;
use crate::modules::ai_gateway::repository as gateway_repo;
use crate::modules::ai_gateway::types::CreateProviderInput;
use crate::modules::ai_gateway::AiGatewayServiceHandle;
use crate::modules::call_records::CallRecordsHandle;
use crate::modules::gateway_runtime::service::GatewaySharedState;
use crate::modules::logger::LoggerServiceHandle;
use crate::modules::secret::SecretServiceHandle;
use crate::modules::settings::SettingsServiceHandle;
use crate::modules::virtual_provider::repository as vp_repo;
use crate::modules::virtual_provider::types::{
    CreateVirtualProviderInput, SaveVirtualModelInput, SaveVirtualModelRouteInput,
};
use crate::modules::virtual_provider::VirtualProviderHandle;

use super::context::{ForwardRequest, GatewayProtocol};
use super::forwarder::ForwardPipeline;

/// 初始化临时数据库并返回其路径
fn init_test_db() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("i-code-p0-{}.db", uuid::Uuid::new_v4()));
    db::init_db_pool(&path).expect("初始化测试数据库失败");
    db::run_migrations().expect("执行迁移失败");
    path
}

/// 构造无窗口的共享状态（app_handle=None，事件发送被跳过）
fn build_shared() -> GatewaySharedState {
    let settings = SettingsServiceHandle::new();
    let secret = SecretServiceHandle::new(settings).expect("构造 Secret 服务失败");
    let ai_gateway = AiGatewayServiceHandle::new(secret.clone());
    GatewaySharedState {
        app_handle: None,
        ai_gateway_handle: ai_gateway,
        secret_handle: secret,
        logger_handle: LoggerServiceHandle::with_default(),
        virtual_provider_handle: VirtualProviderHandle::new(),
        call_records_handle: CallRecordsHandle::new(),
        inner_cli_api_key: "p0-test".to_string(),
    }
}

/// mock 上游句柄
struct MockUpstream {
    base_url: String,
    hits: Arc<AtomicUsize>,
}

#[derive(Clone)]
struct MockState {
    hits: Arc<AtomicUsize>,
    /// 失败状态码；0 表示始终成功
    fail_status: u16,
    fail_body: Value,
    /// true：始终返回失败；false：仅首次请求失败，之后成功
    fail_always: bool,
}

async fn mock_handler(State(st): State<MockState>) -> AxumResponse {
    let n = st.hits.fetch_add(1, Ordering::SeqCst);
    if st.fail_status > 0 && (st.fail_always || n == 0) {
        return (
            StatusCode::from_u16(st.fail_status).expect("非法状态码"),
            Json(st.fail_body.clone()),
        )
            .into_response();
    }
    (StatusCode::OK, Json(ok_body())).into_response()
}

/// OpenAI Chat Completions 成功响应
fn ok_body() -> Value {
    json!({
        "id": "chatcmpl-p0",
        "object": "chat.completion",
        "created": 0,
        "model": "mock-model",
        "choices": [{
            "index": 0,
            "message": { "role": "assistant", "content": "ok" },
            "finish_reason": "stop"
        }],
        "usage": { "prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2 }
    })
}

/// 启动 mock 上游；`fail_status = 0` 表示始终返回成功
async fn spawn_mock(fail_status: u16, fail_body: Value) -> MockUpstream {
    spawn_mock_inner(fail_status, fail_body, false).await
}

/// 启动始终返回失败的 mock 上游
async fn spawn_fail_always(fail_status: u16, fail_body: Value) -> MockUpstream {
    spawn_mock_inner(fail_status, fail_body, true).await
}

async fn spawn_mock_inner(
    fail_status: u16,
    fail_body: Value,
    fail_always: bool,
) -> MockUpstream {
    let hits = Arc::new(AtomicUsize::new(0));
    let state = MockState {
        hits: hits.clone(),
        fail_status,
        fail_body,
        fail_always,
    };
    let app = Router::new()
        .route("/v1/chat/completions", post(mock_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("绑定 mock 端口失败");
    let addr = listener.local_addr().expect("读取 mock 端口失败");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock 服务异常");
    });

    MockUpstream {
        base_url: format!("http://{}", addr),
        hits,
    }
}

/// 插入一个已启用的真实供应商，返回 ID
fn create_provider(slug: &str, base_url: &str) -> String {
    gateway_repo::insert_provider(
        &CreateProviderInput {
            slug: slug.to_string(),
            display_name: slug.to_string(),
            provider_type: "openai-chat-completion".to_string(),
            base_url: base_url.to_string(),
            use_raw_base_url: false,
            remark: None,
            transport: None,
            auth: None,
            auth_method: None,
            auto_fetch_official_models: false,
            is_enabled: true,
            is_media_generation: false,
            sort_order: None,
            balance_provider_json: None,
            timeout_json: None,
            retry_json: None,
            proxy_json: None,
            script_variables_json: None,
            extra_headers: None,
            default_models: None,
        },
        None,
        None,
        None,
        None,
    )
    .expect("插入供应商失败")
    .id
}

/// 创建虚拟供应商 + 虚拟模型（两条路由：priority 0 指向 bad，1 指向 good）
fn seed_virtual(alias: &str, bad_provider_id: &str, good_provider_id: &str) -> String {
    vp_repo::insert_provider(&CreateVirtualProviderInput {
        name: format!("vp-{alias}"),
        alias: alias.to_string(),
        display_name: Some(alias.to_string()),
        is_enabled: true,
        strategy: Some("fallback".to_string()),
        max_retries: 0,
        retry_interval_ms: 0,
    })
    .expect("插入虚拟供应商失败");

    let provider = vp_repo::find_provider_by_alias(alias)
        .expect("查询虚拟供应商失败")
        .expect("虚拟供应商不存在");

    vp_repo::save_model(&SaveVirtualModelInput {
        id: None,
        virtual_provider_id: provider.id,
        model_id: "mock-model".to_string(),
        display_name: None,
        is_enabled: true,
        routes: vec![
            SaveVirtualModelRouteInput {
                target_provider_id: bad_provider_id.to_string(),
                target_model_id: "mock-model".to_string(),
                priority: 0,
                enabled: true,
                extra_headers: None,
                extra_body: None,
            },
            SaveVirtualModelRouteInput {
                target_provider_id: good_provider_id.to_string(),
                target_model_id: "mock-model".to_string(),
                priority: 1,
                enabled: true,
                extra_headers: None,
                extra_body: None,
            },
        ],
    })
    .expect("保存虚拟模型失败")
    .id
}

/// 发起一次 Chat Completions 网关请求
async fn send_request(shared: &GatewaySharedState, model: &str) -> axum::response::Response {
    let body = json!({
        "model": model,
        "messages": [{ "role": "user", "content": "ping" }],
        "stream": false
    });
    ForwardPipeline::run(
        shared,
        ForwardRequest {
            protocol: GatewayProtocol::ChatCompletions,
            body,
            api_key_secret_id: None,
            request_headers_json: None,
        },
    )
    .await
    .expect("网关请求失败")
}

/// 读取虚拟模型下 priority=0 的路由
fn first_route(virtual_model_id: &str) -> crate::modules::virtual_provider::types::VirtualModelRoute {
    let routes = vp_repo::list_routes_by_virtual_model(virtual_model_id)
        .expect("查询路由失败");
    routes
        .into_iter()
        .min_by_key(|r| r.priority)
        .expect("虚拟模型没有任何路由")
}

#[tokio::test]
#[ignore = "端到端测试会初始化全局 DB 与本地 mock 上游，需单独运行：cargo test --bin i-code p0_virtual_failover -- --ignored"]
async fn p0_virtual_failover_end_to_end() {
    let db_path = init_test_db();
    let shared = build_shared();

    // ===== 场景 1：402 额度耗尽 → 同一次请求内切到第 2 条 =====
    let bad = spawn_mock(
        402,
        json!({"error": {"message": "quota exhausted", "type": "insufficient_quota", "code": "insufficient_quota"}}),
    )
    .await;
    let good = spawn_mock(0, json!({})).await;
    let bad_provider = create_provider("p0-quota-bad", &bad.base_url);
    let good_provider = create_provider("p0-quota-good", &good.base_url);
    let vm1 = seed_virtual("p0-quota", &bad_provider, &good_provider);

    let resp = send_request(&shared, "p0-quota/mock-model").await;
    assert_eq!(resp.status(), StatusCode::OK, "402 后应切换到第 2 条并返回 200");
    assert_eq!(bad.hits.load(Ordering::SeqCst), 1, "首条路由只应被请求 1 次（429/402 不重试）");
    assert_eq!(good.hits.load(Ordering::SeqCst), 1, "第 2 条路由应被请求 1 次");

    let route = first_route(&vm1);
    assert!(!route.is_healthy, "额度耗尽路由应被降级");
    assert_eq!(route.failure_kind.as_deref(), Some("quota_exhausted"));
    assert!(route.cooldown_until.is_some(), "额度耗尽应写入长冷却");

    // 模拟编辑弹窗保存（路由全量重写）：运行态健康 / 冷却必须保留，不能被表单覆盖
    let vm_record = vp_repo::find_model_by_id(&vm1).expect("读取虚拟模型失败");
    let current_routes = vp_repo::list_all_routes_by_virtual_model(&vm1).expect("读取路由失败");
    vp_repo::save_model(&SaveVirtualModelInput {
        id: Some(vm1.clone()),
        virtual_provider_id: vm_record.virtual_provider_id.clone(),
        model_id: vm_record.model_id.clone(),
        display_name: vm_record.display_name.clone(),
        is_enabled: vm_record.is_enabled,
        routes: current_routes
            .iter()
            .map(|r| SaveVirtualModelRouteInput {
                target_provider_id: r.target_provider_id.clone(),
                target_model_id: r.target_model_id.clone(),
                priority: r.priority,
                enabled: r.enabled,
                extra_headers: None,
                extra_body: None,
            })
            .collect(),
    })
    .expect("重新保存虚拟模型失败");
    let route = first_route(&vm1);
    assert!(!route.is_healthy, "编辑保存不应复活已降级路由");
    assert_eq!(route.failure_kind.as_deref(), Some("quota_exhausted"));
    assert!(route.cooldown_until.is_some(), "编辑保存不应清空冷却");

    // 验收：冷却期内该路由不参与选路（后续请求应只命中健康路由）
    let selected = shared
        .virtual_provider_handle
        .service()
        .resolve_routes(&vm_record.virtual_provider_id, "mock-model")
        .expect("解析候选路由失败");
    assert!(
        selected.iter().all(|r| r.target_provider_id != bad_provider),
        "冷却期内的额度耗尽路由不应被选中"
    );
    assert_eq!(selected.len(), 1, "只剩健康路由参与选路");

    // ===== 场景 2：400 上下文超限 → 请求级错误，不切换、不降级 =====
    let bad2 = spawn_mock(
        400,
        json!({"error": {"message": "maximum context length is 8192 tokens", "type": "invalid_request_error", "code": "context_length_exceeded"}}),
    )
    .await;
    let good2 = spawn_mock(0, json!({})).await;
    let bad2_provider = create_provider("p0-ctx-bad", &bad2.base_url);
    let good2_provider = create_provider("p0-ctx-good", &good2.base_url);
    let vm2 = seed_virtual("p0-ctx", &bad2_provider, &good2_provider);

    let resp = send_request(&shared, "p0-ctx/mock-model").await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "请求级错误应原样透传");
    assert_eq!(bad2.hits.load(Ordering::SeqCst), 1);
    assert_eq!(good2.hits.load(Ordering::SeqCst), 0, "请求级错误不应切换路由");

    let route = first_route(&vm2);
    assert!(route.is_healthy, "上下文超限不应降级路由");
    assert!(route.failure_kind.is_none());
    assert!(route.cooldown_until.is_none());

    // ===== 场景 3：429 限流 → 立即切换（不重试），写短冷却 =====
    let bad3 = spawn_mock(
        429,
        json!({"error": {"message": "Rate limit reached for requests", "type": "rate_limit_error", "code": "rate_limit_exceeded"}}),
    )
    .await;
    let good3 = spawn_mock(0, json!({})).await;
    let bad3_provider = create_provider("p0-rate-bad", &bad3.base_url);
    let good3_provider = create_provider("p0-rate-good", &good3.base_url);
    let vm3 = seed_virtual("p0-rate", &bad3_provider, &good3_provider);

    let resp = send_request(&shared, "p0-rate/mock-model").await;
    assert_eq!(resp.status(), StatusCode::OK, "429 后应切换到第 2 条");
    assert_eq!(bad3.hits.load(Ordering::SeqCst), 1, "429 不应在同路由上重试");
    assert_eq!(good3.hits.load(Ordering::SeqCst), 1);

    let route = first_route(&vm3);
    assert_eq!(route.failure_kind.as_deref(), Some("rate_limited"));
    assert!(route.cooldown_until.is_some(), "限流应写入短冷却");

    // ===== 场景 4：全部路由失败 → 每条路由只调用一次，返回最后错误 =====
    let quota = json!({"error": {"message": "quota exhausted", "code": "insufficient_quota"}});
    let bad4a = spawn_fail_always(402, quota.clone()).await;
    let bad4b = spawn_fail_always(402, quota).await;
    let bad4a_provider = create_provider("p0-all-bad-1", &bad4a.base_url);
    let bad4b_provider = create_provider("p0-all-bad-2", &bad4b.base_url);
    let vm4 = seed_virtual("p0-all", &bad4a_provider, &bad4b_provider);

    let resp = send_request(&shared, "p0-all/mock-model").await;
    assert_eq!(
        resp.status(),
        StatusCode::PAYMENT_REQUIRED,
        "全部失败应返回聚合状态码（402 优先于最后一条）"
    );
    let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .expect("读取聚合错误体失败");
    let body: Value = serde_json::from_slice(&body_bytes).expect("解析聚合错误体失败");
    assert_eq!(body["error"]["code"], "all_routes_failed");
    let attempts = body["error"]["details"]["attempts"]
        .as_array()
        .expect("聚合错误应包含 attempts 尝试链");
    assert_eq!(attempts.len(), 2, "两条路由的失败都应出现在尝试链中");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("第 2 条路由"),
        "聚合消息应逐条列出路由失败原因"
    );
    assert_eq!(bad4a.hits.load(Ordering::SeqCst), 1, "402 不应在同路由重试");
    assert_eq!(bad4b.hits.load(Ordering::SeqCst), 1);
    let routes4 = vp_repo::list_all_routes_by_virtual_model(&vm4).expect("读取路由失败");
    assert!(routes4.iter().all(|r| !r.is_healthy), "两条路由均应被降级");

    // 全部永久失活：兜底间隔内再次请求应直接返回 all_routes_cooling，不再调用上游
    let resp2 = send_request(&shared, "p0-all/mock-model").await;
    assert_eq!(resp2.status(), StatusCode::PAYMENT_REQUIRED);
    let body2 = axum::body::to_bytes(resp2.into_body(), 1024 * 1024)
        .await
        .expect("读取兜底错误体失败");
    let body2: Value = serde_json::from_slice(&body2).expect("解析兜底错误体失败");
    assert_eq!(body2["error"]["code"], "all_routes_cooling");
    assert_eq!(bad4a.hits.load(Ordering::SeqCst), 1, "兜底间隔内不应调用上游");
    assert_eq!(bad4b.hits.load(Ordering::SeqCst), 1);

    // 全部路由均已失活：将最近尝试时间改到 2 小时前（超过默认 30 分钟兜底间隔），
    // 下一次请求只允许尝试「最久未尝试」的一条（priority 0 的那条）
    {
        let conn = db::get_db_pool().expect("获取连接池失败").get().expect("获取连接失败");
        let old = (chrono::Utc::now() - chrono::Duration::minutes(120)).to_rfc3339();
        for r in &routes4 {
            conn.execute(
                "UPDATE virtual_model_routes SET last_attempted_at = ?1 WHERE id = ?2",
                rusqlite::params![old, r.id],
            )
            .expect("更新最近尝试时间失败");
        }
    }
    let resp3 = send_request(&shared, "p0-all/mock-model").await;
    assert_eq!(resp3.status(), StatusCode::PAYMENT_REQUIRED);
    assert_eq!(
        bad4a.hits.load(Ordering::SeqCst),
        2,
        "超过兜底间隔后应且仅应尝试最久未尝试的 1 条"
    );
    assert_eq!(bad4b.hits.load(Ordering::SeqCst), 1, "其余失活路由不应被调用");

    // 失败请求链查询：同一 request_id 的多次尝试应聚合为一条链，按 attempt_index 排序
    vp_repo::insert_route_attempt(
        &routes4[0].id,
        "p0-all",
        "req-chain-test",
        0,
        false,
        Some(402),
        Some("quota exhausted"),
        10,
    )
    .expect("写入尝试记录失败");
    vp_repo::insert_route_attempt(
        &routes4[1].id,
        "p0-all",
        "req-chain-test",
        1,
        false,
        Some(402),
        Some("quota exhausted"),
        10,
    )
    .expect("写入尝试记录失败");
    let chains = shared
        .virtual_provider_handle
        .service()
        .list_recent_failed_chains(20)
        .expect("查询失败请求链失败");
    let chain = chains
        .iter()
        .find(|c| c.request_id == "req-chain-test")
        .expect("应返回该失败请求链");
    assert_eq!(chain.virtual_provider_alias, "p0-all");
    assert_eq!(chain.attempts.len(), 2);
    assert_eq!(chain.attempts[0].attempt_index, 0);
    assert_eq!(chain.attempts[0].status_code, Some(402));

    // 清理临时数据库（连接池持有句柄时 Windows 删除可能失败，忽略即可）
    drop(shared);
    let _ = std::fs::remove_file(db_path);
}

/// 插入一条网关模型（连同其 model_configs 记录），返回 gateway_models.id
fn seed_gateway_model(provider_id: &str, model_id: &str) -> String {
    let conn = db::get_db_pool().expect("获取连接池失败").get().expect("获取连接失败");
    let now = chrono::Utc::now().to_rfc3339();
    let config_id = uuid::Uuid::new_v4().to_string();
    let gateway_model_id = uuid::Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO model_configs (id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        rusqlite::params![config_id, model_id, now],
    )
    .expect("插入模型配置失败");
    conn.execute(
        "INSERT INTO gateway_models
            (id, provider_id, model_config_id, model_id, source, is_exposed, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, 'manual', 1, ?5, ?5)",
        rusqlite::params![gateway_model_id, provider_id, config_id, model_id, now],
    )
    .expect("插入网关模型失败");

    gateway_model_id
}

/// P1：删除供应商模型时联动清理虚拟路由；保存时拒绝指向不存在模型的路由
#[tokio::test]
#[ignore = "端到端测试会初始化全局 DB，需单独运行：cargo test --bin i-code p1_route_cleanup -- --ignored"]
async fn p1_route_cleanup_and_save_validation() {
    let db_path = init_test_db();
    let shared = build_shared();
    let ai_gateway = shared.ai_gateway_handle.service();
    let virtual_provider = crate::modules::virtual_provider::service::VirtualProviderService::new();

    let provider_id = create_provider("p1-provider", "http://127.0.0.1:9");
    let gateway_model_id = seed_gateway_model(&provider_id, "kept-model");

    // 创建一个引用 kept-model 的虚拟路由
    vp_repo::insert_provider(&CreateVirtualProviderInput {
        name: "vp-p1".to_string(),
        alias: "p1-vp".to_string(),
        display_name: None,
        is_enabled: true,
        strategy: Some("fallback".to_string()),
        max_retries: 0,
        retry_interval_ms: 0,
    })
    .expect("插入虚拟供应商失败");
    let vp = vp_repo::find_provider_by_alias("p1-vp")
        .expect("查询虚拟供应商失败")
        .expect("虚拟供应商不存在");
    let vm = vp_repo::save_model(&SaveVirtualModelInput {
        id: None,
        virtual_provider_id: vp.id.clone(),
        model_id: "route-model".to_string(),
        display_name: None,
        is_enabled: true,
        routes: vec![SaveVirtualModelRouteInput {
            target_provider_id: provider_id.clone(),
            target_model_id: "kept-model".to_string(),
            priority: 0,
            enabled: true,
            extra_headers: None,
            extra_body: None,
        }],
    })
    .expect("保存虚拟模型失败");
    assert_eq!(
        vp_repo::list_all_routes_by_virtual_model(&vm.id).expect("查询路由失败").len(),
        1
    );

    // 删除供应商模型 → 引用它的虚拟路由应被联动清理
    ai_gateway
        .delete_gateway_model(&gateway_model_id)
        .expect("删除网关模型失败");
    assert!(
        vp_repo::list_all_routes_by_virtual_model(&vm.id)
            .expect("查询路由失败")
            .is_empty(),
        "删除供应商模型后应清理指向它的虚拟路由"
    );

    // 保存校验：目标模型不存在时应返回 VALIDATION
    let err = virtual_provider
        .save_model(
            ai_gateway,
            SaveVirtualModelInput {
                id: None,
                virtual_provider_id: vp.id.clone(),
                model_id: "ghost-route-model".to_string(),
                display_name: None,
                is_enabled: true,
                routes: vec![SaveVirtualModelRouteInput {
                    target_provider_id: provider_id.clone(),
                    target_model_id: "ghost-model".to_string(),
                    priority: 0,
                    enabled: true,
                    extra_headers: None,
                    extra_body: None,
                }],
            },
        )
        .expect_err("目标模型不存在时应拒绝保存");
    assert_eq!(err.code, "VALIDATION");
    assert!(
        err.message.contains("不存在"),
        "错误信息应提示模型不存在: {}",
        err.message
    );

    drop(shared);
    let _ = std::fs::remove_file(db_path);
}
