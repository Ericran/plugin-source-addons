//! GET  /servers/{id}/catalog          — the curated catalog for this game
//! POST /servers/{id}/catalog/install  — download a release and install it
//!
//! Small release archives travel through the panel's plugin-HTTP (10MB cap)
//! and are unpacked in-plugin, so nothing beyond the daemon is required on the
//! node. Layout quirks are handled by `archive::detect_install_root`.

use std::collections::HashMap;

use crate::handlers::ctx::ServerCtx;
use crate::host_api::{HostApi, HttpFetchParams};
use crate::http::{ApiError, ApiResult, json_response, parse_json_body};
use crate::model::{
    CatalogEntryInfo, CatalogInstallRequest, CatalogInstallResponse, CatalogResponse,
};
use crate::source::archive;
use crate::source::catalog::{self, EntryKind};

const DOWNLOAD_TIMEOUT_SECONDS: i32 = 25;

fn kind_str(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Plugin => "plugin",
        EntryKind::Extension => "extension",
    }
}

pub fn handle_list<H: HostApi>(host: &mut H, params: &HashMap<String, String>) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;
    let entries = catalog::for_game(&ctx.game_code)
        .map(|entry| CatalogEntryInfo {
            key: entry.key.to_string(),
            name: entry.name.to_string(),
            description: entry.description.to_string(),
            homepage: format!("https://github.com/{}", entry.repo),
            kind: kind_str(entry.kind).to_string(),
            plugin: entry.plugin.to_string(),
        })
        .collect();
    Ok(json_response(200, &CatalogResponse { entries }))
}

pub fn handle_install<H: HostApi>(
    host: &mut H,
    params: &HashMap<String, String>,
    body: &[u8],
    actor: Option<&str>,
) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;
    let request: CatalogInstallRequest = parse_json_body(body)?;
    let entry = catalog::find(&request.key)
        .filter(|entry| catalog::for_game(&ctx.game_code).any(|shown| shown.key == entry.key))
        .ok_or_else(|| {
            ApiError::not_found(
                "CATALOG_KEY_UNKNOWN",
                "unknown catalog entry, or not offered for this game",
            )
        })?;

    super::require_sourcemod_installed(host, &ctx)?;

    let release = super::updates::fetch_github_latest(host, entry.repo, entry.asset_contains)
        .map_err(|err| ApiError::unprocessable("RELEASE_LOOKUP_FAILED", err))?;
    let download_url = release.download_url.clone().ok_or_else(|| {
        ApiError::unprocessable(
            "NO_MATCHING_ASSET",
            format!(
                "the latest {} release has no matching .zip or .tar.gz asset; install it from {} by hand via Upload",
                entry.name, release.release_url
            ),
        )
    })?;

    let resp = host
        .http_fetch(&HttpFetchParams {
            method: "GET".into(),
            url: download_url,
            headers: vec![("User-Agent".into(), "gameap-source-addons".into())],
            timeout_seconds: DOWNLOAD_TIMEOUT_SECONDS,
        })
        .map_err(|err| {
            ApiError::unprocessable(
                "DOWNLOAD_FAILED",
                format!(
                    "release download failed ({err:?}); note the panel caps plugin downloads at 10MB"
                ),
            )
        })?;
    if resp.status != 200 {
        return Err(ApiError::unprocessable(
            "DOWNLOAD_FAILED",
            format!("release download returned HTTP {}", resp.status),
        ));
    }

    let entries =
        archive::extract(&resp.body).map_err(|err| ApiError::unprocessable("BAD_ARCHIVE", err))?;
    let root = archive::detect_install_root(&entries)
        .map_err(|err| ApiError::unprocessable("BAD_ARCHIVE", err))?;

    // Reinstalls overwrite files — keep a way back.
    super::snapshots::try_auto_snapshot(host, &ctx, actor, None);

    let files_written = super::write_archive_entries(host, &ctx, &entries, &root)?;

    // Register whatever plugins the archive brought, like a manual upload
    // would, so the rows appear with metadata. An extension installs files but
    // adds no row.
    let plugins = archive::plugin_names(&entries, &root);
    if !plugins.is_empty() {
        let mut manifest = super::read_manifest(host, &ctx)?;
        for plugin in &plugins {
            manifest.ensure(plugin);
        }
        super::write_manifest(host, &ctx, &manifest)?;
    }

    super::audit::record(
        host,
        ctx.server_id,
        actor,
        "catalog-install",
        &format!("{} {}", entry.name, release.version),
    );

    Ok(json_response(
        200,
        &CatalogInstallResponse {
            key: entry.key.to_string(),
            plugins,
            version: release.version,
            files_written,
        },
    ))
}
