//! # 额度监控模块类型定义
//!
//! 与前端 `src/modules/balance/types.ts` 对齐。
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 额度监控方法枚举
///
/// 对应 `docs/database.md` §5.10，多态联合类型的判别字段。
/// 新增方法时需同步前端 `BalanceMethod` 与 `BalanceConfig`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BalanceMethod {
    /// 不监控
    None,
    /// Moonshot AI（Kimi 国内站）
    MoonshotAi,
    /// Kimi Code（Kimi 国际站）
    KimiCode,
    /// New API（OneAPI 系分支，需 userId 与 systemToken）
    Newapi,
    /// DeepSeek
    Deepseek,
    /// OpenRouter
    Openrouter,
    /// 硅基流动
    Siliconflow,
    /// AIHubMix
    Aihubmix,
    /// Claude Relay Service
    ClaudeRelayService,
    /// Google Antigravity
    Antigravity,
    /// Gemini CLI
    GeminiCli,
    /// OpenAI Codex
    Codex,
    /// 合成数据（测试用）
    Synthetic,
    /// MiniMax
    Minimax,
}

impl BalanceMethod {
    /// 从字符串解析为 BalanceMethod；未知值返回 None
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "moonshot-ai" => Some(Self::MoonshotAi),
            "kimi-code" => Some(Self::KimiCode),
            "newapi" => Some(Self::Newapi),
            "deepseek" => Some(Self::Deepseek),
            "openrouter" => Some(Self::Openrouter),
            "siliconflow" => Some(Self::Siliconflow),
            "aihubmix" => Some(Self::Aihubmix),
            "claude-relay-service" => Some(Self::ClaudeRelayService),
            "antigravity" => Some(Self::Antigravity),
            "gemini-cli" => Some(Self::GeminiCli),
            "codex" => Some(Self::Codex),
            "synthetic" => Some(Self::Synthetic),
            "minimax" => Some(Self::Minimax),
            _ => None,
        }
    }

    /// 转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::MoonshotAi => "moonshot-ai",
            Self::KimiCode => "kimi-code",
            Self::Newapi => "newapi",
            Self::Deepseek => "deepseek",
            Self::Openrouter => "openrouter",
            Self::Siliconflow => "siliconflow",
            Self::Aihubmix => "aihubmix",
            Self::ClaudeRelayService => "claude-relay-service",
            Self::Antigravity => "antigravity",
            Self::GeminiCli => "gemini-cli",
            Self::Codex => "codex",
            Self::Synthetic => "synthetic",
            Self::Minimax => "minimax",
        }
    }
}

/// New API 额度转换配置
///
/// 用于将原始 quota 数值转换为标准额度值
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewApiQuotaTransform {
    /// 主额度字段名，默认 `quota`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota_field: Option<String>,
    /// 额外累加额度字段名列表
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_quota_fields: Vec<String>,
    /// 原始额度转换除数，默认 500000
    #[serde(skip_serializing_if = "Option::is_none")]
    pub divisor: Option<f64>,
    /// 除法后乘数，默认 1
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f64>,
}

/// New API 方法的额外配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewApiConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// 系统 token，明文或 `$SECRET:{snowflake_id}$` 引用（由调用方解析）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota_transform: Option<NewApiQuotaTransform>,
}

/// Claude Relay Service 方法的额外配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeRelayServiceConfig {
    /// 自定义 apiStats API 地址
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

/// 额度监控配置
///
/// 由 `method` 字段区分不同供应商的查询参数。
/// 对应前端 `BalanceConfig` 多态联合类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", rename_all = "kebab-case")]
pub enum BalanceConfig {
    None,
    MoonshotAi,
    KimiCode,
    Newapi(NewApiConfig),
    Deepseek,
    Openrouter,
    Siliconflow,
    Aihubmix,
    ClaudeRelayService(ClaudeRelayServiceConfig),
    Antigravity,
    GeminiCli,
    Codex,
    Synthetic,
    Minimax,
}

impl BalanceConfig {
    /// 获取方法枚举
    pub fn method(&self) -> BalanceMethod {
        match self {
            Self::None => BalanceMethod::None,
            Self::MoonshotAi => BalanceMethod::MoonshotAi,
            Self::KimiCode => BalanceMethod::KimiCode,
            Self::Newapi(_) => BalanceMethod::Newapi,
            Self::Deepseek => BalanceMethod::Deepseek,
            Self::Openrouter => BalanceMethod::Openrouter,
            Self::Siliconflow => BalanceMethod::Siliconflow,
            Self::Aihubmix => BalanceMethod::Aihubmix,
            Self::ClaudeRelayService(_) => BalanceMethod::ClaudeRelayService,
            Self::Antigravity => BalanceMethod::Antigravity,
            Self::GeminiCli => BalanceMethod::GeminiCli,
            Self::Codex => BalanceMethod::Codex,
            Self::Synthetic => BalanceMethod::Synthetic,
            Self::Minimax => BalanceMethod::Minimax,
        }
    }
}

/// 额度指标时间范围
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalancePeriod {
    /// 当前周期
    #[serde(rename = "current")]
    Current,
    /// 自然月
    #[serde(rename = "month")]
    Month,
    /// 自然日
    #[serde(rename = "day")]
    Day,
    /// 周
    #[serde(rename = "week")]
    Week,
    /// 累计
    #[serde(rename = "total")]
    Total,
}

/// 额度指标类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceMetricType {
    #[serde(rename = "amount")]
    Amount,
    #[serde(rename = "integer")]
    Integer,
    #[serde(rename = "token")]
    Token,
    #[serde(rename = "percent")]
    Percent,
    #[serde(rename = "time")]
    Time,
    #[serde(rename = "status")]
    Status,
}

/// 额度指标方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceDirection {
    #[serde(rename = "remaining")]
    Remaining,
    #[serde(rename = "used")]
    Used,
    #[serde(rename = "limit")]
    Limit,
}

/// 状态类指标值
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceStatusValue {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "unlimited")]
    Unlimited,
    #[serde(rename = "exhausted")]
    Exhausted,
    #[serde(rename = "error")]
    Error,
    #[serde(rename = "unavailable")]
    Unavailable,
}

/// 额度指标公共基座字段
///
/// 所有 `BalanceMetric` 子类型都包含这些字段。
/// 使用扁平化结构 + `metric_type` 判别字段，对应前端联合类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceMetric {
    /// 指标唯一标识，如 `balance` / `tokens` / `expires` / `status`
    pub id: String,
    /// 指标类型
    #[serde(rename = "type")]
    pub metric_type: BalanceMetricType,
    /// 时间范围
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<BalancePeriod>,
    /// 自定义周期标签（覆盖 period）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_label: Option<String>,
    /// 作用域，如 `account` / `model`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// 是否主指标，UI 中高亮显示
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<bool>,
    /// UI 展示标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    // ===== 各类型特有字段（按 metric_type 选择性使用）=====

    /// 方向（amount / integer 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<BalanceDirection>,
    /// 数值（amount / integer / percent 类型）
    /// 使用字符串以避免大数精度丢失
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// 货币符号（amount 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency_symbol: Option<String>,
    /// Token 用量（token 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub used: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining: Option<serde_json::Value>,
    /// 基准（percent 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basis: Option<BalanceDirection>,
    /// 时间类型（time 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// 毫秒时间戳（time 类型，便于排序）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_ms: Option<i64>,
    /// 状态值（status 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_value: Option<BalanceStatusValue>,
    /// 状态描述消息（status 类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl BalanceMetric {
    /// 构造金额类指标
    pub fn amount(
        id: impl Into<String>,
        direction: BalanceDirection,
        value: serde_json::Value,
        currency_symbol: Option<&str>,
    ) -> Self {
        Self {
            id: id.into(),
            metric_type: BalanceMetricType::Amount,
            direction: Some(direction),
            value: Some(value),
            currency_symbol: currency_symbol.map(|s| s.to_string()),
            ..Default::default()
        }
    }

    /// 设置周期
    pub fn with_period(mut self, period: BalancePeriod) -> Self {
        self.period = Some(period);
        self
    }

    /// 设置是否主指标
    pub fn with_primary(mut self, primary: bool) -> Self {
        self.primary = Some(primary);
        self
    }

    /// 设置作用域
    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    /// 设置展示标签
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// 构造状态类指标
    pub fn status(id: impl Into<String>, value: BalanceStatusValue, message: Option<&str>) -> Self {
        let value_str = match value {
            BalanceStatusValue::Ok => "ok",
            BalanceStatusValue::Unlimited => "unlimited",
            BalanceStatusValue::Exhausted => "exhausted",
            BalanceStatusValue::Error => "error",
            BalanceStatusValue::Unavailable => "unavailable",
        };
        Self {
            id: id.into(),
            metric_type: BalanceMetricType::Status,
            // 同时写入 value 与 status_value，兼容前端联合类型与历史字段
            value: Some(serde_json::Value::String(value_str.to_string())),
            status_value: Some(value),
            message: message.map(|s| s.to_string()),
            ..Default::default()
        }
    }

    /// 构造整数类指标
    pub fn integer(
        id: impl Into<String>,
        direction: BalanceDirection,
        value: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            metric_type: BalanceMetricType::Integer,
            direction: Some(direction),
            value: Some(value),
            ..Default::default()
        }
    }

    /// 构造 Token 用量指标
    pub fn token(
        id: impl Into<String>,
        used: Option<serde_json::Value>,
        limit: Option<serde_json::Value>,
        remaining: Option<serde_json::Value>,
    ) -> Self {
        Self {
            id: id.into(),
            metric_type: BalanceMetricType::Token,
            used,
            limit,
            remaining,
            ..Default::default()
        }
    }

    /// 构造百分比指标
    pub fn percent(
        id: impl Into<String>,
        value: f64,
        basis: Option<BalanceDirection>,
    ) -> Self {
        Self {
            id: id.into(),
            metric_type: BalanceMetricType::Percent,
            value: Some(serde_json::json!(value)),
            basis,
            ..Default::default()
        }
    }

    /// 构造时间类指标
    pub fn time(
        id: impl Into<String>,
        kind: impl Into<String>,
        value: impl Into<String>,
        timestamp_ms: Option<i64>,
    ) -> Self {
        Self {
            id: id.into(),
            metric_type: BalanceMetricType::Time,
            kind: Some(kind.into()),
            value: Some(serde_json::Value::String(value.into())),
            timestamp_ms,
            ..Default::default()
        }
    }

    /// 设置自定义周期标签
    pub fn with_period_label(mut self, label: impl Into<String>) -> Self {
        self.period_label = Some(label.into());
        self
    }
}

impl Default for BalanceMetric {
    fn default() -> Self {
        Self {
            id: String::new(),
            metric_type: BalanceMetricType::Status,
            period: None,
            period_label: None,
            scope: None,
            primary: None,
            label: None,
            direction: None,
            value: None,
            currency_symbol: None,
            used: None,
            limit: None,
            remaining: None,
            basis: None,
            kind: None,
            timestamp_ms: None,
            status_value: None,
            message: None,
        }
    }
}

/// 额度快照
///
/// 由 `BalanceService::query_balance()` 调用供应商 API 后生成，
/// 由调用方写入 `cli_providers.balance_json`。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceSnapshot {
    /// 快照更新时间戳（毫秒）
    pub updated_at: i64,
    /// 额度指标数组
    pub items: Vec<BalanceMetric>,
}

/// 额度刷新结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceRefreshResult {
    pub cli_provider_id: String,
    pub snapshot: BalanceSnapshot,
    /// 刷新过程中发生的错误（非致命，部分指标可能仍可获取）
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// 额度警告事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceWarning {
    pub cli_provider_id: String,
    /// 触发警告的指标 ID
    pub metric_id: String,
    /// 警告消息
    pub message: String,
    /// 触发时间戳（毫秒）
    pub triggered_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_method_roundtrip() {
        for method in [
            BalanceMethod::None,
            BalanceMethod::MoonshotAi,
            BalanceMethod::KimiCode,
            BalanceMethod::Newapi,
            BalanceMethod::Deepseek,
            BalanceMethod::Openrouter,
            BalanceMethod::Siliconflow,
            BalanceMethod::Aihubmix,
            BalanceMethod::ClaudeRelayService,
            BalanceMethod::Antigravity,
            BalanceMethod::GeminiCli,
            BalanceMethod::Codex,
            BalanceMethod::Synthetic,
            BalanceMethod::Minimax,
        ] {
            let s = method.as_str();
            assert_eq!(BalanceMethod::from_str(s), Some(method));
        }
        assert_eq!(BalanceMethod::from_str("unknown"), None);
    }

    #[test]
    fn test_balance_config_serde() {
        // none
        let config = BalanceConfig::None;
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(json, "{\"method\":\"none\"}");

        // newapi 带参数
        let config = BalanceConfig::Newapi(NewApiConfig {
            user_id: Some("123".to_string()),
            system_token: Some("$SECRET:abc$".to_string()),
            quota_transform: None,
        });
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"method\":\"newapi\""));
        assert!(json.contains("\"userId\":\"123\""));
    }

    #[test]
    fn test_balance_metric_amount() {
        let metric = BalanceMetric::amount(
            "balance",
            BalanceDirection::Remaining,
            serde_json::json!(99.5),
            Some("$"),
        );
        assert_eq!(metric.metric_type, BalanceMetricType::Amount);
        assert_eq!(metric.direction, Some(BalanceDirection::Remaining));
        assert_eq!(metric.currency_symbol.as_deref(), Some("$"));
    }
}
