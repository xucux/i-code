//! # WebDAV 客户端辅助
//!
//! 提供 WebDAV 备份所需的基本操作：列出目录、上传文件、下载文件、删除文件。
//! 使用 HTTP Basic Auth 认证。

use log;
use reqwest::{Client, Method, StatusCode};

use crate::error::IcodeError;
use crate::error::IcodeResult;

/// WebDAV 操作结果
#[derive(Debug, Clone)]
pub struct WebDavItem {
    pub path: String,
    pub display_name: String,
    pub content_length: u64,
    pub last_modified: String,
    #[expect(dead_code)]
    pub is_collection: bool,
}

/// 创建 reqwest 客户端
pub fn build_client() -> IcodeResult<Client> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| IcodeError::internal(format!("创建 HTTP 客户端失败: {e}")))
}

/// 规范化远程路径
///
/// 确保路径以 `/` 开头， remote_dir 以 `/` 结尾。
pub fn normalize_remote_path(remote_dir: &str, file_name: &str) -> String {
    let dir = if remote_dir.is_empty() {
        "/".to_string()
    } else {
        let mut d = remote_dir.to_string();
        if !d.starts_with('/') {
            d = format!("/{d}");
        }
        if !d.ends_with('/') {
            d.push('/');
        }
        d
    };
    format!("{dir}{file_name}")
}

/// 发送 PROPFIND 请求列出目录
pub async fn list_directory(
    client: &Client,
    base_url: &str,
    username: &str,
    password: &str,
    remote_path: &str,
) -> IcodeResult<Vec<WebDavItem>> {
    let url = ensure_collection_url(base_url, remote_path);
    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<D:propfind xmlns:D="DAV:">
  <D:prop>
    <D:displayname/>
    <D:getcontentlength/>
    <D:getlastmodified/>
    <D:resourcetype/>
  </D:prop>
</D:propfind>"#;

    log::info!(
        "WebDAV PROPFIND 开始: url={}, remote_path={}",
        redact_url(&url),
        remote_path
    );
    log::debug!("WebDAV PROPFIND 请求体: {body}");
    let start = std::time::Instant::now();

    let response = client
        .request(Method::from_bytes(b"PROPFIND").unwrap(), &url)
        .header("Depth", "1")
        .header("Content-Type", "text/xml; charset=utf-8")
        .basic_auth(username, Some(password))
        .body(body)
        .send()
        .await
        .map_err(map_network_error)?;

    let status = response.status();
    let duration_ms = start.elapsed().as_millis() as u64;
    let text = response.text().await.map_err(map_network_error)?;

    log::info!(
        "WebDAV PROPFIND 响应: url={}, status={}, duration={}ms",
        redact_url(&url),
        status,
        duration_ms
    );
    log::debug!("WebDAV PROPFIND 响应体: {text}");

    if status == StatusCode::UNAUTHORIZED {
        log::warn!("WebDAV PROPFIND 认证失败: url={}", redact_url(&url));
        return Err(IcodeError::unauthorized("WebDAV 认证失败，请检查用户名和密码"));
    }
    if !status.is_success() {
        log::warn!(
            "WebDAV PROPFIND 失败: url={}, status={}, body={}",
            redact_url(&url),
            status,
            text
        );
        return Err(IcodeError::internal(format!(
            "WebDAV PROPFIND 失败，状态码: {status}"
        )));
    }

    parse_propfind(&text)
}

/// 上传文件到 WebDAV
pub async fn upload_file(
    client: &Client,
    base_url: &str,
    username: &str,
    password: &str,
    remote_path: &str,
    data: Vec<u8>,
) -> IcodeResult<()> {
    let url = ensure_file_url(base_url, remote_path);
    let size = data.len();

    log::info!(
        "WebDAV PUT 开始: url={}, size={}bytes",
        redact_url(&url),
        size
    );
    let start = std::time::Instant::now();

    let response = client
        .put(&url)
        .basic_auth(username, Some(password))
        .body(data)
        .send()
        .await
        .map_err(map_network_error)?;

    let status = response.status();
    let duration_ms = start.elapsed().as_millis() as u64;
    let body = response.text().await.unwrap_or_default();

    log::info!(
        "WebDAV PUT 响应: url={}, status={}, duration={}ms",
        redact_url(&url),
        status,
        duration_ms
    );
    log::debug!("WebDAV PUT 响应体: {body}");

    if status == StatusCode::UNAUTHORIZED {
        log::warn!("WebDAV PUT 认证失败: url={}", redact_url(&url));
        return Err(IcodeError::unauthorized("WebDAV 认证失败，请检查用户名和密码"));
    }
    if status == StatusCode::INSUFFICIENT_STORAGE {
        log::warn!("WebDAV PUT 空间不足: url={}", redact_url(&url));
        return Err(IcodeError::internal("WebDAV 空间不足"));
    }
    if !status.is_success() {
        log::warn!(
            "WebDAV PUT 失败: url={}, status={}, body={}",
            redact_url(&url),
            status,
            body
        );
        return Err(IcodeError::internal(format!(
            "WebDAV 上传失败，状态码: {status}"
        )));
    }

    log::info!("WebDAV PUT 成功: url={}, size={}bytes", redact_url(&url), size);
    Ok(())
}

/// 从 WebDAV 下载文件
pub async fn download_file(
    client: &Client,
    base_url: &str,
    username: &str,
    password: &str,
    remote_path: &str,
) -> IcodeResult<Vec<u8>> {
    let url = ensure_file_url(base_url, remote_path);

    log::info!("WebDAV GET 开始: url={}", redact_url(&url));
    let start = std::time::Instant::now();

    let response = client
        .get(&url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(map_network_error)?;

    let status = response.status();
    let duration_ms = start.elapsed().as_millis() as u64;

    log::info!(
        "WebDAV GET 响应: url={}, status={}, duration={}ms",
        redact_url(&url),
        status,
        duration_ms
    );

    if status == StatusCode::UNAUTHORIZED {
        log::warn!("WebDAV GET 认证失败: url={}", redact_url(&url));
        return Err(IcodeError::unauthorized("WebDAV 认证失败，请检查用户名和密码"));
    }
    if status == StatusCode::NOT_FOUND {
        log::warn!("WebDAV GET 文件不存在: url={}", redact_url(&url));
        return Err(IcodeError::not_found("WebDAV 备份文件", Some(&url)));
    }
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        log::warn!(
            "WebDAV GET 失败: url={}, status={}, body={}",
            redact_url(&url),
            status,
            body
        );
        return Err(IcodeError::internal(format!(
            "WebDAV 下载失败，状态码: {status}"
        )));
    }

    response.bytes().await.map_err(map_network_error).map(|b| b.to_vec())
}

/// 删除 WebDAV 文件
pub async fn delete_file(
    client: &Client,
    base_url: &str,
    username: &str,
    password: &str,
    remote_path: &str,
) -> IcodeResult<()> {
    let url = ensure_file_url(base_url, remote_path);

    log::info!("WebDAV DELETE 开始: url={}", redact_url(&url));
    let start = std::time::Instant::now();

    let response = client
        .delete(&url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(map_network_error)?;

    let status = response.status();
    let duration_ms = start.elapsed().as_millis() as u64;
    let body = response.text().await.unwrap_or_default();

    log::info!(
        "WebDAV DELETE 响应: url={}, status={}, duration={}ms",
        redact_url(&url),
        status,
        duration_ms
    );
    log::debug!("WebDAV DELETE 响应体: {body}");

    if status == StatusCode::UNAUTHORIZED {
        log::warn!("WebDAV DELETE 认证失败: url={}", redact_url(&url));
        return Err(IcodeError::unauthorized("WebDAV 认证失败，请检查用户名和密码"));
    }
    if !status.is_success() && status != StatusCode::NOT_FOUND {
        log::warn!(
            "WebDAV DELETE 失败: url={}, status={}, body={}",
            redact_url(&url),
            status,
            body
        );
        return Err(IcodeError::internal(format!(
            "WebDAV 删除失败，状态码: {status}"
        )));
    }

    log::info!("WebDAV DELETE 成功: url={}", redact_url(&url));
    Ok(())
}

/// 创建 WebDAV 目录（幂等）
pub async fn ensure_directory(
    client: &Client,
    base_url: &str,
    username: &str,
    password: &str,
    remote_path: &str,
) -> IcodeResult<()> {
    let url = ensure_collection_url(base_url, remote_path);

    log::info!("WebDAV MKCOL 开始: url={}", redact_url(&url));
    let start = std::time::Instant::now();

    let response = client
        .request(Method::from_bytes(b"MKCOL").unwrap(), &url)
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(map_network_error)?;

    let status = response.status();
    let duration_ms = start.elapsed().as_millis() as u64;
    let body = response.text().await.unwrap_or_default();

    log::info!(
        "WebDAV MKCOL 响应: url={}, status={}, duration={}ms",
        redact_url(&url),
        status,
        duration_ms
    );
    log::debug!("WebDAV MKCOL 响应体: {body}");

    if status == StatusCode::UNAUTHORIZED {
        log::warn!("WebDAV MKCOL 认证失败: url={}", redact_url(&url));
        return Err(IcodeError::unauthorized("WebDAV 认证失败，请检查用户名和密码"));
    }
    // 201 Created 或 405 Method Not Allowed（目录已存在）均视为成功
    if !status.is_success() && status != StatusCode::METHOD_NOT_ALLOWED {
        log::warn!(
            "WebDAV MKCOL 失败: url={}, status={}, body={}",
            redact_url(&url),
            status,
            body
        );
        return Err(IcodeError::internal(format!(
            "WebDAV 创建目录失败，状态码: {status}"
        )));
    }

    log::info!("WebDAV MKCOL 成功（或目录已存在）: url={}", redact_url(&url));
    Ok(())
}

/// 确保返回集合 URL（以 `/` 结尾）
fn ensure_collection_url(base_url: &str, remote_path: &str) -> String {
    let base = normalize_base_url(base_url);
    let path = if remote_path.is_empty() || remote_path == "/" {
        "".to_string()
    } else {
        let mut p = remote_path.to_string();
        if !p.starts_with('/') {
            p = format!("/{p}");
        }
        if !p.ends_with('/') {
            p.push('/');
        }
        p
    };
    format!("{base}{path}")
}

/// 确保返回文件 URL（不以 `/` 结尾）
fn ensure_file_url(base_url: &str, remote_path: &str) -> String {
    let base = normalize_base_url(base_url);
    let path = if remote_path.is_empty() {
        "".to_string()
    } else {
        let mut p = remote_path.to_string();
        if !p.starts_with('/') {
            p = format!("/{p}");
        }
        p
    };
    format!("{base}{path}")
}

/// 规范化 base URL，去掉末尾 `/`
fn normalize_base_url(base_url: &str) -> String {
    let s = base_url.trim();
    if s.ends_with('/') {
        s[..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// 将网络错误映射为 IcodeError
fn map_network_error(e: reqwest::Error) -> IcodeError {
    IcodeError::internal(format!("WebDAV 网络错误: {e}"))
}

/// 对 WebDAV URL 进行脱敏，去除 user:pass 等敏感信息
///
/// 仅保留 scheme://host/path，用于日志输出。
fn redact_url(url: &str) -> String {
    // 简单字符串处理：去掉 "//" 之后到 "@" 之间的认证信息
    let mut s = url.to_string();
    if let Some(scheme_end) = s.find("://") {
        let after_scheme = &s[scheme_end + 3..];
        if let Some(at_pos) = after_scheme.find('@') {
            let host_path = &after_scheme[at_pos + 1..];
            s = format!("{}://{}", &s[..scheme_end], host_path);
        }
    }
    s
}

/// 简单解析 PROPFIND 响应 XML
///
/// 仅提取必要的 displayname / getcontentlength / getlastmodified / resourcetype。
fn parse_propfind(xml: &str) -> IcodeResult<Vec<WebDavItem>> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut items = Vec::new();
    let mut buf = Vec::new();
    let mut current_href = String::new();
    let mut current_display_name = String::new();
    let mut current_content_length = String::new();
    let mut current_last_modified = String::new();
    let mut current_is_collection = false;
    let mut in_resourcetype = false;
    let mut current_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                current_tag = local_name(&e);
                if current_tag == "resourcetype" {
                    in_resourcetype = true;
                }
            }
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "href" => current_href = text,
                    "displayname" => current_display_name = text,
                    "getcontentlength" => current_content_length = text,
                    "getlastmodified" => current_last_modified = text,
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = local_name(&e);
                if in_resourcetype && tag == "collection" {
                    current_is_collection = true;
                }
            }
            Ok(Event::End(e)) => {
                let tag = local_end_name(&e);
                if tag == "response" {
                    // 过滤掉目录本身（集合）
                    if !current_is_collection {
                        let content_length = current_content_length.parse::<u64>().unwrap_or(0);
                        let path = if current_href.starts_with('/') {
                            current_href.clone()
                        } else {
                            format!("/{}", current_href)
                        };
                        items.push(WebDavItem {
                            path,
                            display_name: current_display_name.clone(),
                            content_length,
                            last_modified: current_last_modified.clone(),
                            is_collection: current_is_collection,
                        });
                    }
                    current_href.clear();
                    current_display_name.clear();
                    current_content_length.clear();
                    current_last_modified.clear();
                    current_is_collection = false;
                    in_resourcetype = false;
                }
                if tag == "resourcetype" {
                    in_resourcetype = false;
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(IcodeError::internal(format!("解析 WebDAV PROPFIND 响应失败: {e}")));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(items)
}

/// 获取 XML 开始元素的本地标签名（去掉命名空间前缀）
fn local_name(e: &quick_xml::events::BytesStart<'_>) -> String {
    let binding = e.name();
    let name = std::str::from_utf8(binding.as_ref()).unwrap_or("");
    name.split(':').next_back().unwrap_or(name).to_string()
}

/// 获取 XML 结束元素的本地标签名（去掉命名空间前缀）
fn local_end_name(e: &quick_xml::events::BytesEnd<'_>) -> String {
    let binding = e.name();
    let name = std::str::from_utf8(binding.as_ref()).unwrap_or("");
    name.split(':').next_back().unwrap_or(name).to_string()
}
