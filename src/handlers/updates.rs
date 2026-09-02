//! GET /servers/{id}/updates — latest upstream versions of Metamod:Source,
//! SourceMod and the catalog entries, cached in plugin storage.
//! `?refresh=1` forces a re-fetch; the nightly scheduled task does the same.

use std::collections::HashMap;

use gameap_plugin_sdk::proto::gameap::plugin as pb;

use crate::handlers::ctx::ServerCtx;
use crate::host_api::{HostApi, HttpFetchParams};
use crate::http::{ApiResult, json_response};
use crate::model::{PlatformRelease, PluginRelease, PluginUpdateInfo, UpdatesCache, UpdatesResponse};
use crate::source::catalog;

const CACHE_KEY: &str = "updates:v1";
const CACHE_TTL_SECONDS: u64 = 6 * 60 * 60;
const FETCH_TIMEOUT_SECONDS: i32 = 20;

/// AlliedModders publishes each stable branch as a directory of dated builds
/// plus a `*-latest-linux` pointer file naming the current one. 1.12 is the
/// current stable line of both projects for Source 1.
const MMSOURCE_BRANCH: &str = "1.12";
const SOURCEMOD_BRANCH: &str = "1.12";

fn mmsource_base() -> String {
    format!("https://mms.alliedmods.net/mmsdrop/{MMSOURCE_BRANCH}/")
}

fn sourcemod_base() -> String {
    format!("https://sm.alliedmods.net/smdrop/{SOURCEMOD_BRANCH}/")
}

pub fn handle<H: HostApi>(
    host: &mut H,
    params: &HashMap<String, String>,
    query: &HashMap<String, pb::QueryParamValues>,
) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;

    let force = query
        .get("refresh")
        .and_then(|v| v.values.first())
        .is_some_and(|v| v == "1" || v == "true");

    let cached = load_cache(host);
    let now = super::now_unix();
    let cache = match cached {
        Some(cache) if !force && now.saturating_sub(cache.fetched_at) < CACHE_TTL_SECONDS => cache,
        stale => match refresh_cache(host) {
            Ok(fresh) => fresh,
            Err(err) => {
                host.log_error(&format!("updates refresh failed: {err}"));
                stale.unwrap_or_default()
            }
        },
    };

    // Only entries offered for this game can be installed here, so only those
    // are worth badging.
    let plugins = catalog::for_game(&ctx.game_code)
        .filter(|entry| entry.kind == catalog::EntryKind::Plugin)
        .filter_map(|entry| {
            cache.plugins.get(entry.key).map(|release| PluginUpdateInfo {
                key: entry.key.to_string(),
                plugin: entry.plugin.to_string(),
                version: release.version.clone(),
                release_url: release.release_url.clone(),
            })
        })
        .collect();

    Ok(json_response(
        200,
        &UpdatesResponse {
            fetched_at: cache.fetched_at,
            stale: super::now_unix().saturating_sub(cache.fetched_at) >= CACHE_TTL_SECONDS,
            metamod: cache.metamod,
            sourcemod: cache.sourcemod,
            plugins,
        },
    ))
}

fn load_cache<H: HostApi>(host: &mut H) -> Option<UpdatesCache> {
    match host.storage_get(CACHE_KEY) {
        Ok(Some(bytes)) => serde_json::from_slice(&bytes).ok(),
        _ => None,
    }
}

/// Fetches everything and stores the cache. Individual sources may fail
/// without failing the sweep — a missing entry simply stays unknown.
pub fn refresh_cache<H: HostApi>(host: &mut H) -> Result<UpdatesCache, String> {
    let mut cache = UpdatesCache {
        fetched_at: super::now_unix(),
        metamod: None,
        sourcemod: None,
        plugins: Default::default(),
    };

    match fetch_metamod_latest(host) {
        Ok(release) => cache.metamod = Some(release),
        Err(err) => host.log_error(&format!("metamod version check failed: {err}")),
    }

    match fetch_sourcemod_latest(host) {
        Ok(release) => cache.sourcemod = Some(release),
        Err(err) => host.log_error(&format!("sourcemod version check failed: {err}")),
    }

    for entry in catalog::CATALOG {
        match fetch_github_latest(host, entry.repo, entry.asset_contains) {
            Ok(release) => {
                cache.plugins.insert(entry.key.to_string(), release);
            }
            Err(err) => host.log_error(&format!("{} version check failed: {err}", entry.key)),
        }
    }

    let bytes = serde_json::to_vec(&cache).map_err(|e| e.to_string())?;
    host.storage_set(CACHE_KEY, &bytes)
        .map_err(|e| format!("{e:?}"))?;
    Ok(cache)
}

pub fn fetch_metamod_latest<H: HostApi>(host: &mut H) -> Result<PlatformRelease, String> {
    fetch_alliedmods_drop(host, &mmsource_base(), "mmsource")
}

pub fn fetch_sourcemod_latest<H: HostApi>(host: &mut H) -> Result<PlatformRelease, String> {
    fetch_alliedmods_drop(host, &sourcemod_base(), "sourcemod")
}

/// `<base><product>-latest-linux` is a one-line text file holding the current
/// build's file name, e.g. "sourcemod-1.12.0-git7210-linux.tar.gz".
fn fetch_alliedmods_drop<H: HostApi>(
    host: &mut H,
    base: &str,
    product: &str,
) -> Result<PlatformRelease, String> {
    let pointer = format!("{base}{product}-latest-linux");
    let resp = host
        .http_fetch(&HttpFetchParams {
            method: "GET".into(),
            url: pointer.clone(),
            headers: default_headers(),
            timeout_seconds: FETCH_TIMEOUT_SECONDS,
        })
        .map_err(|e| format!("{e:?}"))?;
    if resp.status != 200 {
        return Err(format!("{product}-latest-linux returned HTTP {}", resp.status));
    }
    let file_name = String::from_utf8_lossy(&resp.body).trim().to_string();
    let prefix = format!("{product}-");
    if !file_name.starts_with(&prefix) || file_name.contains('/') {
        return Err(format!("unexpected latest-build answer: {file_name:?}"));
    }
    // sourcemod-1.12.0-git7210-linux.tar.gz → "1.12.0-git7210"
    let version = file_name
        .trim_start_matches(&prefix)
        .trim_end_matches(".tar.gz")
        .trim_end_matches("-linux")
        .to_string();
    Ok(PlatformRelease {
        version,
        download_url: format!("{base}{file_name}"),
    })
}

/// GitHub "latest release" lookup, picking the first asset whose lowercase
/// name contains every pattern.
pub fn fetch_github_latest<H: HostApi>(
    host: &mut H,
    repo: &str,
    asset_contains: &[&str],
) -> Result<PluginRelease, String> {
    let resp = host
        .http_fetch(&HttpFetchParams {
            method: "GET".into(),
            url: format!("https://api.github.com/repos/{repo}/releases/latest"),
            headers: default_headers(),
            timeout_seconds: FETCH_TIMEOUT_SECONDS,
        })
        .map_err(|e| format!("{e:?}"))?;
    if resp.status != 200 {
        return Err(format!("github api returned HTTP {}", resp.status));
    }
    let release: serde_json::Value =
        serde_json::from_slice(&resp.body).map_err(|e| format!("bad github response: {e}"))?;
    let version = release["tag_name"]
        .as_str()
        .ok_or("release has no tag_name")?
        .trim_start_matches('v')
        .to_string();
    let release_url = release["html_url"].as_str().unwrap_or_default().to_string();
    let download_url = release["assets"].as_array().and_then(|assets| {
        assets.iter().find_map(|asset| {
            let name = asset["name"].as_str()?.to_ascii_lowercase();
            // Only containers the in-plugin extractor understands.
            if !crate::source::archive::has_archive_extension(&name) {
                return None;
            }
            asset_contains
                .iter()
                .all(|pattern| name.contains(pattern))
                .then(|| asset["browser_download_url"].as_str().map(str::to_string))
                .flatten()
        })
    });
    Ok(PluginRelease {
        version,
        release_url,
        download_url,
    })
}

fn default_headers() -> Vec<(String, String)> {
    vec![
        // GitHub rejects requests without a User-Agent.
        ("User-Agent".into(), "gameap-source-addons".into()),
        ("Accept".into(), "application/vnd.github+json".into()),
    ]
}
