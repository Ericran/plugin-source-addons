//! POST /servers/{id}/platform/install — one-click install/update of
//! Metamod:Source or SourceMod.
//!
//! Both are downloaded BY THE NODE (curl/wget) and unpacked there (tar): the
//! drops run tens of megabytes, well past the panel's 10MB plugin-HTTP cap,
//! and the node-side path also keeps big buffers out of the wasm heap. Only
//! the release lookup goes through panel HTTP.
//!
//! Both AlliedModders drops are plain `.tar.gz` rooted at the mod dir, so one
//! `tar -xzf` into the mod directory is the whole install — no unzip tool
//! hunting, unlike the Source 2 equivalent.

use std::collections::HashMap;

use crate::handlers::ctx::ServerCtx;
use crate::host_api::HostApi;
use crate::http::{ApiError, ApiResult, json_response, parse_json_body};
use crate::model::{PlatformInstallRequest, PlatformInstallResponse};
use crate::source::{self, paths};

pub fn handle<H: HostApi>(
    host: &mut H,
    params: &HashMap<String, String>,
    body: &[u8],
    actor: Option<&str>,
) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;
    let request: PlatformInstallRequest = parse_json_body(body)?;

    if !ctx.node_os.is_empty() && !ctx.node_os.eq_ignore_ascii_case("linux") {
        return Err(ApiError::unprocessable(
            "LINUX_NODE_REQUIRED",
            "platform install downloads and unpacks on the node and supports linux nodes only",
        ));
    }

    if !matches!(request.kind.as_str(), "metamod" | "sourcemod") {
        return Err(ApiError::bad_request(format!(
            "unknown platform kind {:?}; expected \"metamod\" or \"sourcemod\"",
            request.kind
        )));
    }

    // Updates overwrite in place — keep a way back.
    super::snapshots::try_auto_snapshot(host, &ctx, actor, None);

    let (version, loader_written) = match request.kind.as_str() {
        "metamod" => install_metamod(host, &ctx)?,
        _ => install_sourcemod(host, &ctx)?,
    };

    super::audit::record(
        host,
        ctx.server_id,
        actor,
        "platform-install",
        &format!("{} {version}", request.kind),
    );

    Ok(json_response(
        200,
        &PlatformInstallResponse {
            kind: request.kind,
            version,
            loader_written,
        },
    ))
}

/// The daemon shellquote-splits exec commands and we cannot verify its quote
/// handling from here, so paths that would split wrong are refused outright
/// instead of corrupting a command: no shell, no games.
fn require_exec_safe(path: &str) -> Result<(), ApiError> {
    if path.chars().any(|c| c.is_whitespace() || c == '"' || c == '\'') {
        return Err(ApiError::unprocessable(
            "UNSAFE_PATH",
            format!(
                "server path {path:?} contains whitespace or quotes; node-side commands cannot handle it safely"
            ),
        ));
    }
    Ok(())
}

fn install_metamod<H: HostApi>(host: &mut H, ctx: &ServerCtx) -> Result<(String, bool), ApiError> {
    let release = super::updates::fetch_metamod_latest(host)
        .map_err(|err| ApiError::unprocessable("RELEASE_LOOKUP_FAILED", err))?;
    unpack_drop(host, ctx, &release.download_url, "mmsource.tar.gz")?;

    // The tarball ships addons/metamod; the loader vdf completes the install.
    let loader_written = match super::repair::repair_vdf(host, ctx) {
        Ok(changed) => changed,
        Err(err) => {
            host.log_error(&format!(
                "metamod loader vdf not written after install: {}",
                err.message
            ));
            false
        }
    };
    Ok((release.version, loader_written))
}

fn install_sourcemod<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
) -> Result<(String, bool), ApiError> {
    let release = super::updates::fetch_sourcemod_latest(host)
        .map_err(|err| ApiError::unprocessable("RELEASE_LOOKUP_FAILED", err))?;
    unpack_drop(host, ctx, &release.download_url, "sourcemod.tar.gz")?;

    let sm_abs = super::sm_abs(ctx);
    if !host.stat(ctx.node_id, &sm_abs)?.is_some_and(|s| s.is_dir) {
        return Err(ApiError::unprocessable(
            "EXTRACT_FAILED",
            "archive extracted but addons/sourcemod did not appear; unexpected release layout",
        ));
    }

    // SourceMod is a Metamod plugin: it needs Metamod loading first, and its
    // own alias present. The drop ships addons/metamod/sourcemod.vdf itself,
    // so only Metamod's loader may still be missing — and only if Metamod is
    // already installed, which repair_vdf checks for us.
    let loader_written = super::repair::repair_vdf(host, ctx).unwrap_or(false);
    Ok((release.version, loader_written))
}

/// Downloads an AlliedModders drop onto the node and untars it into the mod
/// directory, then removes the archive.
fn unpack_drop<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    url: &str,
    file_name: &str,
) -> Result<(), ApiError> {
    let archive_abs = download_to_node(host, ctx, url, file_name)?;
    let untar = format!("tar -xzf {archive_abs} -C {}", ctx.mod_abs);
    let result = host.exec(ctx.node_id, &untar, None)?;
    let _ = host.remove(ctx.node_id, &archive_abs, false);
    if result.exit_code != 0 {
        return Err(ApiError::unprocessable(
            "EXTRACT_FAILED",
            format!("tar failed (exit {}): {}", result.exit_code, result.output),
        ));
    }
    Ok(())
}

/// Downloads a URL into the server's scratch dir with curl, falling back to
/// wget. Returns the absolute archive path.
fn download_to_node<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    url: &str,
    file_name: &str,
) -> Result<String, ApiError> {
    require_exec_safe(&ctx.root_abs)?;
    require_exec_safe(&ctx.mod_abs)?;
    require_exec_safe(url)?;
    let scratch_abs = paths::join(&ctx.root_abs, source::DOWNLOAD_SCRATCH_DIR);
    if host.stat(ctx.node_id, &scratch_abs)?.is_none() {
        host.mk_dir(ctx.node_id, &scratch_abs)?;
    }
    let archive_abs = paths::join(&scratch_abs, file_name);

    let attempts = [
        format!("curl -fsSL --retry 2 -o {archive_abs} {url}"),
        format!("wget -q -O {archive_abs} {url}"),
    ];
    let mut last_failure = String::new();
    for command in &attempts {
        match host.exec(ctx.node_id, command, None) {
            Ok(result) if result.exit_code == 0 => {
                if host
                    .stat(ctx.node_id, &archive_abs)?
                    .is_some_and(|s| !s.is_dir && s.size > 0)
                {
                    return Ok(archive_abs);
                }
                last_failure = "download produced an empty file".into();
            }
            Ok(result) => {
                last_failure = format!("exit {}: {}", result.exit_code, result.output);
            }
            Err(err) => {
                last_failure = format!("{err:?}");
            }
        }
    }
    Err(ApiError::unprocessable(
        "DOWNLOAD_FAILED",
        format!("could not download on the node (tried curl and wget): {last_failure}"),
    ))
}
