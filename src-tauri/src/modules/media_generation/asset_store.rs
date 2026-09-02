//! # 媒体产物存储（asset store）
//!
//! 生成成功后**立即**将供应商返回的图片下载到应用数据目录
//! （如 `%APPDATA%/com.icode.app/media/`），DB 中仅保存相对路径。
//!
//! 原因：供应商返回的图片 URL 为临时链接（如 SenseNova 固定 1 小时过期），
//! 不做本地化会导致历史记录无法再次查看。
//!
//! 目录结构：`{assets_dir}/{generation_id}/{index}.{ext}`
//!
//! 初始化：在 `main.rs` 的 `.setup()` 中调用 [`init_assets_dir`]，
//! 传入应用配置目录（与数据库同目录）。

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::{IcodeError, IcodeResult};

/// 媒体产物根目录（应用启动时初始化）
static ASSETS_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 初始化媒体产物根目录
///
/// 在 `main.rs` 的 `.setup()` 中调用，`base` 传入应用配置目录。
/// 产物存放于 `{base}/media/`，目录不存在时自动创建。
pub fn init_assets_dir(base: &Path) -> IcodeResult<()> {
    let dir = base.join("media");
    std::fs::create_dir_all(&dir)
        .map_err(|e| IcodeError::internal(format!("创建媒体产物目录失败：{e}")))?;
    let _ = ASSETS_DIR.set(dir);
    Ok(())
}

/// 获取媒体产物根目录
///
/// 未初始化时返回 `INTERNAL` 错误（仅发生在测试或未走正常启动流程的场景）。
pub fn assets_dir() -> IcodeResult<PathBuf> {
    ASSETS_DIR
        .get()
        .cloned()
        .ok_or_else(|| IcodeError::internal("媒体产物目录未初始化"))
}

/// 从相对路径解析产物绝对路径
///
/// 拒绝包含 `..` 的路径，防止目录穿越。
pub fn absolute_path(relative: &str) -> IcodeResult<PathBuf> {
    if relative.split(['/', '\\']).any(|seg| seg == "..") {
        return Err(IcodeError::validation("非法的媒体产物路径"));
    }
    Ok(assets_dir()?.join(relative))
}

/// 批量下载图片到产物目录
///
/// - `generation_id`：本次生成的历史记录 ID（用作子目录名）
/// - `urls`：供应商返回的图片 URL 列表
///
/// 返回产物相对路径（`{generation_id}/{index}.png`），按输入顺序排列。
/// 任一下载失败即整体返回错误（调用方将该次生成记为失败）。
pub async fn download_images(generation_id: &str, urls: &[String]) -> IcodeResult<Vec<String>> {
    if urls.is_empty() {
        return Err(IcodeError::validation("上游未返回任何图片 URL"));
    }
    let dir = assets_dir()?.join(generation_id);
    std::fs::create_dir_all(&dir)
        .map_err(|e| IcodeError::internal(format!("创建产物子目录失败：{e}")))?;

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| IcodeError::internal(format!("构建下载客户端失败：{e}")))?;

    let mut relative_paths = Vec::with_capacity(urls.len());
    for (index, url) in urls.iter().enumerate() {
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| IcodeError::internal(format!("下载产物失败（{url}）：{e}")))?;
        if !response.status().is_success() {
            return Err(IcodeError::internal(format!(
                "下载产物失败（{url}）：HTTP {}",
                response.status()
            )));
        }
        // 从 URL 或 Content-Type 猜测扩展名，默认 png
        // （需在 bytes() 消费 response 前读取响应头）
        let ext = guess_extension(url, response.headers());
        let bytes = response
            .bytes()
            .await
            .map_err(|e| IcodeError::internal(format!("读取产物内容失败（{url}）：{e}")))?;
        let file_name = format!("{index}.{ext}");
        let file_path = dir.join(&file_name);
        std::fs::write(&file_path, &bytes)
            .map_err(|e| IcodeError::internal(format!("写入产物文件失败：{e}")))?;
        relative_paths.push(format!("{generation_id}/{file_name}"));
    }
    Ok(relative_paths)
}

/// 读取产物文件内容（供前端 Base64 展示）
pub fn read_asset(relative: &str) -> IcodeResult<Vec<u8>> {
    let path = absolute_path(relative)?;
    match std::fs::read(&path) {
        Ok(bytes) => Ok(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(IcodeError::not_found("MediaAsset", Some(relative)))
        }
        Err(e) => Err(IcodeError::internal(format!("读取产物文件失败：{e}"))),
    }
}

/// 删除某次生成的全部产物文件
///
/// 仅删除记录的相对路径对应的文件与其子目录，目录不存在时静默忽略。
pub fn delete_assets(relative_paths: &[String]) {
    if relative_paths.is_empty() {
        return;
    }
    // 以首条路径的子目录为单位清理（同一 generation_id 共用一个子目录）
    let first = &relative_paths[0];
    if let Some(idx) = first.find(['/', '\\']) {
        let sub_dir = first[..idx].to_string();
        if let Ok(dir) = absolute_path(&sub_dir) {
            if dir.is_dir() {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }
}

/// 从 URL 与响应头猜测图片扩展名
fn guess_extension(url: &str, headers: &reqwest::header::HeaderMap) -> String {
    if let Some(ext) = url.rsplit('.').next() {
        let ext = ext.split(['?', '&']).next().unwrap_or(ext).to_lowercase();
        if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") && ext.len() <= 5 {
            return ext;
        }
    }
    if let Some(content_type) = headers.get(reqwest::header::CONTENT_TYPE) {
        let ct = content_type.to_str().unwrap_or("");
        if ct.contains("jpeg") {
            return "jpg".to_string();
        }
        if ct.contains("webp") {
            return "webp".to_string();
        }
        if ct.contains("png") {
            return "png".to_string();
        }
    }
    "png".to_string()
}
