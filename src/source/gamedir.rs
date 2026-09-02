//! Locating the Source 1 mod directory — the one holding gameinfo.txt, into
//! which addons/ is installed.
//!
//! Unlike Source 2 (always `game/<mod>`) a Source 1 server keeps its mod
//! directory directly in the server root, beside the engine content it
//! inherits. That content has a gameinfo.txt too: an hl2mp install ships
//! `hl2/gameinfo.txt` next to `hl2mp/gameinfo.txt`, and picking `hl2` would
//! install every addon into a directory the server never loads. So the search
//! is ordered — launch command, then game code, then a scan that skips the
//! known base-content directories.

/// Mod dir for a stock GameAP game code. These are the `-game` values from the
/// panel's own start commands.
pub fn known_mod_dir(game_code: &str) -> Option<&'static str> {
    match game_code {
        "cssource" | "cssv34" => Some("cstrike"),
        "csgo" => Some("csgo"),
        "dods" => Some("dod"),
        "garrysmod" => Some("garrysmod"),
        "hl2mp" => Some("hl2mp"),
        "l4d" => Some("left4dead"),
        "l4d2" => Some("left4dead2"),
        "tf2" => Some("tf"),
        "bms" => Some("bms"),
        "synergy" => Some("synergy"),
        _ => None,
    }
}

/// Directories that carry a gameinfo.txt but are engine/base content, never
/// the mod a server runs. Installing into one of these silently does nothing.
const BASE_CONTENT_DIRS: &[&str] = &[
    "hl2",
    "hl1",
    "hl1mp",
    "episodic",
    "ep2",
    "lostcoast",
    "platform",
    "bin",
    "core",
    "sourcetest",
    "hl2mp_lv",
];

/// Upper bound of subdirectories probed for gameinfo.txt.
pub const SCAN_CAP: usize = 40;

pub fn is_base_content_dir(name: &str) -> bool {
    BASE_CONTENT_DIRS
        .iter()
        .any(|known| name.eq_ignore_ascii_case(known))
}

/// The `-game <dir>` argument of a srcds launch command — the server's own
/// answer to which mod directory it runs, and the only one that is right for a
/// custom game entry.
///
/// Returns a single path segment only: the value is fed to nodefs, and a
/// launch command is operator-supplied text.
pub fn mod_dir_from_start_command(command: &str) -> Option<String> {
    let mut tokens = command.split_whitespace();
    while let Some(token) = tokens.next() {
        if !token.eq_ignore_ascii_case("-game") && !token.eq_ignore_ascii_case("+game") {
            continue;
        }
        let raw = tokens.next()?.trim_matches(['"', '\'']);
        if raw.is_empty()
            || raw.starts_with('-')
            || raw.starts_with('+')
            || raw.starts_with('{')
            || raw.contains(['/', '\\', '\0'])
            || raw == "."
            || raw == ".."
        {
            return None;
        }
        return Some(raw.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dirs() {
        assert_eq!(known_mod_dir("cssource"), Some("cstrike"));
        assert_eq!(known_mod_dir("cssv34"), Some("cstrike"));
        assert_eq!(known_mod_dir("tf2"), Some("tf"));
        assert_eq!(known_mod_dir("l4d2"), Some("left4dead2"));
        assert_eq!(known_mod_dir("dods"), Some("dod"));
        // Source 2 and the mislabeled non-Source entries stay unknown.
        assert_eq!(known_mod_dir("cs2"), None);
        assert_eq!(known_mod_dir("ark"), None);
    }

    #[test]
    fn reads_game_from_start_command() {
        assert_eq!(
            mod_dir_from_start_command(
                "./srcds_run -game cstrike -console -ip {ip} +map de_dust2"
            )
            .as_deref(),
            Some("cstrike")
        );
        assert_eq!(
            mod_dir_from_start_command("./srcds_run -console -game \"garrysmod\" +port 27015")
                .as_deref(),
            Some("garrysmod")
        );
        // An unsubstituted template var, a missing value or a path is not a hint.
        assert_eq!(mod_dir_from_start_command("./srcds_run -game {game}"), None);
        assert_eq!(mod_dir_from_start_command("./srcds_run -game"), None);
        assert_eq!(mod_dir_from_start_command("./srcds_run -game -console"), None);
        assert_eq!(mod_dir_from_start_command("./srcds_run -game ../etc"), None);
        assert_eq!(mod_dir_from_start_command("./run.sh -console"), None);
    }

    #[test]
    fn base_content_is_recognized() {
        assert!(is_base_content_dir("hl2"));
        assert!(is_base_content_dir("Platform"));
        assert!(!is_base_content_dir("cstrike"));
        assert!(!is_base_content_dir("hl2mp"));
    }
}
