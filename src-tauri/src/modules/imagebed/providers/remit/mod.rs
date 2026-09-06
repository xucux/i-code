//! 内置图床：img.remit.ee
//!
//! `user.js` 为注入脚本（篡改猴脚本的初始化脚本适配版，见文件头注释），
//! 通过 `include_str!` 编译进二进制，避免运行时读文件。

use crate::modules::imagebed::types::ImagebedProviderSpec;

/// img.remit.ee 图床 provider 规格（名称、地址、注入脚本）
pub const REMIT_PROVIDER: ImagebedProviderSpec = ImagebedProviderSpec {
    id: "remit",
    name: "img.remit.ee",
    url: "https://img.remit.ee/",
    inject_script: include_str!("user.js"),
};