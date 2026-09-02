//! POST /servers/{id}/plugins — register an uploaded plugin file.
//!
//! The frontend uploads `plugins/<name>.smx` through the panel file manager
//! first; this route confirms it landed and records the plugin in
//! plugins_meta.json so it survives deletion as a "Missing" entry — the
//! GoldSource plugins.ini registration, kept as an explicit step.

use std::collections::HashMap;

use crate::handlers::ctx::{self, ServerCtx};
use crate::host_api::HostApi;
use crate::http::{ApiError, ApiResult, json_response, parse_json_body};
use crate::model::{AddPluginRequest, AddPluginResponse};
use crate::source::paths;

pub fn handle<H: HostApi>(
    host: &mut H,
    params: &HashMap<String, String>,
    body: &[u8],
    actor: Option<&str>,
) -> ApiResult {
    let context = ServerCtx::resolve(host, params)?;
    let request: AddPluginRequest = parse_json_body(body)?;
    ctx::sanitize_plugin_name(&request.name)?;

    super::require_sourcemod_installed(host, &context)?;

    let file = ctx::plugin_file_name(&request.name);
    let smx_abs = paths::join(&super::sm_plugins_abs(&context), &file);
    if host.stat(context.node_id, &smx_abs)?.is_none_or(|s| s.is_dir) {
        return Err(ApiError::unprocessable(
            "FILE_NOT_UPLOADED",
            format!("{file} was not found in plugins/; upload the plugin first"),
        ));
    }

    let mut manifest = super::read_manifest(host, &context)?;
    let replaced = manifest.contains(&request.name);
    if replaced && !request.force {
        return Err(ApiError::conflict(
            "ALREADY_REGISTERED",
            format!("{} is already registered", request.name),
        ));
    }
    manifest.ensure(&request.name);
    super::write_manifest(host, &context, &manifest)?;

    super::audit::record(
        host,
        context.server_id,
        actor,
        if replaced { "plugin-update" } else { "plugin-install" },
        &request.name,
    );

    Ok(json_response(
        if replaced { 200 } else { 201 },
        &AddPluginResponse {
            name: request.name,
            replaced,
        },
    ))
}
