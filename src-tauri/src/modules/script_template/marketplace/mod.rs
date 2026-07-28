//! 脚本模板市场：只读拉取公共仓 catalog / 脚本，并映射为本地 create
//!
//! 公共仓：https://github.com/xucux/i-code-script-templates
//! 设计见 `docs/proposals/script-template-marketplace.md`

mod types;

pub use types::*;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Utc;
use once_cell::sync::Lazy;

use crate::error::{IcodeError, IcodeResult};
use crate::modules::shared;

use super::repository;
use super::types::{CreateScriptTemplateInput, ScriptTemplate, ScriptTemplateKind};

/// 进程内 catalog 缓存
struct CatalogCache {
    catalog: RemoteCatalog,
    fetched_at: Instant,
    etag: Option<String>,
}

static CATALOG_CACHE: Lazy<Mutex<Option<CatalogCache>>> = Lazy::new(|| Mutex::new(None));

fn catalog_url() -> String {
    format!("{}/{}", MARKETPLACE_BASE_URL.trim_end_matches('/'), CATALOG_PATH)
}

fn join_base(path: &str) -> String {
    let base = MARKETPLACE_BASE_URL.trim_end_matches('/');
    let p = path.trim_start_matches('/');
    format!("{base}/{p}")
}

fn build_http_client() -> IcodeResult<reqwest::Client> {
    let builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent(concat!("i-code/", env!("CARGO_PKG_VERSION")));
    shared::apply_global_proxy(builder)
        .build()
        .map_err(|e| IcodeError::internal(format!("创建市场 HTTP 客户端失败：{e}")))
}

/// 拉取文本，限制体积
async fn fetch_text(url: &str, max_bytes: usize) -> IcodeResult<(String, Option<String>)> {
    let client = build_http_client()?;
    log::info!("[marketplace] GET {url}");
    let resp = client.get(url).send().await.map_err(|e| {
        if e.is_timeout() {
            IcodeError::gateway(format!("市场请求超时：{e}"))
        } else if e.is_connect() {
            IcodeError::gateway(format!("无法连接脚本模板市场：{e}"))
        } else {
            IcodeError::gateway(format!("市场请求失败：{e}"))
        }
    })?;

    let status = resp.status();
    let etag = resp
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(IcodeError::not_found("MarketplaceResource", Some(url)));
    }
    if !status.is_success() {
        return Err(IcodeError::gateway(format!(
            "市场 HTTP {}：{}",
            status.as_u16(),
            url
        )));
    }

    let bytes = resp.bytes().await.map_err(|e| {
        IcodeError::gateway(format!("读取市场响应失败：{e}"))
    })?;
    if bytes.len() > max_bytes {
        return Err(IcodeError::validation(format!(
            "市场资源过大（{} > {} 字节）",
            bytes.len(),
            max_bytes
        )));
    }
    let text = String::from_utf8(bytes.to_vec())
        .map_err(|_| IcodeError::validation("市场资源不是合法 UTF-8 文本"))?;
    Ok((text, etag))
}

fn cache_is_fresh(cache: &CatalogCache) -> bool {
    cache.fetched_at.elapsed() < Duration::from_secs(DEFAULT_CACHE_TTL_SECS)
}

/// 获取 catalog（带内存缓存）
async fn load_catalog(force_refresh: bool) -> IcodeResult<(RemoteCatalog, bool)> {
    if !force_refresh {
        if let Ok(guard) = CATALOG_CACHE.lock() {
            if let Some(cache) = guard.as_ref() {
                if cache_is_fresh(cache) {
                    return Ok((cache.catalog.clone(), true));
                }
            }
        }
    }

    let url = catalog_url();
    let (text, etag) = fetch_text(&url, MAX_CATALOG_BYTES).await?;
    let catalog: RemoteCatalog = serde_json::from_str(&text).map_err(|e| {
        IcodeError::validation(format!("解析 catalog.json 失败：{e}"))
    })?;

    if catalog.schema_version < 1 {
        return Err(IcodeError::validation(format!(
            "不支持的 catalog schemaVersion: {}",
            catalog.schema_version
        )));
    }

    if let Ok(mut guard) = CATALOG_CACHE.lock() {
        *guard = Some(CatalogCache {
            catalog: catalog.clone(),
            fetched_at: Instant::now(),
            etag,
        });
    }

    Ok((catalog, false))
}

fn find_item<'a>(catalog: &'a RemoteCatalog, id: &str) -> IcodeResult<&'a RemoteCatalogItem> {
    catalog
        .items
        .iter()
        .find(|i| i.id == id)
        .ok_or_else(|| IcodeError::not_found("MarketplaceItem", Some(id)))
}

fn resolve_script_path(item: &RemoteCatalogItem) -> String {
    if let Some(p) = item.script_path.as_deref().filter(|s| !s.is_empty()) {
        return p.to_string();
    }
    if let Some(path) = item.path.as_deref().filter(|s| !s.is_empty()) {
        return format!("{}/script.rhai", path.trim_end_matches('/'));
    }
    // 回退：id = kind/slug
    format!("templates/{}/script.rhai", item.id.trim_start_matches('/'))
}

fn validate_marketplace_item(item: &RemoteCatalogItem) -> IcodeResult<()> {
    if item.slug.is_empty() || item.name.is_empty() {
        return Err(IcodeError::validation("市场模板 name/slug 不能为空"));
    }
    if ScriptTemplateKind::from_str(&item.kind).is_none()
        || !SUPPORTED_KINDS.contains(&item.kind.as_str())
    {
        return Err(IcodeError::validation(format!(
            "当前应用不支持市场模板类型: {}",
            item.kind
        )));
    }
    if item.engine != "rhai" {
        return Err(IcodeError::validation(format!(
            "不支持的脚本引擎: {}",
            item.engine
        )));
    }
    Ok(())
}

fn item_matches_keyword(item: &RemoteCatalogItem, kw: &str) -> bool {
    let kw = kw.to_lowercase();
    if item.name.to_lowercase().contains(&kw)
        || item.slug.to_lowercase().contains(&kw)
        || item.author.to_lowercase().contains(&kw)
        || item.id.to_lowercase().contains(&kw)
    {
        return true;
    }
    if let Some(desc) = &item.description {
        if desc.to_lowercase().contains(&kw) {
            return true;
        }
    }
    if let Some(tags) = &item.tags {
        if tags.iter().any(|t| t.to_lowercase().contains(&kw)) {
            return true;
        }
    }
    false
}

/// 列出市场模板
pub async fn list_marketplace(filter: MarketplaceListFilter) -> IcodeResult<MarketplaceListResult> {
    let (catalog, from_cache) = load_catalog(filter.force_refresh).await?;

    let mut items: Vec<MarketplaceItemSummary> = catalog
        .items
        .iter()
        .filter(|item| {
            // 仅展示客户端支持的 kind；未知 kind 隐藏（协议预留）
            SUPPORTED_KINDS.contains(&item.kind.as_str())
        })
        .filter(|item| {
            if let Some(kind) = filter.kind.as_deref().filter(|s| !s.is_empty() && *s != "all") {
                item.kind == kind
            } else {
                true
            }
        })
        .filter(|item| {
            if let Some(kw) = filter.keyword.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                item_matches_keyword(item, kw)
            } else {
                true
            }
        })
        .map(MarketplaceItemSummary::from)
        .collect();

    // 更新时间倒序，其次 name
    items.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.name.cmp(&b.name))
    });

    Ok(MarketplaceListResult {
        source: MARKETPLACE_REPO_URL.to_string(),
        generated_at: catalog.generated_at,
        items,
        fetched_at: Utc::now().to_rfc3339(),
        from_cache,
    })
}

/// 获取单条详情（可选附带脚本正文）
pub async fn get_marketplace_item(
    id: &str,
    include_script: bool,
) -> IcodeResult<MarketplaceItemDetail> {
    let (catalog, _) = load_catalog(false).await?;
    let item = find_item(&catalog, id)?;
    // 详情允许展示，但应用前仍会校验 kind
    let script_path = resolve_script_path(item);
    let script_body = if include_script {
        let url = join_base(&script_path);
        let (text, _) = fetch_text(&url, MAX_SCRIPT_BYTES).await?;
        Some(text)
    } else {
        None
    };

    let homepage = Some(format!(
        "{}/tree/main/{}",
        MARKETPLACE_REPO_URL,
        item.path
            .clone()
            .unwrap_or_else(|| format!("templates/{}", item.id))
    ));

    Ok(MarketplaceItemDetail {
        summary: MarketplaceItemSummary::from(item),
        script_body,
        script_path: Some(script_path),
        homepage,
    })
}

/// 只读预览脚本
pub async fn preview_marketplace_script(id: &str) -> IcodeResult<MarketplaceScriptPreview> {
    let detail = get_marketplace_item(id, true).await?;
    let body = detail.script_body.unwrap_or_default();
    Ok(MarketplaceScriptPreview {
        id: detail.summary.id,
        slug: detail.summary.slug,
        name: detail.summary.name,
        version: detail.summary.version,
        script_body: body,
    })
}

fn merge_description(item: &RemoteCatalogItem) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(d) = item.description.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(d.to_string());
    }
    let version = item.version.as_deref().unwrap_or("?");
    parts.push(format!(
        "Author: {} · Market: {}@{}",
        item.author, item.id, version
    ));
    parts.join("\n")
}

/// 解析 slug 冲突
fn resolve_slug(
    desired: &str,
    strategy: MarketplaceConflictStrategy,
) -> IcodeResult<String> {
    if repository::find_by_slug(desired)?.is_none() {
        return Ok(desired.to_string());
    }
    match strategy {
        MarketplaceConflictStrategy::Fail => Err(IcodeError::conflict(format!(
            "模板 slug '{desired}' 已存在"
        ))),
        MarketplaceConflictStrategy::Rename => {
            // deepseek-balance → deepseek-balance-2 …
            for n in 2..=99 {
                let candidate = format!("{desired}-{n}");
                if candidate.len() > 64 {
                    break;
                }
                if repository::find_by_slug(&candidate)?.is_none() {
                    return Ok(candidate);
                }
            }
            Err(IcodeError::conflict(format!(
                "无法为 slug '{desired}' 自动生成可用名称"
            )))
        }
    }
}

/// 从市场应用为本地 draft 模板
pub async fn apply_marketplace_item(
    input: MarketplaceApplyInput,
    create_fn: impl Fn(CreateScriptTemplateInput) -> IcodeResult<ScriptTemplate>,
    publish_fn: impl Fn(&str) -> IcodeResult<ScriptTemplate>,
) -> IcodeResult<ScriptTemplate> {
    let (catalog, _) = load_catalog(false).await?;
    let item = find_item(&catalog, &input.id)?;
    validate_marketplace_item(item)?;

    let script_path = resolve_script_path(item);
    let script_url = join_base(&script_path);
    let (script_body, _) = fetch_text(&script_url, MAX_SCRIPT_BYTES).await?;
    if script_body.trim().is_empty() {
        return Err(IcodeError::validation("市场模板脚本正文为空"));
    }

    let desired_slug = input
        .slug_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(item.slug.as_str());
    let slug = resolve_slug(desired_slug, input.conflict_strategy)?;

    let name = input
        .name_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(item.name.as_str())
        .to_string();

    let allowed_hosts_json = item.allowed_hosts.as_ref().and_then(|hosts| {
        if hosts.is_empty() {
            None
        } else {
            serde_json::to_string(hosts).ok()
        }
    });

    let timeout = item
        .default_timeout_ms
        .unwrap_or(15000)
        .max(1000);

    let create_input = CreateScriptTemplateInput {
        name,
        slug,
        kind: item.kind.clone(),
        description: Some(merge_description(item)),
        script_body,
        default_timeout_ms: timeout,
        allowed_hosts_json,
        snippet_id: Some(format!("marketplace:{}", item.id)),
        sort_order: 0,
    };

    let mut created = create_fn(create_input)?;
    if input.publish_after_create {
        created = publish_fn(&created.id)?;
    }
    Ok(created)
}
