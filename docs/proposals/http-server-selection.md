# 本地 HTTP 网关 Server 选型方案对比

> 状态：已采用方案 A（axum），由 `Cargo.toml` 与 `docs/development.md` §5.8 确定。
> 关联：`docs/development.md` §5.8 gateway-runtime、`Cargo.toml` 依赖配置

## 背景

应用需要在本地启动 HTTP Server，对外暴露 OpenAI 兼容的 API：

- `GET /health`、`GET /readyz`：存活与就绪检查
- `GET /v1/models`：模型列表
- `POST /v1/chat/completions`：聊天补全（支持 SSE 流式响应）
- `POST /v1/responses`：OpenAI Responses API
- `POST /v1/embeddings`：嵌入向量（可选）

约束：

- 默认监听 `127.0.0.1:54321`，支持 `0.0.0.0`
- 必须支持流式响应（SSE），用于 AI 模型 token-by-token 输出
- 需要中间件：API Key 校验、请求日志、超时控制、CORS
- 在 Tauri 进程内运行，不能启动独立进程
- 与 Tauri 的 `tokio` 运行时共享

## 方案 A：axum（已选）

**依赖**：`axum = "0.7"`、`tower = "0.5"`、`tower-http = "0.6"`、`hyper = "1"`

### 设计要点

- 基于 `hyper` 与 `tokio`，异步原生
- 路由用宏与链式 `Router::new().route(...)` 声明
- 中间件通过 `tower::Layer` 组合：`CORS / Trace / Timeout / RequestBodyLimit`
- 流式响应：`axum::response::sse::Sse` 或自定义 `Body::from_stream`
- 状态共享：`Router::with_state(AppState)` 传入 Tauri Handle

### 优点

- **生态成熟**：`tower` 中间件生态丰富，与 `tonic` / `reqwest` 共享
- **类型安全路由**：handler 参数自动从 `Path / Query / Json / State` 提取
- **流式响应一等公民**：原生 SSE 支持，无需手动管理 chunk
- **Tauri 兼容性好**：与 Tauri 的 `tokio` 运行时直接复用
- **社区活跃**：axum 是 Rust Web 框架主流选择之一

### 缺点

- 二进制体积略增（约 1-2MB）
- API 在 0.6 → 0.7 间有 breaking changes，需锁定版本

### 适用场景

- 需要中间件链与复杂路由的应用
- 长期维护、社区支持重要

## 方案 B：hyper 原生

**依赖**：仅 `hyper = "1"`（含 `full` feature）

### 设计要点

- 直接使用 `hyper::server::conn::http1::Builder` 与 `service::service_fn`
- 手动匹配路径与方法
- SSE 通过 `Body::from_stream` + `Stream` 手动构造

### 优点

- 依赖最少，二进制最小
- 完全控制请求/响应生命周期
- 学习 hyper 底层原理

### 缺点

- **实现量大**：路由、参数提取、中间件都要手写
- 流式响应处理复杂，易出错
- 缺少类型安全的提取器
- 维护成本高

### 适用场景

- 嵌入式或对二进制体积敏感
- 路由数量极少（少于 5 个固定路径）

## 方案 C：actix-web

**依赖**：`actix-web = "4"`

### 设计要点

- 自带 actor 模型运行时（actix-rt）
- 路由宏：`#[get("/v1/models")]`
- 中间件：`wrap()` 链式组合

### 优点

- 性能 benchmarks 历史最高
- 中间件生态丰富
- 文档完整

### 缺点

- **运行时与 Tauri 不兼容**：actix-rt 与 tokio runtime 需要单独管理
- 需要在 Tauri 进程内启动独立 actix runtime，与 Tauri 事件循环冲突
- actor 模式学习曲线陡
- Tauri 社区较少使用

### 适用场景

- 独立后端服务（非嵌入 Tauri）

## 方案 D：warp

**依赖**：`warp = "0.3"`

### 设计要点

- Filter 组合式路由：`warp::path("v1").and(warp::path("models"))`
- 基于 `hyper` + `tokio`

### 优点

- 组合式 API 优雅
- 类型推导好

### 缺点

- 复杂路由的 Filter 链可读性下降
- 中间件生态少于 axum
- 维护活跃度低于 axum

### 适用场景

- 简单 API 服务

## 方案 E：Tauri 内置 HTTP 插件

**依赖**：无（Tauri 自带），但仅暴露静态文件服务

### 设计要点

- 通过 `tauri-plugin-localhost` 或自定义 IPC

### 优点

- 零额外依赖

### 缺点

- **不支持动态路由**：仅适合静态资源服务
- **不支持 SSE**：无法实现流式响应
- 完全不适合 API 网关场景

### 适用场景

- 仅托管前端静态文件

## 选型对比

| 维度 | axum | hyper | actix-web | warp | Tauri 内置 |
|------|------|-------|-----------|------|-----------|
| 流式 SSE 支持 | 一等公民 ✓ | 手动 | 支持 ✓ | 支持 ✓ | 不支持 ✗ |
| 中间件生态 | tower 生态 ✓ | 自写 | 丰富 ✓ | 一般 | 无 ✗ |
| Tauri 兼容 | 完美 ✓ ✓ | 完美 ✓ ✓ | 冲突 ✗ | 良好 ✓ | 完美 ✓ ✓ |
| 实现成本 | 低 ✓ | 高 ✗ | 中 | 中 | 不适用 |
| 二进制体积 | 中 | 最小 ✓ | 大 | 中 | 最小 ✓ |
| 社区活跃度 | 高 ✓ ✓ | 高 | 高 | 中 | N/A |
| 类型安全 | 强 ✓ | 弱 | 强 ✓ | 强 ✓ | N/A |

## 决策

**采用方案 A：axum**

### 决策依据

1. `docs/development.md` §11 已指定 axum 作为 HTTP Server 依赖
2. `Cargo.toml` 已引入 `axum`、`tower`、`tower-http`、`hyper`
3. 与 Tauri 的 `tokio` 运行时共享，无 runtime 冲突
4. tower 中间件生态满足 CORS / Trace / Timeout / Limit 需求
5. SSE 一等公民支持，契合 AI 网关流式响应需求
6. 类型安全的 handler 提取器降低 bug 风险

## 架构草图（方案 A 落地参考）

```
┌──────────────────────────────────────────────────┐
│ Tauri 进程                                        │
│ ┌────────────────────────────────────────────┐  │
│ │ tokio runtime（共享）                       │  │
│ │ ┌──────────────────────────────────────┐  │  │
│ │ │ axum::Server::bind(addr).serve(...)  │  │  │
│ │ │                                       │  │  │
│ │ │ Router::new()                          │  │  │
│ │ │   .route("/health", get(...))         │  │  │
│ │ │   .route("/v1/models", get(...))      │  │  │
│ │ │   .route("/v1/chat/completions",     │  │  │
│ │ │         post(chat_completions))       │  │  │
│ │ │   .layer(TraceLayer)                  │  │  │
│ │ │   .layer(TimeoutLayer)                │  │  │
│ │ │   .layer(CorsLayer)                   │  │  │
│ │ │   .with_state(AppState {              │  │  │
│ │ │     ai_gateway: Arc<...>,             │  │  │
│ │ │     secret: Arc<...>,                 │  │  │
│ │ │     call_records: Arc<...>,           │  │  │
│ │ │   })                                  │  │  │
│ │ └──────────────────────────────────────┘  │  │
│ └────────────────────────────────────────────┘  │
│ ┌──────────────────────────────────────────┐  │
│ │ Tauri 事件循环 + Webview                  │  │
│ └──────────────────────────────────────────┘  │
└──────────────────────────────────────────────┘
```

### 生命周期

1. 前端调用 `invoke('gateway_runtime_start')`
2. 后端从 `SettingsServiceHandle` 读取监听地址
3. `tokio::spawn` 启动 axum Server，句柄存入 Tauri State
4. 监听 `oneshot::Receiver` 实现优雅停止
5. 前端 `invoke('gateway_runtime_stop')` 发送停止信号

## 参考实现

- axum 官方 SSE 示例：https://github.com/tokio-rs/axum/blob/main/examples/sse/src/main.rs
- tower-http 中间件：https://docs.rs/tower-http/latest/tower_http/
- Tauri + axum 集成范例：https://github.com/tauri-apps/tauri/discussions/3824
