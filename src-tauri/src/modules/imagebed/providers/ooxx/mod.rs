//! 内置图床：ooxx.ooo（https://ooxx.ooo/upload）
//!
//! 站点能力：服务端渲染，隐藏 `<input id="import-file-select" type="file">` +
//! 拖拽区上传，表单带 `_xsrf`；上传成功后自动在 `#tab-markdown` 生成 `![](url)`。
//! 注入脚本（user.js）只做「剪贴板粘贴上传 + 结果桥接回传」，不注入去广告/复制按钮。

use crate::modules::imagebed::types::ImagebedProviderSpec;

/// ooxx.ooo 图床 provider 规格（名称、地址、注入脚本）
pub const OOXX_PROVIDER: ImagebedProviderSpec = ImagebedProviderSpec {
    id: "ooxx",
    name: "ooxx.ooo",
    url: "https://ooxx.ooo/upload",
    inject_script: include_str!("user.js"),
};