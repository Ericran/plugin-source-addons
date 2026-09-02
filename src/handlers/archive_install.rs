//! POST /servers/{id}/plugins/install-archive — install from an archive the
//! user uploaded via the panel file manager.
//!
//! The archive goes through the file manager (plugin HTTP bodies are capped at
//! 1MB, file-manager uploads are not); this route then pulls it off the node,
//! unpacks it in-wasm with the same layout detection the catalog uses, writes
//! the files, registers the plugins, and deletes the archive.

use std::collections::HashMap;

use crate::handlers::ctx::ServerCtx;
use crate::host_api::HostApi;
use crate::http::{ApiError, ApiResult, json_response, parse_json_body};
use crate::model::{InstallArchiveRequest, InstallArchiveResponse};
use crate::source::{archive, paths};

/// Archive size cap: the whole archive is inflated in wasm memory.
const MAX_ARCHIVE_BYTES: u64 = 32 * 1024 * 1024;

pub fn handle<H: HostApi>(
    host: &mut H,
    params: &HashMap<String, String>,
    body: &[u8],
    actor: Option<&str>,
) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;
    let request: InstallArchiveRequest = parse_json_body(body)?;
    paths::sanitize_rel_path(&request.path).map_err(ApiError::bad_request)?;
    if !archive::has_archive_extension(&request.path) {
        return Err(ApiError::bad_request("a .zip or .tar.gz archive is required"));
    }

    super::require_sourcemod_installed(host, &ctx)?;

    let archive_abs = paths::join(&ctx.root_abs, &request.path);
    let stat = host
        .stat(ctx.node_id, &archive_abs)?
        .filter(|s| !s.is_dir)
        .ok_or_else(|| {
            ApiError::not_found("ARCHIVE_NOT_FOUND", "uploaded archive not found on the server")
        })?;
    if stat.size > MAX_ARCHIVE_BYTES {
        let _ = host.remove(ctx.node_id, &archive_abs, false);
        return Err(ApiError::unprocessable(
            "ARCHIVE_TOO_LARGE",
            "archives over 32MB are not supported here; unpack via the file manager instead",
        ));
    }

    let bytes = host.download(ctx.node_id, &archive_abs)?;
    let result = install_bytes(host, &ctx, &bytes, request.force, actor);
    // The uploaded archive is scratch — except on a conflict, where the
    // frontend asks the user and retries with force against the same upload.
    if !matches!(&result, Err(err) if err.status == 409) {
        let _ = host.remove(ctx.node_id, &archive_abs, false);
    }
    let (plugins, files_written) = result?;

    // An archive that installs only an extension names no plugin, so the audit
    // line falls back to the file the operator uploaded.
    let subject = if plugins.is_empty() {
        paths::file_name(&request.path).to_string()
    } else {
        plugins.join(", ")
    };
    super::audit::record(host, ctx.server_id, actor, "plugin-install-archive", &subject);

    Ok(json_response(200, &InstallArchiveResponse { plugins, files_written }))
}

fn install_bytes<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    bytes: &[u8],
    force: bool,
    actor: Option<&str>,
) -> Result<(Vec<String>, u32), ApiError> {
    let entries =
        archive::extract(bytes).map_err(|err| ApiError::unprocessable("BAD_ARCHIVE", err))?;
    let root = archive::detect_install_root(&entries)
        .map_err(|err| ApiError::unprocessable("BAD_ARCHIVE", err))?;
    let plugins = archive::plugin_names(&entries, &root);

    if !force {
        let manifest = super::read_manifest(host, ctx)?;
        for plugin in &plugins {
            let exists =
                super::find_plugin_file(host, ctx, plugin)?.is_some() || manifest.contains(plugin);
            if exists {
                return Err(ApiError::conflict(
                    "ALREADY_REGISTERED",
                    format!("{plugin} is already installed"),
                ));
            }
        }
    }

    // Overwrites are reversible: snapshot first, best-effort.
    super::snapshots::try_auto_snapshot(host, ctx, actor, None);

    let files_written = super::write_archive_entries(host, ctx, &entries, &root)?;

    if !plugins.is_empty() {
        let mut manifest = super::read_manifest(host, ctx)?;
        for plugin in &plugins {
            manifest.ensure(plugin);
        }
        super::write_manifest(host, ctx, &manifest)?;
    }

    Ok((plugins, files_written))
}
