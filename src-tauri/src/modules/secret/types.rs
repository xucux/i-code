//! # 敏感凭据模块类型定义
//!
//! 与前端 `src/modules/secret/types.ts` 对齐。
//! 序列化字段使用 camelCase，确保前后端 JSON 字段名一致。

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// 敏感凭据类型
///
/// 对应 `secrets.kind` 列，用于区分不同用途的密钥
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecretKind {
    /// API Key（最常用）
    ApiKey,
    /// OAuth 2.0 Token
    OauthToken,
    /// 代理认证凭据
    ProxyAuth,
    /// 网关对外 API Key
    GatewayKey,
    /// WebDAV 密码
    ///
    /// 显式指定 serde 名称，避免 `WebDavPassword` 被 kebab-case 拆分为 `web-dav-password`，
    /// 保持与数据库 `as_str()` 及前端 `SecretKind` 字符串一致。
    #[serde(rename = "webdav-password")]
    WebDavPassword,
    /// 供应商扩展模板变量
    ScriptVariable,
}

impl SecretKind {
    /// 从字符串解析为 SecretKind；未知值返回 None
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "api-key" => Some(Self::ApiKey),
            "oauth-token" => Some(Self::OauthToken),
            "proxy-auth" => Some(Self::ProxyAuth),
            "gateway-key" => Some(Self::GatewayKey),
            "webdav-password" => Some(Self::WebDavPassword),
            "script-variable" => Some(Self::ScriptVariable),
            _ => None,
        }
    }

    /// 转换为数据库存储的字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApiKey => "api-key",
            Self::OauthToken => "oauth-token",
            Self::ProxyAuth => "proxy-auth",
            Self::GatewayKey => "gateway-key",
            Self::WebDavPassword => "webdav-password",
            Self::ScriptVariable => "script-variable",
        }
    }
}

/// 敏感数据存储模式
///
/// 对应 `app_settings.store_secrets_in_keychain` 字段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecretStorageMode {
    /// 使用系统密钥链（macOS Keychain / Windows Credential Manager / libsecret）
    Keychain,
    /// 本地 AES-GCM 加密，加密密钥由系统密钥链保护
    Encrypted,
}

/// 敏感凭据掩码视图（前端展示用）
///
/// 后端返回 Secret 列表时使用此类型，**永远不暴露明文或密文**。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretMask {
    pub id: String,
    pub kind: SecretKind,
    pub label: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// 保存 Secret 的输入参数
///
/// 前端通过 `secret-input.tsx` 收集明文后传给后端 Command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSecretInput {
    pub kind: SecretKind,
    /// 明文值（仅在后端短暂存在，加密后立即丢弃）
    pub plaintext: String,
    pub label: Option<String>,
}

/// Secret 引用解析结果
///
/// 后端扫描配置对象中所有 `$SECRET:{snowflake_id}$` 字符串后返回
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretReferenceScanResult {
    /// 找到的所有 Secret 引用 ID
    pub secret_ids: Vec<String>,
    /// 不存在或无法读取的 Secret ID
    pub missing: Vec<String>,
}

/// `$SECRET:{snowflake_id}$` 引用前缀
pub const SECRET_PREFIX: &str = "$SECRET:";

/// `$SECRET:{snowflake_id}$` 引用后缀
pub const SECRET_SUFFIX: &str = "$";

/// 构造 Secret 引用字符串
///
/// 在配置中作为占位符存储，运行时由后端 `secret.service.resolve()` 替换为明文
pub fn build_secret_ref(id: &str) -> String {
    format!("{SECRET_PREFIX}{id}{SECRET_SUFFIX}")
}

/// 从字符串中解析 Secret 引用
///
/// 仅匹配完整的 `$SECRET:{snowflake_id}$` 格式，部分匹配返回 None
pub fn parse_secret_ref(s: &str) -> Option<&str> {
    let trimmed = s.strip_prefix(SECRET_PREFIX)?;
    let id = trimmed.strip_suffix(SECRET_SUFFIX)?;
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_and_parse_secret_ref() {
        let id = "1234567890123456789";
        let r = build_secret_ref(id);
        assert_eq!(r, "$SECRET:1234567890123456789$");
        assert_eq!(parse_secret_ref(&r), Some(id));
    }

    #[test]
    fn test_parse_invalid_ref() {
        assert_eq!(parse_secret_ref(""), None);
        assert_eq!(parse_secret_ref("$SECRET:uuid"), None);
        assert_eq!(parse_secret_ref("uuid$"), None);
        assert_eq!(parse_secret_ref("$SECRET:$"), None);
        assert_eq!(parse_secret_ref("not a ref"), None);
    }

    #[test]
    fn test_secret_kind_roundtrip() {
        for kind in [
            SecretKind::ApiKey,
            SecretKind::OauthToken,
            SecretKind::ProxyAuth,
            SecretKind::GatewayKey,
            SecretKind::WebDavPassword,
        ] {
            let s = kind.as_str();
            assert_eq!(SecretKind::from_str(s), Some(kind));
        }
        assert_eq!(SecretKind::from_str("unknown"), None);
    }
}
