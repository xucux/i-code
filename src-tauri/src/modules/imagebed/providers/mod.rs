//! 内置图床 provider 资源（含注入脚本定义与查找）

pub mod ooxx;
pub mod remit;

pub use ooxx::OOXX_PROVIDER;
pub use remit::REMIT_PROVIDER;

use super::types::ImagebedProviderSpec;

/// 全部内置图床 provider（后续迭代可扩展为运行时用户自定义，归档 DB）
pub const ALL_PROVIDERS: &[ImagebedProviderSpec] = &[REMIT_PROVIDER, OOXX_PROVIDER];

/// 按 id 查找内置图床 provider
pub fn provider_by_id(id: &str) -> Option<&'static ImagebedProviderSpec> {
    ALL_PROVIDERS.iter().find(|p| p.id == id)
}