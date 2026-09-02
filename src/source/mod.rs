pub mod archive;
pub mod catalog;
pub mod gamedir;
pub mod gameinfo;
pub mod manifest;
pub mod metaplugins;
pub mod paths;
pub mod vdf;

/// SourceMod layout relative to the mod dir (e.g. cstrike, garrysmod).
pub const SM_DIR: &str = "addons/sourcemod";
/// Compiled plugins. SourceMod loads every `*.smx` directly in here — one flat
/// file per plugin, unlike CounterStrikeSharp's per-plugin folders.
pub const SM_PLUGINS_DIR: &str = "addons/sourcemod/plugins";
/// Plugin configuration files (admins_simple.ini, per-plugin .cfg/.txt).
pub const SM_CONFIGS_DIR: &str = "addons/sourcemod/configs";
pub const SM_DATA_DIR: &str = "addons/sourcemod/data";
pub const SM_GAMEDATA_DIR: &str = "addons/sourcemod/gamedata";
pub const SM_TRANSLATIONS_DIR: &str = "addons/sourcemod/translations";
pub const SM_EXTENSIONS_DIR: &str = "addons/sourcemod/extensions";
pub const SM_LOGS_DIR: &str = "addons/sourcemod/logs";
/// Auto-generated per-plugin cvar configs. Lives at the MOD dir root, outside
/// addons/ — the one part of a SourceMod setup that does.
pub const SM_CFG_DIR: &str = "cfg/sourcemod";
/// SourceMod's own binaries; their absence means the tarball never unpacked.
pub const SM_BIN_DIR: &str = "addons/sourcemod/bin";
/// Where SourceMod itself parks a disabled plugin. Unlike CS2 this is
/// SourceMod's own convention, not a trick: the loader skips subdirectories.
pub const DISABLED_DIR_NAME: &str = "disabled";
/// Compiled SourcePawn plugin extension.
pub const SMX_EXT: &str = ".smx";
/// Per-plugin annotations (comments, groups). Panel-owned: SourceMod has no
/// in-game counterpart writing this file, so the shape is ours alone.
pub const META_MANIFEST: &str = "addons/sourcemod/configs/gameap/plugins_meta.json";
pub const METAMOD_DIR: &str = "addons/metamod";
/// The Valve server-plugin loader file that makes the engine start
/// Metamod:Source. The canonical Source 1 install method — unlike the
/// gameinfo.txt search path, game updates do not overwrite it.
pub const METAMOD_VDF: &str = "addons/metamod.vdf";
/// Alternative loader: a `Game <mod>/addons/metamod` line in the SearchPaths
/// block. Required by a few games and by Metamod 2.x; supported here for
/// detection so an existing install is not reported as broken.
pub const GAMEINFO_FILE: &str = "gameinfo.txt";
/// Metamod's own plugin list, one path per line (`;` comments it out). The
/// direct descendant of GoldSource Metamod's plugins.ini.
pub const METAPLUGINS_INI: &str = "addons/metamod/metaplugins.ini";
/// Snapshot tarballs. Deliberately NOT under any snapshot member dir, so a
/// restore cannot delete the snapshots it restores from.
pub const BACKUPS_DIR: &str = "addons/sourcemod/backups";
/// Scratch dir for node-side downloads, relative to the server root.
pub const DOWNLOAD_SCRATCH_DIR: &str = ".srcaddons";
