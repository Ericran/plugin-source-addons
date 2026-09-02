//! POST /servers/{id}/metamod/toggle — enable/disable a binary Metamod plugin.
//!
//! Two mechanisms, two edits: a `.vdf` alias is renamed to `.vdf.disabled`, a
//! `metaplugins.ini` line is commented out with `;`. The request says which.

use std::collections::HashMap;

use crate::handlers::ctx::{self, ServerCtx};
use crate::host_api::HostApi;
use crate::http::{ApiError, ApiResult, json_response, parse_json_body};
use crate::model::{MetamodToggleRequest, MetamodToggleResponse};
use crate::source::{self, metaplugins, paths, vdf};

const INI_PERMISSIONS: u32 = 0o644;

pub fn handle<H: HostApi>(
    host: &mut H,
    params: &HashMap<String, String>,
    body: &[u8],
    actor: Option<&str>,
) -> ApiResult {
    let context = ServerCtx::resolve(host, params)?;
    let request: MetamodToggleRequest = parse_json_body(body)?;

    let is_ini = request
        .source
        .as_deref()
        .is_some_and(|source| source.eq_ignore_ascii_case("ini"));

    let changed = if is_ini {
        toggle_ini(host, &context, &request)?
    } else {
        toggle_vdf(host, &context, &request)?
    };

    if changed {
        super::audit::record(
            host,
            context.server_id,
            actor,
            if request.enabled {
                "metamod-plugin-enable"
            } else {
                "metamod-plugin-disable"
            },
            &request.name,
        );
    }

    Ok(json_response(
        200,
        &MetamodToggleResponse {
            name: request.name,
            enabled: request.enabled,
            changed,
        },
    ))
}

/// Disabling SourceMod's own registration unloads the platform this tab
/// manages: every plugin stops and `sm` becomes an unknown console command. It
/// is still allowed, but never as an unremarkable row switch.
fn guard_platform(request: &MetamodToggleRequest, is_platform: bool) -> Result<(), ApiError> {
    if !request.enabled && !request.force && is_platform {
        return Err(ApiError::conflict(
            "PLATFORM_ENTRY",
            "this entry registers SourceMod itself; disabling it unloads every plugin and the sm console commands this tab relies on",
        ));
    }
    Ok(())
}

fn toggle_vdf<H: HostApi>(
    host: &mut H,
    context: &ServerCtx,
    request: &MetamodToggleRequest,
) -> Result<bool, ApiError> {
    ctx::sanitize_plugin_name(&request.name)?;
    guard_platform(request, vdf::is_platform(&request.name))?;

    let metamod_abs = paths::join(&context.mod_abs, source::METAMOD_DIR);
    let live_abs = paths::join(&metamod_abs, &format!("{}{}", request.name, vdf::VDF_EXT));
    let parked_abs = paths::join(
        &metamod_abs,
        &format!("{}{}", request.name, vdf::DISABLED_SUFFIX),
    );

    let live = host
        .stat(context.node_id, &live_abs)?
        .is_some_and(|s| !s.is_dir);
    let parked = host
        .stat(context.node_id, &parked_abs)?
        .is_some_and(|s| !s.is_dir);

    if !live && !parked {
        return Err(ApiError::not_found(
            "VDF_NOT_FOUND",
            format!("no {}.vdf in addons/metamod", request.name),
        ));
    }
    if live && parked {
        return Err(ApiError::conflict(
            "VDF_AMBIGUOUS",
            format!(
                "both {0}.vdf and {0}.vdf.disabled exist; delete one in the file manager",
                request.name
            ),
        ));
    }

    if live == request.enabled {
        return Ok(false);
    }
    let (from, to) = if request.enabled {
        (&parked_abs, &live_abs)
    } else {
        (&live_abs, &parked_abs)
    };
    host.move_path(context.node_id, from, to)?;
    Ok(true)
}

fn toggle_ini<H: HostApi>(
    host: &mut H,
    context: &ServerCtx,
    request: &MetamodToggleRequest,
) -> Result<bool, ApiError> {
    let line = request
        .line
        .ok_or_else(|| ApiError::bad_request("line is required for metaplugins.ini entries"))?;

    let ini_abs = paths::join(&context.mod_abs, source::METAPLUGINS_INI);
    if host
        .stat(context.node_id, &ini_abs)?
        .is_none_or(|s| s.is_dir)
    {
        return Err(ApiError::not_found(
            "METAPLUGINS_NOT_FOUND",
            "addons/metamod/metaplugins.ini does not exist",
        ));
    }
    let content = host.download(context.node_id, &ini_abs)?;

    let entry = metaplugins::parse(&content)
        .into_iter()
        .find(|entry| entry.line == line)
        .ok_or_else(|| {
            ApiError::not_found(
                "METAPLUGINS_LINE_NOT_FOUND",
                format!("metaplugins.ini line {line} is not a plugin entry (the file changed?)"),
            )
        })?;
    guard_platform(
        request,
        entry.path.to_ascii_lowercase().contains("addons/sourcemod")
            || vdf::is_platform(&entry.name),
    )?;

    let Some(patched) = metaplugins::set_enabled(&content, line, request.enabled) else {
        return Ok(false);
    };
    host.upload(context.node_id, &ini_abs, &patched, INI_PERMISSIONS)?;
    Ok(true)
}
