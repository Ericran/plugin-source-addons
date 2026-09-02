// Mirrors of the Rust backend DTOs (src/model.rs).

export interface StatePaths {
    gameinfo: string;
    metamod_dir: string;
    metamod_vdf: string;
    metaplugins_ini: string;
    sm_dir: string;
    sm_plugins_dir: string;
    sm_disabled_dir: string;
    sm_configs_dir: string;
    sm_cfg_dir: string;
    meta_manifest: string;
}

export interface SmPluginEntry {
    name: string;
    file: string;
    enabled: boolean;
    missing: boolean;
    comment: string | null;
    group: string | null;
    has_config: boolean;
    config_path: string | null;
    group_index: number;
    group_title: string | null;
}

/** Which file registers a Metamod plugin — they need different edits. */
export type MetamodSource = 'vdf' | 'ini';

export interface MetamodPluginEntry {
    name: string;
    enabled: boolean;
    /** The SourceMod registration itself, not a plugin. */
    platform: boolean;
    source: MetamodSource;
    /** metaplugins.ini line, for ini entries only. */
    line: number | null;
    path: string | null;
}

export interface MetamodState {
    installed: boolean;
    dir_present: boolean;
    vdf_present: boolean;
    gameinfo_wired: boolean;
    sourcemod_registered: boolean;
    plugins: MetamodPluginEntry[];
}

export interface SourcemodState {
    installed: boolean;
    bin_present: boolean;
    plugins: SmPluginEntry[];
}

export interface StateResponse {
    server_id: number;
    game_code: string;
    engine: string;
    engine_version: string;
    mod_dir: string;
    paths: StatePaths;
    metamod: MetamodState;
    sourcemod: SourcemodState;
}

// Runtime info assembled from RCON output.

export interface PlatformVersion {
    build: string;
    version: string;
}

export interface RuntimePluginInfo {
    /** Display name as printed by `sm plugins list`. */
    name: string;
    /** Plugin index in the running server — what `sm plugins info <#>` takes. */
    index: number;
    /** File name, when the console gave one (it does for failed plugins). */
    file: string | null;
    version: string | null;
    author: string | null;
    /** Normalized runtime state. */
    status: 'running' | 'paused' | 'error';
    rawStatus: string;
}

/** Row model the plugin table renders. */
export interface PluginRow {
    key: string;
    /** Plugin file stem — the stable identity. */
    name: string;
    /** `<name>.smx`. */
    file: string;
    /** Human-oriented name: the runtime title, or a prettified file stem. */
    displayName: string;
    version: string | null;
    author: string | null;
    /** File is in plugins/ (not parked in plugins/disabled/). */
    enabled: boolean;
    /** plugins_meta.json note; editable. */
    comment: string | null;
    missing: boolean;
    runtime: RuntimePluginInfo | null;
    hasConfig: boolean;
    configPath: string | null;
    status: RowStatus;
    statusDetail: string | null;
    /** Display group id; ungrouped entries share one trailing "Other" group. */
    groupIndex: number;
    /** Display group header, `null` for the common "Other" group. */
    groupTitle: string | null;
}

export type RowStatus =
    | 'running'
    | 'paused'
    | 'enabled'
    | 'stopped'
    | 'pending'
    | 'error'
    | 'missing';

// New-feature DTOs (updates, catalog, snapshots, logs, audit).

export interface PlatformRelease {
    version: string;
    download_url: string;
}

export interface PluginUpdateInfo {
    key: string;
    /** .smx stem the release belongs to. */
    plugin: string;
    version: string;
    release_url: string;
}

export interface UpdatesResponse {
    fetched_at: number;
    stale: boolean;
    metamod: PlatformRelease | null;
    sourcemod: PlatformRelease | null;
    plugins: PluginUpdateInfo[];
}

export interface CatalogEntryInfo {
    key: string;
    name: string;
    description: string;
    homepage: string;
    kind: 'plugin' | 'extension';
    plugin: string;
}

export interface CatalogInstallResult {
    key: string;
    plugins: string[];
    version: string;
    files_written: number;
}

export interface PlatformInstallResult {
    kind: string;
    version: string;
    loader_written: boolean;
}

export interface RepairResult {
    changed: boolean;
    method: string;
}

export interface SnapshotInfo {
    name: string;
    created_at: number;
    size: number;
    /** Server-dir-relative path, usable with the file-manager download. */
    path: string;
}

export interface SnapshotCreateResult {
    snapshot: SnapshotInfo;
    pruned: string[];
}

export type LogKind = 'errors' | 'general';

export interface LogsResponse {
    file: string | null;
    kind: LogKind;
    lines: string[];
}

export interface AuditEntry {
    ts: number;
    user: string;
    action: string;
    subject: string;
}

export interface InstallArchiveResult {
    plugins: string[];
    files_written: number;
}

export interface DoctorCheck {
    id: string;
    status: 'ok' | 'warn' | 'fail';
    detail: string;
}

// Local mirror of the SDK's ServerData / ServerTabProps contract.
//
// Declared here rather than imported from @gameap/plugin-sdk so that
// `defineProps<ServerTabProps>()` compiles against the shipped plugin build: the
// Vue SFC compiler statically resolves the props type at build time, and the CI
// SDK build runs vite only (its `tsc --emitDeclarationOnly` step fails on
// @gameap/ui and emits no declarations), so the SDK's types are not on disk to
// resolve against.

export interface ServerData {
    id: number;
    uuid: string;
    name: string;
    game_id: string;
    game_mod_id: number;
    ip: string;
    port: number;
    query_port: number;
    rcon_port: number;
    enabled: boolean;
    installed: boolean;
    blocked: boolean;
    start_command: string;
    dir: string;
    process_active: boolean;
    last_process_check: string;
}

export interface ServerTabProps {
    serverId: number;
    server: ServerData;
    pluginId: string;
}
