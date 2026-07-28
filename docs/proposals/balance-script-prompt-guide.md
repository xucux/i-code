# 生成 i-code 额度监控脚本的提示词指南

> 本文档供**其他 AI Agent 工具**使用，用于将任意供应商的额度查询请求（curl / Python / JS 等）转换为 i-code 的 Rhai 额度监控脚本。
>
> 关键约束：i-code 的 Rhai 引擎使用 `::` 调用模块函数（如 `http::get`、`json::parse`），**禁止**使用点号（如 `http.get` 会报错）。脚本最终返回一个包含 `items` 数组的 map。

---

## 1. Rhai 语法速查

Rhai 是一种嵌入 Rust 的脚本语言，语法接近 JavaScript，但有以下关键区别：

| 概念 | Rhai 语法 | 说明 |
|------|-----------|------|
| 注释 | `// 单行` | 不支持 `/* */` 多行注释 |
| 字符串连接 | `"a" + "b"` | 与 JS 相同 |
| 模板字符串 | `` `Hello ${name}` `` | 支持插值，用反引号 |
| 变量 | `let x = 10;` | 可变；`const` 变量由系统注入 |
| 条件 | `if cond { } else if cond { } else { }` | 与 JS 相同 |
| 循环 | `for item in arr { }` | 遍历数组 |
| Map (对象) | `#{ key: value, "k2": v2 }` | 用 `#{}` 而非 `{}` |
| 数组 | `[1, 2, 3]` | 与 JS 相同 |
| 属性访问 | `map["key"]` 或 `map.key` | 点号仅用于 map 属性，**不可用于模块函数** |
| 模块函数调用 | `http::get(url)` | 用 `::` 而非 `.` |
| 空值判断 | `if val == () { }` | 空值写作 `()`，不是 `null` / `undefined` |
| 错误抛出 | `error("msg")` | 调用 `error()` 函数立即终止脚本 |
| 比较 | `==` / `!=` | 严格比较，不支持 `===` |
| 布尔 | `true` / `false` | 小写 |
| 函数 | `fn add(x, y) { x + y }` | 支持自定义函数 |

---

## 2. 系统变量

脚本运行时自动注入以下变量，**只读**（不可赋值）：

### 2.1 直接变量

| 变量名 | 类型 | 说明 | 示例 |
|--------|------|------|------|
| `api_key` | String | 解密后的 API Key / Token | `"sk-xxx..."` |
| `now_ms` | Integer | 当前 Unix 毫秒时间戳 | `1712345678000` |

### 2.2 `provider` 对象

| 字段 | 类型 | 说明 |
|------|------|------|
| `provider.id` | String | 供应商数据库 ID |
| `provider.slug` | String | 供应商标识符 |
| `provider.name` | String | 供应商显示名称 |
| `provider.base_url` | String | 供应商基础 URL |
| `provider.provider_type` | String | 供应商类型 |
| `provider.is_enabled` | Boolean | 是否启用 |

### 2.3 `auth` 对象

从 `auth_json` 解析出的认证信息（部分字段可选）：

| 字段 | 类型 | 说明 |
|------|------|------|
| `auth.method` | String | 认证方法（如 `"api_key"`、`"bearer"`、`"none"`） |
| `auth.project_id` | String (可选) | 项目 ID |
| `auth.managed_project_id` | String (可选) | 托管项目 ID |
| `auth.account_id` | String (可选) | 账户 ID |

### 2.4 `template` 对象

| 字段 | 类型 | 说明 |
|------|------|------|
| `template.id` | String | 模板 ID |
| `template.name` | String | 模板名称 |
| `template.kind` | String | 模板类型（固定 `"balance"`） |

### 2.5 `variables` 对象（模板变量）

> **v0.0.7+ 已实现**

供应商「扩展模板变量」中配置的 key-value 对，解密后注入。常用于传递 Cookie、额外 Token 等不能通过 `api_key` 单一字段表达的多凭证场景。

| 访问方式 | 示例 | 说明 |
|----------|------|------|
| `variables["key"]` | `variables["cookie"]` | 通过 `variables` map 访问 |
| `key`（顶层常量） | `cookie` | 非保留名变量同时扁平注入为顶层常量 |

**保留名列表**（以下名称不能用作变量 key，会跳过扁平注入，仅可通过 `variables["key"]` 访问）：

`api_key`、`now_ms`、`provider`、`auth`、`template`、`variables`、`pi`、`e`

**使用示例**（Cookie 鉴权场景）：

```rhai
// 通过 variables map 访问（推荐，清晰无歧义）
let headers = #{
    "Cookie": variables["cookie"],
    "Accept": "application/json"
};

// 扁平注入的顶层变量也等价可用（但需注意不与保留名冲突）
let c = cookie;  // 与 variables["cookie"] 相同
```

### 2.6 使用示例

```rhai
let base = provider.base_url;
let key = api_key;
let url = base + "/v1/user/balance";

log::info(`请求 URL: ${url}`);
log::info(`供应商: ${provider.name}`);
```

---

## 3. 系统函数

所有模块函数**必须**用 `::` 调用（如 `http::get`），不要用点号（如 `http.get` 是错误的）。

### 3.1 HTTP 请求

| 函数 | 签名 | 说明 |
|------|------|------|
| `http::get(url)` | `(String) → Map` | GET 请求，无自定义头 |
| `http::get(url, headers)` | `(String, Map) → Map` | GET 请求，带自定义头 |
| `http::post(url, body)` | `(String, String) → Map` | POST 请求，无自定义头 |
| `http::post(url, body, headers)` | `(String, String, Map) → Map` | POST 请求，带自定义头 |
| `http::request(method, url)` | `(String, String) → Map` | 任意 HTTP 方法（**仅 2 参**，不带 body/headers，见下扁平别名） |
| `http::get_json(url)` | `(String) → Dynamic` | GET 后自动解析 JSON，非 2xx 抛错 |

**扁平别名**（等价的函数名，可用作普通函数调用；支持更多参数重载）：

| 别名 | 等价模块调用 |
|------|-------------|
| `http_get(url)` | `http::get(url)` |
| `http_get(url, headers)` | `http::get(url, headers)` |
| `http_post(url, body)` | `http::post(url, body)` |
| `http_post(url, body, headers)` | `http::post(url, body, headers)` |
| `http_request(method, url, body, headers)` | 比 `http::request` 多 body/headers 参数 |
| `http_get_json(url)` | `http::get_json(url)` |
| `http_get_json(url, headers)` | `http::get_json` + 自定义头（**模块版本无此重载，仅扁平版本有**） |

**HTTP 返回值**：所有 `http::get` / `http::post` / `http::request` 返回一个 Map：

```rhai
#{
    status: 200,       // Integer: HTTP 状态码
    body: "...",       // String: 响应体
    headers: #{ ... }  // Map: 响应头
}
```

**安全约束**：
- 仅允许请求 `provider.base_url` 所在 host 和配置的额外 `allowed_hosts`
- 超时默认 10 秒，最大 30 秒
- 响应体上限 2 MB
- URL 仅支持 `http://` 和 `https://`
- 脚本最大操作数 100,000（防止死循环）

### 3.2 JSON 处理

| 函数 | 签名 | 说明 |
|------|------|------|
| `json::parse(text)` | `(String) → Dynamic` | 解析 JSON 字符串为 Rhai 值 |
| `json::stringify(value)` | `(Dynamic) → String` | 序列化为 JSON 字符串 |
| `json::stringify_pretty(value)` | `(Dynamic) → String` | 格式化 JSON 字符串 |

**扁平别名**：

| 别名 | 等价模块调用 |
|------|-------------|
| `json_parse(text)` | `json::parse(text)` |
| `json_stringify(value)` | `json::stringify(value)` |
| `json_stringify_pretty(value)` | `json::stringify_pretty(value)` |

### 3.3 日志

| 函数 | 签名 | 说明 |
|------|------|------|
| `log::info(msg)` | `(String) → ()` | 信息日志 |
| `log::warn(msg)` | `(String) → ()` | 警告日志 |
| `log::error(msg)` | `(String) → ()` | 错误日志 |

**扁平别名**：

| 别名 | 等价模块调用 |
|------|-------------|
| `log_info(msg)` | `log::info(msg)` |
| `log_warn(msg)` | `log::warn(msg)` |
| `log_error(msg)` | `log::error(msg)` |

**自动脱敏**：日志内容中若包含 `api_key` 明文，会自动替换为 `***`。

### 3.4 字符串操作

| 函数 | 签名 | 说明 |
|------|------|------|
| `str::contains(text, sub)` | `(String, String) → Bool` | 是否包含子串 |
| `str::replace(text, from, to)` | `(String, String, String) → String` | 替换 |
| `str::starts_with(text, prefix)` | `(String, String) → Bool` | 是否以 .. 开头 |
| `str::ends_with(text, suffix)` | `(String, String) → Bool` | 是否以 .. 结尾 |
| `str::trim(text)` | `(String) → String` | 去首尾空白 |
| `str::to_lower(text)` | `(String) → String` | 转小写 |
| `str::to_upper(text)` | `(String) → String` | 转大写 |
| `str::len(text)` | `(String) → Integer` | 字符长度 |
| `str::sub_string(text, start, end)` | `(String, Integer, Integer) → String` | 子串（按字符索引） |
| `str::to_float(text)` | `(String) → Float` | 字符串转浮点，失败抛错（如 `"80138.77"` → `80138.77`） |
| `str::to_int(text)` | `(String) → Integer` | 字符串转整数，失败抛错 |

**扁平别名**：`str_contains`、`str_replace`、`str_starts_with`、`str_ends_with`、`str_trim`、`str_to_lower`、`str_to_upper`、`str_len`、`str_sub_string`、`str_to_float`、`str_to_int`。

### 3.5 数学函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `math::abs(x)` | `(f64\|i64) → f64\|i64` | 绝对值 |
| `math::min(a, b)` | `(f64\|i64, f64\|i64) → f64\|i64` | 最小值 |
| `math::max(a, b)` | `(f64\|i64, f64\|i64) → f64\|i64` | 最大值 |
| `math::floor(x)` | `(f64) → f64` | 向下取整 |
| `math::ceil(x)` | `(f64) → f64` | 向上取整 |
| `math::round(x)` | `(f64) → f64` | 四舍五入 |
| `math::sqrt(x)` | `(f64) → f64` | 平方根 |
| `math::pow(base, exp)` | `(f64, f64) → f64` | 幂运算 |

**扁平别名**：`math_abs`、`math_min`、`math_max`、`math_floor`、`math_ceil`、`math_round`、`math_sqrt`、`math_pow`。

### 3.6 其他函数

| 函数 | 签名 | 说明 |
|------|------|------|
| `error(msg)` | `(String) → !` | 立即终止脚本并抛出错误 |
| `url_join(base, path)` | `(String, String) → String` | 拼接 URL（自动处理首尾斜杠） |

---

## 4. 返回值结构

脚本必须返回一个 **Map**，包含 `items` 数组。整体结构：

```rhai
#{
    updatedAt: 1712345678000,  // 可选，毫秒时间戳，不填则自动使用 now_ms
    items: [ /* ... BalanceMetric 对象 ... */ ]
}
```

### 4.1 通用字段（所有指标类型都支持）

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `id` | String | **是** | 指标唯一标识，如 `"balance"`、`"tokens"` |
| `type` | String | **是** | 指标类型：`amount` / `integer` / `token` / `percent` / `time` / `status` |
| `period` | String | 否 | 时间范围：`current` / `month` / `day` / `week` / `total` |
| `periodLabel` | String | 否 | 自定义周期标签，覆盖 `period` |
| `scope` | String | 否 | 作用域，如 `"account"`、`"model"` |
| `primary` | Bool | 否 | 是否主指标，UI 中高亮显示 |
| `label` | String | 否 | UI 展示标签 |

### 4.2 金额 (`type: "amount"`)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `direction` | String | **是** | `"remaining"` / `"used"` / `"limit"` |
| `value` | Number | **是** | 数值（浮点数） |
| `currencySymbol` | String | 否 | 货币符号，如 `"$"`、`"¥"` |
| `period` | String | 否 | 时间范围 |
| `primary` | Bool | 否 | 是否主指标 |

**示例**：
```rhai
#{
    id: "balance",
    type: "amount",
    direction: "remaining",
    value: 99.50,
    currencySymbol: "$",
    primary: true,
    label: "剩余额度",
    period: "current"
}
```

### 4.3 整数 (`type: "integer"`)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `direction` | String | **是** | `"remaining"` / `"used"` / `"limit"` |
| `value` | Number | **是** | 整数值 |

**示例**：
```rhai
#{
    id: "requests_today",
    type: "integer",
    direction: "used",
    value: 42,
    label: "今日请求",
    period: "day"
}
```

### 4.4 Token (`type: "token"`)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `used` | Number | 否* | 已用量 |
| `limit` | Number | 否* | 总量 |
| `remaining` | Number | 否* | 剩余量 |

**至少需要 `used` / `limit` / `remaining` 其中之一。**

**示例**：
```rhai
#{
    id: "tokens",
    type: "token",
    used: 500000,
    limit: 1000000,
    remaining: 500000,
    label: "Token 用量",
    period: "current",
    primary: true
}
```

### 4.5 百分比 (`type: "percent"`)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `value` | Number | **是** | 百分比值 **0–100** |
| `basis` | String | 否 | 基准：`"remaining"` / `"used"` |

**示例**：
```rhai
#{
    id: "usage_pct",
    type: "percent",
    value: 75.5,
    basis: "used",
    label: "已消耗",
    period: "current"
}
```

### 4.6 时间 (`type: "time"`)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `kind` | String | **是** | `"expiresAt"` 或 `"resetAt"` |
| `value` | String | **是** | 时间描述文本，如 `"2025-12-31T23:59:59Z"` |
| `timestampMs` | Integer | 否 | 毫秒时间戳，便于排序 |

**示例**：
```rhai
#{
    id: "expires",
    type: "time",
    kind: "expiresAt",
    value: "2025-12-31T23:59:59Z",
    timestampMs: 1767225599000,
    label: "过期时间",
    period: "current"
}
```

### 4.7 状态 (`type: "status"`)

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `value` | String | **是** | 状态值：`"ok"` / `"unlimited"` / `"exhausted"` / `"error"` / `"unavailable"` |
| `message` | String | 否 | 状态描述消息 |

**示例**：
```rhai
#{
    id: "account_status",
    type: "status",
    value: "ok",
    message: "账户正常，模式=standard"
}
```

---

## 5. 转换工作流

将任意供应商的额度查询 API 转换为 Rhai 脚本的通用步骤：

### 步骤 1：分析原始请求

从 curl / Python / JS 等代码中提取：

| 信息 | 提取方式 | 映射到 |
|------|---------|--------|
| HTTP 方法 | `GET` / `POST` | `http::get` / `http::post` |
| URL | 完整 URL | 用 `provider.base_url` + `url_join` 构造，或直接写固定 URL |
| 请求头 | Authorization, Content-Type 等 | 构造 `headers` Map |
| 请求体 | JSON body | 字符串参数传给 `http::post` |
| 完整 URL 路径 | 从 curl 中提取 | 拼接：`url_join(provider.base_url, "/v1/balance")` |

### 步骤 2：确定鉴权方式

| 原始鉴权方式 | Rhai 脚本写法 |
|-------------|-------------|
| `Authorization: Bearer sk-xxx` | `let headers = #{ "Authorization": "Bearer " + api_key }` |
| `Authorization: Bearer xxx`（api_key 就是 token） | 同上 |
| `x-api-key: xxx` | `let headers = #{ "x-api-key": api_key }` |
| `Cookie: session=xxx`（单 Cookie） | `let headers = #{ "Cookie": api_key }`（api_key 存完整 Cookie 字符串） |
| `Cookie: ...`（多凭证/复杂 Cookie） | 供应商「扩展模板变量」添加 `key="cookie"`，脚本用 `variables["cookie"]` |
| Basic Auth | `let headers = #{ "Authorization": "Basic " + api_key }` |
| 无认证（公开接口） | `let headers = #{}` 或省略 |

### 步骤 3：解析响应

从 JSON 响应中提取额度字段：

```rhai
// 解析 JSON
let json = json::parse(resp.body);

// 按路径提取字段
let remaining = json["data"]["balance"]["remaining"];
let used = json["data"]["balance"]["used"];
let total = json["data"]["balance"]["total"];
```

### 步骤 4：构造返回 Map

根据提取的字段构造 `items` 数组，选择合适的指标类型。

### 步骤 5：添加错误处理

```rhai
// 检查 HTTP 状态码
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

// 检查 API 业务错误
if json["code"] != 0 {
    error(`API error: ${json["message"]}`);
}

// 检查数据有效性
if json["isValid"] == false {
    error("接口返回数据无效");
}
```

### 步骤 6：添加日志（可选，推荐试运行用）

```rhai
log::info(`余额: ${remaining}`);
log::info(`已用: ${used}`);
log::warn(`注意: 可用额度低于 10%`);
```

---

## 6. 完整示例

### 示例 1：OpenAI 兼容余额查询

```rhai
// OpenAI 兼容 /v1/user/balance 风格
// api_key 自动注入（Bearer token）

let base = provider.base_url;
// 去除末尾斜杠
if base.ends_with("/") {
    base = str::sub_string(base, 0, str::len(base) - 1);
}
let url = base + "/v1/user/balance";

let headers = #{
    "Authorization": "Bearer " + api_key,
    "Accept": "application/json"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let data = json::parse(resp.body);
let total = data["balance"];

#{
    items: [
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: total,
            currencySymbol: "$",
            primary: true,
            label: "余额",
            period: "current"
        }
    ]
}
```

### 示例 2：公益 Grok 额度监控

```rhai
// 公益 Grok 额度监控
// 鉴权：x-api-key
// 返回：quota 金额 + 今日/累计 token + 账户状态

let base = provider.base_url;
if base == "" {
    base = "https://sub.your-service.com";
}
if base.ends_with("/") {
    base = str::sub_string(base, 0, str::len(base) - 1);
}
let url = base + "/v1/usage";

let headers = #{
    "x-api-key": api_key,
    "Accept": "application/json"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let data = json::parse(resp.body);

// 账户有效性检查
if data["isValid"] == false {
    error("额度接口返回 isValid=false");
}

// 提取额度数据
let q = data["quota"];
let remaining = q["remaining"];
let used = q["used"];
let limit = q["limit"];
let pct = 0.0;
if limit != () && limit != 0 {
    pct = remaining * 100.0 / limit;
}

// Token 数据
let usage = data["usage"];
let today = usage["today"];
let total = usage["total"];

// 状态
let st = data["status"];
let status_value = "ok";
if st == "active" {
    status_value = "ok";
} else if st == "exhausted" {
    status_value = "exhausted";
} else if st == () {
    status_value = "ok";
} else {
    status_value = "unavailable";
}

#{
    items: [
        // 主指标：剩余额度
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: remaining,
            currencySymbol: "$",
            primary: true,
            label: "剩余额度",
            period: "current"
        },
        // 已用金额
        #{
            id: "used",
            type: "amount",
            direction: "used",
            value: used,
            currencySymbol: "$",
            label: "已用金额",
            period: "current"
        },
        // 上限
        #{
            id: "limit",
            type: "amount",
            direction: "limit",
            value: limit,
            currencySymbol: "$",
            label: "额度上限",
            period: "current"
        },
        // 剩余百分比
        #{
            id: "remaining_pct",
            type: "percent",
            value: pct,
            basis: "remaining",
            label: "剩余%",
            period: "current"
        },
        // 今日 Token
        #{
            id: "tokens_today",
            type: "token",
            used: today["total_tokens"],
            label: "今日 Token",
            period: "day"
        },
        // 累计 Token
        #{
            id: "tokens_total",
            type: "token",
            used: total["total_tokens"],
            label: "累计 Token",
            period: "total"
        },
        // 账户状态
        #{
            id: "status",
            type: "status",
            value: status_value,
            message: `mode=${data["mode"]}, status=${st}`
        }
    ]
}
```

### 示例 3：Cookie 鉴权（如小米 MiMo）

```rhai
// Cookie 鉴权示例
// api_key 字段填入 Cookie 字符串

let url = url_join("https://platform.example.com", "/api/v1/balance");

let headers = #{
    "Cookie": api_key,
    "Accept": "application/json",
    "User-Agent": "i-code/1.0"
};

let resp = http::get(url, headers);
if resp.status < 200 || resp.status >= 300 {
    error(`HTTP ${resp.status}: ${resp.body}`);
}

let json = json::parse(resp.body);
if json["code"] != 0 {
    error(`API error: ${json["message"]}`);
}

let data = json["data"];

let items = [];
items.push(#{
    id: "balance",
    type: "amount",
    direction: "remaining",
    value: data["balance"],
    currencySymbol: "¥",
    primary: true,
    label: "总余额",
    period: "current"
});

#{
    items: items
}
```

### 示例 4：返回骨架（空模板，便于改写）

```rhai
#{
    items: [
        #{
            id: "balance",
            type: "amount",
            direction: "remaining",
            value: 0,
            currencySymbol: "$",
            primary: true,
            label: "余额",
            period: "current"
        }
    ]
}
```

---

## 7. 常见问题

### 7.1 空值判断

Rhai 中空值（null/undefined）写作 `()`：

```rhai
// 正确：判断字段是否存在
if val == () {
    val = 0;
}

// 错误：不要用 null 或 undefined
```

### 7.2 字符串拼接

```rhai
// 正确
let url = base + "/v1/balance";
let msg = `余额: ${amount}`;

// 错误：不要用模板字符串的 ${} 之外的方式
```

### 7.3 Map 写法

```rhai
// 正确
let headers = #{
    "Authorization": "Bearer " + api_key,
    "Accept": "application/json"
};

// 访问属性
let auth = headers["Authorization"];  // 或 headers.Authorization

// 错误：不要用 {} 代替 #{}
```

### 7.4 数组操作

```rhai
// 支持 push
let items = [];
items.push(#{
    id: "balance",
    type: "amount",
    value: 100
});

// 遍历
for item in items {
    log::info(`item: ${item.id}`);
}
```

### 7.5 判断相等

```rhai
// 正确
if resp.status != 200 { }
if data["code"] == 0 { }

// 不支持 === / !==
```

### 7.6 字符串方法

Rhai 字符串本身没有 `.contains()` 等方法，必须用模块函数：

```rhai
// 正确
if str::contains(base, "/v1") { }

// 错误：不要写 base.contains("/v1")
```

---

## 8. 典型转换场景速查表

| 原始 API 风格 | Rhai 转换要点 |
|---------------|-------------|
| `curl -H "Authorization: Bearer $KEY" $URL` | `headers = #{ "Authorization": "Bearer " + api_key }` |
| `curl -H "x-api-key: $KEY" $URL` | `headers = #{ "x-api-key": api_key }` |
| `curl -b "session=$COOKIE" $URL` | `headers = #{ "Cookie": api_key }`（api_key 存完整 Cookie） |
| `curl -b "复杂多 Cookie"` | 供应商「扩展模板变量」存 Cookie，脚本用 `variables["cookie"]` |
| `POST` + JSON body | `http::post(url, json::stringify(body_map), headers)` |
| Python `requests.get(url, headers=headers)` | `http::get(url, headers)` |
| 响应嵌套 `data.balance.remaining` | `json["data"]["balance"]["remaining"]` |
| 响应是数组 `[{...}]` | `let arr = json::parse(resp.body); let first = arr[0];` |
| 需要拼接 base URL | `url_join(provider.base_url, "/v1/balance")` |
| 需要额外凭证（Cookie/Token 等） | 在供应商「扩展模板变量」中添加，脚本用 `variables["key"]` 读取 |

---

## 9. 技术限制

| 限制项 | 值 |
|--------|-----|
| 脚本最大体积 | 64 KiB |
| 最大操作数 | 100,000 |
| HTTP 超时 | 默认 10s，最大 30s |
| 响应体上限 | 2 MB |
| 表达式嵌套深度 | 64 层 |
| 仅支持 `http://` / `https://` | 否 |
| 文件 IO | **禁止** |
| 脚本引擎 | Rhai（纯 Rust） |
| 调用记法 | 模块函数用 `::`，**禁止** 点号 |

---

## 10. 编写步骤总结

1. **确定 API 端点** → 用 `url_join(provider.base_url, "/path")` 或硬编码
2. **确定鉴权方式** → 用 `api_key` 变量 + headers Map
3. **发送请求** → `http::get` / `http::post` / `http::get_json`
4. **检查响应** → 状态码 + 业务错误码
5. **解析 JSON** → `json::parse(resp.body)`
6. **提取字段** → 按路径访问 map
7. **构造 items** → 选择合适的 `type` 和字段
8. **返回 Map** → `#{ items: [...] }`