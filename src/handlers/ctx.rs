//! Shared request context: server → game (engine gate) → node → mod dir.

use std::collections::HashMap;

use crate::host_api::{HostApi, HostApiError, ServerInfo};
use crate::http::ApiError;
use crate::source::{gamedir, paths};

const SOURCE_ENGINE: &str = "source";

pub struct ServerCtx {
    pub server_id: u64,
    pub node_id: u64,
    /// Node operating system as reported by the panel ("linux", "windows").
    pub node_os: String,
    pub game_code: String,
    pub engine: String,
    pub engine_version: String,
    /// Absolute server root on the node.
    pub root_abs: String,
    /// Mod folder path relative to the server root, e.g. "cstrike".
    pub mod_dir: String,
    /// Absolute mod folder path — everything addon-related hangs off this.
    pub mod_abs: String,
}

impl ServerCtx {
    pub fn resolve<H: HostApi>(
        host: &mut H,
        params: &HashMap<String, String>,
    ) -> Result<ServerCtx, ApiError> {
        let server_id: u64 = params
            .get("id")
            .and_then(|raw| raw.parse().ok())
            .ok_or_else(|| ApiError::bad_request("invalid server id"))?;

        let server = host
            .get_server(server_id)?
            .ok_or_else(|| ApiError::not_found("SERVER_NOT_FOUND", "server not found"))?;

        let game = host
            .get_game(&server.game_code)?
            .ok_or_else(|| ApiError::not_found("GAME_NOT_FOUND", "game not found"))?;
        if !is_source_one(&game.engine, &game.engine_version) {
            return Err(ApiError::unprocessable(
                "UNSUPPORTED_ENGINE",
                format!(
                    "server engine is {:?} v{:?}, expected Source 1 (Source 2 servers are handled by the CS2 Addons plugin)",
                    game.engine, game.engine_version
                ),
            ));
        }

        let node = host
            .get_node(server.node_id)?
            .ok_or_else(|| ApiError::not_found("NODE_NOT_FOUND", "node not found"))?;

        let root_abs = paths::join(&node.work_path, &server.dir);
        let mod_dir = resolve_mod_dir(host, node.id, &root_abs, &server)?.ok_or_else(|| {
            ApiError::unprocessable(
                "MOD_DIR_NOT_FOUND",
                "could not locate the mod directory (gameinfo.txt) inside the server directory",
            )
        })?;
        let mod_abs = paths::join(&root_abs, &mod_dir);

        Ok(ServerCtx {
            server_id,
            node_id: node.id,
            node_os: node.os,
            game_code: server.game_code,
            engine: game.engine,
            engine_version: game.engine_version,
            root_abs,
            mod_dir,
            mod_abs,
        })
    }

    /// Path relative to the server dir, for the frontend file-manager API.
    pub fn rel(&self, mod_relative: &str) -> String {
        paths::join(&self.mod_dir, mod_relative)
    }
}

/// Source, but not Source 2. GameAP spells the engine version "1", "1.0", "4"
/// (Black Mesa) or "v34" depending on the game entry, so the rule is stated as
/// "not 2" rather than as a whitelist — a custom Source 1 game entry with an
/// unusual version string still works.
pub fn is_source_one(engine: &str, engine_version: &str) -> bool {
    if !engine.eq_ignore_ascii_case(SOURCE_ENGINE) {
        return false;
    }
    !engine_version.trim().starts_with('2')
}

/// Finds the directory holding gameinfo.txt, best evidence first:
/// the launch command's `-game`, then the game code's known mod dir, then a
/// scan that walks past the base-content directories a Source install ships.
fn resolve_mod_dir<H: HostApi>(
    host: &mut H,
    node_id: u64,
    root_abs: &str,
    server: &ServerInfo,
) -> Result<Option<String>, HostApiError> {
    let mut tried: Vec<String> = Vec::new();

    for hint in [
        gamedir::mod_dir_from_start_command(&server.start_command),
        gamedir::known_mod_dir(&server.game_code).map(str::to_string),
    ]
    .into_iter()
    .flatten()
    {
        if tried.iter().any(|seen| seen == &hint) {
            continue;
        }
        if has_gameinfo(host, node_id, root_abs, &hint)? {
            return Ok(Some(hint));
        }
        tried.push(hint);
    }

    let Some(entries) = host.read_dir(node_id, root_abs)? else {
        return Ok(None);
    };
    let candidates: Vec<String> = entries
        .into_iter()
        .filter(|entry| entry.is_dir)
        .map(|entry| entry.name)
        .take(gamedir::SCAN_CAP)
        .collect();

    // Two passes: real mod directories first, base content only as a last
    // resort. hl2/ has a gameinfo.txt in every Source 1 install, and installing
    // addons there is a silent no-op.
    for allow_base in [false, true] {
        for name in &candidates {
            if tried.iter().any(|seen| seen == name) {
                continue;
            }
            if gamedir::is_base_content_dir(name) != allow_base {
                continue;
            }
            if has_gameinfo(host, node_id, root_abs, name)? {
                return Ok(Some(name.clone()));
            }
        }
    }
    Ok(None)
}

fn has_gameinfo<H: HostApi>(
    host: &mut H,
    node_id: u64,
    root_abs: &str,
    dir: &str,
) -> Result<bool, HostApiError> {
    let gameinfo = paths::join(&paths::join(root_abs, dir), crate::source::GAMEINFO_FILE);
    Ok(host.stat(node_id, &gameinfo)?.is_some_and(|s| !s.is_dir))
}

/// Validates a plugin file stem coming from a request.
pub fn sanitize_plugin_name(name: &str) -> Result<(), ApiError> {
    paths::sanitize_file_name(name).map_err(ApiError::bad_request)?;
    if name.to_ascii_lowercase().ends_with(crate::source::SMX_EXT) {
        return Err(ApiError::bad_request(
            "pass the plugin name without the .smx extension",
        ));
    }
    if name.eq_ignore_ascii_case(crate::source::DISABLED_DIR_NAME) {
        return Err(ApiError::bad_request(
            "the disabled folder itself is not a plugin",
        ));
    }
    Ok(())
}

/// `<name>.smx`.
pub fn plugin_file_name(name: &str) -> String {
    format!("{name}{}", crate::source::SMX_EXT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_gate_accepts_source_one_only() {
        assert!(is_source_one("Source", "1"));
        assert!(is_source_one("source", "1.0"));
        assert!(is_source_one("Source", "4")); // Black Mesa
        assert!(is_source_one("Source", "v34")); // CS:S v34
        assert!(!is_source_one("source", "2")); // CS2
        assert!(!is_source_one("GoldSource", "1"));
        assert!(!is_source_one("unreal", "4"));
    }

    #[test]
    fn plugin_names_are_validated() {
        assert!(sanitize_plugin_name("funcommands").is_ok());
        // The .smx belongs to the file, not to the identity.
        assert!(sanitize_plugin_name("funcommands.smx").is_err());
        assert!(sanitize_plugin_name("disabled").is_err());
        assert!(sanitize_plugin_name("../etc").is_err());
        assert!(sanitize_plugin_name("").is_err());
        assert_eq!(plugin_file_name("rtd"), "rtd.smx");
    }
}
