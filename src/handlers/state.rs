//! GET /servers/{id}/state — assembles the Metamod:Source/SourceMod picture of
//! a Source 1 server.

use std::collections::HashMap;

use crate::handlers::ctx::ServerCtx;
use crate::host_api::HostApi;
use crate::http::{ApiResult, json_response};
use crate::model::{
    MetamodPluginEntry, MetamodSource, MetamodState, SmPluginEntry, SourcemodState, StatePaths,
    StateResponse,
};
use crate::source::{self, gameinfo, metaplugins, paths, vdf};

pub fn handle<H: HostApi>(host: &mut H, params: &HashMap<String, String>) -> ApiResult {
    let ctx = ServerCtx::resolve(host, params)?;

    // --- Metamod:Source: the addons dir plus whichever loader wires it in ---
    let metamod_abs = paths::join(&ctx.mod_abs, source::METAMOD_DIR);
    let dir_present = host
        .stat(ctx.node_id, &metamod_abs)?
        .is_some_and(|s| s.is_dir);

    let vdf_abs = paths::join(&ctx.mod_abs, source::METAMOD_VDF);
    let vdf_present = match host.stat(ctx.node_id, &vdf_abs)? {
        Some(stat) if !stat.is_dir => {
            gameinfo::is_metamod_vdf(&host.download(ctx.node_id, &vdf_abs)?)
        }
        _ => false,
    };

    let gameinfo_abs = paths::join(&ctx.mod_abs, source::GAMEINFO_FILE);
    let gameinfo_wired = match host.stat(ctx.node_id, &gameinfo_abs)? {
        Some(stat) if !stat.is_dir => {
            gameinfo::is_gameinfo_wired(&host.download(ctx.node_id, &gameinfo_abs)?)
        }
        _ => false,
    };

    let metamod_plugins = collect_metamod_plugins(host, &ctx, &metamod_abs)?;
    // SourceMod is a Metamod plugin like any other: registered through its own
    // alias or a metaplugins.ini line, and dead without one.
    let sourcemod_registered = metamod_plugins
        .iter()
        .any(|entry| entry.platform && entry.enabled);

    // --- SourceMod ---
    let sm_abs = super::sm_abs(&ctx);
    let sm_installed = host.stat(ctx.node_id, &sm_abs)?.is_some_and(|s| s.is_dir);
    let bin_present = host
        .stat(ctx.node_id, &paths::join(&ctx.mod_abs, source::SM_BIN_DIR))?
        .is_some_and(|s| s.is_dir);

    let manifest = super::read_manifest(host, &ctx)?;

    // File scan: plugins/ (enabled) and plugins/disabled/ (disabled).
    let mut plugins: Vec<(String, bool, bool)> = Vec::new(); // (name, enabled, missing)
    collect_plugin_files(host, &ctx, &super::sm_plugins_abs(&ctx), true, &mut plugins)?;
    collect_plugin_files(host, &ctx, &super::sm_disabled_abs(&ctx), false, &mut plugins)?;

    // Manifest entries whose .smx has vanished — the "Missing" status.
    let seen: Vec<String> = plugins.iter().map(|(name, ..)| name.clone()).collect();
    for name in manifest.names() {
        if !seen.iter().any(|existing| existing.eq_ignore_ascii_case(name)) {
            plugins.push((name.to_string(), false, true));
        }
    }

    // Group display order: named groups alphabetically, ungrouped last.
    let mut group_names: Vec<String> = Vec::new();
    for (name, ..) in &plugins {
        if let Some(group) = manifest.group(name)
            && !group_names
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&group))
        {
            group_names.push(group);
        }
    }
    group_names.sort_by_key(|a| a.to_ascii_lowercase());

    let mut entries = Vec::new();
    for (name, enabled, missing) in plugins {
        let config_rel = find_config(host, &ctx, &name)?;
        let group = manifest.group(&name);
        let (group_index, group_title) = match &group {
            Some(title) => {
                let index = group_names
                    .iter()
                    .position(|existing| existing.eq_ignore_ascii_case(title))
                    .unwrap_or(group_names.len()) as u32;
                (index, Some(title.clone()))
            }
            None => (u32::MAX, None),
        };

        entries.push(SmPluginEntry {
            file: super::ctx::plugin_file_name(&name),
            comment: manifest.comment(&name),
            group,
            enabled,
            missing,
            has_config: config_rel.is_some(),
            config_path: config_rel,
            group_index,
            group_title,
            name,
        });
    }

    let response = StateResponse {
        server_id: ctx.server_id,
        game_code: ctx.game_code.clone(),
        engine: ctx.engine.clone(),
        engine_version: ctx.engine_version.clone(),
        mod_dir: ctx.mod_dir.clone(),
        paths: StatePaths {
            gameinfo: ctx.rel(source::GAMEINFO_FILE),
            metamod_dir: ctx.rel(source::METAMOD_DIR),
            metamod_vdf: ctx.rel(source::METAMOD_VDF),
            metaplugins_ini: ctx.rel(source::METAPLUGINS_INI),
            sm_dir: ctx.rel(source::SM_DIR),
            sm_plugins_dir: ctx.rel(source::SM_PLUGINS_DIR),
            sm_disabled_dir: ctx.rel(&paths::join(
                source::SM_PLUGINS_DIR,
                source::DISABLED_DIR_NAME,
            )),
            sm_configs_dir: ctx.rel(source::SM_CONFIGS_DIR),
            sm_cfg_dir: ctx.rel(source::SM_CFG_DIR),
            meta_manifest: ctx.rel(source::META_MANIFEST),
        },
        metamod: MetamodState {
            installed: dir_present && (vdf_present || gameinfo_wired),
            dir_present,
            vdf_present,
            gameinfo_wired,
            sourcemod_registered,
            plugins: metamod_plugins,
        },
        sourcemod: SourcemodState {
            installed: sm_installed,
            bin_present,
            plugins: entries,
        },
    };

    Ok(json_response(200, &response))
}

/// Both Metamod plugin mechanisms in one list: the `.vdf` aliases first, then
/// the metaplugins.ini lines.
pub(crate) fn collect_metamod_plugins<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    metamod_abs: &str,
) -> Result<Vec<MetamodPluginEntry>, crate::host_api::HostApiError> {
    let mut out: Vec<MetamodPluginEntry> = vdf::list(host, ctx.node_id, metamod_abs)?
        .into_iter()
        .map(|plugin| MetamodPluginEntry {
            platform: vdf::is_platform(&plugin.name),
            name: plugin.name,
            enabled: plugin.enabled,
            source: MetamodSource::Vdf,
            line: None,
            path: None,
        })
        .collect();

    let ini_abs = paths::join(&ctx.mod_abs, source::METAPLUGINS_INI);
    if host.stat(ctx.node_id, &ini_abs)?.is_some_and(|s| !s.is_dir) {
        let content = host.download(ctx.node_id, &ini_abs)?;
        for entry in metaplugins::parse(&content) {
            // A binary whose path points into addons/sourcemod is SourceMod's
            // own registration, whatever the file happens to be called.
            let platform = entry.path.to_ascii_lowercase().contains("addons/sourcemod")
                || vdf::is_platform(&entry.name);
            out.push(MetamodPluginEntry {
                name: entry.name,
                enabled: entry.enabled,
                platform,
                source: MetamodSource::Ini,
                line: Some(entry.line),
                path: Some(entry.path),
            });
        }
    }
    Ok(out)
}

/// Collects `(name, enabled, missing)` for `.smx` files directly inside
/// `dir_abs`. A file on disk is never "missing"; that state belongs to
/// manifest entries whose file is gone.
fn collect_plugin_files<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    dir_abs: &str,
    enabled: bool,
    out: &mut Vec<(String, bool, bool)>,
) -> Result<(), crate::host_api::HostApiError> {
    let Some(dir_entries) = host.read_dir(ctx.node_id, dir_abs)? else {
        return Ok(());
    };
    for entry in dir_entries {
        if entry.is_dir {
            continue;
        }
        if !entry.name.to_ascii_lowercase().ends_with(source::SMX_EXT) {
            continue;
        }
        let stem = paths::file_stem(&entry.name);
        if stem.is_empty() {
            continue;
        }
        out.push((stem.to_string(), enabled, false));
    }
    Ok(())
}

/// A plugin's editable config, if it has one. SourceMod auto-generates
/// `cfg/sourcemod/<name>.cfg` for any plugin declaring cvars; bigger plugins
/// ship a hand-written one under `addons/sourcemod/configs/` instead.
fn find_config<H: HostApi>(
    host: &mut H,
    ctx: &ServerCtx,
    name: &str,
) -> Result<Option<String>, crate::host_api::HostApiError> {
    for rel in [
        paths::join(source::SM_CFG_DIR, &format!("{name}.cfg")),
        paths::join(source::SM_CONFIGS_DIR, &format!("{name}.cfg")),
    ] {
        let abs = paths::join(&ctx.mod_abs, &rel);
        if host.stat(ctx.node_id, &abs)?.is_some_and(|s| !s.is_dir) {
            return Ok(Some(ctx.rel(&rel)));
        }
    }
    Ok(None)
}
