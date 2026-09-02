//! How the engine is told to start Metamod:Source, and how to repair it.
//!
//! Source 1 supports two install methods, and this module understands both:
//!
//! 1. **The VDF method** (`addons/metamod.vdf`) — a Valve server-plugin
//!    manifest naming Metamod's loader binary. This is what the official
//!    installer generates, it is the only file involved, and a game update
//!    does not touch it. Repairs write this one.
//! 2. **The gameinfo.txt method** — a `Game <mod>/addons/metamod` line in the
//!    SearchPaths block. Needed by a few games, and what Source 2 uses
//!    exclusively. Detected so an existing install is never reported broken,
//!    and patchable on request.
//!
//! Content is treated as lossy UTF-8, line-based; `//` comments are ignored.

/// The loader binary the VDF points at, relative to the mod dir. Written
/// without an extension: the engine appends `.so` / `.dll` itself.
pub const LOADER_32: &str = "addons/metamod/bin/server";
/// 64-bit games (L4D2, current TF2/CS:GO builds) load from here instead.
pub const LOADER_64: &str = "addons/metamod/bin/linux64/server";

/// Builds an `addons/metamod.vdf` naming `loader`.
///
/// The path is prefixed with `../<mod>/` exactly as the official generator
/// emits it: the engine resolves a plugin `file` against the srcds root, not
/// against the mod directory the vdf lives in.
pub fn build_metamod_vdf(mod_dir: &str, loader: &str) -> Vec<u8> {
    format!("\"Metamod Plugin\"\n{{\n\t\"file\"\t\"../{mod_dir}/{loader}\"\n}}\n").into_bytes()
}

/// True when the file looks like a Metamod loader manifest — a `file` value
/// pointing into `addons/metamod`. A vdf naming some other plugin is not one.
pub fn is_metamod_vdf(content: &[u8]) -> bool {
    let text = String::from_utf8_lossy(content).to_ascii_lowercase();
    text.replace('\\', "/").contains("addons/metamod/")
}

/// True when a non-comment line of gameinfo.txt adds the Metamod search path.
pub fn is_gameinfo_wired(content: &[u8]) -> bool {
    let text = String::from_utf8_lossy(content);
    for line in text.lines() {
        let line = match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        };
        if line.to_ascii_lowercase().contains("addons/metamod") {
            return true;
        }
    }
    false
}

/// Inserts the Metamod search path into the SearchPaths block, mimicking the
/// official install instructions: the line goes right before the first `Game`
/// entry, copying that entry's indentation. Returns `None` when the content is
/// already wired or has no SearchPaths block to patch (both cases mean
/// "nothing to write"; the caller distinguishes them via [`is_gameinfo_wired`]).
///
/// `|gameinfo_path|` is the Source 1 spelling — it resolves to the directory
/// gameinfo.txt itself sits in, so the line is mod-name independent.
pub fn wire_gameinfo(content: &[u8]) -> Option<Vec<u8>> {
    if is_gameinfo_wired(content) {
        return None;
    }
    let text = String::from_utf8_lossy(content).into_owned();
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };

    let mut in_search_paths = false;
    let mut entered_block = false;
    let mut depth_into_block = 0i32;
    let mut insert_at: Option<(usize, String)> = None; // (byte offset of line start, indentation)
    let mut offset = 0usize;

    for line in text.split_inclusive('\n') {
        let code = match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        };
        let trimmed = code.trim();
        if !in_search_paths {
            if trimmed.to_ascii_lowercase().starts_with("searchpaths") {
                in_search_paths = true;
                // Tolerate "SearchPaths {" on one line.
                depth_into_block += code.matches('{').count() as i32;
                if depth_into_block > 0 {
                    entered_block = true;
                }
            }
        } else {
            depth_into_block += code.matches('{').count() as i32;
            if depth_into_block > 0 {
                entered_block = true;
                // First Game entry inside the block anchors the insertion.
                let first_token = trimmed.split_whitespace().next().unwrap_or("");
                if first_token.eq_ignore_ascii_case("game") {
                    let indent: String = line
                        .chars()
                        .take_while(|c| *c == ' ' || *c == '\t')
                        .collect();
                    insert_at = Some((offset, indent));
                    break;
                }
            }
            depth_into_block -= code.matches('}').count() as i32;
            if entered_block && depth_into_block <= 0 {
                break; // SearchPaths block closed without a Game line
            }
        }
        offset += line.len();
    }

    let (at, indent) = insert_at?;
    let mut patched = String::with_capacity(text.len() + 64);
    patched.push_str(&text[..at]);
    patched.push_str(&indent);
    patched.push_str("Game\t|gameinfo_path|addons/metamod");
    patched.push_str(newline);
    patched.push_str(&text[at..]);
    Some(patched.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_loader_vdf() {
        let vdf = build_metamod_vdf("cstrike", LOADER_32);
        let text = String::from_utf8(vdf).expect("utf8");
        assert!(text.contains("\"Metamod Plugin\""));
        assert!(text.contains("\"file\"\t\"../cstrike/addons/metamod/bin/server\""));
        assert!(is_metamod_vdf(text.as_bytes()));

        let vdf64 = build_metamod_vdf("left4dead2", LOADER_64);
        let text64 = String::from_utf8(vdf64).expect("utf8");
        assert!(text64.contains("../left4dead2/addons/metamod/bin/linux64/server"));
    }

    #[test]
    fn recognizes_only_metamod_vdfs() {
        assert!(is_metamod_vdf(
            b"\"Metamod Plugin\"\n{\n\t\"file\"\t\"../tf/addons/metamod/bin/server\"\n}\n"
        ));
        // Windows-style separators from a hand-written file still count.
        assert!(is_metamod_vdf(b"\"file\" \"..\\tf\\addons\\metamod\\bin\\server\""));
        // Some other server plugin's manifest is not the Metamod loader.
        assert!(!is_metamod_vdf(
            b"\"Plugin\"\n{\n\t\"file\"\t\"addons/otherplugin/bin/server\"\n}\n"
        ));
        assert!(!is_metamod_vdf(b""));
    }

    #[test]
    fn detects_wired_gameinfo() {
        let gi = br#"
"GameInfo"
{
	game "Counter-Strike: Source"
	FileSystem
	{
		SearchPaths
		{
			Game	|gameinfo_path|addons/metamod
			Game	|gameinfo_path|.
			Game	cstrike
		}
	}
}
"#;
        assert!(is_gameinfo_wired(gi));
    }

    #[test]
    fn ignores_comments_and_absence() {
        assert!(!is_gameinfo_wired(b"Game cstrike\n"));
        assert!(!is_gameinfo_wired(b"// Game |gameinfo_path|addons/metamod\n"));
        assert!(is_gameinfo_wired(b"Game |gameinfo_path|Addons/Metamod // required\n"));
    }

    #[test]
    fn wires_before_first_game_entry() {
        let gi = b"\"GameInfo\"\n{\n\tFileSystem\n\t{\n\t\tSearchPaths\n\t\t{\n\t\t\tGame_LowViolence\tcstrike_lv\n\t\t\tGame\t|gameinfo_path|.\n\t\t\tGame\t|all_source_engine_paths|hl2\n\t\t}\n\t}\n}\n";
        let patched = wire_gameinfo(gi).expect("patches");
        let text = String::from_utf8(patched).expect("utf8");
        assert!(is_gameinfo_wired(text.as_bytes()));
        let mm_pos = text.find("addons/metamod").expect("inserted");
        let game_pos = text.find("Game\t|gameinfo_path|.\n").expect("original kept");
        assert!(mm_pos < game_pos, "metamod line must precede the mod's own path");
        // Indentation copied from the anchor line.
        assert!(text.contains("\t\t\tGame\t|gameinfo_path|addons/metamod\n"));
        // Game_LowViolence must NOT anchor the insert.
        assert!(text.find("Game_LowViolence").expect("lv kept") < mm_pos);
    }

    #[test]
    fn wire_preserves_crlf() {
        let gi = b"SearchPaths\r\n{\r\n\tGame\t|gameinfo_path|.\r\n}\r\n";
        let patched = wire_gameinfo(gi).expect("patches");
        let text = String::from_utf8(patched).expect("utf8");
        assert!(text.contains("\tGame\t|gameinfo_path|addons/metamod\r\n\tGame\t|gameinfo_path|.\r\n"));
    }

    #[test]
    fn wire_noop_when_wired_or_unpatchable() {
        assert!(wire_gameinfo(b"SearchPaths\n{\n\tGame\t|gameinfo_path|addons/metamod\n}\n").is_none());
        assert!(wire_gameinfo(b"\"GameInfo\"\n{\n}\n").is_none());
        assert!(wire_gameinfo(b"SearchPaths\n{\n}\n").is_none());
    }
}
