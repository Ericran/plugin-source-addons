//! Curated catalog of well-known SourceMod plugins and extensions, installable
//! straight from their GitHub releases. Kept deliberately small and static:
//! every entry here is a promise that the install pipeline can handle its
//! release layout.
//!
//! Most of the Source 1 ecosystem lives on the AlliedModders forums as bare
//! attachments with no version feed, so only projects that publish real GitHub
//! releases can appear here. Everything else installs through the Upload
//! dialog, which takes the same archives by hand.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// Ships one or more .smx plugins; installed rows carry its update badge.
    Plugin,
    /// A binary extension (addons/sourcemod/extensions). Plugins depend on it,
    /// but it never appears in the plugin table, so it gets no row badge.
    Extension,
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    /// Stable key used by the install route and the updates cache.
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// GitHub "owner/repo".
    pub repo: &'static str,
    pub kind: EntryKind,
    /// The .smx file stem the release installs — the row identity that carries
    /// the update badge. Empty for extensions.
    pub plugin: &'static str,
    /// Substrings the release asset name must contain, lowercase. Every
    /// pattern must match.
    pub asset_contains: &'static [&'static str],
    /// Games this is worth offering on, by GameAP game code. Empty means all.
    pub games: &'static [&'static str],
}

pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        key: "sourcebans-pp",
        name: "SourceBans++",
        description: "Web-backed bans, mutes and gags shared across a whole server group, with an admin web panel.",
        repo: "sbpp/sourcebans-pp",
        kind: EntryKind::Plugin,
        plugin: "sbpp_main",
        asset_contains: &["plugin"],
        games: &[],
    },
    CatalogEntry {
        key: "mapchooser-extended",
        name: "MapChooser Extended",
        description: "End-of-map votes, nominations, RTV and map groups - the standard replacement for SourceMod's bundled mapchooser.",
        repo: "Powerlord/sourcemod-mapchooser-extended",
        kind: EntryKind::Plugin,
        plugin: "mapchooser_extended",
        asset_contains: &[".zip"],
        games: &[],
    },
    CatalogEntry {
        key: "rtd",
        name: "Roll The Dice",
        description: "Classic fun plugin: players roll for a random timed perk. Highly configurable per game.",
        repo: "Phil25/RTD",
        kind: EntryKind::Plugin,
        plugin: "rtd",
        asset_contains: &[".zip"],
        games: &[],
    },
    CatalogEntry {
        key: "accelerator",
        name: "Accelerator",
        description: "Crash reporting extension: uploads minidumps so a repeating server crash can actually be traced.",
        repo: "asherkin/accelerator",
        kind: EntryKind::Extension,
        plugin: "",
        asset_contains: &["linux"],
        games: &[],
    },
    CatalogEntry {
        key: "steamworks",
        name: "SteamWorks",
        description: "Extension exposing the Steam API to plugins (HTTP requests, group membership, server tags). A dependency of many plugins.",
        repo: "KyleSanderson/SteamWorks",
        kind: EntryKind::Extension,
        plugin: "",
        asset_contains: &["linux"],
        games: &[],
    },
    CatalogEntry {
        key: "tf2items",
        name: "TF2Items",
        description: "Extension for rewriting item attributes on the fly - the base of most custom-weapon plugins.",
        repo: "asherkin/TF2Items",
        kind: EntryKind::Extension,
        plugin: "",
        asset_contains: &["linux"],
        games: &["tf2"],
    },
];

pub fn find(key: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|entry| entry.key == key)
}

/// Entries worth showing for a game code.
pub fn for_game(game_code: &str) -> impl Iterator<Item = &'static CatalogEntry> {
    let code = game_code.to_string();
    CATALOG.iter().filter(move |entry| {
        entry.games.is_empty() || entry.games.iter().any(|game| game.eq_ignore_ascii_case(&code))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_findable() {
        for entry in CATALOG {
            assert_eq!(find(entry.key).map(|e| e.repo), Some(entry.repo));
        }
        let mut keys: Vec<&str> = CATALOG.iter().map(|e| e.key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), CATALOG.len());
    }

    #[test]
    fn only_plugin_entries_claim_a_row() {
        for entry in CATALOG {
            match entry.kind {
                EntryKind::Plugin => assert!(!entry.plugin.is_empty(), "{}", entry.key),
                EntryKind::Extension => assert!(entry.plugin.is_empty(), "{}", entry.key),
            }
        }
    }

    #[test]
    fn game_specific_entries_are_filtered_out() {
        let tf2: Vec<&str> = for_game("tf2").map(|e| e.key).collect();
        let css: Vec<&str> = for_game("cssource").map(|e| e.key).collect();
        assert!(tf2.contains(&"tf2items"));
        assert!(!css.contains(&"tf2items"));
        assert!(css.contains(&"rtd"), "unrestricted entries show everywhere");
    }
}
