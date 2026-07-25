# 参考项目 Tokenizer 模块分析报告

> 分析对象：`vscode-unify-chat-provider-7.12.3/src/tokenizer/`  
> 生成时间：2026-07-16

---

## 1. 模块职责概述

Tokenizer 模块的核心职责是**估算 LLM 对话上下文的 token 用量**，供 VS Code Language Model Chat Provider API 的 `provideTokenCount` 回调使用。VS Code 依赖此计数来：

- 判断当前对话上下文是否接近模型上下文窗口上限
- 决定是否触发上下文压缩（context compaction）
- 在 UI 中展示 token 用量提示

由于不同模型/供应商使用不同的分词算法，精确 token 计数在跨供应商场景下**几乎不可能**，因此该模块提供了多种策略，从粗略近似到精确分词，按需选择。

---

## 2. 文件结构

| 文件 | 职责 |
|------|------|
| `tokenizers.ts` | 注册表与统一入口：定义 5 种 tokenizer，提供 ID 解析与乘数解析 |
| `char4.ts` | 最简近似算法：4 字符 ≈ 1 token |
| `conservative.ts` | 保守估算算法：3 UTF-8 字节 ≈ 1 token + 消息开销 + 图片固定 512 token |
| `content.ts` | 消息内容提取工具：从 `LanguageModelChatRequestMessage` 中提取纯文本与额外 token |
| `openai.ts` | OpenAI tiktoken 精确分词：按模型自动选择编码（o200k / cl100k / p50k / gpt2） |
| `deepseek.ts` | DeepSeek 官方分词：加载 HuggingFace tokenizer JSON 文件进行编码 |

---

## 3. 五种 Tokenizer 策略详解

### 3.1 `default` / `char4` — 字符近似（≈4字符/token）

```ts
return Math.ceil(content.length / 4);
```

- **算法**：字符串长度 ÷ 4，向上取整
- **适用**：通用 fallback，VS Code 官方近似行为
- **优点**：零依赖、O(1) 计算、速度快
- **缺点**：对中文/多字节语言偏差大（中文约 1.5-2 字符/token，远少于 4）
- **输入处理**：仅提取 `LanguageModelTextPart`，忽略图片/工具调用等

### 3.2 `conservative` — 保守估算（3 UTF-8 字节/token）

- **算法**：`ceil(utf8Bytes / 3) + extraTokens`
- **常量**：
  - `BYTES_PER_TOKEN = 3`：每 3 个 UTF-8 字节算 1 token（比 char4 更保守）
  - `MESSAGE_OVERHEAD_TOKENS = 4`：每条消息额外 4 token 开销（对齐 OpenAI 消息格式开销）
  - `IMAGE_PART_TOKENS = 512`：每个图片部分固定 512 token
- **输入处理**：完整遍历消息所有 Part 类型：
  - `LanguageModelTextPart` → UTF-8 字节计数
  - `LanguageModelThinkingPart` → 思维链文本字节计数
  - `LanguageModelToolCallPart` → 工具名 + 输入 JSON 字节计数
  - `LanguageModelToolResultPart` → 递归计数子内容
  - `LanguageModelPromptTsxPart` → TSX 值序列化后字节计数
  - `LanguageModelDataPart` → 区分内部标记（跳过）、图片标记（+512）、其他（+byteLength）
  - `Uint8Array` → +byteLength
- **设计意图**：宁可高估也不低估，确保不超出上下文窗口，代价是可能过早触发压缩

### 3.3 `openai` — OpenAI tiktoken 精确分词

- **依赖**：`tiktoken`（OpenAI 官方 BPE 分词库的 WASM 移植）
- **模型→编码映射**：

  | 编码名 | 模型前缀/精确匹配 |
  |--------|------------------|
  | `o200k_base` | gpt-4o、gpt-4.1、gpt-4.5、gpt-5、o1、o3、o4、chatgpt-4o、codex-mini 等 |
  | `cl100k_base` | gpt-4-turbo、gpt-4-32k、gpt-3.5-turbo、babbage-002、davinci-002 等 |
  | `p50k_base` | text-davinci-003、code-davinci-002 等 |
  | `gpt2` | gpt2 |

- **缓存**：`Map<TiktokenEncoding, Tiktoken>`，同一编码只初始化一次
- **降级**：try-catch 内执行，失败时 fallback 到 `char4`
- **流程**：`collectTokenizedInput` 提取文本 → `resolveOpenAIEncodingName` 选编码 → `getTokenizer` 取缓存/创建 → `tokenizer.encode(textContent).length + extraTokens`

### 3.4 `deepseek` — DeepSeek 官方分词

- **依赖**：`@huggingface/tokenizers`（HuggingFace tokenizers 的 Node.js 绑定）
- **数据文件**：`data/tokenizers/deepseek/tokenizer.json` + `tokenizer_config.json`
- **懒加载**：首次调用时加载，Promise 缓存，加载失败重置 Promise 允许重试
- **编码参数**：`add_special_tokens: false`
- **降级**：同 openai，失败时 fallback 到 `char4`

### 3.5 策略对比矩阵

| 维度 | char4 | conservative | openai | deepseek |
|------|-------|-------------|--------|----------|
| 精度 | 低 | 中（偏高估） | 高（仅 OpenAI 模型） | 高（仅 DeepSeek 模型） |
| 速度 | 最快 | 快 | 中（WASM 编码） | 中（首次加载慢，后续快） |
| 依赖 | 无 | 无 | tiktoken (WASM) | @huggingface/tokenizers |
| 多字节语言 | 偏差大 | 偏保守 | 精确 | 精确 |
| 消息开销 | 不计 | +4/msg | 通过 extraTokens | 通过 extraTokens |
| 图片处理 | 不计 | 512/img | 512/img | 512/img |
| 工具调用 | 不计 | 计入 | 计入 | 计入 |
| 思维链 | 不计 | 计入 | 计入 | 计入 |

---

## 4. 公共层：`content.ts` 消息提取器

`content.ts` 是 `openai.ts` 和 `deepseek.ts` 共用的工具函数，职责是将 `LanguageModelChatRequestMessage` 拆解为：

```ts
type TokenizedInput = {
  textContent: string;    // 所有文本部分拼接的纯文本
  extraTokens: number;    // 非文本部分折算的 token 数（图片 512、二进制 byteLength）
};
```

处理逻辑与 `conservative.ts` 高度同构，区别在于：
- `conservative.ts` 直接按 UTF-8 字节计数（`Buffer.byteLength`）
- `content.ts` 收集纯文本字符串，交给下游真正的 BPE 分词器编码

两者共享的常量与 Part 类型处理一致（`MESSAGE_OVERHEAD_TOKENS = 4`、`IMAGE_PART_TOKENS = 512`）。

---

## 5. 注册表与调用链

### 5.1 注册表（`tokenizers.ts`）

```ts
TOKENIZERS = {
  default:      → char4
  conservative: → conservative
  char4:        → char4
  openai:       → openai (tiktoken)
  deepseek:     → deepseek (HuggingFace)
}
```

辅助函数：
- `isTokenizerId(value)` — 类型守卫
- `resolveTokenizerId(value)` — 未知值回退到 `default`
- `resolveTokenCountMultiplier(value)` — 非正/非有限回退到 `1.0`

### 5.2 模型配置中的 tokenizer 字段

在 `types.ts` 的 `ModelConfig` 中：

```ts
tokenizer?: TokenizerId;            // 选择哪种分词策略，默认 'default'
tokenCountMultiplier?: number;       // 最终计数的乘数，默认 1.0
```

### 5.3 调用链（`service.ts` → `provideTokenCount`）

```
VS Code 调用 provideTokenCount(model, text, token)
  → findProviderAndModel(model.id) 找到模型配置
  → resolveTokenizerId(model.tokenizer) 解析策略 ID
  → TOKENIZERS[id].provideTokenCount(model, text, token) 执行计数
  → 失败时 fallback 到 TOKENIZERS.default
  → resolveTokenCountMultiplier(model.tokenCountMultiplier) 解析乘数
  → return ceil(base × multiplier)
```

两层降级保障：策略执行失败 → fallback default；整个链路异常 → 返回 0。**永远不抛错**。

---

## 6. 设计亮点与取舍

### 6.1 亮点

1. **策略可插拔**：新增供应商分词器只需在 `TOKENIZERS` 注册一项 + 实现一个 `ProvideTokenCountFn`
2. **双层降级**：策略失败 → char4 fallback；再失败 → 返回 0；绝不阻塞主流程
3. **乘数机制**：`tokenCountMultiplier` 允许用户微调，补偿特定模型的系统性偏差
4. **懒加载 + 缓存**：tiktoken 和 DeepSeek tokenizer 只在首次使用时加载，之后复用实例
5. **完整 Part 覆盖**：conservative/content 两个提取器覆盖了 VS Code LM API 的所有 Part 类型（Text、Thinking、ToolCall、ToolResult、PromptTsx、Data、Uint8Array）

### 6.2 取舍

1. **char4 对 CJK 偏差大**：中文约 1.5 字符/token，char4 按 4 字符/token 严重低估；conservative 按 3 字节/token 更合理（中文字符 ≈ 3 UTF-8 字节 ≈ 1 token）
2. **openai/deepseek 仅对各自模型精确**：用 openai tokenizer 算 Anthropic 模型的 token 数会有偏差
3. **图片 token 固定 512**：实际图片 token 取决于分辨率，OpenAI 公式为 `ceil(√(h/512) × √(w/512)) × 170 + 85`，此处简化为常量
4. **无 Anthropic 专用分词器**：Anthropic 未开源其分词器，参考项目未提供对应实现

---

## 7. 对 i-code 项目的启示

i-code 作为本地 AI Gateway，token 计数的用途场景包括：

| 场景 | 说明 |
|------|------|
| 上下文窗口校验 | 代理请求前估算 token 用量，超限时提前返回错误而非透传上游 400 |
| 调用记录统计 | 在 `call-records` 中记录请求/响应的 token 数（估算或从上游响应头提取） |
| 虚拟供应商选路 | 多模型故障转移时，可参考 token 用量选择上下文窗口足够的模型 |
| 余额/配额预判 | 估算本次调用 token 数，与剩余额度比较决定是否放行 |

### 建议的实现路径

1. **Rust 后端实现**：i-code 的网关在 Rust/axum 侧，token 计数应在后端完成
   - `conservative` 策略最易移植（纯字节运算，无外部依赖）
   - `char4` 作为快速 fallback
   - tiktoken 有 Rust 移植（`tiktoken-rs`），可按需引入
2. **前端可选展示**：在供应商/模型编辑页提供 tokenizer 选择和乘数配置（与参考项目 `ModelConfig` 一致）
3. **类型对齐**：在 `ModelConfig` 的 `types.rs` / `types.ts` 中保留 `tokenizer` 和 `token_count_multiplier` 字段
4. **渐进式**：先实现 conservative + char4，满足网关预校验需求；后续按需引入 tiktoken-rs 支持精确计数

---

## 8. 关键类型速查

```ts
// 分词函数签名
type ProvideTokenCountFn = (
  model: LanguageModelChatInformation,
  text: string | LanguageModelChatRequestMessage,
  token: CancellationToken,
) => number | Promise<number>;

// 分词器定义
type TokenizerDef = {
  label: string;
  description?: string;
  provideTokenCount: ProvideTokenCountFn;
};

// 模型配置中的相关字段
tokenizer?: 'default' | 'conservative' | 'char4' | 'openai' | 'deepseek';
tokenCountMultiplier?: number;  // default 1.0

// 消息提取结果
type TokenizedInput = {
  textContent: string;    // 纯文本拼接
  extraTokens: number;    // 非文本折算 token
};
```
