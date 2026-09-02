//! `addons/metamod/metaplugins.ini` — Metamod's own plugin list, one binary
//! path per line, `;` (or `//`) commenting a line out.
//!
//! This is the file the GoldSource plugin's `plugins.ini` grew into, and the
//! second of the two ways a Source 1 Metamod plugin gets loaded (the other
//! being a `.vdf` alias, see `vdf.rs`). Editing is line-preserving: unknown
//! lines, blank lines, comments and the original line endings all survive a
//! toggle, because operators keep notes in this file.

/// One managed line of metaplugins.ini.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IniPlugin {
    /// Display name: the binary's file stem, e.g. `stripper_mm_i486`.
    pub name: String,
    /// The path as written, without the comment marker.
    pub path: String,
    /// The line is live (not commented out).
    pub enabled: bool,
    /// Zero-based index into the file's lines — the identity used for toggles,
    /// since the same binary may legitimately appear more than once.
    pub line: usize,
}

fn strip_comment_marker(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    for marker in [";", "//", "#"] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            return Some(rest.trim());
        }
    }
    None
}

/// A payload line is a path to a binary. Prose in a comment must not be read
/// as a disabled plugin, so a commented line only counts when what follows
/// looks like one: a single token that ends in a shared-library extension.
fn looks_like_plugin_path(text: &str) -> bool {
    let text = text.trim().trim_matches('"');
    if text.is_empty() || text.split_whitespace().count() != 1 {
        return false;
    }
    let lower = text.to_ascii_lowercase();
    lower.ends_with(".so") || lower.ends_with(".dll") || lower.ends_with(".dylib")
}

fn stem_of(path: &str) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match file.rfind('.') {
        Some(idx) if idx > 0 => file[..idx].to_string(),
        _ => file.to_string(),
    }
}

/// Parses every plugin line, live and commented alike.
pub fn parse(content: &[u8]) -> Vec<IniPlugin> {
    let text = String::from_utf8_lossy(content);
    let mut plugins = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let (payload, enabled) = match strip_comment_marker(raw) {
            Some(rest) => (rest, false),
            None => (raw.trim(), true),
        };
        if !looks_like_plugin_path(payload) {
            continue;
        }
        let path = payload.trim().trim_matches('"').to_string();
        plugins.push(IniPlugin {
            name: stem_of(&path),
            path,
            enabled,
            line: index,
        });
    }
    plugins
}

/// Comments or uncomments the plugin line at `line`. Returns the new content,
/// or `None` when the line is already in the requested state or is not a
/// plugin line at all.
pub fn set_enabled(content: &[u8], line: usize, enabled: bool) -> Option<Vec<u8>> {
    let text = String::from_utf8_lossy(content).into_owned();
    let target = parse(content).into_iter().find(|entry| entry.line == line)?;
    if target.enabled == enabled {
        return None;
    }

    let mut out = String::with_capacity(text.len() + 2);
    for (index, raw) in text.split_inclusive('\n').enumerate() {
        if index != line {
            out.push_str(raw);
            continue;
        }
        // Keep whatever line ending this line already had.
        let (body, ending) = match raw.strip_suffix('\n') {
            Some(body) => match body.strip_suffix('\r') {
                Some(body) => (body, "\r\n"),
                None => (body, "\n"),
            },
            None => (raw, ""),
        };
        if enabled {
            let indent: String = body.chars().take_while(|c| c.is_whitespace()).collect();
            out.push_str(&indent);
            out.push_str(&target.path);
        } else {
            out.push_str("; ");
            out.push_str(body.trim_start());
        }
        out.push_str(ending);
    }
    Some(out.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"; Metamod plugin list\n\
; one path per line, relative to the mod dir\n\
addons/stripper/bin/stripper_mm_i486.so\n\
; addons/sourcemod/bin/sourcemod_mm_i486.so\n\
\n\
addons/nextmap/nextmap.so\n";

    #[test]
    fn parses_live_and_commented_lines() {
        let plugins = parse(SAMPLE);
        assert_eq!(plugins.len(), 3);
        assert_eq!(plugins[0].name, "stripper_mm_i486");
        assert!(plugins[0].enabled);
        assert_eq!(plugins[0].line, 2);
        assert_eq!(plugins[1].name, "sourcemod_mm_i486");
        assert!(!plugins[1].enabled);
        assert_eq!(plugins[2].name, "nextmap");
    }

    #[test]
    fn prose_comments_are_not_plugins() {
        // The two header lines are prose; only real paths become entries.
        let plugins = parse(b"; one path per line\n; TODO: re-enable stripper later\n");
        assert!(plugins.is_empty());
        // A quoted path still counts.
        let quoted = parse(b"\"addons/stripper/bin/stripper.so\"\n");
        assert_eq!(quoted.len(), 1);
        assert_eq!(quoted[0].name, "stripper");
    }

    #[test]
    fn toggles_off_and_back_on() {
        let off = set_enabled(SAMPLE, 2, false).expect("changes");
        let text = String::from_utf8(off.clone()).expect("utf8");
        assert!(text.contains("; addons/stripper/bin/stripper_mm_i486.so\n"));
        // Every other line survives verbatim.
        assert!(text.contains("; Metamod plugin list\n"));
        assert!(text.contains("addons/nextmap/nextmap.so\n"));
        assert!(!parse(&off)[0].enabled);

        let on = set_enabled(&off, 2, true).expect("changes back");
        assert_eq!(
            String::from_utf8(on).expect("utf8"),
            String::from_utf8(SAMPLE.to_vec()).expect("utf8")
        );
    }

    #[test]
    fn noop_and_unknown_line() {
        assert!(set_enabled(SAMPLE, 2, true).is_none()); // already enabled
        assert!(set_enabled(SAMPLE, 0, false).is_none()); // a prose comment
        assert!(set_enabled(SAMPLE, 99, false).is_none()); // past the end
    }

    #[test]
    fn preserves_crlf_and_indentation() {
        let crlf = b"\taddons/stripper/bin/stripper.so\r\n";
        let off = set_enabled(crlf, 0, false).expect("changes");
        assert_eq!(
            String::from_utf8(off.clone()).expect("utf8"),
            "; addons/stripper/bin/stripper.so\r\n"
        );
        let on = set_enabled(&off, 0, true).expect("changes back");
        assert_eq!(
            String::from_utf8(on).expect("utf8"),
            "addons/stripper/bin/stripper.so\r\n"
        );
    }
}
