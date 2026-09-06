//! # imagebed 模块（社区图床上传）
//!
//! 内置「应用内浏览器」加载图床站点，并在页面 document-start 注入篡改猴式增强脚本
//! （去广告 / 粘贴上传 / 复制按钮）；上传完成后脚本把外链写入 `document.title`，
//! 由本模块的轮询线程解析桥接并 `emit("imagebed:link-ready")` 回传社区编辑器。

pub mod commands;
pub mod providers;
pub mod service;
pub mod types;

pub use service::ImagebedHandle;