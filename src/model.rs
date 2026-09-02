//! JSON request/response DTOs of the plugin API.

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct ToggleRequest {
    /// Plugin file stem inside addons/sourcemod/plugins (no .smx).
    pub name: String,
    pub enabled: bool,
}

#[derive(Deserialize, Debug)]
pub struct AddPluginRequest {
    /// Plugin file stem; `<name>.smx` must already be uploaded.
    pub name: String,
    /// Re-register an already known plugin instead of failing with 409.
    #[serde(default)]
    pub force: bool,
}

#[derive(Deserialize, Debug)]
pub struct RemovePluginRequest {
    pub name: String,
}

#[derive(Serialize, Debug)]
pub struct StateResponse {
    pub server_id: u64,
    pub game_code: String,
    pub engine: String,
    pub engine_version: String,
    /// Server-dir-relative mod directory, e.g. "cstrike".
    pub mod_dir: String,
    pub paths: StatePaths,
    pub metamod: MetamodState,
    pub sourcemod: SourcemodState,
}

/// All paths are relative to the server directory, ready to be passed to the
/// panel file-manager API as-is.
#[derive(Serialize, Debug)]
pub struct StatePaths {
    pub gameinfo: String,
    pub metamod_dir: String,
    pub metamod_vdf: String,
    pub metaplugins_ini: String,
    pub sm_dir: String,
    pub sm_plugins_dir: String,
    pub sm_disabled_dir: String,
    pub sm_configs_dir: String,
    /// cfg/sourcemod — the auto-generated per-plugin cvar configs.
    pub sm_cfg_dir: String,
    pub meta_manifest: String,
}

#[derive(Serialize, Debug)]
pub struct MetamodState {
    /// Present on disk AND wired into the engine.
    pub installed: bool,
    /// The addons/metamod directory exists.
    pub dir_present: bool,
    /// addons/metamod.vdf exists and names the Metamod loader.
    pub vdf_present: bool,
    /// gameinfo.txt carries the addons/metamod search path (the alternative
    /// install method; either one is enough).
    pub gameinfo_wired: bool,
    /// SourceMod is registered with Metamod — via addons/metamod/sourcemod.vdf
    /// or a live metaplugins.ini line. Without it SourceMod never loads, no
    /// matter how complete its own directory is.
    pub sourcemod_registered: bool,
    /// Binary Metamod plugins, from both `addons/metamod/*.vdf` aliases and
    /// `addons/metamod/metaplugins.ini` lines.
    pub plugins: Vec<MetamodPluginEntry>,
}

/// Which file registers a Metamod plugin — the two mechanisms need different
/// edits, so a row carries its own.
#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MetamodSource {
    Vdf,
    Ini,
}

#[derive(Serialize, Debug)]
pub struct MetamodPluginEntry {
    /// VDF file stem, or the binary's file stem for a metaplugins.ini line.
    pub name: String,
    /// The .vdf is live / the .ini line is not commented out.
    pub enabled: bool,
    /// This entry is SourceMod itself, not a plugin: switching it off unloads
    /// the platform. The frontend confirms before touching it.
    pub platform: bool,
    pub source: MetamodSource,
    /// Zero-based metaplugins.ini line, for ini entries only — the same binary
    /// may legitimately be listed twice, so the name is not a key.
    pub line: Option<usize>,
    /// The path as written in metaplugins.ini, for display.
    pub path: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct SourcemodState {
    /// addons/sourcemod exists.
    pub installed: bool,
    /// addons/sourcemod/bin exists — an unpacked, runnable install rather than
    /// a leftover configs directory.
    pub bin_present: bool,
    pub plugins: Vec<SmPluginEntry>,
}

#[derive(Serialize, Debug)]
pub struct SmPluginEntry {
    /// Plugin file stem — the stable identity of a SourceMod plugin, and what
    /// `sm plugins load/unload` takes.
    pub name: String,
    /// File name as it sits on disk, e.g. "funcommands.smx".
    pub file: String,
    /// The file is in plugins/ (not parked in plugins/disabled/).
    pub enabled: bool,
    /// Tracked in plugins_meta.json but the .smx is gone.
    pub missing: bool,
    /// Free-text note from plugins_meta.json.
    pub comment: Option<String>,
    /// Group name from plugins_meta.json.
    pub group: Option<String>,
    pub has_config: bool,
    /// Server-dir-relative path of the plugin's config file, when present.
    pub config_path: Option<String>,
    /// Index of the display group; ungrouped entries share one trailing "Other".
    pub group_index: u32,
    /// Header of the display group, `None` for the common "Other" group.
    pub group_title: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ToggleResponse {
    pub name: String,
    pub enabled: bool,
    pub changed: bool,
}

#[derive(Serialize, Debug)]
pub struct AddPluginResponse {
    pub name: String,
    /// True when the plugin was already known (force re-register).
    pub replaced: bool,
}

#[derive(Serialize, Debug)]
pub struct RemovePluginResponse {
    pub name: String,
    pub file_deleted: bool,
    pub entry_removed: bool,
}

#[derive(Deserialize, Debug)]
pub struct SetAttributesRequest {
    pub name: String,
    /// Full desired comment; `null`/absent clears it.
    #[serde(default)]
    pub comment: Option<String>,
    /// Full desired group; `null`/absent clears it.
    #[serde(default)]
    pub group: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct SetAttributesResponse {
    pub name: String,
    pub comment: Option<String>,
    pub group: Option<String>,
    pub changed: bool,
}

#[derive(Deserialize, Debug, Default)]
pub struct RepairMetamodRequest {
    /// Also add the search path to gameinfo.txt. Off by default: the vdf alone
    /// starts Metamod on every stock Source 1 game, and gameinfo.txt is a game
    /// file that updates overwrite.
    #[serde(default)]
    pub gameinfo: bool,
}

#[derive(Serialize, Debug)]
pub struct RepairMetamodResponse {
    /// False when Metamod was already wired and nothing was written.
    pub changed: bool,
    /// What was written: "vdf", "gameinfo", "vdf+gameinfo" or "" for a no-op.
    pub method: String,
}

#[derive(Deserialize, Debug)]
pub struct MetamodToggleRequest {
    /// VDF stem inside addons/metamod (no extension), or the binary's stem for
    /// a metaplugins.ini entry.
    pub name: String,
    pub enabled: bool,
    /// Which file registers it; defaults to the vdf alias.
    #[serde(default)]
    pub source: Option<String>,
    /// metaplugins.ini line number, required for `source: "ini"`.
    #[serde(default)]
    pub line: Option<usize>,
    /// Required to disable SourceMod's own registration, which is a
    /// platform-wide off switch rather than a per-plugin one.
    #[serde(default)]
    pub force: bool,
}

#[derive(Serialize, Debug)]
pub struct MetamodToggleResponse {
    pub name: String,
    pub enabled: bool,
    pub changed: bool,
}

#[derive(Serialize, Debug)]
pub struct LogsResponse {
    /// Server-dir-relative path of the log file that was read, if any.
    pub file: Option<String>,
    /// Which log was served: "errors" or "general".
    pub kind: String,
    pub lines: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct RestartResponse {
    pub restarted: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct UpdatesCache {
    /// Unix seconds of the last successful refresh.
    pub fetched_at: u64,
    pub metamod: Option<PlatformRelease>,
    pub sourcemod: Option<PlatformRelease>,
    /// Catalog key → latest known release.
    pub plugins: std::collections::BTreeMap<String, PluginRelease>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlatformRelease {
    pub version: String,
    pub download_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginRelease {
    pub version: String,
    pub release_url: String,
    pub download_url: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct UpdatesResponse {
    pub fetched_at: u64,
    pub stale: bool,
    pub metamod: Option<PlatformRelease>,
    pub sourcemod: Option<PlatformRelease>,
    /// Catalog entries with their latest release, keyed for row matching:
    /// entry.plugin is the .smx stem the release belongs to.
    pub plugins: Vec<PluginUpdateInfo>,
}

#[derive(Serialize, Debug)]
pub struct PluginUpdateInfo {
    pub key: String,
    pub plugin: String,
    pub version: String,
    pub release_url: String,
}

#[derive(Serialize, Debug)]
pub struct CatalogResponse {
    pub entries: Vec<CatalogEntryInfo>,
}

#[derive(Serialize, Debug)]
pub struct CatalogEntryInfo {
    pub key: String,
    pub name: String,
    pub description: String,
    pub homepage: String,
    /// "plugin" or "extension" — an extension installs but never gets a row.
    pub kind: String,
    /// The .smx stem the install creates (row identity), empty for extensions.
    pub plugin: String,
}

#[derive(Deserialize, Debug)]
pub struct CatalogInstallRequest {
    pub key: String,
}

#[derive(Serialize, Debug)]
pub struct CatalogInstallResponse {
    pub key: String,
    pub plugins: Vec<String>,
    pub version: String,
    pub files_written: u32,
}

#[derive(Deserialize, Debug)]
pub struct PlatformInstallRequest {
    /// "metamod" or "sourcemod".
    pub kind: String,
}

#[derive(Serialize, Debug)]
pub struct PlatformInstallResponse {
    pub kind: String,
    pub version: String,
    /// True when the Metamod loader was written as part of the install.
    pub loader_written: bool,
}

#[derive(Serialize, Debug)]
pub struct SnapshotInfo {
    pub name: String,
    /// Unix seconds parsed from the snapshot name.
    pub created_at: u64,
    pub size: u64,
    /// Server-dir-relative path (for file-manager download).
    pub path: String,
}

#[derive(Serialize, Debug)]
pub struct SnapshotListResponse {
    pub snapshots: Vec<SnapshotInfo>,
}

#[derive(Deserialize, Debug)]
pub struct SnapshotNameRequest {
    pub name: String,
}

#[derive(Serialize, Debug)]
pub struct SnapshotCreateResponse {
    pub snapshot: SnapshotInfo,
    /// Older snapshots deleted to honor the retention cap.
    pub pruned: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct SnapshotRestoreResponse {
    pub name: String,
    pub restored: bool,
}

#[derive(Deserialize, Debug)]
pub struct InstallArchiveRequest {
    /// Server-dir-relative path of the uploaded .zip or .tar.gz.
    pub path: String,
    /// Overwrite already-installed plugins instead of failing with 409.
    #[serde(default)]
    pub force: bool,
}

#[derive(Serialize, Debug)]
pub struct InstallArchiveResponse {
    /// Plugin file stems the archive created/updated.
    pub plugins: Vec<String>,
    pub files_written: u32,
}

#[derive(Serialize, Debug)]
pub struct DoctorCheck {
    pub id: String,
    /// "ok" | "warn" | "fail".
    pub status: String,
    pub detail: String,
}

#[derive(Serialize, Debug)]
pub struct DoctorResponse {
    pub checks: Vec<DoctorCheck>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuditEntry {
    /// Unix seconds.
    pub ts: u64,
    pub user: String,
    pub action: String,
    pub subject: String,
}

#[derive(Serialize, Debug)]
pub struct AuditResponse {
    pub entries: Vec<AuditEntry>,
}
