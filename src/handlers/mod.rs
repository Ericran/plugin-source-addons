pub mod add;
pub mod archive_install;
pub mod attributes;
pub mod audit;
pub mod catalog_routes;
pub mod ctx;
pub mod doctor;
pub mod logs;
pub mod metamod;
pub mod platform;
pub mod remove;
pub mod repair;
pub mod restart;
pub mod snapshots;
pub mod state;
pub mod toggle;
pub mod updates;

#[cfg(test)]
mod tests;

use crate::host_api::{HostApi, HostApiError};
use crate::http::ApiError;
use crate::source::{self, archive::InstallRoot, manifest::Manifest, paths};

use ctx::ServerCtx;

const MANIFEST_FILE_PERMISSIONS: u32 = 0o644;

/// Unix seconds. wasm32-wasip1 backs this with the WASI clock; native tests
/// use the OS clock.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Absolute path of the SourceMod dir.
pub fn sm_abs(ctx: &ServerCtx) -> String {
    paths::join(&ctx.mod_abs, source::SM_DIR)
}

/// Absolute path of the plugins dir.
pub fn sm_plugins_abs(ctx: &ServerCtx) -> String {
    paths::join(&ctx.mod_abs, source::SM_PLUGINS_DIR)
}

/// Absolute path of the disabled-plugins parking dir.
pub fn sm_disabled_abs(ctx: &ServerCtx) -> String {
    paths::join(&sm_plugins_abs(ctx), source::DISABLED_DIR_NAME)
}

/// 409 unless addons/sourcemod exists on the server.
fn require_sourcemod_installed<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
) -> Result<(), ApiError> {
    if !host.stat(ctx.node_id, &sm_abs(ctx))?.is_some_and(|s| s.is_dir) {
        return Err(ApiError::conflict(
            "SOURCEMOD_NOT_INSTALLED",
            "SourceMod is not installed on this server",
        ));
    }
    Ok(())
}

/// Reads plugins_meta.json; a missing file yields an empty manifest.
fn read_manifest<H: HostApi>(host: &mut H, ctx: &ServerCtx) -> Result<Manifest, ApiError> {
    let manifest_abs = paths::join(&ctx.mod_abs, source::META_MANIFEST);
    if host.stat(ctx.node_id, &manifest_abs)?.is_none() {
        return Ok(Manifest::default());
    }
    let bytes = host.download(ctx.node_id, &manifest_abs)?;
    Ok(Manifest::parse(&bytes))
}

fn write_manifest<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    manifest: &Manifest,
) -> Result<(), ApiError> {
    let manifest_abs = paths::join(&ctx.mod_abs, source::META_MANIFEST);
    // configs/gameap/ does not exist on a fresh SourceMod install.
    let mut ensured = std::collections::BTreeSet::new();
    ensure_parent_dirs(host, ctx.node_id, &manifest_abs, &mut ensured)?;
    host.upload(
        ctx.node_id,
        &manifest_abs,
        &manifest.to_bytes(),
        MANIFEST_FILE_PERMISSIONS,
    )?;
    Ok(())
}

/// The plugin's .smx if it exists: (absolute path, currently enabled).
/// Enabled = directly in plugins/, disabled = in plugins/disabled/.
fn find_plugin_file<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    name: &str,
) -> Result<Option<(String, bool)>, HostApiError> {
    let file = ctx::plugin_file_name(name);
    let enabled_abs = paths::join(&sm_plugins_abs(ctx), &file);
    if host.stat(ctx.node_id, &enabled_abs)?.is_some_and(|s| !s.is_dir) {
        return Ok(Some((enabled_abs, true)));
    }
    let disabled_abs = paths::join(&sm_disabled_abs(ctx), &file);
    if host
        .stat(ctx.node_id, &disabled_abs)?
        .is_some_and(|s| !s.is_dir)
    {
        return Ok(Some((disabled_abs, false)));
    }
    Ok(None)
}

/// Where an extracted archive entry lands, per install root.
pub(crate) fn archive_target_abs(
    ctx: &ServerCtx,
    entry_path: &str,
    root: &InstallRoot,
) -> String {
    match root {
        InstallRoot::ModDir => paths::join(&ctx.mod_abs, entry_path),
        InstallRoot::SourcemodDir => paths::join(&sm_abs(ctx), entry_path),
        InstallRoot::PluginsDir => paths::join(&sm_plugins_abs(ctx), entry_path),
    }
}

/// Writes extracted archive entries to their install root. Shared by the
/// catalog installer and the archive-upload installer.
pub(crate) fn write_archive_entries<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    entries: &[source::archive::ArchiveEntry],
    root: &InstallRoot,
) -> Result<u32, ApiError> {
    let mut written = 0u32;
    let mut ensured: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for entry in entries {
        let target_abs = archive_target_abs(ctx, &entry.path, root);
        ensure_parent_dirs(host, ctx.node_id, &target_abs, &mut ensured)?;
        host.upload(ctx.node_id, &target_abs, &entry.data, entry.mode)?;
        written += 1;
    }
    Ok(written)
}

/// Creates every missing ancestor of `file_abs` (the daemon's mk_dir is not
/// guaranteed to be recursive). Already-known dirs are skipped via the set.
fn ensure_parent_dirs<H: HostApi>(
    host: &mut H,
    node_id: u64,
    file_abs: &str,
    ensured: &mut std::collections::BTreeSet<String>,
) -> Result<(), HostApiError> {
    let Some(parent_end) = file_abs.rfind('/') else {
        return Ok(());
    };
    let parent = &file_abs[..parent_end];
    if ensured.contains(parent) {
        return Ok(());
    }
    // Walk down from the shortest missing ancestor.
    let mut prefixes: Vec<&str> = Vec::new();
    let mut idx = parent.len();
    loop {
        let candidate = &parent[..idx];
        if ensured.contains(candidate) || host.stat(node_id, candidate)?.is_some() {
            break;
        }
        prefixes.push(candidate);
        match candidate.rfind('/') {
            Some(next) if next > 0 => idx = next,
            _ => break,
        }
    }
    for candidate in prefixes.into_iter().rev() {
        host.mk_dir(node_id, candidate)?;
        ensured.insert(candidate.to_string());
    }
    ensured.insert(parent.to_string());
    Ok(())
}

/// Moves a file via the daemon's native nodefs move, creating the destination
/// parent when missing. NEVER via execute_command: the daemon shellquote-splits
/// and execs commands directly — there is no shell, so `mkdir -p X && mv A B`
/// becomes one mkdir call that creates junk directories (including an empty one
/// AT the move target) and exits 0.
fn move_file<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    src_abs: &str,
    dst_abs: &str,
) -> Result<(), ApiError> {
    let dst_parent = match dst_abs.rfind('/') {
        Some(idx) => &dst_abs[..idx],
        None => return Err(ApiError::internal("move destination has no parent")),
    };
    if host.stat(ctx.node_id, dst_parent)?.is_none() {
        host.mk_dir(ctx.node_id, dst_parent)?;
    }
    host.move_path(ctx.node_id, src_abs, dst_abs)?;
    Ok(())
}
