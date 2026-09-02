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

/// The panel's per-call nodefs limit, PLUGINS_NODEFS_MAX_INLINE, at its default.
///
/// GameAP 4.5 caps what one gameap-nodefs download or upload may carry, because
/// the payload is materialized in panel memory and then copied into the guest.
/// The default is `32M`, and the panel's ByteSize parser reads every suffix as
/// binary (`M` = 1 << 20), so it is this number to the byte. Both comparisons
/// are strict: something exactly this size passes, one byte more is refused
/// outright rather than truncated.
///
/// Two call paths here are bounded by it, and both check against this constant
/// rather than keeping their own copy of "32MB" - the point is that they move
/// together if the panel's default ever does. Note the margin is exactly zero:
/// an operator who *lowers* PLUGINS_NODEFS_MAX_INLINE puts the panel's limit
/// below these gates, and the refusal then comes from the panel with a less
/// useful message. Nothing here can detect that - the panel does not report its
/// limit to the guest.
pub(crate) const PANEL_MAX_INLINE_BYTES: u64 = 32 * 1024 * 1024;

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
    // Pre-flight, before a single byte is written. The panel refuses a nodefs
    // upload over its inline limit, and these entries go up one at a time - so
    // discovering an oversized one halfway through would leave a half-installed
    // plugin, which is worse than refusing the archive outright. It is
    // reachable without the archive itself being oversized: extraction bounds
    // the total at MAX_TOTAL_UNCOMPRESSED, twice this limit, and nothing bounds
    // any single member.
    if let Some(entry) = entries
        .iter()
        .find(|entry| entry.data.len() as u64 > PANEL_MAX_INLINE_BYTES)
    {
        return Err(ApiError::unprocessable(
            "ENTRY_TOO_LARGE",
            format!(
                "{} is {} bytes, over the {}-byte limit for one file; unpack this archive with the file manager instead",
                entry.path,
                entry.data.len(),
                PANEL_MAX_INLINE_BYTES
            ),
        ));
    }

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
