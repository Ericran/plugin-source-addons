//! POST /servers/{id}/metamod/repair — make the engine load Metamod:Source.
//!
//! Writes `addons/metamod.vdf`, the loader manifest the official installer
//! generates. It names the Metamod binary matching what is actually on disk
//! (32- or 64-bit), and unlike the gameinfo.txt search path it is a file the
//! game itself never touches — so a game update cannot silently undo it.
//!
//! `{"gameinfo": true}` additionally patches gameinfo.txt, for the handful of
//! games and setups that need the search-path method instead.

use std::collections::HashMap;

use crate::handlers::ctx::ServerCtx;
use crate::host_api::HostApi;
use crate::http::{ApiError, ApiResult, json_response};
use crate::model::{RepairMetamodRequest, RepairMetamodResponse};
use crate::source::{self, gameinfo, paths};

const FILE_PERMISSIONS: u32 = 0o644;

pub fn handle<H: HostApi>(
    host: &mut H,
    params: &HashMap<String, String>,
    body: &[u8],
    actor: Option<&str>,
) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;
    // The body is optional: the tab's Fix button posts nothing at all.
    let request: RepairMetamodRequest = if body.is_empty() {
        RepairMetamodRequest::default()
    } else {
        crate::http::parse_json_body(body)?
    };

    let mut written: Vec<&str> = Vec::new();
    if repair_vdf(host, &ctx)? {
        written.push("vdf");
    }
    if request.gameinfo && repair_gameinfo(host, &ctx)? {
        written.push("gameinfo");
    }

    let method = written.join("+");
    if !written.is_empty() {
        super::audit::record(host, ctx.server_id, actor, "metamod-repair", &method);
    }

    Ok(json_response(
        200,
        &RepairMetamodResponse {
            changed: !written.is_empty(),
            method,
        },
    ))
}

/// Ensures `addons/metamod.vdf` exists and names the Metamod loader. Returns
/// whether a write happened. Shared with the platform installer and the
/// scheduled sweep.
pub fn repair_vdf<H: HostApi>(host: &mut H, ctx: &ServerCtx) -> Result<bool, ApiError> {
    let vdf_abs = paths::join(&ctx.mod_abs, source::METAMOD_VDF);
    if let Some(stat) = host.stat(ctx.node_id, &vdf_abs)?
        && !stat.is_dir
        && gameinfo::is_metamod_vdf(&host.download(ctx.node_id, &vdf_abs)?)
    {
        return Ok(false);
    }

    let metamod_abs = paths::join(&ctx.mod_abs, source::METAMOD_DIR);
    if !host.stat(ctx.node_id, &metamod_abs)?.is_some_and(|s| s.is_dir) {
        return Err(ApiError::unprocessable(
            "METAMOD_NOT_INSTALLED",
            "addons/metamod does not exist; install Metamod:Source before wiring it in",
        ));
    }

    let loader = pick_loader(host, ctx)?;
    let content = gameinfo::build_metamod_vdf(&ctx.mod_dir, loader);
    host.upload(ctx.node_id, &vdf_abs, &content, FILE_PERMISSIONS)?;
    Ok(true)
}

/// The loader path to write into the vdf. A 64-bit-only install (L4D2 and the
/// current TF2/CS:GO builds) keeps its binary one directory deeper, and naming
/// the wrong one leaves the engine silently starting no plugin at all.
fn pick_loader<H: HostApi>(host: &mut H, ctx: &ServerCtx) -> Result<&'static str, ApiError> {
    let has = |host: &mut H, rel: &str| -> Result<bool, ApiError> {
        let abs = paths::join(&ctx.mod_abs, rel);
        Ok(host.stat(ctx.node_id, &abs)?.is_some_and(|s| !s.is_dir))
    };
    let has_32 = has(host, "addons/metamod/bin/server.so")?
        || has(host, "addons/metamod/bin/server.dll")?;
    if has_32 {
        return Ok(gameinfo::LOADER_32);
    }
    let has_64 = has(host, "addons/metamod/bin/linux64/server.so")?
        || has(host, "addons/metamod/bin/win64/server.dll")?;
    if has_64 {
        return Ok(gameinfo::LOADER_64);
    }
    // Neither present (a partial unpack, or a layout we do not know): the
    // 32-bit path is what every stock Source 1 srcds looks for.
    Ok(gameinfo::LOADER_32)
}

/// Adds the Metamod search path to gameinfo.txt; returns whether a write
/// happened. Only reached when the caller explicitly asks for it.
pub fn repair_gameinfo<H: HostApi>(host: &mut H, ctx: &ServerCtx) -> Result<bool, ApiError> {
    let gameinfo_abs = paths::join(&ctx.mod_abs, source::GAMEINFO_FILE);
    if host
        .stat(ctx.node_id, &gameinfo_abs)?
        .is_none_or(|s| s.is_dir)
    {
        return Err(ApiError::unprocessable(
            "GAMEINFO_NOT_FOUND",
            "gameinfo.txt not found in the mod directory",
        ));
    }
    let content = host.download(ctx.node_id, &gameinfo_abs)?;
    if gameinfo::is_gameinfo_wired(&content) {
        return Ok(false);
    }
    let Some(patched) = gameinfo::wire_gameinfo(&content) else {
        return Err(ApiError::unprocessable(
            "GAMEINFO_UNPATCHABLE",
            "gameinfo.txt has no SearchPaths Game entry to anchor the Metamod line; edit it manually",
        ));
    };
    host.upload(ctx.node_id, &gameinfo_abs, &patched, FILE_PERMISSIONS)?;
    Ok(true)
}
